use anyhow::{Context, Result};
use ash::vk;

use crate::{Renderer, overlay::OverlayFrame};

impl<W> Renderer<W> {
    pub(super) fn record(
        &mut self,
        index: usize,
        groups: [u32; 2],
        overlay: Option<OverlayFrame>,
    ) -> Result<()> {
        let swapchain = self.swapchain.as_ref().expect("swapchain was acquired");
        let image = swapchain.images[index].handle;
        let device = &self.device.handle;
        let command = self.dispatch.command;
        let begin = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { device.begin_command_buffer(command, &begin) }
            .context("begin recording the ray march dispatch")?;

        self.pipeline.record(command, self.dispatch.set, groups);
        copy_pixels(
            device,
            command,
            swapchain.pixels.handle,
            image,
            swapchain.extent,
        );
        if let Some(frame) = overlay {
            let painter = self
                .overlay
                .as_mut()
                .expect("egui renderer was initialized");
            let target = swapchain
                .overlay
                .as_ref()
                .expect("egui swapchain was initialized");
            painter.record(command, target, index, swapchain.extent, frame)?;
        } else {
            transition_to_present(device, command, image);
        }
        unsafe { device.end_command_buffer(command) }.context("record the ray march dispatch")
    }
}

fn copy_pixels(
    device: &ash::Device,
    command: vk::CommandBuffer,
    pixels: vk::Buffer,
    image: vk::Image,
    extent: vk::Extent2D,
) {
    let pixels_written = vk::MemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::SHADER_WRITE)
        .dst_access_mask(vk::AccessFlags::TRANSFER_READ);
    let to_copy = color_image_barrier(image)
        .old_layout(vk::ImageLayout::UNDEFINED)
        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
    let copy = vk::BufferImageCopy::default()
        .image_subresource(
            vk::ImageSubresourceLayers::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .layer_count(1),
        )
        .image_extent(vk::Extent3D {
            width: extent.width,
            height: extent.height,
            depth: 1,
        });
    unsafe {
        // The transfer stage also waits for acquisition before transitioning the image.
        device.cmd_pipeline_barrier(
            command,
            vk::PipelineStageFlags::COMPUTE_SHADER | vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[pixels_written],
            &[],
            &[to_copy],
        );
        device.cmd_copy_buffer_to_image(
            command,
            pixels,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[copy],
        );
    }
}

fn transition_to_present(device: &ash::Device, command: vk::CommandBuffer, image: vk::Image) {
    let to_present = color_image_barrier(image)
        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE);
    unsafe {
        device.cmd_pipeline_barrier(
            command,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[to_present],
        );
    }
}

fn color_image_barrier(image: vk::Image) -> vk::ImageMemoryBarrier<'static> {
    vk::ImageMemoryBarrier::default()
        .image(image)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .level_count(1)
                .layer_count(1),
        )
}
