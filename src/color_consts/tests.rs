use ratatui::style::Color;

use crate::color_consts::{
    available_color_names, color_to_lua_string, current_theme_name, parse_color, reset_theme,
    set_color_override, theme_named_colors, title_color, ThemeName,
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
    reset_theme(ThemeName::Light);
    assert_eq!(current_theme_name(), ThemeName::Light);
    assert_eq!(title_color(), Color::Rgb(96, 48, 0));

    set_color_override("text.title", Color::Rgb(1, 2, 3)).expect("override should succeed");
    assert_eq!(title_color(), Color::Rgb(1, 2, 3));

    reset_theme(ThemeName::Dark);
    assert_eq!(title_color(), Color::Yellow);
}

#[test]
fn exposes_named_colors_for_scaffolding() {
    let names = available_color_names();
    assert!(names.contains(&"text.title"));
    assert!(names.contains(&"chart.series_8"));

    let dark = theme_named_colors(ThemeName::Dark);
    assert!(dark.iter().any(|(name, _)| *name == "surface.panel_border"));
    assert_eq!(color_to_lua_string(Color::Rgb(12, 34, 56)), "#0c2238");
}
