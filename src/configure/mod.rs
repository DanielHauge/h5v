use std::path::PathBuf;

use self::errors::ConfigureErrors;

pub mod colors;
pub mod errors;
mod loading;
mod lua;
mod presentation;
pub mod registry;
pub mod symbols;

pub(crate) use colors::themed_color;
#[allow(unused_imports)]
pub use colors::{
    available_color_names, available_theme_names, color_to_lua_string, current_theme_name,
    parse_color, prefers_strong_text, rgb_channels, theme_named_colors, ThemeName,
};
#[allow(unused_imports)]
pub use presentation::{
    apply_registry_snapshot, current_auto_layout_settings, current_config_generation,
    current_content_mode_order, current_content_mode_order_handles, current_heatmap_default_range,
    current_heatmap_default_settings, current_heatmap_range_modes, current_keymaps,
    current_multichart_settings, current_theme_handle, ordered_content_mode_handles,
    ordered_content_modes, reset_config, restore_config, set_auto_layout_settings,
    set_heatmap_ranges, set_keymap_config, snapshot_config, AutoLayoutSettings, ConfigSnapshot,
    LayoutSize, MultiChartSettings, PanelLayoutSizes,
};
#[cfg(test)]
pub use presentation::{set_color_override, set_content_mode_order};
pub(crate) use symbols::configured_symbol;
#[allow(unused_imports)]
pub use symbols::{
    available_symbol_names, available_symbol_theme_names, current_symbol_theme_name,
    theme_named_symbols, SymbolThemeName,
};

pub fn ensure_config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::ensure_config_exists()
}

pub fn config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::config_path()
}

pub fn set_config_path_override(path: Option<PathBuf>) -> Result<Option<PathBuf>, ConfigureErrors> {
    loading::set_config_path_override(path)
}

pub fn reset_config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::reset_config_to_default()
}

pub fn refresh_plugin_lua_ls_support_files(path: &std::path::Path) -> Result<(), ConfigureErrors> {
    loading::refresh_plugin_lua_ls_support_files(path)
}

#[cfg(test)]
pub(crate) use lua::reset_mchart_worker_runtime;
pub(crate) use lua::{
    available_lua_content_mode_handles, available_lua_content_mode_handles_for_item,
    build_app_context as build_lua_app_context, build_config_context as build_lua_config_context,
    build_fs_context as build_lua_fs_context, build_log_context as build_lua_log_context,
    build_log_context_with_handle as build_lua_log_context_with_handle,
    build_plugin_context as build_lua_plugin_context,
    build_plugin_fs_context as build_lua_plugin_fs_context,
    build_process_context as build_lua_process_context,
    build_selection_context as build_lua_selection_context, dispatch_lua_event,
    open_content_mode_target as open_lua_content_mode_target,
    parse_process_json_output as parse_lua_process_json, run_registered_mchart_function,
    set_lua_toast, LuaMchartArgValue, LuaMchartReturnValue, LuaToastLevel,
};
pub use lua::{
    last_config_load_metrics, load_config_compatibility, run_lua_engine, with_command_lua_callback,
    with_content_mode_lua_callback, with_keymap_lua_callback,
};
#[allow(unused_imports)]
pub use registry::{
    builtin_registry_builder, builtin_registry_snapshot, current_registry_snapshot,
    install_registry_snapshot, with_registry_snapshot, RegistryBuilder, RegistrySnapshot,
};
