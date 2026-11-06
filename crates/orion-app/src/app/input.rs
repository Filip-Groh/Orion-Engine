use std::{path::Path, time::Instant};

use winit::{
    event::KeyEvent,
    keyboard::{KeyCode, PhysicalKey},
    window::CursorGrabMode,
};

use super::App;
use crate::{checkpoint, freecam::FreecamInput};

const QUICK_CHECKPOINT: &str = "orion-quicksave.json";

impl App {
    pub(super) fn capture_cursor(&mut self) {
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
                self.hud.pointer_gone();
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

    pub(super) fn handle_key(&mut self, event: KeyEvent) {
        use KeyCode::*;
        let PhysicalKey::Code(key) = event.physical_key else {
            return;
        };
        if event.repeat {
            return;
        }
        let pressed = event.state.is_pressed();
        match key {
            F5 if pressed => self.quick_save(),
            F9 if pressed => self.quick_load(),
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

    fn quick_save(&self) {
        match checkpoint::save(&self.simulation, Path::new(QUICK_CHECKPOINT)) {
            Ok(()) => eprintln!("Saved tick {} to {QUICK_CHECKPOINT}", self.simulation.tick),
            Err(error) => eprintln!("Could not save {QUICK_CHECKPOINT}: {error:#}"),
        }
    }

    fn quick_load(&mut self) {
        match checkpoint::load(Path::new(QUICK_CHECKPOINT)) {
            Ok(simulation) => {
                eprintln!("Loaded tick {} from {QUICK_CHECKPOINT}", simulation.tick);
                self.replace_simulation(simulation, Instant::now());
            }
            Err(error) => eprintln!("Could not load {QUICK_CHECKPOINT}: {error:#}"),
        }
    }
}
