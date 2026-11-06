use std::{fs, io::BufReader, process::Command};

use anyhow::Result;
use orion_scene::{Mesh, SceneSnapshot, WrapMode};
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

// A 2 × 2 RGBA PNG with red and green above blue and white.
const TEST_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0,
    0, 0, 114, 182, 13, 36, 0, 0, 0, 19, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 12,
    25, 24, 254, 131, 1, 0, 73, 200, 9, 247, 20, 99, 50, 157, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
    96, 130,
];

#[test]
fn snapshot_saves_the_requested_tick_and_embeds_obj_materials() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let assets = directory.path().join("assets");
    fs::create_dir(&assets)?;
    fs::write(assets.join("checker map.png"), TEST_PNG)?;
    fs::write(
        assets.join("extra library.mtl"),
        concat!(
            "newmtl patterned surface\nKd 0.8 0.6 0.4\nPm 0.4\nPr 0.5\n",
            "map_Kd -clamp on -s 2 3 -o 0.25 0.5 checker map.png\n",
            "map_Pm checker map.png\nmap_Pr checker map.png\n",
        ),
    )?;
    fs::write(
        directory.path().join("extra.obj"),
        concat!(
            "mtllib assets/extra library.mtl\nusemtl patterned surface\n",
            "v 1 0 0\nv 2 0 0\nv 1 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nf 1/-3 2/-2 3/-1\n"
        ),
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

    fs::remove_file(assets.join("checker map.png"))?;
    let checkpoint = saved_checkpoint(&directory)?;
    assert_eq!(checkpoint.tick, 17);
    let scene = checkpoint.scene.restore()?;
    let vertices = [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
    let mut meshes = scene.world.query::<&Mesh>();
    let mesh = meshes
        .iter()
        .find(|mesh| mesh.triangles[0].vertices == vertices)
        .unwrap();
    let triangle = mesh.triangles[0];
    assert_eq!(triangle.albedo, [0.8, 0.6, 0.4]);
    assert_eq!((triangle.metallic, triangle.roughness), (0.4, 0.5));
    let uvs = mesh.uvs[triangle.uv_index.unwrap() as usize];
    assert_eq!(uvs.primary, [[0.0, 1.0], [1.0, 1.0], [0.0, 0.0]]);
    assert_eq!(uvs.maps.metallic, uvs.maps.roughness);
    assert!(uvs.maps.roughness.is_some());
    let texture = &scene.textures[uvs.maps.base_color.unwrap() as usize];
    assert_eq!((texture.width, texture.height), (2, 2));
    assert_eq!(
        texture.texels,
        [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255; 4]
        ]
    );
    assert_eq!(texture.wrap_u, WrapMode::Clamp);
    assert_eq!(texture.scale, [2.0, 3.0]);
    assert_eq!(texture.offset, [0.25, -2.5]);
    Ok(())
}

#[test]
fn snapshot_embeds_glb_materials_and_keeps_uvs_with_mirrored_vertices() -> Result<()> {
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
    bin.resize(56, 0);
    bin.extend(
        [0.0_f32, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0]
            .into_iter()
            .flat_map(f32::to_le_bytes),
    );
    bin.extend(
        [0_u16, 0, u16::MAX, 0, 0, u16::MAX, u16::MAX, u16::MAX]
            .into_iter()
            .flat_map(u16::to_le_bytes),
    );
    bin.extend_from_slice(TEST_PNG);
    fs::write(directory.path().join("external image.png"), TEST_PNG)?;
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
            {"attributes": {"POSITION": 0, "TEXCOORD_0": 2, "TEXCOORD_1": 3}, "indices": 1, "material": 0},
            {"attributes": {"POSITION": 0}, "mode": 5}
        ]}],
        "extensionsUsed": ["KHR_texture_transform"],
        "materials": [{"pbrMetallicRoughness": {
            "baseColorFactor": [0.5, 0.7, 0.9, 1.0], "metallicFactor": 0.2, "roughnessFactor": 0.3,
            "baseColorTexture": {"index": 0},
            "metallicRoughnessTexture": {"index": 1, "texCoord": 1,
                "extensions": {"KHR_texture_transform": {"offset": [0.25, 0.5], "scale": [0.5, 0.5]}}}
        }}],
        "samplers": [{"magFilter": 9728, "wrapS": 33648, "wrapT": 33071}],
        "textures": [{"sampler": 0, "source": 0}, {"source": 1}],
        "images": [{"bufferView": 4, "mimeType": "image/png"}, {"uri": "external%20image.png"}],
        "buffers": [{"byteLength": bin.len()}],
        "bufferViews": [
            {"buffer": 0, "byteLength": 48},
            {"buffer": 0, "byteOffset": 48, "byteLength": 6},
            {"buffer": 0, "byteOffset": 56, "byteLength": 32},
            {"buffer": 0, "byteOffset": 88, "byteLength": 16},
            {"buffer": 0, "byteOffset": 104, "byteLength": TEST_PNG.len()}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3",
             "min": [0, 0, 0], "max": [1, 1, 0]},
            {"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"},
            {"bufferView": 2, "componentType": 5126, "count": 4, "type": "VEC2"},
            {"bufferView": 3, "componentType": 5123, "normalized": true, "count": 4, "type": "VEC2"}
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

    fs::remove_file(directory.path().join("external image.png"))?;
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
    }
    let triangle = mesh.triangles[0];
    assert_eq!(triangle.albedo, [0.5, 0.7, 0.9]);
    assert_eq!((triangle.metallic, triangle.roughness), (0.2, 0.3));
    assert_eq!(mesh.triangles[1].albedo, [0.65; 3]);
    let uvs = mesh.uvs[triangle.uv_index.unwrap() as usize];
    assert_eq!(uvs.primary, [[0.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
    assert_eq!(
        uvs.metallic_roughness,
        Some([[0.25, 1.0], [0.75, 0.5], [0.25, 0.5]])
    );
    let base = &scene.textures[uvs.maps.base_color.unwrap() as usize];
    let factors = &scene.textures[uvs.maps.metallic_roughness.unwrap() as usize];
    assert_eq!(
        (base.wrap_u, base.wrap_v, base.linear_filter),
        (WrapMode::Mirror, WrapMode::Clamp, false)
    );
    assert_eq!(base.texels, factors.texels);
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
