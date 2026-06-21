mod hello_triangle;
mod hello_cube;
mod free_cam;
mod scene;
mod fps_overlay;
mod wireframe_view;

use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{WgpuFeatures, WgpuSettings};
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
        .run();
}