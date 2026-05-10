use macros::{ColorGroup, ThemeColorCatalog};
use ratatui::prelude::Color;

use crate::color_consts::ThemeName;

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct TextColors {
    pub(crate) title: Color,
    pub(crate) meta_section: Color,
    pub(crate) primary: Color,
    pub(crate) built_in_value: Color,
    pub(crate) number: Color,
    pub(crate) string: Color,
    pub(crate) opaque: Color,
    pub(crate) bool_value: Color,
    pub(crate) error: Color,
    pub(crate) search_text: Color,
    pub(crate) search_count: Color,
    pub(crate) type_desc: Color,
    pub(crate) line_num: Color,
    pub(crate) command_usage: Color,
    pub(crate) key_hint: Color,
    pub(crate) command_no_match: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct SurfaceColors {
    pub(crate) title_bg: Color,
    pub(crate) focus_bg: Color,
    pub(crate) bg: Color,
    pub(crate) bg_val1: Color,
    pub(crate) bg_val2: Color,
    pub(crate) bg_val3: Color,
    pub(crate) bg_val4: Color,
    pub(crate) break_line: Color,
    pub(crate) highlight_bg: Color,
    pub(crate) highlight_bg_copy: Color,
    pub(crate) panel_border: Color,
    pub(crate) panel_title: Color,
    pub(crate) help_key_bg: Color,
    pub(crate) image_border: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct TreeColors {
    pub(crate) lines: Color,
    pub(crate) root_file: Color,
    pub(crate) variable: Color,
    pub(crate) variable_builtin: Color,
    pub(crate) file: Color,
    pub(crate) group: Color,
    pub(crate) compound_name: Color,
    pub(crate) dataset: Color,
    pub(crate) dataset_file: Color,
    pub(crate) compound: Color,
    pub(crate) load_more: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct AccentColors {
    pub(crate) selected_index: Color,
    pub(crate) selected_dim: Color,
    pub(crate) equal_sign: Color,
    pub(crate) symbol: Color,
    pub(crate) selection_fg: Color,
    pub(crate) selection_bg: Color,
    pub(crate) search_highlight: Color,
    pub(crate) search_icon: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct ChartColors {
    pub(crate) axis: Color,
    pub(crate) grid: Color,
    pub(crate) label: Color,
    pub(crate) preview_line: Color,
    pub(crate) plot_bg: Color,
    pub(crate) series: [Color; 8],
    pub(crate) r#enum: [Color; 8],
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct StatusColors {
    pub(crate) readonly: Color,
    pub(crate) writable: Color,
    pub(crate) linked: Color,
    pub(crate) compability: Color,
    pub(crate) update_available: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct ToastColors {
    pub(crate) info: Color,
    pub(crate) warning: Color,
    pub(crate) neutral: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ThemeColorCatalog)]
pub(crate) struct ThemeColors {
    pub(crate) text: TextColors,
    pub(crate) surface: SurfaceColors,
    pub(crate) tree: TreeColors,
    pub(crate) accent: AccentColors,
    pub(crate) chart: ChartColors,
    pub(crate) status: StatusColors,
    pub(crate) toast: ToastColors,
}

#[derive(Clone, Debug)]
pub struct ThemeSnapshot {
    pub(crate) active_theme: ThemeName,
    pub(crate) colors: ThemeColors,
}

#[derive(Clone, Debug)]
pub(crate) struct ThemeState {
    pub(crate) active_theme: ThemeName,
    pub(crate) colors: ThemeColors,
}
