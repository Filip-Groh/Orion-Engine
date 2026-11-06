use std::{
    collections::VecDeque,
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::Path,
};

use anyhow::{Context, Result, ensure};
use orion_scene::SceneSnapshot;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::{
    freecam::{Freecam, FreecamInput},
    simulation::Simulation,
};

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    version: u32,
    tick: u64,
    scene: SceneSnapshot,
    freecam: Freecam,
    input: VecDeque<(u64, FreecamInput)>,
}

pub(crate) fn save(simulation: &Simulation, path: &Path) -> Result<()> {
    let checkpoint = Checkpoint {
        version: VERSION,
        tick: simulation.tick,
        scene: SceneSnapshot::capture(&simulation.scene)?,
        freecam: simulation.freecam.clone(),
        input: simulation.input.clone(),
    };
    let mut temp = NamedTempFile::new_in(parent_directory(path))
        .with_context(|| format!("create a temporary checkpoint for {}", path.display()))?;
    {
        let mut writer = BufWriter::new(temp.as_file_mut());
        serde_json::to_writer(&mut writer, &checkpoint).context("write the checkpoint")?;
        writer.flush().context("flush the checkpoint")?;
    }
    temp.as_file().sync_all().context("sync the checkpoint")?;
    temp.persist(path)
        .with_context(|| format!("save {}", path.display()))?;
    Ok(())
}

pub(crate) fn load(path: &Path) -> Result<Simulation> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let checkpoint: Checkpoint = serde_json::from_reader(BufReader::new(file))
        .with_context(|| format!("read checkpoint {}", path.display()))?;
    ensure!(
        checkpoint.version == VERSION,
        "unsupported checkpoint version {}",
        checkpoint.version
    );
    let mut last = checkpoint.tick;
    for (tick, _) in &checkpoint.input {
        ensure!(
            *tick > checkpoint.tick && *tick >= last,
            "checkpoint inputs must be ordered and scheduled after the saved tick"
        );
        last = *tick;
    }
    Ok(Simulation {
        scene: checkpoint.scene.restore()?,
        tick: checkpoint.tick,
        freecam: checkpoint.freecam,
        input: checkpoint.input,
    })
}

pub(crate) fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

#[cfg(test)]
mod tests {
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
}
