use anyhow::{Context, Result};
use ash::vk;

use crate::{
    SceneFrame,
    device::ComputeDevice,
    scene_buffer::pack_scene,
    storage_buffer::{StorageBuffer, max_storage_bytes},
};

pub(super) struct Dispatch {
    device: ash::Device,
    descriptor_pool: vk::DescriptorPool,
    pub set: vk::DescriptorSet,
    pub command_pool: vk::CommandPool,
    pub command: vk::CommandBuffer,
    pub acquired: vk::Semaphore,
    fence: vk::Fence,
    pending: bool,
    input: Option<StorageBuffer>,
}

impl Dispatch {
    pub(super) fn new(device: &ComputeDevice, layout: vk::DescriptorSetLayout) -> Result<Self> {
        let mut dispatch = Self {
            device: device.handle.clone(),
            descriptor_pool: vk::DescriptorPool::null(),
            set: vk::DescriptorSet::null(),
            command_pool: vk::CommandPool::null(),
            command: vk::CommandBuffer::null(),
            acquired: vk::Semaphore::null(),
            fence: vk::Fence::null(),
            pending: false,
            input: None,
        };
        dispatch.allocate_descriptors(layout)?;
        dispatch.allocate_commands(device.compute_family)?;
        unsafe {
            if device.surface.is_some() {
                dispatch.acquired = dispatch
                    .device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .context("create the image acquisition semaphore")?;
            }
            dispatch.fence = dispatch
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .context("create the dispatch fence")?;
        }
        Ok(dispatch)
    }

    fn allocate_descriptors(&mut self, layout: vk::DescriptorSetLayout) -> Result<()> {
        let sizes = [vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(2)];
        let info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(1)
            .pool_sizes(&sizes);
        self.descriptor_pool = unsafe { self.device.create_descriptor_pool(&info, None) }
            .context("create the dispatch descriptor pool")?;
        let layouts = [layout];
        let info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.descriptor_pool)
            .set_layouts(&layouts);
        self.set = unsafe { self.device.allocate_descriptor_sets(&info) }
            .context("allocate the ray march descriptor set")?[0];
        Ok(())
    }

    fn allocate_commands(&mut self, family: u32) -> Result<()> {
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        self.command_pool = unsafe { self.device.create_command_pool(&pool_info, None) }
            .context("create the dispatch command pool")?;
        let commands = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        self.command = unsafe { self.device.allocate_command_buffers(&commands) }
            .context("allocate the ray march command buffer")?[0];
        Ok(())
    }

    pub(super) fn bind_scene(
        &mut self,
        device: &ComputeDevice,
        frame: SceneFrame<'_>,
        extent: vk::Extent2D,
        bgra: bool,
        pixels: &StorageBuffer,
    ) -> Result<()> {
        let max_bytes = max_storage_bytes(device.limits.max_storage_buffer_range);
        let scene = pack_scene(frame, extent, bgra, max_bytes)?;
        let input = [self.upload(device, &scene)?];
        let output = [pixels.descriptor(pixels.size)];
        let writes = [(0, &input), (1, &output)].map(|(binding, info)| {
            vk::WriteDescriptorSet::default()
                .dst_set(self.set)
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(info)
        });
        unsafe { self.device.update_descriptor_sets(&writes, &[]) };
        Ok(())
    }

    fn upload(&mut self, device: &ComputeDevice, bytes: &[u8]) -> Result<vk::DescriptorBufferInfo> {
        if self
            .input
            .as_ref()
            .is_none_or(|buffer| buffer.size < bytes.len())
        {
            self.input = Some(StorageBuffer::mapped(device, bytes.len())?);
        }
        let input = self.input.as_mut().expect("scene buffer was allocated");
        input.upload(bytes);
        Ok(input.descriptor(bytes.len()))
    }

    pub(super) fn begin_recording(&self) -> ash::prelude::VkResult<()> {
        let begin = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { self.device.begin_command_buffer(self.command, &begin) }
    }

    pub(super) fn submit(&mut self, queue: vk::Queue, ready: Option<vk::Semaphore>) -> Result<()> {
        let commands = [self.command];
        let waits = [self.acquired];
        let stages = [vk::PipelineStageFlags::TRANSFER];
        let mut submit = vk::SubmitInfo::default().command_buffers(&commands);
        if ready.is_some() {
            submit = submit
                .wait_semaphores(&waits)
                .wait_dst_stage_mask(&stages)
                .signal_semaphores(ready.as_slice());
        }
        unsafe {
            self.device
                .reset_fences(&[self.fence])
                .context("reset the dispatch fence")?;
            self.device
                .queue_submit(queue, &[submit], self.fence)
                .context("submit the ray march dispatch")?;
        }
        self.pending = true;
        Ok(())
    }

    pub(super) fn wait(&mut self) -> Result<()> {
        if self.pending {
            unsafe { self.device.wait_for_fences(&[self.fence], true, u64::MAX) }
                .context("wait for the previous ray march dispatch")?;
            self.pending = false;
        }
        Ok(())
    }
}

impl Drop for Dispatch {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_semaphore(self.acquired, None);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}
