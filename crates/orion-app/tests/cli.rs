use std::{fs, process::Command};

use anyhow::Result;
use serde_json::Value;

fn orion(directory: &tempfile::TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_orion-app"));
    command.current_dir(directory.path());
    command
}

#[test]
fn snapshot_saves_the_requested_tick_and_embeds_the_obj() -> Result<()> {
    let directory = tempfile::tempdir()?;
    fs::write(
        directory.path().join("extra.obj"),
        "v 1 0 0\nv 2 0 0\nv 1 1 0\nf 1 2 3\n",
    )?;
    let result = orion(&directory)
        .args([
            "snapshot",
            "saved.json",
            "--tick",
            "17",
            "--obj",
            "extra.obj",
        ])
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let checkpoint: Value =
        serde_json::from_slice(&fs::read(directory.path().join("saved.json"))?)?;
    assert_eq!(checkpoint["tick"], 17);
    let entities = checkpoint["scene"]["entities"].as_array().unwrap();
    let vertices = serde_json::json!([[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 1.0, 0.0]]);
    assert!(
        entities
            .iter()
            .any(|entity| entity["mesh"]["triangles"][0]["vertices"] == vertices)
    );
    Ok(())
}

#[test]
fn render_preserves_an_existing_output() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let result = orion(&directory)
        .args(["snapshot", "saved.json", "--tick", "0"])
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let output = directory.path().join("existing.mp4");
    fs::write(&output, b"existing output")?;
    let result = orion(&directory)
        .args(["render", "saved.json", "existing.mp4", "--ticks", "1"])
        .output()?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("existing.mp4 already exists"));
    assert_eq!(fs::read(output)?, b"existing output");
    Ok(())
}
