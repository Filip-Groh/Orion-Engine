mod selection;

use std::ffi::CStr;

use anyhow::{Context, Result};
use ash::{khr, vk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::surface::Surface;
use selection::DeviceCandidate;

pub(crate) struct ComputeDevice {
    pub handle: ash::Device,
    pub physical: vk::PhysicalDevice,
    pub compute_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub compute_family: u32,
    pub present_family: u32,
    pub limits: vk::PhysicalDeviceLimits,
    pub surface: Surface,
    pub memory: vk::PhysicalDeviceMemoryProperties,
}

impl ComputeDevice {
    pub fn new(window: &(impl HasDisplayHandle + HasWindowHandle), overlay: bool) -> Result<Self> {
        let surface = Surface::new(window)?;
        let devices = unsafe { surface.instance.enumerate_physical_devices() }
            .context("enumerate Vulkan devices")?;
        for physical in devices {
            if let Some(candidate) = DeviceCandidate::inspect(&surface, physical, overlay)? {
                return Self::open(surface, physical, candidate);
            }
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

    fn open(
        surface: Surface,
        physical: vk::PhysicalDevice,
        candidate: DeviceCandidate,
    ) -> Result<Self> {
        let DeviceCandidate {
            compute_family,
            present_family,
            limits,
            portability_subset,
        } = candidate;
        let mut extensions = vec![khr::swapchain::NAME.as_ptr()];
        if portability_subset {
            extensions.push(PORTABILITY_SUBSET.as_ptr());
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
        Ok(Self {
            compute_queue: unsafe { handle.get_device_queue(compute_family, 0) },
            present_queue: unsafe { handle.get_device_queue(present_family, 0) },
            memory: unsafe {
                surface
                    .instance
                    .get_physical_device_memory_properties(physical)
            },
            handle,
            physical,
            compute_family,
            present_family,
            limits,
            surface,
        })
    }
}

const PORTABILITY_SUBSET: &CStr = c"VK_KHR_portability_subset";

impl Drop for ComputeDevice {
    fn drop(&mut self) {
        unsafe { self.handle.destroy_device(None) };
    }
}
