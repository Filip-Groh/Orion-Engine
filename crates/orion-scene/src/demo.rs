use crate::{Camera, DEFAULT_ROUGHNESS, Mesh, Scene, SineBob, Sphere, Triangle, glb, obj};

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
                metallic: 0.0,
                roughness: DEFAULT_ROUGHNESS,
            },),
            (Sphere {
                center: [0.92, 0.72, -0.3],
                radius: 0.72,
                albedo: [0.05, 0.42, 0.82],
                metallic: 0.0,
                roughness: 0.28,
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
                metallic: 0.7,
                roughness: 0.33,
            },
            bob,
        ));
        scene.world.spawn_batch([
            (Mesh {
                triangles: vec![Triangle {
                    // This triangle crosses the red sphere and extends past its silhouette.
                    vertices: [[-2.35, 0.30, 0.50], [0.25, 0.50, 0.50], [-0.95, 2.28, 0.50]],
                    albedo: [0.12, 0.76, 0.38],
                    metallic: 0.0,
                    roughness: DEFAULT_ROUGHNESS,
                    uv_index: None,
                }],
                ..Mesh::default()
            },),
            (suzanne(),),
        ]);
        let (mut lamp, textures) = glb::parse(
            include_bytes!("../assets/lamp.glb"),
            [0.65; 3],
            None,
            scene.textures.len(),
        )
        .expect("bundled lamp.glb is valid");
        scene.textures.extend(textures);
        for triangle in &mut lamp.triangles {
            triangle.vertices = triangle
                .vertices
                .map(|[x, y, z]| [2.2 * x - 1.0, 2.2 * y + 2.01, 2.2 * z + 1.85]);
        }
        // Each mesh is a BVH leaf. Keep the lamp's triangles and their UVs together.
        scene
            .world
            .spawn_batch(lamp.triangles.chunks(256).map(|triangles| {
                let mut mesh = Mesh {
                    triangles: triangles.to_vec(),
                    ..Mesh::default()
                };
                for triangle in &mut mesh.triangles {
                    if let Some(index) = triangle.uv_index {
                        triangle.uv_index = Some(mesh.uvs.len() as u32);
                        mesh.uvs.push(lamp.uvs[index as usize]);
                    }
                }
                (mesh,)
            }));
        scene
    }
}

fn suzanne() -> Mesh {
    let (mut mesh, _) = obj::parse(
        include_str!("../assets/monkey.obj"),
        [0.68, 0.36, 0.12],
        None,
        0,
    )
    .expect("bundled Suzanne OBJ is valid");
    for triangle in &mut mesh.triangles {
        triangle.vertices = triangle
            .vertices
            .map(|[x, y, z]| [0.75 * x + 1.85, 0.75 * y + 0.85, 0.75 * z - 1.25]);
    }
    mesh
}
