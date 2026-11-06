mod config;

use anyhow::{Context, Result, ensure};
use ash::{khr, vk};

use crate::{device::ComputeDevice, overlay_target::OverlayTarget, storage_buffer::StorageBuffer};
use config::SwapchainConfig;
pub(crate) use config::{choose_format, image_usage};

pub(crate) struct PresentImage {
    pub handle: vk::Image,
    pub ready: vk::Semaphore,
}

pub(crate) struct Swapchain {
    device: ash::Device,
    loader: khr::swapchain::Device,
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
        let Some(config) = SwapchainConfig::query(device, requested_extent, overlay)? else {
            return Ok(None);
        };
        let extent = config.extent;
        let format = config.format.format;
        let pixel_count = u64::from(extent.width) * u64::from(extent.height);
        ensure!(
            pixel_count <= u64::from(device.limits.max_storage_buffer_range / 4),
            "the swapchain extent exceeds the device's storage buffer limit"
        );
        let pixels = StorageBuffer::new(device, pixel_count as usize * 4, false)?;

        let families = [device.compute_family, device.present_family];
        let info = config.create_info(surface.handle, old, &families);
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
                format,
                vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB
            ),
            overlay: None,
            pixels,
            images: Vec::new(),
        };
        swapchain.load_images()?;
        if overlay {
            swapchain.overlay = Some(OverlayTarget::new(
                &device.handle,
                format,
                extent,
                swapchain.images.iter().map(|image| image.handle),
            )?);
        }
        Ok(Some(swapchain))
    }

    fn load_images(&mut self) -> Result<()> {
        let images = unsafe { self.loader.get_swapchain_images(self.handle) }
            .context("get the swapchain images")?;
        for handle in images {
            // Reacquiring this image guarantees the previous presentation consumed its semaphore.
            let ready = unsafe {
                self.device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
            }
            .context("create a presentation semaphore")?;
            self.images.push(PresentImage { handle, ready });
        }
        Ok(())
    }

    pub(super) fn acquire(&self, acquired: vk::Semaphore) -> ash::prelude::VkResult<(u32, bool)> {
        unsafe {
            self.loader
                .acquire_next_image(self.handle, 16_000_000, acquired, vk::Fence::null())
        }
    }

    pub(super) fn present(&self, queue: vk::Queue, index: u32) -> ash::prelude::VkResult<bool> {
        let waits = [self.images[index as usize].ready];
        let chains = [self.handle];
        let indices = [index];
        let info = vk::PresentInfoKHR::default()
            .wait_semaphores(&waits)
            .swapchains(&chains)
            .image_indices(&indices);
        unsafe { self.loader.queue_present(queue, &info) }
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
