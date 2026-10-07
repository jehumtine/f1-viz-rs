use std::collections::HashMap;

use bevy_egui::egui::{self, Align2, Pos2, Stroke, Vec2};
use f1_data::engine::track::UnifiedCarState;
use f1_data::model::domain::Driver;

use crate::telemetry::TelemetrySelection;
use crate::theme::{FontRoles, TeamPalette, chrome};
use bevy::prelude::Resource;
use std::collections::VecDeque;

#[derive(Resource, Default)]
pub struct TrailState {
    pub trails: HashMap<u8, VecDeque<[f32; 2]>>,
}

/// Ring-buffer the last ~150 frames of position for the given cars.
pub fn update_trails(state: &mut TrailState, nums: &[u8], cars: &HashMap<u8, UnifiedCarState>) {
    state.trails.retain(|num, _| nums.contains(num));
    for num in nums {
        if let Some(st) = cars.get(num) {
            let q = state.trails.entry(*num).or_default();
            q.push_back([st.position.x_m as f32, st.position.y_m as f32]);
            while q.len() > 150 {
                q.pop_front();
            }
        }
    }
}

/// Alpha-ramped, width-tapered polylines under the labels layer.
pub fn render_trails(
    ctx: &egui::Context,
    screen: egui::Rect,
    cam_xy: Vec2,
    world_per_px: f32,
    state: &TrailState,
    palette: &TeamPalette,
) {
    egui::Area::new("trails".into())
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let painter = ui.painter();
            let center = screen.center();
            let proj = |w: [f32; 2]| {
                egui::pos2(
                    center.x + (w[0] - cam_xy.x) / world_per_px,
                    center.y - (w[1] - cam_xy.y) / world_per_px,
                )
            };
            for (&num, q) in state.trails.iter() {
                if q.len() < 2 {
                    continue;
                }
                let c = palette.get(num);
                let pts: Vec<egui::Pos2> = q.iter().map(|w| proj(*w)).collect();
                let len = pts.len();
                for i in 1..len {
                    let t = i as f32 / len as f32;
                    let alpha = (t * t * 170.0) as u8;
                    let col = egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha);
                    painter
                        .line_segment([pts[i - 1], pts[i]], egui::Stroke::new(1.0 + 2.5 * t, col));
                }
            }
        });
}

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
