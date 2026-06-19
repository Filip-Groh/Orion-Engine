mod hello_triangle;
mod hello_cube;
mod free_cam;
mod scene;

use bevy::prelude::*;
use crate::free_cam::CameraPlugin;
use crate::hello_cube::HelloCube;
use crate::hello_triangle::HelloTriangle;
use crate::scene::ScenePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CameraPlugin)
        .add_plugins(ScenePlugin)
        .run();
}