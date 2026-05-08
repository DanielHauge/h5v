use ratatui::prelude::Color;

use crate::compat;

use super::state::{compat_color, themed_color};

pub fn selected_index_color() -> Color {
    themed_color(|colors| colors.accent.selected_index)
}

pub fn selected_dim_color() -> Color {
    themed_color(|colors| colors.accent.selected_dim)
}

pub fn title_color() -> Color {
    compat_color(themed_color(|colors| colors.text.title), Color::Yellow)
}

pub fn title_bg_color() -> Color {
    themed_color(|colors| colors.surface.title_bg)
}

pub fn meta_section_color() -> Color {
    compat_color(
        themed_color(|colors| colors.text.meta_section),
        Color::Yellow,
    )
}

pub fn primary_text_color() -> Color {
    themed_color(|colors| colors.text.primary)
}

pub fn number_color() -> Color {
    themed_color(|colors| colors.text.number)
}

pub fn string_color() -> Color {
    themed_color(|colors| colors.text.string)
}

pub fn opaque_color() -> Color {
    themed_color(|colors| colors.text.opaque)
}

pub fn bool_color() -> Color {
    themed_color(|colors| colors.text.bool_value)
}

pub fn error_color() -> Color {
    themed_color(|colors| colors.text.error)
}

pub fn lines_color() -> Color {
    compat_color(themed_color(|colors| colors.tree.lines), Color::DarkGray)
}

pub fn root_file_color() -> Color {
    compat_color(themed_color(|colors| colors.tree.root_file), Color::Cyan)
}

pub fn variable_blue_color() -> Color {
    compat_color(themed_color(|colors| colors.tree.variable), Color::Cyan)
}

pub fn variable_blue_builtin_color() -> Color {
    compat_color(
        themed_color(|colors| colors.tree.variable_builtin),
        Color::Blue,
    )
}

pub fn built_in_value_color() -> Color {
    compat_color(
        themed_color(|colors| colors.text.built_in_value),
        Color::White,
    )
}

pub fn highlight_bg_color() -> Color {
    themed_color(|colors| colors.surface.highlight_bg)
}

pub fn highlight_bg_copy_color() -> Color {
    themed_color(|colors| colors.surface.highlight_bg_copy)
}

pub fn focus_bg_color() -> Color {
    themed_color(|colors| colors.surface.focus_bg)
}

pub fn bg_color() -> Color {
    themed_color(|colors| colors.surface.bg)
}

pub fn bg_val1_color() -> Color {
    themed_color(|colors| colors.surface.bg_val1)
}

pub fn bg_val2_color() -> Color {
    themed_color(|colors| colors.surface.bg_val2)
}

pub fn bg_val3_color() -> Color {
    themed_color(|colors| colors.surface.bg_val3)
}

pub fn bg_val4_color() -> Color {
    themed_color(|colors| colors.surface.bg_val4)
}

pub fn break_color() -> Color {
    themed_color(|colors| colors.surface.break_line)
}

pub fn equal_sign_color() -> Color {
    compat_color(themed_color(|colors| colors.accent.equal_sign), Color::Blue)
}

pub fn symbol_color() -> Color {
    compat_color(themed_color(|colors| colors.accent.symbol), Color::Yellow)
}

pub fn file_color() -> Color {
    compat_color(themed_color(|colors| colors.tree.file), Color::Blue)
}

pub fn group_color() -> Color {
    compat_color(themed_color(|colors| colors.tree.group), Color::Yellow)
}

pub fn compound_name_color() -> Color {
    compat_color(
        themed_color(|colors| colors.tree.compound_name),
        Color::Yellow,
    )
}

pub fn dataset_color() -> Color {
    compat_color(themed_color(|colors| colors.tree.dataset), Color::White)
}

pub fn compound_color() -> Color {
    compat_color(themed_color(|colors| colors.tree.compound), Color::Magenta)
}

pub fn search_text_color() -> Color {
    themed_color(|colors| colors.text.search_text)
}

pub fn search_count_color() -> Color {
    themed_color(|colors| colors.text.search_count)
}

pub fn dataset_file_color() -> Color {
    compat_color(
        themed_color(|colors| colors.tree.dataset_file),
        Color::Green,
    )
}

pub fn type_desc_color() -> Color {
    themed_color(|colors| colors.text.type_desc)
}

pub fn load_more_color() -> Color {
    compat_color(themed_color(|colors| colors.tree.load_more), Color::Yellow)
}

pub fn line_num_color() -> Color {
    themed_color(|colors| colors.text.line_num)
}

pub fn chart_axis_color() -> Color {
    themed_color(|colors| colors.chart.chart_axis)
}

pub fn chart_grid_color() -> Color {
    themed_color(|colors| colors.chart.chart_grid)
}

pub fn chart_label_color() -> Color {
    themed_color(|colors| colors.chart.chart_label)
}

pub fn chart_preview_line_color() -> Color {
    themed_color(|colors| colors.chart.chart_preview_line)
}

pub fn chart_plot_bg_color() -> Color {
    themed_color(|colors| colors.chart.chart_plot_bg)
}

pub fn chart_series_color(slot: usize) -> Color {
    if compat::current().compatibility_mode {
        const COMPAT_CHART_SERIES_COLORS: [Color; 8] = [
            Color::Blue,
            Color::Green,
            Color::Yellow,
            Color::Magenta,
            Color::Red,
            Color::Cyan,
            Color::White,
            Color::DarkGray,
        ];
        COMPAT_CHART_SERIES_COLORS[slot % COMPAT_CHART_SERIES_COLORS.len()]
    } else {
        themed_color(|colors| colors.chart.chart_series[slot % colors.chart.chart_series.len()])
    }
}

pub fn enum_series_color(slot: usize) -> Color {
    themed_color(|colors| colors.chart.enum_series[slot % colors.chart.enum_series.len()])
}

pub fn panel_border_color() -> Color {
    themed_color(|colors| colors.surface.panel_border)
}

pub fn panel_title_color() -> Color {
    themed_color(|colors| colors.surface.panel_title)
}

pub fn status_readonly_color() -> Color {
    themed_color(|colors| colors.status.status_readonly)
}

pub fn status_writable_color() -> Color {
    themed_color(|colors| colors.status.status_writable)
}

pub fn status_linked_color() -> Color {
    themed_color(|colors| colors.status.status_linked)
}

pub fn status_compatibility_color() -> Color {
    themed_color(|colors| colors.status.status_compatibility)
}

pub fn status_update_available_color() -> Color {
    themed_color(|colors| colors.status.status_update_available)
}

pub fn toast_info_color() -> Color {
    themed_color(|colors| colors.status.toast_info)
}

pub fn toast_warning_color() -> Color {
    themed_color(|colors| colors.status.toast_warning)
}

pub fn toast_neutral_color() -> Color {
    themed_color(|colors| colors.status.toast_neutral)
}

pub fn selection_fg_color() -> Color {
    themed_color(|colors| colors.accent.selection_fg)
}

pub fn selection_bg_color() -> Color {
    themed_color(|colors| colors.accent.selection_bg)
}

pub fn command_usage_color() -> Color {
    themed_color(|colors| colors.text.command_usage)
}

pub fn key_hint_color() -> Color {
    themed_color(|colors| colors.text.key_hint)
}

pub fn command_no_match_color() -> Color {
    themed_color(|colors| colors.text.command_no_match)
}

pub fn search_highlight_color() -> Color {
    themed_color(|colors| colors.accent.search_highlight)
}

pub fn search_icon_color() -> Color {
    themed_color(|colors| colors.accent.search_icon)
}

pub fn help_key_bg_color() -> Color {
    themed_color(|colors| colors.surface.help_key_bg)
}

pub fn image_border_color() -> Color {
    themed_color(|colors| colors.surface.image_border)
}
