use std::{
    ffi::{CStr, c_void},
    ptr,
};

use anyhow::{Context, Result};
use ash::{Entry, khr, vk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::swapchain::{choose_format, image_usage};

pub(crate) struct Surface {
    pub instance: ash::Instance,
    pub loader: khr::surface::Instance,
    pub handle: vk::SurfaceKHR,
    _entry: Entry,
}

impl Surface {
    fn new(window: &(impl HasDisplayHandle + HasWindowHandle)) -> Result<Self> {
        let display = window.display_handle()?.as_raw();
        let window = window.window_handle()?.as_raw();
        let entry = unsafe { Entry::load() }.context("load Vulkan")?;
        let mut extensions = ash_window::enumerate_required_extensions(display)
            .context("find the Vulkan extensions for this display")?
            .to_vec();
        let available = unsafe { entry.enumerate_instance_extension_properties(None) }
            .context("enumerate Vulkan instance extensions")?;
        let mut flags = vk::InstanceCreateFlags::empty();
        if available.iter().any(|extension| {
            extension.extension_name_as_c_str() == Ok(khr::portability_enumeration::NAME)
        }) {
            extensions.push(khr::portability_enumeration::NAME.as_ptr());
            flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
        }
        let application = vk::ApplicationInfo::default()
            .application_name(c"Orion Engine")
            .api_version(vk::API_VERSION_1_1);
        let info = vk::InstanceCreateInfo::default()
            .application_info(&application)
            .enabled_extension_names(&extensions)
            .flags(flags);
        let instance = unsafe { entry.create_instance(&info, None) }
            .context("create a Vulkan 1.1 instance")?;
        let mut surface = Self {
            loader: khr::surface::Instance::new(&entry, &instance),
            instance,
            handle: vk::SurfaceKHR::null(),
            _entry: entry,
        };
        // The renderer owns the window until after the surface is destroyed.
        surface.handle = unsafe {
            ash_window::create_surface(&surface._entry, &surface.instance, display, window, None)
        }
        .context("create the Vulkan window surface")?;
        Ok(surface)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.loader.destroy_surface(self.handle, None);
            self.instance.destroy_instance(None);
        }
    }
}

pub(crate) struct ComputeDevice {
    pub handle: ash::Device,
    pub physical: vk::PhysicalDevice,
    pub compute_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub compute_family: u32,
    pub present_family: u32,
    pub limits: vk::PhysicalDeviceLimits,
    pub surface: Surface,
    memory: vk::PhysicalDeviceMemoryProperties,
}

impl ComputeDevice {
    pub fn new(window: &(impl HasDisplayHandle + HasWindowHandle), overlay: bool) -> Result<Self> {
        let surface = Surface::new(window)?;
        let devices = unsafe { surface.instance.enumerate_physical_devices() }
            .context("enumerate Vulkan devices")?;
        for physical in devices {
            let properties = unsafe { surface.instance.get_physical_device_properties(physical) };
            if properties.api_version < vk::API_VERSION_1_1 {
                continue;
            }
            let available = unsafe {
                surface
                    .instance
                    .enumerate_device_extension_properties(physical)
            }
            .context("enumerate Vulkan device extensions")?;
            let supports = |name: &CStr| {
                available
                    .iter()
                    .any(|extension| extension.extension_name_as_c_str() == Ok(name))
            };
            if !supports(khr::swapchain::NAME) {
                continue;
            }
            let Some((compute_family, present_family)) =
                queue_families(&surface, physical, overlay)?
            else {
                continue;
            };
            let capabilities = unsafe {
                surface
                    .loader
                    .get_physical_device_surface_capabilities(physical, surface.handle)
            }
            .context("query Vulkan surface capabilities")?;
            let formats = unsafe {
                surface
                    .loader
                    .get_physical_device_surface_formats(physical, surface.handle)
            }
            .context("query Vulkan surface formats")?;
            if !capabilities
                .supported_usage_flags
                .contains(image_usage(overlay))
                || choose_format(&formats).is_none()
            {
                continue;
            }

            let mut extensions = vec![khr::swapchain::NAME.as_ptr()];
            if supports(c"VK_KHR_portability_subset") {
                extensions.push(c"VK_KHR_portability_subset".as_ptr());
            }
            let queues = [compute_family, present_family].map(|family| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family)
                    .queue_priorities(&[1.0])
            });
            let queue_count = if compute_family == present_family {
                1
            } else {
                2
            };
            let info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queues[..queue_count])
                .enabled_extension_names(&extensions);
            let handle = unsafe { surface.instance.create_device(physical, &info, None) }
                .context("create the Vulkan device")?;
            let compute_queue = unsafe { handle.get_device_queue(compute_family, 0) };
            let present_queue = unsafe { handle.get_device_queue(present_family, 0) };
            let memory = unsafe {
                surface
                    .instance
                    .get_physical_device_memory_properties(physical)
            };
            return Ok(Self {
                handle,
                physical,
                compute_queue,
                present_queue,
                compute_family,
                present_family,
                limits: properties.limits,
                surface,
                memory,
            });
        }
        if overlay {
            anyhow::bail!(
                "no Vulkan 1.1 device supports the graphics, compute, and swapchain usage required for the egui overlay"
            );
        }
        anyhow::bail!(
            "no Vulkan 1.1 device can compute and present to an RGBA8 or BGRA8 swapchain with transfer support"
        )
    }
}

fn queue_families(
    surface: &Surface,
    physical: vk::PhysicalDevice,
    overlay: bool,
) -> Result<Option<(u32, u32)>> {
    let families = unsafe {
        surface
            .instance
            .get_physical_device_queue_family_properties(physical)
    };
    let mut required = vk::QueueFlags::COMPUTE;
    if overlay {
        required |= vk::QueueFlags::GRAPHICS;
    }
    let mut compute = None;
    let mut present = None;
    for (index, family) in families
        .iter()
        .enumerate()
        .filter(|(_, family)| family.queue_count > 0)
    {
        let index = index as u32;
        let can_render = family.queue_flags.contains(required);
        let can_present = unsafe {
            surface
                .loader
                .get_physical_device_surface_support(physical, index, surface.handle)
        }
        .context("query Vulkan queue presentation support")?;
        if can_render && can_present {
            return Ok(Some((index, index)));
        }
        if can_render {
            compute.get_or_insert(index);
        }
        if can_present {
            present.get_or_insert(index);
        }
    }
    Ok(compute.zip(present))
}

impl Drop for ComputeDevice {
    fn drop(&mut self) {
        unsafe { self.handle.destroy_device(None) };
    }
}

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
        let requirements = unsafe { device.handle.get_buffer_memory_requirements(buffer.handle) };
        let memory_type = (0..device.memory.memory_type_count)
            .find(|&index| {
                requirements.memory_type_bits & (1 << index) != 0
                    && device.memory.memory_types[index as usize]
                        .property_flags
                        .contains(flags)
            })
            .context("no suitable memory type for the storage buffer")?;
        let allocation = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type);
        unsafe {
            buffer.memory = device
                .handle
                .allocate_memory(&allocation, None)
                .context("allocate storage buffer memory")?;
            device
                .handle
                .bind_buffer_memory(buffer.handle, buffer.memory, 0)
                .context("bind storage buffer memory")?;
            if upload {
                buffer.mapped = device
                    .handle
                    .map_memory(
                        buffer.memory,
                        0,
                        vk::WHOLE_SIZE,
                        vk::MemoryMapFlags::empty(),
                    )
                    .context("map scene upload memory")?;
            }
        }
        Ok(buffer)
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
