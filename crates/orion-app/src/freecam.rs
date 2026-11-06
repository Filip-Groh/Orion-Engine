use std::{collections::BTreeSet, f32::consts::TAU};

use orion_scene::Camera;
use winit::keyboard::KeyCode;

use crate::tick::TICK_SECONDS;

const LOOK_SENSITIVITY: f32 = 0.0025;
const PITCH_LIMIT: f32 = 89.0_f32.to_radians();
const MOVE_SPEED: f32 = 3.0;
const BOOST_MULTIPLIER: f32 = 3.0;

pub(crate) enum FreecamInput {
    Key { key: KeyCode, pressed: bool },
    Look { dx: f32, dy: f32 },
    ReleaseAll,
}

pub(crate) struct Freecam {
    keys: BTreeSet<KeyCode>,
    yaw: f32,
    pitch: f32,
}

impl Freecam {
    pub fn new(camera: &Camera) -> Self {
        let [x, y, z] = std::array::from_fn(|i| camera.target[i] - camera.eye[i]);
        Self {
            keys: BTreeSet::new(),
            yaw: libm::atan2f(x, -z),
            pitch: libm::atan2f(y, libm::hypotf(x, z)).clamp(-PITCH_LIMIT, PITCH_LIMIT),
        }
    }

    pub fn clear_keys(&mut self) {
        self.keys.clear();
    }

    pub fn step(&mut self, camera: &mut Camera, inputs: impl IntoIterator<Item = FreecamInput>) {
        let mut looked = false;
        for input in inputs {
            match input {
                FreecamInput::Key { key, pressed: true } => {
                    self.keys.insert(key);
                }
                FreecamInput::Key {
                    key,
                    pressed: false,
                } => {
                    self.keys.remove(&key);
                }
                FreecamInput::Look { dx, dy } => {
                    if dx.is_finite() && dy.is_finite() && (dx != 0.0 || dy != 0.0) {
                        self.yaw = (self.yaw + dx * LOOK_SENSITIVITY).rem_euclid(TAU);
                        self.pitch =
                            (self.pitch - dy * LOOK_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
                        looked = true;
                    }
                }
                FreecamInput::ReleaseAll => self.clear_keys(),
            }
        }

        let axes = self.axes();
        if axes == [0.0; 3] && !looked {
            return;
        }
        let (sin_yaw, cos_yaw) = libm::sincosf(self.yaw);
        let (sin_pitch, cos_pitch) = libm::sincosf(self.pitch);
        let forward = [sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch];
        let [strafe, rise, advance] = axes;
        let movement = [
            strafe * cos_yaw + advance * forward[0],
            rise + advance * forward[1],
            strafe * sin_yaw + advance * forward[2],
        ];
        let length = movement
            .iter()
            .map(|component| component * component)
            .sum::<f32>()
            .sqrt();
        let scale = self.speed() * TICK_SECONDS / length.max(1.0);
        for (coordinate, direction) in camera.eye.iter_mut().zip(movement) {
            *coordinate += direction * scale;
        }
        camera.target = std::array::from_fn(|i| camera.eye[i] + forward[i]);
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
}

#[cfg(test)]
mod tests;
