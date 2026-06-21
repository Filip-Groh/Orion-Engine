use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::prelude::*;

pub struct WireframeViewPlugin;

impl Plugin for WireframeViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            // You need to add this plugin to enable wireframe rendering
            WireframePlugin::default(),
        ))
            // Wireframes can be configured with this resource. This can be changed at runtime.
            .insert_resource(WireframeConfig {
                // The global wireframe config enables drawing of wireframes on every mesh,
                // except those with `NoWireframe`. Meshes with `Wireframe` will always have a wireframe,
                // regardless of the global configuration.
                global: true,
                // Controls the default color of all wireframes. Used as the default color for global wireframes.
                // Can be changed per mesh using the `WireframeColor` component.
                default_color: LinearRgba::WHITE.into(),
                ..default()
            });
    }
}