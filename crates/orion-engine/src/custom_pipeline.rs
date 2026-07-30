use crate::blit::{custom_blit_pass, init_custom_blit_pipeline, CustomBlitPipeline};
use bevy::ecs::schedule::{ScheduleBuildSettings, ScheduleLabel};
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_resource::{LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp};
use bevy::render::renderer::{RenderContext, ViewQuery};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderSystems};

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CustomPipeline;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CustomPipelineSystems {
    MainPass,
    PostProcess,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct CustomPipelineBlitPass;

impl CustomPipeline {
    pub fn base_schedule() -> Schedule {
        let mut schedule = Schedule::new(Self);

        schedule.set_build_settings(ScheduleBuildSettings {
            auto_insert_apply_deferred: false,
            ..Default::default()
        });

        schedule.configure_sets(
            (CustomPipelineSystems::MainPass, CustomPipelineSystems::PostProcess, CustomPipelineBlitPass).chain(),
        );

        schedule
    }
}

pub fn custom_main_pass(
    view: ViewQuery<(&ExtractedCamera, &ViewTarget)>,
    mut ctx: RenderContext,
) {
    let (camera, target) = view.into_inner();

    let color_attachment = RenderPassColorAttachment {
        view: target.main_texture_view(),
        resolve_target: None,
        depth_slice: None,
        ops: Operations {
            load: LoadOp::Clear(LinearRgba::GREEN.into()),
            store: StoreOp::Store,
        },
    };

    let mut render_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("custom_main_pass"),
        color_attachments: &[Some(color_attachment)],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    if let Some(viewport) = &camera.viewport {
        render_pass.set_camera_viewport(viewport);
    }
}

pub struct CustomPipelinePlugin;

impl Plugin for CustomPipelinePlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.add_schedule(CustomPipeline::base_schedule());

        render_app
            .add_systems(CustomPipeline, custom_main_pass.in_set(CustomPipelineSystems::MainPass))
            .add_systems(Render, init_custom_blit_pipeline.run_if(not(resource_exists::<CustomBlitPipeline>)).in_set(RenderSystems::PrepareResources))
            .add_systems(CustomPipeline, custom_blit_pass.in_set(CustomPipelineBlitPass))
        ;
    }
}