mod minimal_camera_driver_plugin;
mod custom_pipeline;
mod camera;
mod tint;
mod blit;

use crate::camera::CustomCamera;
use crate::custom_pipeline::{CustomPipeline, CustomPipelinePlugin};
use crate::minimal_camera_driver_plugin::MinimalCameraDriverPlugin;
use crate::tint::{CustomTintPlugin, ScreenTint};
use bevy::a11y::AccessibilityPlugin;
use bevy::app::PluginGroupBuilder;
use bevy::asset::AssetPlugin;
use bevy::camera::CameraPlugin;
use bevy::diagnostic::FrameCountPlugin;
use bevy::input::InputPlugin;
use bevy::log::LogPlugin;
use bevy::mesh::MeshPlugin;
use bevy::render::camera::CameraRenderGraph;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::settings::WgpuSettings;
use bevy::render::RenderPlugin;
use bevy::time::TimePlugin;
use bevy::window::WindowPlugin;
use bevy::winit::WinitPlugin;
use bevy::{
    prelude::*,
    render::render_resource::*,
};

pub struct MyAppPlugins;

impl PluginGroup for MyAppPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(TaskPoolPlugin::default())
            .add(FrameCountPlugin::default())
            .add(TimePlugin::default())
            .add(LogPlugin::default())
            .add(InputPlugin)
            .add(WindowPlugin::default())
            .add(AccessibilityPlugin)
            .add(WinitPlugin::default())
            .add(AssetPlugin {
                file_path: "../../assets".to_string(),
                ..default()
            })
            .add(MeshPlugin)
            .add(RenderPlugin {
                render_creation: WgpuSettings {
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    ..default()
                }
                .into(),
                ..default()
            })
            .add(ImagePlugin::default())
            .add(PipelinedRenderingPlugin)
            .add(CameraPlugin)
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins((MyAppPlugins, MinimalCameraDriverPlugin, CustomPipelinePlugin, CustomTintPlugin))
        .add_systems(Startup, setup_scene);

    app.run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn((CustomCamera, ScreenTint(LinearRgba::new(0.5, 0.0, 0.0, 0.5))));
}