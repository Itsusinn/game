use crate::{scene, session::Session};
use anyhow::{Context, Result};
use game_engine::{wgpu, Camera};
use protocol::PlayerInput;
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};
mod graphics;
mod ui;
use graphics::{capture, Graphics};
enum Command {
    Connect,
    Disconnect,
    Demo,
    Chat(String),
}
struct App {
    graphics: Option<Graphics>,
    session: Session,
    inventory: bool,
    chat: String,
    error: Option<anyhow::Error>,
    capture: Option<String>,
    frames: u32,
    started: Instant,
    last_frame: Instant,
    accumulator: f32,
    yaw: f32,
    pitch: f32,
    fov: f32,
    captured: bool,
    keys: HashSet<KeyCode>,
    attack: bool,
    pickup: bool,
    cursor: [f32; 2],
    viewport: [f32; 4],
    mesh_revision: Option<(u64, u64)>,
}
impl App {
    fn release(&mut self) {
        self.captured = false;
        self.keys.clear();
        self.attack = false;
        self.pickup = false;
        if let Some(g) = &self.graphics {
            let _ = g.window.set_cursor_grab(CursorGrabMode::None);
            g.window.set_cursor_visible(true);
        }
    }
    fn capture_mouse(&mut self) {
        if let Some(g) = &self.graphics {
            if g.window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| g.window.set_cursor_grab(CursorGrabMode::Confined))
                .is_ok()
            {
                g.window.set_cursor_visible(false);
                self.captured = true;
            }
        }
    }
    fn simulate(&mut self) {
        let now = Instant::now();
        let elapsed = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        if !self.session.model.ready {
            self.accumulator = 0.0;
            return;
        }
        self.accumulator = (self.accumulator + elapsed).min(0.1);
        while self.accumulator >= game_simulation::DT {
            self.accumulator -= game_simulation::DT;
            let held = |key| self.captured && self.keys.contains(&key);
            let input = PlayerInput {
                forward: i32::from(held(KeyCode::KeyW) || held(KeyCode::ArrowUp)) as f32
                    - i32::from(held(KeyCode::KeyS) || held(KeyCode::ArrowDown)) as f32,
                strafe: i32::from(held(KeyCode::KeyD) || held(KeyCode::ArrowRight)) as f32
                    - i32::from(held(KeyCode::KeyA) || held(KeyCode::ArrowLeft)) as f32,
                yaw: self.yaw,
                pitch: self.pitch,
                jump: held(KeyCode::Space),
                attack: self.attack,
                pickup: self.pickup,
            };
            self.attack = false;
            self.pickup = false;
            self.session.input(input);
        }
    }
    fn draw(&mut self) -> Result<bool> {
        self.session.tick();
        self.simulate();
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
            viewport = self.ui(ui, &mut commands);
        });
        for command in commands {
            match command {
                Command::Connect => {
                    self.session.connect();
                    self.mesh_revision = None;
                }
                Command::Disconnect => {
                    self.session.disconnect();
                    self.mesh_revision = None;
                }
                Command::Demo => {
                    self.session.load_demo();
                    self.yaw = 0.0;
                    self.pitch = -0.10;
                    self.inventory = false;
                    self.mesh_revision = None;
                }
                Command::Chat(text) => {
                    self.session.chat(text);
                    self.chat.clear();
                }
            }
        }
        g.ui_input
            .handle_platform_output(&g.window, output.platform_output);
        let ppp = output.pixels_per_point;
        self.viewport = [
            viewport.min.x * ppp,
            viewport.min.y * ppp,
            (viewport.width() * ppp).max(1.0),
            (viewport.height() * ppp).max(1.0),
        ];
        let revision = (self.session.generation, self.session.model.revision);
        if self.mesh_revision != Some(revision) {
            g.renderer
                .world(&g.device, &g.queue, &scene::terrain(&self.session.model));
            self.mesh_revision = Some(revision);
        }
        let p = self.session.model.body.position;
        let camera = Camera {
            eye: [p[0], p[1] + game_simulation::EYE_HEIGHT, p[2]],
            yaw: self.yaw,
            pitch: self.pitch,
            fov: self.fov.to_radians(),
            aspect: self.viewport[2] / self.viewport[3],
        };
        g.renderer.prepare(
            &g.device,
            &g.queue,
            &camera,
            &scene::actors(&self.session.model),
        );
        let paint_jobs = g.ctx.tessellate(output.shapes, ppp);
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                g.ui_renderer
                    .update_texture(&g.device, &g.queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [g.config.width, g.config.height],
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
                        r: 0.55,
                        g: 0.68,
                        b: 0.72,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3D world"),
                color_attachments: &attachments,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &g.renderer.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_viewport(
                self.viewport[0],
                self.viewport[1],
                self.viewport[2],
                self.viewport[3],
                0.0,
                1.0,
            );
            g.renderer.render(&mut pass);
        }
        {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })];
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HUD"),
                color_attachments: &attachments,
                ..Default::default()
            });
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
        let ready = self.capture.is_some() && self.frames > 5 && self.session.model.ready;
        if ready {
            capture(&g, &frame.texture, self.capture.as_deref().unwrap())?;
        }
        g.window.pre_present_notify();
        g.queue.present(frame);
        self.graphics = Some(g);
        if self.capture.is_some() && self.started.elapsed() > Duration::from_secs(30) {
            anyhow::bail!("Capture timed out waiting for 3D world");
        }
        Ok(ready)
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
                        .with_title("Dungeon 3D - Voxel frontier")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
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
    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if self.captured {
            if let DeviceEvent::MouseMotion { delta } = event {
                self.yaw = (self.yaw + delta.0 as f32 * 0.0025).rem_euclid(std::f32::consts::TAU);
                self.pitch = (self.pitch - delta.1 as f32 * 0.0025).clamp(-1.5, 1.5);
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(g) = &mut self.graphics else {
            return;
        };
        let response = g.ui_input.on_window_event(&g.window, &event);
        let typing = g.ctx.egui_wants_keyboard_input();
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => g.resize(size.width, size.height),
            WindowEvent::Focused(false) => self.release(),
            WindowEvent::RedrawRequested => match self.draw() {
                Ok(true) => event_loop.exit(),
                Ok(false) => {}
                Err(e) => {
                    self.error = Some(e);
                    event_loop.exit();
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = [position.x as f32, position.y as f32]
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.captured {
                    self.attack = true;
                } else if !response.consumed
                    && self.session.model.ready
                    && self.cursor[0] >= self.viewport[0]
                    && self.cursor[0] < self.viewport[0] + self.viewport[2]
                    && self.cursor[1] >= self.viewport[1]
                    && self.cursor[1] < self.viewport[1] + self.viewport[3]
                {
                    self.capture_mouse();
                }
            }
            WindowEvent::MouseWheel { delta, .. } if !response.consumed => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                };
                self.fov = (self.fov - amount * 2.0).clamp(55.0, 100.0);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if event.state == ElementState::Released {
                        self.keys.remove(&code);
                        return;
                    }
                    if code == KeyCode::Escape {
                        self.release();
                        return;
                    }
                    if !event.repeat && code == KeyCode::Tab && (self.captured || !typing) {
                        self.inventory = !self.inventory;
                        self.release();
                        return;
                    }
                    if response.consumed && !self.captured {
                        return;
                    }
                    if self.captured {
                        self.keys.insert(code);
                        if !event.repeat {
                            if code == KeyCode::KeyF {
                                self.attack = true;
                            }
                            if code == KeyCode::KeyG {
                                self.pickup = true;
                            }
                        }
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
        inventory: false,
        chat: String::new(),
        error: None,
        capture: None,
        frames: 0,
        started: Instant::now(),
        last_frame: Instant::now(),
        accumulator: 0.0,
        yaw: 0.0,
        pitch: -0.10,
        fov: 72.0,
        captured: false,
        keys: HashSet::new(),
        attack: false,
        pickup: false,
        cursor: [0.; 2],
        viewport: [0.; 4],
        mesh_revision: None,
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
            "--name" => app.session.name = args.next().context("--name requires a name")?,
            "--capture" => app.capture = Some(args.next().context("--capture requires a path")?),
            "--fov" => {
                app.fov = args.next().context("--fov requires degrees")?.parse()?;
                anyhow::ensure!((55.0..=100.0).contains(&app.fov), "FOV must be 55..100");
            }
            "--yaw" => {
                let degrees: f32 = args.next().context("--yaw requires degrees")?.parse()?;
                anyhow::ensure!(degrees.is_finite(), "yaw must be finite");
                app.yaw = degrees.to_radians().rem_euclid(std::f32::consts::TAU);
            }
            "--help" => {
                println!("client [--demo] [--connect] [--server IP:PORT] [--cert FILE.der] [--name NAME] [--capture FILE.png] [--fov 55..100] [--yaw DEGREES]");
                return Ok(());
            }
            _ => anyhow::bail!("Unknown option: {arg}"),
        }
    }
    if connect {
        app.session.connect();
    }
    EventLoop::new()?.run_app(&mut app)?;
    app.session.shutdown();
    if let Some(e) = app.error {
        return Err(e);
    }
    Ok(())
}
