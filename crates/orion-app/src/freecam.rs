use std::{collections::HashSet, f32::consts::TAU, time::Instant};

use orion_scene::Camera;
use winit::{
    keyboard::KeyCode,
    window::{CursorGrabMode, Window},
};

const LOOK_SENSITIVITY: f32 = 0.0025;
const PITCH_LIMIT: f32 = 89.0_f32.to_radians();
const MOVE_SPEED: f32 = 3.0;
const BOOST_MULTIPLIER: f32 = 3.0;

pub(crate) struct Freecam {
    captured: bool,
    keys: HashSet<KeyCode>,
    last_step: Option<Instant>,
    yaw: f32,
    pitch: f32,
}

impl Freecam {
    pub fn new(camera: &Camera) -> Self {
        let [x, y, z] = std::array::from_fn(|i| camera.target[i] - camera.eye[i]);
        Self {
            captured: false,
            keys: HashSet::new(),
            last_step: None,
            yaw: x.atan2(-z),
            pitch: y.atan2(x.hypot(z)).clamp(-PITCH_LIMIT, PITCH_LIMIT),
        }
    }

    pub fn capture(&mut self, window: &Window) {
        if self.captured {
            return;
        }
        match window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
        {
            Ok(()) => {
                self.captured = true;
                window.set_cursor_visible(false);
            }
            Err(error) => eprintln!("Could not capture the cursor: {error}"),
        }
    }

    pub fn release(&mut self, window: &Window) {
        if self.captured {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
            self.captured = false;
            self.keys.clear();
            self.last_step = None;
        }
    }

    pub fn set_key(&mut self, camera: &mut Camera, key: KeyCode, pressed: bool) -> bool {
        use KeyCode::*;
        if !self.captured
            || !matches!(
                key,
                KeyW | KeyA
                    | KeyS
                    | KeyD
                    | Space
                    | ControlLeft
                    | ControlRight
                    | ShiftLeft
                    | ShiftRight
            )
        {
            return false;
        }
        self.step(camera);
        if pressed {
            self.keys.insert(key);
        } else {
            self.keys.remove(&key);
        }
        self.last_step = (self.axes() != [0.0; 3]).then(Instant::now);
        true
    }

    pub fn look(&mut self, camera: &mut Camera, (dx, dy): (f64, f64)) -> bool {
        if !self.captured || (dx == 0.0 && dy == 0.0) {
            return false;
        }
        self.step(camera);
        self.yaw = (self.yaw + dx as f32 * LOOK_SENSITIVITY).rem_euclid(TAU);
        self.pitch = (self.pitch - dy as f32 * LOOK_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        Self::aim(camera, self.forward());
        true
    }

    /// Advances held movement to the current time.
    pub fn step(&mut self, camera: &mut Camera) {
        self.step_at(camera, Instant::now());
    }

    fn step_at(&mut self, camera: &mut Camera, now: Instant) {
        let Some(previous) = self.last_step else {
            return;
        };
        self.last_step = Some(now);
        let dt = (now - previous).as_secs_f32().min(0.1);
        let forward = self.forward();
        let movement = self.movement(forward);
        let length = movement
            .iter()
            .map(|component| component * component)
            .sum::<f32>()
            .sqrt();
        let scale = self.speed() * dt / length.max(1.0);
        for (coordinate, direction) in camera.eye.iter_mut().zip(movement) {
            *coordinate += direction * scale;
        }
        Self::aim(camera, forward);
    }

    fn movement(&self, forward: [f32; 3]) -> [f32; 3] {
        let [strafe, rise, advance] = self.axes();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        [
            strafe * cos_yaw + advance * forward[0],
            rise + advance * forward[1],
            strafe * sin_yaw + advance * forward[2],
        ]
    }

    fn speed(&self) -> f32 {
        let boosted =
            self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
        if boosted {
            MOVE_SPEED * BOOST_MULTIPLIER
        } else {
            MOVE_SPEED
        }
    }

    fn axes(&self) -> [f32; 3] {
        use KeyCode::*;
        let down = |key| self.keys.contains(&key);
        let axis =
            |positive, negative| f32::from(u8::from(positive)) - f32::from(u8::from(negative));
        [
            axis(down(KeyD), down(KeyA)),
            axis(down(Space), down(ControlLeft) || down(ControlRight)),
            axis(down(KeyW), down(KeyS)),
        ]
    }

    fn forward(&self) -> [f32; 3] {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        [sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch]
    }

    fn aim(camera: &mut Camera, forward: [f32; 3]) {
        camera.target = std::array::from_fn(|i| camera.eye[i] + forward[i]);
    }
}

#[cfg(test)]
mod tests;
