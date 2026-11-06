use egui::{Event, Modifiers, PointerButton, Pos2, Rect};
use orion_renderer::{SceneStats, Wireframes};

use crate::rates::RateCounter;

#[derive(Default)]
pub(super) struct Hud {
    pub wireframes: Wireframes,
    pub events: Vec<Event>,
    pub rect: Option<Rect>,
    pointer: Option<Pos2>,
    pressed: bool,
}

impl Hud {
    pub fn pointer_moved(&mut self, pos: Pos2) {
        self.pointer = Some(pos);
        self.events.push(Event::PointerMoved(pos));
    }

    pub fn pointer_gone(&mut self) {
        if self.pointer.take().is_some() {
            if self.pressed {
                // Cancel a press before egui loses its pointer position.
                let outside = egui::pos2(-1.0, -1.0);
                self.events.push(Event::PointerMoved(outside));
                self.queue_button(outside, false);
            }
            self.events.push(Event::PointerGone);
        }
    }

    pub fn pointer_button(&mut self, pressed: bool) -> bool {
        let Some(pos) = self.pointer else {
            return false;
        };
        if pressed && !self.rect.is_some_and(|rect| rect.contains(pos)) {
            return false;
        }
        if pressed || self.pressed {
            self.queue_button(pos, pressed);
        }
        true
    }

    fn queue_button(&mut self, pos: Pos2, pressed: bool) {
        self.events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        self.pressed = pressed;
    }
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    fps: &RateCounter,
    tps: &RateCounter,
    stats: SceneStats,
    wireframes: &mut Wireframes,
) -> Rect {
    let text = format!(
        "{} FPS  {} TPS\n\nScene Info\nTriangles: {}\nSDFs: {}\nBVH nodes: {}",
        fps.label(),
        tps.label(),
        stats.triangles,
        stats.sdfs,
        stats.bvh_nodes,
    );
    let viewport = ui.clip_rect();
    let origin = viewport.min + egui::vec2(12.0, 12.0);
    let bounds = Rect::from_min_max(origin, viewport.max.max(origin));
    ui.scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 4))
            .corner_radius(4)
            .fill(egui::Color32::from_black_alpha(192))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(text)
                        .monospace()
                        .size(14.0)
                        .color(egui::Color32::WHITE),
                );
                ui.checkbox(&mut wireframes.meshes, "Mesh wireframes");
                ui.checkbox(&mut wireframes.bvh, "BVH wireframes");
            })
            .response
            .rect
    })
    .inner
    .intersect(viewport)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_hud(context: &egui::Context, hud: &mut Hud) -> Vec<egui::epaint::ClippedShape> {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(960.0, 540.0))),
                events: std::mem::take(&mut hud.events),
                ..Default::default()
            },
            |ui| {
                hud.rect = Some(draw(
                    ui,
                    &RateCounter::default(),
                    &RateCounter::default(),
                    SceneStats {
                        triangles: 4,
                        sdfs: 2,
                        bvh_nodes: 3,
                    },
                    &mut hud.wireframes,
                ));
            },
        );
        output.textures_delta.clear();
        output.shapes
    }

    #[test]
    fn wireframe_checkboxes_toggle_independently_and_cancel_when_the_pointer_leaves() {
        let context = egui::Context::default();
        let mut hud = Hud::default();
        let shapes = render_hud(&context, &mut hud);
        let label_position = |label| {
            shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.pos + text.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .expect("checkbox label is visible")
        };
        let mesh = label_position("Mesh wireframes");
        let bvh = label_position("BVH wireframes");
        assert!(!hud.wireframes.meshes && !hud.wireframes.bvh);

        for (pos, expected) in [
            (mesh, (true, false)),
            (bvh, (true, true)),
            (mesh, (false, true)),
        ] {
            hud.pointer_moved(pos);
            assert!(hud.pointer_button(true));
            render_hud(&context, &mut hud);
            hud.pointer_button(false);
            render_hud(&context, &mut hud);
            assert_eq!((hud.wireframes.meshes, hud.wireframes.bvh), expected);
        }

        hud.pointer_moved(bvh);
        assert!(hud.pointer_button(true));
        render_hud(&context, &mut hud);
        hud.pointer_gone();
        render_hud(&context, &mut hud);
        assert!(hud.wireframes.bvh);

        hud.pointer_moved(egui::pos2(500.0, 400.0));
        assert!(!hud.pointer_button(true));
    }
}
