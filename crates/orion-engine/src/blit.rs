use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_resource::{BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, CachedRenderPipelineId, ColorTargetState, ColorWrites, FilterMode, FragmentState, MultisampleState, PipelineCache, PrimitiveState, RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, TextureFormat, TextureSampleType, TextureViewDimension, VertexState};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedWindows, ViewTarget};

#[derive(Resource)]
pub struct CustomBlitPipeline {
    pub bind_group_layout: BindGroupLayout,
    pub sampler: Sampler,
    pub pipeline_id: CachedRenderPipelineId,
}

pub fn init_custom_blit_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
    extracted_windows: Res<ExtractedWindows>,
) {
    let target_format = extracted_windows
        .primary
        .and_then(|id| extracted_windows.get(&id))
        .and_then(|w| w.swap_chain_texture_format)
        .unwrap_or(TextureFormat::Bgra8UnormSrgb);

    let bind_group_layout_descriptor = BindGroupLayoutDescriptor::new(
        "custom_blit_bind_group_layout",
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
        ],
    );

    let bind_group_layout = render_device.create_bind_group_layout(
        "custom_blit_bind_group_layout",
        &*bind_group_layout_descriptor.entries,
    );

    let sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("custom_blit_sampler"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..Default::default()
    });

    let shader = asset_server.load("shaders/blit.wgsl");

    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("custom_blit_pipeline".into()),
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

    commands.insert_resource(CustomBlitPipeline {
        bind_group_layout,
        sampler,
        pipeline_id,
    });
}

pub fn custom_blit_pass(
    view: ViewQuery<(&ExtractedCamera, &ViewTarget)>,
    pipeline: Res<CustomBlitPipeline>,
    pipeline_cache: Res<PipelineCache>,
    render_device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline_id) else {
        return;
    };

    let (camera, target) = view.into_inner();

    let source_view = target.main_texture_view();

    let Some(out_attachment) = target.out_texture_color_attachment(None) else {
        return;
    };

    let bind_group = render_device.create_bind_group(
        Some("custom_blit_bind_group"),
        &pipeline.bind_group_layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(source_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(&pipeline.sampler),
            },
        ],
    );

    let mut render_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("custom_blit_pass"),
        color_attachments: &[Some(out_attachment)],
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