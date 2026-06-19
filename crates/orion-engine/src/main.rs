mod hello_triangle;
mod hello_cube;

use bevy::prelude::*;
use crate::hello_cube::HelloCube;
use crate::hello_triangle::HelloTriangle;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(HelloCube)
        .run();
}