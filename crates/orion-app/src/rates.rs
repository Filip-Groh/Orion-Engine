use std::time::{Duration, Instant};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Default)]
pub(crate) struct RateCounter {
    sample_start: Option<Instant>,
    count: u64,
    rate: Option<f64>,
}

impl RateCounter {
    pub fn record(&mut self, now: Instant, count: u64) {
        let Some(start) = self.sample_start else {
            self.sample_start = Some(now);
            return;
        };
        self.count += count;
        let elapsed = now - start;
        if elapsed >= SAMPLE_INTERVAL {
            self.rate = Some(self.count as f64 / elapsed.as_secs_f64());
            self.sample_start = Some(now);
            self.count = 0;
        }
    }

    pub(crate) fn label(&self) -> String {
        self.rate
            .map_or_else(|| "--".to_owned(), |rate| format!("{rate:.0}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_events_over_elapsed_time_and_starts_a_new_window() {
        let mut counter = RateCounter::default();
        let start = Instant::now();
        counter.record(start, 0);
        counter.record(start + Duration::from_millis(250), 15);
        assert_eq!(counter.rate, None);

        counter.record(start + SAMPLE_INTERVAL, 15);
        assert_eq!(counter.rate, Some(60.0));
        counter.record(start + Duration::from_millis(1_500), 1);
        assert_eq!(counter.rate, Some(1.0));
    }
}
