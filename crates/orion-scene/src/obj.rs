mod triangulation;

use anyhow::{Context, Result, ensure};

use crate::Triangle;
use triangulation::triangulate;

pub(crate) fn parse(source: &str, albedo: [f32; 3]) -> Result<Vec<Triangle>> {
    ensure!(
        albedo.iter().all(|color| (0.0..=1.0).contains(color)),
        "OBJ albedo must be in 0..=1"
    );
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for (line_index, line) in source.trim_start_matches('\u{feff}').lines().enumerate() {
        let content = line.split('#').next().unwrap_or_default();
        let mut fields = content.split_whitespace();
        let result = match fields.next() {
            Some("v") => parse_vertex(fields).map(|vertex| vertices.push(vertex)),
            Some("f") => parse_face(fields, &vertices)
                .and_then(|face| triangulate(&face, albedo, &mut triangles)),
            _ => continue,
        };
        result.with_context(|| format!("OBJ line {}", line_index + 1))?;
    }
    ensure!(!triangles.is_empty(), "OBJ contains no polygon faces");
    Ok(triangles)
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

fn parse_face(
    fields: std::str::SplitWhitespace<'_>,
    vertices: &[[f32; 3]],
) -> Result<Vec<[f32; 3]>> {
    let mut face = Vec::new();
    for corner in fields {
        // Only the position index is needed in v, v/vt, v//vn and v/vt/vn.
        let index: isize = corner
            .split('/')
            .next()
            .unwrap_or_default()
            .parse()
            .with_context(|| format!("invalid face vertex {corner:?}"))?;
        let position = match index.cmp(&0) {
            std::cmp::Ordering::Greater => Some(index as usize - 1),
            std::cmp::Ordering::Less => vertices.len().checked_add_signed(index),
            std::cmp::Ordering::Equal => None,
        };
        face.push(
            *position
                .and_then(|position| vertices.get(position))
                .with_context(|| format!("face vertex index {index} is out of bounds"))?,
        );
    }
    ensure!(face.len() >= 3, "face requires at least three vertices");
    Ok(face)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_face_preserves_both_windings() {
        let vertices = "v 0 0 0\nv 0.5 0.5 0\nv 2 0 0\nv 0 2 0\n";
        for (face, expected_sign) in [
            ("f 1 2/1 3//1 4/1/1 # concave quad", 1.0),
            ("f -1 -2 -3 -4", -1.0),
        ] {
            let albedo = [0.2, 0.4, 0.6];
            let triangles = parse(&format!("{vertices}{face}"), albedo).unwrap();
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
            let error = parse(&source, [1.0; 3]).unwrap_err();
            assert_eq!(
                format!("{error:#}"),
                format!("OBJ line 4: face vertex index {index} is out of bounds")
            );
        }
    }

    #[test]
    fn triangulates_a_polygon_in_the_yz_plane() {
        let source = "v 3 0 0\nv 3 2 0\nv 3 2 2\nv 3 0 2\nf 1 2 3 4";
        let triangles = parse(source, [1.0; 3]).unwrap();
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
        let error = parse(source, [1.0; 3]).unwrap_err();
        assert_eq!(format!("{error:#}"), "OBJ line 5: polygon has no area");
    }
}
