mod catalog;
mod palette;
mod parsing;
mod state;
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests;
mod types;

#[allow(unused_imports)]
pub use catalog::{
    available_color_names, available_symbol_names, available_symbol_theme_names,
    available_theme_names, theme_named_colors, theme_named_symbols,
};
pub use palette::{SymbolThemeName, ThemeName};
#[allow(unused_imports)]
pub use parsing::{color_to_lua_string, parse_color, rgb_channels};
pub(crate) use state::configured_symbol;
pub(crate) use state::themed_color;
#[allow(unused_imports)]
pub use state::{
    apply_registry_snapshot, current_auto_layout_settings, current_config_generation,
    current_content_mode_order, current_content_mode_order_handles, current_heatmap_default_range,
    current_heatmap_default_settings, current_heatmap_range_modes, current_keymaps,
    current_multichart_settings, current_symbol_theme_name, current_theme_handle,
    current_theme_name, ordered_content_mode_handles, ordered_content_modes, prefers_strong_text,
    reset_config, restore_config, set_auto_layout_settings, set_heatmap_ranges, set_keymap_config,
    snapshot_config,
};
#[cfg(test)]
pub use state::{set_color_override, set_content_mode_order};
#[allow(unused_imports)]
pub use types::{
    AutoLayoutSettings, ConfigSnapshot, LayoutSize, MultiChartSettings, PanelLayoutSizes,
};
