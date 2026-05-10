use ratatui::prelude::Color;

use crate::color_consts::palette::ThemeName;

use super::types::ThemeColors;

pub fn available_color_names() -> Vec<&'static str> {
    ThemeColors::for_theme(ThemeName::Dark).all_color_names()
}

pub fn theme_named_colors(theme: ThemeName) -> Vec<(&'static str, Color)> {
    ThemeColors::for_theme(theme).all_color_entries()
}

pub(crate) fn normalize_color_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_sep = false;
    for ch in name.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else if ch == '.' {
            '.'
        } else {
            '_'
        };
        if mapped == '_' || mapped == '.' {
            if !(last_sep && out.ends_with(mapped)) {
                out.push(mapped);
            }
            last_sep = true;
        } else {
            out.push(mapped);
            last_sep = false;
        }
    }
    out.trim_matches(|c| c == '_' || c == '.').to_string()
}

pub fn available_theme_names() -> &'static [&'static str] {
    &["dark", "light"]
}
