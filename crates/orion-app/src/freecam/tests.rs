use std::time::Duration;

use super::*;

fn camera() -> Camera {
    Camera {
        eye: [0.0; 3],
        target: [0.0, 0.0, -1.0],
        vertical_fov_degrees: 38.0,
    }
}

#[test]
fn mouse_look_requires_capture_and_clamps_pitch_in_both_directions() {
    let mut camera = camera();
    let mut freecam = Freecam::new(&camera);
    assert!(!freecam.look(&mut camera, (0.0, -1_000_000.0)));
    assert_eq!(camera.target, [0.0, 0.0, -1.0]);
    freecam.captured = true;
    assert!(freecam.look(&mut camera, (0.0, -1_000_000.0)));
    assert_eq!(freecam.pitch, PITCH_LIMIT);
    assert!(camera.target[1] > 0.99);
    assert!(freecam.look(&mut camera, (0.0, 1_000_000.0)));
    assert_eq!(freecam.pitch, -PITCH_LIMIT);
    assert!(camera.target[1] < -0.99);
}

#[test]
fn diagonal_movement_uses_the_same_speed_as_forward_movement() {
    let mut camera = camera();
    let mut freecam = Freecam::new(&camera);
    freecam.keys.extend([KeyCode::KeyW, KeyCode::KeyD]);
    let start = Instant::now();
    freecam.last_step = Some(start);
    freecam.step_at(&mut camera, start + Duration::from_millis(50));
    let [x, y, z] = camera.eye;
    assert!(x > 0.0 && z < 0.0);
    assert_eq!(y, 0.0);
    assert!((x.hypot(z) - 0.15).abs() < 1e-6);
    assert_eq!(camera.target, [x, y, z - 1.0]);
}

#[test]
fn boosted_movement_caps_the_time_step() {
    let mut camera = camera();
    let mut freecam = Freecam::new(&camera);
    freecam.keys.extend([KeyCode::KeyW, KeyCode::ShiftRight]);
    let start = Instant::now();
    freecam.last_step = Some(start);
    freecam.step_at(&mut camera, start + Duration::from_secs(1));
    assert!((camera.eye[2] + 0.9).abs() < 1e-6);
}
