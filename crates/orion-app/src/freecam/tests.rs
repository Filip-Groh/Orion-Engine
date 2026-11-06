use super::*;

fn camera() -> Camera {
    Camera {
        eye: [0.0; 3],
        target: [0.0, 0.0, -1.0],
        vertical_fov_degrees: 38.0,
    }
}

#[test]
fn mouse_look_clamps_pitch_in_both_directions() {
    let mut camera = camera();
    let mut freecam = Freecam::new(&camera);
    freecam.step(
        &mut camera,
        [FreecamInput::Look {
            dx: 0.0,
            dy: -1_000_000.0,
        }],
    );
    assert_eq!(freecam.pitch, PITCH_LIMIT);
    assert!(camera.target[1] > 0.99);
    freecam.step(
        &mut camera,
        [FreecamInput::Look {
            dx: 0.0,
            dy: 1_000_000.0,
        }],
    );
    assert_eq!(freecam.pitch, -PITCH_LIMIT);
    assert!(camera.target[1] < -0.99);
}

#[test]
fn diagonal_movement_uses_the_same_speed_as_forward_movement() {
    let mut camera = camera();
    let mut freecam = Freecam::new(&camera);
    let inputs = [KeyCode::KeyW, KeyCode::KeyD].map(|key| FreecamInput::Key { key, pressed: true });
    freecam.step(&mut camera, inputs);
    let [x, y, z] = camera.eye;
    assert!(x > 0.0 && z < 0.0);
    assert_eq!(y, 0.0);
    assert!((x.hypot(z) - 0.05).abs() < 1e-6);
    assert_eq!(camera.target, [x, y, z - 1.0]);
}

#[test]
fn boosted_movement_uses_the_fixed_time_step() {
    let mut camera = camera();
    let mut freecam = Freecam::new(&camera);
    let inputs =
        [KeyCode::KeyW, KeyCode::ShiftRight].map(|key| FreecamInput::Key { key, pressed: true });
    freecam.step(&mut camera, inputs);
    assert!((camera.eye[2] + 0.15).abs() < 1e-6);
}
