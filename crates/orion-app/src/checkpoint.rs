use std::{
    collections::VecDeque,
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::Path,
};

use anyhow::{Context, Result, ensure};
use orion_scene::SceneSnapshot;
use serde::{Deserialize, Serialize};

use crate::{
    freecam::{Freecam, FreecamInput},
    output_file,
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

impl Checkpoint {
    fn capture(simulation: &Simulation) -> Result<Self> {
        Ok(Self {
            version: VERSION,
            tick: simulation.tick,
            scene: SceneSnapshot::capture(&simulation.scene)?,
            freecam: simulation.freecam.clone(),
            input: simulation.input.clone(),
        })
    }

    fn restore(self) -> Result<Simulation> {
        ensure!(
            self.version == VERSION,
            "unsupported checkpoint version {}",
            self.version
        );
        let mut last = self.tick;
        for (tick, _) in &self.input {
            ensure!(
                *tick > self.tick && *tick >= last,
                "checkpoint inputs must be ordered and scheduled after the saved tick"
            );
            last = *tick;
        }
        Ok(Simulation {
            scene: self.scene.restore()?,
            tick: self.tick,
            freecam: self.freecam,
            input: self.input,
        })
    }
}

pub(crate) fn save(simulation: &Simulation, path: &Path) -> Result<()> {
    let checkpoint = Checkpoint::capture(simulation)?;
    let mut temp = output_file::temporary(path, "checkpoint")?;
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
    checkpoint.restore()
}

#[cfg(test)]
mod tests;
