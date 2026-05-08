use std::sync::{LazyLock, RwLock};

use ratatui::prelude::Color;

use crate::compat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeName {
    Dark,
    Light,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextColors {
    title: Color,
    meta_section: Color,
    primary: Color,
    built_in_value: Color,
    number: Color,
    string: Color,
    opaque: Color,
    bool_value: Color,
    error: Color,
    search_text: Color,
    search_count: Color,
    type_desc: Color,
    line_num: Color,
    command_usage: Color,
    key_hint: Color,
    command_no_match: Color,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceColors {
    title_bg: Color,
    focus_bg: Color,
    bg: Color,
    bg_val1: Color,
    bg_val2: Color,
    bg_val3: Color,
    bg_val4: Color,
    break_line: Color,
    highlight_bg: Color,
    highlight_bg_copy: Color,
    panel_border: Color,
    panel_title: Color,
    help_key_bg: Color,
    image_border: Color,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeColors {
    lines: Color,
    root_file: Color,
    variable: Color,
    variable_builtin: Color,
    file: Color,
    group: Color,
    compound_name: Color,
    dataset: Color,
    dataset_file: Color,
    compound: Color,
    load_more: Color,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccentColors {
    selected_index: Color,
    selected_dim: Color,
    equal_sign: Color,
    symbol: Color,
    selection_fg: Color,
    selection_bg: Color,
    search_highlight: Color,
    search_icon: Color,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartColors {
    chart_axis: Color,
    chart_grid: Color,
    chart_label: Color,
    chart_preview_line: Color,
    chart_plot_bg: Color,
    chart_series: [Color; 8],
    enum_series: [Color; 8],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusColors {
    status_readonly: Color,
    status_writable: Color,
    status_linked: Color,
    status_compatibility: Color,
    status_update_available: Color,
    toast_info: Color,
    toast_warning: Color,
    toast_neutral: Color,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeColors {
    text: TextColors,
    surface: SurfaceColors,
    tree: TreeColors,
    accent: AccentColors,
    chart: ChartColors,
    status: StatusColors,
}

#[derive(Clone, Debug)]
struct ThemeState {
    active_theme: ThemeName,
    colors: ThemeColors,
}

#[derive(Clone, Debug)]
pub struct ThemeSnapshot {
    active_theme: ThemeName,
    colors: ThemeColors,
}

static THEME_STATE: LazyLock<RwLock<ThemeState>> = LazyLock::new(|| {
    RwLock::new(ThemeState {
        active_theme: ThemeName::Dark,
        colors: ThemeColors::for_theme(ThemeName::Dark),
    })
});

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

impl ThemeName {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

impl ThemeColors {
    pub fn for_theme(theme: ThemeName) -> Self {
        match theme {
            ThemeName::Dark => Self::dark(),
            ThemeName::Light => Self::light(),
        }
    }

    fn dark() -> Self {
        Self {
            text: TextColors {
                title: Color::Yellow,
                meta_section: Color::Rgb(214, 190, 110),
                primary: Color::White,
                built_in_value: Color::Rgb(222, 222, 222),
                number: Color::Rgb(181, 206, 168),
                string: Color::Rgb(206, 145, 120),
                opaque: Color::Rgb(198, 160, 255),
                bool_value: Color::Rgb(255, 204, 0),
                error: Color::Rgb(255, 0, 0),
                search_text: Color::Rgb(222, 222, 222),
                search_count: Color::DarkGray,
                type_desc: Color::Rgb(150, 150, 150),
                line_num: Color::DarkGray,
                command_usage: Color::Cyan,
                key_hint: Color::Yellow,
                command_no_match: Color::DarkGray,
            },
            surface: SurfaceColors {
                title_bg: Color::Rgb(83, 86, 89),
                focus_bg: Color::Rgb(41, 42, 45),
                bg: Color::Rgb(61, 62, 65),
                bg_val1: Color::Rgb(55, 55, 60),
                bg_val2: Color::Rgb(45, 50, 45),
                bg_val3: Color::Rgb(45, 46, 50),
                bg_val4: Color::Rgb(55, 56, 60),
                break_line: Color::Rgb(83, 86, 89),
                highlight_bg: Color::Rgb(55, 62, 70),
                highlight_bg_copy: Color::Rgb(255, 153, 0),
                panel_border: Color::Green,
                panel_title: Color::Yellow,
                help_key_bg: Color::Rgb(60, 90, 120),
                image_border: Color::DarkGray,
            },
            tree: TreeColors {
                lines: Color::Rgb(83, 86, 89),
                root_file: Color::Rgb(186, 230, 250),
                variable: Color::Rgb(136, 200, 230),
                variable_builtin: Color::Rgb(66, 165, 245),
                file: Color::Rgb(66, 165, 245),
                group: Color::Rgb(255, 204, 0),
                compound_name: Color::Rgb(214, 170, 0),
                dataset: Color::Rgb(222, 222, 222),
                dataset_file: Color::Rgb(38, 166, 154),
                compound: Color::Rgb(200, 140, 255),
                load_more: Color::Yellow,
            },
            accent: AccentColors {
                selected_index: Color::Cyan,
                selected_dim: Color::Yellow,
                equal_sign: Color::Rgb(66, 165, 245),
                symbol: Color::Rgb(255, 225, 0),
                selection_fg: Color::Black,
                selection_bg: Color::Yellow,
                search_highlight: Color::Yellow,
                search_icon: Color::LightYellow,
            },
            chart: ChartColors {
                chart_axis: Color::Rgb(222, 222, 222),
                chart_grid: Color::Rgb(83, 86, 89),
                chart_label: Color::Rgb(150, 150, 150),
                chart_preview_line: Color::Rgb(66, 165, 245),
                chart_plot_bg: Color::Rgb(41, 42, 45),
                chart_series: [
                    Color::Rgb(66, 165, 245),
                    Color::Rgb(38, 166, 154),
                    Color::Rgb(255, 204, 0),
                    Color::Rgb(200, 140, 255),
                    Color::Rgb(206, 145, 120),
                    Color::Rgb(255, 204, 0),
                    Color::Rgb(186, 230, 250),
                    Color::Rgb(129, 199, 132),
                ],
                enum_series: [
                    Color::Rgb(255, 204, 0),
                    Color::Rgb(38, 166, 154),
                    Color::Rgb(66, 165, 245),
                    Color::Rgb(200, 140, 255),
                    Color::Rgb(255, 112, 67),
                    Color::Rgb(181, 206, 168),
                    Color::Rgb(240, 98, 146),
                    Color::Rgb(129, 199, 132),
                ],
            },
            status: StatusColors {
                status_readonly: Color::Yellow,
                status_writable: Color::LightGreen,
                status_linked: Color::Cyan,
                status_compatibility: Color::Magenta,
                status_update_available: Color::Yellow,
                toast_info: Color::LightGreen,
                toast_warning: Color::Yellow,
                toast_neutral: Color::White,
            },
        }
    }

    fn light() -> Self {
        Self {
            text: TextColors {
                title: Color::Rgb(96, 48, 0),
                meta_section: Color::Rgb(110, 65, 0),
                primary: Color::Rgb(5, 5, 8),
                built_in_value: Color::Rgb(5, 5, 8),
                number: Color::Rgb(0, 90, 30),
                string: Color::Rgb(140, 30, 0),
                opaque: Color::Rgb(90, 35, 195),
                bool_value: Color::Rgb(120, 65, 0),
                error: Color::Rgb(180, 10, 20),
                search_text: Color::Rgb(5, 5, 8),
                search_count: Color::Rgb(40, 45, 55),
                type_desc: Color::Rgb(40, 45, 55),
                line_num: Color::Rgb(40, 45, 55),
                command_usage: Color::Rgb(0, 65, 170),
                key_hint: Color::Rgb(96, 48, 0),
                command_no_match: Color::Rgb(40, 45, 55),
            },
            surface: SurfaceColors {
                title_bg: Color::Rgb(188, 180, 166),
                focus_bg: Color::Rgb(210, 210, 205),
                bg: Color::Rgb(225, 224, 220),
                bg_val1: Color::Rgb(232, 231, 227),
                bg_val2: Color::Rgb(220, 226, 218),
                bg_val3: Color::Rgb(218, 222, 228),
                bg_val4: Color::Rgb(210, 216, 224),
                break_line: Color::Rgb(80, 77, 72),
                highlight_bg: Color::Rgb(225, 200, 120),
                highlight_bg_copy: Color::Rgb(220, 110, 0),
                panel_border: Color::Rgb(30, 55, 85),
                panel_title: Color::Rgb(96, 48, 0),
                help_key_bg: Color::Rgb(175, 195, 220),
                image_border: Color::Rgb(65, 62, 58),
            },
            tree: TreeColors {
                lines: Color::Rgb(80, 77, 72),
                root_file: Color::Rgb(0, 88, 215),
                variable: Color::Rgb(0, 70, 195),
                variable_builtin: Color::Rgb(0, 88, 215),
                file: Color::Rgb(0, 88, 215),
                group: Color::Rgb(210, 108, 0),
                compound_name: Color::Rgb(108, 58, 0),
                dataset: Color::Rgb(5, 5, 8),
                dataset_file: Color::Rgb(0, 142, 88),
                compound: Color::Rgb(124, 52, 235),
                load_more: Color::Rgb(96, 48, 0),
            },
            accent: AccentColors {
                selected_index: Color::Rgb(0, 58, 150),
                selected_dim: Color::Rgb(120, 65, 0),
                equal_sign: Color::Rgb(0, 88, 215),
                symbol: Color::Rgb(45, 25, 0),
                selection_fg: Color::Rgb(5, 5, 8),
                selection_bg: Color::Rgb(225, 200, 120),
                search_highlight: Color::Rgb(120, 65, 0),
                search_icon: Color::Rgb(108, 58, 0),
            },
            chart: ChartColors {
                chart_axis: Color::Rgb(5, 5, 8),
                chart_grid: Color::Rgb(145, 142, 136),
                chart_label: Color::Rgb(40, 45, 55),
                chart_preview_line: Color::Rgb(0, 65, 170),
                chart_plot_bg: Color::Rgb(210, 210, 205),
                chart_series: [
                    Color::Rgb(0, 65, 170),
                    Color::Rgb(0, 88, 62),
                    Color::Rgb(120, 65, 0),
                    Color::Rgb(90, 35, 195),
                    Color::Rgb(140, 30, 0),
                    Color::Rgb(0, 90, 30),
                    Color::Rgb(25, 55, 130),
                    Color::Rgb(0, 110, 90),
                ],
                enum_series: [
                    Color::Rgb(120, 65, 0),
                    Color::Rgb(0, 88, 62),
                    Color::Rgb(0, 65, 170),
                    Color::Rgb(90, 35, 195),
                    Color::Rgb(150, 40, 0),
                    Color::Rgb(0, 90, 30),
                    Color::Rgb(145, 20, 100),
                    Color::Rgb(45, 85, 0),
                ],
            },
            status: StatusColors {
                status_readonly: Color::Rgb(96, 48, 0),
                status_writable: Color::Rgb(0, 90, 30),
                status_linked: Color::Rgb(0, 65, 170),
                status_compatibility: Color::Rgb(90, 35, 195),
                status_update_available: Color::Rgb(96, 48, 0),
                toast_info: Color::Rgb(0, 90, 30),
                toast_warning: Color::Rgb(96, 48, 0),
                toast_neutral: Color::Rgb(5, 5, 8),
            },
        }
    }

    pub fn named_color(&self, name: &str) -> Option<Color> {
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

    pub fn set_named_color(&mut self, name: &str, color: Color) -> bool {
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

pub fn reset_theme(theme: ThemeName) {
    with_theme_write(|state| {
        state.active_theme = theme;
        state.colors = ThemeColors::for_theme(theme);
    });
}

pub fn snapshot_theme() -> ThemeSnapshot {
    with_theme_read(|state| ThemeSnapshot {
        active_theme: state.active_theme,
        colors: state.colors.clone(),
    })
}

pub fn restore_theme(snapshot: ThemeSnapshot) {
    with_theme_write(|state| {
        state.active_theme = snapshot.active_theme;
        state.colors = snapshot.colors;
    });
}

pub fn set_color_override(name: &str, color: Color) -> Result<(), String> {
    with_theme_write(|state| {
        if state.colors.set_named_color(name, color) {
            Ok(())
        } else {
            Err(format!(
                "Unknown color '{name}'. Available colors: {}",
                available_color_names().join(", ")
            ))
        }
    })
}

pub fn current_theme_name() -> ThemeName {
    with_theme_read(|state| state.active_theme)
}

pub fn prefers_strong_text() -> bool {
    matches!(current_theme_name(), ThemeName::Light)
}

pub fn parse_color(value: &str) -> Option<Color> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.to_ascii_lowercase();
    if let Some(hex) = normalized.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
    }

    match normalized.as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" | "purple" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" | "dark_gray" | "dark_grey" => Some(Color::DarkGray),
        "lightred" | "light_red" | "pink" => Some(Color::LightRed),
        "lightgreen" | "light_green" => Some(Color::LightGreen),
        "lightyellow" | "light_yellow" => Some(Color::LightYellow),
        "lightblue" | "light_blue" => Some(Color::LightBlue),
        "lightmagenta" | "light_magenta" => Some(Color::LightMagenta),
        "lightcyan" | "light_cyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        "amber" => Some(Color::Rgb(255, 191, 0)),
        "orange" => Some(Color::Rgb(255, 165, 0)),
        _ => None,
    }
}

pub fn color_to_lua_string(color: Color) -> String {
    match color {
        Color::Black => "black".to_string(),
        Color::Red => "red".to_string(),
        Color::Green => "green".to_string(),
        Color::Yellow => "yellow".to_string(),
        Color::Blue => "blue".to_string(),
        Color::Magenta => "magenta".to_string(),
        Color::Cyan => "cyan".to_string(),
        Color::Gray => "gray".to_string(),
        Color::DarkGray => "darkgray".to_string(),
        Color::LightRed => "lightred".to_string(),
        Color::LightGreen => "lightgreen".to_string(),
        Color::LightYellow => "lightyellow".to_string(),
        Color::LightBlue => "lightblue".to_string(),
        Color::LightMagenta => "lightmagenta".to_string(),
        Color::LightCyan => "lightcyan".to_string(),
        Color::White => "white".to_string(),
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        other => format!("{other:?}"),
    }
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

fn with_theme_read<R>(f: impl FnOnce(&ThemeState) -> R) -> R {
    let guard = match THEME_STATE.read() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    f(&guard)
}

fn with_theme_write<R>(f: impl FnOnce(&mut ThemeState) -> R) -> R {
    let mut guard = match THEME_STATE.write() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    f(&mut guard)
}

fn themed_color(getter: impl FnOnce(&ThemeColors) -> Color) -> Color {
    with_theme_read(|state| getter(&state.colors))
}

fn compat_color(rich: Color, fallback: Color) -> Color {
    if compat::current().compatibility_mode {
        fallback
    } else {
        rich
    }
}

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

pub fn rgb_channels(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Black => (0, 0, 0),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 128, 0),
        Color::Yellow => (255, 255, 0),
        Color::Blue => (0, 0, 255),
        Color::Magenta => (255, 0, 255),
        Color::Cyan => (0, 255, 255),
        Color::Gray => (128, 128, 128),
        Color::DarkGray => (169, 169, 169),
        Color::LightRed => (255, 102, 102),
        Color::LightGreen => (144, 238, 144),
        Color::LightYellow => (255, 255, 224),
        Color::LightBlue => (173, 216, 230),
        Color::LightMagenta => (238, 130, 238),
        Color::LightCyan => (224, 255, 255),
        Color::White => (255, 255, 255),
        Color::Rgb(r, g, b) => (r, g, b),
        other => {
            let fallback = format!("{other:?}");
            if fallback == "Reset" {
                (255, 255, 255)
            } else {
                (200, 200, 200)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_color, set_color_override, theme_named_colors, ThemeName};
    use crate::color_consts::{
        available_color_names, color_to_lua_string, current_theme_name, reset_theme, title_color,
    };
    use ratatui::style::Color;

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
}
