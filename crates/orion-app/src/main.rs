use anyhow::{Context, Result};
use orion_renderer::Renderer;
use orion_scene::Scene;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct App {
    renderer: Option<Renderer<Window>>,
    scene: Scene,
    error: Option<anyhow::Error>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let renderer = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Orion Engine")
                    .with_inner_size(LogicalSize::new(960, 540)),
            )
            .context("create the Orion window")
            .and_then(|window| Renderer::new(window).context("initialize the Vulkan renderer"));
        match renderer {
            Ok(renderer) => {
                renderer.window().request_redraw();
                self.renderer = Some(renderer);
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(renderer) = self
            .renderer
            .as_mut()
            .filter(|renderer| renderer.window().id() == id)
        else {
            return;
        };
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => event_loop.exit(),
            WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::Occluded(false) => {
                renderer.window().request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if renderer.window().is_minimized() == Some(true) {
                    return;
                }
                let size = renderer.window().inner_size();
                match renderer.render(
                    &self.scene,
                    [size.width, size.height],
                    Window::pre_present_notify,
                ) {
                    Ok(true) => renderer.window().request_redraw(),
                    Ok(false) => {}
                    Err(error) => {
                        self.error = Some(error.context("render the demo scene"));
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.renderer = None;
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.renderer = None;
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new().context("create the window event loop")?;
    let mut app = App {
        renderer: None,
        scene: Scene::demo(),
        error: None,
    };
    event_loop
        .run_app(&mut app)
        .context("run the window event loop")?;
    app.error.map_or(Ok(()), Err)
}
