use ratatui::style::Color;
use std::sync::MutexGuard;

use crate::configure::{
    available_color_names, available_symbol_names, color_to_lua_string,
    current_auto_layout_settings, current_config_generation, current_theme_name,
    ordered_content_modes, parse_color, reset_config, set_auto_layout_settings, set_color_override,
    set_content_mode_order, theme_named_colors, themed_color, AutoLayoutSettings, LayoutSize,
    PanelLayoutSizes, SymbolThemeName, ThemeName,
};
use crate::ui::state::ContentShowMode;

fn test_guard() -> MutexGuard<'static, ()> {
    crate::test_support::serial_test_guard()
}

#[test]
fn parses_named_and_hex_colors() {
    let _guard = test_guard();
    assert_eq!(parse_color("blue"), Some(Color::Blue));
    assert_eq!(parse_color("#00ff7f"), Some(Color::Rgb(0, 255, 127)));
    assert_eq!(parse_color(""), None);
    assert_eq!(parse_color("bogus"), None);
}

#[test]
fn resets_to_selected_theme_and_applies_overrides() {
    let _guard = test_guard();
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
    let _guard = test_guard();
    let names = available_color_names();
    assert!(names.contains(&"content.app_brand"));
    assert!(names.contains(&"chart.series_8"));
    assert!(names.contains(&"mchart.item_id"));

    let dark = theme_named_colors(ThemeName::Dark);
    assert!(dark.iter().any(|(name, _)| *name == "surface.panel_border"));
    assert_eq!(color_to_lua_string(Color::Rgb(12, 34, 56)), "#0c2238");
}

#[test]
fn exposes_named_symbols_for_scaffolding() {
    let _guard = test_guard();
    let names = available_symbol_names();
    assert!(names.contains(&"chart.error_marker"));
    assert!(names.contains(&"chart.loading_indicator"));
}

#[test]
fn parses_symbol_theme_aliases() {
    let _guard = test_guard();
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

#[test]
fn content_mode_order_reorders_available_modes() {
    let _guard = test_guard();
    reset_config(ThemeName::Dark);
    set_content_mode_order(&[ContentShowMode::Matrix]);
    assert_eq!(
        ordered_content_modes(&[ContentShowMode::Preview, ContentShowMode::Matrix]),
        vec![ContentShowMode::Matrix, ContentShowMode::Preview]
    );
    reset_config(ThemeName::Dark);
}

#[test]
fn config_generation_tracks_successful_mutations() {
    let _guard = test_guard();
    reset_config(ThemeName::Dark);
    let start = current_config_generation();
    set_content_mode_order(&[ContentShowMode::Heatmap, ContentShowMode::Preview]);
    let after_reorder = current_config_generation();
    assert!(after_reorder > start);

    let failed = set_color_override("bogus.color", Color::Blue);
    assert!(failed.is_err());
    assert_eq!(current_config_generation(), after_reorder);
}

#[test]
fn auto_layout_settings_round_trip() {
    let _guard = test_guard();
    reset_config(ThemeName::Dark);
    let custom = AutoLayoutSettings {
        tree: PanelLayoutSizes::new(LayoutSize::percent(32), LayoutSize::percent(18)),
        attributes: PanelLayoutSizes::new(LayoutSize::cells(14), LayoutSize::cells(6)),
        content: PanelLayoutSizes::new(LayoutSize::fill(), LayoutSize::fill()),
    };
    set_auto_layout_settings(&custom);
    assert_eq!(current_auto_layout_settings(), custom);
    reset_config(ThemeName::Dark);
}
