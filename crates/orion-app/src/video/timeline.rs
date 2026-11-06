use std::num::{NonZeroU32, NonZeroU64};

use anyhow::{Context, Result};

pub(super) struct Timeline {
    pub(super) start: u64,
    pub(super) end: u64,
    tps: u128,
    fps: u128,
    pub(super) frames: u64,
}

impl Timeline {
    pub(super) fn new(
        start: u64,
        ticks: NonZeroU64,
        tps: NonZeroU32,
        fps: NonZeroU32,
    ) -> Result<Self> {
        let ticks = ticks.get();
        let end = start
            .checked_add(ticks)
            .context("export would overflow the simulation tick counter")?;
        let tps = u128::from(tps.get());
        let fps = u128::from(fps.get());
        let frames = u64::try_from((u128::from(ticks) * fps).div_ceil(tps))
            .context("export would contain too many frames")?;
        Ok(Self {
            start,
            end,
            tps,
            fps,
            frames,
        })
    }

    /// Returns the enclosing completed tick and the blend from its predecessor.
    pub(super) fn sample(&self, index: u64) -> (u64, f32) {
        debug_assert!(index < self.frames);
        let position = u128::from(index) * self.tps;
        let tick = self.start + position.div_ceil(self.fps) as u64;
        let remainder = position % self.fps;
        let fraction = if remainder == 0 {
            1.0
        } else {
            (remainder as f64 / self.fps as f64) as f32
        };
        (tick, fraction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline(start: u64, ticks: u64, tps: u32, fps: u32) -> Timeline {
        Timeline::new(
            start,
            ticks.try_into().unwrap(),
            tps.try_into().unwrap(),
            fps.try_into().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn samples_the_checkpoint_and_interpolates_through_the_last_tick() {
        let video = timeline(1000, 100, 20, 60);
        assert_eq!(video.frames, 300);
        assert_eq!(video.sample(0), (1000, 1.0));
        assert_eq!(video.sample(1), (1001, 1.0 / 3.0));
        assert_eq!(video.sample(2), (1001, 2.0 / 3.0));
        assert_eq!(video.sample(3), (1001, 1.0));
        assert_eq!(video.sample(299), (1100, 2.0 / 3.0));
    }

    #[test]
    fn rounds_fractional_frame_counts_up_and_supports_skipping_ticks() {
        let video = timeline(1000, 5, 30, 24);
        assert_eq!(video.frames, 4);
        assert_eq!(video.sample(1), (1002, 0.25));
        assert_eq!(video.sample(3), (1004, 0.75));

        let short = timeline(u64::MAX - 1, 1, 30, 24);
        assert_eq!(short.frames, 1);
        assert_eq!(short.sample(0), (u64::MAX - 1, 1.0));
    }

    #[test]
    fn rejects_tick_and_frame_count_overflow() {
        let one = NonZeroU32::MIN;
        let tick_error = Timeline::new(u64::MAX, NonZeroU64::MIN, one, one)
            .err()
            .unwrap();
        assert_eq!(
            tick_error.to_string(),
            "export would overflow the simulation tick counter"
        );

        let frame_error = Timeline::new(0, NonZeroU64::MAX, one, NonZeroU32::new(2).unwrap())
            .err()
            .unwrap();
        assert_eq!(
            frame_error.to_string(),
            "export would contain too many frames"
        );
    }
}
