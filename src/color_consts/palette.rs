use ratatui::prelude::Color;

use super::types::{
    AccentColors, ChartColors, StatusColors, SurfaceColors, TextColors, ThemeColors, ThemeName,
    TreeColors,
};

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
    pub(crate) fn for_theme(theme: ThemeName) -> Self {
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
}
