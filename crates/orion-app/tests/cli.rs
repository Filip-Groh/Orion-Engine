use std::{fs, io::BufReader, process::Command};

use anyhow::Result;
use orion_scene::{Mesh, SceneSnapshot};
use serde::Deserialize;

#[derive(Deserialize)]
struct SavedCheckpoint {
    tick: u64,
    scene: SceneSnapshot,
}

fn saved_checkpoint(directory: &tempfile::TempDir) -> Result<SavedCheckpoint> {
    let file = fs::File::open(directory.path().join("saved.json"))?;
    Ok(serde_json::from_reader(BufReader::new(file))?)
}

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

    let checkpoint = saved_checkpoint(&directory)?;
    assert_eq!(checkpoint.tick, 17);
    let scene = checkpoint.scene.restore()?;
    let vertices = [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
    assert!(
        scene
            .world
            .query::<&Mesh>()
            .iter()
            .any(|mesh| mesh.triangles[0].vertices == vertices)
    );
    Ok(())
}

#[test]
fn snapshot_embeds_the_glb_default_scene_with_node_transforms() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let positions = [
        [0.0_f32, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ];
    let mut bin: Vec<_> = positions
        .into_iter()
        .flatten()
        .flat_map(f32::to_le_bytes)
        .collect();
    bin.extend([2_u16, 0, 1].into_iter().flat_map(u16::to_le_bytes));
    let mut json = serde_json::to_vec(&serde_json::json!({
        "asset": {"version": "2.0"},
        "scene": 1,
        "scenes": [{"nodes": [2]}, {"nodes": [0]}],
        "nodes": [
            {"children": [1], "translation": [5, 3, 2], "scale": [1, 2, 1]},
            {"mesh": 0, "translation": [1, 1, 0], "scale": [-2, 1, 1]},
            {"mesh": 0}
        ],
        "meshes": [{"primitives": [
            {"attributes": {"POSITION": 0}, "indices": 1},
            {"attributes": {"POSITION": 0}, "mode": 5}
        ]}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": [
            {"buffer": 0, "byteLength": 48},
            {"buffer": 0, "byteOffset": 48, "byteLength": 6}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3",
             "min": [0, 0, 0], "max": [1, 1, 0]},
            {"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}
        ]
    }))?;
    json.resize(json.len().next_multiple_of(4), b' ');
    bin.resize(bin.len().next_multiple_of(4), 0);
    let mut glb = b"glTF".to_vec();
    glb.extend(2_u32.to_le_bytes());
    glb.extend(((28 + json.len() + bin.len()) as u32).to_le_bytes());
    glb.extend((json.len() as u32).to_le_bytes());
    glb.extend(b"JSON");
    glb.extend(json);
    glb.extend((bin.len() as u32).to_le_bytes());
    glb.extend(b"BIN\0");
    glb.extend(bin);
    fs::write(directory.path().join("extra.GLB"), glb)?;

    let result = orion(&directory)
        .args([
            "snapshot",
            "saved.json",
            "--tick",
            "0",
            "--mesh",
            "extra.GLB",
        ])
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let scene = saved_checkpoint(&directory)?.scene.restore()?;
    let mut meshes = scene.world.query::<&Mesh>();
    let mesh = meshes
        .iter()
        .find(|mesh| mesh.triangles.len() == 3)
        .expect("imported GLB mesh");
    let a = [6.0, 5.0, 2.0];
    let b = [4.0, 5.0, 2.0];
    let c = [6.0, 7.0, 2.0];
    let d = [4.0, 7.0, 2.0];
    for (triangle, expected) in mesh.triangles.iter().zip([[c, b, a], [a, c, b], [c, d, b]]) {
        assert_eq!(triangle.vertices, expected);
        assert_eq!(triangle.albedo, [0.65; 3]);
    }
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
