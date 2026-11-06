use std::{
    ops::Range,
    time::{Duration, Instant},
};

const TICKS_PER_SECOND: u64 = 60;
const NANOS_PER_SECOND: u128 = 1_000_000_000;
pub(crate) const TICK_SECONDS: f32 = 1.0 / TICKS_PER_SECOND as f32;

pub(crate) struct TickClock {
    start: Instant,
    completed: u64,
}

impl TickClock {
    pub fn new(start: Instant) -> Self {
        Self {
            start,
            completed: 0,
        }
    }

    fn elapsed_ticks(&self, now: Instant) -> u64 {
        let elapsed = now.duration_since(self.start);
        (elapsed.as_nanos() * u128::from(TICKS_PER_SECOND) / NANOS_PER_SECOND) as u64
    }

    pub fn input_tick(&self, now: Instant) -> u64 {
        // New input belongs to the next boundary, even when older ticks are overdue.
        self.elapsed_ticks(now) + 1
    }

    pub fn take_due_ticks(&mut self, now: Instant) -> Range<u64> {
        let first = self.completed + 1;
        self.completed = self.elapsed_ticks(now);
        first..self.completed + 1
    }

    pub fn next_deadline(&self) -> Instant {
        // Round each boundary from the epoch so nanosecond rounding cannot accumulate.
        let next = self.completed + 1;
        let seconds = next / TICKS_PER_SECOND;
        let fraction = u128::from(next % TICKS_PER_SECOND) * NANOS_PER_SECOND;
        let nanos = fraction.div_ceil(u128::from(TICKS_PER_SECOND)) as u32;
        self.start + Duration::new(seconds, nanos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_boundaries_do_not_drift_or_skip_overdue_ticks() {
        let start = Instant::now();
        let mut clock = TickClock::new(start);
        let first = clock.next_deadline();
        assert_eq!(first - start, Duration::from_nanos(16_666_667));
        assert!(
            clock
                .take_due_ticks(first - Duration::from_nanos(1))
                .is_empty()
        );
        assert_eq!(clock.take_due_ticks(first), 1..2);
        assert_eq!(clock.take_due_ticks(start + Duration::from_secs(1)), 2..61);
        assert_eq!(clock.next_deadline(), first + Duration::from_secs(1));
    }
}
