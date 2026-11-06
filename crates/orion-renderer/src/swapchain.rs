use anyhow::{Context, Result, ensure};
use ash::{khr, vk};

use crate::device::{ComputeDevice, StorageBuffer};
use crate::overlay::OverlayTarget;

pub(crate) struct PresentImage {
    pub handle: vk::Image,
    pub ready: vk::Semaphore,
}

pub(crate) struct Swapchain {
    device: ash::Device,
    pub loader: khr::swapchain::Device,
    pub handle: vk::SwapchainKHR,
    pub requested_extent: vk::Extent2D,
    pub extent: vk::Extent2D,
    pub bgra: bool,
    pub overlay: Option<OverlayTarget>,
    pub pixels: StorageBuffer,
    pub images: Vec<PresentImage>,
}

impl Swapchain {
    pub fn new(
        device: &ComputeDevice,
        requested_extent: vk::Extent2D,
        old: vk::SwapchainKHR,
        overlay: bool,
    ) -> Result<Option<Self>> {
        let surface = &device.surface;
        let capabilities = unsafe {
            surface
                .loader
                .get_physical_device_surface_capabilities(device.physical, surface.handle)
        }
        .context("query swapchain capabilities")?;
        let extent = if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            let min = capabilities.min_image_extent;
            let max = capabilities.max_image_extent;
            vk::Extent2D {
                width: requested_extent.width.clamp(min.width, max.width),
                height: requested_extent.height.clamp(min.height, max.height),
            }
        };
        if extent.width == 0 || extent.height == 0 {
            return Ok(None);
        }
        let usage = image_usage(overlay);
        ensure!(
            capabilities.supported_usage_flags.contains(usage),
            "the surface no longer supports the required swapchain image usage {usage:?}"
        );
        let formats = unsafe {
            surface
                .loader
                .get_physical_device_surface_formats(device.physical, surface.handle)
        }
        .context("query swapchain formats")?;
        let format = choose_format(&formats).context("no compatible 8-bit sRGB surface format")?;
        let present_modes = unsafe {
            surface
                .loader
                .get_physical_device_surface_present_modes(device.physical, surface.handle)
        }
        .context("query swapchain present modes")?;
        let present_mode = [vk::PresentModeKHR::IMMEDIATE, vk::PresentModeKHR::MAILBOX]
            .into_iter()
            .find(|mode| present_modes.contains(mode))
            .unwrap_or(vk::PresentModeKHR::FIFO);
        let alpha = [
            vk::CompositeAlphaFlagsKHR::OPAQUE,
            vk::CompositeAlphaFlagsKHR::INHERIT,
            vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        ]
        .into_iter()
        .find(|flag| capabilities.supported_composite_alpha.contains(*flag))
        .context("no supported swapchain composite alpha mode")?;
        let mut image_count = capabilities.min_image_count.saturating_add(1);
        if capabilities.max_image_count > 0 {
            image_count = image_count.min(capabilities.max_image_count);
        }
        let pixel_count = u64::from(extent.width) * u64::from(extent.height);
        ensure!(
            pixel_count <= u64::from(device.limits.max_storage_buffer_range / 4),
            "the swapchain extent exceeds the device's storage buffer limit"
        );
        let pixels = StorageBuffer::new(device, pixel_count as usize * 4, false)?;

        let families = [device.compute_family, device.present_family];
        let mut info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.handle)
            .min_image_count(image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(usage)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(alpha)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(old);
        if families[0] != families[1] {
            info = info
                .image_sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&families);
        }
        let loader = khr::swapchain::Device::new(&surface.instance, &device.handle);
        let handle = unsafe { loader.create_swapchain(&info, None) }
            .context("create the Vulkan swapchain")?;
        let mut swapchain = Self {
            device: device.handle.clone(),
            loader,
            handle,
            requested_extent,
            extent,
            bgra: matches!(
                format.format,
                vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB
            ),
            overlay: None,
            pixels,
            images: Vec::new(),
        };
        let images = unsafe { swapchain.loader.get_swapchain_images(handle) }
            .context("get the swapchain images")?;
        for handle in images {
            // Reacquiring this image guarantees the previous presentation consumed its semaphore.
            let ready = unsafe {
                device
                    .handle
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
            }
            .context("create a presentation semaphore")?;
            swapchain.images.push(PresentImage { handle, ready });
        }
        if overlay {
            swapchain.overlay = Some(OverlayTarget::new(
                &device.handle,
                format.format,
                extent,
                swapchain.images.iter().map(|image| image.handle),
            )?);
        }
        Ok(Some(swapchain))
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        self.overlay = None;
        unsafe {
            for image in &self.images {
                self.device.destroy_semaphore(image.ready, None);
            }
            self.loader.destroy_swapchain(self.handle, None);
        }
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
