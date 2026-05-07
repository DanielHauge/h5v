use crate::compat;

use ratatui::prelude::Color;
pub const SELECTED_INDEX: Color = Color::Cyan;
pub const SELECTED_DIM: Color = Color::Yellow;
pub const TITLE: Color = Color::Yellow;
pub const META_SECTION_COLOR: Color = Color::Rgb(214, 190, 110);
pub const COLOR_WHITE: Color = Color::White;

pub const NUMBER_COLOR: Color = Color::Rgb(181, 206, 168);
pub const UINT_COLOR: Color = NUMBER_COLOR;
pub const INT_COLOR: Color = NUMBER_COLOR;
pub const FLOAT_COLOR: Color = NUMBER_COLOR;

pub const STRING_COLOR: Color = Color::Rgb(206, 145, 120);
pub const OPAQUE_COLOR: Color = Color::Rgb(198, 160, 255);
pub const BOOL_COLOR: Color = Color::Rgb(255, 204, 0);
pub const ERROR_COLOR: Color = Color::Rgb(255, 0, 0);

pub const LINES_COLOR: Color = Color::Rgb(83, 86, 89);

pub const ROOT_FILE_COLOR: Color = Color::Rgb(186, 230, 250);
pub const VARIABLE_BLUE: Color = Color::Rgb(136, 200, 230);
pub const VARIABLE_BLUE_BUILTIN: Color = Color::Rgb(66, 165, 245);
pub const BUILT_IN_VALUE_COLOR: Color = Color::Rgb(222, 222, 222);
pub const HIGHLIGHT_BG_COLOR: Color = Color::Rgb(55, 62, 70);
// Slightly orangy
pub const HIGHLIGHT_BG_COLOR_COPY: Color = Color::Rgb(255, 153, 0);

pub const FOCUS_BG_COLOR: Color = Color::Rgb(41, 42, 45);
pub const BG_COLOR: Color = Color::Rgb(61, 62, 65);
pub const BG_VAL1_COLOR: Color = Color::Rgb(55, 55, 60);
pub const BG_VAL2_COLOR: Color = Color::Rgb(45, 50, 45);
pub const BG_VAL3_COLOR: Color = Color::Rgb(45, 46, 50);
pub const BG_VAL4_COLOR: Color = Color::Rgb(55, 56, 60);
pub const BREAK_COLOR: Color = Color::Rgb(83, 86, 89);

pub const EQUAL_SIGN_COLOR: Color = Color::Rgb(66, 165, 245);
pub const SYMBOL_COLOR: Color = Color::Rgb(255, 225, 0);
pub const FILE_COLOR: Color = Color::Rgb(66, 165, 245);
pub const GROUP_COLOR: Color = Color::Rgb(255, 204, 0);
pub const COMPOUND_NAME_COLOR: Color = Color::Rgb(214, 170, 0);
// grp file color normal variable name blue
pub const DATASET_COLOR: Color = Color::Rgb(222, 222, 222);
pub const COMPOUND_COLOR: Color = Color::Rgb(200, 140, 255);
pub const SEARCH_TEXT_COLOR: Color = Color::Rgb(222, 222, 222);
pub const SEARCH_COUNT_COLOR: Color = Color::DarkGray;
pub const DATASET_FILE_COLOR: Color = Color::Rgb(38, 166, 154);

// Slightly greyed out
// pub const TYPE_DESC_COLOR: ratatui::prelude::Color = Color::Rgb(150, 150, 150);
// More grey
pub const TYPE_DESC_COLOR: ratatui::prelude::Color = Color::Rgb(150, 150, 150);

pub const LOAD_MORE_COLOR: Color = Color::Yellow;

pub const LINE_NUM_COLOR: Color = Color::DarkGray;

pub const CHART_AXIS_COLOR: Color = BUILT_IN_VALUE_COLOR;
pub const CHART_GRID_COLOR: Color = BREAK_COLOR;
pub const CHART_LABEL_COLOR: Color = TYPE_DESC_COLOR;
pub const CHART_PREVIEW_LINE_COLOR: Color = VARIABLE_BLUE_BUILTIN;
pub const CHART_PLOT_BG_COLOR: Color = FOCUS_BG_COLOR;

pub const CHART_SERIES_COLORS: [Color; 8] = [
    VARIABLE_BLUE_BUILTIN,
    DATASET_FILE_COLOR,
    GROUP_COLOR,
    COMPOUND_COLOR,
    STRING_COLOR,
    BOOL_COLOR,
    ROOT_FILE_COLOR,
    Color::Rgb(129, 199, 132),
];

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
        CHART_SERIES_COLORS[slot % CHART_SERIES_COLORS.len()]
    }
}

fn compat_color(rich: Color, fallback: Color) -> Color {
    if compat::current().compatibility_mode {
        fallback
    } else {
        rich
    }
}

pub fn title_color() -> Color {
    compat_color(TITLE, Color::Yellow)
}

pub fn meta_section_color() -> Color {
    compat_color(META_SECTION_COLOR, Color::Yellow)
}

pub fn lines_color() -> Color {
    compat_color(LINES_COLOR, Color::DarkGray)
}

pub fn root_file_color() -> Color {
    compat_color(ROOT_FILE_COLOR, Color::Cyan)
}

pub fn variable_blue_color() -> Color {
    compat_color(VARIABLE_BLUE, Color::Cyan)
}

pub fn variable_blue_builtin_color() -> Color {
    compat_color(VARIABLE_BLUE_BUILTIN, Color::Blue)
}

pub fn equal_sign_color() -> Color {
    compat_color(EQUAL_SIGN_COLOR, Color::Blue)
}

pub fn symbol_color() -> Color {
    compat_color(SYMBOL_COLOR, Color::Yellow)
}

pub fn file_color() -> Color {
    compat_color(FILE_COLOR, Color::Blue)
}

pub fn group_color() -> Color {
    compat_color(GROUP_COLOR, Color::Yellow)
}

pub fn compound_color() -> Color {
    compat_color(COMPOUND_COLOR, Color::Magenta)
}

pub fn compound_name_color() -> Color {
    compat_color(COMPOUND_NAME_COLOR, Color::Yellow)
}

pub fn dataset_color() -> Color {
    compat_color(DATASET_COLOR, Color::White)
}

pub fn dataset_file_color() -> Color {
    compat_color(DATASET_FILE_COLOR, Color::Green)
}

pub fn load_more_color() -> Color {
    compat_color(LOAD_MORE_COLOR, Color::Yellow)
}

pub fn built_in_value_color() -> Color {
    compat_color(BUILT_IN_VALUE_COLOR, Color::White)
}

pub fn rgb_channels(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::White => (255, 255, 255),
        Color::Yellow => (255, 255, 0),
        Color::Cyan => (0, 255, 255),
        Color::DarkGray => (169, 169, 169),
        other => {
            let fallback = format!("{other:?}");
            if fallback == "Reset" {
                (255, 255, 255)
            } else {
                (255, 255, 255)
            }
        }
    }
}
