use anyhow::{Context, Result};
use ash::vk;
use egui_ash_renderer::{Options, RenderMode, Renderer, allocator::DefaultAllocator};

use crate::{device::ComputeDevice, overlay_target::OverlayTarget};

pub(crate) struct Overlay {
    pub format: vk::Format,
    context: egui::Context,
    renderer: Renderer<DefaultAllocator>,
    retired_textures: Vec<egui::TextureId>,
}

pub(crate) struct OverlayFrame {
    primitives: Vec<egui::ClippedPrimitive>,
    pixels_per_point: f32,
}

impl Overlay {
    pub fn new(device: &ComputeDevice, target: &OverlayTarget) -> Result<Self> {
        let options = Options {
            srgb_framebuffer: matches!(
                target.format,
                vk::Format::B8G8R8A8_SRGB | vk::Format::R8G8B8A8_SRGB
            ),
            ..Options::default()
        };
        let renderer = Renderer::with_default_allocator(
            &device.instance.handle,
            device.physical,
            device.handle.clone(),
            RenderMode::RenderPass(target.render_pass),
            options,
        )
        .context("initialize the Vulkan egui renderer")?;
        Ok(Self {
            format: target.format,
            context: egui::Context::default(),
            renderer,
            retired_textures: Vec::new(),
        })
    }

    pub fn prepare(
        &mut self,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        extent: vk::Extent2D,
        scale: f32,
        events: Vec<egui::Event>,
        draw_ui: impl FnMut(&mut egui::Ui),
    ) -> Result<OverlayFrame> {
        // The previous dispatch fence has signaled, so its textures can be freed.
        for id in self.retired_textures.drain(..) {
            self.renderer
                .free_texture(id)
                .context("free an egui texture")?;
        }

        let screen_size = egui::vec2(extent.width as f32 / scale, extent.height as f32 / scale);
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen_size)),
            events,
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(scale);
        let mut output = self.context.run_ui(input, draw_ui);
        let primitives = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        self.retired_textures
            .extend(output.textures_delta.free.drain());
        for (id, deltas) in std::mem::take(&mut output.textures_delta.set) {
            for delta in deltas {
                self.renderer
                    .set_texture(queue, command_pool, id, &delta)
                    .context("upload an egui texture")?;
            }
        }
        Ok(OverlayFrame {
            primitives,
            pixels_per_point: output.pixels_per_point,
        })
    }

    pub fn record(
        &mut self,
        command: vk::CommandBuffer,
        target: &OverlayTarget,
        index: usize,
        extent: vk::Extent2D,
        frame: OverlayFrame,
    ) -> Result<()> {
        let begin = vk::RenderPassBeginInfo::default()
            .render_pass(target.render_pass)
            .framebuffer(target.framebuffer(index))
            .render_area(vk::Rect2D::default().extent(extent));
        unsafe {
            target
                .device
                .cmd_begin_render_pass(command, &begin, vk::SubpassContents::INLINE);
        }
        let result =
            self.renderer
                .cmd_draw(command, extent, frame.pixels_per_point, &frame.primitives);
        unsafe { target.device.cmd_end_render_pass(command) };
        result.context("record the egui overlay")
    }
}
