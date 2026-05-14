use std::path::PathBuf;

use self::errors::ConfigureErrors;

pub mod colors;
pub mod errors;
mod loading;
mod lua;
mod presentation;
pub mod symbols;

pub(crate) use colors::themed_color;
#[allow(unused_imports)]
pub use colors::{
    available_color_names, available_theme_names, color_to_lua_string, current_theme_name,
    parse_color, prefers_strong_text, rgb_channels, set_color_override, theme_named_colors,
    ThemeName,
};
#[allow(unused_imports)]
pub use presentation::reset_symbol_theme;
#[allow(unused_imports)]
pub use presentation::{
    current_config_generation, current_content_mode_order, current_heatmap_default_range,
    current_heatmap_default_settings, current_heatmap_range_modes, current_keymaps,
    ordered_content_modes, reset_config, restore_config, set_content_mode_order,
    set_heatmap_default_settings, set_heatmap_ranges, set_keymap_config, snapshot_config,
    ConfigSnapshot,
};
pub(crate) use symbols::configured_symbol;
#[allow(unused_imports)]
pub use symbols::{
    available_symbol_names, available_symbol_theme_names, current_symbol_theme_name,
    set_symbol_override, theme_named_symbols, SymbolThemeName,
};

pub fn ensure_config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::ensure_config_exists()
}

pub fn config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::config_path()
}

pub fn reset_config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::reset_config_to_default()
}

pub use lua::{load_config_compatibility, run_lua_engine, with_keymap_lua_callback};
