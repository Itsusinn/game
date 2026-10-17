//! Host-independent 3D triangle renderer: camera, mesh buffers and depth only.
use bytemuck::{Pod, Zeroable};
pub use wgpu;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub aspect: f32,
}
impl Camera {
    pub fn forward(&self) -> [f32; 3] {
        [
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        ]
    }
    fn uniform(&self) -> Uniform {
        let forward = self.forward();
        let right = [self.yaw.cos(), 0.0, self.yaw.sin()];
        let up = [
            -self.yaw.sin() * self.pitch.sin(),
            self.pitch.cos(),
            self.yaw.cos() * self.pitch.sin(),
        ];
        Uniform {
            eye: [self.eye[0], self.eye[1], self.eye[2], self.aspect],
            right: [right[0], right[1], right[2], (self.fov * 0.5).tan()],
            up: [up[0], up[1], up[2], 0.0],
            forward: [forward[0], forward[1], forward[2], 0.0],
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    eye: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    forward: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    pub uv: [f32; 2],
    pub pattern: u32,
}
#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
}
impl Mesh {
    pub fn face(&mut self, points: [[f32; 3]; 4], normal: [f32; 3], color: [f32; 3], pattern: u32) {
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for i in [0, 2, 1, 0, 3, 2] {
            self.vertices.push(Vertex {
                position: points[i],
                normal,
                color,
                uv: uv[i],
                pattern,
            });
        }
    }
    pub fn cuboid(&mut self, min: [f32; 3], size: [f32; 3], color: [f32; 3], pattern: u32) {
        for face in 0..6 {
            self.block_face(min, size, face, color, pattern);
        }
    }
    pub fn block_face(
        &mut self,
        min: [f32; 3],
        size: [f32; 3],
        face: usize,
        color: [f32; 3],
        pattern: u32,
    ) {
        const POINTS: [[[f32; 3]; 4]; 6] = [
            [[0., 0., 1.], [0., 0., 0.], [0., 1., 0.], [0., 1., 1.]],
            [[1., 0., 0.], [1., 0., 1.], [1., 1., 1.], [1., 1., 0.]],
            [[0., 0., 0.], [0., 0., 1.], [1., 0., 1.], [1., 0., 0.]],
            [[0., 1., 1.], [0., 1., 0.], [1., 1., 0.], [1., 1., 1.]],
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            [[1., 0., 1.], [0., 0., 1.], [0., 1., 1.], [1., 1., 1.]],
        ];
        let points = POINTS[face].map(|p| std::array::from_fn(|i| min[i] + p[i] * size[i]));
        self.face(points, NORMALS[face], color, pattern);
    }
}
pub const NORMALS: [[f32; 3]; 6] = [
    [-1., 0., 0.],
    [1., 0., 0.],
    [0., -1., 0.],
    [0., 1., 0.],
    [0., 0., -1.],
    [0., 0., 1.],
];
struct Buffer {
    buffer: wgpu::Buffer,
    capacity: usize,
    count: u32,
}
impl Buffer {
    fn new(device: &wgpu::Device, capacity: usize) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("3D mesh"),
                size: (capacity * std::mem::size_of::<Vertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity,
            count: 0,
        }
    }
    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mesh: &Mesh) {
        if mesh.vertices.len() > self.capacity {
            *self = Self::new(device, mesh.vertices.len().next_power_of_two());
        }
        self.count = mesh.vertices.len() as u32;
        if self.count > 0 {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&mesh.vertices));
        }
    }
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count > 0 {
            pass.set_vertex_buffer(0, self.buffer.slice(..));
            pass.draw(0..self.count, 0..1);
        }
    }
}
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    world: Buffer,
    actors: Buffer,
    pub depth: wgpu::TextureView,
}
impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel world"),
            source: wgpu::ShaderSource::Wgsl(include_str!("world.wgsl").into()),
        });
        let attrs =
            wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3,3=>Float32x2,4=>Uint32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("3D voxel pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            pipeline,
            uniform,
            bind,
            world: Buffer::new(device, 1024),
            actors: Buffer::new(device, 1024),
            depth: Self::depth(device, width, height),
        }
    }
    fn depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("world depth"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    }
    pub fn resize(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        self.depth = Self::depth(device, w, h);
    }
    pub fn world(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mesh: &Mesh) {
        self.world.upload(device, queue, mesh);
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        actors: &Mesh,
    ) {
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&camera.uniform()));
        self.actors.upload(device, queue, actors);
    }
    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        self.world.draw(pass);
        self.actors.draw(pass);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_basis_and_cube_winding() {
        for yaw in [0.0, 1.0, 3.0] {
            let c = Camera {
                eye: [0.; 3],
                yaw,
                pitch: 0.4,
                fov: 1.2,
                aspect: 1.5,
            };
            let u = c.uniform();
            let dot = (0..3).map(|i| u.right[i] * u.forward[i]).sum::<f32>();
            assert!(dot.abs() < 0.001);
        }
        let mut m = Mesh::default();
        m.cuboid([0.; 3], [1.; 3], [1.; 3], 0);
        assert_eq!(m.vertices.len(), 36);
        for triangle in m.vertices.chunks(3) {
            let a = std::array::from_fn::<_, 3, _>(|i| {
                triangle[1].position[i] - triangle[0].position[i]
            });
            let b = std::array::from_fn::<_, 3, _>(|i| {
                triangle[2].position[i] - triangle[0].position[i]
            });
            let n = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            assert!((0..3).map(|i| n[i] * triangle[0].normal[i]).sum::<f32>() > 0.0);
        }
    }
}
