use anyhow::{Context, Result, bail, ensure};

use crate::Triangle;

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

fn project(point: [f32; 3], axis: usize) -> [f64; 2] {
    [point[(axis + 1) % 3].into(), point[(axis + 2) % 3].into()]
}

fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn triangulate(face: &[[f32; 3]], albedo: [f32; 3], triangles: &mut Vec<Triangle>) -> Result<()> {
    if let &[a, b, c] = face {
        triangles.push(Triangle {
            vertices: [a, b, c],
            albedo,
        });
        return Ok(());
    }

    // Drop the axis with the largest normal component, then clip ears in 2D.
    let projected_area = [0, 1, 2].map(|axis| {
        let first = project(face[0], axis);
        face[1..]
            .windows(2)
            .map(|pair| orient(first, project(pair[0], axis), project(pair[1], axis)))
            .sum::<f64>()
    });
    let axis = (0..3)
        .max_by(|&a, &b| projected_area[a].abs().total_cmp(&projected_area[b].abs()))
        .expect("three projection axes");
    ensure!(projected_area[axis] != 0.0, "polygon has no area");
    let winding = projected_area[axis].signum();
    let points: Vec<_> = face.iter().map(|&point| project(point, axis)).collect();
    let mut remaining: Vec<_> = (0..face.len()).collect();
    while remaining.len() > 3 {
        let count = remaining.len();
        let mut ear = None;
        for i in 0..count {
            let corners = [
                remaining[(i + count - 1) % count],
                remaining[i],
                remaining[(i + 1) % count],
            ];
            let [a, b, c] = corners.map(|index| points[index]);
            if winding * orient(a, b, c) <= 0.0 {
                continue;
            }
            let contains_vertex = remaining.iter().any(|&index| {
                let point = points[index];
                !corners.contains(&index)
                    && winding * orient(a, b, point) >= 0.0
                    && winding * orient(b, c, point) >= 0.0
                    && winding * orient(c, a, point) >= 0.0
            });
            if !contains_vertex {
                ear = Some((i, corners));
                break;
            }
        }
        let Some((i, corners)) = ear else {
            bail!("could not triangulate polygon");
        };
        triangles.push(Triangle {
            vertices: corners.map(|index| face[index]),
            albedo,
        });
        remaining.remove(i);
    }
    triangles.push(Triangle {
        vertices: [face[remaining[0]], face[remaining[1]], face[remaining[2]]],
        albedo,
    });
    Ok(())
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
                let [a, b, c] = triangle.vertices.map(|point| project(point, 2));
                let signed_area = orient(a, b, c);
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
    fn demo_keeps_the_triangle_and_suzanne_in_separate_meshes() {
        let scene = crate::Scene::demo();
        let mut triangle_counts: Vec<_> = scene
            .world
            .query::<&crate::Mesh>()
            .iter()
            .map(|mesh| mesh.triangles.len())
            .collect();
        triangle_counts.sort_unstable();
        assert_eq!(triangle_counts, [1, 32 + 468 * 2]);
    }
}
