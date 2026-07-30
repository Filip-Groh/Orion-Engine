use crate::CustomPipeline;
use crate::CameraRenderGraph;
use bevy::prelude::*;

#[derive(Component, Default, Debug, Clone, Reflect)]
#[require(
    Camera,
    Projection,
    CameraRenderGraph(*CameraRenderGraph::new(CustomPipeline))
)]
pub struct CustomCamera;