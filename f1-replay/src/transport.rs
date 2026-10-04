use std::time::Duration;

use bevy::prelude::{ButtonInput, KeyCode};
use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

use crate::playback::PlaybackClock;
use crate::theme::{FontRoles, chrome};

const AMBER: Color32 = Color32::from_rgb(0xFF, 0xB1, 0x00);

pub fn render_transport(
    ui: &mut Ui,
    clock: &mut PlaybackClock,
    max_secs: f32,
    fps: f32,
    keys: &ButtonInput<KeyCode>,
) {
    let ctx = ui.ctx().clone();
    let screen = ctx.viewport_rect();

    // Deterministic content width: fixed parts (~470 px) + sized slider.
    let w = (screen.width() * 0.62).clamp(560.0, 1040.0);
    let slider_w = (w - 470.0).max(140.0);

    egui::Area::new("transport".into())
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(screen.center().x, screen.max.y - 14.0))
        .show(&ctx, |ui| {
            crate::theme::glass().show(ui, |ui| {
                ui.horizontal(|ui| {
                    if icon_button(ui, clock.playing).clicked() {
                        clock.playing = !clock.playing;
                    }
                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new(fmt(clock.t.as_secs_f32()))
                            .font(FontRoles::mono(14.0))
                            .color(chrome::TEXT),
                    );
                    ui.add_space(6.0);

                    // Amber scrubber — explicit size, no greedy available-width
                    ui.style_mut().visuals.selection.bg_fill = AMBER;
                    ui.style_mut().visuals.selection.stroke = Stroke::new(1.0, AMBER);
                    let mut secs = clock.t.as_secs_f32();
                    let resp = ui.add_sized(
                        Vec2::new(slider_w, 20.0),
                        egui::Slider::new(&mut secs, 0.0..=max_secs).show_value(false),
                    );
                    if resp.changed() {
                        clock.t = Duration::from_secs_f32(secs);
                    }
                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new(fmt(max_secs))
                            .font(FontRoles::mono(14.0))
                            .color(chrome::MUTED),
                    );

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    for s in [0.5f32, 1.0, 2.0, 4.0, 8.0] {
                        let active = (clock.speed - s).abs() < 1e-3;
                        let text = if s < 1.0 {
                            "0.5×".into()
                        } else {
                            format!("{s:.0}×")
                        };
                        let color = if active { AMBER } else { chrome::MUTED };
                        if ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(text)
                                        .font(FontRoles::body(13.0))
                                        .color(color),
                                )
                                .sense(Sense::click()),
                            )
                            .clicked()
                        {
                            clock.speed = s;
                        }
                        ui.add_space(6.0);
                    }

                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new(format!("{fps:.0}"))
                            .font(FontRoles::mono(11.0))
                            .color(chrome::MUTED),
                    );
                });
            });
        });

    if keys.just_pressed(KeyCode::Space) {
        clock.playing = !clock.playing;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        clock.t = clock.t.saturating_add(Duration::from_secs(10));
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        clock.t = clock.t.saturating_sub(Duration::from_secs(10));
    }
}

fn fmt(secs: f32) -> String {
    let s = secs as u32;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// Play/pause drawn with the painter — immune to missing font glyphs.
fn icon_button(ui: &mut Ui, playing: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    let p = ui.painter();
    let color = if resp.hovered() { chrome::TEXT } else { AMBER };
    if playing {
        let bar = Vec2::new(3.0, 12.0);
        p.rect_filled(
            Rect::from_center_size(Pos2::new(rect.center().x - 3.5, rect.center().y), bar),
            1.0,
            color,
        );
        p.rect_filled(
            Rect::from_center_size(Pos2::new(rect.center().x + 3.5, rect.center().y), bar),
            1.0,
            color,
        );
    } else {
        let pts = vec![
            Pos2::new(rect.center().x - 5.0, rect.center().y - 7.0),
            Pos2::new(rect.center().x - 5.0, rect.center().y + 7.0),
            Pos2::new(rect.center().x + 7.0, rect.center().y),
        ];
        p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
    }
    resp
}
