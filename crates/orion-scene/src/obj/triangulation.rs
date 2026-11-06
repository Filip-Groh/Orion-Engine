use anyhow::{Context, Result, ensure};

use crate::Triangle;

type Point = [f64; 2];

pub(super) fn triangulate(
    face: &[[f32; 3]],
    albedo: [f32; 3],
    triangles: &mut Vec<Triangle>,
) -> Result<()> {
    let mut emit = |corners: [usize; 3]| {
        triangles.push(Triangle {
            vertices: corners.map(|index| face[index]),
            albedo,
        });
    };
    if face.len() == 3 {
        emit([0, 1, 2]);
        return Ok(());
    }

    let (points, winding) = project_polygon(face)?;
    let mut remaining: Vec<_> = (0..face.len()).collect();
    while remaining.len() > 3 {
        let (index, corners) =
            find_ear(&points, winding, &remaining).context("could not triangulate polygon")?;
        emit(corners);
        remaining.remove(index);
    }
    emit([remaining[0], remaining[1], remaining[2]]);
    Ok(())
}

fn project_polygon(face: &[[f32; 3]]) -> Result<(Vec<Point>, f64)> {
    // Drop the axis with the largest normal component to clip ears in 2D.
    let areas = [0, 1, 2].map(|axis| {
        let first = project(face[0], axis);
        face[1..]
            .windows(2)
            .map(|pair| orient(first, project(pair[0], axis), project(pair[1], axis)))
            .sum::<f64>()
    });
    let axis = (0..3)
        .max_by(|&a, &b| areas[a].abs().total_cmp(&areas[b].abs()))
        .expect("three projection axes");
    ensure!(areas[axis] != 0.0, "polygon has no area");
    let points = face.iter().map(|&point| project(point, axis)).collect();
    Ok((points, areas[axis].signum()))
}

fn find_ear(points: &[Point], winding: f64, remaining: &[usize]) -> Option<(usize, [usize; 3])> {
    let count = remaining.len();
    (0..count).find_map(|index| {
        let corners = [
            remaining[(index + count - 1) % count],
            remaining[index],
            remaining[(index + 1) % count],
        ];
        is_ear(points, winding, remaining, corners).then_some((index, corners))
    })
}

fn is_ear(points: &[Point], winding: f64, remaining: &[usize], corners: [usize; 3]) -> bool {
    let [a, b, c] = corners.map(|index| points[index]);
    winding * orient(a, b, c) > 0.0
        && !remaining.iter().any(|&index| {
            let point = points[index];
            !corners.contains(&index)
                && winding * orient(a, b, point) >= 0.0
                && winding * orient(b, c, point) >= 0.0
                && winding * orient(c, a, point) >= 0.0
        })
}

fn project(point: [f32; 3], axis: usize) -> Point {
    [point[(axis + 1) % 3].into(), point[(axis + 2) % 3].into()]
}

fn orient(a: Point, b: Point, c: Point) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
