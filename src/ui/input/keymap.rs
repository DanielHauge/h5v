use std::{collections::HashSet, fmt};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalAction {
    EnterCommand,
    ShowHelp,
    Quit,
    ReloadFile,
    ToggleMultiChart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalAction {
    EnterCommand,
    RepeatCommand,
    EnterSearch,
    Quit,
    ToggleContentMode,
    ShowHelp,
    ToggleMultiChart,
    ToggleTreeView,
    ReloadFile,
    Focus(Direction),
    StartWindowChord,
    ChangeX(isize),
    ChangeRow(isize),
    ChangeCol(isize),
    ChangeSelectedIndex(isize),
    ChangeSelectedDimension(isize),
    Scroll(Direction, usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    Focus(Direction),
    ToggleTreeView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeAction {
    MoveUp(usize),
    MoveDown(usize),
    MoveTop,
    MoveBottom,
    Collapse,
    Expand,
    Toggle,
    AddToMultiChart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentAction {
    Move(Direction, usize),
    Edit,
    Copy,
    HeatmapZoomIn,
    HeatmapZoomOut,
    HeatmapResetView,
    HeatmapClearSelection,
    HeatmapPan(Direction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributesAction {
    Move(Direction, usize),
    Edit,
    Copy,
    Create,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAction {
    ClearQuery,
    Insert(char),
    Backspace,
    Delete,
    Move(Direction),
    Submit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiChartAction {
    EnterCommand,
    Exit,
    Quit,
    ShowHelp,
    ZoomIn,
    ZoomOut,
    PanLeft,
    PanRight,
    ClearZoom,
    DeleteSelected,
    ClearAll,
    ToggleSelectedVisible,
    OpenExpressionPrompt,
    EditSelectedExpression,
    MoveUp,
    MoveDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAction {
    Submit,
    Cancel,
    SelectPrevSuggestion,
    SelectNextSuggestion,
    SelectPrevHistory,
    SelectNextHistory,
    ClearWord,
    MoveToStart,
    MoveToEnd,
    Clear,
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    InsertChar(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapScope {
    Global,
    Normal,
    Window,
    Tree,
    Content,
    Heatmap,
    Attributes,
    MultiChart,
}

impl KeymapScope {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "global" => Some(Self::Global),
            "normal" => Some(Self::Normal),
            "window" => Some(Self::Window),
            "tree" => Some(Self::Tree),
            "content" => Some(Self::Content),
            "heatmap" => Some(Self::Heatmap),
            "attributes" | "attrs" => Some(Self::Attributes),
            "mchart" | "multichart" => Some(Self::MultiChart),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Normal => "normal",
            Self::Window => "window",
            Self::Tree => "tree",
            Self::Content => "content",
            Self::Heatmap => "heatmap",
            Self::Attributes => "attributes",
            Self::MultiChart => "mchart",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionCode<T> {
    pub symbol: &'static str,
    pub code: &'static str,
    pub action: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundAction<T> {
    Action(T),
    Command(String),
    Script(String),
    LuaCallback(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPattern {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding<T> {
    pub key: KeyPattern,
    pub target: BoundAction<T>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeKeymapConfig<T> {
    pub clear_defaults: bool,
    pub unbind: Vec<KeyPattern>,
    pub bind: Vec<KeyBinding<T>>,
}

impl<T> Default for ScopeKeymapConfig<T> {
    fn default() -> Self {
        Self {
            clear_defaults: false,
            unbind: Vec::new(),
            bind: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeymapConfig {
    pub global: ScopeKeymapConfig<GlobalAction>,
    pub normal: ScopeKeymapConfig<NormalAction>,
    pub window: ScopeKeymapConfig<WindowAction>,
    pub tree: ScopeKeymapConfig<TreeAction>,
    pub content: ScopeKeymapConfig<ContentAction>,
    pub heatmap: ScopeKeymapConfig<ContentAction>,
    pub attributes: ScopeKeymapConfig<AttributesAction>,
    pub multichart: ScopeKeymapConfig<MultiChartAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveKeymaps {
    pub global: Vec<KeyBinding<GlobalAction>>,
    pub normal: Vec<KeyBinding<NormalAction>>,
    pub window: Vec<KeyBinding<WindowAction>>,
    pub tree: Vec<KeyBinding<TreeAction>>,
    pub content: Vec<KeyBinding<ContentAction>>,
    pub heatmap: Vec<KeyBinding<ContentAction>>,
    pub attributes: Vec<KeyBinding<AttributesAction>>,
    pub multichart: Vec<KeyBinding<MultiChartAction>>,
}

impl Default for EffectiveKeymaps {
    fn default() -> Self {
        Self {
            global: Vec::new(),
            normal: default_normal_bindings(),
            window: default_window_bindings(),
            tree: default_tree_bindings(),
            content: default_content_bindings(),
            heatmap: default_heatmap_bindings(),
            attributes: default_attributes_bindings(),
            multichart: default_multichart_bindings(),
        }
    }
}

impl KeyPattern {
    pub fn to_key_event(self) -> KeyEvent {
        KeyEvent::new(self.code, self.modifiers)
    }

    pub fn matches(self, key: &KeyEvent) -> bool {
        let (code, modifiers) = normalize_key_parts(key.code, key.modifiers);
        if self.code != code {
            return false;
        }
        let required_non_shift = self.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT);
        let actual_non_shift = modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT);
        if required_non_shift != actual_non_shift {
            return false;
        }
        if matches!(self.code, KeyCode::Char(_)) {
            !self.modifiers.contains(KeyModifiers::SHIFT) || modifiers.contains(KeyModifiers::SHIFT)
        } else {
            self.modifiers == modifiers
        }
    }
}

impl fmt::Display for KeyPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt".to_string());
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift".to_string());
        }
        parts.push(key_code_name(self.code));
        write!(f, "{}", parts.join("+"))
    }
}

pub fn parse_key_pattern(key_spec: &str) -> Result<KeyPattern, String> {
    let normalized = key_spec.trim();
    if normalized.is_empty() {
        return Err("Key spec cannot be empty".to_string());
    }
    if normalized == "+" {
        return Ok(KeyPattern {
            code: KeyCode::Char('+'),
            modifiers: KeyModifiers::NONE,
        });
    }

    let parts = normalized
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(format!("Invalid key spec '{key_spec}'"));
    }

    let mut modifiers = KeyModifiers::NONE;
    let mut key_name = None;
    for part in parts {
        let lowered = part.to_ascii_lowercase();
        match lowered.as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" | "meta" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ if key_name.is_none() => key_name = Some(part.to_string()),
            _ => return Err(format!("Invalid key spec '{key_spec}'")),
        }
    }

    let Some(key_name) = key_name else {
        return Err(format!("Invalid key spec '{key_spec}'"));
    };

    let lowered = key_name.to_ascii_lowercase();
    let code = match lowered.as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "shift-tab" | "backtab" => {
            modifiers |= KeyModifiers::SHIFT;
            KeyCode::BackTab
        }
        "space" => KeyCode::Char(' '),
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "page-up" => KeyCode::PageUp,
        "pagedown" | "page-down" => KeyCode::PageDown,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        _ if key_name.chars().count() == 1 => {
            let ch = key_name
                .chars()
                .next()
                .ok_or_else(|| format!("Invalid key spec '{key_spec}'"))?;
            KeyCode::Char(ch)
        }
        _ => return Err(format!("Invalid key spec '{key_spec}'")),
    };

    let (code, modifiers) = normalize_key_parts(code, modifiers);
    Ok(KeyPattern { code, modifiers })
}

pub fn parse_simulated_key(key_spec: &str) -> Result<KeyEvent, AppError> {
    parse_key_pattern(key_spec)
        .map(KeyPattern::to_key_event)
        .map_err(AppError::InvalidCommand)
}

pub fn global_action(
    key: &KeyEvent,
    keymaps: &EffectiveKeymaps,
) -> Option<BoundAction<GlobalAction>> {
    find_binding(&keymaps.global, key)
}

pub fn normal_action(
    key: &KeyEvent,
    keymaps: &EffectiveKeymaps,
) -> Option<BoundAction<NormalAction>> {
    find_binding(&keymaps.normal, key)
}

pub fn window_action(
    key: &KeyEvent,
    keymaps: &EffectiveKeymaps,
) -> Option<BoundAction<WindowAction>> {
    find_binding(&keymaps.window, key)
}

pub fn tree_action(key: &KeyEvent, keymaps: &EffectiveKeymaps) -> Option<BoundAction<TreeAction>> {
    find_binding(&keymaps.tree, key)
}

pub fn content_action(
    key: &KeyEvent,
    keymaps: &EffectiveKeymaps,
) -> Option<BoundAction<ContentAction>> {
    find_binding(&keymaps.content, key)
}

pub fn heatmap_action(
    key: &KeyEvent,
    keymaps: &EffectiveKeymaps,
) -> Option<BoundAction<ContentAction>> {
    find_binding(&keymaps.heatmap, key)
}

pub fn attributes_action(
    key: &KeyEvent,
    keymaps: &EffectiveKeymaps,
) -> Option<BoundAction<AttributesAction>> {
    find_binding(&keymaps.attributes, key)
}

pub fn search_action(key: &KeyEvent) -> Option<SearchAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('w'), KeyModifiers::CONTROL)
        | (KeyCode::Backspace, KeyModifiers::CONTROL) => Some(SearchAction::ClearQuery),
        (KeyCode::Char(c), _) if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' => {
            Some(SearchAction::Insert(c))
        }
        (KeyCode::Backspace, _) => Some(SearchAction::Backspace),
        (KeyCode::Delete, _) => Some(SearchAction::Delete),
        (KeyCode::Left, _) | (KeyCode::Char('h'), _) => Some(SearchAction::Move(Direction::Left)),
        (KeyCode::Right, _) | (KeyCode::Char('l'), _) => Some(SearchAction::Move(Direction::Right)),
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => Some(SearchAction::Move(Direction::Up)),
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => Some(SearchAction::Move(Direction::Down)),
        (KeyCode::Enter, _) => Some(SearchAction::Submit),
        _ => None,
    }
}

pub fn multichart_action(
    key: &KeyEvent,
    keymaps: &EffectiveKeymaps,
) -> Option<BoundAction<MultiChartAction>> {
    find_binding(&keymaps.multichart, key)
}

pub fn command_action(key: &KeyEvent) -> Option<CommandAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Enter, _) => Some(CommandAction::Submit),
        (KeyCode::Esc, _) => Some(CommandAction::Cancel),
        (KeyCode::Tab, _) | (KeyCode::Down, _) => Some(CommandAction::SelectNextSuggestion),
        (KeyCode::BackTab, _) | (KeyCode::Up, _) => Some(CommandAction::SelectPrevSuggestion),
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => Some(CommandAction::SelectPrevHistory),
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => Some(CommandAction::SelectNextHistory),
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => Some(CommandAction::ClearWord),
        (KeyCode::Char('a'), KeyModifiers::CONTROL) => Some(CommandAction::MoveToStart),
        (KeyCode::Char('e'), KeyModifiers::CONTROL) => Some(CommandAction::MoveToEnd),
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => Some(CommandAction::Clear),
        (KeyCode::Home, _) => Some(CommandAction::MoveToStart),
        (KeyCode::End, _) => Some(CommandAction::MoveToEnd),
        (KeyCode::Backspace, _) => Some(CommandAction::Backspace),
        (KeyCode::Delete, _) => Some(CommandAction::Delete),
        (KeyCode::Left, _) => Some(CommandAction::MoveLeft),
        (KeyCode::Right, _) => Some(CommandAction::MoveRight),
        (KeyCode::Char(c), modifiers)
            if (modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT)
                && c.is_ascii()
                && !c.is_ascii_control() =>
        {
            Some(CommandAction::InsertChar(c))
        }
        _ => None,
    }
}

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

pub fn merge_keymap_config(config: &KeymapConfig) -> Result<EffectiveKeymaps, String> {
    Ok(EffectiveKeymaps {
        global: merge_scope(KeymapScope::Global, &[], &config.global)?,
        normal: merge_scope(
            KeymapScope::Normal,
            &default_normal_bindings(),
            &config.normal,
        )?,
        window: merge_scope(
            KeymapScope::Window,
            &default_window_bindings(),
            &config.window,
        )?,
        tree: merge_scope(KeymapScope::Tree, &default_tree_bindings(), &config.tree)?,
        content: merge_scope(
            KeymapScope::Content,
            &default_content_bindings(),
            &config.content,
        )?,
        heatmap: merge_scope(
            KeymapScope::Heatmap,
            &default_heatmap_bindings(),
            &config.heatmap,
        )?,
        attributes: merge_scope(
            KeymapScope::Attributes,
            &default_attributes_bindings(),
            &config.attributes,
        )?,
        multichart: merge_scope(
            KeymapScope::MultiChart,
            &default_multichart_bindings(),
            &config.multichart,
        )?,
    })
}

fn parse_action_code<T: Copy>(codes: &[ActionCode<T>], value: &str) -> Option<T> {
    let normalized = value.trim();
    codes
        .iter()
        .find(|entry| entry.code.eq_ignore_ascii_case(normalized))
        .map(|entry| entry.action)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportedActionCode {
    pub symbol: &'static str,
    pub code: &'static str,
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

fn find_binding<T: Clone>(bindings: &[KeyBinding<T>], key: &KeyEvent) -> Option<BoundAction<T>> {
    bindings
        .iter()
        .find(|binding| binding.key.matches(key))
        .map(|binding| binding.target.clone())
}

fn merge_scope<T: Clone>(
    scope: KeymapScope,
    defaults: &[KeyBinding<T>],
    config: &ScopeKeymapConfig<T>,
) -> Result<Vec<KeyBinding<T>>, String> {
    let mut merged = if config.clear_defaults {
        Vec::new()
    } else {
        defaults.to_vec()
    };
    merged.retain(|binding| !config.unbind.contains(&binding.key));
    for binding in &config.bind {
        if merged.iter().any(|existing| existing.key == binding.key) {
            return Err(format!(
                "Duplicate key '{}' in h5v.keymaps.{}",
                binding.key,
                scope.as_str()
            ));
        }
        merged.push(binding.clone());
    }
    Ok(merged)
}

fn normalize_key_parts(code: KeyCode, modifiers: KeyModifiers) -> (KeyCode, KeyModifiers) {
    let mut modifiers =
        modifiers & (KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT);
    let code = match code {
        KeyCode::BackTab => {
            modifiers |= KeyModifiers::SHIFT;
            KeyCode::BackTab
        }
        KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
        other => other,
    };
    (code, modifiers)
}

fn key_code_name(code: KeyCode) -> String {
    match code {
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "Tab".to_string(),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        _ => "Key".to_string(),
    }
}

fn binding<T>(key: &str, target: BoundAction<T>) -> KeyBinding<T> {
    KeyBinding {
        key: match parse_key_pattern(key) {
            Ok(key) => key,
            Err(error) => {
                eprintln!("Invalid built-in key binding '{key}': {error}");
                KeyPattern {
                    code: KeyCode::F(24),
                    modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT,
                }
            }
        },
        target,
        description: None,
    }
}

fn default_normal_bindings() -> Vec<KeyBinding<NormalAction>> {
    vec![
        binding(":", BoundAction::Action(NormalAction::EnterCommand)),
        binding(".", BoundAction::Action(NormalAction::RepeatCommand)),
        binding("/", BoundAction::Action(NormalAction::EnterSearch)),
        binding("q", BoundAction::Action(NormalAction::Quit)),
        binding("Ctrl+c", BoundAction::Action(NormalAction::Quit)),
        binding("Tab", BoundAction::Action(NormalAction::ToggleContentMode)),
        binding("?", BoundAction::Action(NormalAction::ShowHelp)),
        binding("M", BoundAction::Action(NormalAction::ToggleMultiChart)),
        binding("s", BoundAction::Action(NormalAction::ToggleTreeView)),
        binding("Ctrl+r", BoundAction::Action(NormalAction::ReloadFile)),
        binding(
            "Shift+Right",
            BoundAction::Action(NormalAction::Focus(Direction::Right)),
        ),
        binding(
            "Shift+Left",
            BoundAction::Action(NormalAction::Focus(Direction::Left)),
        ),
        binding(
            "Shift+Down",
            BoundAction::Action(NormalAction::Focus(Direction::Down)),
        ),
        binding(
            "Shift+Up",
            BoundAction::Action(NormalAction::Focus(Direction::Up)),
        ),
        binding(
            "Ctrl+w",
            BoundAction::Action(NormalAction::StartWindowChord),
        ),
        binding(
            "Ctrl+a",
            BoundAction::Action(NormalAction::ChangeSelectedIndex(1)),
        ),
        binding(
            "Ctrl+x",
            BoundAction::Action(NormalAction::ChangeSelectedIndex(-1)),
        ),
        binding("x", BoundAction::Action(NormalAction::ChangeX(1))),
        binding("X", BoundAction::Action(NormalAction::ChangeX(-1))),
        binding("r", BoundAction::Action(NormalAction::ChangeRow(1))),
        binding("R", BoundAction::Action(NormalAction::ChangeRow(-1))),
        binding("c", BoundAction::Action(NormalAction::ChangeCol(1))),
        binding("C", BoundAction::Action(NormalAction::ChangeCol(-1))),
        binding(
            "]",
            BoundAction::Action(NormalAction::ChangeSelectedDimension(1)),
        ),
        binding(
            "Alt+Right",
            BoundAction::Action(NormalAction::ChangeSelectedDimension(1)),
        ),
        binding(
            "[",
            BoundAction::Action(NormalAction::ChangeSelectedDimension(-1)),
        ),
        binding(
            "Alt+Left",
            BoundAction::Action(NormalAction::ChangeSelectedDimension(-1)),
        ),
        binding(
            "Alt+Up",
            BoundAction::Action(NormalAction::ChangeSelectedIndex(-1)),
        ),
        binding(
            "Alt+Down",
            BoundAction::Action(NormalAction::ChangeSelectedIndex(1)),
        ),
        binding(
            "Alt+PageUp",
            BoundAction::Action(NormalAction::ChangeSelectedIndex(-10)),
        ),
        binding(
            "Alt+PageDown",
            BoundAction::Action(NormalAction::ChangeSelectedIndex(10)),
        ),
        binding(
            "Ctrl+Up",
            BoundAction::Action(NormalAction::Scroll(Direction::Up, 1)),
        ),
        binding(
            "Ctrl+Down",
            BoundAction::Action(NormalAction::Scroll(Direction::Down, 1)),
        ),
        binding(
            "Ctrl+Right",
            BoundAction::Action(NormalAction::Scroll(Direction::Right, 1)),
        ),
        binding(
            "Ctrl+Left",
            BoundAction::Action(NormalAction::Scroll(Direction::Left, 1)),
        ),
        binding(
            "PageDown",
            BoundAction::Action(NormalAction::Scroll(Direction::Down, 20)),
        ),
        binding(
            "PageUp",
            BoundAction::Action(NormalAction::Scroll(Direction::Up, 20)),
        ),
    ]
}

fn default_window_bindings() -> Vec<KeyBinding<WindowAction>> {
    vec![
        binding(
            "h",
            BoundAction::Action(WindowAction::Focus(Direction::Left)),
        ),
        binding(
            "Left",
            BoundAction::Action(WindowAction::Focus(Direction::Left)),
        ),
        binding(
            "j",
            BoundAction::Action(WindowAction::Focus(Direction::Down)),
        ),
        binding(
            "Down",
            BoundAction::Action(WindowAction::Focus(Direction::Down)),
        ),
        binding("k", BoundAction::Action(WindowAction::Focus(Direction::Up))),
        binding(
            "Up",
            BoundAction::Action(WindowAction::Focus(Direction::Up)),
        ),
        binding(
            "l",
            BoundAction::Action(WindowAction::Focus(Direction::Right)),
        ),
        binding(
            "Right",
            BoundAction::Action(WindowAction::Focus(Direction::Right)),
        ),
        binding("o", BoundAction::Action(WindowAction::ToggleTreeView)),
    ]
}

fn default_tree_bindings() -> Vec<KeyBinding<TreeAction>> {
    vec![
        binding("Up", BoundAction::Action(TreeAction::MoveUp(1))),
        binding("k", BoundAction::Action(TreeAction::MoveUp(1))),
        binding("K", BoundAction::Action(TreeAction::MoveUp(1))),
        binding("Down", BoundAction::Action(TreeAction::MoveDown(1))),
        binding("j", BoundAction::Action(TreeAction::MoveDown(1))),
        binding("J", BoundAction::Action(TreeAction::MoveDown(1))),
        binding("u", BoundAction::Action(TreeAction::MoveUp(10))),
        binding("Ctrl+u", BoundAction::Action(TreeAction::MoveUp(10))),
        binding("Ctrl+d", BoundAction::Action(TreeAction::MoveDown(10))),
        binding("g", BoundAction::Action(TreeAction::MoveTop)),
        binding("Home", BoundAction::Action(TreeAction::MoveTop)),
        binding("G", BoundAction::Action(TreeAction::MoveBottom)),
        binding("End", BoundAction::Action(TreeAction::MoveBottom)),
        binding("h", BoundAction::Action(TreeAction::Collapse)),
        binding("H", BoundAction::Action(TreeAction::Collapse)),
        binding("Left", BoundAction::Action(TreeAction::Collapse)),
        binding("l", BoundAction::Action(TreeAction::Expand)),
        binding("L", BoundAction::Action(TreeAction::Expand)),
        binding("Right", BoundAction::Action(TreeAction::Expand)),
        binding("Enter", BoundAction::Action(TreeAction::Toggle)),
        binding("Space", BoundAction::Action(TreeAction::Toggle)),
        binding("m", BoundAction::Action(TreeAction::AddToMultiChart)),
    ]
}

fn default_content_bindings() -> Vec<KeyBinding<ContentAction>> {
    vec![
        binding(
            "Left",
            BoundAction::Action(ContentAction::Move(Direction::Left, 1)),
        ),
        binding(
            "h",
            BoundAction::Action(ContentAction::Move(Direction::Left, 1)),
        ),
        binding(
            "Right",
            BoundAction::Action(ContentAction::Move(Direction::Right, 1)),
        ),
        binding(
            "l",
            BoundAction::Action(ContentAction::Move(Direction::Right, 1)),
        ),
        binding(
            "Up",
            BoundAction::Action(ContentAction::Move(Direction::Up, 1)),
        ),
        binding(
            "k",
            BoundAction::Action(ContentAction::Move(Direction::Up, 1)),
        ),
        binding(
            "Down",
            BoundAction::Action(ContentAction::Move(Direction::Down, 1)),
        ),
        binding(
            "j",
            BoundAction::Action(ContentAction::Move(Direction::Down, 1)),
        ),
        binding(
            "Ctrl+u",
            BoundAction::Action(ContentAction::Move(Direction::Up, 10)),
        ),
        binding(
            "Ctrl+d",
            BoundAction::Action(ContentAction::Move(Direction::Down, 10)),
        ),
        binding("Enter", BoundAction::Action(ContentAction::Edit)),
        binding("e", BoundAction::Action(ContentAction::Edit)),
        binding("y", BoundAction::Action(ContentAction::Copy)),
    ]
}

fn default_heatmap_bindings() -> Vec<KeyBinding<ContentAction>> {
    vec![
        binding("z", BoundAction::Action(ContentAction::HeatmapZoomIn)),
        binding("Z", BoundAction::Action(ContentAction::HeatmapZoomOut)),
        binding("0", BoundAction::Action(ContentAction::HeatmapResetView)),
        binding(
            "v",
            BoundAction::Action(ContentAction::HeatmapClearSelection),
        ),
        binding(
            "H",
            BoundAction::Action(ContentAction::HeatmapPan(Direction::Left)),
        ),
        binding(
            "L",
            BoundAction::Action(ContentAction::HeatmapPan(Direction::Right)),
        ),
        binding(
            "K",
            BoundAction::Action(ContentAction::HeatmapPan(Direction::Up)),
        ),
        binding(
            "J",
            BoundAction::Action(ContentAction::HeatmapPan(Direction::Down)),
        ),
    ]
}

fn default_attributes_bindings() -> Vec<KeyBinding<AttributesAction>> {
    vec![
        binding(
            "Up",
            BoundAction::Action(AttributesAction::Move(Direction::Up, 1)),
        ),
        binding(
            "k",
            BoundAction::Action(AttributesAction::Move(Direction::Up, 1)),
        ),
        binding(
            "Down",
            BoundAction::Action(AttributesAction::Move(Direction::Down, 1)),
        ),
        binding(
            "j",
            BoundAction::Action(AttributesAction::Move(Direction::Down, 1)),
        ),
        binding(
            "Ctrl+u",
            BoundAction::Action(AttributesAction::Move(Direction::Up, 10)),
        ),
        binding(
            "Ctrl+d",
            BoundAction::Action(AttributesAction::Move(Direction::Down, 10)),
        ),
        binding(
            "PageUp",
            BoundAction::Action(AttributesAction::Move(Direction::Up, 10)),
        ),
        binding(
            "PageDown",
            BoundAction::Action(AttributesAction::Move(Direction::Down, 10)),
        ),
        binding(
            "Left",
            BoundAction::Action(AttributesAction::Move(Direction::Left, 1)),
        ),
        binding(
            "h",
            BoundAction::Action(AttributesAction::Move(Direction::Left, 1)),
        ),
        binding(
            "Right",
            BoundAction::Action(AttributesAction::Move(Direction::Right, 1)),
        ),
        binding(
            "l",
            BoundAction::Action(AttributesAction::Move(Direction::Right, 1)),
        ),
        binding("Enter", BoundAction::Action(AttributesAction::Edit)),
        binding("e", BoundAction::Action(AttributesAction::Edit)),
        binding("y", BoundAction::Action(AttributesAction::Copy)),
        binding("a", BoundAction::Action(AttributesAction::Create)),
        binding("d", BoundAction::Action(AttributesAction::Delete)),
        binding("Delete", BoundAction::Action(AttributesAction::Delete)),
    ]
}

fn default_multichart_bindings() -> Vec<KeyBinding<MultiChartAction>> {
    vec![
        binding(":", BoundAction::Action(MultiChartAction::EnterCommand)),
        binding("Esc", BoundAction::Action(MultiChartAction::Exit)),
        binding("M", BoundAction::Action(MultiChartAction::Exit)),
        binding("q", BoundAction::Action(MultiChartAction::Quit)),
        binding("Shift+Up", BoundAction::Action(MultiChartAction::ZoomIn)),
        binding("+", BoundAction::Action(MultiChartAction::ZoomIn)),
        binding("=", BoundAction::Action(MultiChartAction::ZoomIn)),
        binding("Shift+Down", BoundAction::Action(MultiChartAction::ZoomOut)),
        binding("-", BoundAction::Action(MultiChartAction::ZoomOut)),
        binding("Shift+Left", BoundAction::Action(MultiChartAction::PanLeft)),
        binding("h", BoundAction::Action(MultiChartAction::PanLeft)),
        binding(
            "Shift+Right",
            BoundAction::Action(MultiChartAction::PanRight),
        ),
        binding("l", BoundAction::Action(MultiChartAction::PanRight)),
        binding("c", BoundAction::Action(MultiChartAction::ClearZoom)),
        binding("C", BoundAction::Action(MultiChartAction::ClearAll)),
        binding(
            "e",
            BoundAction::Action(MultiChartAction::EditSelectedExpression),
        ),
        binding("?", BoundAction::Action(MultiChartAction::ShowHelp)),
        binding(
            "Enter",
            BoundAction::Action(MultiChartAction::OpenExpressionPrompt),
        ),
        binding(
            "Space",
            BoundAction::Action(MultiChartAction::ToggleSelectedVisible),
        ),
        binding(
            "v",
            BoundAction::Action(MultiChartAction::ToggleSelectedVisible),
        ),
        binding(
            "Delete",
            BoundAction::Action(MultiChartAction::DeleteSelected),
        ),
        binding(
            "Backspace",
            BoundAction::Action(MultiChartAction::DeleteSelected),
        ),
        binding("d", BoundAction::Action(MultiChartAction::DeleteSelected)),
        binding("Down", BoundAction::Action(MultiChartAction::MoveDown)),
        binding("j", BoundAction::Action(MultiChartAction::MoveDown)),
        binding("Up", BoundAction::Action(MultiChartAction::MoveUp)),
        binding("k", BoundAction::Action(MultiChartAction::MoveUp)),
    ]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_shift_tab_normalizes_to_backtab() {
        let key = parse_key_pattern("shift+tab").expect("shift+tab key");
        assert_eq!(key.code, KeyCode::BackTab);
        assert!(key.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn multichart_enter_opens_expression_prompt() {
        let keymaps = EffectiveKeymaps::default();
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            multichart_action(&key, &keymaps),
            Some(BoundAction::Action(MultiChartAction::OpenExpressionPrompt))
        );
    }

    #[test]
    fn multichart_e_edits_selected_expression() {
        let keymaps = EffectiveKeymaps::default();
        let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        assert_eq!(
            multichart_action(&key, &keymaps),
            Some(BoundAction::Action(
                MultiChartAction::EditSelectedExpression
            ))
        );
    }

    #[test]
    fn multichart_space_toggles_selected_visibility() {
        let keymaps = EffectiveKeymaps::default();
        let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(
            multichart_action(&key, &keymaps),
            Some(BoundAction::Action(MultiChartAction::ToggleSelectedVisible))
        );
    }

    #[test]
    fn multichart_question_mark_opens_help() {
        let keymaps = EffectiveKeymaps::default();
        let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(
            multichart_action(&key, &keymaps),
            Some(BoundAction::Action(MultiChartAction::ShowHelp))
        );
    }

    #[test]
    fn custom_heatmap_binding_overrides_default_after_unbind() {
        let mut config = KeymapConfig::default();
        config
            .heatmap
            .unbind
            .push(parse_key_pattern("z").expect("z"));
        config.heatmap.bind.push(KeyBinding {
            key: parse_key_pattern("Ctrl+z").expect("ctrl+z"),
            target: BoundAction::Action(ContentAction::HeatmapZoomIn),
            description: None,
        });
        let effective = merge_keymap_config(&config).expect("merged keymaps");
        assert!(heatmap_action(
            &KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
            &effective
        )
        .is_none());
        assert_eq!(
            heatmap_action(
                &KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
                &effective
            ),
            Some(BoundAction::Action(ContentAction::HeatmapZoomIn))
        );
    }
}
