use anyhow::{Context, Result};
use ash::vk;

use crate::{
    SceneFrame,
    device::ComputeDevice,
    dispatch::Dispatch,
    pipeline::{Pipeline, dispatch_groups},
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
        let size = pixel_buffer_size(extent, device.limits.max_storage_buffer_range)?;
        let pixels = StorageBuffer::mapped(&device, size)?;
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
        self.dispatch
            .bind_scene(&self.device, frame, self.extent, false, &self.pixels)?;
        self.record()?;
        self.dispatch.submit(self.device.compute_queue, None)?;
        self.dispatch.wait()?;
        // The host-coherent buffer is fully written and the submission fence has signaled.
        Ok(unsafe { self.pixels.read_mapped() })
    }

    fn record(&self) -> Result<()> {
        let device = &self.device.handle;
        let command = self.dispatch.command;
        self.dispatch
            .begin_recording()
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
        .context("record the offscreen dispatch")
    }
}

impl Drop for OffscreenRenderer {
    fn drop(&mut self) {
        let _ = unsafe { self.device.handle.device_wait_idle() };
    }
}
