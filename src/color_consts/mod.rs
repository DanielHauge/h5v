mod accessors;
mod catalog;
mod palette;
mod parsing;
mod state;
#[cfg(test)]
mod tests;
mod types;

#[allow(unused_imports)]
pub use accessors::{
    bg_color, bg_val1_color, bg_val2_color, bg_val3_color, bg_val4_color, bool_color, break_color,
    built_in_value_color, chart_axis_color, chart_grid_color, chart_label_color,
    chart_plot_bg_color, chart_preview_line_color, chart_series_color, command_no_match_color,
    command_usage_color, compound_color, compound_name_color, dataset_color, dataset_file_color,
    enum_series_color, equal_sign_color, error_color, file_color, focus_bg_color, group_color,
    help_key_bg_color, highlight_bg_color, highlight_bg_copy_color, image_border_color,
    key_hint_color, line_num_color, lines_color, load_more_color, meta_section_color, number_color,
    opaque_color, panel_border_color, panel_title_color, primary_text_color, root_file_color,
    search_count_color, search_highlight_color, search_icon_color, search_text_color,
    selected_dim_color, selected_index_color, selection_bg_color, selection_fg_color,
    status_compatibility_color, status_linked_color, status_readonly_color,
    status_update_available_color, status_writable_color, string_color, symbol_color,
    title_bg_color, title_color, toast_info_color, toast_neutral_color, toast_warning_color,
    type_desc_color, variable_blue_builtin_color, variable_blue_color,
};
#[allow(unused_imports)]
pub use catalog::{available_color_names, available_theme_names, theme_named_colors};
#[allow(unused_imports)]
pub use parsing::{color_to_lua_string, parse_color, rgb_channels};
#[allow(unused_imports)]
pub use state::{
    current_theme_name, prefers_strong_text, reset_theme, restore_theme, set_color_override,
    snapshot_theme,
};
#[allow(unused_imports)]
pub use types::{ThemeName, ThemeSnapshot};
