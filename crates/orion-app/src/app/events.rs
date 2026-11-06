use std::{path::Path, time::Instant};

use anyhow::Context;
use orion_renderer::Renderer;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

use super::App;
use crate::{checkpoint, freecam::FreecamInput, rates::RateCounter};

const QUICK_CHECKPOINT: &str = "orion-quicksave.json";

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
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.capture_cursor();
            }
            WindowEvent::Focused(false) => {
                self.release_cursor();
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if occluded {
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

    fn capture_cursor(&mut self) {
        let Some(renderer) = &self.renderer else {
            return;
        };
        if self.cursor_captured {
            return;
        }
        let window = renderer.window();
        match window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
        {
            Ok(()) => {
                self.cursor_captured = true;
                window.set_cursor_visible(false);
            }
            Err(error) => eprintln!("Could not capture the cursor: {error}"),
        }
    }

    pub(super) fn release_cursor(&mut self) {
        if !self.cursor_captured {
            return;
        }
        let now = Instant::now();
        if let Some(renderer) = &self.renderer {
            let window = renderer.window();
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
        self.cursor_captured = false;
        self.queue_input(now, FreecamInput::ReleaseAll);
    }

    fn handle_key(&mut self, event: KeyEvent) {
        use KeyCode::*;
        let PhysicalKey::Code(key) = event.physical_key else {
            return;
        };
        if event.repeat {
            return;
        }
        let pressed = event.state.is_pressed();
        match key {
            F5 if pressed => {
                match checkpoint::save(&self.simulation, Path::new(QUICK_CHECKPOINT)) {
                    Ok(()) => {
                        eprintln!("Saved tick {} to {QUICK_CHECKPOINT}", self.simulation.tick)
                    }
                    Err(error) => eprintln!("Could not save {QUICK_CHECKPOINT}: {error:#}"),
                }
            }
            F9 if pressed => match checkpoint::load(Path::new(QUICK_CHECKPOINT)) {
                Ok(simulation) => {
                    eprintln!("Loaded tick {} from {QUICK_CHECKPOINT}", simulation.tick);
                    self.simulation = simulation;
                    self.previous = Default::default();
                    self.tps = RateCounter::default();
                    self.start_simulation(Instant::now());
                    if let Some(renderer) = &self.renderer {
                        renderer.window().request_redraw();
                    }
                }
                Err(error) => eprintln!("Could not load {QUICK_CHECKPOINT}: {error:#}"),
            },
            Escape if pressed => {
                if self.cursor_captured {
                    self.release_cursor();
                } else {
                    self.queue_input(Instant::now(), FreecamInput::ReleaseAll);
                }
            }
            KeyW | KeyA | KeyS | KeyD | Space | ControlLeft | ControlRight | ShiftLeft
            | ShiftRight
                if self.cursor_captured =>
            {
                self.queue_input(Instant::now(), FreecamInput::Key { key, pressed });
            }
            _ => {}
        }
    }
}
