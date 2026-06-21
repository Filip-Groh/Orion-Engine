mod hello_triangle;
mod hello_cube;
mod free_cam;
mod scene;
mod fps_overlay;
mod wireframe_view;

use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{WgpuFeatures, WgpuSettings};
use bevy_dev_tools::fps_overlay::FpsOverlayPlugin;
use orion_mesh::OrionMeshPlugin;
use orion_sdf::primitives::SDFSphere;
use orion_sdf::SDF;
use orion_voxels::{OrionVoxelGrid, OrionVoxelPlugin, VoxelConfig};
use crate::fps_overlay::FPSOverlayPlugin;
use crate::free_cam::CameraPlugin;
use crate::hello_cube::HelloCube;
use crate::hello_triangle::HelloTriangle;
use crate::scene::ScenePlugin;
use crate::wireframe_view::WireframeViewPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(RenderPlugin {
            render_creation: WgpuSettings {
                features: WgpuFeatures::POLYGON_MODE_LINE,
                ..default()
            }
                .into(),
            ..default()
        }))
        .add_plugins(CameraPlugin)
        .add_plugins(ScenePlugin)
        .add_plugins(FPSOverlayPlugin)
        .add_plugins(WireframeViewPlugin)
        .add_plugins(OrionVoxelPlugin)
        .add_plugins(OrionMeshPlugin)
        .add_systems(Startup, setup_sdf)
        .run();
}

fn setup_sdf(
    mut commands: Commands,
    config: Res<VoxelConfig>,
) {
    let sphere = SDFSphere::new(Vec3::new(5.0, 5.0, 5.0), 5.0);
    let sphere2 = SDFSphere::new(Vec3::new(1.5, 1.5, 1.5), 2.0);

    let sdf = sphere.smooth_union(sphere2, 2.0);

    for x in -2..=2 {
        for y in -2..=2 {
            for z in -2..=2 {
                let chunk_coord = IVec3::new(x, y, z);

                let voxel_grid = OrionVoxelGrid::new(
                    config.array_size,
                    config.grid_size,
                    chunk_coord,
                    &sdf,
                );

                commands.spawn(voxel_grid);
            }
        }
    }
}