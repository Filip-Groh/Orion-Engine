use crate::custom_pipeline::{CustomPipeline, CustomPipelineSystems};
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use bevy::render::render_resource::{BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBindingType, BufferInitDescriptor, BufferUsages, CachedRenderPipelineId, ColorTargetState, ColorWrites, FilterMode, FragmentState, LoadOp, MultisampleState, Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType, StoreOp, TextureFormat, TextureSampleType, TextureViewDimension, VertexState};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::ViewTarget;
use bevy::render::{Render, RenderApp, RenderSystems};

#[derive(Component, Clone, Copy, ExtractComponent)]
#[extract_component_filter(With<Camera>)]
pub struct ScreenTint(pub LinearRgba);

#[derive(Resource)]
pub struct ScreenTintPipeline {
    pub bind_group_layout: BindGroupLayout,
    pub sampler: Sampler,
    pub pipeline_id: CachedRenderPipelineId,
}

pub fn init_screen_tint_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>
) {
    let target_format = TextureFormat::Rgba8UnormSrgb;

    let bind_group_layout_descriptor = BindGroupLayoutDescriptor::new(
        "screen_tint_bind_group_layout",
        &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(Vec4::min_size()),
                },
                count: None,
            },
        ]
    );

    let bind_group_layout = render_device.create_bind_group_layout(
        "screen_tint_bind_group",
        &*bind_group_layout_descriptor.entries
    );

    let sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("screen_tint_sampler"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..Default::default()
    });

    let shader = asset_server.load("shaders/shader.wgsl");

    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("screen_tint_pipeline".into()),
        layout: vec![bind_group_layout_descriptor],
        immediate_size: 0,
        vertex: VertexState {
            shader: shader.clone(),
            entry_point: Some("vertex".into()),
            buffers: vec![],
            shader_defs: vec![],
        },
        fragment: Some(FragmentState {
            shader,
            entry_point: Some("fragment".into()),
            targets: vec![Some(ColorTargetState {
                format: target_format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            shader_defs: vec![],
        }),
        primitive: PrimitiveState::default(),
        depth_stencil: None,
        multisample: MultisampleState::default(),
        zero_initialize_workgroup_memory: false,
    });

    commands.insert_resource(ScreenTintPipeline {
        bind_group_layout,
        sampler,
        pipeline_id,
    });
}

#[derive(Component)]
pub struct ScreenTintBuffer(pub Buffer);

pub fn prepare_tint_buffers(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &ScreenTint)>,
) {
    for (entity, tint) in &views {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("screen_tint_uniform_buffer"),
            contents: bytemuck::bytes_of(&tint.0.to_f32_array()),
            usage: BufferUsages::UNIFORM,
        });

        commands.entity(entity).insert(ScreenTintBuffer(buffer));
    }
}

pub fn screen_tint_pass(
    view: ViewQuery<(&ExtractedCamera, &ViewTarget, &ScreenTintBuffer)>,
    pipeline: Res<ScreenTintPipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
        return;
    };

    let (camera, target, tint_buffer) = view.into_inner();

    let post_process = target.post_process_write();

    let bind_group = render_device.create_bind_group(
        Some("screen_tint_bind_group"),
        &pipeline.bind_group_layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(post_process.source),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(&pipeline.sampler),
            },
            BindGroupEntry {
                binding: 2,
                resource: tint_buffer.0.as_entire_binding(),
            },
        ],
    );

    let color_attachments = [Some(RenderPassColorAttachment {
        view: post_process.destination,
        depth_slice: None,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Clear(LinearRgba::BLACK.into()),
            store: StoreOp::Store,
        },
    })];

    let mut render_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("screen_tint_pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    if let Some(viewport) = camera.viewport.as_ref() {
        render_pass.set_camera_viewport(viewport);
    }

    render_pass.set_render_pipeline(render_pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

pub struct CustomTintPlugin;

impl Plugin for CustomTintPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<ScreenTint>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(Render, init_screen_tint_pipeline.run_if(not(resource_exists::<ScreenTintPipeline>)).in_set(RenderSystems::PrepareResources))
            .add_systems(Render, prepare_tint_buffers.in_set(RenderSystems::PrepareResources))
            .add_systems(CustomPipeline, screen_tint_pass.in_set(CustomPipelineSystems::PostProcess));
    }
}