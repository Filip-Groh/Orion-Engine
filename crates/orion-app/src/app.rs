mod events;

use std::{collections::VecDeque, time::Instant};

use anyhow::{Context, Result};
use orion_renderer::Renderer;
use orion_scene::Scene;
use winit::{event_loop::EventLoop, window::Window};

use crate::{
    freecam::{Freecam, FreecamInput},
    rates::{self, RateCounter},
    tick::TickClock,
};

pub(crate) struct App {
    renderer: Option<Renderer<Window>>,
    scene: Scene,
    freecam: Freecam,
    clock: Option<TickClock>,
    input: VecDeque<(u64, FreecamInput)>,
    cursor_captured: bool,
    fps: RateCounter,
    tps: RateCounter,
    occluded: bool,
    error: Option<anyhow::Error>,
}

#[cfg(test)]
mod tests;

impl App {
    pub(crate) fn new(scene: Scene) -> Result<Self> {
        Ok(Self {
            renderer: None,
            freecam: Freecam::new(&scene.camera()?),
            clock: None,
            input: VecDeque::new(),
            cursor_captured: false,
            fps: RateCounter::default(),
            tps: RateCounter::default(),
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

    fn start_simulation(&mut self, now: Instant) {
        self.clock = Some(TickClock::new(now));
        self.tps.record(now, 0);
    }

    fn queue_input(&mut self, now: Instant, input: FreecamInput) {
        if let Some(clock) = &self.clock {
            self.input.push_back((clock.input_tick(now), input));
        }
    }

    fn advance_simulation(&mut self, now: Instant) -> Result<()> {
        let Some(clock) = &mut self.clock else {
            return Ok(());
        };
        let ticks = clock.take_due_ticks(now);
        if ticks.is_empty() {
            return Ok(());
        }

        let count = ticks.end - ticks.start;
        let camera = self.scene.camera_mut()?;
        for tick in ticks {
            let inputs = std::iter::from_fn(|| {
                let (input_tick, _) = self.input.front()?;
                if *input_tick == tick {
                    self.input.pop_front().map(|(_, input)| input)
                } else {
                    None
                }
            });
            self.freecam.step(camera, inputs);
        }
        self.tps.record(now, count);
        Ok(())
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
            self.fps = RateCounter::default();
            return Ok(());
        }
        let scale = window.scale_factor() as f32;
        let presented = renderer
            .render_with_overlay(
                &self.scene,
                [size.width, size.height],
                scale,
                |ui| rates::draw(ui, &self.fps, &self.tps),
                Window::pre_present_notify,
            )
            .context("render the scene")?;
        if presented {
            self.fps.record(Instant::now(), 1);
        }
        renderer.window().request_redraw();
        Ok(())
    }

    fn drop_renderer(&mut self) {
        self.release_cursor();
        self.renderer = None;
        self.clock = None;
        self.input.clear();
        self.freecam.clear_keys();
        self.fps = RateCounter::default();
        self.tps = RateCounter::default();
        self.occluded = false;
    }
}
