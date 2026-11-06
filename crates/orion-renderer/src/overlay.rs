use anyhow::{Context, Result};
use ash::vk;
use egui_ash_renderer::{Options, RenderMode, Renderer, allocator::DefaultAllocator};

use crate::device::ComputeDevice;

pub(crate) struct OverlayTarget {
    device: ash::Device,
    pub format: vk::Format,
    pub render_pass: vk::RenderPass,
    images: Vec<OverlayImage>,
}

struct OverlayImage {
    view: vk::ImageView,
    framebuffer: vk::Framebuffer,
}

impl OverlayTarget {
    pub fn new(
        device: &ash::Device,
        format: vk::Format,
        extent: vk::Extent2D,
        images: impl Iterator<Item = vk::Image>,
    ) -> Result<Self> {
        let attachments = [vk::AttachmentDescription::default()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
        let color = [vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let subpasses = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color)];
        let dependencies = [vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::TRANSFER)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )];
        let info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);
        let render_pass = unsafe { device.create_render_pass(&info, None) }
            .context("create the egui render pass")?;
        let mut target = Self {
            device: device.clone(),
            format,
            render_pass,
            images: Vec::new(),
        };
        for image in images {
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(1),
                );
            let view = unsafe { device.create_image_view(&view_info, None) }
                .context("create a swapchain image view for egui")?;
            target.images.push(OverlayImage {
                view,
                framebuffer: vk::Framebuffer::null(),
            });
            let attachments = [view];
            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(extent.width)
                .height(extent.height)
                .layers(1);
            target.images.last_mut().unwrap().framebuffer =
                unsafe { device.create_framebuffer(&framebuffer_info, None) }
                    .context("create a swapchain framebuffer for egui")?;
        }
        Ok(target)
    }
}

impl Drop for OverlayTarget {
    fn drop(&mut self) {
        unsafe {
            for image in &self.images {
                self.device.destroy_framebuffer(image.framebuffer, None);
                self.device.destroy_image_view(image.view, None);
            }
            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

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
            &device.surface.instance,
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
            .framebuffer(target.images[index].framebuffer)
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
