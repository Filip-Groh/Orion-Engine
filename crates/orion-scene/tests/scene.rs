use std::path::Path;

use orion_scene::{Camera, Mesh, Scene, SceneSnapshot, SineBob, Sphere};

fn camera() -> Camera {
    Camera {
        eye: [0.0, 1.0, 2.0],
        target: [0.0; 3],
        vertical_fov_degrees: 38.0,
    }
}

#[test]
fn active_camera_reads_and_writes_follow_the_selected_entity() {
    let mut scene = Scene::new(camera());
    let original = scene.active_camera;
    scene.active_camera = scene.world.spawn((camera(),));
    scene.camera_mut().unwrap().eye = [2.0, 1.0, 0.0];
    assert_eq!(scene.camera().unwrap().eye, [2.0, 1.0, 0.0]);
    scene.active_camera = original;
    assert_eq!(scene.camera().unwrap().eye, camera().eye);

    scene.world.despawn(original).unwrap();
    assert!(scene.camera().is_err());
    assert!(scene.camera_mut().is_err());
}

#[test]
fn obj_import_spawns_one_mesh_with_the_requested_albedo() {
    let mut scene = Scene::new(camera());
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/monkey.obj");
    let albedo = [0.1, 0.2, 0.3];
    let entity = scene.import_obj(path, albedo).unwrap();
    let mesh = scene.world.get::<&Mesh>(entity).unwrap();
    assert_eq!(mesh.triangles.len(), 968);
    assert!(
        mesh.triangles
            .iter()
            .all(|triangle| triangle.albedo == albedo)
    );
    assert_eq!(scene.world.query::<&Mesh>().iter().len(), 1);
}

#[test]
fn demo_keeps_the_triangle_and_suzanne_in_separate_meshes() {
    let scene = Scene::demo();
    let mut triangle_counts: Vec<_> = scene
        .world
        .query::<&Mesh>()
        .iter()
        .map(|mesh| mesh.triangles.len())
        .collect();
    triangle_counts.sort_unstable();
    assert_eq!(triangle_counts, [1, 968]);
}

#[test]
fn scene_snapshot_restores_the_selected_camera_and_its_other_components() {
    let mut scene = Scene::new(camera());
    let selected = Camera {
        eye: [0.0, 3.0, 2.0],
        ..camera()
    };
    let sphere = Sphere {
        center: [1.0, 2.0, 3.0],
        radius: 1.0,
        albedo: [0.2; 3],
    };
    let bob = SineBob {
        rest_y: 2.0,
        amplitude: 0.5,
    };
    scene.active_camera = scene.world.spawn((selected, sphere, bob));

    let mut restored = SceneSnapshot::capture(&scene).unwrap().restore().unwrap();
    assert_eq!(restored.world.len(), 2);
    assert_eq!(restored.camera().unwrap().eye, selected.eye);
    let (restored_sphere, restored_bob) = restored
        .world
        .query_one_mut::<(&Sphere, &SineBob)>(restored.active_camera)
        .unwrap();
    assert_eq!(restored_sphere.center, sphere.center);
    assert_eq!(restored_bob.amplitude, bob.amplitude);
}

#[test]
fn scene_snapshot_rejects_unsupported_components() {
    struct Unsupported;
    let mut scene = Scene::new(camera());
    let entity = scene.world.spawn((Unsupported,));
    let error = SceneSnapshot::capture(&scene).err().unwrap();
    assert_eq!(
        error.to_string(),
        format!("entity {entity:?} has a component that checkpoints do not support")
    );
}
