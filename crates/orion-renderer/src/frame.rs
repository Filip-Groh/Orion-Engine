mod commands;

use anyhow::{Context, Result, ensure};
use ash::vk;
use orion_scene::Scene;

use crate::{Renderer, overlay::Overlay, scene_buffer::pack_scene, swapchain::Swapchain};

// Matches @workgroup_size in raymarch.wgsl.
const WORKGROUP_SIZE: u32 = 8;

pub(super) enum FrameOutcome {
    Skipped,
    Retry,
    Presented,
}

impl<W> Renderer<W> {
    pub(super) fn render_frame(
        &mut self,
        scene: &Scene,
        [width, height]: [u32; 2],
        overlay: Option<(f32, impl FnMut(&mut egui::Ui))>,
        before_present: impl FnOnce(&W),
    ) -> Result<FrameOutcome> {
        if width == 0 || height == 0 {
            return Ok(FrameOutcome::Skipped);
        }
        self.dispatch.wait()?;
        self.ensure_swapchain(vk::Extent2D { width, height })?;
        let Some(swapchain) = &self.swapchain else {
            return Ok(FrameOutcome::Skipped);
        };
        let limits = &self.device.limits;
        let extent = swapchain.extent;
        let groups = dispatch_groups(extent, limits.max_compute_work_group_count)?;
        let max_bytes = (limits.max_storage_buffer_range as usize).min(isize::MAX as usize);
        let scene_bytes = pack_scene(scene, extent, swapchain.bgra, max_bytes)?;
        self.dispatch
            .bind_buffers(&self.device, &scene_bytes, &swapchain.pixels)?;

        let (index, suboptimal) = match swapchain.acquire(self.dispatch.acquired) {
            Ok(image) => image,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.swapchain_dirty = true;
                return Ok(FrameOutcome::Retry);
            }
            Err(vk::Result::TIMEOUT | vk::Result::NOT_READY) => return Ok(FrameOutcome::Retry),
            Err(error) => return Err(error).context("acquire a swapchain image"),
        };
        let ready = swapchain.images[index as usize].ready;
        // Build egui only after acquisition so a retry cannot discard texture updates.
        let overlay_frame = overlay
            .map(|(scale, draw_ui)| {
                self.overlay
                    .as_mut()
                    .expect("egui renderer was initialized")
                    .prepare(
                        self.device.compute_queue,
                        self.dispatch.command_pool,
                        extent,
                        scale,
                        draw_ui,
                    )
            })
            .transpose()?;
        self.record(index as usize, groups, overlay_frame)?;
        self.dispatch.submit(self.device.compute_queue, ready)?;
        before_present(&self.window);
        self.present(index, suboptimal)
    }

    fn present(&mut self, index: u32, acquired_suboptimal: bool) -> Result<FrameOutcome> {
        let swapchain = self.swapchain.as_ref().expect("swapchain was acquired");
        match swapchain.present(self.device.present_queue, index) {
            Ok(present_suboptimal) => {
                self.swapchain_dirty = acquired_suboptimal || present_suboptimal;
                Ok(FrameOutcome::Presented)
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.swapchain_dirty = true;
                Ok(FrameOutcome::Retry)
            }
            Err(error) => Err(error).context("present the ray marched frame"),
        }
    }

    fn ensure_swapchain(&mut self, extent: vk::Extent2D) -> Result<()> {
        if !self.swapchain_dirty
            && self
                .swapchain
                .as_ref()
                .is_some_and(|chain| chain.requested_extent == extent)
        {
            return Ok(());
        }
        unsafe { self.device.handle.device_wait_idle() }
            .context("wait to recreate the swapchain")?;
        let old = self.swapchain.take();
        let old_handle = old
            .as_ref()
            .map_or(vk::SwapchainKHR::null(), |swapchain| swapchain.handle);
        self.swapchain = Swapchain::new(&self.device, extent, old_handle, self.overlay_enabled)?;
        if let Some(target) = self
            .swapchain
            .as_ref()
            .and_then(|chain| chain.overlay.as_ref())
            && self
                .overlay
                .as_ref()
                .is_none_or(|overlay| overlay.format != target.format)
        {
            self.overlay = Some(Overlay::new(&self.device, target)?);
        }
        self.swapchain_dirty = false;
        Ok(())
    }
}

fn dispatch_groups(extent: vk::Extent2D, limits: [u32; 3]) -> Result<[u32; 2]> {
    let groups = [
        extent.width.div_ceil(WORKGROUP_SIZE),
        extent.height.div_ceil(WORKGROUP_SIZE),
    ];
    ensure!(
        groups[0] <= limits[0] && groups[1] <= limits[1],
        "render extent exceeds the device's compute dispatch limits"
    );
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_rounds_up_and_checks_both_axis_limits() {
        let extent = vk::Extent2D {
            width: 9,
            height: 17,
        };
        assert_eq!(dispatch_groups(extent, [2, 3, 1]).unwrap(), [2, 3]);
        assert!(dispatch_groups(extent, [1, 3, 1]).is_err());
        assert!(dispatch_groups(extent, [2, 2, 1]).is_err());
    }
}
