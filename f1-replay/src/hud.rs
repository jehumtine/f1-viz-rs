use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, RichText, Ui};

use crate::theme::{FontRoles, chrome};
use f1_data::engine::player::Frame;
use f1_data::model::domain::SessionInfo;

/// Track status code → (label, color)
fn track_status_display(code: u8) -> (&'static str, Color32) {
    match code {
        1 => ("CLEAR", Color32::from_rgb(0x33, 0xD1, 0x7A)),
        2 => ("YELLOW", Color32::from_rgb(0xFF, 0xD2, 0x3D)),
        4 | 6 | 7 => ("SAFETY CAR", Color32::from_rgb(0xFF, 0x8A, 0x3D)),
        5 => ("RED FLAG", Color32::from_rgb(0xFF, 0x46, 0x55)),
        _ => ("UNKNOWN", chrome::MUTED),
    }
}

/// Format wall clock from session-relative time
fn format_wall_clock(elapsed: std::time::Duration) -> String {
    let hours = elapsed.as_secs() / 3600;
    let mins = (elapsed.as_secs() % 3600) / 60;
    let secs = elapsed.as_secs() % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

/// Render the top HUD bar
pub fn render_hud(ui: &mut Ui, info: &SessionInfo, frame: &Frame, elapsed: std::time::Duration) {
    // egui 0.36: Panel::top() instead of TopBottomPanel::top()
    egui::Panel::top("hud_top")
        .frame(
            egui::Frame::NONE
                .fill(chrome::PANEL)
                .corner_radius(CornerRadius::same(14)), // CornerRadius instead of Rounding
        )
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                // Left: session name + type
                ui.label(
                    RichText::new(format!("{} · {}", info.meeting_name, info.session_name))
                        .font(FontRoles::display(24.0))
                        .color(chrome::TEXT),
                );

                ui.add_space(ui.available_width() * 0.2);

                // Center: lap counter (now a tuple (current, total))
                let (current_lap, total_laps) = frame.state.lap;
                ui.label(
                    RichText::new(format!("LAP {}/{}", current_lap, total_laps))
                        .font(FontRoles::mono(20.0))
                        .color(chrome::TEXT),
                );

                ui.add_space(ui.available_width() * 0.3);

                // Right: track status badge
                let (status_label, status_color) = track_status_display(frame.state.track_status);
                ui.label(
                    RichText::new(status_label)
                        .color(status_color)
                        .font(FontRoles::body(14.0)),
                );

                ui.separator();

                // Track temp (directly from state.weather)
                ui.label(
                    RichText::new(format!(
                        "{}°C TRK",
                        frame.state.weather.unwrap().track_temp_c as u32
                    ))
                    .font(FontRoles::mono(14.0))
                    .color(chrome::MUTED),
                );

                ui.separator();

                // Session elapsed time (simplified — no wall clock for now)
                ui.label(
                    RichText::new(format_wall_clock(elapsed))
                        .font(FontRoles::mono(14.0))
                        .color(chrome::MUTED),
                );
            });
        });
}
