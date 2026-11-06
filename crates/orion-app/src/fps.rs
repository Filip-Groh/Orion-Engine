use std::time::{Duration, Instant};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Default)]
pub(crate) struct FpsCounter {
    sample_start: Option<Instant>,
    frames: u32,
    fps: Option<f64>,
}

impl FpsCounter {
    pub fn presented(&mut self) {
        let now = Instant::now();
        let Some(start) = self.sample_start else {
            self.sample_start = Some(now);
            return;
        };
        self.frames += 1;
        let elapsed = now - start;
        if elapsed >= SAMPLE_INTERVAL {
            self.fps = Some(f64::from(self.frames) / elapsed.as_secs_f64());
            self.sample_start = Some(now);
            self.frames = 0;
        }
    }

    pub fn draw(&self, ui: &egui::Ui) {
        let text = self
            .fps
            .map_or_else(|| "-- FPS".to_owned(), |fps| format!("{fps:.0} FPS"));
        let painter = ui.painter();
        let text =
            painter.layout_no_wrap(text, egui::FontId::monospace(14.0), egui::Color32::WHITE);
        let padding = egui::vec2(8.0, 4.0);
        let origin = ui.clip_rect().min + egui::vec2(12.0, 12.0);
        painter.rect_filled(
            egui::Rect::from_min_size(origin, text.size() + 2.0 * padding),
            4.0,
            egui::Color32::from_black_alpha(192),
        );
        painter.galley(origin + padding, text, egui::Color32::WHITE);
    }
}
