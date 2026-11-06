use std::time::Duration;

use orion_scene::Camera;
use winit::keyboard::KeyCode;

use super::*;

fn start_app(now: Instant) -> App {
    let mut app = App::new(Scene::new(Camera {
        eye: [0.0; 3],
        target: [0.0, 0.0, -1.0],
        vertical_fov_degrees: 38.0,
    }))
    .unwrap();
    app.start_simulation(now);
    app
}

#[test]
fn new_input_does_not_affect_overdue_ticks() {
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let mut app = start_app(start);
    app.queue_input(
        at(90),
        FreecamInput::Key {
            key: KeyCode::KeyW,
            pressed: true,
        },
    );
    app.advance_simulation(at(99)).unwrap();
    assert_eq!(app.scene.camera().unwrap().eye, [0.0; 3]);
    app.advance_simulation(at(100)).unwrap();
    let eye = app.scene.camera().unwrap().eye;
    assert_eq!(eye[..2], [0.0; 2]);
    assert!((eye[2] + 0.05).abs() < 1e-6);

    app.queue_input(at(101), FreecamInput::ReleaseAll);
    app.advance_simulation(at(200)).unwrap();
    assert_eq!(app.scene.camera().unwrap().eye, eye);
}

#[test]
fn batched_and_individual_ticks_replay_to_identical_camera_state() {
    let run = |updates: &[u64]| {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut app = start_app(start);
        app.queue_input(
            at(10),
            FreecamInput::Key {
                key: KeyCode::KeyW,
                pressed: true,
            },
        );
        // Both deltas land on tick 2. Applying them in order matters at the pitch limit.
        app.queue_input(
            at(20),
            FreecamInput::Look {
                dx: 20.0,
                dy: -800.0,
            },
        );
        app.queue_input(at(21), FreecamInput::Look { dx: 1.0, dy: 90.0 });
        app.queue_input(
            at(70),
            FreecamInput::Key {
                key: KeyCode::KeyW,
                pressed: false,
            },
        );
        for &ms in updates {
            app.advance_simulation(at(ms)).unwrap();
        }
        let camera = app.scene.camera().unwrap();
        (
            camera.eye.map(f32::to_bits),
            camera.target.map(f32::to_bits),
        )
    };
    assert_eq!(run(&[17, 34, 50, 67, 84, 100]), run(&[100]));
}
