use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::{
    parse_key_pattern, AttributesAction, BoundAction, ContentAction, Direction, KeyBinding,
    KeyPattern, MultiChartAction, NormalAction, TreeAction, WindowAction,
};

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

pub(super) fn default_normal_bindings() -> Vec<KeyBinding<NormalAction>> {
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

pub(super) fn default_window_bindings() -> Vec<KeyBinding<WindowAction>> {
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

pub(super) fn default_tree_bindings() -> Vec<KeyBinding<TreeAction>> {
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

pub(super) fn default_content_bindings() -> Vec<KeyBinding<ContentAction>> {
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

pub(super) fn default_heatmap_bindings() -> Vec<KeyBinding<ContentAction>> {
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

pub(super) fn default_attributes_bindings() -> Vec<KeyBinding<AttributesAction>> {
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

pub(super) fn default_multichart_bindings() -> Vec<KeyBinding<MultiChartAction>> {
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
