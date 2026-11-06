use std::{ffi::c_void, ptr};

use anyhow::{Context, Result};
use ash::vk;

use crate::device::ComputeDevice;

pub(crate) struct StorageBuffer {
    device: ash::Device,
    pub handle: vk::Buffer,
    memory: vk::DeviceMemory,
    mapped: *mut c_void,
    pub size: usize,
}

impl StorageBuffer {
    pub fn new(device: &ComputeDevice, size: usize, upload: bool) -> Result<Self> {
        let (usage, flags) = if upload {
            (
                vk::BufferUsageFlags::STORAGE_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )
        } else {
            (
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
        };
        let info = vk::BufferCreateInfo::default()
            .size(size as u64)
            .usage(usage);
        let mut buffer = Self {
            device: device.handle.clone(),
            handle: unsafe { device.handle.create_buffer(&info, None) }
                .context("create a storage buffer")?,
            memory: vk::DeviceMemory::null(),
            mapped: ptr::null_mut(),
            size,
        };
        buffer.allocate_memory(&device.memory, flags)?;
        if upload {
            buffer.mapped = unsafe {
                buffer.device.map_memory(
                    buffer.memory,
                    0,
                    vk::WHOLE_SIZE,
                    vk::MemoryMapFlags::empty(),
                )
            }
            .context("map scene upload memory")?;
        }
        Ok(buffer)
    }

    fn allocate_memory(
        &mut self,
        properties: &vk::PhysicalDeviceMemoryProperties,
        flags: vk::MemoryPropertyFlags,
    ) -> Result<()> {
        let requirements = unsafe { self.device.get_buffer_memory_requirements(self.handle) };
        let memory_type = (0..properties.memory_type_count)
            .find(|&index| {
                requirements.memory_type_bits & (1 << index) != 0
                    && properties.memory_types[index as usize]
                        .property_flags
                        .contains(flags)
            })
            .context("no suitable memory type for the storage buffer")?;
        let allocation = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type);
        self.memory = unsafe { self.device.allocate_memory(&allocation, None) }
            .context("allocate storage buffer memory")?;
        unsafe { self.device.bind_buffer_memory(self.handle, self.memory, 0) }
            .context("bind storage buffer memory")
    }

    pub fn descriptor(&self, size: usize) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo::default()
            .buffer(self.handle)
            .range(size as u64)
    }

    pub fn upload(&mut self, bytes: &[u8]) {
        assert!(!self.mapped.is_null() && bytes.len() <= self.size);
        // The frame fence has completed before updating mapped scene memory.
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), self.mapped.cast(), bytes.len()) };
    }
}

impl Drop for StorageBuffer {
    fn drop(&mut self) {
        unsafe {
            if !self.mapped.is_null() {
                self.device.unmap_memory(self.memory);
            }
            self.device.destroy_buffer(self.handle, None);
            self.device.free_memory(self.memory, None);
        }
    }
}
