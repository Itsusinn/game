//! Portable compute ray tracing; no GPU ray-query extensions or game dependencies.
use crate::{bvh::Bvh, Camera, Mesh, Uniform};
use bytemuck::{Pod, Zeroable};

/// Diagnostic modes share the same lighting computation and exposure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LightingMode {
    #[default]
    Global,
    Direct,
    Indirect,
    Legacy,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    camera: Uniform,
    previous: Uniform,
    viewport: [f32; 4],
    old_viewport: [f32; 4],
    size: [u32; 4],
    scene: [u32; 4],
}

fn sampling_stride(width: u32, height: u32, binding_limit: u64) -> u32 {
    let mut stride = 2;
    while (u64::from(width.max(1).div_ceil(stride))
        * u64::from(height.max(1).div_ceil(stride))
        * 64)
        .next_power_of_two()
        > binding_limit
    {
        stride *= 2;
    }
    stride
}

struct Storage {
    buffer: wgpu::Buffer,
    capacity: u64,
}
impl Storage {
    fn new(device: &wgpu::Device, size: u64, label: &str) -> Self {
        let capacity = size.max(256).next_power_of_two();
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: capacity,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
        }
    }
    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        fixed: &[u8],
        moving: &[u8],
        dirty: bool,
    ) {
        let size = (fixed.len() + moving.len()) as u64;
        let grow = size > self.capacity;
        if grow {
            *self = Self::new(device, size, "ray scene");
        }
        if (grow || dirty) && !fixed.is_empty() {
            queue.write_buffer(&self.buffer, 0, fixed);
        }
        if !moving.is_empty() {
            queue.write_buffer(&self.buffer, fixed.len() as u64, moving);
        }
    }
}

pub(crate) struct Lighting {
    pub positions: wgpu::TextureView,
    pub normals: wgpu::TextureView,
    pub albedo: wgpu::TextureView,
    params: Params,
    uniform: wgpu::Buffer,
    display: wgpu::Buffer,
    nodes: Storage,
    triangles: Storage,
    cache: [Storage; 2],
    history: [Storage; 2],
    scene: Bvh,
    actors: Bvh,
    actor_bytes: Vec<u8>,
    dirty: bool,
    index: usize,
    update: wgpu::ComputePipeline,
    gather: wgpu::ComputePipeline,
    compose: wgpu::RenderPipeline,
    mode: LightingMode,
    srgb: bool,
}

fn texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
fn buffer_entry(
    binding: u32,
    uniform: bool,
    read_only: bool,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: if uniform {
                wgpu::BufferBindingType::Uniform
            } else {
                wgpu::BufferBindingType::Storage { read_only }
            },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn texture_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}
fn layout(device: &wgpu::Device, entries: &[wgpu::BindGroupLayoutEntry]) -> wgpu::PipelineLayout {
    let group = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("lighting bindings"),
        entries,
    });
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("lighting layout"),
        bind_group_layouts: &[Some(&group)],
        immediate_size: 0,
    })
}
fn bind_buffer(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}
fn bind_texture(binding: u32, view: &wgpu::TextureView) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(view),
    }
}

impl Lighting {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let c = wgpu::ShaderStages::COMPUTE;
        let f = wgpu::ShaderStages::FRAGMENT;
        let update_layout = layout(
            device,
            &[
                buffer_entry(0, true, true, c),
                buffer_entry(1, false, true, c),
                buffer_entry(2, false, true, c),
                buffer_entry(3, false, true, c),
                buffer_entry(4, false, false, c),
            ],
        );
        let gather_layout = layout(
            device,
            &[
                buffer_entry(0, true, true, c),
                buffer_entry(1, false, true, c),
                buffer_entry(2, false, true, c),
                buffer_entry(3, false, true, c),
                texture_entry(5, c),
                texture_entry(6, c),
                buffer_entry(7, false, true, c),
                buffer_entry(8, false, false, c),
            ],
        );
        let compose_layout = layout(
            device,
            &[
                buffer_entry(0, true, true, f),
                texture_entry(1, f),
                texture_entry(2, f),
                texture_entry(3, f),
                buffer_entry(4, false, true, f),
                buffer_entry(5, true, true, f),
            ],
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kajiya-inspired diffuse transport"),
            source: wgpu::ShaderSource::Wgsl(include_str!("lighting.wgsl").into()),
        });
        let compute = |entry: &str, layout: &wgpu::PipelineLayout| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let update = compute("update_cache", &update_layout);
        let gather = compute("gather", &gather_layout);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bilateral lighting resolve"),
            source: wgpu::ShaderSource::Wgsl(include_str!("compose.wgsl").into()),
        });
        let compose = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lighting resolve"),
            layout: Some(&compose_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_fullscreen"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_compose"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = |size, label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let mut result = Self {
            positions: texture(
                device,
                width,
                height,
                wgpu::TextureFormat::Rgba32Float,
                "world position",
            ),
            normals: texture(
                device,
                width,
                height,
                wgpu::TextureFormat::Rgba16Float,
                "world normal",
            ),
            albedo: texture(
                device,
                width,
                height,
                wgpu::TextureFormat::Rgba8Unorm,
                "surface albedo",
            ),
            params: Params::zeroed(),
            uniform: uniform(std::mem::size_of::<Params>() as u64, "lighting parameters"),
            display: uniform(16, "lighting display"),
            nodes: Storage::new(device, 256, "ray nodes"),
            triangles: Storage::new(device, 256, "ray triangles"),
            cache: std::array::from_fn(|_| Storage::new(device, 256, "irradiance cache")),
            history: std::array::from_fn(|_| Storage::new(device, 256, "diffuse history")),
            scene: Bvh::default(),
            actors: Bvh::default(),
            actor_bytes: Vec::new(),
            dirty: true,
            index: 0,
            update,
            gather,
            compose,
            mode: LightingMode::Global,
            srgb: format.is_srgb(),
        };
        result.resize_history(device, width, height);
        result
    }
    fn resize_history(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.params.size = [width.max(1), height.max(1), 0, 0];
        let limits = device.limits();
        let stride = sampling_stride(
            width,
            height,
            limits
                .max_storage_buffer_binding_size
                .min(limits.max_buffer_size),
        );
        self.params.scene[0] = stride;
        let count =
            u64::from(width.max(1).div_ceil(stride)) * u64::from(height.max(1).div_ceil(stride));
        self.history = std::array::from_fn(|_| Storage::new(device, count * 64, "diffuse history"));
    }
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.positions = texture(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba32Float,
            "world position",
        );
        self.normals = texture(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            "world normal",
        );
        self.albedo = texture(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba8Unorm,
            "surface albedo",
        );
        self.resize_history(device, width, height);
    }
    pub fn set_mode(&mut self, mode: LightingMode) {
        if self.mode != mode {
            self.params.size[3] = 0;
        }
        self.mode = mode;
    }
    pub fn mode(&self) -> LightingMode {
        self.mode
    }
    pub fn world(&mut self, device: &wgpu::Device, mesh: &Mesh) {
        self.scene = Bvh::build(mesh, 0, 0);
        self.cache = std::array::from_fn(|_| {
            Storage::new(
                device,
                self.scene.triangles.len() as u64 * 16,
                "irradiance cache",
            )
        });
        self.params.size[2] = 0;
        self.params.size[3] = 0;
        self.dirty = true;
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        actors: &Mesh,
        viewport: [f32; 4],
    ) {
        self.params.previous = self.params.camera;
        self.params.old_viewport = self.params.viewport;
        self.params.camera = camera.uniform();
        self.params.viewport = viewport;
        if viewport != self.params.old_viewport {
            self.params.size[3] = 0;
        }
        let bytes = bytemuck::cast_slice(&actors.vertices);
        let changed = bytes != self.actor_bytes;
        if changed || self.dirty {
            self.actors = Bvh::build(
                actors,
                self.scene.nodes.len() as u32,
                self.scene.triangles.len() as u32,
            );
            self.actor_bytes.clear();
            self.actor_bytes.extend_from_slice(bytes);
            self.nodes.upload(
                device,
                queue,
                bytemuck::cast_slice(&self.scene.nodes),
                bytemuck::cast_slice(&self.actors.nodes),
                self.dirty,
            );
            self.triangles.upload(
                device,
                queue,
                bytemuck::cast_slice(&self.scene.triangles),
                bytemuck::cast_slice(&self.actors.triangles),
                self.dirty,
            );
        }
        self.params.scene = [
            self.params.scene[0],
            (self.scene.nodes.len() + self.actors.nodes.len()) as u32,
            self.scene.triangles.len() as u32,
            u32::from(changed),
        ];
        self.dirty = false;
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&self.params));
        let mode = match self.mode {
            LightingMode::Direct => 1u32,
            LightingMode::Indirect => 2,
            _ => 0,
        };
        queue.write_buffer(
            &self.display,
            0,
            bytemuck::cast_slice(&[mode, u32::from(self.srgb), 0, 0]),
        );
    }
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        viewport: [f32; 4],
    ) {
        let previous = self.index;
        let next = 1 - previous;
        let update = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cache update"),
            layout: &self.update.get_bind_group_layout(0),
            entries: &[
                bind_buffer(0, &self.uniform),
                bind_buffer(1, &self.nodes.buffer),
                bind_buffer(2, &self.triangles.buffer),
                bind_buffer(3, &self.cache[previous].buffer),
                bind_buffer(4, &self.cache[next].buffer),
            ],
        });
        let gather = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diffuse gather"),
            layout: &self.gather.get_bind_group_layout(0),
            entries: &[
                bind_buffer(0, &self.uniform),
                bind_buffer(1, &self.nodes.buffer),
                bind_buffer(2, &self.triangles.buffer),
                bind_buffer(3, &self.cache[next].buffer),
                bind_texture(5, &self.positions),
                bind_texture(6, &self.normals),
                bind_buffer(7, &self.history[previous].buffer),
                bind_buffer(8, &self.history[next].buffer),
            ],
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("recurrent irradiance cache"),
                ..Default::default()
            });
            pass.set_pipeline(&self.update);
            pass.set_bind_group(0, &update, &[]);
            pass.dispatch_workgroups(
                (self.scene.triangles.len() as u32).div_ceil(64).max(1),
                1,
                1,
            );
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("diffuse final gather and reprojection"),
                ..Default::default()
            });
            pass.set_pipeline(&self.gather);
            pass.set_bind_group(0, &gather, &[]);
            pass.dispatch_workgroups(
                self.params.size[0]
                    .div_ceil(self.params.scene[0])
                    .div_ceil(8),
                self.params.size[1]
                    .div_ceil(self.params.scene[0])
                    .div_ceil(8),
                1,
            );
        }
        let compose = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lighting resolve"),
            layout: &self.compose.get_bind_group_layout(0),
            entries: &[
                bind_buffer(0, &self.uniform),
                bind_texture(1, &self.positions),
                bind_texture(2, &self.normals),
                bind_texture(3, &self.albedo),
                bind_buffer(4, &self.history[next].buffer),
                bind_buffer(5, &self.display),
            ],
        });
        let attachments = [Some(super::attachment(
            target,
            wgpu::LoadOp::Clear(wgpu::Color::BLACK),
        ))];
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bilateral lighting resolve"),
                color_attachments: &attachments,
                ..Default::default()
            });
            pass.set_viewport(viewport[0], viewport[1], viewport[2], viewport[3], 0.0, 1.0);
            pass.set_pipeline(&self.compose);
            pass.set_bind_group(0, &compose, &[]);
            pass.draw(0..3, 0..1);
        }
        self.index = next;
        self.params.size[2] = self.params.size[2].wrapping_add(1);
        self.params.size[3] = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::sampling_stride;
    #[test]
    fn high_dpi_history_stays_within_binding_budget() {
        let limit = 128 * 1024 * 1024;
        assert_eq!(sampling_stride(3840, 2160, limit), 2);
        assert_eq!(sampling_stride(5120, 2880, limit), 4);
        for (w, h) in [(0, 0), (97, 65), (3840, 2160), (7680, 4320), (16384, 16384)] {
            let stride = sampling_stride(w, h, limit);
            let size =
                u64::from(w.max(1).div_ceil(stride)) * u64::from(h.max(1).div_ceil(stride)) * 64;
            assert!(size.next_power_of_two() <= limit);
        }
    }
}
