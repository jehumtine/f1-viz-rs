use std::collections::HashMap;

use bevy_egui::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Shadow, Style,
};
use f1_data::Driver;

pub mod chrome {
    use bevy_egui::egui::Color32;
    pub const ASPHALT: Color32 = Color32::from_rgb(0x0A, 0x0D, 0x12);
    pub const PANEL: Color32 = Color32::from_rgba_unmultiplied_const(232, 234, 237, 18); // .07
    pub const TEXT: Color32 = Color32::from_rgb(0xE8, 0xEA, 0xED);
    pub const MUTED: Color32 = Color32::from_rgb(0x83, 0x8B, 0x99);
    pub const AMBER: Color32 = Color32::from_rgb(0xFF, 0xB1, 0x00);
}

pub struct FontRoles;
impl FontRoles {
    pub fn install(ctx: &egui::Context) {
        let mut defs = FontDefinitions::default();

        defs.font_data.insert(
            "body".into(),
            FontData::from_static(include_bytes!("../assets/fonts/Barlow-Medium.ttf")).into(),
        );

        defs.font_data.insert(
            "mono".into(),
            FontData::from_static(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"))
                .into(),
        );

        defs.font_data.insert(
            "display".into(),
            FontData::from_static(include_bytes!("../assets/fonts/BarlowCondensed-Bold.ttf"))
                .into(),
        );

        defs.families
            .insert(FontFamily::Proportional, vec!["body".into()]);

        defs.families
            .insert(FontFamily::Monospace, vec!["mono".into()]);

        defs.families
            .insert(FontFamily::Name("display".into()), vec!["display".into()]);

        ctx.set_fonts(defs);

        ctx.global_style_mut(|style| {
            apply_glass(style);
        });
    }
    pub fn display(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name("display".into()))
    }
    pub fn body(size: f32) -> FontId {
        FontId::new(size, FontFamily::Proportional)
    }
    pub fn mono(size: f32) -> FontId {
        FontId::new(size, FontFamily::Monospace)
    }
}

pub fn apply_glass(style: &mut Style) {
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = chrome::PANEL;
    style.visuals.window_fill = chrome::PANEL;
    style.visuals.window_shadow = Shadow::NONE;
    style.visuals.override_text_color = Some(chrome::TEXT);
}

pub struct TeamPalette {
    pub by_number: HashMap<u8, Color32>,
}

impl TeamPalette {
    pub fn from_drivers(drivers: &HashMap<u8, Driver>) -> Self {
        let by_number = drivers
            .iter()
            .map(|(n, d)| {
                let hex =
                    u32::from_str_radix(d.color.trim_start_matches('#'), 16).unwrap_or(0x838B99);
                (
                    *n,
                    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8),
                )
            })
            .collect();
        Self { by_number }
    }
    pub fn get(&self, number: u8) -> Color32 {
        self.by_number
            .get(&number)
            .copied()
            .unwrap_or(chrome::MUTED)
    }
}

pub mod chrome_bevy {
    use bevy::prelude::Color;
    pub const ASPHALT: Color = Color::srgb(
        0x0A as f32 / 255.0,
        0x0D as f32 / 255.0,
        0x12 as f32 / 255.0,
    );
    pub const PANEL: Color = Color::srgba(232.0 / 255.0, 234.0 / 255.0, 237.0 / 255.0, 0.07);
    pub const TEXT: Color = Color::srgb(
        0xE8 as f32 / 255.0,
        0xEA as f32 / 255.0,
        0xED as f32 / 255.0,
    );
    pub const MUTED: Color = Color::srgb(
        0x83 as f32 / 255.0,
        0x8B as f32 / 255.0,
        0x99 as f32 / 255.0,
    );
    pub const AMBER: Color = Color::srgb(
        0xFF as f32 / 255.0,
        0xB1 as f32 / 255.0,
        0x00 as f32 / 255.0,
    );
}
