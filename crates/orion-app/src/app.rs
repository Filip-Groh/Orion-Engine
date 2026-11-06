mod events;

use anyhow::{Context, Result};
use orion_renderer::Renderer;
use orion_scene::Scene;
use winit::{event_loop::EventLoop, window::Window};

use crate::{fps::FpsCounter, freecam::Freecam};

pub(crate) struct App {
    renderer: Option<Renderer<Window>>,
    scene: Scene,
    freecam: Freecam,
    fps: FpsCounter,
    occluded: bool,
    error: Option<anyhow::Error>,
}

impl App {
    pub(crate) fn new(scene: Scene) -> Result<Self> {
        Ok(Self {
            renderer: None,
            freecam: Freecam::new(&scene.camera()?),
            fps: FpsCounter::default(),
            occluded: false,
            scene,
            error: None,
        })
    }

    pub(crate) fn run(mut self) -> Result<()> {
        EventLoop::new()
            .context("create the window event loop")?
            .run_app(&mut self)
            .context("run the window event loop")?;
        self.error.map_or(Ok(()), Err)
    }

    fn redraw(&mut self) -> Result<()> {
        let Some(renderer) = &mut self.renderer else {
            return Ok(());
        };
        let window = renderer.window();
        let size = window.inner_size();
        if self.occluded
            || size.width == 0
            || size.height == 0
            || window.is_minimized() == Some(true)
        {
            self.freecam.release(window);
            self.fps = FpsCounter::default();
            return Ok(());
        }
        if let Ok(camera) = self.scene.camera_mut() {
            self.freecam.step(camera);
        }
        let scale = window.scale_factor() as f32;
        let presented = renderer
            .render_with_overlay(
                &self.scene,
                [size.width, size.height],
                scale,
                |ui| self.fps.draw(ui),
                Window::pre_present_notify,
            )
            .context("render the scene")?;
        if presented {
            self.fps.presented();
        }
        renderer.window().request_redraw();
        Ok(())
    }

    fn drop_renderer(&mut self) {
        if let Some(renderer) = self.renderer.take() {
            self.freecam.release(renderer.window());
        }
        self.fps = FpsCounter::default();
        self.occluded = false;
    }
}
