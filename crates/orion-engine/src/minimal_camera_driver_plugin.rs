use std::fmt;
use std::fmt::{Display, Formatter};
use bevy::app::{App, Plugin};
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::EntityHashSet;
use bevy::ecs::schedule::InternedScheduleLabel;
use bevy::render::renderer::{CurrentView, PendingCommandBuffers, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue};
use bevy::prelude::*;
use bevy::render::camera::{ExtractedCamera, SortedCameras};
use bevy::render::render_resource::{CommandEncoderDescriptor, LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor, StoreOp};
use bevy::render::RenderApp;
use bevy::render::view::ExtractedWindows;

pub struct MinimalCameraDriverPlugin;

impl Plugin for MinimalCameraDriverPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.add_systems(
            RenderGraph,
            (
                camera_driver.in_set(RenderGraphSystems::Render),
                (submit_pending_command_buffers, handle_uncovered_swap_chains)
                    .chain()
                    .in_set(RenderGraphSystems::Submit),
            ),
        );
    }
}

#[derive(Resource)]
struct CameraWindows(EntityHashSet);

#[derive(Clone, Copy, Component, Debug, Reflect)]
#[reflect(Clone, Component)]
#[reflect(from_reflect = false)]
pub struct RootNonCameraView(#[reflect(ignore)] pub InternedScheduleLabel);

fn camera_driver(world: &mut World) {
    let root_views: Vec<_> = {
        let mut auxiliary_views = world.query_filtered::<Entity, With<RootNonCameraView>>();
        let sorted = world.resource::<SortedCameras>();
        auxiliary_views
            .iter(world)
            .map(RootView::Auxiliary)
            .chain(sorted.0.iter().map(|c| RootView::Camera {
                entity: c.entity,
                order: c.order,
            }))
            .collect()
    };

    let mut camera_windows = EntityHashSet::default();

    for root_view in root_views {
        let mut run_schedule = true;
        let (schedule, view_entity);

        match root_view {
            RootView::Camera {
                entity: camera_entity,
                ..
            } => {
                let Some(camera) = world.get::<ExtractedCamera>(camera_entity) else {
                    continue;
                };

                schedule = camera.schedule;
                let target = camera.target.clone();

                if let Some(NormalizedRenderTarget::Window(window_ref)) = &target {
                    let window_entity = window_ref.entity();
                    let windows = world.resource::<ExtractedWindows>();
                    if windows
                        .windows
                        .get(&window_entity)
                        .is_some_and(|w| w.physical_width > 0 && w.physical_height > 0)
                    {
                        camera_windows.insert(window_entity);
                    } else {
                        run_schedule = false;
                    }
                }

                view_entity = camera_entity;
            }

            RootView::Auxiliary(auxiliary_view_entity) => {
                let Some(root_view) = world.get::<RootNonCameraView>(auxiliary_view_entity) else {
                    continue;
                };

                view_entity = auxiliary_view_entity;
                schedule = root_view.0;
            }
        }

        if run_schedule {
            world.insert_resource(CurrentView(view_entity));

            world.run_schedule(schedule);
        }
    }
    world.remove_resource::<CurrentView>();

    world.insert_resource(CameraWindows(camera_windows));
}

enum RootView {
    Camera { entity: Entity, order: isize },
    Auxiliary(Entity),
}

impl Display for RootView {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            RootView::Camera { entity, order } => write!(f, "Camera {} ({:?})", order, entity),
            RootView::Auxiliary(entity) => write!(f, "Auxiliary View {:?}", entity),
        }
    }
}

fn submit_pending_command_buffers(world: &mut World) {
    let mut pending = world.resource_mut::<PendingCommandBuffers>();
    let buffers = pending.take();

    if !buffers.is_empty() {
        let queue = world.resource::<RenderQueue>();
        queue.submit(buffers);
    }
}

fn handle_uncovered_swap_chains(world: &mut World) {
    let windows_to_clear: Vec<_> = {
        let clear_color = world.resource::<ClearColor>().0.to_linear();
        let Some(camera_windows) = world.remove_resource::<CameraWindows>() else {
            return;
        };
        let windows = world.resource::<ExtractedWindows>();
        windows
            .iter()
            .filter_map(|(window_entity, window)| {
                if camera_windows.0.contains(window_entity) {
                    return None;
                }
                let swap_chain_texture = window.swap_chain_texture_view.as_ref()?;
                Some((swap_chain_texture.clone(), clear_color))
            })
            .collect()
    };

    if windows_to_clear.is_empty() {
        return;
    }

    let render_device = world.resource::<RenderDevice>();
    let render_queue = world.resource::<RenderQueue>();

    let mut encoder = render_device.create_command_encoder(&CommandEncoderDescriptor::default());

    for (swap_chain_texture, clear_color) in &windows_to_clear {
        let pass_descriptor = RenderPassDescriptor {
            label: Some("no_camera_clear_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: swap_chain_texture,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear((*clear_color).into()),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        };

        encoder.begin_render_pass(&pass_descriptor);
    }

    render_queue.submit([encoder.finish()]);
}