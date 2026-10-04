use std::collections::HashMap;

use bevy_egui::egui::{self, RichText, Ui};

use crate::race::{GapKind, LeaderRow};
use crate::theme::{FontRoles, TeamPalette, chrome};
use f1_data::model::domain::Driver;

fn gap_text(gap: &GapKind) -> String {
    match gap {
        GapKind::Leader => "—".to_string(),
        GapKind::Time(t) => format!("+{:.3}", t),
        GapKind::Laps(n) => format!("+{}L", n),
        GapKind::Retired => "RET".to_string(),
    }
}

/// Render the right-side leaderboard panel
pub fn render_leaderboard(
    ui: &mut Ui,
    rows: &[LeaderRow],
    drivers: &HashMap<u8, Driver>,
    palette: &TeamPalette,
) {
    let ctx = ui.ctx().clone();
    egui::Area::new("leaderboard".into())
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-14.0, 14.0))
        .show(&ctx, |ui| {
            ui.set_width(230.0);
            crate::theme::glass().show(ui, |ui| {
                // Header
                ui.label(
                    RichText::new("CLASSIFICATION")
                        .font(FontRoles::display(18.0))
                        .color(chrome::MUTED),
                );
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                egui::Grid::new("leaderboard_grid")
                    .num_columns(3)
                    .spacing([8.0, 6.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for (i, row) in rows.iter().enumerate() {
                            let driver = drivers.get(&row.num);
                            let code = driver.map(|d| d.code.as_str()).unwrap_or("???");
                            let team_color = palette.get(row.num);

                            // Position (right-aligned mono)
                            ui.label(
                                RichText::new(format!("{:>2}", i + 1))
                                    .font(FontRoles::mono(14.0))
                                    .color(chrome::MUTED),
                            );

                            // Driver code with team color accent
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(3.0, 14.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 1.0, team_color);
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(code)
                                        .font(FontRoles::body(14.0))
                                        .color(chrome::TEXT),
                                );
                            });

                            // Gap (right-aligned mono)
                            ui.label(
                                RichText::new(gap_text(&row.gap))
                                    .font(FontRoles::mono(14.0))
                                    .color(chrome::TEXT),
                            );

                            ui.end_row();
                        }
                    });
            });
        });
}
