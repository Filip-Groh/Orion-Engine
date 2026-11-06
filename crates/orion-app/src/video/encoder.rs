use std::{
    num::NonZeroU32,
    path::Path,
    process::{ChildStdin, Command, Stdio},
};

use anyhow::{Context, Result, ensure};

use crate::output_file;

pub(super) fn encode(
    output: &Path,
    [width, height]: [u32; 2],
    fps: NonZeroU32,
    write_frames: impl FnOnce(ChildStdin) -> Result<()>,
) -> Result<()> {
    let temp = output_file::temporary(output, "video")?.into_temp_path();
    let mut encoder = ffmpeg_command([width, height], fps, &temp)
        .spawn()
        .context("start FFmpeg (install a build with libx264 and add ffmpeg to PATH)")?;

    let stdin = encoder.stdin.take().expect("FFmpeg stdin was piped");
    let result = write_frames(stdin);
    if result.is_err() {
        let _ = encoder.kill();
    }
    let status = encoder.wait();
    result?;
    let status = status.context("wait for FFmpeg")?;
    ensure!(status.success(), "FFmpeg exited with {status}");
    temp.persist_noclobber(output)
        .with_context(|| format!("save video to {}", output.display()))?;
    Ok(())
}

fn ffmpeg_command([width, height]: [u32; 2], fps: NonZeroU32, path: &Path) -> Command {
    let mut command = Command::new("ffmpeg");
    command
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
        .arg(path)
        .stdin(Stdio::piped());
    command
}
