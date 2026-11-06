use std::time::{Duration, Instant};

pub(crate) const TICKS_PER_SECOND: u64 = 60;
const NANOS_PER_SECOND: u128 = 1_000_000_000;
pub(crate) const TICK_SECONDS: f32 = 1.0 / TICKS_PER_SECOND as f32;

pub(crate) struct TickClock {
    start: Instant,
    start_tick: u64,
}

impl TickClock {
    pub fn new(start: Instant, start_tick: u64) -> Self {
        Self { start, start_tick }
    }

    pub fn due_tick(&self, now: Instant) -> u64 {
        let elapsed = now.duration_since(self.start);
        let ticks = elapsed.as_nanos() * u128::from(TICKS_PER_SECOND) / NANOS_PER_SECOND;
        self.start_tick
            .saturating_add(u64::try_from(ticks).unwrap_or(u64::MAX))
    }

    pub fn next_deadline(&self, completed: u64) -> Option<Instant> {
        Some(self.deadline(completed.checked_add(1)? - self.start_tick))
    }

    pub fn interpolation_fraction(&self, now: Instant, completed: u64) -> f32 {
        let Some(next) = self.next_deadline(completed) else {
            return 1.0;
        };
        let last = self.deadline(completed - self.start_tick);
        let interval = next - last;
        let elapsed = now.saturating_duration_since(last).min(interval);
        (elapsed.as_secs_f64() / interval.as_secs_f64()) as f32
    }

    fn deadline(&self, tick: u64) -> Instant {
        // Round each boundary from the epoch so nanosecond rounding cannot accumulate.
        let seconds = tick / TICKS_PER_SECOND;
        let fraction = u128::from(tick % TICKS_PER_SECOND) * NANOS_PER_SECOND;
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
        let clock = TickClock::new(start, 1000);
        let first = clock.next_deadline(1000).unwrap();
        assert_eq!(first - start, Duration::from_nanos(16_666_667));
        assert_eq!(clock.due_tick(first - Duration::from_nanos(1)), 1000);
        assert_eq!(clock.due_tick(first), 1001);
        assert_eq!(clock.due_tick(start + Duration::from_secs(1)), 1060);
        assert_eq!(
            clock.next_deadline(1060).unwrap(),
            first + Duration::from_secs(1)
        );
    }

    #[test]
    fn interpolation_fraction_stays_between_the_last_two_states() {
        let start = Instant::now();
        let clock = TickClock::new(start, 0);
        let first = clock.next_deadline(0).unwrap();
        let next = clock.next_deadline(1).unwrap();
        assert_eq!(clock.interpolation_fraction(start, 1), 0.0);
        assert_eq!(clock.interpolation_fraction(first, 1), 0.0);
        assert!((clock.interpolation_fraction(first + (next - first) / 2, 1) - 0.5).abs() < 1e-6);
        assert_eq!(clock.interpolation_fraction(next, 1), 1.0);
        assert_eq!(
            clock.interpolation_fraction(next + Duration::from_secs(1), 1),
            1.0
        );
        assert_eq!(clock.interpolation_fraction(next, 2), 0.0);
    }
}
