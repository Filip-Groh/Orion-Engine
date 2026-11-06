use std::{ffi::c_void, ptr, slice};

use anyhow::{Context, Result, ensure};
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
    pub fn mapped(device: &ComputeDevice, size: usize) -> Result<Self> {
        Self::new(
            device,
            size,
            vk::BufferUsageFlags::STORAGE_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )
    }

    pub fn device_local_pixels(device: &ComputeDevice, size: usize) -> Result<Self> {
        Self::new(
            device,
            size,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )
    }

    fn new(
        device: &ComputeDevice,
        size: usize,
        usage: vk::BufferUsageFlags,
        flags: vk::MemoryPropertyFlags,
    ) -> Result<Self> {
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
        if flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
            buffer.mapped = unsafe {
                buffer.device.map_memory(
                    buffer.memory,
                    0,
                    vk::WHOLE_SIZE,
                    vk::MemoryMapFlags::empty(),
                )
            }
            .context("map storage buffer memory")?;
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

    /// The caller must make the entire buffer visible to the host and wait for GPU writes.
    pub unsafe fn read_mapped(&self) -> &[u8] {
        assert!(!self.mapped.is_null());
        unsafe { slice::from_raw_parts(self.mapped.cast(), self.size) }
    }
}

pub(super) fn max_storage_bytes(storage_limit: u32) -> usize {
    (storage_limit as usize).min(isize::MAX as usize)
}

pub(super) fn pixel_buffer_size(extent: vk::Extent2D, storage_limit: u32) -> Result<usize> {
    let count = u64::from(extent.width) * u64::from(extent.height);
    let max_bytes = max_storage_bytes(storage_limit) as u64;
    ensure!(
        count > 0 && count <= max_bytes / 4,
        "render extent exceeds the device's pixel buffer limits"
    );
    Ok(count as usize * 4)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_size_accepts_exact_capacity_and_rejects_one_byte_less() {
        let extent = vk::Extent2D {
            width: 3,
            height: 5,
        };
        assert_eq!(pixel_buffer_size(extent, 60).unwrap(), 60);
        assert!(pixel_buffer_size(extent, 59).is_err());
    }

    #[test]
    fn pixel_size_rejects_empty_and_oversized_extents() {
        let empty = vk::Extent2D {
            width: 0,
            height: 5,
        };
        assert!(pixel_buffer_size(empty, 60).is_err());
        let oversized = vk::Extent2D {
            width: u32::MAX,
            height: u32::MAX,
        };
        assert!(pixel_buffer_size(oversized, u32::MAX).is_err());
    }
}
