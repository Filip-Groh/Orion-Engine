mod events;
mod hud;
mod input;

use std::time::Instant;

use anyhow::{Context, Result};
use orion_renderer::{Renderer, SceneFrame, SceneStats, TransformSnapshot};
use winit::{event_loop::EventLoop, window::Window};

use crate::{freecam::FreecamInput, rates::RateCounter, simulation::Simulation, tick::TickClock};

pub(crate) struct App {
    renderer: Option<Renderer<Window>>,
    simulation: Simulation,
    clock: Option<TickClock>,
    previous: TransformSnapshot,
    cursor_captured: bool,
    hud: hud::Hud,
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
            hud: hud::Hud::default(),
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

    fn replace_simulation(&mut self, simulation: Simulation, now: Instant) {
        self.simulation = simulation;
        self.previous = TransformSnapshot::default();
        self.tps = RateCounter::default();
        self.start_simulation(now);
        if let Some(renderer) = &self.renderer {
            renderer.window().request_redraw();
        }
    }

    fn queue_input(&mut self, now: Instant, input: FreecamInput) {
        if let Some(clock) = &self.clock
            && let Some(tick) = clock.due_tick(now).checked_add(1)
        {
            self.simulation.queue_input(tick, input);
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
        let scene = &self.simulation.scene;
        let frame = SceneFrame::interpolated(scene, &self.previous, fraction)
            .with_wireframes(self.hud.wireframes);
        let hud = &mut self.hud;
        let presented = renderer
            .render_frame_with_overlay_input(
                frame,
                [size.width, size.height],
                scale,
                &mut hud.events,
                |ui| {
                    hud.rect = Some(hud::draw(
                        ui,
                        &self.fps,
                        &self.tps,
                        SceneStats::from_scene(scene),
                        &mut hud.wireframes,
                    ));
                },
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
        self.hud.pointer_gone();
        self.hud.events.clear();
        self.hud.rect = None;
        self.clock = None;
        self.previous = TransformSnapshot::default();
        self.simulation.clear_input();
        self.fps = RateCounter::default();
        self.tps = RateCounter::default();
        self.occluded = false;
    }
}
