use anyhow::{Context, Result, ensure};
use ash::vk;

use crate::device::ComputeDevice;

pub(super) struct SwapchainConfig {
    pub extent: vk::Extent2D,
    pub format: vk::SurfaceFormatKHR,
    usage: vk::ImageUsageFlags,
    transform: vk::SurfaceTransformFlagsKHR,
    present_mode: vk::PresentModeKHR,
    alpha: vk::CompositeAlphaFlagsKHR,
    image_count: u32,
}

impl SwapchainConfig {
    pub(super) fn query(
        device: &ComputeDevice,
        requested_extent: vk::Extent2D,
        overlay: bool,
    ) -> Result<Option<Self>> {
        let surface = device.window_surface();
        let physical = device.physical;
        let capabilities = unsafe {
            surface
                .loader
                .get_physical_device_surface_capabilities(physical, surface.handle)
        }
        .context("query swapchain capabilities")?;
        let Some(extent) = choose_extent(&capabilities, requested_extent) else {
            return Ok(None);
        };
        let usage = image_usage(overlay);
        ensure!(
            capabilities.supported_usage_flags.contains(usage),
            "the surface no longer supports the required swapchain image usage {usage:?}"
        );
        let formats = unsafe {
            surface
                .loader
                .get_physical_device_surface_formats(physical, surface.handle)
        }
        .context("query swapchain formats")?;
        let format = choose_format(&formats).context("no compatible 8-bit sRGB surface format")?;
        let modes = unsafe {
            surface
                .loader
                .get_physical_device_surface_present_modes(physical, surface.handle)
        }
        .context("query swapchain present modes")?;
        let alpha = choose_alpha(capabilities.supported_composite_alpha)
            .context("no supported swapchain composite alpha mode")?;
        Ok(Some(Self {
            extent,
            format,
            usage,
            transform: capabilities.current_transform,
            present_mode: choose_present_mode(&modes),
            alpha,
            image_count: image_count(&capabilities),
        }))
    }

    pub(super) fn create_info<'a>(
        &self,
        surface: vk::SurfaceKHR,
        old: vk::SwapchainKHR,
        families: &'a [u32; 2],
    ) -> vk::SwapchainCreateInfoKHR<'a> {
        let info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(self.image_count)
            .image_format(self.format.format)
            .image_color_space(self.format.color_space)
            .image_extent(self.extent)
            .image_array_layers(1)
            .image_usage(self.usage)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(self.transform)
            .composite_alpha(self.alpha)
            .present_mode(self.present_mode)
            .clipped(true)
            .old_swapchain(old);
        if families[0] == families[1] {
            info
        } else {
            info.image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(families)
        }
    }
}

fn choose_extent(
    capabilities: &vk::SurfaceCapabilitiesKHR,
    requested: vk::Extent2D,
) -> Option<vk::Extent2D> {
    let extent = if capabilities.current_extent.width != u32::MAX {
        capabilities.current_extent
    } else {
        let min = capabilities.min_image_extent;
        let max = capabilities.max_image_extent;
        vk::Extent2D {
            width: requested.width.clamp(min.width, max.width),
            height: requested.height.clamp(min.height, max.height),
        }
    };
    (extent.width > 0 && extent.height > 0).then_some(extent)
}

fn choose_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    [vk::PresentModeKHR::IMMEDIATE, vk::PresentModeKHR::MAILBOX]
        .into_iter()
        .find(|mode| modes.contains(mode))
        .unwrap_or(vk::PresentModeKHR::FIFO)
}

fn choose_alpha(supported: vk::CompositeAlphaFlagsKHR) -> Option<vk::CompositeAlphaFlagsKHR> {
    use vk::CompositeAlphaFlagsKHR as Alpha;
    [
        Alpha::OPAQUE,
        Alpha::INHERIT,
        Alpha::PRE_MULTIPLIED,
        Alpha::POST_MULTIPLIED,
    ]
    .into_iter()
    .find(|flag| supported.contains(*flag))
}

fn image_count(capabilities: &vk::SurfaceCapabilitiesKHR) -> u32 {
    let preferred = capabilities.min_image_count.saturating_add(1);
    if capabilities.max_image_count == 0 {
        preferred
    } else {
        preferred.min(capabilities.max_image_count)
    }
}

pub(crate) fn image_usage(overlay: bool) -> vk::ImageUsageFlags {
    let mut usage = vk::ImageUsageFlags::TRANSFER_DST;
    if overlay {
        usage |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
    }
    usage
}

pub(crate) fn choose_format(formats: &[vk::SurfaceFormatKHR]) -> Option<vk::SurfaceFormatKHR> {
    use vk::Format as F;
    formats.iter().copied().find(|format| {
        format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            && matches!(
                format.format,
                F::B8G8R8A8_SRGB | F::B8G8R8A8_UNORM | F::R8G8B8A8_SRGB | F::R8G8B8A8_UNORM
            )
    })
}

#[cfg(test)]
mod tests;
