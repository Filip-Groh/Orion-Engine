use std::ffi::c_char;

use anyhow::{Context, Result};
use ash::{Entry, khr, vk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub(crate) struct VulkanInstance {
    pub handle: ash::Instance,
    entry: Entry,
}

impl VulkanInstance {
    pub(super) fn new(extensions: &[*const c_char]) -> Result<Self> {
        let entry = unsafe { Entry::load() }.context("load Vulkan")?;
        let mut extensions = extensions.to_vec();
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
        let handle = unsafe { entry.create_instance(&info, None) }
            .context("create a Vulkan 1.1 instance")?;
        Ok(Self { handle, entry })
    }
}

impl Drop for VulkanInstance {
    fn drop(&mut self) {
        unsafe { self.handle.destroy_instance(None) };
    }
}

pub(crate) struct Surface {
    pub loader: khr::surface::Instance,
    pub handle: vk::SurfaceKHR,
}

impl Surface {
    pub(super) fn new(
        instance: &VulkanInstance,
        window: &(impl HasDisplayHandle + HasWindowHandle),
    ) -> Result<Self> {
        let display = window.display_handle()?.as_raw();
        let window = window.window_handle()?.as_raw();
        // The renderer owns the window until after the surface is destroyed.
        let handle = unsafe {
            ash_window::create_surface(&instance.entry, &instance.handle, display, window, None)
        }
        .context("create the Vulkan window surface")?;
        Ok(Self {
            loader: khr::surface::Instance::new(&instance.entry, &instance.handle),
            handle,
        })
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface(self.handle, None) };
    }
}
