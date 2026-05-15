use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

mod catalogs;
mod defaults;
mod parse;

pub use catalogs::{
    exported_action_codes, exported_mode_codes, is_valid_action_name_for_scope,
    parse_attributes_action_name, parse_content_action_name, parse_global_action_name,
    parse_multichart_action_name, parse_normal_action_name, parse_tree_action_name,
    parse_window_action_name,
};
use defaults::{
    default_attributes_bindings, default_content_bindings, default_heatmap_bindings,
    default_multichart_bindings, default_normal_bindings, default_tree_bindings,
    default_window_bindings,
};
pub use parse::{parse_key_pattern, parse_simulated_key};

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
    FitAll,
    FitSelected,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportedActionCode {
    pub symbol: &'static str,
    pub code: &'static str,
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
    fn multichart_zoom_keys_match_defaults() {
        let keymaps = EffectiveKeymaps::default();
        assert_eq!(
            multichart_action(
                &KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
                &keymaps
            ),
            Some(BoundAction::Action(MultiChartAction::ZoomIn))
        );
        assert_eq!(
            multichart_action(
                &KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::SHIFT),
                &keymaps
            ),
            Some(BoundAction::Action(MultiChartAction::ZoomOut))
        );
        assert_eq!(
            multichart_action(
                &KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
                &keymaps
            ),
            Some(BoundAction::Action(MultiChartAction::ClearZoom))
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
