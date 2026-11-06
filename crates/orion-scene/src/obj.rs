mod material;
mod triangulation;

use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::{Mesh, Texture, texture::ImportedMaterial};
use material::MaterialLibrary;
use triangulation::triangulate;

#[derive(Clone, Copy)]
struct Corner {
    position: [f32; 3],
    uv: Option<[f32; 2]>,
}

pub(crate) fn parse(
    source: &str,
    albedo: [f32; 3],
    directory: Option<&Path>,
    texture_base: usize,
) -> Result<(Mesh, Vec<Texture>)> {
    ensure!(
        albedo.iter().all(|color| (0.0..=1.0).contains(color)),
        "OBJ albedo must be in 0..=1"
    );
    let mut vertices = Vec::new();
    let mut texcoords = Vec::new();
    let mut mesh = Mesh::default();
    let fallback = ImportedMaterial::new(albedo);
    let mut library = MaterialLibrary::new(fallback, texture_base);
    let mut active = fallback;
    let source = source.trim_start_matches('\u{feff}');
    for (line_index, (keyword, rest)) in source.lines().map(statement).enumerate() {
        if keyword == "mtllib"
            && let Some(directory) = directory
        {
            library
                .load(directory, rest)
                .with_context(|| format!("OBJ line {}", line_index + 1))?;
        }
    }
    for (line_index, (keyword, rest)) in source.lines().map(statement).enumerate() {
        let fields = rest.split_whitespace();
        let result = match keyword {
            "v" => parse_vertex(fields).map(|vertex| vertices.push(vertex)),
            "vt" => parse_texcoord(fields).map(|uv| texcoords.push(uv)),
            "usemtl" => library.resolve(rest).map(|material| active = material),
            "f" => parse_face(fields, &vertices, &texcoords)
                .and_then(|face| triangulate(&face, active, &mut mesh)),
            _ => continue,
        };
        result.with_context(|| format!("OBJ line {}", line_index + 1))?;
    }
    ensure!(!mesh.triangles.is_empty(), "OBJ contains no polygon faces");
    Ok((mesh, library.textures))
}

fn statement(line: &str) -> (&str, &str) {
    let content = line.split('#').next().unwrap_or_default().trim();
    let (keyword, rest) =
        content.split_at(content.find(char::is_whitespace).unwrap_or(content.len()));
    (keyword, rest.trim_start())
}

fn parse_vertex(mut fields: std::str::SplitWhitespace<'_>) -> Result<[f32; 3]> {
    let mut vertex = [0.0; 3];
    for coordinate in &mut vertex {
        *coordinate = fields
            .next()
            .context("vertex requires three coordinates")?
            .parse::<f32>()
            .context("invalid vertex coordinate")?;
        ensure!(coordinate.is_finite(), "vertex coordinates must be finite");
    }
    Ok(vertex)
}

fn parse_texcoord(mut fields: std::str::SplitWhitespace<'_>) -> Result<[f32; 2]> {
    let u: f32 = fields
        .next()
        .context("texture coordinate requires U")?
        .parse()?;
    let v: f32 = fields.next().unwrap_or("0").parse()?;
    ensure!(
        u.is_finite() && v.is_finite(),
        "texture coordinates must be finite"
    );
    // OBJ V grows up. Texture rows grow down.
    Ok([u, 1.0 - v])
}

fn parse_face(
    fields: std::str::SplitWhitespace<'_>,
    vertices: &[[f32; 3]],
    texcoords: &[[f32; 2]],
) -> Result<Vec<Corner>> {
    let mut face = Vec::new();
    for corner in fields {
        let mut indices = corner.split('/');
        let position = face_value(indices.next().unwrap_or_default(), vertices, "vertex")?;
        let uv = indices
            .next()
            .filter(|index| !index.is_empty())
            .map(|index| face_value(index, texcoords, "texture coordinate"))
            .transpose()?;
        face.push(Corner { position, uv });
    }
    ensure!(face.len() >= 3, "face requires at least three vertices");
    Ok(face)
}

fn face_value<T: Copy>(field: &str, values: &[T], name: &str) -> Result<T> {
    let index: isize = field
        .parse()
        .with_context(|| format!("invalid face {name} index {field:?}"))?;
    let position = match index.cmp(&0) {
        std::cmp::Ordering::Greater => Some(index as usize - 1),
        std::cmp::Ordering::Less => values.len().checked_add_signed(index),
        std::cmp::Ordering::Equal => None,
    };
    position
        .and_then(|position| values.get(position))
        .copied()
        .with_context(|| format!("face {name} index {index} is out of bounds"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_face_preserves_both_windings() {
        let vertices = "v 0 0 0\nv 0.5 0.5 0\nv 2 0 0\nv 0 2 0\nvt 0.25 0.75\n";
        for (face, expected_sign) in [
            ("f 1 2/1 3//1 4/1/1 # concave quad", 1.0),
            ("f -1 -2 -3 -4", -1.0),
        ] {
            let albedo = [0.2, 0.4, 0.6];
            let (mesh, _) = parse(&format!("{vertices}{face}"), albedo, None, 0).unwrap();
            let triangles = mesh.triangles;
            assert_eq!(triangles.len(), 2);
            let mut area = 0.0;
            for triangle in triangles {
                let [[ax, ay, _], [bx, by, _], [cx, cy, _]] = triangle.vertices;
                let signed_area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
                assert_eq!(signed_area.signum(), expected_sign);
                assert_eq!(triangle.albedo, albedo);
                area += signed_area;
            }
            assert_eq!(area, 3.0 * expected_sign);
        }
    }

    #[test]
    fn rejects_invalid_indices_with_line_context() {
        for index in ["0", "4", "-4"] {
            let source = format!("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 {index}");
            let error = parse(&source, [1.0; 3], None, 0).unwrap_err();
            assert_eq!(
                format!("{error:#}"),
                format!("OBJ line 4: face vertex index {index} is out of bounds")
            );
        }
    }

    #[test]
    fn triangulates_a_polygon_in_the_yz_plane() {
        let source = "v 3 0 0\nv 3 2 0\nv 3 2 2\nv 3 0 2\nf 1 2 3 4";
        let (mesh, _) = parse(source, [1.0; 3], None, 0).unwrap();
        let triangles = mesh.triangles;
        assert_eq!(triangles.len(), 2);
        for triangle in triangles {
            let [[ax, ay, az], [bx, by, bz], [cx, cy, cz]] = triangle.vertices;
            assert_eq!([ax, bx, cx], [3.0; 3]);
            assert_eq!((by - ay) * (cz - az) - (bz - az) * (cy - ay), 4.0);
        }
    }

    #[test]
    fn reports_the_line_of_a_zero_area_polygon() {
        let source = "v 0 0 0\nv 1 0 0\nv 2 0 0\nv 3 0 0\nf 1 2 3 4";
        let error = parse(source, [1.0; 3], None, 0).unwrap_err();
        assert_eq!(format!("{error:#}"), "OBJ line 5: polygon has no area");
    }
}
