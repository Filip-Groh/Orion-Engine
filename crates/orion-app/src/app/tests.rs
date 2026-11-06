use std::time::Duration;

use orion_scene::{Camera, Entity, Scene, SineBob, Sphere};
use winit::keyboard::KeyCode;

use super::*;

fn start_app(now: Instant) -> App {
    let mut app = App::new(
        Simulation::new(Scene::new(Camera {
            eye: [0.0; 3],
            target: [0.0, 0.0, -1.0],
            vertical_fov_degrees: 38.0,
        }))
        .unwrap(),
    );
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
    assert_eq!(app.simulation.scene.camera().unwrap().eye, [0.0; 3]);
    app.advance_simulation(at(100)).unwrap();
    let eye = app.simulation.scene.camera().unwrap().eye;
    assert_eq!(eye[..2], [0.0; 2]);
    assert!((eye[2] + 0.05).abs() < 1e-6);

    app.queue_input(at(101), FreecamInput::ReleaseAll);
    app.advance_simulation(at(200)).unwrap();
    assert_eq!(app.simulation.scene.camera().unwrap().eye, eye);
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
        let camera = app.simulation.scene.camera().unwrap();
        (
            camera.eye.map(f32::to_bits),
            camera.target.map(f32::to_bits),
        )
    };
    assert_eq!(run(&[17, 34, 50, 67, 84, 100]), run(&[100]));
}

#[test]
fn only_the_marked_sphere_moves_and_its_sine_wave_repeats() {
    let start = Instant::now();
    let mut app = App::new(Simulation::new(Scene::demo()).unwrap());
    app.start_simulation(start);
    let (animated, bob) = animated_sphere(&app.simulation.scene);
    let static_centers: Vec<_> = app
        .simulation
        .scene
        .world
        .query::<(Entity, &Sphere)>()
        .iter()
        .filter(|(entity, _)| *entity != animated)
        .map(|(entity, sphere)| (entity, sphere.center))
        .collect();
    assert_eq!(static_centers.len(), 2);

    for (ms, offset) in [(0, 0.0), (500, 1.0), (1000, 0.0), (1500, -1.0), (2000, 0.0)] {
        app.advance_simulation(start + Duration::from_millis(ms))
            .unwrap();
        let y = sphere_center(&app.simulation.scene, animated)[1];
        assert_eq!(y, bob.rest_y + offset * bob.amplitude);
        for &(entity, center) in &static_centers {
            assert_eq!(sphere_center(&app.simulation.scene, entity), center);
        }
    }
}

#[test]
fn catch_up_keeps_the_penultimate_sphere_state_and_resume_preserves_phase() {
    let start = Instant::now();
    let at = |ms| start + Duration::from_millis(ms);
    let mut batched = App::new(Simulation::new(Scene::demo()).unwrap());
    let mut split = App::new(Simulation::new(Scene::demo()).unwrap());
    batched.start_simulation(start);
    split.start_simulation(start);
    let (animated, _) = animated_sphere(&batched.simulation.scene);
    let (split_sphere, _) = animated_sphere(&split.simulation.scene);
    assert_eq!(batched.previous.sphere_center(animated), None);

    split.advance_simulation(at(84)).unwrap();
    let penultimate = sphere_center(&split.simulation.scene, split_sphere);
    split.advance_simulation(at(100)).unwrap();
    batched.advance_simulation(at(100)).unwrap();
    assert_eq!(batched.previous.sphere_center(animated), Some(penultimate));
    assert_eq!(
        sphere_center(&batched.simulation.scene, animated),
        sphere_center(&split.simulation.scene, split_sphere)
    );

    batched.drop_renderer();
    batched.start_simulation(at(1000));
    assert_eq!(batched.previous.sphere_center(animated), None);
    batched.advance_simulation(at(1017)).unwrap();
    split.advance_simulation(at(117)).unwrap();
    assert_eq!(
        sphere_center(&batched.simulation.scene, animated),
        sphere_center(&split.simulation.scene, split_sphere)
    );
}

fn animated_sphere(scene: &Scene) -> (Entity, SineBob) {
    let mut query = scene.world.query::<(Entity, &SineBob)>();
    let mut animated = query.iter();
    let (entity, bob) = animated.next().unwrap();
    assert!(animated.next().is_none());
    (entity, *bob)
}

fn sphere_center(scene: &Scene, entity: Entity) -> [f32; 3] {
    scene.world.get::<&Sphere>(entity).unwrap().center
}
