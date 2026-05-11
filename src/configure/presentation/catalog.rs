use ratatui::prelude::Color;

use super::{
    palette::{SymbolThemeName, ThemeName},
    types::{ThemeColors, UiSymbols},
};

pub fn available_color_names() -> Vec<&'static str> {
    ThemeColors::for_theme(ThemeName::Dark).all_color_names()
}

pub fn theme_named_colors(theme: ThemeName) -> Vec<(&'static str, Color)> {
    ThemeColors::for_theme(theme).all_color_entries()
}

pub fn available_symbol_names() -> Vec<&'static str> {
    UiSymbols::for_theme(SymbolThemeName::Rich).all_symbol_names()
}

pub fn theme_named_symbols(theme: SymbolThemeName) -> Vec<(&'static str, &'static str)> {
    UiSymbols::for_theme(theme).all_symbol_entries()
}

fn normalize_catalog_name(name: &str) -> String {
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

pub(crate) fn normalize_color_name(name: &str) -> String {
    normalize_catalog_name(name)
}

pub(crate) fn normalize_symbol_name(name: &str) -> String {
    normalize_catalog_name(name)
}

pub fn available_theme_names() -> &'static [&'static str] {
    &["dark", "light"]
}

pub fn available_symbol_theme_names() -> &'static [&'static str] {
    &["rich", "compatibility"]
}
