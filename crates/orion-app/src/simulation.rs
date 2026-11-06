use std::{collections::VecDeque, f64::consts::TAU};

use anyhow::Result;
use orion_renderer::TransformSnapshot;
use orion_scene::{Scene, SineBob, Sphere};

use crate::{
    freecam::{Freecam, FreecamInput},
    tick::TICKS_PER_SECOND,
};

const BOB_PERIOD_TICKS: u64 = 2 * TICKS_PER_SECOND;

pub(crate) struct Simulation {
    pub scene: Scene,
    pub tick: u64,
    pub freecam: Freecam,
    pub input: VecDeque<(u64, FreecamInput)>,
}

impl Simulation {
    pub fn new(scene: Scene) -> Result<Self> {
        let freecam = Freecam::new(&scene.camera()?);
        Ok(Self {
            scene,
            tick: 0,
            freecam,
            input: VecDeque::new(),
        })
    }

    pub fn queue_input(&mut self, tick: u64, input: FreecamInput) {
        // A checkpoint can already contain inputs scheduled beyond this boundary.
        let index = self
            .input
            .partition_point(|(queued_tick, _)| *queued_tick <= tick);
        self.input.insert(index, (tick, input));
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        self.freecam.clear_keys();
    }

    pub fn advance_to(&mut self, target: u64, previous: &mut TransformSnapshot) -> Result<()> {
        for completed in self.tick..target {
            let tick = completed + 1;
            if tick == target {
                previous.capture(&self.scene)?;
            }
            self.step(tick)?;
        }
        Ok(())
    }

    fn step(&mut self, tick: u64) -> Result<()> {
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
        self.animate_spheres(tick);
        self.tick = tick;
        Ok(())
    }

    fn animate_spheres(&mut self, tick: u64) {
        let phase = tick % BOB_PERIOD_TICKS;
        let angle = TAU * phase as f64 / BOB_PERIOD_TICKS as f64;
        let wave = libm::sin(angle) as f32;
        for (sphere, bob) in self.scene.world.query_mut::<(&mut Sphere, &SineBob)>() {
            sphere.center[1] = bob.rest_y + bob.amplitude * wave;
        }
    }
}
