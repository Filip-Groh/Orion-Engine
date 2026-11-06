use std::{
    io::Write,
    num::{NonZeroU32, NonZeroU64},
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, ensure};
use clap::Args;
use orion_renderer::{OffscreenRenderer, SceneFrame, TransformSnapshot};
use tempfile::NamedTempFile;

use crate::{checkpoint::parent_directory, simulation::Simulation};

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

struct Timeline {
    start: u64,
    end: u64,
    tps: u128,
    fps: u128,
    frames: u64,
}

impl Timeline {
    fn new(start: u64, ticks: u64, tps: NonZeroU32, fps: NonZeroU32) -> Result<Self> {
        let end = start
            .checked_add(ticks)
            .context("export would overflow the simulation tick counter")?;
        let tps = u128::from(tps.get());
        let fps = u128::from(fps.get());
        let frames = u64::try_from((u128::from(ticks) * fps).div_ceil(tps))
            .context("export would contain too many frames")?;
        Ok(Self {
            start,
            end,
            tps,
            fps,
            frames,
        })
    }

    /// Returns the enclosing completed tick and the blend from its predecessor.
    fn sample(&self, index: u64) -> (u64, f32) {
        let position = u128::from(index) * self.tps;
        let tick = self.start + position.div_ceil(self.fps) as u64;
        let remainder = position % self.fps;
        let fraction = if remainder == 0 {
            1.0
        } else {
            (remainder as f64 / self.fps as f64) as f32
        };
        (tick, fraction)
    }
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
    let timeline = Timeline::new(simulation.tick, ticks.get(), tps, fps)?;
    let mut renderer =
        OffscreenRenderer::new([width, height]).context("initialize offline Vulkan rendering")?;
    let temp = NamedTempFile::new_in(parent_directory(output))
        .with_context(|| format!("create a temporary video for {}", output.display()))?
        .into_temp_path();
    let mut encoder = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .args(["-f", "rawvideo", "-pixel_format", "rgba"])
        .args([
            "-video_size",
            &format!("{width}x{height}"),
            "-framerate",
            &fps.to_string(),
        ])
        .args([
            "-i",
            "pipe:0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ])
        .arg(temp.as_os_str())
        .stdin(Stdio::piped())
        .spawn()
        .context("start FFmpeg (install a build with libx264 and add ffmpeg to PATH)")?;

    let stdin = encoder.stdin.take().expect("FFmpeg stdin was piped");
    let result = write_frames(&mut simulation, &timeline, &mut renderer, stdin);
    if result.is_err() {
        let _ = encoder.kill();
    }
    let status = encoder.wait();
    result?;
    let status = status.context("wait for FFmpeg")?;
    ensure!(status.success(), "FFmpeg exited with {status}");
    temp.persist_noclobber(output)
        .with_context(|| format!("save video to {}", output.display()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline(start: u64, ticks: u64, tps: u32, fps: u32) -> Timeline {
        Timeline::new(
            start,
            ticks,
            tps.try_into().unwrap(),
            fps.try_into().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn samples_the_checkpoint_and_interpolates_through_the_last_tick() {
        let video = timeline(1000, 100, 20, 60);
        assert_eq!(video.frames, 300);
        assert_eq!(video.sample(0), (1000, 1.0));
        assert_eq!(video.sample(1), (1001, 1.0 / 3.0));
        assert_eq!(video.sample(2), (1001, 2.0 / 3.0));
        assert_eq!(video.sample(3), (1001, 1.0));
        assert_eq!(video.sample(299), (1100, 2.0 / 3.0));
    }

    #[test]
    fn rounds_fractional_frame_counts_up_and_supports_skipping_ticks() {
        let video = timeline(1000, 5, 30, 24);
        assert_eq!(video.frames, 4);
        assert_eq!(video.sample(1), (1002, 0.25));
        assert_eq!(video.sample(3), (1004, 0.75));

        let short = timeline(u64::MAX - 1, 1, 30, 24);
        assert_eq!(short.frames, 1);
        assert_eq!(short.sample(0), (u64::MAX - 1, 1.0));
    }
}
