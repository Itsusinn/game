use anyhow::{Context, Result};
use game_engine::{wgpu, Renderer};
use std::{sync::Arc, time::Duration};
use winit::window::Window;

pub(super) struct Graphics {
    pub(super) instance: wgpu::Instance,
    pub(super) window: Arc<Window>,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) config: wgpu::SurfaceConfiguration,
    pub(super) renderer: Renderer,
    pub(super) ui_renderer: egui_wgpu::Renderer,
    pub(super) ui_input: egui_winit::State,
    pub(super) ctx: egui::Context,
}
impl Graphics {
    pub(super) async fn new(window: Arc<Window>, capture: bool) -> Result<Self> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await?;
        println!("3D renderer: {}", adapter.get_info().name);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("dungeon device"),
                ..Default::default()
            })
            .await?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("No supported surface configuration")?;
        let capabilities = surface.get_capabilities(&adapter);
        config.format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::AutoVsync;
        if capture {
            anyhow::ensure!(
                capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC),
                "Surface does not support capture"
            );
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        surface.configure(&device, &config);
        let renderer = Renderer::new(&device, config.format, config.width, config.height);
        let ctx = egui::Context::default();
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(31, 35, 33);
        visuals.window_fill = egui::Color32::from_rgb(35, 39, 36);
        visuals.extreme_bg_color = egui::Color32::from_rgb(23, 27, 25);
        visuals.override_text_color = Some(egui::Color32::from_rgb(199, 201, 188));
        visuals.selection.bg_fill = egui::Color32::from_rgb(81, 101, 86);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(48, 55, 49);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(65, 76, 65);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(83, 97, 79);
        ctx.set_visuals(visuals);
        let ui_input = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let ui_renderer = egui_wgpu::Renderer::new(&device, config.format, Default::default());
        Ok(Self {
            instance,
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            ui_renderer,
            ui_input,
            ctx,
        })
    }
    pub(super) fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.renderer.resize(&self.device, width, height);
        self.surface.configure(&self.device, &self.config);
    }
}
pub(super) fn capture(g: &Graphics, texture: &wgpu::Texture, path: &str) -> Result<()> {
    let row = (g.config.width * 4).div_ceil(256) * 256;
    let buffer = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture readback"),
        size: row as u64 * g.config.height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = g.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
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
            width: g.config.width,
            height: g.config.height,
            depth_or_array_layers: 1,
        },
    );
    g.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    g.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(10)),
    })?;
    rx.recv_timeout(Duration::from_secs(10))??;
    let data = buffer.slice(..).get_mapped_range()?;
    let mut pixels = Vec::with_capacity((g.config.width * g.config.height * 4) as usize);
    for line in data.chunks(row as usize) {
        pixels.extend_from_slice(&line[..(g.config.width * 4) as usize]);
    }
    if matches!(
        g.config.format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, g.config.width, g.config.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    println!(
        "Captured {}x{} frame to {path}",
        g.config.width, g.config.height
    );
    Ok(())
}
