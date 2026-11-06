use bytemuck::pod_read_unaligned;
use orion_scene::{DEFAULT_ROUGHNESS, Scene, Texture, TriangleUvs, WrapMode};

use super::*;
use crate::{TransformSnapshot, Wireframes};

const EXTENT: vk::Extent2D = vk::Extent2D {
    width: 960,
    height: 540,
};

fn empty_scene() -> Scene {
    Scene::new(Camera {
        eye: [0.0, 1.0, 2.0],
        target: [0.0, 0.0, 0.0],
        vertical_fov_degrees: 38.0,
    })
}

#[test]
fn packs_each_wireframe_flag_without_changing_the_scene_format() {
    let scene = empty_scene();
    for (meshes, bvh, flags) in [
        (false, false, 0),
        (true, false, 1),
        (false, true, 2),
        (true, true, 3),
    ] {
        let frame = SceneFrame::new(&scene).with_wireframes(Wireframes { meshes, bvh });
        let bytes = pack_scene(frame, EXTENT, true, 1024).unwrap();
        let header: SceneHeader = pod_read_unaligned(&bytes[..size_of::<SceneHeader>()]);
        assert_eq!(header.extent_format, [960, 540, 1, flags]);
    }
}

#[test]
fn packs_the_active_camera_and_current_world_geometry() {
    let mut scene = empty_scene();
    scene.active_camera = scene.world.spawn((scene.camera().unwrap(),));
    scene.camera_mut().unwrap().eye = [1.0, 2.0, 3.0];
    let sphere = scene.world.spawn((Sphere {
        center: [0.0; 3],
        radius: 1.0,
        albedo: [1.0; 3],
        metallic: 0.0,
        roughness: DEFAULT_ROUGHNESS,
    },));
    let triangle = Triangle {
        vertices: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        albedo: [1.0; 3],
        metallic: 0.0,
        roughness: DEFAULT_ROUGHNESS,
        uv_index: None,
    };
    let mesh = scene.world.spawn((Mesh {
        triangles: vec![triangle],
        ..Mesh::default()
    },));
    let other_mesh = scene.world.spawn((Mesh {
        triangles: vec![triangle; 3],
        ..Mesh::default()
    },));
    scene.world.spawn((Mesh::default(),));

    let header_size = size_of::<SceneHeader>();
    let bytes = pack_scene(SceneFrame::new(&scene), EXTENT, false, 1024).unwrap();
    let header: SceneHeader = pod_read_unaligned(&bytes[..header_size]);
    assert_eq!(header.eye_and_fov_scale[..3], [1.0, 2.0, 3.0]);
    assert_eq!(header.geometry_counts, [1, 4, 3, 0]);
    assert_eq!(
        SceneStats::from_scene(&scene),
        SceneStats {
            triangles: 4,
            sdfs: 2,
            bvh_nodes: 3,
        }
    );

    scene.world.despawn(sphere).unwrap();
    scene.world.remove_one::<Mesh>(mesh).unwrap();
    scene.world.despawn(other_mesh).unwrap();
    let bytes = pack_scene(SceneFrame::new(&scene), EXTENT, false, 1024).unwrap();
    let header: SceneHeader = pod_read_unaligned(&bytes[..header_size]);
    assert_eq!(header.geometry_counts, [0; 4]);
    assert_eq!(bytes.len(), header_size + size_of::<[f32; 4]>());
    assert_eq!(
        SceneStats::from_scene(&scene),
        SceneStats {
            triangles: 0,
            sdfs: 1,
            bvh_nodes: 0,
        }
    );
}

#[test]
fn packs_geometry_in_shader_order_and_respects_the_exact_buffer_limit() {
    let mut scene = empty_scene();
    scene.textures.push(Texture {
        width: 2,
        height: 1,
        texels: vec![[255, 0, 0, 255], [0, 255, 0, 255]],
        wrap_u: WrapMode::Clamp,
        wrap_v: WrapMode::Mirror,
        linear_filter: true,
        scale: [1.0; 2],
        offset: [0.0; 2],
    });
    scene.world.spawn((Sphere {
        center: [1.0, 2.0, 3.0],
        radius: 0.5,
        albedo: [1.0, 0.5, 0.25],
        metallic: 0.5,
        roughness: 1.0,
    },));
    scene.world.spawn((Mesh {
        triangles: vec![Triangle {
            vertices: [[-1.0, 2.0, 3.0], [4.0, 2.0, 3.0], [-1.0, 5.0, 3.0]],
            albedo: [0.25, 0.5, 1.0],
            metallic: 1.0,
            roughness: 0.25,
            uv_index: Some(0),
        }],
        uvs: vec![TriangleUvs {
            primary: [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            metallic_roughness: None,
            maps: MaterialMaps {
                base_color: Some(0),
                ..MaterialMaps::default()
            },
        }],
    },));

    let bytes = pack_scene(SceneFrame::new(&scene), EXTENT, true, 304).unwrap();
    assert_eq!(bytes.len(), 304);
    let header: SceneHeader = pod_read_unaligned(&bytes[..64]);
    assert_eq!(header.extent_format, [960, 540, 1, 0]);
    assert_eq!(header.geometry_counts, [1, 1, 1, 0]);
    let sphere: GpuSphere = pod_read_unaligned(&bytes[64..96]);
    assert_eq!(sphere.center_radius, [1.0, 2.0, 3.0, 0.5]);
    assert_eq!(sphere.material[..3], [1.0_f32, 0.5, 0.25].map(f32::to_bits));
    assert_eq!(sphere.material[3], 0xffff_8000);
    let triangle: GpuTriangle = pod_read_unaligned(&bytes[96..160]);
    assert_eq!(
        triangle.vertices.map(|[x, y, z, _]| [x, y, z]),
        [[-1.0_f32, 2.0, 3.0], [4.0, 2.0, 3.0], [-1.0, 5.0, 3.0],]
            .map(|vertex| vertex.map(f32::to_bits))
    );
    assert_eq!(triangle.vertices[0][3], 1);
    assert_eq!(
        triangle.material[..3],
        [0.25_f32, 0.5, 1.0].map(f32::to_bits)
    );
    assert_eq!(triangle.material[3], 0x4000_ffff);
    let bounds: [u32; 8] = pod_read_unaligned(&bytes[160..192]);
    assert_eq!(
        bounds,
        [
            (-1.0_f32).to_bits(),
            2.0_f32.to_bits(),
            3.0_f32.to_bits(),
            0,
            4.0_f32.to_bits(),
            5.0_f32.to_bits(),
            3.0_f32.to_bits(),
            1,
        ]
    );
    let uv: [f32; 12] = pod_read_unaligned(&bytes[192..240]);
    assert_eq!(
        uv,
        [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]
    );
    let maps: [u32; 4] = pod_read_unaligned(&bytes[240..256]);
    assert_eq!(maps, [12, 0, 0, 0]);
    let sampler: [u32; 4] = pod_read_unaligned(&bytes[256..272]);
    assert_eq!(sampler, [2, 1, 56, 25]);
    assert_eq!(&bytes[288..296], &[255, 0, 0, 255, 0, 255, 0, 255]);
    let error = pack_scene(SceneFrame::new(&scene), EXTENT, true, 303).unwrap_err();
    assert_eq!(
        error.to_string(),
        "scene exceeds the device's storage buffer limit"
    );
}

#[test]
fn rejects_a_vertical_camera_and_non_finite_triangle_vertices() {
    let mut scene = empty_scene();
    scene.camera_mut().unwrap().target = [0.0, 0.0, 2.0];
    let error = pack_scene(SceneFrame::new(&scene), EXTENT, false, 1024).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("camera must look in a finite direction")
    );

    scene.camera_mut().unwrap().target = [0.0; 3];
    scene.world.spawn((Mesh {
        triangles: vec![Triangle {
            vertices: [[f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            albedo: [1.0; 3],
            metallic: 0.0,
            roughness: DEFAULT_ROUGHNESS,
            uv_index: None,
        }],
        ..Mesh::default()
    },));
    let error = pack_scene(SceneFrame::new(&scene), EXTENT, false, 1024).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("triangles require finite vertices")
    );
}

#[test]
fn interpolated_sphere_upload_blends_positions_without_mutating_the_scene() {
    let mut scene = empty_scene();
    let sphere = scene.world.spawn((Sphere {
        center: [1.0, 2.0, 3.0],
        radius: 1.0,
        albedo: [1.0; 3],
        metallic: 0.0,
        roughness: DEFAULT_ROUGHNESS,
    },));
    let mut snapshot = TransformSnapshot::default();
    snapshot.capture(&scene).unwrap();
    scene.world.get::<&mut Sphere>(sphere).unwrap().center[1] = 6.0;

    for (fraction, expected_y) in [(-1.0, 2.0), (0.0, 2.0), (0.25, 3.0), (1.0, 6.0), (2.0, 6.0)] {
        let frame = SceneFrame::interpolated(&scene, &snapshot, fraction);
        let bytes = pack_scene(frame, EXTENT, false, 1024).unwrap();
        let packed: GpuSphere = pod_read_unaligned(&bytes[size_of::<SceneHeader>()..]);
        assert_eq!(packed.center_radius, [1.0, expected_y, 3.0, 1.0]);
    }
    assert_eq!(
        scene.world.get::<&Sphere>(sphere).unwrap().center,
        [1.0, 6.0, 3.0]
    );
}

#[test]
fn new_entities_and_camera_switches_use_current_transforms() {
    let mut scene = empty_scene();
    let old = scene.world.spawn((Sphere {
        center: [1.0; 3],
        radius: 1.0,
        albedo: [1.0; 3],
        metallic: 0.0,
        roughness: DEFAULT_ROUGHNESS,
    },));
    let mut snapshot = TransformSnapshot::default();
    snapshot.capture(&scene).unwrap();
    scene.world.despawn(old).unwrap();
    scene.world.spawn((Sphere {
        center: [3.0; 3],
        radius: 1.0,
        albedo: [1.0; 3],
        metallic: 0.0,
        roughness: DEFAULT_ROUGHNESS,
    },));
    scene.active_camera = scene.world.spawn((Camera {
        eye: [1.0, 2.0, 3.0],
        ..scene.camera().unwrap()
    },));
    let frame = SceneFrame::interpolated(&scene, &snapshot, 0.0);
    let bytes = pack_scene(frame, EXTENT, false, 1024).unwrap();
    let baseline = pack_scene(SceneFrame::new(&scene), EXTENT, false, 1024).unwrap();
    assert_eq!(bytes, baseline);

    let empty = TransformSnapshot::default();
    let frame = SceneFrame::interpolated(&scene, &empty, 0.0);
    assert_eq!(pack_scene(frame, EXTENT, false, 1024).unwrap(), baseline);
}

#[test]
fn interpolating_opposite_camera_headings_keeps_the_camera_valid() {
    let mut scene = Scene::new(Camera {
        eye: [0.0; 3],
        target: [-1.0, 0.0, 0.0],
        vertical_fov_degrees: 38.0,
    });
    let mut snapshot = TransformSnapshot::default();
    snapshot.capture(&scene).unwrap();
    let camera = scene.camera_mut().unwrap();
    camera.eye = [2.0, 0.0, 0.0];
    camera.target = [3.0, 0.0, 0.0];

    let frame = SceneFrame::interpolated(&scene, &snapshot, 0.5);
    let bytes = pack_scene(frame, EXTENT, false, 1024).unwrap();
    let header: SceneHeader = pod_read_unaligned(&bytes[..size_of::<SceneHeader>()]);
    assert_eq!(header.eye_and_fov_scale[..3], [1.0, 0.0, 0.0]);
    assert!((header.look_at[0] - 1.0).abs() < 1e-6);
    assert!((header.look_at[2].abs() - 1.0).abs() < 1e-6);
}

#[test]
fn interpolation_falls_back_when_a_camera_target_rounds_onto_its_eye() {
    let mut scene = Scene::new(Camera {
        eye: [1e20, 0.0, 0.0],
        target: [1e20, 0.0, 1.0],
        vertical_fov_degrees: 38.0,
    });
    let mut snapshot = TransformSnapshot::default();
    snapshot.capture(&scene).unwrap();
    let camera = scene.camera_mut().unwrap();
    camera.eye = [0.0, 0.0, 1e20];
    camera.target = [1.0, 0.0, 1e20];

    let frame = SceneFrame::interpolated(&scene, &snapshot, 0.5);
    let bytes = pack_scene(frame, EXTENT, false, 1024).unwrap();
    let current = pack_scene(SceneFrame::new(&scene), EXTENT, false, 1024).unwrap();
    assert_eq!(bytes, current);
}
