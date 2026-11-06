mod textures;

use std::{collections::HashSet, path::Path};

use anyhow::{Context, Result, ensure};
use gltf::{
    Accessor, Document, Semantic,
    accessor::{DataType, Dimensions},
    binary::Glb,
    buffer::{Source, View},
    mesh::Mode,
};

use crate::{MaterialMaps, Mesh, Texture, texture::ImportedMaterial};
use textures::{Textures, UvSet};

// glTF matrices store columns, with translation in the fourth column.
type Matrix = [[f32; 4]; 4];

const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub(crate) fn parse(
    source: &[u8],
    albedo: [f32; 3],
    directory: Option<&Path>,
    texture_base: usize,
) -> Result<(Mesh, Vec<Texture>)> {
    ensure!(
        albedo.iter().all(|color| (0.0..=1.0).contains(color)),
        "GLB albedo must be in 0..=1"
    );

    let length = source.get(8..12).context("incomplete GLB header")?;
    ensure!(
        u32::from_le_bytes(length.try_into()?) as usize == source.len(),
        "GLB header length does not match the file size"
    );
    let asset = Glb::from_slice(source)?;
    let blob = asset.bin.as_deref().context("GLB has no binary chunk")?;
    let document = Document::from_json(gltf::json::deserialize::from_slice(&asset.json)?)?;
    let buffer = document.buffers().next().context("GLB has no buffers")?;
    ensure!(
        matches!(buffer.source(), Source::Bin),
        "GLB's first buffer must refer to the binary chunk"
    );
    let bin = blob
        .get(..buffer.length())
        .context("GLB buffer exceeds the binary chunk")?;

    let mut mesh_data = Mesh::default();
    let mut textures = Textures::new(bin, directory, texture_base);
    if let Some(scene) = document
        .default_scene()
        .or_else(|| document.scenes().next())
    {
        let mut pending: Vec<_> = scene.nodes().map(|node| (node, IDENTITY)).collect();
        let mut visited = HashSet::new();
        while let Some((node, parent)) = pending.pop() {
            ensure!(
                visited.insert(node.index()),
                "GLB scene contains repeated node {}",
                node.index()
            );
            let transform = node
                .transform()
                .matrix()
                .map(|column| transform_vector(parent, column));
            if let Some(mesh) = node.mesh() {
                append_mesh(mesh, transform, albedo, &mut textures, &mut mesh_data)?;
            }
            pending.extend(node.children().map(|child| (child, transform)));
        }
    } else {
        for mesh in document.meshes() {
            append_mesh(mesh, IDENTITY, albedo, &mut textures, &mut mesh_data)?;
        }
    }

    ensure!(
        !mesh_data.triangles.is_empty(),
        "GLB contains no triangle geometry"
    );
    Ok((mesh_data, textures.images))
}

fn append_mesh(
    mesh: gltf::Mesh<'_>,
    transform: Matrix,
    albedo: [f32; 3],
    textures: &mut Textures<'_>,
    mesh_data: &mut Mesh,
) -> Result<()> {
    let [[ax, ay, az, _], [bx, by, bz, _], [cx, cy, cz, _], _] =
        transform.map(|column| column.map(f64::from));
    let determinant =
        ax * (by * cz - bz * cy) - bx * (ay * cz - az * cy) + cx * (ay * bz - az * by);
    for primitive in mesh.primitives() {
        let index = primitive.index();
        append_primitive(
            primitive,
            transform,
            determinant < 0.0,
            albedo,
            textures,
            mesh_data,
        )
        .with_context(|| format!("GLB mesh {} primitive {index}", mesh.index()))?;
    }
    Ok(())
}

fn append_primitive(
    primitive: gltf::Primitive<'_>,
    transform: Matrix,
    mirrored: bool,
    albedo: [f32; 3],
    textures: &mut Textures<'_>,
    mesh: &mut Mesh,
) -> Result<()> {
    let mode = primitive.mode();
    if !matches!(
        mode,
        Mode::Triangles | Mode::TriangleStrip | Mode::TriangleFan
    ) {
        return Ok(());
    }

    let bin = textures.bin;
    let positions = primitive
        .get(&Semantic::Positions)
        .context("missing vertex positions")?;
    ensure!(
        positions.dimensions() == Dimensions::Vec3 && positions.data_type() == DataType::F32,
        "vertex positions must be VEC3 floats"
    );
    validate_accessor(positions, bin.len())?;

    let reader = primitive.reader(|buffer| (buffer.index() == 0).then_some(bin));
    let positions = reader
        .read_positions()
        .context("cannot read vertex positions")?;
    let vertices = positions
        .map(|[x, y, z]| {
            let [x, y, z, _] = transform_vector(transform, [x, y, z, 1.0]);
            let vertex = [x, y, z];
            ensure!(
                vertex.iter().all(|value| value.is_finite()),
                "vertex coordinates must be finite"
            );
            Ok(vertex)
        })
        .collect::<Result<Vec<_>>>()?;
    let indices: Vec<usize> = match primitive.indices() {
        Some(indices) => {
            ensure!(
                indices.dimensions() == Dimensions::Scalar
                    && matches!(
                        indices.data_type(),
                        DataType::U8 | DataType::U16 | DataType::U32
                    ),
                "triangle indices must be unsigned scalars"
            );
            validate_accessor(indices, bin.len())?;
            reader
                .read_indices()
                .context("cannot read triangle indices")?
                .into_u32()
                .map(|index| index as usize)
                .collect()
        }
        None => (0..vertices.len()).collect(),
    };
    ensure!(
        indices.len() >= 3 && (mode != Mode::Triangles || indices.len().is_multiple_of(3)),
        "invalid vertex count for {mode:?}"
    );

    let source_material = primitive.material();
    let pbr = source_material.pbr_metallic_roughness();
    let base_color = pbr.base_color_texture();
    let metallic_roughness = pbr.metallic_roughness_texture();
    let mut material = ImportedMaterial::new(albedo);
    if source_material.index().is_some() {
        let [r, g, b, _] = pbr.base_color_factor();
        material = ImportedMaterial {
            albedo: [r, g, b],
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            maps: MaterialMaps {
                base_color: base_color
                    .as_ref()
                    .map(|info| textures.load(info))
                    .transpose()?,
                metallic_roughness: metallic_roughness
                    .as_ref()
                    .map(|info| textures.load(info))
                    .transpose()?,
                ..MaterialMaps::default()
            },
        };
    }
    let read_uvs = |set: UvSet, required: bool| -> Result<Option<Vec<[f32; 2]>>> {
        let Some(accessor) = primitive.get(&Semantic::TexCoords(set.index)) else {
            ensure!(!required, "texture requires TEXCOORD_{}", set.index);
            return Ok(None);
        };
        ensure!(
            accessor.dimensions() == Dimensions::Vec2 && accessor.count() == vertices.len(),
            "TEXCOORD_{} must have one VEC2 per vertex",
            set.index
        );
        ensure!(
            accessor.data_type() == DataType::F32
                || (matches!(accessor.data_type(), DataType::U8 | DataType::U16)
                    && accessor.normalized()),
            "texture coordinates must be floats or normalized unsigned integers"
        );
        validate_accessor(accessor, bin.len())?;
        let uvs: Vec<_> = reader
            .read_tex_coords(set.index)
            .context("cannot read texture coordinates")?
            .into_f32()
            .map(|uv| set.apply(uv))
            .collect();
        ensure!(
            uvs.len() == vertices.len() && uvs.iter().flatten().all(|v| v.is_finite()),
            "invalid texture coordinates"
        );
        Ok(Some(uvs))
    };
    let first_map = base_color.as_ref().or(metallic_roughness.as_ref());
    let primary = UvSet::new(first_map);
    let primary_uvs = read_uvs(primary, first_map.is_some())?;
    let secondary_uvs = if let Some(info) = metallic_roughness.as_ref() {
        let secondary = UvSet::new(Some(info));
        if secondary == primary {
            None
        } else {
            read_uvs(secondary, true)?
        }
    } else {
        None
    };

    let mut push_triangle = |a, b, c| -> Result<()> {
        let mut indices = [a, b, c];
        if mirrored {
            indices.swap(1, 2);
        }
        for index in indices {
            ensure!(
                index < vertices.len(),
                "vertex index {index} is out of bounds"
            );
        }
        mesh.push_triangle(
            indices.map(|index| vertices[index]),
            material,
            primary_uvs
                .as_ref()
                .map(|uvs| indices.map(|index| uvs[index])),
            secondary_uvs
                .as_ref()
                .map(|uvs| indices.map(|index| uvs[index])),
        )
    };

    match mode {
        Mode::Triangles => {
            for &[a, b, c] in indices.as_chunks::<3>().0 {
                push_triangle(a, b, c)?;
            }
        }
        Mode::TriangleStrip => {
            for (index, &[a, b, c]) in indices.array_windows::<3>().enumerate() {
                if index % 2 == 0 {
                    push_triangle(a, b, c)?;
                } else {
                    push_triangle(b, a, c)?;
                }
            }
        }
        Mode::TriangleFan => {
            for &[b, c] in indices[1..].array_windows::<2>() {
                push_triangle(indices[0], b, c)?;
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

// The gltf reader assumes positive counts and in-bounds byte ranges.
fn validate_accessor(accessor: Accessor<'_>, buffer_len: usize) -> Result<()> {
    let count = accessor.count();
    let size = accessor.size();
    ensure!(count > 0, "accessor has no elements");
    let validate =
        |view, offset, count, size| validate_range(view, offset, count, size, buffer_len);
    if let Some(view) = accessor.view() {
        validate(view, accessor.offset(), count, size)?;
    }
    if let Some(sparse) = accessor.sparse() {
        ensure!(
            (1..=count).contains(&sparse.count()),
            "invalid sparse accessor count"
        );
        let indices = sparse.indices();
        validate(
            indices.view(),
            indices.offset(),
            sparse.count(),
            indices.index_type().size(),
        )?;
        let values = sparse.values();
        validate(values.view(), values.offset(), sparse.count(), size)?;
    }
    Ok(())
}

fn validate_range(
    view: View<'_>,
    offset: usize,
    count: usize,
    size: usize,
    buffer_len: usize,
) -> Result<()> {
    ensure!(
        view.buffer().index() == 0,
        "geometry must be embedded in the GLB"
    );
    ensure!(
        view.offset() <= buffer_len && view.length() <= buffer_len - view.offset(),
        "buffer view {} exceeds the binary chunk",
        view.index()
    );
    let stride = view.stride().unwrap_or(size);
    ensure!(
        stride >= size,
        "buffer view stride is smaller than an element"
    );
    let available = view
        .length()
        .checked_sub(offset)
        .context("invalid accessor offset")?;
    ensure!(
        available >= size && count - 1 <= (available - size) / stride,
        "accessor exceeds its buffer view"
    );
    Ok(())
}

fn transform_vector(matrix: Matrix, [x, y, z, w]: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|row| {
        matrix[0][row] * x + matrix[1][row] * y + matrix[2][row] * z + matrix[3][row] * w
    })
}
