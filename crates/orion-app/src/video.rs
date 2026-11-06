mod encoder;
mod timeline;

use std::{
    io::Write,
    num::{NonZeroU32, NonZeroU64},
    path::Path,
};

use anyhow::{Context, Result, ensure};
use clap::Args;
use orion_renderer::{OffscreenRenderer, SceneFrame, TransformSnapshot};

use crate::simulation::Simulation;
use timeline::Timeline;

#[derive(Args)]
pub(crate) struct VideoOptions {
    /// Number of simulation ticks to export.
    #[arg(long)]
    ticks: NonZeroU64,
    /// Playback speed in simulation ticks per video second.
    #[arg(long, default_value = "60")]
    tps: NonZeroU32,
    /// Video frames per second.
    #[arg(long, default_value = "60")]
    fps: NonZeroU32,
    /// Video width in pixels (even).
    #[arg(long, default_value_t = 960)]
    width: u32,
    /// Video height in pixels (even).
    #[arg(long, default_value_t = 540)]
    height: u32,
}

pub(crate) fn export(
    mut simulation: Simulation,
    output: &Path,
    options: VideoOptions,
) -> Result<()> {
    let VideoOptions {
        ticks,
        tps,
        fps,
        width,
        height,
    } = options;
    ensure!(
        width > 0 && height > 0 && width.is_multiple_of(2) && height.is_multiple_of(2),
        "H.264 video requires positive, even dimensions"
    );
    ensure!(!output.try_exists()?, "{} already exists", output.display());
    let timeline = Timeline::new(simulation.tick, ticks, tps, fps)?;
    let mut renderer =
        OffscreenRenderer::new([width, height]).context("initialize offline Vulkan rendering")?;
    encoder::encode(output, [width, height], fps, |stdin| {
        write_frames(&mut simulation, &timeline, &mut renderer, stdin)
    })?;
    println!(
        "Rendered {} frames at {fps} FPS from tick {} to {} into {}",
        timeline.frames,
        timeline.start,
        timeline.end,
        output.display()
    );
    Ok(())
}

fn write_frames(
    simulation: &mut Simulation,
    timeline: &Timeline,
    renderer: &mut OffscreenRenderer,
    mut output: impl Write,
) -> Result<()> {
    let mut previous = TransformSnapshot::default();
    for index in 0..timeline.frames {
        let (tick, fraction) = timeline.sample(index);
        simulation.advance_to(tick, &mut previous)?;
        let frame = SceneFrame::interpolated(&simulation.scene, &previous, fraction);
        let pixels = renderer
            .render(frame)
            .with_context(|| format!("render video frame {index}"))?;
        output
            .write_all(pixels)
            .with_context(|| format!("send video frame {index} to FFmpeg"))?;
    }
    simulation.advance_to(timeline.end, &mut previous)?;
    Ok(())
}
