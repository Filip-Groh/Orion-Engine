mod events;

use std::{collections::VecDeque, f64::consts::TAU, time::Instant};

use anyhow::{Context, Result};
use orion_renderer::{Renderer, SceneFrame, TransformSnapshot};
use orion_scene::{Scene, SineBob, Sphere};
use winit::{event_loop::EventLoop, window::Window};

use crate::{
    freecam::{Freecam, FreecamInput},
    rates::{self, RateCounter},
    tick::{TICKS_PER_SECOND, TickClock},
};

const BOB_PERIOD_TICKS: u64 = 2 * TICKS_PER_SECOND;

pub(crate) struct App {
    renderer: Option<Renderer<Window>>,
    scene: Scene,
    freecam: Freecam,
    clock: Option<TickClock>,
    previous: TransformSnapshot,
    bob_phase: u64,
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
            previous: TransformSnapshot::default(),
            bob_phase: 0,
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
        let last_tick = ticks.end - 1;
        for tick in ticks {
            // Only the state immediately before the last tick can be rendered after catch-up.
            if tick == last_tick {
                self.previous.capture(&self.scene)?;
            }
            let camera = self.scene.camera_mut()?;
            let inputs = std::iter::from_fn(|| {
                let (input_tick, _) = self.input.front()?;
                if *input_tick == tick {
                    self.input.pop_front().map(|(_, input)| input)
                } else {
                    None
                }
            });
            self.freecam.step(camera, inputs);
            self.bob_phase = (self.bob_phase + 1) % BOB_PERIOD_TICKS;
            let angle = TAU * self.bob_phase as f64 / BOB_PERIOD_TICKS as f64;
            let wave = libm::sin(angle) as f32;
            for (sphere, bob) in self.scene.world.query_mut::<(&mut Sphere, &SineBob)>() {
                sphere.center[1] = bob.rest_y + bob.amplitude * wave;
            }
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
        let now = Instant::now();
        let fraction = self
            .clock
            .as_ref()
            .map_or(1.0, |clock| clock.interpolation_fraction(now));
        let frame = SceneFrame::interpolated(&self.scene, &self.previous, fraction);
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
        self.input.clear();
        self.freecam.clear_keys();
        self.fps = RateCounter::default();
        self.tps = RateCounter::default();
        self.occluded = false;
    }
}
