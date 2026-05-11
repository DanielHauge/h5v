use ratatui::prelude::Color;

use super::types::{
    AccentColors, BadgeSymbols, ChartColors, ChartSymbols as UiChartSymbols, CommandColors,
    ContentColors, FileColors, HelpColors, MchartColors, MetadataColors, SectionSymbols,
    StatusColors, SurfaceColors, TextColors, ThemeColors, TitleSymbols, ToastColors, TreeColors,
    TreeSymbols, UiSymbols,
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

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ThemeName {
    Dark,
    Light,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum SymbolThemeName {
    Rich,
    Compatibility,
}

impl SymbolThemeName {
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-', '_'], "");
        match normalized.as_str() {
            "rich" | "default" => Some(Self::Rich),
            "compatibility" | "compatibilitymode" | "compat" | "ascii" | "plain" => {
                Some(Self::Compatibility)
            }
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rich => "rich",
            Self::Compatibility => "compatibility",
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
                primary: Color::White,
                number: Color::Rgb(145, 255, 145),
                string: Color::Rgb(206, 145, 120),
                opaque: Color::Rgb(198, 160, 255),
                bool_value: Color::Rgb(255, 204, 0),
                error: Color::Rgb(255, 0, 0),
                search_text: Color::Rgb(222, 222, 222),
                search_count: Color::DarkGray,
                type_desc: Color::Rgb(150, 150, 150),
                line_num: Color::DarkGray,
            },
            content: ContentColors {
                app_brand: Color::Yellow,
                app_version: Color::Rgb(222, 222, 222),
                help_hint: Color::Rgb(150, 150, 150),
                empty_state: Color::Yellow,
                tab_active: Color::Yellow,
                tab_inactive: Color::Rgb(222, 222, 222),
                tree_membership_more: Color::Yellow,
            },
            command: CommandColors {
                prompt_prefix: Color::Yellow,
                usage: Color::Cyan,
                description: Color::Rgb(150, 150, 150),
                suggestion_label: Color::Rgb(150, 150, 150),
                no_match: Color::DarkGray,
                key_hint: Color::Yellow,
            },
            help: HelpColors {
                title: Color::Yellow,
                section: Color::Yellow,
                description: Color::Rgb(222, 222, 222),
                muted: Color::Rgb(150, 150, 150),
            },
            metadata: MetadataColors {
                section: Color::Rgb(214, 190, 110),
                property_name: Color::Rgb(66, 165, 245),
                property_value: Color::Rgb(222, 222, 222),
                attribute_name: Color::Rgb(136, 200, 230),
            },
            file: FileColors {
                section_title: Color::Rgb(150, 150, 150),
                label: Color::Rgb(66, 165, 245),
                value: Color::Rgb(222, 222, 222),
            },
            mchart: MchartColors {
                empty_state: Color::Yellow,
                item_selected: Color::Yellow,
                item_selected_hidden: Color::Yellow,
                item_visible: Color::Rgb(222, 222, 222),
                item_hidden: Color::Rgb(150, 150, 150),
                prefix_selected: Color::Yellow,
                prefix: Color::Rgb(83, 86, 89),
                detail_label: Color::Rgb(66, 165, 245),
                prompt_prefix: Color::Yellow,
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
                group_name: Color::Rgb(136, 200, 230),
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
                enums: [
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
                primary: Color::Rgb(0, 0, 0),
                number: Color::Rgb(14, 124, 58),
                string: Color::Rgb(168, 41, 14),
                opaque: Color::Rgb(124, 58, 237),
                bool_value: Color::Rgb(0, 100, 180),
                error: Color::Rgb(200, 20, 20),
                search_text: Color::Rgb(10, 10, 10),
                search_count: Color::Rgb(90, 95, 110),
                type_desc: Color::Rgb(90, 95, 110),
                line_num: Color::Rgb(140, 145, 158),
            },
            content: ContentColors {
                app_brand: Color::Rgb(30, 58, 95),
                app_version: Color::Rgb(30, 30, 30),
                help_hint: Color::Rgb(90, 95, 110),
                empty_state: Color::Rgb(30, 58, 95),
                tab_active: Color::Rgb(30, 58, 95),
                tab_inactive: Color::Rgb(90, 95, 110),
                tree_membership_more: Color::Rgb(30, 58, 95),
            },
            command: CommandColors {
                prompt_prefix: Color::Rgb(30, 58, 95),
                usage: Color::Rgb(0, 90, 190),
                description: Color::Rgb(90, 95, 110),
                suggestion_label: Color::Rgb(90, 95, 110),
                no_match: Color::Rgb(140, 145, 158),
                key_hint: Color::Rgb(30, 58, 95),
            },
            help: HelpColors {
                title: Color::Rgb(30, 58, 95),
                section: Color::Rgb(30, 58, 95),
                description: Color::Rgb(30, 30, 30),
                muted: Color::Rgb(90, 95, 110),
            },
            metadata: MetadataColors {
                section: Color::Rgb(40, 70, 110),
                property_name: Color::Rgb(0, 80, 200),
                property_value: Color::Rgb(30, 30, 30),
                attribute_name: Color::Rgb(0, 100, 225),
            },
            file: FileColors {
                section_title: Color::Rgb(90, 95, 110),
                label: Color::Rgb(0, 80, 200),
                value: Color::Rgb(30, 30, 30),
            },
            mchart: MchartColors {
                empty_state: Color::Rgb(10, 10, 10),
                item_selected: Color::Rgb(30, 58, 95),
                item_selected_hidden: Color::Rgb(30, 58, 95),
                item_visible: Color::Rgb(30, 30, 30),
                item_hidden: Color::Rgb(90, 95, 110),
                prefix_selected: Color::Rgb(30, 58, 95),
                prefix: Color::Rgb(65, 65, 65),
                detail_label: Color::Blue,
                prompt_prefix: Color::Rgb(30, 58, 95),
            },
            surface: SurfaceColors {
                title_bg: Color::Rgb(235, 225, 235),
                focus_bg: Color::Rgb(245, 245, 245),
                bg: Color::Rgb(255, 255, 255),
                bg_val1: Color::Rgb(247, 249, 252),
                bg_val2: Color::Rgb(242, 247, 242),
                bg_val3: Color::Rgb(242, 245, 250),
                bg_val4: Color::Rgb(247, 249, 253),
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
                group_name: Color::Rgb(00, 90, 245),
                file: Color::Rgb(20, 100, 230),
                group: Color::Rgb(220, 130, 0),
                compound_name: Color::Rgb(190, 155, 30),
                dataset: Color::Rgb(0, 0, 0),
                dataset_file: Color::Rgb(0, 188, 120),
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
                label: Color::Rgb(0, 0, 0),
                preview_line: Color::Rgb(0, 90, 190),
                plot_bg: Color::Rgb(255, 255, 255),
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
                enums: [
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
}

impl UiSymbols {
    pub(crate) fn for_theme(theme: SymbolThemeName) -> Self {
        match theme {
            SymbolThemeName::Rich => Self::rich(),
            SymbolThemeName::Compatibility => Self::compatibility(),
        }
    }

    fn rich() -> Self {
        Self {
            tree: TreeSymbols {
                horizontal_rule: "─",
                connector_last: "└─",
                connector_middle: "├─",
                vertical_guide: "│   ",
                collapse_expanded: " ",
                collapse_collapsed: " ",
                folder_open_branch: "",
                folder_open_leaf: "",
                folder_closed_branch: "",
                folder_closed_leaf: "",
                root_file_icon: "󰈚 ",
                dataset_icon: "󰈚 ",
                dataset_link_icon: "󰈚🔗",
                compound_container_icon: "󰆼 ",
                compound_leaf_icon: "󰈚 ",
                link_marker: "🔗",
                broken_node_icon: "*- ",
                load_more_label: "⤵ Load more",
            },
            section: SectionSymbols {
                properties_title: "󰜉 Properties",
                attributes_title: "󰠱 Attributes",
            },
            title: TitleSymbols {
                preview: "📈 Preview",
                tree: " 🔍 Tree ",
                meta: " 🧾 Meta ",
                file_metadata: " 📄 File metadata ",
                empty_group: " 📁 Empty group preview ",
                empty_dataset: " 🧮 Empty dataset ",
                error: " ⚠ Error ",
                create_attribute: " ✨ Create attribute ",
                delete_attribute: " 🗑 Delete attribute ",
                fixed_string_overflow: " 🧵 Fixed string overflow ",
                fixed_string_resize: " 📏 Change fixed string size ",
                help: " ❔ Help ",
                matrix_tab: "🧮 Matrix",
            },
            badge: BadgeSymbols {
                readonly: " 🔒 read-only ",
                writable: " ✏ write ",
                linked: " 🔗 linked ",
                linked_root_suffix: " ({count}) 🔗 linked ",
                compatibility_mode: " compatibility mode ",
            },
            chart: UiChartSymbols {
                membership_marker: "●",
                visibility_visible: "●",
                visibility_hidden: "○",
                r#enum: ["●", "■", "▲", "◆", "✦", "✚", "⬢", "◉"],
            },
        }
    }

    fn compatibility() -> Self {
        Self {
            tree: TreeSymbols {
                horizontal_rule: "-",
                connector_last: "`-",
                connector_middle: "|-",
                vertical_guide: "|   ",
                collapse_expanded: "v ",
                collapse_collapsed: "> ",
                folder_open_branch: "G",
                folder_open_leaf: "G",
                folder_closed_branch: "G",
                folder_closed_leaf: "g",
                root_file_icon: "F ",
                dataset_icon: "D ",
                dataset_link_icon: "D@",
                compound_container_icon: "C ",
                compound_leaf_icon: "c ",
                link_marker: "@",
                broken_node_icon: "*- ",
                load_more_label: "Load more",
            },
            section: SectionSymbols {
                properties_title: "Properties",
                attributes_title: "Attributes",
            },
            title: TitleSymbols {
                preview: "Preview",
                tree: "Tree",
                meta: "Meta",
                file_metadata: " File metadata ",
                empty_group: " Empty group preview ",
                empty_dataset: " Empty dataset ",
                error: "Error",
                create_attribute: " Create attribute ",
                delete_attribute: " Delete attribute ",
                fixed_string_overflow: " Fixed string overflow ",
                fixed_string_resize: " Change fixed string size ",
                help: " Help ",
                matrix_tab: "Matrix",
            },
            badge: BadgeSymbols {
                readonly: " [ro] read-only ",
                writable: " [rw] write ",
                linked: " linked ",
                linked_root_suffix: " ({count}) linked ",
                compatibility_mode: " compatibility mode ",
            },
            chart: UiChartSymbols {
                membership_marker: "*",
                visibility_visible: "*",
                visibility_hidden: "o",
                r#enum: ["*", "+", "^", "#", "x", "%", "@", "o"],
            },
        }
    }
}
