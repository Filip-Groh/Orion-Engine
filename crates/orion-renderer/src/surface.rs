use anyhow::{Context, Result};
use ash::{Entry, khr, vk};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub(crate) struct Surface {
    pub instance: ash::Instance,
    pub loader: khr::surface::Instance,
    pub handle: vk::SurfaceKHR,
    _entry: Entry,
}

impl Surface {
    pub(super) fn new(window: &(impl HasDisplayHandle + HasWindowHandle)) -> Result<Self> {
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
