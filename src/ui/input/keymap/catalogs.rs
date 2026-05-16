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
        default_keys: &[],
    },
    ActionCode {
        symbol: "ShowHelp",
        code: "show-help",
        action: GlobalAction::ShowHelp,
        default_keys: &[],
    },
    ActionCode {
        symbol: "Quit",
        code: "quit",
        action: GlobalAction::Quit,
        default_keys: &[],
    },
    ActionCode {
        symbol: "ReloadFile",
        code: "reload-file",
        action: GlobalAction::ReloadFile,
        default_keys: &[],
    },
    ActionCode {
        symbol: "ToggleMultiChart",
        code: "toggle-multichart",
        action: GlobalAction::ToggleMultiChart,
        default_keys: &[],
    },
];

const NORMAL_ACTION_CODES: &[ActionCode<NormalAction>] = &[
    ActionCode {
        symbol: "EnterCommand",
        code: "enter-command",
        action: NormalAction::EnterCommand,
        default_keys: &[":"],
    },
    ActionCode {
        symbol: "RepeatCommand",
        code: "repeat-command",
        action: NormalAction::RepeatCommand,
        default_keys: &["."],
    },
    ActionCode {
        symbol: "EnterSearch",
        code: "enter-search",
        action: NormalAction::EnterSearch,
        default_keys: &["/"],
    },
    ActionCode {
        symbol: "Quit",
        code: "quit",
        action: NormalAction::Quit,
        default_keys: &["q", "Ctrl+c"],
    },
    ActionCode {
        symbol: "ToggleContentMode",
        code: "toggle-content-mode",
        action: NormalAction::ToggleContentMode,
        default_keys: &["Tab"],
    },
    ActionCode {
        symbol: "ShowHelp",
        code: "show-help",
        action: NormalAction::ShowHelp,
        default_keys: &["?"],
    },
    ActionCode {
        symbol: "ToggleMultiChart",
        code: "toggle-multichart",
        action: NormalAction::ToggleMultiChart,
        default_keys: &["M"],
    },
    ActionCode {
        symbol: "ToggleTreeView",
        code: "toggle-tree-view",
        action: NormalAction::ToggleTreeView,
        default_keys: &["s"],
    },
    ActionCode {
        symbol: "ReloadFile",
        code: "reload-file",
        action: NormalAction::ReloadFile,
        default_keys: &["Ctrl+r"],
    },
    ActionCode {
        symbol: "FocusLeft",
        code: "focus-left",
        action: NormalAction::Focus(Direction::Left),
        default_keys: &["Shift+Left"],
    },
    ActionCode {
        symbol: "FocusRight",
        code: "focus-right",
        action: NormalAction::Focus(Direction::Right),
        default_keys: &["Shift+Right"],
    },
    ActionCode {
        symbol: "FocusUp",
        code: "focus-up",
        action: NormalAction::Focus(Direction::Up),
        default_keys: &["Shift+Up"],
    },
    ActionCode {
        symbol: "FocusDown",
        code: "focus-down",
        action: NormalAction::Focus(Direction::Down),
        default_keys: &["Shift+Down"],
    },
    ActionCode {
        symbol: "StartWindowChord",
        code: "start-window-chord",
        action: NormalAction::StartWindowChord,
        default_keys: &["Ctrl+w"],
    },
    ActionCode {
        symbol: "ChangeXNext",
        code: "change-x-next",
        action: NormalAction::ChangeX(1),
        default_keys: &["x"],
    },
    ActionCode {
        symbol: "ChangeXPrev",
        code: "change-x-prev",
        action: NormalAction::ChangeX(-1),
        default_keys: &["X"],
    },
    ActionCode {
        symbol: "ChangeRowNext",
        code: "change-row-next",
        action: NormalAction::ChangeRow(1),
        default_keys: &["r"],
    },
    ActionCode {
        symbol: "ChangeRowPrev",
        code: "change-row-prev",
        action: NormalAction::ChangeRow(-1),
        default_keys: &["R"],
    },
    ActionCode {
        symbol: "ChangeColNext",
        code: "change-col-next",
        action: NormalAction::ChangeCol(1),
        default_keys: &["c"],
    },
    ActionCode {
        symbol: "ChangeColPrev",
        code: "change-col-prev",
        action: NormalAction::ChangeCol(-1),
        default_keys: &["C"],
    },
    ActionCode {
        symbol: "ChangeSelectedIndexNext",
        code: "change-selected-index-next",
        action: NormalAction::ChangeSelectedIndex(1),
        default_keys: &["Ctrl+a", "Alt+Down"],
    },
    ActionCode {
        symbol: "ChangeSelectedIndexPrev",
        code: "change-selected-index-prev",
        action: NormalAction::ChangeSelectedIndex(-1),
        default_keys: &["Ctrl+x", "Alt+Up"],
    },
    ActionCode {
        symbol: "ChangeSelectedIndexNext10",
        code: "change-selected-index-next-10",
        action: NormalAction::ChangeSelectedIndex(10),
        default_keys: &["Alt+PageDown"],
    },
    ActionCode {
        symbol: "ChangeSelectedIndexPrev10",
        code: "change-selected-index-prev-10",
        action: NormalAction::ChangeSelectedIndex(-10),
        default_keys: &["Alt+PageUp"],
    },
    ActionCode {
        symbol: "ChangeSelectedDimensionNext",
        code: "change-selected-dimension-next",
        action: NormalAction::ChangeSelectedDimension(1),
        default_keys: &["]", "Alt+Right"],
    },
    ActionCode {
        symbol: "ChangeSelectedDimensionPrev",
        code: "change-selected-dimension-prev",
        action: NormalAction::ChangeSelectedDimension(-1),
        default_keys: &["[", "Alt+Left"],
    },
    ActionCode {
        symbol: "ScrollLeft",
        code: "scroll-left",
        action: NormalAction::Scroll(Direction::Left, 1),
        default_keys: &["Ctrl+Left"],
    },
    ActionCode {
        symbol: "ScrollRight",
        code: "scroll-right",
        action: NormalAction::Scroll(Direction::Right, 1),
        default_keys: &["Ctrl+Right"],
    },
    ActionCode {
        symbol: "ScrollUp",
        code: "scroll-up",
        action: NormalAction::Scroll(Direction::Up, 1),
        default_keys: &["Ctrl+Up"],
    },
    ActionCode {
        symbol: "ScrollDown",
        code: "scroll-down",
        action: NormalAction::Scroll(Direction::Down, 1),
        default_keys: &["Ctrl+Down"],
    },
    ActionCode {
        symbol: "PageUp",
        code: "page-up",
        action: NormalAction::Scroll(Direction::Up, 20),
        default_keys: &["PageUp"],
    },
    ActionCode {
        symbol: "PageDown",
        code: "page-down",
        action: NormalAction::Scroll(Direction::Down, 20),
        default_keys: &["PageDown"],
    },
];

const WINDOW_ACTION_CODES: &[ActionCode<WindowAction>] = &[
    ActionCode {
        symbol: "FocusLeft",
        code: "focus-left",
        action: WindowAction::Focus(Direction::Left),
        default_keys: &["h", "Left"],
    },
    ActionCode {
        symbol: "FocusRight",
        code: "focus-right",
        action: WindowAction::Focus(Direction::Right),
        default_keys: &["l", "Right"],
    },
    ActionCode {
        symbol: "FocusUp",
        code: "focus-up",
        action: WindowAction::Focus(Direction::Up),
        default_keys: &["k", "Up"],
    },
    ActionCode {
        symbol: "FocusDown",
        code: "focus-down",
        action: WindowAction::Focus(Direction::Down),
        default_keys: &["j", "Down"],
    },
    ActionCode {
        symbol: "ToggleTreeView",
        code: "toggle-tree-view",
        action: WindowAction::ToggleTreeView,
        default_keys: &["o"],
    },
];

const TREE_ACTION_CODES: &[ActionCode<TreeAction>] = &[
    ActionCode {
        symbol: "MoveUp",
        code: "move-up",
        action: TreeAction::MoveUp(1),
        default_keys: &["Up", "k", "K"],
    },
    ActionCode {
        symbol: "MoveDown",
        code: "move-down",
        action: TreeAction::MoveDown(1),
        default_keys: &["Down", "j", "J"],
    },
    ActionCode {
        symbol: "MoveUp10",
        code: "move-up-10",
        action: TreeAction::MoveUp(10),
        default_keys: &["u", "Ctrl+u"],
    },
    ActionCode {
        symbol: "MoveDown10",
        code: "move-down-10",
        action: TreeAction::MoveDown(10),
        default_keys: &["Ctrl+d"],
    },
    ActionCode {
        symbol: "MoveTop",
        code: "move-top",
        action: TreeAction::MoveTop,
        default_keys: &["g", "Home"],
    },
    ActionCode {
        symbol: "MoveBottom",
        code: "move-bottom",
        action: TreeAction::MoveBottom,
        default_keys: &["G", "End"],
    },
    ActionCode {
        symbol: "Collapse",
        code: "collapse",
        action: TreeAction::Collapse,
        default_keys: &["h", "H", "Left"],
    },
    ActionCode {
        symbol: "Expand",
        code: "expand",
        action: TreeAction::Expand,
        default_keys: &["l", "L", "Right"],
    },
    ActionCode {
        symbol: "Toggle",
        code: "toggle",
        action: TreeAction::Toggle,
        default_keys: &["Enter", "Space"],
    },
    ActionCode {
        symbol: "AddToMchart",
        code: "add-to-mchart",
        action: TreeAction::AddToMultiChart,
        default_keys: &["m"],
    },
];

const CONTENT_ACTION_CODES: &[ActionCode<ContentAction>] = &[
    ActionCode {
        symbol: "MoveLeft",
        code: "move-left",
        action: ContentAction::Move(Direction::Left, 1),
        default_keys: &["Left", "h"],
    },
    ActionCode {
        symbol: "MoveRight",
        code: "move-right",
        action: ContentAction::Move(Direction::Right, 1),
        default_keys: &["Right", "l"],
    },
    ActionCode {
        symbol: "MoveUp",
        code: "move-up",
        action: ContentAction::Move(Direction::Up, 1),
        default_keys: &["Up", "k"],
    },
    ActionCode {
        symbol: "MoveDown",
        code: "move-down",
        action: ContentAction::Move(Direction::Down, 1),
        default_keys: &["Down", "j"],
    },
    ActionCode {
        symbol: "MoveUp10",
        code: "move-up-10",
        action: ContentAction::Move(Direction::Up, 10),
        default_keys: &["Ctrl+u"],
    },
    ActionCode {
        symbol: "MoveDown10",
        code: "move-down-10",
        action: ContentAction::Move(Direction::Down, 10),
        default_keys: &["Ctrl+d"],
    },
    ActionCode {
        symbol: "Edit",
        code: "edit",
        action: ContentAction::Edit,
        default_keys: &["Enter", "e"],
    },
    ActionCode {
        symbol: "Copy",
        code: "copy",
        action: ContentAction::Copy,
        default_keys: &["y"],
    },
    ActionCode {
        symbol: "HeatmapZoomIn",
        code: "heatmap-zoom-in",
        action: ContentAction::HeatmapZoomIn,
        default_keys: &["z"],
    },
    ActionCode {
        symbol: "HeatmapZoomOut",
        code: "heatmap-zoom-out",
        action: ContentAction::HeatmapZoomOut,
        default_keys: &["Z"],
    },
    ActionCode {
        symbol: "HeatmapResetView",
        code: "heatmap-reset-view",
        action: ContentAction::HeatmapResetView,
        default_keys: &["0"],
    },
    ActionCode {
        symbol: "HeatmapClearSelection",
        code: "heatmap-clear-selection",
        action: ContentAction::HeatmapClearSelection,
        default_keys: &["v"],
    },
    ActionCode {
        symbol: "HeatmapPanLeft",
        code: "heatmap-pan-left",
        action: ContentAction::HeatmapPan(Direction::Left),
        default_keys: &["H"],
    },
    ActionCode {
        symbol: "HeatmapPanRight",
        code: "heatmap-pan-right",
        action: ContentAction::HeatmapPan(Direction::Right),
        default_keys: &["L"],
    },
    ActionCode {
        symbol: "HeatmapPanUp",
        code: "heatmap-pan-up",
        action: ContentAction::HeatmapPan(Direction::Up),
        default_keys: &["K"],
    },
    ActionCode {
        symbol: "HeatmapPanDown",
        code: "heatmap-pan-down",
        action: ContentAction::HeatmapPan(Direction::Down),
        default_keys: &["J"],
    },
];

const ATTRIBUTES_ACTION_CODES: &[ActionCode<AttributesAction>] = &[
    ActionCode {
        symbol: "MoveLeft",
        code: "move-left",
        action: AttributesAction::Move(Direction::Left, 1),
        default_keys: &["Left", "h"],
    },
    ActionCode {
        symbol: "MoveRight",
        code: "move-right",
        action: AttributesAction::Move(Direction::Right, 1),
        default_keys: &["Right", "l"],
    },
    ActionCode {
        symbol: "MoveUp",
        code: "move-up",
        action: AttributesAction::Move(Direction::Up, 1),
        default_keys: &["Up", "k"],
    },
    ActionCode {
        symbol: "MoveDown",
        code: "move-down",
        action: AttributesAction::Move(Direction::Down, 1),
        default_keys: &["Down", "j"],
    },
    ActionCode {
        symbol: "MoveUp10",
        code: "move-up-10",
        action: AttributesAction::Move(Direction::Up, 10),
        default_keys: &["Ctrl+u", "PageUp"],
    },
    ActionCode {
        symbol: "MoveDown10",
        code: "move-down-10",
        action: AttributesAction::Move(Direction::Down, 10),
        default_keys: &["Ctrl+d", "PageDown"],
    },
    ActionCode {
        symbol: "Edit",
        code: "edit",
        action: AttributesAction::Edit,
        default_keys: &["Enter", "e"],
    },
    ActionCode {
        symbol: "Copy",
        code: "copy",
        action: AttributesAction::Copy,
        default_keys: &["y"],
    },
    ActionCode {
        symbol: "Create",
        code: "create",
        action: AttributesAction::Create,
        default_keys: &["a"],
    },
    ActionCode {
        symbol: "Delete",
        code: "delete",
        action: AttributesAction::Delete,
        default_keys: &["d", "Delete"],
    },
];

const MULTICHART_ACTION_CODES: &[ActionCode<MultiChartAction>] = &[
    ActionCode {
        symbol: "EnterCommand",
        code: "enter-command",
        action: MultiChartAction::EnterCommand,
        default_keys: &[":"],
    },
    ActionCode {
        symbol: "Exit",
        code: "exit",
        action: MultiChartAction::Exit,
        default_keys: &["Esc", "M"],
    },
    ActionCode {
        symbol: "Quit",
        code: "quit",
        action: MultiChartAction::Quit,
        default_keys: &["q"],
    },
    ActionCode {
        symbol: "ShowHelp",
        code: "show-help",
        action: MultiChartAction::ShowHelp,
        default_keys: &["?"],
    },
    ActionCode {
        symbol: "CycleViewMode",
        code: "cycle-view-mode",
        action: MultiChartAction::CycleViewMode,
        default_keys: &["Tab", "Shift+Tab", "t"],
    },
    ActionCode {
        symbol: "ZoomIn",
        code: "zoom-in",
        action: MultiChartAction::ZoomIn,
        default_keys: &["Shift+Up", "z", "+", "="],
    },
    ActionCode {
        symbol: "ZoomOut",
        code: "zoom-out",
        action: MultiChartAction::ZoomOut,
        default_keys: &["Shift+Down", "Z", "-"],
    },
    ActionCode {
        symbol: "PanLeft",
        code: "pan-left",
        action: MultiChartAction::PanLeft,
        default_keys: &["Shift+Left", "h"],
    },
    ActionCode {
        symbol: "PanRight",
        code: "pan-right",
        action: MultiChartAction::PanRight,
        default_keys: &["Shift+Right", "l"],
    },
    ActionCode {
        symbol: "ClearZoom",
        code: "clear-zoom",
        action: MultiChartAction::ClearZoom,
        default_keys: &["c", "0"],
    },
    ActionCode {
        symbol: "FitAll",
        code: "fit-all",
        action: MultiChartAction::FitAll,
        default_keys: &["f"],
    },
    ActionCode {
        symbol: "FitSelected",
        code: "fit-selected",
        action: MultiChartAction::FitSelected,
        default_keys: &["F"],
    },
    ActionCode {
        symbol: "DeleteSelected",
        code: "delete-selected",
        action: MultiChartAction::DeleteSelected,
        default_keys: &["Delete", "Backspace", "d"],
    },
    ActionCode {
        symbol: "ClearAll",
        code: "clear-all",
        action: MultiChartAction::ClearAll,
        default_keys: &["C"],
    },
    ActionCode {
        symbol: "ToggleSelectedVisible",
        code: "toggle-selected-visible",
        action: MultiChartAction::ToggleSelectedVisible,
        default_keys: &["Space", "v"],
    },
    ActionCode {
        symbol: "OpenExpressionPrompt",
        code: "open-expression-prompt",
        action: MultiChartAction::OpenExpressionPrompt,
        default_keys: &["Enter", "n"],
    },
    ActionCode {
        symbol: "EditSelectedExpression",
        code: "edit-selected-expression",
        action: MultiChartAction::EditSelectedExpression,
        default_keys: &["e"],
    },
    ActionCode {
        symbol: "MoveUp",
        code: "move-up",
        action: MultiChartAction::MoveUp,
        default_keys: &["Up", "k"],
    },
    ActionCode {
        symbol: "MoveDown",
        code: "move-down",
        action: MultiChartAction::MoveDown,
        default_keys: &["Down", "j"],
    },
    ActionCode {
        symbol: "ReorderUp",
        code: "reorder-up",
        action: MultiChartAction::ReorderUp,
        default_keys: &["Alt+Up"],
    },
    ActionCode {
        symbol: "ReorderDown",
        code: "reorder-down",
        action: MultiChartAction::ReorderDown,
        default_keys: &["Alt+Down"],
    },
];

pub(super) fn global_action_codes() -> &'static [ActionCode<GlobalAction>] {
    GLOBAL_ACTION_CODES
}

pub(super) fn normal_action_codes() -> &'static [ActionCode<NormalAction>] {
    NORMAL_ACTION_CODES
}

pub(super) fn window_action_codes() -> &'static [ActionCode<WindowAction>] {
    WINDOW_ACTION_CODES
}

pub(super) fn tree_action_codes() -> &'static [ActionCode<TreeAction>] {
    TREE_ACTION_CODES
}

pub(super) fn content_action_codes() -> &'static [ActionCode<ContentAction>] {
    CONTENT_ACTION_CODES
}

pub(super) fn attributes_action_codes() -> &'static [ActionCode<AttributesAction>] {
    ATTRIBUTES_ACTION_CODES
}

pub(super) fn multichart_action_codes() -> &'static [ActionCode<MultiChartAction>] {
    MULTICHART_ACTION_CODES
}

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
