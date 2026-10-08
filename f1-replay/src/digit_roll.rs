use std::collections::HashMap;

use bevy::ecs::resource::Resource;
use bevy_egui::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};

#[derive(Default)]
pub struct GapRoll {
    pub prev: String,
    pub curr: String,
    pub progress: f32,
}

#[derive(Default, Resource)]
pub struct DigitRollState {
    pub gaps: HashMap<u8, GapRoll>,
}

impl DigitRollState {
    /// Update all rows in one pass: snap changed digits to progress 0, advance all.
    pub fn update(&mut self, rows: &[(u8, String)], dt: f32) {
        const ANIM_SECS: f32 = 0.18;

        // Snapshot current keys so we can drop stale entries
        let live: std::collections::HashSet<u8> = rows.iter().map(|(n, _)| *n).collect();
        self.gaps.retain(|n, _| live.contains(n));

        for (num, new_text) in rows {
            let entry = self.gaps.entry(*num).or_default();
            if entry.curr != *new_text {
                entry.prev = std::mem::replace(&mut entry.curr, new_text.clone());
                entry.progress = 0.0;
            }
            if entry.progress < 1.0 {
                entry.progress = (entry.progress + dt / ANIM_SECS).min(1.0);
            }
        }
    }

    pub fn snapshot(&self, num: u8) -> (&str, &str, f32) {
        match self.gaps.get(&num) {
            Some(e) => (e.prev.as_str(), e.curr.as_str(), e.progress),
            None => ("", "", 1.0),
        }
    }
}

pub fn render_digit_roll(
    ui: &mut egui::Ui,
    value: &str,
    prev_value: &str,
    progress: f32,
    font: FontId,
    color: Color32,
) {
    let digit_width = font.size * 0.62 + 2.0;
    let digit_height = font.size * 1.3;

    let max_len = value.len().max(prev_value.len()).max(1);
    let total_width = digit_width * max_len as f32;

    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(total_width, digit_height), egui::Sense::hover());

    let padded_value = format!("{:>width$}", value, width = max_len);
    let padded_prev = format!("{:>width$}", prev_value, width = max_len);

    for (i, (curr_char, prev_char)) in padded_value.chars().zip(padded_prev.chars()).enumerate() {
        let x = rect.min.x + i as f32 * digit_width;
        let digit_rect = Rect::from_min_size(
            Pos2::new(x, rect.min.y),
            Vec2::new(digit_width, digit_height),
        );

        if curr_char != prev_char && progress < 1.0 {
            ui.painter().with_clip_rect(digit_rect).text(
                Pos2::new(
                    x + digit_width / 2.0,
                    rect.center().y - digit_height * progress,
                ),
                Align2::CENTER_CENTER,
                prev_char.to_string(),
                font.clone(),
                color.gamma_multiply(1.0 - progress),
            );
            ui.painter().with_clip_rect(digit_rect).text(
                Pos2::new(
                    x + digit_width / 2.0,
                    rect.center().y + digit_height * (1.0 - progress),
                ),
                Align2::CENTER_CENTER,
                curr_char.to_string(),
                font.clone(),
                color.gamma_multiply(progress),
            );
        } else {
            ui.painter().text(
                Pos2::new(x + digit_width / 2.0, rect.center().y),
                Align2::CENTER_CENTER,
                curr_char.to_string(),
                font.clone(),
                color,
            );
        }
    }
}
