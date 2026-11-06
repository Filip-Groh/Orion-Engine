use std::ffi::CStr;

use anyhow::{Context, Result};
use ash::{khr, vk};

use super::PORTABILITY_SUBSET;
use crate::{
    surface::Surface,
    swapchain::{choose_format, image_usage},
};

pub(super) struct DeviceCandidate {
    pub(super) compute_family: u32,
    pub(super) present_family: u32,
    pub(super) limits: vk::PhysicalDeviceLimits,
    pub(super) portability_subset: bool,
}

impl DeviceCandidate {
    pub(super) fn inspect(
        surface: &Surface,
        physical: vk::PhysicalDevice,
        overlay: bool,
    ) -> Result<Option<Self>> {
        let properties = unsafe { surface.instance.get_physical_device_properties(physical) };
        if properties.api_version < vk::API_VERSION_1_1 {
            return Ok(None);
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
            return Ok(None);
        }
        let Some((compute_family, present_family)) = queue_families(surface, physical, overlay)?
        else {
            return Ok(None);
        };
        if !supports_swapchain(surface, physical, overlay)? {
            return Ok(None);
        }
        Ok(Some(Self {
            compute_family,
            present_family,
            limits: properties.limits,
            portability_subset: supports(PORTABILITY_SUBSET),
        }))
    }
}

fn supports_swapchain(
    surface: &Surface,
    physical: vk::PhysicalDevice,
    overlay: bool,
) -> Result<bool> {
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
    Ok(capabilities
        .supported_usage_flags
        .contains(image_usage(overlay))
        && choose_format(&formats).is_some())
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
