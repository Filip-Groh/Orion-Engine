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
    /// Add an OBJ or GLB mesh to the interactive demo.
    mesh: Option<PathBuf>,
    /// Resume a checkpoint in the interactive demo.
    #[arg(long, conflicts_with = "mesh")]
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
        /// Add an OBJ or GLB mesh to the checkpoint.
        #[arg(long, visible_alias = "obj")]
        mesh: Option<PathBuf>,
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
        Some(Action::Snapshot { output, tick, mesh }) => {
            let mut simulation = demo(mesh.as_deref())?;
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
                None => demo(cli.mesh.as_deref())?,
            };
            app::App::new(simulation).run()
        }
    }
}

fn demo(mesh: Option<&Path>) -> Result<Simulation> {
    let mut scene = Scene::demo();
    if let Some(path) = mesh {
        let albedo = [0.65; 3];
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("glb"))
        {
            scene.import_glb(path, albedo)?;
        } else {
            scene.import_obj(path, albedo)?;
        }
    }
    Simulation::new(scene)
}
