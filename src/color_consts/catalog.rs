use ratatui::prelude::Color;

use super::types::{ThemeColors, ThemeName};

const COLOR_NAMES: &[&str] = &[
    "accent.selected_index",
    "accent.selected_dim",
    "accent.equal_sign",
    "accent.symbol",
    "accent.selection_fg",
    "accent.selection_bg",
    "accent.search_highlight",
    "accent.search_icon",
    "text.title",
    "surface.title_bg",
    "text.meta_section",
    "text.primary",
    "text.built_in_value",
    "text.number",
    "text.string",
    "text.opaque",
    "text.bool",
    "text.error",
    "text.search_text",
    "text.search_count",
    "text.type_desc",
    "text.line_num",
    "text.command_usage",
    "text.key_hint",
    "text.command_no_match",
    "surface.focus_bg",
    "surface.bg",
    "surface.bg_val1",
    "surface.bg_val2",
    "surface.bg_val3",
    "surface.bg_val4",
    "surface.break_line",
    "surface.highlight_bg",
    "surface.highlight_bg_copy",
    "surface.panel_border",
    "surface.panel_title",
    "surface.help_key_bg",
    "surface.image_border",
    "tree.lines",
    "tree.root_file",
    "tree.variable",
    "tree.variable_builtin",
    "tree.file",
    "tree.group",
    "tree.compound_name",
    "tree.dataset",
    "tree.dataset_file",
    "tree.compound",
    "tree.load_more",
    "chart.axis",
    "chart.grid",
    "chart.label",
    "chart.preview_line",
    "chart.plot_bg",
    "chart.series_1",
    "chart.series_2",
    "chart.series_3",
    "chart.series_4",
    "chart.series_5",
    "chart.series_6",
    "chart.series_7",
    "chart.series_8",
    "chart.enum_series_1",
    "chart.enum_series_2",
    "chart.enum_series_3",
    "chart.enum_series_4",
    "chart.enum_series_5",
    "chart.enum_series_6",
    "chart.enum_series_7",
    "chart.enum_series_8",
    "status.readonly",
    "status.writable",
    "status.linked",
    "status.compatibility",
    "status.update_available",
    "status.toast_info",
    "status.toast_warning",
    "status.toast_neutral",
];

impl ThemeColors {
    pub(crate) fn named_color(&self, name: &str) -> Option<Color> {
        match normalize_color_name(name).as_str() {
            "accent.selected_index" | "selected_index" => Some(self.accent.selected_index),
            "accent.selected_dim" | "selected_dim" => Some(self.accent.selected_dim),
            "text.title" | "title" => Some(self.text.title),
            "surface.title_bg" | "title_bg" => Some(self.surface.title_bg),
            "text.meta_section" | "meta_section" => Some(self.text.meta_section),
            "text.primary" | "primary_text" => Some(self.text.primary),
            "text.number" | "number" => Some(self.text.number),
            "text.string" | "string" => Some(self.text.string),
            "text.opaque" | "opaque" => Some(self.text.opaque),
            "text.bool" | "bool" => Some(self.text.bool_value),
            "text.error" | "error" => Some(self.text.error),
            "tree.lines" | "lines" => Some(self.tree.lines),
            "tree.root_file" | "root_file" => Some(self.tree.root_file),
            "tree.variable" | "variable_blue" => Some(self.tree.variable),
            "tree.variable_builtin" | "variable_blue_builtin" => Some(self.tree.variable_builtin),
            "text.built_in_value" | "built_in_value" => Some(self.text.built_in_value),
            "surface.highlight_bg" | "highlight_bg" => Some(self.surface.highlight_bg),
            "surface.highlight_bg_copy" | "highlight_bg_copy" => {
                Some(self.surface.highlight_bg_copy)
            }
            "surface.focus_bg" | "focus_bg" => Some(self.surface.focus_bg),
            "surface.bg" | "bg" => Some(self.surface.bg),
            "surface.bg_val1" | "bg_val1" => Some(self.surface.bg_val1),
            "surface.bg_val2" | "bg_val2" => Some(self.surface.bg_val2),
            "surface.bg_val3" | "bg_val3" => Some(self.surface.bg_val3),
            "surface.bg_val4" | "bg_val4" => Some(self.surface.bg_val4),
            "surface.break_line" | "break_line" => Some(self.surface.break_line),
            "accent.equal_sign" | "equal_sign" => Some(self.accent.equal_sign),
            "accent.symbol" | "symbol" => Some(self.accent.symbol),
            "tree.file" | "file" => Some(self.tree.file),
            "tree.group" | "group" => Some(self.tree.group),
            "tree.compound_name" | "compound_name" => Some(self.tree.compound_name),
            "tree.dataset" | "dataset" => Some(self.tree.dataset),
            "tree.compound" | "compound" => Some(self.tree.compound),
            "text.search_text" | "search_text" => Some(self.text.search_text),
            "text.search_count" | "search_count" => Some(self.text.search_count),
            "tree.dataset_file" | "dataset_file" => Some(self.tree.dataset_file),
            "text.type_desc" | "type_desc" => Some(self.text.type_desc),
            "tree.load_more" | "load_more" => Some(self.tree.load_more),
            "text.line_num" | "line_num" => Some(self.text.line_num),
            "chart.axis" | "chart_axis" => Some(self.chart.chart_axis),
            "chart.grid" | "chart_grid" => Some(self.chart.chart_grid),
            "chart.label" | "chart_label" => Some(self.chart.chart_label),
            "chart.preview_line" | "chart_preview_line" => Some(self.chart.chart_preview_line),
            "chart.plot_bg" | "chart_plot_bg" => Some(self.chart.chart_plot_bg),
            "chart.series_1" | "chart_series_1" => Some(self.chart.chart_series[0]),
            "chart.series_2" | "chart_series_2" => Some(self.chart.chart_series[1]),
            "chart.series_3" | "chart_series_3" => Some(self.chart.chart_series[2]),
            "chart.series_4" | "chart_series_4" => Some(self.chart.chart_series[3]),
            "chart.series_5" | "chart_series_5" => Some(self.chart.chart_series[4]),
            "chart.series_6" | "chart_series_6" => Some(self.chart.chart_series[5]),
            "chart.series_7" | "chart_series_7" => Some(self.chart.chart_series[6]),
            "chart.series_8" | "chart_series_8" => Some(self.chart.chart_series[7]),
            "chart.enum_series_1" | "enum_series_1" => Some(self.chart.enum_series[0]),
            "chart.enum_series_2" | "enum_series_2" => Some(self.chart.enum_series[1]),
            "chart.enum_series_3" | "enum_series_3" => Some(self.chart.enum_series[2]),
            "chart.enum_series_4" | "enum_series_4" => Some(self.chart.enum_series[3]),
            "chart.enum_series_5" | "enum_series_5" => Some(self.chart.enum_series[4]),
            "chart.enum_series_6" | "enum_series_6" => Some(self.chart.enum_series[5]),
            "chart.enum_series_7" | "enum_series_7" => Some(self.chart.enum_series[6]),
            "chart.enum_series_8" | "enum_series_8" => Some(self.chart.enum_series[7]),
            "surface.panel_border" | "panel_border" => Some(self.surface.panel_border),
            "surface.panel_title" | "panel_title" => Some(self.surface.panel_title),
            "status.readonly" | "status_readonly" => Some(self.status.status_readonly),
            "status.writable" | "status_writable" => Some(self.status.status_writable),
            "status.linked" | "status_linked" => Some(self.status.status_linked),
            "status.compatibility" | "status_compatibility" => {
                Some(self.status.status_compatibility)
            }
            "status.update_available" | "status_update_available" => {
                Some(self.status.status_update_available)
            }
            "status.toast_info" | "toast_info" => Some(self.status.toast_info),
            "status.toast_warning" | "toast_warning" => Some(self.status.toast_warning),
            "status.toast_neutral" | "toast_neutral" => Some(self.status.toast_neutral),
            "accent.selection_fg" | "selection_fg" => Some(self.accent.selection_fg),
            "accent.selection_bg" | "selection_bg" => Some(self.accent.selection_bg),
            "text.command_usage" | "command_usage" => Some(self.text.command_usage),
            "text.key_hint" | "key_hint" => Some(self.text.key_hint),
            "text.command_no_match" | "command_no_match" => Some(self.text.command_no_match),
            "accent.search_highlight" | "search_highlight" => Some(self.accent.search_highlight),
            "accent.search_icon" | "search_icon" => Some(self.accent.search_icon),
            "surface.help_key_bg" | "help_key_bg" => Some(self.surface.help_key_bg),
            "surface.image_border" | "image_border" => Some(self.surface.image_border),
            _ => None,
        }
    }

    pub(crate) fn set_named_color(&mut self, name: &str, color: Color) -> bool {
        match normalize_color_name(name).as_str() {
            "accent.selected_index" | "selected_index" => self.accent.selected_index = color,
            "accent.selected_dim" | "selected_dim" => self.accent.selected_dim = color,
            "text.title" | "title" => self.text.title = color,
            "surface.title_bg" | "title_bg" => self.surface.title_bg = color,
            "text.meta_section" | "meta_section" => self.text.meta_section = color,
            "text.primary" | "primary_text" => self.text.primary = color,
            "text.number" | "number" => self.text.number = color,
            "text.string" | "string" => self.text.string = color,
            "text.opaque" | "opaque" => self.text.opaque = color,
            "text.bool" | "bool" => self.text.bool_value = color,
            "text.error" | "error" => self.text.error = color,
            "tree.lines" | "lines" => self.tree.lines = color,
            "tree.root_file" | "root_file" => self.tree.root_file = color,
            "tree.variable" | "variable_blue" => self.tree.variable = color,
            "tree.variable_builtin" | "variable_blue_builtin" => self.tree.variable_builtin = color,
            "text.built_in_value" | "built_in_value" => self.text.built_in_value = color,
            "surface.highlight_bg" | "highlight_bg" => self.surface.highlight_bg = color,
            "surface.highlight_bg_copy" | "highlight_bg_copy" => {
                self.surface.highlight_bg_copy = color
            }
            "surface.focus_bg" | "focus_bg" => self.surface.focus_bg = color,
            "surface.bg" | "bg" => self.surface.bg = color,
            "surface.bg_val1" | "bg_val1" => self.surface.bg_val1 = color,
            "surface.bg_val2" | "bg_val2" => self.surface.bg_val2 = color,
            "surface.bg_val3" | "bg_val3" => self.surface.bg_val3 = color,
            "surface.bg_val4" | "bg_val4" => self.surface.bg_val4 = color,
            "surface.break_line" | "break_line" => self.surface.break_line = color,
            "accent.equal_sign" | "equal_sign" => self.accent.equal_sign = color,
            "accent.symbol" | "symbol" => self.accent.symbol = color,
            "tree.file" | "file" => self.tree.file = color,
            "tree.group" | "group" => self.tree.group = color,
            "tree.compound_name" | "compound_name" => self.tree.compound_name = color,
            "tree.dataset" | "dataset" => self.tree.dataset = color,
            "tree.compound" | "compound" => self.tree.compound = color,
            "text.search_text" | "search_text" => self.text.search_text = color,
            "text.search_count" | "search_count" => self.text.search_count = color,
            "tree.dataset_file" | "dataset_file" => self.tree.dataset_file = color,
            "text.type_desc" | "type_desc" => self.text.type_desc = color,
            "tree.load_more" | "load_more" => self.tree.load_more = color,
            "text.line_num" | "line_num" => self.text.line_num = color,
            "chart.axis" | "chart_axis" => self.chart.chart_axis = color,
            "chart.grid" | "chart_grid" => self.chart.chart_grid = color,
            "chart.label" | "chart_label" => self.chart.chart_label = color,
            "chart.preview_line" | "chart_preview_line" => self.chart.chart_preview_line = color,
            "chart.plot_bg" | "chart_plot_bg" => self.chart.chart_plot_bg = color,
            "chart.series_1" | "chart_series_1" => self.chart.chart_series[0] = color,
            "chart.series_2" | "chart_series_2" => self.chart.chart_series[1] = color,
            "chart.series_3" | "chart_series_3" => self.chart.chart_series[2] = color,
            "chart.series_4" | "chart_series_4" => self.chart.chart_series[3] = color,
            "chart.series_5" | "chart_series_5" => self.chart.chart_series[4] = color,
            "chart.series_6" | "chart_series_6" => self.chart.chart_series[5] = color,
            "chart.series_7" | "chart_series_7" => self.chart.chart_series[6] = color,
            "chart.series_8" | "chart_series_8" => self.chart.chart_series[7] = color,
            "chart.enum_series_1" | "enum_series_1" => self.chart.enum_series[0] = color,
            "chart.enum_series_2" | "enum_series_2" => self.chart.enum_series[1] = color,
            "chart.enum_series_3" | "enum_series_3" => self.chart.enum_series[2] = color,
            "chart.enum_series_4" | "enum_series_4" => self.chart.enum_series[3] = color,
            "chart.enum_series_5" | "enum_series_5" => self.chart.enum_series[4] = color,
            "chart.enum_series_6" | "enum_series_6" => self.chart.enum_series[5] = color,
            "chart.enum_series_7" | "enum_series_7" => self.chart.enum_series[6] = color,
            "chart.enum_series_8" | "enum_series_8" => self.chart.enum_series[7] = color,
            "surface.panel_border" | "panel_border" => self.surface.panel_border = color,
            "surface.panel_title" | "panel_title" => self.surface.panel_title = color,
            "status.readonly" | "status_readonly" => self.status.status_readonly = color,
            "status.writable" | "status_writable" => self.status.status_writable = color,
            "status.linked" | "status_linked" => self.status.status_linked = color,
            "status.compatibility" | "status_compatibility" => {
                self.status.status_compatibility = color
            }
            "status.update_available" | "status_update_available" => {
                self.status.status_update_available = color
            }
            "status.toast_info" | "toast_info" => self.status.toast_info = color,
            "status.toast_warning" | "toast_warning" => self.status.toast_warning = color,
            "status.toast_neutral" | "toast_neutral" => self.status.toast_neutral = color,
            "accent.selection_fg" | "selection_fg" => self.accent.selection_fg = color,
            "accent.selection_bg" | "selection_bg" => self.accent.selection_bg = color,
            "text.command_usage" | "command_usage" => self.text.command_usage = color,
            "text.key_hint" | "key_hint" => self.text.key_hint = color,
            "text.command_no_match" | "command_no_match" => self.text.command_no_match = color,
            "accent.search_highlight" | "search_highlight" => self.accent.search_highlight = color,
            "accent.search_icon" | "search_icon" => self.accent.search_icon = color,
            "surface.help_key_bg" | "help_key_bg" => self.surface.help_key_bg = color,
            "surface.image_border" | "image_border" => self.surface.image_border = color,
            _ => return false,
        }
        true
    }
}

pub fn available_theme_names() -> &'static [&'static str] {
    &["dark", "light"]
}

pub fn available_color_names() -> &'static [&'static str] {
    COLOR_NAMES
}

pub fn theme_named_colors(theme: ThemeName) -> Vec<(&'static str, Color)> {
    let colors = ThemeColors::for_theme(theme);
    COLOR_NAMES
        .iter()
        .filter_map(|name| colors.named_color(name).map(|color| (*name, color)))
        .collect()
}

fn normalize_color_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    let mut last_was_separator = false;
    for ch in name.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else if ch == '.' {
            '.'
        } else {
            '_'
        };
        if mapped == '_' || mapped == '.' {
            if !(last_was_separator && normalized.ends_with(mapped)) {
                normalized.push(mapped);
            }
            last_was_separator = true;
        } else {
            normalized.push(mapped);
            last_was_separator = false;
        }
    }
    normalized
        .trim_matches(|c| c == '_' || c == '.')
        .to_string()
}
