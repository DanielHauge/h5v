use std::collections::HashSet;

use super::{
    ActionCode, AttributesAction, ContentAction, Direction, ExportedActionCode, GlobalAction,
    KeymapScope, MultiChartAction, NormalAction, TreeAction, WindowAction,
};

const GLOBAL_ACTION_CODES: &[ActionCode<GlobalAction>] = &[
    ActionCode {
        symbol: "EnterCommand",
        code: "enter-command",
        action: GlobalAction::EnterCommand,
    },
    ActionCode {
        symbol: "ShowHelp",
        code: "show-help",
        action: GlobalAction::ShowHelp,
    },
    ActionCode {
        symbol: "Quit",
        code: "quit",
        action: GlobalAction::Quit,
    },
    ActionCode {
        symbol: "ReloadFile",
        code: "reload-file",
        action: GlobalAction::ReloadFile,
    },
    ActionCode {
        symbol: "ToggleMultiChart",
        code: "toggle-multichart",
        action: GlobalAction::ToggleMultiChart,
    },
];

const NORMAL_ACTION_CODES: &[ActionCode<NormalAction>] = &[
    ActionCode {
        symbol: "EnterCommand",
        code: "enter-command",
        action: NormalAction::EnterCommand,
    },
    ActionCode {
        symbol: "RepeatCommand",
        code: "repeat-command",
        action: NormalAction::RepeatCommand,
    },
    ActionCode {
        symbol: "EnterSearch",
        code: "enter-search",
        action: NormalAction::EnterSearch,
    },
    ActionCode {
        symbol: "Quit",
        code: "quit",
        action: NormalAction::Quit,
    },
    ActionCode {
        symbol: "ToggleContentMode",
        code: "toggle-content-mode",
        action: NormalAction::ToggleContentMode,
    },
    ActionCode {
        symbol: "ShowHelp",
        code: "show-help",
        action: NormalAction::ShowHelp,
    },
    ActionCode {
        symbol: "ToggleMultiChart",
        code: "toggle-multichart",
        action: NormalAction::ToggleMultiChart,
    },
    ActionCode {
        symbol: "ToggleTreeView",
        code: "toggle-tree-view",
        action: NormalAction::ToggleTreeView,
    },
    ActionCode {
        symbol: "ReloadFile",
        code: "reload-file",
        action: NormalAction::ReloadFile,
    },
    ActionCode {
        symbol: "FocusLeft",
        code: "focus-left",
        action: NormalAction::Focus(Direction::Left),
    },
    ActionCode {
        symbol: "FocusRight",
        code: "focus-right",
        action: NormalAction::Focus(Direction::Right),
    },
    ActionCode {
        symbol: "FocusUp",
        code: "focus-up",
        action: NormalAction::Focus(Direction::Up),
    },
    ActionCode {
        symbol: "FocusDown",
        code: "focus-down",
        action: NormalAction::Focus(Direction::Down),
    },
    ActionCode {
        symbol: "StartWindowChord",
        code: "start-window-chord",
        action: NormalAction::StartWindowChord,
    },
    ActionCode {
        symbol: "ChangeXNext",
        code: "change-x-next",
        action: NormalAction::ChangeX(1),
    },
    ActionCode {
        symbol: "ChangeXPrev",
        code: "change-x-prev",
        action: NormalAction::ChangeX(-1),
    },
    ActionCode {
        symbol: "ChangeRowNext",
        code: "change-row-next",
        action: NormalAction::ChangeRow(1),
    },
    ActionCode {
        symbol: "ChangeRowPrev",
        code: "change-row-prev",
        action: NormalAction::ChangeRow(-1),
    },
    ActionCode {
        symbol: "ChangeColNext",
        code: "change-col-next",
        action: NormalAction::ChangeCol(1),
    },
    ActionCode {
        symbol: "ChangeColPrev",
        code: "change-col-prev",
        action: NormalAction::ChangeCol(-1),
    },
    ActionCode {
        symbol: "ChangeSelectedIndexNext",
        code: "change-selected-index-next",
        action: NormalAction::ChangeSelectedIndex(1),
    },
    ActionCode {
        symbol: "ChangeSelectedIndexPrev",
        code: "change-selected-index-prev",
        action: NormalAction::ChangeSelectedIndex(-1),
    },
    ActionCode {
        symbol: "ChangeSelectedIndexNext10",
        code: "change-selected-index-next-10",
        action: NormalAction::ChangeSelectedIndex(10),
    },
    ActionCode {
        symbol: "ChangeSelectedIndexPrev10",
        code: "change-selected-index-prev-10",
        action: NormalAction::ChangeSelectedIndex(-10),
    },
    ActionCode {
        symbol: "ChangeSelectedDimensionNext",
        code: "change-selected-dimension-next",
        action: NormalAction::ChangeSelectedDimension(1),
    },
    ActionCode {
        symbol: "ChangeSelectedDimensionPrev",
        code: "change-selected-dimension-prev",
        action: NormalAction::ChangeSelectedDimension(-1),
    },
    ActionCode {
        symbol: "ScrollLeft",
        code: "scroll-left",
        action: NormalAction::Scroll(Direction::Left, 1),
    },
    ActionCode {
        symbol: "ScrollRight",
        code: "scroll-right",
        action: NormalAction::Scroll(Direction::Right, 1),
    },
    ActionCode {
        symbol: "ScrollUp",
        code: "scroll-up",
        action: NormalAction::Scroll(Direction::Up, 1),
    },
    ActionCode {
        symbol: "ScrollDown",
        code: "scroll-down",
        action: NormalAction::Scroll(Direction::Down, 1),
    },
    ActionCode {
        symbol: "PageUp",
        code: "page-up",
        action: NormalAction::Scroll(Direction::Up, 20),
    },
    ActionCode {
        symbol: "PageDown",
        code: "page-down",
        action: NormalAction::Scroll(Direction::Down, 20),
    },
];

const WINDOW_ACTION_CODES: &[ActionCode<WindowAction>] = &[
    ActionCode {
        symbol: "FocusLeft",
        code: "focus-left",
        action: WindowAction::Focus(Direction::Left),
    },
    ActionCode {
        symbol: "FocusRight",
        code: "focus-right",
        action: WindowAction::Focus(Direction::Right),
    },
    ActionCode {
        symbol: "FocusUp",
        code: "focus-up",
        action: WindowAction::Focus(Direction::Up),
    },
    ActionCode {
        symbol: "FocusDown",
        code: "focus-down",
        action: WindowAction::Focus(Direction::Down),
    },
    ActionCode {
        symbol: "ToggleTreeView",
        code: "toggle-tree-view",
        action: WindowAction::ToggleTreeView,
    },
];

const TREE_ACTION_CODES: &[ActionCode<TreeAction>] = &[
    ActionCode {
        symbol: "MoveUp",
        code: "move-up",
        action: TreeAction::MoveUp(1),
    },
    ActionCode {
        symbol: "MoveDown",
        code: "move-down",
        action: TreeAction::MoveDown(1),
    },
    ActionCode {
        symbol: "MoveUp10",
        code: "move-up-10",
        action: TreeAction::MoveUp(10),
    },
    ActionCode {
        symbol: "MoveDown10",
        code: "move-down-10",
        action: TreeAction::MoveDown(10),
    },
    ActionCode {
        symbol: "MoveTop",
        code: "move-top",
        action: TreeAction::MoveTop,
    },
    ActionCode {
        symbol: "MoveBottom",
        code: "move-bottom",
        action: TreeAction::MoveBottom,
    },
    ActionCode {
        symbol: "Collapse",
        code: "collapse",
        action: TreeAction::Collapse,
    },
    ActionCode {
        symbol: "Expand",
        code: "expand",
        action: TreeAction::Expand,
    },
    ActionCode {
        symbol: "Toggle",
        code: "toggle",
        action: TreeAction::Toggle,
    },
    ActionCode {
        symbol: "AddToMchart",
        code: "add-to-mchart",
        action: TreeAction::AddToMultiChart,
    },
];

const CONTENT_ACTION_CODES: &[ActionCode<ContentAction>] = &[
    ActionCode {
        symbol: "MoveLeft",
        code: "move-left",
        action: ContentAction::Move(Direction::Left, 1),
    },
    ActionCode {
        symbol: "MoveRight",
        code: "move-right",
        action: ContentAction::Move(Direction::Right, 1),
    },
    ActionCode {
        symbol: "MoveUp",
        code: "move-up",
        action: ContentAction::Move(Direction::Up, 1),
    },
    ActionCode {
        symbol: "MoveDown",
        code: "move-down",
        action: ContentAction::Move(Direction::Down, 1),
    },
    ActionCode {
        symbol: "MoveUp10",
        code: "move-up-10",
        action: ContentAction::Move(Direction::Up, 10),
    },
    ActionCode {
        symbol: "MoveDown10",
        code: "move-down-10",
        action: ContentAction::Move(Direction::Down, 10),
    },
    ActionCode {
        symbol: "Edit",
        code: "edit",
        action: ContentAction::Edit,
    },
    ActionCode {
        symbol: "Copy",
        code: "copy",
        action: ContentAction::Copy,
    },
    ActionCode {
        symbol: "HeatmapZoomIn",
        code: "heatmap-zoom-in",
        action: ContentAction::HeatmapZoomIn,
    },
    ActionCode {
        symbol: "HeatmapZoomOut",
        code: "heatmap-zoom-out",
        action: ContentAction::HeatmapZoomOut,
    },
    ActionCode {
        symbol: "HeatmapResetView",
        code: "heatmap-reset-view",
        action: ContentAction::HeatmapResetView,
    },
    ActionCode {
        symbol: "HeatmapClearSelection",
        code: "heatmap-clear-selection",
        action: ContentAction::HeatmapClearSelection,
    },
    ActionCode {
        symbol: "HeatmapPanLeft",
        code: "heatmap-pan-left",
        action: ContentAction::HeatmapPan(Direction::Left),
    },
    ActionCode {
        symbol: "HeatmapPanRight",
        code: "heatmap-pan-right",
        action: ContentAction::HeatmapPan(Direction::Right),
    },
    ActionCode {
        symbol: "HeatmapPanUp",
        code: "heatmap-pan-up",
        action: ContentAction::HeatmapPan(Direction::Up),
    },
    ActionCode {
        symbol: "HeatmapPanDown",
        code: "heatmap-pan-down",
        action: ContentAction::HeatmapPan(Direction::Down),
    },
];

const ATTRIBUTES_ACTION_CODES: &[ActionCode<AttributesAction>] = &[
    ActionCode {
        symbol: "MoveLeft",
        code: "move-left",
        action: AttributesAction::Move(Direction::Left, 1),
    },
    ActionCode {
        symbol: "MoveRight",
        code: "move-right",
        action: AttributesAction::Move(Direction::Right, 1),
    },
    ActionCode {
        symbol: "MoveUp",
        code: "move-up",
        action: AttributesAction::Move(Direction::Up, 1),
    },
    ActionCode {
        symbol: "MoveDown",
        code: "move-down",
        action: AttributesAction::Move(Direction::Down, 1),
    },
    ActionCode {
        symbol: "MoveUp10",
        code: "move-up-10",
        action: AttributesAction::Move(Direction::Up, 10),
    },
    ActionCode {
        symbol: "MoveDown10",
        code: "move-down-10",
        action: AttributesAction::Move(Direction::Down, 10),
    },
    ActionCode {
        symbol: "Edit",
        code: "edit",
        action: AttributesAction::Edit,
    },
    ActionCode {
        symbol: "Copy",
        code: "copy",
        action: AttributesAction::Copy,
    },
    ActionCode {
        symbol: "Create",
        code: "create",
        action: AttributesAction::Create,
    },
    ActionCode {
        symbol: "Delete",
        code: "delete",
        action: AttributesAction::Delete,
    },
];

const MULTICHART_ACTION_CODES: &[ActionCode<MultiChartAction>] = &[
    ActionCode {
        symbol: "EnterCommand",
        code: "enter-command",
        action: MultiChartAction::EnterCommand,
    },
    ActionCode {
        symbol: "Exit",
        code: "exit",
        action: MultiChartAction::Exit,
    },
    ActionCode {
        symbol: "Quit",
        code: "quit",
        action: MultiChartAction::Quit,
    },
    ActionCode {
        symbol: "ShowHelp",
        code: "show-help",
        action: MultiChartAction::ShowHelp,
    },
    ActionCode {
        symbol: "ZoomIn",
        code: "zoom-in",
        action: MultiChartAction::ZoomIn,
    },
    ActionCode {
        symbol: "ZoomOut",
        code: "zoom-out",
        action: MultiChartAction::ZoomOut,
    },
    ActionCode {
        symbol: "PanLeft",
        code: "pan-left",
        action: MultiChartAction::PanLeft,
    },
    ActionCode {
        symbol: "PanRight",
        code: "pan-right",
        action: MultiChartAction::PanRight,
    },
    ActionCode {
        symbol: "ClearZoom",
        code: "clear-zoom",
        action: MultiChartAction::ClearZoom,
    },
    ActionCode {
        symbol: "FitAll",
        code: "fit-all",
        action: MultiChartAction::FitAll,
    },
    ActionCode {
        symbol: "FitSelected",
        code: "fit-selected",
        action: MultiChartAction::FitSelected,
    },
    ActionCode {
        symbol: "DeleteSelected",
        code: "delete-selected",
        action: MultiChartAction::DeleteSelected,
    },
    ActionCode {
        symbol: "ClearAll",
        code: "clear-all",
        action: MultiChartAction::ClearAll,
    },
    ActionCode {
        symbol: "ToggleSelectedVisible",
        code: "toggle-selected-visible",
        action: MultiChartAction::ToggleSelectedVisible,
    },
    ActionCode {
        symbol: "OpenExpressionPrompt",
        code: "open-expression-prompt",
        action: MultiChartAction::OpenExpressionPrompt,
    },
    ActionCode {
        symbol: "EditSelectedExpression",
        code: "edit-selected-expression",
        action: MultiChartAction::EditSelectedExpression,
    },
    ActionCode {
        symbol: "MoveUp",
        code: "move-up",
        action: MultiChartAction::MoveUp,
    },
    ActionCode {
        symbol: "MoveDown",
        code: "move-down",
        action: MultiChartAction::MoveDown,
    },
];

fn parse_action_code<T: Copy>(codes: &[ActionCode<T>], value: &str) -> Option<T> {
    let normalized = value.trim();
    codes
        .iter()
        .find(|entry| entry.code.eq_ignore_ascii_case(normalized))
        .map(|entry| entry.action)
}

pub fn exported_action_codes() -> Vec<ExportedActionCode> {
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    for code in GLOBAL_ACTION_CODES
        .iter()
        .map(|entry| ExportedActionCode {
            symbol: entry.symbol,
            code: entry.code,
        })
        .chain(NORMAL_ACTION_CODES.iter().map(|entry| ExportedActionCode {
            symbol: entry.symbol,
            code: entry.code,
        }))
        .chain(WINDOW_ACTION_CODES.iter().map(|entry| ExportedActionCode {
            symbol: entry.symbol,
            code: entry.code,
        }))
        .chain(TREE_ACTION_CODES.iter().map(|entry| ExportedActionCode {
            symbol: entry.symbol,
            code: entry.code,
        }))
        .chain(CONTENT_ACTION_CODES.iter().map(|entry| ExportedActionCode {
            symbol: entry.symbol,
            code: entry.code,
        }))
        .chain(
            ATTRIBUTES_ACTION_CODES
                .iter()
                .map(|entry| ExportedActionCode {
                    symbol: entry.symbol,
                    code: entry.code,
                }),
        )
        .chain(
            MULTICHART_ACTION_CODES
                .iter()
                .map(|entry| ExportedActionCode {
                    symbol: entry.symbol,
                    code: entry.code,
                }),
        )
    {
        if seen.insert(code.symbol) {
            values.push(code);
        }
    }
    values
}

pub fn exported_mode_codes() -> &'static [(&'static str, &'static str)] {
    &[
        ("Global", "global"),
        ("Normal", "normal"),
        ("Window", "window"),
        ("Tree", "tree"),
        ("Content", "content"),
        ("Heatmap", "heatmap"),
        ("Attributes", "attributes"),
        ("MultiChart", "mchart"),
    ]
}

pub fn parse_global_action_name(value: &str) -> Option<GlobalAction> {
    parse_action_code(GLOBAL_ACTION_CODES, value)
}

pub fn parse_normal_action_name(value: &str) -> Option<NormalAction> {
    parse_action_code(NORMAL_ACTION_CODES, value)
}

pub fn parse_window_action_name(value: &str) -> Option<WindowAction> {
    parse_action_code(WINDOW_ACTION_CODES, value)
}

pub fn parse_tree_action_name(value: &str) -> Option<TreeAction> {
    parse_action_code(TREE_ACTION_CODES, value)
}

pub fn parse_content_action_name(value: &str) -> Option<ContentAction> {
    parse_action_code(CONTENT_ACTION_CODES, value)
}

pub fn parse_attributes_action_name(value: &str) -> Option<AttributesAction> {
    parse_action_code(ATTRIBUTES_ACTION_CODES, value)
}

pub fn parse_multichart_action_name(value: &str) -> Option<MultiChartAction> {
    parse_action_code(MULTICHART_ACTION_CODES, value)
}

pub fn is_valid_action_name_for_scope(scope: KeymapScope, value: &str) -> bool {
    match scope {
        KeymapScope::Global => parse_global_action_name(value).is_some(),
        KeymapScope::Normal => parse_normal_action_name(value).is_some(),
        KeymapScope::Window => parse_window_action_name(value).is_some(),
        KeymapScope::Tree => parse_tree_action_name(value).is_some(),
        KeymapScope::Content | KeymapScope::Heatmap => parse_content_action_name(value).is_some(),
        KeymapScope::Attributes => parse_attributes_action_name(value).is_some(),
        KeymapScope::MultiChart => parse_multichart_action_name(value).is_some(),
    }
}
