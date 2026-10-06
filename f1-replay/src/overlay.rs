use std::collections::HashMap;

use bevy_egui::egui::{self, Align2, Pos2, Stroke, Vec2};
use f1_data::engine::track::UnifiedCarState;
use f1_data::model::domain::Driver;

use crate::telemetry::TelemetrySelection;
use crate::theme::{FontRoles, chrome};

/// Screen-space car codes + selection rings, drawn under the glass panels.
pub fn render_car_labels(
    ctx: &egui::Context,
    screen: egui::Rect,
    cam_xy: Vec2,
    world_per_px: f32,
    cars: &HashMap<u8, UnifiedCarState>,
    drivers: &HashMap<u8, Driver>,
    sel: &TelemetrySelection,
) {
    egui::Area::new("car_labels".into())
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let painter = ui.painter();
            let center = screen.center();

            for (&num, state) in cars.iter() {
                let w = Vec2::new(state.position.x_m as f32, state.position.y_m as f32);
                let sp = Pos2::new(
                    center.x + (w.x - cam_xy.x) / world_per_px,
                    center.y - (w.y - cam_xy.y) / world_per_px,
                );
                if !screen.contains(sp) {
                    continue;
                }

                let selected = sel.0.contains(&num);
                let dot_r = 6.0 / world_per_px;

                if selected {
                    painter.circle_stroke(sp, dot_r + 4.0, Stroke::new(2.0, chrome::AMBER));
                }

                let code = drivers.get(&num).map(|d| d.code.as_str()).unwrap_or("???");
                painter.text(
                    Pos2::new(sp.x, sp.y - dot_r - 6.0),
                    Align2::CENTER_BOTTOM,
                    code,
                    FontRoles::mono(11.0),
                    if selected {
                        chrome::TEXT
                    } else {
                        chrome::MUTED
                    },
                );
            }
        });
}
