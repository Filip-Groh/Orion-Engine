mod fps;
mod freecam;

use std::env;

use anyhow::{Context, Result, ensure};
use fps::FpsCounter;
use freecam::Freecam;
use orion_renderer::Renderer;
use orion_scene::Scene;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

struct App {
    renderer: Option<Renderer<Window>>,
    scene: Scene,
    freecam: Freecam,
    fps: FpsCounter,
    occluded: bool,
    error: Option<anyhow::Error>,
}

impl App {
    fn drop_renderer(&mut self) {
        if let Some(renderer) = self.renderer.take() {
            self.freecam.release(renderer.window());
        }
        self.fps = FpsCounter::default();
        self.occluded = false;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let renderer = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Orion Engine")
                    .with_inner_size(LogicalSize::new(960, 540)),
            )
            .context("create the Orion window")
            .and_then(|window| {
                Renderer::new_with_overlay(window).context("initialize the Vulkan renderer")
            });
        match renderer {
            Ok(renderer) => {
                renderer.window().request_redraw();
                self.renderer = Some(renderer);
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let (Some(renderer), DeviceEvent::MouseMotion { delta }) = (&self.renderer, event)
            && self.freecam.look(&mut self.scene.camera, delta)
        {
            renderer.window().request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(renderer) = self
            .renderer
            .as_mut()
            .filter(|renderer| renderer.window().id() == id)
        else {
            return;
        };
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.freecam.capture(renderer.window());
            }
            WindowEvent::Focused(false) => {
                self.freecam.release(renderer.window());
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if occluded {
                    self.freecam.release(renderer.window());
                    self.fps = FpsCounter::default();
                } else {
                    renderer.window().request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } if !event.repeat => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    let pressed = event.state.is_pressed();
                    if key == KeyCode::Escape && pressed {
                        self.freecam.release(renderer.window());
                    } else if self.freecam.set_key(&mut self.scene.camera, key, pressed) {
                        renderer.window().request_redraw();
                    }
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                renderer.window().request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let size = renderer.window().inner_size();
                if self.occluded
                    || size.width == 0
                    || size.height == 0
                    || renderer.window().is_minimized() == Some(true)
                {
                    self.freecam.release(renderer.window());
                    self.fps = FpsCounter::default();
                    return;
                }
                self.freecam.step(&mut self.scene.camera);
                let scale = renderer.window().scale_factor() as f32;
                match renderer.render_with_overlay(
                    &self.scene,
                    [size.width, size.height],
                    scale,
                    |ui| self.fps.draw(ui),
                    Window::pre_present_notify,
                ) {
                    Ok(presented) => {
                        if presented {
                            self.fps.presented();
                        }
                        renderer.window().request_redraw();
                    }
                    Err(error) => {
                        self.error = Some(error.context("render the scene"));
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.drop_renderer();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.drop_renderer();
    }
}

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let obj_path = args.next();
    ensure!(args.next().is_none(), "usage: orion-app [OBJ path]");
    let mut scene = Scene::demo();
    if let Some(path) = obj_path {
        scene.import_obj(path, [0.65, 0.65, 0.65])?;
    }
    let event_loop = EventLoop::new().context("create the window event loop")?;
    let mut app = App {
        renderer: None,
        freecam: Freecam::new(&scene.camera),
        fps: FpsCounter::default(),
        occluded: false,
        scene,
        error: None,
    };
    event_loop
        .run_app(&mut app)
        .context("run the window event loop")?;
    app.error.map_or(Ok(()), Err)
}
