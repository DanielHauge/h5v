use macros::{ColorGroup, SymbolGroup, ThemeColorCatalog, ThemeSymbolCatalog};
use ratatui::prelude::Color;

use super::palette::{SymbolThemeName, ThemeName};

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct TextColors {
    pub(crate) primary: Color,
    pub(crate) number: Color,
    pub(crate) string: Color,
    pub(crate) opaque: Color,
    pub(crate) bool_value: Color,
    pub(crate) error: Color,
    pub(crate) search_text: Color,
    pub(crate) search_count: Color,
    pub(crate) type_desc: Color,
    pub(crate) line_num: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct ContentColors {
    pub(crate) app_brand: Color,
    pub(crate) app_version: Color,
    pub(crate) help_hint: Color,
    pub(crate) empty_state: Color,
    pub(crate) tab_active: Color,
    pub(crate) tab_inactive: Color,
    pub(crate) tree_membership_more: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct CommandColors {
    pub(crate) prompt_prefix: Color,
    pub(crate) usage: Color,
    pub(crate) description: Color,
    pub(crate) suggestion_label: Color,
    pub(crate) no_match: Color,
    pub(crate) key_hint: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct HelpColors {
    pub(crate) title: Color,
    pub(crate) section: Color,
    pub(crate) description: Color,
    pub(crate) muted: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct MetadataColors {
    pub(crate) section: Color,
    pub(crate) property_name: Color,
    pub(crate) property_value: Color,
    pub(crate) attribute_name: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct FileColors {
    pub(crate) section_title: Color,
    pub(crate) label: Color,
    pub(crate) value: Color,
}

#[derive(Clone, Debug, PartialEq, Eq, ColorGroup)]
pub(crate) struct MchartColors {
    pub(crate) empty_state: Color,
    pub(crate) item_selected: Color,
    pub(crate) item_selected_hidden: Color,
    pub(crate) item_visible: Color,
    pub(crate) item_hidden: Color,
    pub(crate) prefix_selected: Color,
    pub(crate) prefix: Color,
    pub(crate) detail_label: Color,
    pub(crate) prompt_prefix: Color,
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
    pub(crate) content: ContentColors,
    pub(crate) command: CommandColors,
    pub(crate) help: HelpColors,
    pub(crate) metadata: MetadataColors,
    pub(crate) file: FileColors,
    pub(crate) mchart: MchartColors,
    pub(crate) surface: SurfaceColors,
    pub(crate) tree: TreeColors,
    pub(crate) accent: AccentColors,
    pub(crate) chart: ChartColors,
    pub(crate) status: StatusColors,
    pub(crate) toast: ToastColors,
}

#[derive(Clone, Debug)]
pub struct ConfigSnapshot {
    pub(crate) active_theme: ThemeName,
    pub(crate) active_symbol_theme: SymbolThemeName,
    pub(crate) colors: ThemeColors,
    pub(crate) symbols: UiSymbols,
}

#[derive(Clone, Debug)]
pub(crate) struct ConfigState {
    pub(crate) active_theme: ThemeName,
    pub(crate) active_symbol_theme: SymbolThemeName,
    pub(crate) colors: ThemeColors,
    pub(crate) symbols: UiSymbols,
}

#[derive(Clone, Debug, PartialEq, Eq, SymbolGroup)]
pub(crate) struct TreeSymbols {
    pub(crate) horizontal_rule: &'static str,
    pub(crate) connector_last: &'static str,
    pub(crate) connector_middle: &'static str,
    pub(crate) vertical_guide: &'static str,
    pub(crate) collapse_expanded: &'static str,
    pub(crate) collapse_collapsed: &'static str,
    pub(crate) folder_open_branch: &'static str,
    pub(crate) folder_open_leaf: &'static str,
    pub(crate) folder_closed_branch: &'static str,
    pub(crate) folder_closed_leaf: &'static str,
    pub(crate) root_file_icon: &'static str,
    pub(crate) dataset_icon: &'static str,
    pub(crate) dataset_link_icon: &'static str,
    pub(crate) compound_container_icon: &'static str,
    pub(crate) compound_leaf_icon: &'static str,
    pub(crate) link_marker: &'static str,
    pub(crate) broken_node_icon: &'static str,
    pub(crate) load_more_label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, SymbolGroup)]
pub(crate) struct SectionSymbols {
    pub(crate) properties_title: &'static str,
    pub(crate) attributes_title: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, SymbolGroup)]
pub(crate) struct TitleSymbols {
    pub(crate) preview: &'static str,
    pub(crate) tree: &'static str,
    pub(crate) meta: &'static str,
    pub(crate) file_metadata: &'static str,
    pub(crate) empty_group: &'static str,
    pub(crate) empty_dataset: &'static str,
    pub(crate) error: &'static str,
    pub(crate) create_attribute: &'static str,
    pub(crate) delete_attribute: &'static str,
    pub(crate) fixed_string_overflow: &'static str,
    pub(crate) fixed_string_resize: &'static str,
    pub(crate) help: &'static str,
    pub(crate) matrix_tab: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, SymbolGroup)]
pub(crate) struct BadgeSymbols {
    pub(crate) readonly: &'static str,
    pub(crate) writable: &'static str,
    pub(crate) linked: &'static str,
    pub(crate) linked_root_suffix: &'static str,
    pub(crate) compatibility_mode: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, SymbolGroup)]
pub(crate) struct ChartSymbols {
    pub(crate) membership_marker: &'static str,
    pub(crate) visibility_visible: &'static str,
    pub(crate) visibility_hidden: &'static str,
    pub(crate) r#enum: [&'static str; 8],
}

#[derive(Clone, Debug, PartialEq, Eq, ThemeSymbolCatalog)]
pub(crate) struct UiSymbols {
    pub(crate) tree: TreeSymbols,
    pub(crate) section: SectionSymbols,
    pub(crate) title: TitleSymbols,
    pub(crate) badge: BadgeSymbols,
    pub(crate) chart: ChartSymbols,
}
