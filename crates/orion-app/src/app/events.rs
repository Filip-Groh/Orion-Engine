use anyhow::Context;
use orion_renderer::Renderer;
use orion_scene::Scene;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use super::App;
use crate::{fps::FpsCounter, freecam::Freecam};

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
            && let Ok(camera) = self.scene.camera_mut()
            && self.freecam.look(camera, delta)
        {
            renderer.window().request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(renderer) = self
            .renderer
            .as_ref()
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
            } => handle_key(renderer.window(), &mut self.freecam, &mut self.scene, event),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                renderer.window().request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.redraw() {
                    self.error = Some(error);
                    event_loop.exit();
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

fn handle_key(window: &Window, freecam: &mut Freecam, scene: &mut Scene, event: KeyEvent) {
    let PhysicalKey::Code(key) = event.physical_key else {
        return;
    };
    if event.repeat {
        return;
    }
    let pressed = event.state.is_pressed();
    if key == KeyCode::Escape && pressed {
        freecam.release(window);
    } else if let Ok(camera) = scene.camera_mut()
        && freecam.set_key(camera, key, pressed)
    {
        window.request_redraw();
    }
}
