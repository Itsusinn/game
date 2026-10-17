use crate::{scene, session::Session};
use anyhow::{Context, Result};
use game_engine::{wgpu, Camera};
use protocol::ActionType;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

mod graphics;
mod ui;
use graphics::{capture, Graphics};
#[derive(Clone)]
enum Command {
    Connect,
    Disconnect,
    Demo,
    Action(ActionType),
}
struct App {
    graphics: Option<Graphics>,
    session: Session,
    zoom: f32,
    inventory: bool,
    chat: String,
    error: Option<anyhow::Error>,
    last_input: Instant,
    capture: Option<String>,
    frames: u32,
    started: Instant,
}
impl App {
    fn draw(&mut self) -> Result<bool> {
        self.session.tick();
        let Some(mut g) = self.graphics.take() else {
            return Ok(false);
        };
        if g.window.inner_size().width == 0 || g.window.inner_size().height == 0 {
            self.graphics = Some(g);
            return Ok(false);
        }
        let frame = match g.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated => {
                g.surface.configure(&g.device, &g.config);
                self.graphics = Some(g);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                g.surface = g.instance.create_surface(g.window.clone())?;
                g.surface.configure(&g.device, &g.config);
                self.graphics = Some(g);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.graphics = Some(g);
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => anyhow::bail!("Surface validation failed"),
        };
        let raw = g.ui_input.take_egui_input(&g.window);
        let mut viewport = egui::Rect::NOTHING;
        let mut commands = Vec::new();
        let mut output = g.ctx.run_ui(raw, |ui| {
            commands.clear();
            viewport = self.ui(ui, &g.adapter_name, &mut commands);
        });
        // egui may run layout more than once; execute side effects only afterwards.
        for command in commands {
            match command {
                Command::Connect => self.session.connect(),
                Command::Disconnect => self.session.disconnect(),
                Command::Demo => self.session.load_demo(),
                Command::Action(action) => {
                    if matches!(action, ActionType::Chat { .. }) {
                        self.chat.clear();
                    }
                    self.session.action(action);
                }
            }
        }
        g.ui_input
            .handle_platform_output(&g.window, output.platform_output);
        let ppp = output.pixels_per_point;
        let size = [g.config.width, g.config.height];
        let camera = Camera {
            center: [
                self.session.model.state.player_pos.x as f32 + 0.5,
                self.session.model.state.player_pos.y as f32 + 0.5,
            ],
            viewport: [
                viewport.min.x * ppp,
                viewport.min.y * ppp,
                viewport.width() * ppp,
                viewport.height() * ppp,
            ],
            pixels_per_tile: self.zoom * ppp,
        };
        let batch = scene::primitives(&self.session.model, &camera, size);
        g.renderer.prepare(&g.device, &g.queue, &batch);
        let paint_jobs = g.ctx.tessellate(output.shapes, ppp);
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                g.ui_renderer
                    .update_texture(&g.device, &g.queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: ppp,
        };
        let mut encoder = g.device.create_command_encoder(&Default::default());
        let mut extra =
            g.ui_renderer
                .update_buffers(&g.device, &g.queue, &mut encoder, &paint_jobs, &screen);
        let view = frame.texture.create_view(&Default::default());
        {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.025,
                        g: 0.04,
                        b: 0.055,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world + HUD"),
                color_attachments: &attachments,
                ..Default::default()
            });
            g.renderer.render(&mut pass);
            g.ui_renderer
                .render(&mut pass.forget_lifetime(), &paint_jobs, &screen);
        }
        extra.push(encoder.finish());
        g.queue.submit(extra);
        for id in &output.textures_delta.free {
            g.ui_renderer.free_texture(id);
        }
        output.textures_delta.clear();
        self.frames += 1;
        let capture_ready = self.capture.is_some()
            && self.frames > 5
            && (self.session.demo || !self.session.model.state.visible_tiles.is_empty());
        if capture_ready {
            capture(&g, &frame.texture, self.capture.as_deref().unwrap())?;
        }
        g.window.pre_present_notify();
        g.queue.present(frame);
        self.graphics = Some(g);
        if self.capture.is_some() && self.started.elapsed() > Duration::from_secs(30) {
            anyhow::bail!("Capture timed out waiting for world data");
        }
        Ok(capture_ready)
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }
        let result = (|| {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Dungeon - WebGPU")
                        .with_inner_size(winit::dpi::LogicalSize::new(1200.0, 800.0))
                        .with_min_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0)),
                )?,
            );
            pollster::block_on(Graphics::new(window, self.capture.is_some()))
        })();
        match result {
            Ok(g) => self.graphics = Some(g),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(g) = &mut self.graphics else {
            return;
        };
        let response = g.ui_input.on_window_event(&g.window, &event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => g.resize(size.width, size.height),
            WindowEvent::RedrawRequested => match self.draw() {
                Ok(true) => event_loop.exit(),
                Ok(false) => {}
                Err(e) => {
                    self.error = Some(e);
                    event_loop.exit();
                }
            },
            WindowEvent::MouseWheel { delta, .. } if !response.consumed => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                };
                self.zoom = (self.zoom + amount * 2.0).clamp(12.0, 48.0);
            }
            WindowEvent::KeyboardInput { event, .. }
                if !response.consumed
                    && !g.ctx.egui_wants_keyboard_input()
                    && event.state == ElementState::Pressed =>
            {
                if event.repeat && self.last_input.elapsed() < Duration::from_millis(100) {
                    return;
                }
                self.last_input = Instant::now();
                if let PhysicalKey::Code(code) = event.physical_key {
                    let action = match code {
                        KeyCode::KeyW | KeyCode::ArrowUp | KeyCode::Numpad8 => {
                            Some(ActionType::MoveUp)
                        }
                        KeyCode::KeyS | KeyCode::ArrowDown | KeyCode::Numpad2 => {
                            Some(ActionType::MoveDown)
                        }
                        KeyCode::KeyA | KeyCode::ArrowLeft | KeyCode::Numpad4 => {
                            Some(ActionType::MoveLeft)
                        }
                        KeyCode::KeyD | KeyCode::ArrowRight | KeyCode::Numpad6 => {
                            Some(ActionType::MoveRight)
                        }
                        KeyCode::KeyQ | KeyCode::Numpad7 => Some(ActionType::MoveUpLeft),
                        KeyCode::KeyE | KeyCode::Numpad9 => Some(ActionType::MoveUpRight),
                        KeyCode::KeyZ | KeyCode::Numpad1 => Some(ActionType::MoveDownLeft),
                        KeyCode::KeyC | KeyCode::Numpad3 => Some(ActionType::MoveDownRight),
                        KeyCode::Space | KeyCode::Period | KeyCode::Numpad5 => {
                            Some(ActionType::Wait)
                        }
                        KeyCode::KeyF => Some(ActionType::MeleeAttack),
                        KeyCode::KeyG => Some(ActionType::Pickup),
                        KeyCode::Tab => {
                            self.inventory = !self.inventory;
                            None
                        }
                        _ => None,
                    };
                    if let Some(action) = action {
                        self.session.action(action);
                    }
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(g) = &self.graphics {
            g.window.request_redraw();
        }
    }
}

pub fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let mut app = App {
        graphics: None,
        session: Session::default(),
        zoom: 24.0,
        inventory: true,
        chat: String::new(),
        error: None,
        last_input: Instant::now(),
        capture: None,
        frames: 0,
        started: Instant::now(),
    };
    let mut args = std::env::args().skip(1);
    let mut connect = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => app.session.load_demo(),
            "--connect" => connect = true,
            "--server" => {
                app.session.address = args.next().context("--server requires an address")?
            }
            "--cert" => app.session.certificate = args.next().context("--cert requires a file")?,
            "--name" => app.session.name = args.next().context("--name requires a value")?,
            "--capture" => {
                app.capture = Some(args.next().context("--capture requires a PNG path")?)
            }
            "--help" => {
                println!("client [--demo] [--connect] [--server IP:PORT] [--cert FILE.der] [--name NAME] [--capture FILE.png]");
                return Ok(());
            }
            _ => anyhow::bail!("Unknown option: {arg}"),
        }
    }
    if connect {
        app.session.connect();
    }
    EventLoop::new()?.run_app(&mut app)?;
    app.session.disconnect();
    if let Some(error) = app.error {
        return Err(error);
    }
    Ok(())
}
