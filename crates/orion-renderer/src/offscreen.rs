use anyhow::{Context, Result};
use ash::vk;

use crate::{
    SceneFrame,
    device::ComputeDevice,
    dispatch::Dispatch,
    frame::dispatch_groups,
    pipeline::Pipeline,
    scene_buffer::pack_scene,
    storage_buffer::{StorageBuffer, pixel_buffer_size},
};

/// Renders into a host-readable pixel buffer without a window or surface.
pub struct OffscreenRenderer {
    dispatch: Dispatch,
    pipeline: Pipeline,
    pixels: StorageBuffer,
    device: ComputeDevice,
    extent: vk::Extent2D,
    groups: [u32; 2],
}

impl OffscreenRenderer {
    pub fn new([width, height]: [u32; 2]) -> Result<Self> {
        let extent = vk::Extent2D { width, height };
        let device = ComputeDevice::headless()?;
        let groups = dispatch_groups(extent, device.limits.max_compute_work_group_count)?;
        let pixels = StorageBuffer::mapped(&device, pixel_buffer_size(&device, extent)?)?;
        let pipeline = Pipeline::new(&device.handle)?;
        let dispatch = Dispatch::new(&device, pipeline.descriptor_layout)?;
        Ok(Self {
            dispatch,
            pipeline,
            pixels,
            device,
            extent,
            groups,
        })
    }

    /// Returns tightly packed, top-to-bottom RGBA8 pixels. Drop the renderer on Vulkan failure.
    pub fn render(&mut self, frame: SceneFrame<'_>) -> Result<&[u8]> {
        let max_bytes =
            (self.device.limits.max_storage_buffer_range as usize).min(isize::MAX as usize);
        let scene = pack_scene(frame, self.extent, false, max_bytes)?;
        self.dispatch
            .bind_buffers(&self.device, &scene, &self.pixels)?;
        let device = &self.device.handle;
        let command = self.dispatch.command;
        let begin = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { device.begin_command_buffer(command, &begin) }
            .context("begin recording the offscreen dispatch")?;
        self.pipeline
            .record(command, self.dispatch.set, self.groups);
        let readback = vk::MemoryBarrier::default()
            .src_access_mask(vk::AccessFlags::SHADER_WRITE)
            .dst_access_mask(vk::AccessFlags::HOST_READ);
        unsafe {
            device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::HOST,
                vk::DependencyFlags::empty(),
                &[readback],
                &[],
                &[],
            );
            device.end_command_buffer(command)
        }
        .context("record the offscreen dispatch")?;
        self.dispatch.submit(self.device.compute_queue, None)?;
        self.dispatch.wait()?;
        // The host-coherent buffer is fully written and the submission fence has signaled.
        Ok(unsafe { self.pixels.read_mapped() })
    }
}

impl Drop for OffscreenRenderer {
    fn drop(&mut self) {
        let _ = unsafe { self.device.handle.device_wait_idle() };
    }
}
