use ratatui::prelude::Color;

use crate::color_consts::types::ToastColors;

use super::types::{
    AccentColors, ChartColors, StatusColors, SurfaceColors, TextColors, ThemeColors, TreeColors,
};

impl ThemeName {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "light_blue" | "lightblue" | "windows" => Some(Self::LightBlue),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::LightBlue => "light_blue",
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ThemeName {
    Dark,
    Light,
    LightBlue,
}

impl ThemeColors {
    pub(crate) fn for_theme(theme: ThemeName) -> Self {
        match theme {
            ThemeName::Dark => Self::dark(),
            ThemeName::Light => Self::light(),
            ThemeName::LightBlue => Self::light_blue(),
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
                axis: Color::Rgb(222, 222, 222),
                grid: Color::Rgb(83, 86, 89),
                label: Color::Rgb(150, 150, 150),
                preview_line: Color::Rgb(66, 165, 245),
                plot_bg: Color::Rgb(41, 42, 45),
                series: [
                    Color::Rgb(66, 165, 245),
                    Color::Rgb(38, 166, 154),
                    Color::Rgb(255, 204, 0),
                    Color::Rgb(200, 140, 255),
                    Color::Rgb(206, 145, 120),
                    Color::Rgb(255, 204, 0),
                    Color::Rgb(186, 230, 250),
                    Color::Rgb(129, 199, 132),
                ],
                r#enum: [
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
                readonly: Color::Yellow,
                writable: Color::LightGreen,
                linked: Color::Cyan,
                update_available: Color::Yellow,
                compability: Color::Magenta,
            },
            toast: ToastColors {
                info: Color::LightGreen,
                warning: Color::Yellow,
                neutral: Color::White,
            },
        }
    }

    fn light() -> Self {
        Self {
            text: TextColors {
                title: Color::Rgb(30, 58, 95),
                meta_section: Color::Rgb(40, 70, 110),
                primary: Color::Rgb(26, 26, 26),
                built_in_value: Color::Rgb(30, 30, 30),
                number: Color::Rgb(14, 124, 58),
                string: Color::Rgb(168, 41, 14),
                opaque: Color::Rgb(124, 58, 237),
                bool_value: Color::Rgb(0, 100, 180),
                error: Color::Rgb(200, 20, 20),
                search_text: Color::Rgb(26, 26, 26),
                search_count: Color::Rgb(90, 95, 110),
                type_desc: Color::Rgb(90, 95, 110),
                line_num: Color::Rgb(140, 145, 158),
                command_usage: Color::Rgb(0, 90, 190),
                key_hint: Color::Rgb(30, 58, 95),
                command_no_match: Color::Rgb(140, 145, 158),
            },
            surface: SurfaceColors {
                title_bg: Color::Rgb(30, 58, 95),
                focus_bg: Color::Rgb(228, 235, 245),
                bg: Color::Rgb(255, 255, 255),
                bg_val1: Color::Rgb(242, 244, 247),
                bg_val2: Color::Rgb(237, 242, 237),
                bg_val3: Color::Rgb(237, 240, 245),
                bg_val4: Color::Rgb(242, 244, 248),
                break_line: Color::Rgb(180, 185, 195),
                highlight_bg: Color::Rgb(210, 228, 255),
                highlight_bg_copy: Color::Rgb(255, 165, 0),
                panel_border: Color::Rgb(30, 58, 95),
                panel_title: Color::Rgb(30, 58, 95),
                help_key_bg: Color::Rgb(200, 218, 240),
                image_border: Color::Rgb(160, 165, 175),
            },
            tree: TreeColors {
                lines: Color::Rgb(180, 185, 195),
                root_file: Color::Rgb(0, 80, 200),
                variable: Color::Rgb(0, 100, 225),
                variable_builtin: Color::Rgb(0, 80, 200),
                file: Color::Rgb(0, 80, 200),
                group: Color::Rgb(180, 90, 0),
                compound_name: Color::Rgb(100, 55, 0),
                dataset: Color::Rgb(26, 26, 26),
                dataset_file: Color::Rgb(0, 128, 80),
                compound: Color::Rgb(110, 45, 210),
                load_more: Color::Rgb(0, 90, 190),
            },
            accent: AccentColors {
                selected_index: Color::Rgb(0, 90, 190),
                selected_dim: Color::Rgb(100, 120, 160),
                equal_sign: Color::Rgb(0, 80, 200),
                symbol: Color::Rgb(30, 58, 95),
                selection_fg: Color::Rgb(26, 26, 26),
                selection_bg: Color::Rgb(180, 210, 255),
                search_highlight: Color::Rgb(255, 200, 0),
                search_icon: Color::Rgb(0, 90, 190),
            },
            chart: ChartColors {
                axis: Color::Rgb(26, 26, 26),
                grid: Color::Rgb(200, 205, 215),
                label: Color::Rgb(90, 95, 110),
                preview_line: Color::Rgb(0, 90, 190),
                plot_bg: Color::Rgb(238, 242, 248),
                series: [
                    Color::Rgb(0, 90, 190),
                    Color::Rgb(14, 124, 58),
                    Color::Rgb(180, 90, 0),
                    Color::Rgb(110, 45, 210),
                    Color::Rgb(168, 41, 14),
                    Color::Rgb(0, 150, 136),
                    Color::Rgb(25, 60, 140),
                    Color::Rgb(0, 110, 85),
                ],
                r#enum: [
                    Color::Rgb(180, 90, 0),
                    Color::Rgb(14, 124, 58),
                    Color::Rgb(0, 90, 190),
                    Color::Rgb(110, 45, 210),
                    Color::Rgb(168, 41, 14),
                    Color::Rgb(0, 128, 80),
                    Color::Rgb(140, 20, 100),
                    Color::Rgb(40, 85, 0),
                ],
            },
            status: StatusColors {
                readonly: Color::Rgb(160, 80, 0),
                writable: Color::Rgb(14, 124, 58),
                linked: Color::Rgb(0, 90, 190),
                compability: Color::Rgb(110, 45, 210),
                update_available: Color::Rgb(160, 80, 0),
            },
            toast: ToastColors {
                info: Color::Rgb(14, 124, 58),
                warning: Color::Rgb(160, 80, 0),
                neutral: Color::Rgb(26, 26, 26),
            },
        }
    }

    fn light_blue() -> Self {
        Self {
            text: TextColors {
                title: Color::Rgb(0, 56, 117),
                meta_section: Color::Rgb(0, 70, 140),
                primary: Color::Rgb(26, 26, 26),
                built_in_value: Color::Rgb(0, 56, 117),
                number: Color::Rgb(9, 134, 88),
                string: Color::Rgb(163, 21, 21),
                opaque: Color::Rgb(136, 0, 0),
                bool_value: Color::Rgb(0, 56, 117),
                error: Color::Rgb(205, 49, 49),
                search_text: Color::Rgb(26, 26, 26),
                search_count: Color::Rgb(80, 90, 110),
                type_desc: Color::Rgb(80, 90, 110),
                line_num: Color::Rgb(130, 140, 160),
                command_usage: Color::Rgb(0, 103, 192),
                key_hint: Color::Rgb(0, 56, 117),
                command_no_match: Color::Rgb(130, 140, 160),
            },
            surface: SurfaceColors {
                title_bg: Color::Rgb(0, 103, 192),
                focus_bg: Color::Rgb(220, 232, 245),
                bg: Color::Rgb(240, 244, 250),
                bg_val1: Color::Rgb(233, 239, 248),
                bg_val2: Color::Rgb(228, 240, 234),
                bg_val3: Color::Rgb(228, 235, 245),
                bg_val4: Color::Rgb(233, 239, 250),
                break_line: Color::Rgb(160, 175, 200),
                highlight_bg: Color::Rgb(194, 220, 255),
                highlight_bg_copy: Color::Rgb(255, 160, 0),
                panel_border: Color::Rgb(0, 103, 192),
                panel_title: Color::Rgb(0, 56, 117),
                help_key_bg: Color::Rgb(190, 215, 245),
                image_border: Color::Rgb(140, 155, 180),
            },
            tree: TreeColors {
                lines: Color::Rgb(160, 175, 200),
                root_file: Color::Rgb(0, 103, 192),
                variable: Color::Rgb(0, 88, 175),
                variable_builtin: Color::Rgb(0, 103, 192),
                file: Color::Rgb(0, 103, 192),
                group: Color::Rgb(136, 23, 152),
                compound_name: Color::Rgb(100, 15, 115),
                dataset: Color::Rgb(26, 26, 26),
                dataset_file: Color::Rgb(9, 134, 88),
                compound: Color::Rgb(136, 23, 152),
                load_more: Color::Rgb(0, 103, 192),
            },
            accent: AccentColors {
                selected_index: Color::Rgb(0, 103, 192),
                selected_dim: Color::Rgb(80, 120, 175),
                equal_sign: Color::Rgb(0, 103, 192),
                symbol: Color::Rgb(0, 56, 117),
                selection_fg: Color::Rgb(26, 26, 26),
                selection_bg: Color::Rgb(194, 220, 255), // Win highlight blue
                search_highlight: Color::Rgb(255, 215, 0),
                search_icon: Color::Rgb(0, 103, 192),
            },
            chart: ChartColors {
                axis: Color::Rgb(26, 26, 26),
                grid: Color::Rgb(185, 200, 220),
                label: Color::Rgb(80, 90, 110),
                preview_line: Color::Rgb(0, 103, 192),
                plot_bg: Color::Rgb(228, 235, 248),
                series: [
                    Color::Rgb(0, 103, 192),
                    Color::Rgb(9, 134, 88),
                    Color::Rgb(136, 23, 152),
                    Color::Rgb(163, 21, 21),
                    Color::Rgb(255, 140, 0),
                    Color::Rgb(0, 150, 136),
                    Color::Rgb(0, 56, 117),
                    Color::Rgb(100, 15, 115),
                ],
                r#enum: [
                    Color::Rgb(136, 23, 152),
                    Color::Rgb(9, 134, 88),
                    Color::Rgb(0, 103, 192),
                    Color::Rgb(163, 21, 21),
                    Color::Rgb(255, 140, 0),
                    Color::Rgb(0, 128, 80),
                    Color::Rgb(0, 56, 117),
                    Color::Rgb(100, 15, 115),
                ],
            },
            status: StatusColors {
                readonly: Color::Rgb(160, 80, 0),
                writable: Color::Rgb(9, 134, 88),
                linked: Color::Rgb(0, 103, 192),
                compability: Color::Rgb(136, 23, 152),
                update_available: Color::Rgb(160, 80, 0),
            },
            toast: ToastColors {
                info: Color::Rgb(9, 134, 88),
                warning: Color::Rgb(160, 80, 0),
                neutral: Color::Rgb(26, 26, 26),
            },
        }
    }
}
