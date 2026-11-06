use anyhow::{Context, Result};
use ash::vk;

pub(crate) struct OverlayTarget {
    pub(super) device: ash::Device,
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
        let mut target = Self {
            device: device.clone(),
            format,
            render_pass: create_render_pass(device, format)?,
            images: Vec::new(),
        };
        for image in images {
            target.add_image(image, extent)?;
        }
        Ok(target)
    }

    pub(super) fn framebuffer(&self, index: usize) -> vk::Framebuffer {
        self.images[index].framebuffer
    }

    fn add_image(&mut self, image: vk::Image, extent: vk::Extent2D) -> Result<()> {
        let device = &self.device;
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(self.format)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .level_count(1)
                    .layer_count(1),
            );
        let view = unsafe { device.create_image_view(&view_info, None) }
            .context("create a swapchain image view for egui")?;
        let attachments = [view];
        let framebuffer_info = vk::FramebufferCreateInfo::default()
            .render_pass(self.render_pass)
            .attachments(&attachments)
            .width(extent.width)
            .height(extent.height)
            .layers(1);
        let framebuffer = match unsafe { device.create_framebuffer(&framebuffer_info, None) } {
            Ok(framebuffer) => framebuffer,
            Err(error) => {
                unsafe { device.destroy_image_view(view, None) };
                return Err(error).context("create a swapchain framebuffer for egui");
            }
        };
        self.images.push(OverlayImage { view, framebuffer });
        Ok(())
    }
}

fn create_render_pass(device: &ash::Device, format: vk::Format) -> Result<vk::RenderPass> {
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
    unsafe { device.create_render_pass(&info, None) }.context("create the egui render pass")
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
