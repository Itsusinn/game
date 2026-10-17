//! Actual GPU behavior checks, run explicitly with:
//! cargo test -p game-engine --test lighting_gpu -- --ignored --nocapture
use game_engine::{wgpu, Camera, LightingMode, Mesh, Renderer};
use std::time::Duration;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    target: wgpu::Texture,
    size: u32,
}
impl Gpu {
    fn new(size: u32) -> Self {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("GPU adapter required");
        eprintln!("Lighting validation GPU: {}", adapter.get_info().name);
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let renderer = Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, size, size);
        let target = Self::target(&device, size);
        Self {
            device,
            queue,
            renderer,
            target,
            size,
        }
    }
    fn target(device: &wgpu::Device, size: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lighting test"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }
    fn resize(&mut self, size: u32) {
        self.size = size;
        self.target = Self::target(&self.device, size);
        self.renderer.resize(&self.device, size, size);
    }
    fn world(&mut self, mesh: &Mesh) {
        self.renderer.world(&self.device, &self.queue, mesh);
    }
    fn draw(
        &mut self,
        camera: &Camera,
        actors: &Mesh,
        mode: LightingMode,
        frames: usize,
    ) -> Vec<[u8; 4]> {
        self.renderer.set_lighting(mode);
        let viewport = [0., 0., self.size as f32, self.size as f32];
        let view = self.target.create_view(&Default::default());
        for _ in 0..frames {
            self.renderer
                .prepare(&self.device, &self.queue, camera, actors, viewport);
            let mut encoder = self.device.create_command_encoder(&Default::default());
            self.renderer
                .render(&self.device, &mut encoder, &view, viewport);
            let submission = self.queue.submit([encoder.finish()]);
            // Bound the test's GPU queue and make failures attributable to a frame.
            self.device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: Some(Duration::from_secs(30)),
                })
                .unwrap();
        }
        let row = (self.size * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lighting test readback"),
            size: u64::from(row * self.size),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: self.size,
                height: self.size,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(Duration::from_secs(30)),
            })
            .unwrap();
        rx.recv().unwrap().unwrap();
        let mapped = buffer.slice(..).get_mapped_range().unwrap();
        mapped
            .chunks(row as usize)
            .flat_map(|r| {
                r[..self.size as usize * 4]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .copied()
            })
            .collect()
    }
}
fn camera() -> Camera {
    Camera {
        eye: [0., 1.0, 3.0],
        yaw: 0.,
        pitch: -0.12,
        fov: 1.1,
        aspect: 1.0,
    }
}
fn room(roof: bool) -> Mesh {
    let mut mesh = Mesh::default();
    mesh.cuboid([-3., -1., -3.], [6., 1., 7.], [0.6; 3], 0);
    mesh.cuboid([-3., 0., -3.], [6., 3., 0.2], [0.6; 3], 0);
    mesh.cuboid([-3., 0., -3.], [0.2, 3., 7.], [0.75, 0.06, 0.04], 0);
    mesh.cuboid([2.8, 0., -3.], [0.2, 3., 7.], [0.6; 3], 0);
    mesh.cuboid([-3., 0., 3.8], [6., 3., 0.2], [0.6; 3], 0);
    if roof {
        mesh.cuboid([-3., 3., -3.], [6., 0.2, 7.], [0.6; 3], 0);
    }
    mesh
}
fn center_mean(image: &[[u8; 4]], size: usize) -> [f32; 3] {
    let mut sum = [0.; 3];
    for y in size / 3..size * 2 / 3 {
        for x in size / 3..size * 2 / 3 {
            for (c, value) in sum.iter_mut().enumerate() {
                *value += f32::from(image[x + y * size][c]);
            }
        }
    }
    let count = (size * 2 / 3 - size / 3).pow(2) as f32;
    sum.map(|s| s / count)
}

#[test]
#[ignore = "requires a real graphics adapter; explicit GPU validation"]
fn diffuse_transport_occlusion_history_and_resize() {
    let mut gpu = Gpu::new(96);
    let camera = camera();
    let empty = Mesh::default();
    gpu.world(&room(false));
    let direct = gpu.draw(&camera, &empty, LightingMode::Direct, 48);
    let gi = gpu.draw(&camera, &empty, LightingMode::Global, 48);
    let direct_mean = center_mean(&direct, 96);
    let gi_mean = center_mean(&gi, 96);
    eprintln!("Open room direct {direct_mean:?}, GI {gi_mean:?}");
    assert!(
        gi_mean.iter().sum::<f32>() > direct_mean.iter().sum::<f32>() + 30.,
        "sky/bounce light must illuminate a wall outside direct sunlight"
    );

    // Changing only an off-center wall's reflectance must change indirect light
    // on the neutral back wall. This distinguishes color transport from ambient fill.
    let red = gpu.draw(&camera, &empty, LightingMode::Indirect, 64);
    let mut blue_room = room(false);
    for vertex in &mut blue_room.vertices {
        if vertex.color == [0.75, 0.06, 0.04] {
            vertex.color = [0.04, 0.06, 0.75];
        }
    }
    gpu.world(&blue_room);
    let blue = gpu.draw(&camera, &empty, LightingMode::Indirect, 128);
    let red_mean = center_mean(&red, 96);
    let blue_mean = center_mean(&blue, 96);
    eprintln!("Indirect color transport: red wall {red_mean:?}, blue wall {blue_mean:?}");
    assert!(red_mean[0] > blue_mean[0] + 2.0);
    assert!(blue_mean[2] > red_mean[2] + 2.0);

    // A closed room has neither sky nor sun. Replacing the bright world must clear
    // cache/history immediately, including after odd-sized framebuffer changes.
    gpu.world(&room(true));
    let closed = gpu.draw(&camera, &empty, LightingMode::Global, 1);
    assert!(
        center_mean(&closed, 96).into_iter().all(|x| x < 1.),
        "stale GI or light leaked into the sealed room"
    );
    gpu.resize(97);
    let closed = gpu.draw(&camera, &empty, LightingMode::Global, 12);
    assert!(center_mean(&closed, 97).into_iter().all(|x| x < 1.));

    let mut outside = camera;
    outside.eye = [0., 1., 8.];
    let exterior = gpu.draw(&outside, &empty, LightingMode::Global, 48);
    assert!(center_mean(&exterior, 97)[0] > 50.);
    let inside = gpu.draw(&camera, &empty, LightingMode::Global, 1);
    assert!(
        center_mean(&inside, 97).into_iter().all(|x| x < 1.),
        "camera disocclusion must reject exterior history"
    );

    // A dynamic offscreen roof must participate in ray queries; removing it must
    // restore direct light without rebuilding the static mesh.
    let mut ground = Mesh::default();
    ground.cuboid([-5., -1., -5.], [10., 1., 10.], [0.6; 3], 0);
    let mut down = camera;
    down.eye = [0., 2., 0.];
    down.pitch = -1.4;
    gpu.world(&ground);
    let bright = gpu.draw(&down, &empty, LightingMode::Direct, 8);
    let mut roof = Mesh::default();
    roof.cuboid([-20., 4., -20.], [40., 0.2, 40.], [0.7; 3], 0);
    let shadow = gpu.draw(&down, &roof, LightingMode::Direct, 24);
    let restored = gpu.draw(&down, &empty, LightingMode::Direct, 24);
    let b = center_mean(&bright, 97)[0];
    let s = center_mean(&shadow, 97)[0];
    let r = center_mean(&restored, 97)[0];
    eprintln!("Dynamic offscreen occluder: lit {b}, blocked {s}, restored {r}");
    assert!(b > 100. && s < 5. && r > b * 0.9);
}
