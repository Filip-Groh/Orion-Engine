use std::fs;

use orion_renderer::TransformSnapshot;
use orion_scene::{Camera, Scene};
use winit::keyboard::KeyCode;

use super::*;

#[test]
fn checkpoint_continues_with_the_same_scene_and_inputs() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let continuous_path = directory.path().join("continuous.json");
    let restored_path = directory.path().join("restored.json");
    let mut scene = Scene::demo();
    scene.active_camera = scene.world.spawn((Camera {
        eye: [0.0, 0.0, 3.0],
        ..scene.camera()?
    },));
    let mut continuous = Simulation::new(scene)?;
    continuous.input.extend([
        (
            999,
            FreecamInput::Key {
                key: KeyCode::KeyW,
                pressed: true,
            },
        ),
        (
            1002,
            FreecamInput::Look {
                dx: 123.25,
                dy: -803.5,
            },
        ),
        (
            1002,
            FreecamInput::Look {
                dx: 2.75,
                dy: 45.25,
            },
        ),
        (1010, FreecamInput::ReleaseAll),
    ]);
    let mut previous = TransformSnapshot::default();
    continuous.advance_to(1000, &mut previous)?;
    save(&continuous, &continuous_path)?;

    let mut restored = load(&continuous_path)?;
    assert_eq!(restored.tick, 1000);
    assert_eq!(restored.scene.camera()?.eye, continuous.scene.camera()?.eye);
    save(&restored, &restored_path)?;
    assert_eq!(fs::read(&continuous_path)?, fs::read(&restored_path)?);

    continuous.advance_to(1100, &mut previous)?;
    restored.advance_to(1100, &mut previous)?;
    save(&continuous, &continuous_path)?;
    save(&restored, &restored_path)?;
    assert_eq!(fs::read(&continuous_path)?, fs::read(&restored_path)?);
    Ok(())
}

const VERSION_ONE_CHECKPOINT: &str = r#"{
    "version": 1,
    "tick": 10,
    "scene": {
        "active_camera": 0,
        "entities": [{
            "camera": {
                "eye": [0.0, 0.0, 0.0],
                "target": [0.0, 0.0, -1.0],
                "vertical_fov_degrees": 38.0
            }
        }]
    },
    "freecam": {"keys": [], "yaw": 0.0, "pitch": 0.0},
    "input": [[11, "ReleaseAll"], [12, "ReleaseAll"]]
}"#;

#[test]
fn new_input_runs_after_saved_input_for_the_same_tick() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("version-one.json");
    fs::write(&path, VERSION_ONE_CHECKPOINT)?;
    let mut simulation = load(&path)?;

    simulation.queue_input(
        11,
        FreecamInput::Key {
            key: KeyCode::KeyW,
            pressed: true,
        },
    );
    let mut previous = TransformSnapshot::default();
    simulation.advance_to(11, &mut previous)?;
    let eye = simulation.scene.camera()?.eye;
    assert!(eye[2] < 0.0);
    simulation.advance_to(12, &mut previous)?;
    assert_eq!(simulation.scene.camera()?.eye, eye);
    Ok(())
}

#[test]
fn rejects_unsupported_versions_and_invalid_input_schedules() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("invalid.json");
    let version_one: serde_json::Value = serde_json::from_str(VERSION_ONE_CHECKPOINT)?;
    let mut unsupported = version_one.clone();
    unsupported["version"] = 2.into();
    fs::write(&path, serde_json::to_vec(&unsupported)?)?;
    assert_eq!(
        load(&path).err().unwrap().to_string(),
        "unsupported checkpoint version 2"
    );

    for ticks in [[10, 11], [12, 11]] {
        let mut invalid = version_one.clone();
        invalid["input"][0][0] = ticks[0].into();
        invalid["input"][1][0] = ticks[1].into();
        fs::write(&path, serde_json::to_vec(&invalid)?)?;
        assert_eq!(
            load(&path).err().unwrap().to_string(),
            "checkpoint inputs must be ordered and scheduled after the saved tick"
        );
    }
    Ok(())
}
