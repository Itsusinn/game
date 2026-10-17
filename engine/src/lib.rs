//! Small, host-independent WebGPU renderer. Windowing, networking and game rules
//! stay outside this crate. The renderer draws one instanced batch per frame.
use bytemuck::{Pod, Zeroable};
pub use wgpu;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub center: [f32; 2],
    pub viewport: [f32; 4],
    pub pixels_per_tile: f32,
}
impl Camera {
    pub fn project(&self, world: [f32; 2]) -> [f32; 2] {
        [
            self.viewport[0]
                + self.viewport[2] * 0.5
                + (world[0] - self.center[0]) * self.pixels_per_tile,
            self.viewport[1]
                + self.viewport[3] * 0.5
                + (world[1] - self.center[1]) * self.pixels_per_tile,
        ]
    }
    pub fn unproject(&self, screen: [f32; 2]) -> [f32; 2] {
        [
            (screen[0] - self.viewport[0] - self.viewport[2] * 0.5) / self.pixels_per_tile
                + self.center[0],
            (screen[1] - self.viewport[1] - self.viewport[3] * 0.5) / self.pixels_per_tile
                + self.center[1],
        ]
    }
    pub fn visible(&self, pos: [f32; 2]) -> bool {
        let p = self.project(pos);
        let s = self.pixels_per_tile;
        p[0] + s >= self.viewport[0]
            && p[1] + s >= self.viewport[1]
            && p[0] <= self.viewport[0] + self.viewport[2]
            && p[1] <= self.viewport[1] + self.viewport[3]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Primitive {
    rect: [f32; 4],
    color: [f32; 4],
    kind: u32,
    _padding: [u32; 3],
}
impl Primitive {
    /// Pixel-space rectangle. kind: 0 solid, 1 circle, 2 grid tile,
    /// 3 upstairs, 4 downstairs, 5 door frame.
    pub fn new(rect: [f32; 4], color: [f32; 4], kind: u32, screen: [u32; 2]) -> Self {
        let w = screen[0].max(1) as f32;
        let h = screen[1].max(1) as f32;
        Self {
            rect: [
                rect[0] / w * 2.0 - 1.0,
                1.0 - rect[1] / h * 2.0,
                rect[2] / w * 2.0,
                -rect[3] / h * 2.0,
            ],
            color,
            kind,
            _padding: [0; 3],
        }
    }
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    instances: wgpu::Buffer,
    capacity: usize,
    count: u32,
}
impl Renderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("dungeon primitives"),
            source: wgpu::ShaderSource::Wgsl(include_str!("primitives.wgsl").into()),
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Uint32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("instanced 2D renderer"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Primitive>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attributes,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            instances: Self::buffer(device, 1024),
            capacity: 1024,
            count: 0,
        }
    }
    fn buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("2D instances"),
            size: (capacity * std::mem::size_of::<Primitive>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        primitives: &[Primitive],
    ) {
        if primitives.len() > self.capacity {
            self.capacity = primitives.len().next_power_of_two();
            self.instances = Self::buffer(device, self.capacity);
        }
        self.count = primitives.len() as u32;
        if self.count > 0 {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(primitives));
        }
    }
    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_roundtrip_with_sidebar_zoom_and_negative_world_coordinates() {
        let camera = Camera {
            center: [-20.5, 300.5],
            viewport: [0.0, 48.0, 850.0, 600.0],
            pixels_per_tile: 24.0,
        };
        let world = [-18.0, 298.0];
        assert_eq!(camera.unproject(camera.project(world)), world);
        assert!(camera.visible(world));
        assert!(!camera.visible([500.0, 0.0]));
    }
}
