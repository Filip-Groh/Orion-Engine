mod app;
mod checkpoint;
mod freecam;
mod output_file;
mod rates;
mod simulation;
mod tick;
mod video;

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};
use orion_renderer::TransformSnapshot;
use orion_scene::Scene;

use crate::{simulation::Simulation, video::VideoOptions};

#[derive(Parser)]
#[command(
    about = "Run Orion or render a saved simulation",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    /// Add an OBJ mesh to the interactive demo.
    obj: Option<PathBuf>,
    /// Resume a checkpoint in the interactive demo.
    #[arg(long, conflicts_with = "obj")]
    load: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Action>,
}

#[derive(Subcommand)]
enum Action {
    /// Advance the demo without a window and save a checkpoint at an exact tick.
    Snapshot {
        output: PathBuf,
        #[arg(long)]
        tick: u64,
        #[arg(long)]
        obj: Option<PathBuf>,
    },
    /// Render ticks from a checkpoint to an H.264 MP4 using FFmpeg.
    Render {
        checkpoint: PathBuf,
        output: PathBuf,
        #[command(flatten)]
        options: VideoOptions,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Action::Snapshot { output, tick, obj }) => {
            let mut simulation = demo(obj.as_deref())?;
            simulation.advance_to(tick, &mut TransformSnapshot::default())?;
            checkpoint::save(&simulation, &output)?;
            println!("Saved tick {tick} to {}", output.display());
            Ok(())
        }
        Some(Action::Render {
            checkpoint: path,
            output,
            options,
        }) => video::export(checkpoint::load(&path)?, &output, options),
        None => {
            let simulation = match cli.load {
                Some(path) => checkpoint::load(&path)?,
                None => demo(cli.obj.as_deref())?,
            };
            app::App::new(simulation).run()
        }
    }
}

fn demo(obj: Option<&Path>) -> Result<Simulation> {
    let mut scene = Scene::demo();
    if let Some(path) = obj {
        scene.import_obj(path, [0.65, 0.65, 0.65])?;
    }
    Simulation::new(scene)
}
