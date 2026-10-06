use std::collections::HashMap;

use bevy_egui::egui::{self, Align2, Color32, Pos2, Rect, RichText, Ui, Vec2};

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

pub fn render_leaderboard(
    ui: &mut Ui,
    rows: &[LeaderRow],
    drivers: &HashMap<u8, Driver>,
    palette: &TeamPalette,
    selection: &mut crate::telemetry::TelemetrySelection,
) {
    let ctx = ui.ctx().clone();
    egui::Area::new("leaderboard".into())
        .pivot(egui::Align2::RIGHT_TOP)
        .fixed_pos({
            let screen = ctx.viewport_rect();
            egui::pos2(screen.max.x - 14.0, screen.min.y + 14.0)
        })
        .show(&ctx, |ui| {
            ui.set_width(230.0);
            crate::theme::glass().show(ui, |ui| {
                ui.label(
                    RichText::new("CLASSIFICATION")
                        .font(FontRoles::display(18.0))
                        .color(chrome::MUTED),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new("click a driver for telemetry")
                        .font(FontRoles::mono(10.0))
                        .color(chrome::MUTED),
                );
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                for (i, row) in rows.iter().enumerate() {
                    let code = drivers
                        .get(&row.num)
                        .map(|d| d.code.as_str())
                        .unwrap_or("???");
                    let team_color = palette.get(row.num);
                    let selected = selection.0.contains(&row.num);

                    // One full-width click target per row (same pattern as picker rows)
                    let (rect, resp) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 20.0),
                        egui::Sense::click(),
                    );

                    if selected {
                        ui.painter().rect_filled(
                            rect.shrink(1.0),
                            4.0,
                            Color32::from_white_alpha(18),
                        );
                    } else if resp.hovered() {
                        ui.painter().rect_filled(
                            rect.shrink(1.0),
                            4.0,
                            Color32::from_white_alpha(8),
                        );
                    }

                    let cy = rect.center().y;
                    // position
                    ui.painter().text(
                        Pos2::new(rect.left() + 20.0, cy),
                        Align2::RIGHT_CENTER,
                        format!("{}", i + 1),
                        FontRoles::mono(13.0),
                        chrome::MUTED,
                    );
                    // team accent
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            Pos2::new(rect.left() + 30.0, cy - 7.0),
                            Vec2::new(3.0, 14.0),
                        ),
                        1.0,
                        team_color,
                    );
                    // code
                    ui.painter().text(
                        Pos2::new(rect.left() + 40.0, cy),
                        Align2::LEFT_CENTER,
                        code,
                        FontRoles::body(13.0),
                        if selected { team_color } else { chrome::TEXT },
                    );
                    // gap
                    ui.painter().text(
                        Pos2::new(rect.right() - 4.0, cy),
                        Align2::RIGHT_CENTER,
                        gap_text(&row.gap),
                        FontRoles::mono(13.0),
                        chrome::TEXT,
                    );

                    if resp.clicked() {
                        selection.toggle(row.num);
                    }
                    ui.add_space(2.0);
                }
            });
        });
}
