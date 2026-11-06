mod events;

use std::time::Instant;

use anyhow::{Context, Result};
use orion_renderer::{Renderer, SceneFrame, TransformSnapshot};
use winit::{event_loop::EventLoop, window::Window};

use crate::{
    freecam::FreecamInput,
    rates::{self, RateCounter},
    simulation::Simulation,
    tick::TickClock,
};

pub(crate) struct App {
    renderer: Option<Renderer<Window>>,
    simulation: Simulation,
    clock: Option<TickClock>,
    previous: TransformSnapshot,
    cursor_captured: bool,
    fps: RateCounter,
    tps: RateCounter,
    occluded: bool,
    error: Option<anyhow::Error>,
}

#[cfg(test)]
mod tests;

impl App {
    pub(crate) fn new(simulation: Simulation) -> Self {
        Self {
            renderer: None,
            clock: None,
            previous: TransformSnapshot::default(),
            cursor_captured: false,
            fps: RateCounter::default(),
            tps: RateCounter::default(),
            occluded: false,
            simulation,
            error: None,
        }
    }

    pub(crate) fn run(mut self) -> Result<()> {
        EventLoop::new()
            .context("create the window event loop")?
            .run_app(&mut self)
            .context("run the window event loop")?;
        self.error.map_or(Ok(()), Err)
    }

    fn start_simulation(&mut self, now: Instant) {
        self.clock = Some(TickClock::new(now, self.simulation.tick));
        self.tps.record(now, 0);
    }

    fn queue_input(&mut self, now: Instant, input: FreecamInput) {
        if let Some(clock) = &self.clock
            && let Some(tick) = clock.due_tick(now).checked_add(1)
        {
            let queue = &mut self.simulation.input;
            // A checkpoint can already contain inputs scheduled beyond this boundary.
            let index = queue.partition_point(|(queued_tick, _)| *queued_tick <= tick);
            queue.insert(index, (tick, input));
        }
    }

    fn advance_simulation(&mut self, now: Instant) -> Result<()> {
        let Some(clock) = &self.clock else {
            return Ok(());
        };
        let target = clock.due_tick(now);
        if target > self.simulation.tick {
            let count = target - self.simulation.tick;
            self.simulation.advance_to(target, &mut self.previous)?;
            self.tps.record(now, count);
        }
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
        let now = Instant::now();
        let fraction = self.clock.as_ref().map_or(1.0, |clock| {
            clock.interpolation_fraction(now, self.simulation.tick)
        });
        let frame = SceneFrame::interpolated(&self.simulation.scene, &self.previous, fraction);
        let presented = renderer
            .render_frame_with_overlay(
                frame,
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
        self.previous = TransformSnapshot::default();
        self.simulation.input.clear();
        self.simulation.freecam.clear_keys();
        self.fps = RateCounter::default();
        self.tps = RateCounter::default();
        self.occluded = false;
    }
}
