//! Windowed Vulkan compute renderer for signed distance fields and triangles.

mod bvh;
mod device;
mod dispatch;
mod frame;
mod overlay;
mod overlay_target;
mod pipeline;
mod scene_buffer;
mod storage_buffer;
mod surface;
mod swapchain;

use anyhow::{Result, ensure};
use orion_scene::Scene;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use device::ComputeDevice;
use dispatch::Dispatch;
use frame::FrameOutcome;
use overlay::Overlay;
use pipeline::Pipeline;
use swapchain::Swapchain;

pub struct Renderer<W> {
    // Vulkan children are dropped before the device, and the surface before the window.
    swapchain: Option<Swapchain>,
    swapchain_dirty: bool,
    overlay_enabled: bool,
    overlay: Option<Overlay>,
    dispatch: Dispatch,
    pipeline: Pipeline,
    device: ComputeDevice,
    window: W,
}

impl<W: HasDisplayHandle + HasWindowHandle> Renderer<W> {
    pub fn new(window: W) -> Result<Self> {
        Self::create(window, false)
    }

    pub fn new_with_overlay(window: W) -> Result<Self> {
        Self::create(window, true)
    }

    fn create(window: W, overlay_enabled: bool) -> Result<Self> {
        let device = ComputeDevice::new(&window, overlay_enabled)?;
        let pipeline = Pipeline::new(&device.handle)?;
        let dispatch = Dispatch::new(&device, pipeline.descriptor_layout)?;
        Ok(Self {
            swapchain: None,
            swapchain_dirty: false,
            overlay_enabled,
            overlay: None,
            dispatch,
            pipeline,
            device,
            window,
        })
    }

    pub fn window(&self) -> &W {
        &self.window
    }

    /// Renders a frame and returns whether another redraw is needed to present it.
    /// A zero-sized window is skipped. Drop the renderer if a Vulkan operation fails.
    pub fn render(
        &mut self,
        scene: &Scene,
        size: [u32; 2],
        before_present: impl FnOnce(&W),
    ) -> Result<bool> {
        let result = self.render_frame(
            scene,
            size,
            None::<(f32, fn(&mut egui::Ui))>,
            before_present,
        )?;
        Ok(matches!(result, FrameOutcome::Retry))
    }

    /// Renders a passive egui overlay and returns whether the frame was presented.
    pub fn render_with_overlay(
        &mut self,
        scene: &Scene,
        size: [u32; 2],
        scale: f32,
        draw_ui: impl FnMut(&mut egui::Ui),
        before_present: impl FnOnce(&W),
    ) -> Result<bool> {
        ensure!(
            self.overlay_enabled,
            "create the renderer with egui overlay support first"
        );
        ensure!(
            scale.is_finite() && scale > 0.0,
            "egui requires a positive finite scale factor"
        );
        let result = self.render_frame(scene, size, Some((scale, draw_ui)), before_present)?;
        Ok(matches!(result, FrameOutcome::Presented))
    }
}

impl<W> Drop for Renderer<W> {
    fn drop(&mut self) {
        let _ = unsafe { self.device.handle.device_wait_idle() };
    }
}
