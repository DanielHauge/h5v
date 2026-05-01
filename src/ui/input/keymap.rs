use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::mchart::BuiltinDerivedOp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
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
    Exit,
    Quit,
    ZoomIn,
    ZoomOut,
    PanLeft,
    PanRight,
    ClearZoom,
    DeleteSelected,
    ClearAll,
    ToggleSelectedVisible,
    ToggleMarkedBase,
    OpenExpressionPrompt,
    CreateDerived(BuiltinDerivedOp),
    MoveUp,
    MoveDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAction {
    Submit,
    Cancel,
    Complete,
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

pub fn normal_action(key: &KeyEvent) -> Option<NormalAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Char(':'), _) => Some(NormalAction::EnterCommand),
        (KeyCode::Char('.'), _) => Some(NormalAction::RepeatCommand),
        (KeyCode::Char('/'), _) => Some(NormalAction::EnterSearch),
        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            Some(NormalAction::Quit)
        }
        (KeyCode::Tab, _) => Some(NormalAction::ToggleContentMode),
        (KeyCode::Char('?'), _) => Some(NormalAction::ShowHelp),
        (KeyCode::Char('M'), _) => Some(NormalAction::ToggleMultiChart),
        (KeyCode::Char('s'), _) => Some(NormalAction::ToggleTreeView),
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => Some(NormalAction::ReloadFile),
        (KeyCode::Right, KeyModifiers::SHIFT) => Some(NormalAction::Focus(Direction::Right)),
        (KeyCode::Left, KeyModifiers::SHIFT) => Some(NormalAction::Focus(Direction::Left)),
        (KeyCode::Down, KeyModifiers::SHIFT) => Some(NormalAction::Focus(Direction::Down)),
        (KeyCode::Up, KeyModifiers::SHIFT) => Some(NormalAction::Focus(Direction::Up)),
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => Some(NormalAction::StartWindowChord),
        (KeyCode::Char('a'), KeyModifiers::CONTROL) => Some(NormalAction::ChangeSelectedIndex(1)),
        (KeyCode::Char('x'), KeyModifiers::CONTROL) => Some(NormalAction::ChangeSelectedIndex(-1)),
        (KeyCode::Char('x'), _) => Some(NormalAction::ChangeX(1)),
        (KeyCode::Char('X'), _) => Some(NormalAction::ChangeX(-1)),
        (KeyCode::Char('r'), _) => Some(NormalAction::ChangeRow(1)),
        (KeyCode::Char('R'), _) => Some(NormalAction::ChangeRow(-1)),
        (KeyCode::Char('c'), _) => Some(NormalAction::ChangeCol(1)),
        (KeyCode::Char('C'), _) => Some(NormalAction::ChangeCol(-1)),
        (KeyCode::Char(']'), _) | (KeyCode::Right, KeyModifiers::ALT) => {
            Some(NormalAction::ChangeSelectedDimension(1))
        }
        (KeyCode::Char('['), _) | (KeyCode::Left, KeyModifiers::ALT) => {
            Some(NormalAction::ChangeSelectedDimension(-1))
        }
        (KeyCode::Up, KeyModifiers::ALT) => Some(NormalAction::ChangeSelectedIndex(-1)),
        (KeyCode::Down, KeyModifiers::ALT) => Some(NormalAction::ChangeSelectedIndex(1)),
        (KeyCode::PageUp, KeyModifiers::ALT) => Some(NormalAction::ChangeSelectedIndex(-10)),
        (KeyCode::PageDown, KeyModifiers::ALT) => Some(NormalAction::ChangeSelectedIndex(10)),
        (KeyCode::Up, KeyModifiers::CONTROL) => Some(NormalAction::Scroll(Direction::Up, 1)),
        (KeyCode::Down, KeyModifiers::CONTROL) => Some(NormalAction::Scroll(Direction::Down, 1)),
        (KeyCode::Right, KeyModifiers::CONTROL) => Some(NormalAction::Scroll(Direction::Right, 1)),
        (KeyCode::Left, KeyModifiers::CONTROL) => Some(NormalAction::Scroll(Direction::Left, 1)),
        (KeyCode::PageDown, _) => Some(NormalAction::Scroll(Direction::Down, 20)),
        (KeyCode::PageUp, _) => Some(NormalAction::Scroll(Direction::Up, 20)),
        _ => None,
    }
}

pub fn window_action(key: &KeyEvent) -> Option<WindowAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('h'), _) | (KeyCode::Left, _) => Some(WindowAction::Focus(Direction::Left)),
        (KeyCode::Char('j'), _) | (KeyCode::Down, _) => Some(WindowAction::Focus(Direction::Down)),
        (KeyCode::Char('k'), _) | (KeyCode::Up, _) => Some(WindowAction::Focus(Direction::Up)),
        (KeyCode::Char('l'), _) | (KeyCode::Right, _) => {
            Some(WindowAction::Focus(Direction::Right))
        }
        (KeyCode::Char('o'), _) => Some(WindowAction::ToggleTreeView),
        _ => None,
    }
}

pub fn tree_action(key: &KeyEvent) -> Option<TreeAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) | (KeyCode::Char('K'), _) => {
            Some(TreeAction::MoveUp(1))
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) | (KeyCode::Char('J'), _) => {
            Some(TreeAction::MoveDown(1))
        }
        (KeyCode::Char('u'), KeyModifiers::NONE) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            Some(TreeAction::MoveUp(10))
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => Some(TreeAction::MoveDown(10)),
        (KeyCode::Char('g'), _) | (KeyCode::Home, _) => Some(TreeAction::MoveTop),
        (KeyCode::Char('G'), _) | (KeyCode::End, _) => Some(TreeAction::MoveBottom),
        (KeyCode::Char('h'), _) | (KeyCode::Char('H'), _) | (KeyCode::Left, _) => {
            Some(TreeAction::Collapse)
        }
        (KeyCode::Char('l'), _) | (KeyCode::Char('L'), _) | (KeyCode::Right, _) => {
            Some(TreeAction::Expand)
        }
        (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => Some(TreeAction::Toggle),
        (KeyCode::Char('m'), _) => Some(TreeAction::AddToMultiChart),
        _ => None,
    }
}

pub fn content_action(key: &KeyEvent) -> Option<ContentAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Left, _) | (KeyCode::Char('h'), _) => {
            Some(ContentAction::Move(Direction::Left, 1))
        }
        (KeyCode::Right, _) | (KeyCode::Char('l'), _) => {
            Some(ContentAction::Move(Direction::Right, 1))
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => Some(ContentAction::Move(Direction::Up, 1)),
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
            Some(ContentAction::Move(Direction::Down, 1))
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => Some(ContentAction::Move(Direction::Up, 10)),
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            Some(ContentAction::Move(Direction::Down, 10))
        }
        (KeyCode::Enter, _) | (KeyCode::Char('e'), _) => Some(ContentAction::Edit),
        (KeyCode::Char('y'), _) => Some(ContentAction::Copy),
        _ => None,
    }
}

pub fn attributes_action(key: &KeyEvent) -> Option<AttributesAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
            Some(AttributesAction::Move(Direction::Up, 1))
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
            Some(AttributesAction::Move(Direction::Down, 1))
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            Some(AttributesAction::Move(Direction::Up, 10))
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            Some(AttributesAction::Move(Direction::Down, 10))
        }
        (KeyCode::Left, _) | (KeyCode::Char('h'), _) => {
            Some(AttributesAction::Move(Direction::Left, 1))
        }
        (KeyCode::Right, _) | (KeyCode::Char('l'), _) => {
            Some(AttributesAction::Move(Direction::Right, 1))
        }
        (KeyCode::Enter, _) | (KeyCode::Char('e'), _) => Some(AttributesAction::Edit),
        (KeyCode::Char('y'), _) => Some(AttributesAction::Copy),
        (KeyCode::Char('a'), _) => Some(AttributesAction::Create),
        (KeyCode::Char('d'), _) | (KeyCode::Delete, _) => Some(AttributesAction::Delete),
        _ => None,
    }
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

pub fn multichart_action(key: &KeyEvent) -> Option<MultiChartAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) | (KeyCode::Char('M'), _) => Some(MultiChartAction::Exit),
        (KeyCode::Char('q'), _) => Some(MultiChartAction::Quit),
        (KeyCode::Up, KeyModifiers::SHIFT) | (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => {
            Some(MultiChartAction::ZoomIn)
        }
        (KeyCode::Down, KeyModifiers::SHIFT) | (KeyCode::Char('-'), _) => {
            Some(MultiChartAction::ZoomOut)
        }
        (KeyCode::Left, KeyModifiers::SHIFT) | (KeyCode::Char('h'), _) => {
            Some(MultiChartAction::PanLeft)
        }
        (KeyCode::Right, KeyModifiers::SHIFT) | (KeyCode::Char('l'), _) => {
            Some(MultiChartAction::PanRight)
        }
        (KeyCode::Char('c'), _) => Some(MultiChartAction::ClearZoom),
        (KeyCode::Char('C'), _) => Some(MultiChartAction::ClearAll),
        (KeyCode::Char('e'), _) => Some(MultiChartAction::OpenExpressionPrompt),
        (KeyCode::Enter, _) | (KeyCode::Char('v'), _) => {
            Some(MultiChartAction::ToggleSelectedVisible)
        }
        (KeyCode::Char(' '), _) => Some(MultiChartAction::ToggleMarkedBase),
        (KeyCode::Char('D'), _) => Some(MultiChartAction::CreateDerived(
            BuiltinDerivedOp::Difference,
        )),
        (KeyCode::Char('S'), _) => Some(MultiChartAction::CreateDerived(BuiltinDerivedOp::Sum)),
        (KeyCode::Char('R'), _) => Some(MultiChartAction::CreateDerived(BuiltinDerivedOp::Ratio)),
        (KeyCode::Char('P'), _) => Some(MultiChartAction::CreateDerived(BuiltinDerivedOp::Product)),
        (KeyCode::Char('X'), _) => Some(MultiChartAction::CreateDerived(BuiltinDerivedOp::Xy)),
        (KeyCode::Delete, _) | (KeyCode::Backspace, _) | (KeyCode::Char('d'), _) => {
            Some(MultiChartAction::DeleteSelected)
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => Some(MultiChartAction::MoveDown),
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => Some(MultiChartAction::MoveUp),
        _ => None,
    }
}

pub fn command_action(key: &KeyEvent) -> Option<CommandAction> {
    match (key.code, key.modifiers) {
        (KeyCode::Enter, _) => Some(CommandAction::Submit),
        (KeyCode::Esc, _) => Some(CommandAction::Cancel),
        (KeyCode::Tab, _) => Some(CommandAction::Complete),
        (KeyCode::BackTab, _) | (KeyCode::Up, _) => Some(CommandAction::SelectPrevSuggestion),
        (KeyCode::Down, _) => Some(CommandAction::SelectNextSuggestion),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multichart_enter_toggles_selected_visibility() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            multichart_action(&key),
            Some(MultiChartAction::ToggleSelectedVisible)
        );
    }
}
