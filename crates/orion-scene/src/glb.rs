use std::collections::HashSet;

use anyhow::{Context, Result, ensure};
use gltf::{
    Accessor, Document, Semantic,
    accessor::{DataType, Dimensions},
    binary::Glb,
    buffer::{Source, View},
    mesh::Mode,
};

use crate::Triangle;

// glTF matrices store columns, with translation in the fourth column.
type Matrix = [[f32; 4]; 4];

const IDENTITY: Matrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

pub(crate) fn parse(source: &[u8], albedo: [f32; 3]) -> Result<Vec<Triangle>> {
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

    let mut triangles = Vec::new();
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
                append_mesh(mesh, bin, transform, albedo, &mut triangles)?;
            }
            pending.extend(node.children().map(|child| (child, transform)));
        }
    } else {
        for mesh in document.meshes() {
            append_mesh(mesh, bin, IDENTITY, albedo, &mut triangles)?;
        }
    }

    ensure!(!triangles.is_empty(), "GLB contains no triangle geometry");
    Ok(triangles)
}

fn append_mesh(
    mesh: gltf::Mesh<'_>,
    bin: &[u8],
    transform: Matrix,
    albedo: [f32; 3],
    triangles: &mut Vec<Triangle>,
) -> Result<()> {
    let [[ax, ay, az, _], [bx, by, bz, _], [cx, cy, cz, _], _] =
        transform.map(|column| column.map(f64::from));
    let determinant =
        ax * (by * cz - bz * cy) - bx * (ay * cz - az * cy) + cx * (ay * bz - az * by);
    for primitive in mesh.primitives() {
        let index = primitive.index();
        append_primitive(
            primitive,
            bin,
            transform,
            determinant < 0.0,
            albedo,
            triangles,
        )
        .with_context(|| format!("GLB mesh {} primitive {index}", mesh.index()))?;
    }
    Ok(())
}

fn append_primitive(
    primitive: gltf::Primitive<'_>,
    bin: &[u8],
    transform: Matrix,
    mirrored: bool,
    albedo: [f32; 3],
    triangles: &mut Vec<Triangle>,
) -> Result<()> {
    let mode = primitive.mode();
    if !matches!(
        mode,
        Mode::Triangles | Mode::TriangleStrip | Mode::TriangleFan
    ) {
        return Ok(());
    }

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

    let mut push_triangle = |a, b, c| -> Result<()> {
        let vertex = |index| {
            vertices
                .get(index)
                .copied()
                .with_context(|| format!("vertex index {index} is out of bounds"))
        };
        let mut vertices = [vertex(a)?, vertex(b)?, vertex(c)?];
        if mirrored {
            vertices.swap(1, 2);
        }
        triangles.push(Triangle { vertices, albedo });
        Ok(())
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
