#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use ratatui::style::Color;

use crate::configure::{
    available_color_names, color_to_lua_string, current_theme_name, parse_color, reset_config,
    set_color_override, theme_named_colors, themed_color, SymbolThemeName, ThemeName,
};

#[test]
fn parses_named_and_hex_colors() {
    assert_eq!(parse_color("blue"), Some(Color::Blue));
    assert_eq!(parse_color("#00ff7f"), Some(Color::Rgb(0, 255, 127)));
    assert_eq!(parse_color(""), None);
    assert_eq!(parse_color("bogus"), None);
}

#[test]
fn resets_to_selected_theme_and_applies_overrides() {
    reset_config(ThemeName::Light);
    assert_eq!(current_theme_name(), ThemeName::Light);
    assert_eq!(
        themed_color(|colors| colors.content.app_brand),
        Color::Rgb(30, 58, 95)
    );

    set_color_override("content.app_brand", Color::Rgb(1, 2, 3)).expect("override should succeed");
    assert_eq!(
        themed_color(|colors| colors.content.app_brand),
        Color::Rgb(1, 2, 3)
    );

    reset_config(ThemeName::Dark);
    assert_eq!(
        themed_color(|colors| colors.content.app_brand),
        Color::Yellow
    );
}

#[test]
fn exposes_named_colors_for_scaffolding() {
    let names = available_color_names();
    assert!(names.contains(&"content.app_brand"));
    assert!(names.contains(&"chart.series_8"));

    let dark = theme_named_colors(ThemeName::Dark);
    assert!(dark.iter().any(|(name, _)| *name == "surface.panel_border"));
    assert_eq!(color_to_lua_string(Color::Rgb(12, 34, 56)), "#0c2238");
}

#[test]
fn parses_symbol_theme_aliases() {
    assert_eq!(
        SymbolThemeName::parse("compatibility"),
        Some(SymbolThemeName::Compatibility)
    );
    assert_eq!(
        SymbolThemeName::parse("compatibility_mode"),
        Some(SymbolThemeName::Compatibility)
    );
    assert_eq!(
        SymbolThemeName::parse("compatibility-mode"),
        Some(SymbolThemeName::Compatibility)
    );
}
