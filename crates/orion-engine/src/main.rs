mod hello_triangle;

use bevy::prelude::*;
use crate::hello_triangle::HelloTriangle;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(HelloTriangle)
        .run();
}