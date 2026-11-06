use crate::{Camera, Mesh, Scene, SineBob, Sphere, Triangle, obj};

impl Scene {
    pub fn demo() -> Self {
        let mut scene = Self::new(Camera {
            eye: [3.4, 2.5, 5.0],
            target: [0.0, 0.7, 0.0],
            vertical_fov_degrees: 38.0,
        });
        scene.world.spawn_batch([
            (Sphere {
                center: [-1.0, 0.95, 0.0],
                radius: 0.95,
                albedo: [0.90, 0.15, 0.06],
            },),
            (Sphere {
                center: [0.92, 0.72, -0.3],
                radius: 0.72,
                albedo: [0.05, 0.42, 0.82],
            },),
        ]);
        let bob = SineBob {
            rest_y: 0.84,
            amplitude: 0.50,
        };
        scene.world.spawn((
            Sphere {
                center: [0.48, bob.rest_y, 1.05],
                radius: 0.34,
                albedo: [0.94, 0.67, 0.12],
            },
            bob,
        ));
        scene.world.spawn_batch([
            (Mesh {
                triangles: vec![Triangle {
                    // This triangle crosses the red sphere and extends past its silhouette.
                    vertices: [[-2.35, 0.30, 0.50], [0.25, 0.50, 0.50], [-0.95, 2.28, 0.50]],
                    albedo: [0.12, 0.76, 0.38],
                }],
            },),
            (suzanne(),),
        ]);
        scene
    }
}

fn suzanne() -> Mesh {
    let mut triangles = obj::parse(include_str!("../assets/monkey.obj"), [0.68, 0.36, 0.12])
        .expect("bundled Suzanne OBJ is valid");
    for triangle in &mut triangles {
        triangle.vertices = triangle
            .vertices
            .map(|[x, y, z]| [0.75 * x + 1.85, 0.75 * y + 0.85, 0.75 * z - 1.25]);
    }
    Mesh { triangles }
}
