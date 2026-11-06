use std::time::Instant;

use anyhow::Context;
use orion_renderer::Renderer;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{DeviceEvent, DeviceId, MouseButton, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{Window, WindowId},
};

use super::App;
use crate::{freecam::FreecamInput, rates::RateCounter};

impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, _: StartCause) {
        self.run_due_ticks(event_loop);
    }

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
                self.start_simulation(Instant::now());
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if self.cursor_captured
            && let DeviceEvent::MouseMotion { delta: (dx, dy) } = event
            && (dx as f32).is_finite()
            && (dy as f32).is_finite()
        {
            self.queue_input(
                Instant::now(),
                FreecamInput::Look {
                    dx: dx as f32,
                    dy: dy as f32,
                },
            );
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
            WindowEvent::CursorMoved { position, .. } if !self.cursor_captured => {
                let position = position.to_logical::<f32>(renderer.window().scale_factor());
                self.hud.pointer_moved(egui::pos2(position.x, position.y));
            }
            WindowEvent::CursorLeft { .. } => self.hud.pointer_gone(),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } if !self.cursor_captured => {
                let pressed = state.is_pressed();
                if !self.hud.pointer_button(pressed) && pressed {
                    self.capture_cursor();
                }
            }
            WindowEvent::Focused(false) => {
                self.hud.pointer_gone();
                self.release_cursor();
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if occluded {
                    self.hud.pointer_gone();
                    self.release_cursor();
                    self.fps = RateCounter::default();
                } else {
                    renderer.window().request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => self.handle_key(event),
            WindowEvent::Resized(size) => {
                let window = renderer.window();
                window.request_redraw();
                if size.width == 0 || size.height == 0 || window.is_minimized() == Some(true) {
                    self.release_cursor();
                    self.fps = RateCounter::default();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.hud.pointer_gone();
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.run_due_ticks(event_loop);
        let deadline = self
            .clock
            .as_ref()
            .and_then(|clock| clock.next_deadline(self.simulation.tick));
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.drop_renderer();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.drop_renderer();
    }
}

impl App {
    fn run_due_ticks(&mut self, event_loop: &ActiveEventLoop) {
        if !event_loop.exiting()
            && let Err(error) = self.advance_simulation(Instant::now())
        {
            self.error = Some(error);
            event_loop.exit();
        }
    }
}
