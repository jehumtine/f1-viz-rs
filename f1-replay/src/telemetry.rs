use std::collections::HashMap;

use bevy::prelude::Resource;
use bevy_egui::egui::{self, Color32, Rect, RichText, Ui};
use f1_data::engine::track::UnifiedCarState;
use f1_data::model::domain::Driver;

use crate::theme::{FontRoles, TeamPalette, chrome};

const GREEN: Color32 = Color32::from_rgb(0x39, 0xD9, 0x8A);
const RED: Color32 = Color32::from_rgb(0xFF, 0x45, 0x3A);

#[derive(Resource, Default)]
pub struct TelemetrySelection(pub Vec<u8>);

impl TelemetrySelection {
    pub fn toggle(&mut self, num: u8) {
        if let Some(i) = self.0.iter().position(|n| *n == num) {
            self.0.remove(i);
        } else if self.0.len() < 3 {
            self.0.push(num);
        }
    }
}

pub fn render_panel(
    ctx: &egui::Context,
    cars: &HashMap<u8, UnifiedCarState>,
    drivers: &HashMap<u8, Driver>,
    palette: &TeamPalette,
    sel: &TelemetrySelection,
) {
    if sel.0.is_empty() {
        return;
    }
    let screen = ctx.viewport_rect();
    egui::Area::new("telemetry".into())
        .pivot(egui::Align2::LEFT_BOTTOM)
        .fixed_pos(egui::pos2(screen.min.x + 14.0, screen.max.y - 66.0)) // clears the transport
        .show(ctx, |ui| {
            ui.set_width(250.0);
            crate::theme::glass().show(ui, |ui| {
                for (i, &num) in sel.0.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);
                    }
                    render_driver(ui, drivers.get(&num), cars.get(&num), palette.get(num));
                }
            });
        });
}

fn render_driver(
    ui: &mut Ui,
    driver: Option<&Driver>,
    state: Option<&UnifiedCarState>,
    color: Color32,
) {
    let code = driver.map(|d| d.code.as_str()).unwrap_or("???");
    let t = state.map(|s| s.telemetry);

    // Header: accent | code …… big speed
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(3.0, 18.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 1.0, color);
        ui.add_space(6.0);
        ui.label(
            RichText::new(code)
                .font(FontRoles::display(18.0))
                .color(chrome::TEXT),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new("km/h")
                    .font(FontRoles::mono(11.0))
                    .color(chrome::MUTED),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("{}", t.map(|t| t.speed_kph).unwrap_or(0)))
                    .font(FontRoles::display(28.0))
                    .color(chrome::TEXT),
            );
        });
    });
    ui.add_space(4.0);

    // Gear / DRS / brake row
    let gear = t.map(|t| t.gear).unwrap_or(0);
    let drs = t.map(|t| t.drs).unwrap_or(0);
    let brake = t.map(|t| t.brake).unwrap_or(false);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(if gear == 0 {
                "N".into()
            } else {
                format!("G{gear}")
            })
            .font(FontRoles::mono(13.0))
            .color(chrome::TEXT),
        );
        if drs > 0 {
            ui.add_space(10.0);
            ui.label(
                RichText::new("DRS")
                    .font(FontRoles::body(12.0))
                    .color(chrome::AMBER),
            );
        }
        if brake {
            ui.add_space(10.0);
            ui.label(
                RichText::new("BRAKE")
                    .font(FontRoles::body(12.0))
                    .color(RED),
            );
        }
    });
    ui.add_space(4.0);

    // Bars
    bar(
        ui,
        "RPM",
        t.map(|t| t.rpm).unwrap_or(0) as f32 / 12000.0,
        color,
    );
    bar(
        ui,
        "THR",
        t.map(|t| t.throttle_pct).unwrap_or(0) as f32 / 100.0,
        GREEN,
    );
    bar(ui, "BRK", if brake { 1.0 } else { 0.0 }, RED);
}

fn bar(ui: &mut Ui, label: &str, frac: f32, fill: Color32) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .font(FontRoles::mono(10.0))
                .color(chrome::MUTED),
        );
        ui.add_space(4.0);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 3.0, Color32::from_white_alpha(18));
        let w = rect.width() * frac.clamp(0.0, 1.0);
        if w > 1.0 {
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, egui::vec2(w, rect.height())),
                3.0,
                fill,
            );
        }
    });
    ui.add_space(3.0);
}
