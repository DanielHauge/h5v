use crate::error::AppError;
use crate::ui::mchart::BuiltinDerivedOp;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;

use super::{
    input::{handle_input_event, EventResult},
    state::AppState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Navigation,
    View,
    Selection,
    App,
    MultiChart,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
    Seek,
    Goto,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Focus,
    Mode,
    ToggleTree,
    Reload,
    X,
    Row,
    Col,
    Dim,
    Index,
    Help,
    Repeat,
    MultiChart,
    Press,
    Noop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandArgKind {
    UnsignedInt,
    Word,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandArgSpec {
    pub name: &'static str,
    pub kind: CommandArgKind,
    pub required: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandDescriptor {
    pub id: CommandId,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    #[allow(dead_code)]
    pub description: &'static str,
    #[allow(dead_code)]
    pub category: CommandCategory,
    #[allow(dead_code)]
    pub keybindings: &'static [&'static str],
    pub args: &'static [CommandArgSpec],
    pub handler: CommandHandler,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandArgValue {
    UnsignedInt(usize),
    Word(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub id: CommandId,
    pub raw_input: String,
    pub command_name: String,
    pub args: Vec<CommandArgValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandToken {
    pub value: String,
    pub quoted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupCommand {
    pub origin: String,
    pub command_text: String,
}

pub struct CommandState {
    pub command_buffer: String,
    pub cursor: usize,
    pub last_command: Option<CommandInvocation>,
    pub selected_suggestion: usize,
    pub history: VecDeque<String>,
    pub history_cursor: Option<usize>,
    pub history_draft: Option<String>,
}

pub type CommandHandler =
    for<'a> fn(&mut AppState<'a>, &CommandInvocation) -> Result<EventResult, AppError>;

const INDEX_ARG: CommandArgSpec = CommandArgSpec {
    name: "index",
    kind: CommandArgKind::UnsignedInt,
    required: true,
};

const OPTIONAL_AMOUNT_ARG: CommandArgSpec = CommandArgSpec {
    name: "amount",
    kind: CommandArgKind::UnsignedInt,
    required: false,
};

const TARGET_ARG: CommandArgSpec = CommandArgSpec {
    name: "target",
    kind: CommandArgKind::Word,
    required: true,
};

const DIRECTION_ARG: CommandArgSpec = CommandArgSpec {
    name: "direction",
    kind: CommandArgKind::Word,
    required: true,
};

const MODE_ARG: CommandArgSpec = CommandArgSpec {
    name: "mode",
    kind: CommandArgKind::Word,
    required: true,
};

const PATH_ARG: CommandArgSpec = CommandArgSpec {
    name: "path",
    kind: CommandArgKind::Word,
    required: true,
};

const OPTIONAL_COMMAND_ARG: CommandArgSpec = CommandArgSpec {
    name: "command",
    kind: CommandArgKind::Word,
    required: false,
};

const ACTION_ARG: CommandArgSpec = CommandArgSpec {
    name: "action",
    kind: CommandArgKind::Word,
    required: true,
};

const OPTIONAL_WORD_ARG: CommandArgSpec = CommandArgSpec {
    name: "arg",
    kind: CommandArgKind::Word,
    required: false,
};

const KEY_ARG_1: CommandArgSpec = CommandArgSpec {
    name: "key1",
    kind: CommandArgKind::Word,
    required: true,
};

const KEY_ARG_2: CommandArgSpec = CommandArgSpec {
    name: "key2",
    kind: CommandArgKind::Word,
    required: false,
};

const KEY_ARG_3: CommandArgSpec = CommandArgSpec {
    name: "key3",
    kind: CommandArgKind::Word,
    required: false,
};

const KEY_ARG_4: CommandArgSpec = CommandArgSpec {
    name: "key4",
    kind: CommandArgKind::Word,
    required: false,
};

const COMMAND_CATALOG: &[CommandDescriptor] = &[
    CommandDescriptor {
        id: CommandId::Seek,
        name: "seek",
        aliases: &[],
        description: "Jump to an absolute index in the current content view",
        category: CommandCategory::Navigation,
        keybindings: &[],
        args: &[INDEX_ARG],
        handler: handle_seek,
    },
    CommandDescriptor {
        id: CommandId::Goto,
        name: "goto",
        aliases: &["jump", "open"],
        description: "Select a dataset or group by HDF5 path",
        category: CommandCategory::Navigation,
        keybindings: &[],
        args: &[PATH_ARG],
        handler: handle_goto,
    },
    CommandDescriptor {
        id: CommandId::Up,
        name: "up",
        aliases: &["dec", "decrement"],
        description: "Move up by a relative amount",
        category: CommandCategory::Navigation,
        keybindings: &["Up", "k"],
        args: &[OPTIONAL_AMOUNT_ARG],
        handler: handle_up,
    },
    CommandDescriptor {
        id: CommandId::Down,
        name: "down",
        aliases: &["inc", "increment"],
        description: "Move down by a relative amount",
        category: CommandCategory::Navigation,
        keybindings: &["Down", "j"],
        args: &[OPTIONAL_AMOUNT_ARG],
        handler: handle_down,
    },
    CommandDescriptor {
        id: CommandId::Left,
        name: "left",
        aliases: &[],
        description: "Move left by a relative amount",
        category: CommandCategory::Navigation,
        keybindings: &["Left", "h"],
        args: &[OPTIONAL_AMOUNT_ARG],
        handler: handle_left,
    },
    CommandDescriptor {
        id: CommandId::Right,
        name: "right",
        aliases: &[],
        description: "Move right by a relative amount",
        category: CommandCategory::Navigation,
        keybindings: &["Right", "l"],
        args: &[OPTIONAL_AMOUNT_ARG],
        handler: handle_right,
    },
    CommandDescriptor {
        id: CommandId::PageUp,
        name: "page-up",
        aliases: &["pgup"],
        description: "Move up by a page",
        category: CommandCategory::Navigation,
        keybindings: &["PageUp", "Ctrl+u"],
        args: &[],
        handler: handle_page_up,
    },
    CommandDescriptor {
        id: CommandId::PageDown,
        name: "page-down",
        aliases: &["pgdown"],
        description: "Move down by a page",
        category: CommandCategory::Navigation,
        keybindings: &["PageDown", "Ctrl+d"],
        args: &[],
        handler: handle_page_down,
    },
    CommandDescriptor {
        id: CommandId::Focus,
        name: "focus",
        aliases: &[],
        description: "Focus a target pane",
        category: CommandCategory::View,
        keybindings: &["Shift+Arrows"],
        args: &[TARGET_ARG],
        handler: handle_focus,
    },
    CommandDescriptor {
        id: CommandId::Mode,
        name: "mode",
        aliases: &["view-mode"],
        description: "Switch between preview and matrix modes",
        category: CommandCategory::View,
        keybindings: &["Tab"],
        args: &[MODE_ARG],
        handler: handle_mode,
    },
    CommandDescriptor {
        id: CommandId::ToggleTree,
        name: "toggle-tree",
        aliases: &["tree"],
        description: "Show or hide the tree pane",
        category: CommandCategory::View,
        keybindings: &["s"],
        args: &[],
        handler: handle_toggle_tree,
    },
    CommandDescriptor {
        id: CommandId::Reload,
        name: "reload",
        aliases: &["refresh"],
        description: "Reload the current file",
        category: CommandCategory::App,
        keybindings: &["Ctrl+r"],
        args: &[],
        handler: handle_reload,
    },
    CommandDescriptor {
        id: CommandId::X,
        name: "x",
        aliases: &[],
        description: "Move the preview x-dimension selection",
        category: CommandCategory::Selection,
        keybindings: &["x", "X"],
        args: &[DIRECTION_ARG],
        handler: handle_x,
    },
    CommandDescriptor {
        id: CommandId::Row,
        name: "row",
        aliases: &[],
        description: "Move the matrix row-dimension selection",
        category: CommandCategory::Selection,
        keybindings: &["r", "R"],
        args: &[DIRECTION_ARG],
        handler: handle_row,
    },
    CommandDescriptor {
        id: CommandId::Col,
        name: "col",
        aliases: &["column"],
        description: "Move the matrix column-dimension selection",
        category: CommandCategory::Selection,
        keybindings: &["c", "C"],
        args: &[DIRECTION_ARG],
        handler: handle_col,
    },
    CommandDescriptor {
        id: CommandId::Dim,
        name: "dim",
        aliases: &["dimension"],
        description: "Move the selected dimension cursor",
        category: CommandCategory::Selection,
        keybindings: &["[", "]"],
        args: &[DIRECTION_ARG],
        handler: handle_dim,
    },
    CommandDescriptor {
        id: CommandId::Index,
        name: "index",
        aliases: &["selected-index"],
        description: "Move the selected index within the active dimension",
        category: CommandCategory::Selection,
        keybindings: &["Ctrl+a", "Ctrl+x", "Alt+Up/Down"],
        args: &[DIRECTION_ARG, OPTIONAL_AMOUNT_ARG],
        handler: handle_index,
    },
    CommandDescriptor {
        id: CommandId::Help,
        name: "help",
        aliases: &["?"],
        description: "Open help or show details for a command",
        category: CommandCategory::App,
        keybindings: &["?"],
        args: &[OPTIONAL_COMMAND_ARG],
        handler: handle_help,
    },
    CommandDescriptor {
        id: CommandId::Repeat,
        name: "repeat",
        aliases: &["again"],
        description: "Repeat the last successful command",
        category: CommandCategory::App,
        keybindings: &["."],
        args: &[],
        handler: handle_repeat,
    },
    CommandDescriptor {
        id: CommandId::MultiChart,
        name: "mchart",
        aliases: &["multichart"],
        description: "Control multichart from command mode: open, add, expr, derive, select, pan, zoom, clear, and more",
        category: CommandCategory::MultiChart,
        keybindings: &["M"],
        args: &[ACTION_ARG, OPTIONAL_WORD_ARG, OPTIONAL_WORD_ARG, OPTIONAL_WORD_ARG],
        handler: handle_mchart,
    },
    CommandDescriptor {
        id: CommandId::Press,
        name: "press",
        aliases: &["key", "keys"],
        description: "Simulate one or more key presses through the normal keymap dispatcher",
        category: CommandCategory::Input,
        keybindings: &[],
        args: &[KEY_ARG_1, KEY_ARG_2, KEY_ARG_3, KEY_ARG_4],
        handler: handle_press,
    },
];

pub fn command_catalog() -> &'static [CommandDescriptor] {
    COMMAND_CATALOG
}

pub fn find_command_descriptor(name: &str) -> Option<&'static CommandDescriptor> {
    let normalized = name.trim().to_ascii_lowercase();
    COMMAND_CATALOG.iter().find(|descriptor| {
        descriptor.name == normalized
            || descriptor
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(&normalized))
    })
}

pub fn find_command_descriptor_by_id(id: CommandId) -> Option<&'static CommandDescriptor> {
    COMMAND_CATALOG
        .iter()
        .find(|descriptor| descriptor.id == id)
}

impl CommandInvocation {
    pub fn noop(raw_input: impl Into<String>) -> Self {
        Self {
            id: CommandId::Noop,
            raw_input: raw_input.into(),
            command_name: "noop".to_string(),
            args: Vec::new(),
        }
    }

    pub fn is_noop(&self) -> bool {
        matches!(self.id, CommandId::Noop)
    }

    pub fn usize_arg(&self, index: usize) -> Result<usize, AppError> {
        match self.args.get(index) {
            Some(CommandArgValue::UnsignedInt(value)) => Ok(*value),
            Some(CommandArgValue::Word(_)) => Err(AppError::InvalidCommand(format!(
                "Argument {} for command '{}' must be a number",
                index + 1,
                self.command_name
            ))),
            None => Err(AppError::InvalidCommand(format!(
                "Missing argument {} for command '{}'",
                index + 1,
                self.command_name
            ))),
        }
    }

    pub fn usize_arg_or(&self, index: usize, default: usize) -> Result<usize, AppError> {
        match self.args.get(index) {
            Some(CommandArgValue::UnsignedInt(value)) => Ok(*value),
            Some(CommandArgValue::Word(_)) => Err(AppError::InvalidCommand(format!(
                "Argument {} for command '{}' must be a number",
                index + 1,
                self.command_name
            ))),
            None => Ok(default),
        }
    }

    pub fn word_arg(&self, index: usize) -> Result<&str, AppError> {
        match self.args.get(index) {
            Some(CommandArgValue::Word(value)) => Ok(value.as_str()),
            Some(CommandArgValue::UnsignedInt(_)) => Err(AppError::InvalidCommand(format!(
                "Argument {} for command '{}' must be a word",
                index + 1,
                self.command_name
            ))),
            None => Err(AppError::InvalidCommand(format!(
                "Missing argument {} for command '{}'",
                index + 1,
                self.command_name
            ))),
        }
    }

    pub fn word_arg_optional(&self, index: usize) -> Result<Option<&str>, AppError> {
        match self.args.get(index) {
            Some(CommandArgValue::Word(value)) => Ok(Some(value.as_str())),
            Some(CommandArgValue::UnsignedInt(_)) => Err(AppError::InvalidCommand(format!(
                "Argument {} for command '{}' must be a word",
                index + 1,
                self.command_name
            ))),
            None => Ok(None),
        }
    }
}

impl CommandState {
    pub fn parse_command(&self) -> Result<CommandInvocation, AppError> {
        parse_command_text(&self.command_buffer)
    }

    pub fn reset_suggestion_selection(&mut self) {
        self.selected_suggestion = 0;
    }

    pub fn begin_new_entry(&mut self) {
        self.command_buffer.clear();
        self.cursor = 0;
        self.reset_suggestion_selection();
        self.history_cursor = None;
        self.history_draft = None;
    }

    pub fn note_buffer_edited(&mut self) {
        self.reset_suggestion_selection();
        self.history_cursor = None;
        self.history_draft = None;
    }

    pub fn select_next_suggestion(&mut self) {
        let matches = command_matches(&self.command_buffer);
        if matches.is_empty() {
            self.selected_suggestion = 0;
        } else {
            self.selected_suggestion = (self.selected_suggestion + 1) % matches.len();
        }
    }

    pub fn select_previous_suggestion(&mut self) {
        let matches = command_matches(&self.command_buffer);
        if matches.is_empty() {
            self.selected_suggestion = 0;
        } else if self.selected_suggestion == 0 {
            self.selected_suggestion = matches.len() - 1;
        } else {
            self.selected_suggestion -= 1;
        }
    }

    pub fn apply_selected_completion(&mut self) -> bool {
        let Some(descriptor) =
            selected_command_descriptor(&self.command_buffer, self.selected_suggestion)
        else {
            return false;
        };

        let remainder = command_tail(&self.command_buffer)
            .map(str::trim_start)
            .unwrap_or_default();
        self.command_buffer = if remainder.is_empty() {
            if descriptor.args.is_empty() {
                descriptor.name.to_string()
            } else {
                format!("{} ", descriptor.name)
            }
        } else {
            format!("{} {}", descriptor.name, remainder)
        };
        self.cursor = self.command_buffer.len();
        self.selected_suggestion = 0;
        self.history_cursor = None;
        true
    }

    pub fn record_successful_command(&mut self, invocation: &CommandInvocation) {
        if invocation.is_noop() {
            return;
        }

        if invocation.id != CommandId::Repeat {
            self.last_command = Some(invocation.clone());
        }
        let raw = invocation.raw_input.trim();
        if raw.is_empty() {
            return;
        }
        if self.history.back().is_none_or(|existing| existing != raw) {
            self.history.push_back(raw.to_string());
        }
        while self.history.len() > 100 {
            self.history.pop_front();
        }
        self.history_cursor = None;
        self.history_draft = None;
    }

    pub fn history_status(&self) -> Option<(usize, usize)> {
        self.history_cursor
            .map(|cursor| (cursor.saturating_add(1), self.history.len()))
    }

    pub fn select_previous_history(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }

        if self.history_cursor.is_none() {
            self.history_draft = Some(self.command_buffer.clone());
        }

        let next_index = match self.history_cursor {
            Some(index) => index.saturating_sub(1),
            None => self.history.len() - 1,
        };
        self.history_cursor = Some(next_index);
        self.command_buffer = self.history[next_index].clone();
        self.cursor = self.command_buffer.len();
        self.reset_suggestion_selection();
        true
    }

    pub fn select_next_history(&mut self) -> bool {
        let Some(current_index) = self.history_cursor else {
            return false;
        };

        if current_index + 1 < self.history.len() {
            let next_index = current_index + 1;
            self.history_cursor = Some(next_index);
            self.command_buffer = self.history[next_index].clone();
        } else {
            self.history_cursor = None;
            self.command_buffer = self.history_draft.take().unwrap_or_default();
        }
        self.cursor = self.command_buffer.len();
        self.reset_suggestion_selection();
        true
    }
}

pub fn parse_command_text(command_text: &str) -> Result<CommandInvocation, AppError> {
    let trimmed = command_text.trim();
    if trimmed.is_empty() {
        return Ok(CommandInvocation::noop(trimmed));
    }

    if let Some(invocation) = parse_legacy_numeric_alias(trimmed)? {
        return Ok(invocation);
    }

    let tokens = tokenize_command_text(trimmed)?;
    let command_name = tokens
        .first()
        .map(|token| token.value.as_str())
        .ok_or_else(|| AppError::InvalidCommand("Command was empty".to_string()))?;
    let descriptor = find_command_descriptor(command_name).ok_or_else(|| {
        let known = command_catalog()
            .iter()
            .map(|descriptor| descriptor.name)
            .collect::<Vec<_>>()
            .join(", ");
        AppError::InvalidCommand(format!(
            "Unknown command '{}'. Known commands: {}",
            command_name, known
        ))
    })?;

    let args = parse_command_args(descriptor, &tokens[1..])?;
    Ok(CommandInvocation {
        id: descriptor.id,
        raw_input: trimmed.to_string(),
        command_name: descriptor.name.to_string(),
        args,
    })
}

pub fn execute_command(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    if command.is_noop() {
        return Ok(EventResult::Redraw);
    }

    let descriptor = find_command_descriptor_by_id(command.id).ok_or_else(|| {
        AppError::InvalidCommand(format!(
            "Command '{}' is not registered",
            command.command_name
        ))
    })?;
    (descriptor.handler)(state, command)
}

pub fn format_command_invocation(command: &CommandInvocation) -> String {
    if command.is_noop() {
        return String::new();
    }

    let command_name = find_command_descriptor_by_id(command.id)
        .map(|descriptor| descriptor.name)
        .unwrap_or(command.command_name.as_str());
    let args = command
        .args
        .iter()
        .map(format_command_arg)
        .collect::<Vec<_>>();

    if args.is_empty() {
        command_name.to_string()
    } else {
        format!("{} {}", command_name, args.join(" "))
    }
}

pub fn describe_command_invocation(command: &CommandInvocation) -> Option<&'static str> {
    find_command_descriptor_by_id(command.id).map(|descriptor| descriptor.description)
}

pub fn describe_command_descriptor(descriptor: &CommandDescriptor) -> String {
    let mut parts = vec![format!(
        "{} - {}",
        command_usage(descriptor),
        descriptor.description
    )];
    if !descriptor.aliases.is_empty() {
        parts.push(format!("aliases: {}", descriptor.aliases.join(", ")));
    }
    let keybindings = command_keybindings(descriptor);
    if !keybindings.is_empty() {
        parts.push(format!("keys: {keybindings}"));
    }
    parts.join(" | ")
}

pub fn parse_startup_commands(script: &str, origin: &str) -> Vec<StartupCommand> {
    let mut commands = Vec::new();
    let mut current = String::new();
    let mut line = 1usize;
    let mut segment = 1usize;
    let mut start_line = 1usize;
    let mut start_segment = 1usize;
    let mut in_quote = None;
    let mut escaped = false;

    for ch in script.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => {
                current.push(ch);
                escaped = true;
            }
            '"' | '\'' => {
                current.push(ch);
                if let Some(active_quote) = in_quote {
                    if active_quote == ch {
                        in_quote = None;
                    }
                } else {
                    in_quote = Some(ch);
                }
            }
            ';' if in_quote.is_none() => {
                push_startup_command(
                    &mut commands,
                    &mut current,
                    origin,
                    start_line,
                    start_segment,
                );
                segment += 1;
                start_line = line;
                start_segment = segment;
            }
            '\n' if in_quote.is_none() => {
                push_startup_command(
                    &mut commands,
                    &mut current,
                    origin,
                    start_line,
                    start_segment,
                );
                line += 1;
                segment = 1;
                start_line = line;
                start_segment = 1;
            }
            _ => current.push(ch),
        }
    }

    push_startup_command(
        &mut commands,
        &mut current,
        origin,
        start_line,
        start_segment,
    );
    commands
}

pub fn command_matches(buffer: &str) -> Vec<&'static CommandDescriptor> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return command_catalog().iter().collect();
    }

    if let Some(descriptor) = legacy_descriptor_for_input(trimmed) {
        return vec![descriptor];
    }

    let fragment = first_token(trimmed)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut matches = command_catalog()
        .iter()
        .filter(|descriptor| {
            descriptor.name.starts_with(&fragment)
                || descriptor
                    .aliases
                    .iter()
                    .any(|alias| alias.to_ascii_lowercase().starts_with(&fragment))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|descriptor| descriptor.name);
    matches
}

pub fn selected_command_descriptor(
    buffer: &str,
    selected_suggestion: usize,
) -> Option<&'static CommandDescriptor> {
    let matches = command_matches(buffer);
    if matches.is_empty() {
        None
    } else {
        Some(matches[selected_suggestion.min(matches.len() - 1)])
    }
}

pub fn current_command_descriptor(buffer: &str) -> Option<&'static CommandDescriptor> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return None;
    }
    legacy_descriptor_for_input(trimmed)
        .or_else(|| first_token(trimmed).and_then(find_command_descriptor))
}

pub fn command_usage(descriptor: &CommandDescriptor) -> String {
    let args = descriptor
        .args
        .iter()
        .map(|arg| {
            if arg.required {
                format!("<{}>", arg.name)
            } else {
                format!("[{}]", arg.name)
            }
        })
        .collect::<Vec<_>>();
    if args.is_empty() {
        descriptor.name.to_string()
    } else {
        format!("{} {}", descriptor.name, args.join(" "))
    }
}

pub fn command_keybindings(descriptor: &CommandDescriptor) -> String {
    descriptor.keybindings.join(", ")
}

fn parse_legacy_numeric_alias(command_text: &str) -> Result<Option<CommandInvocation>, AppError> {
    let Some(first) = command_text.chars().next() else {
        return Ok(Some(CommandInvocation::noop(command_text)));
    };

    if first == '+' || first == '-' {
        let amount_text = command_text[1..].trim();
        if amount_text.is_empty() {
            return Err(AppError::InvalidCommand(format!(
                "Expected a number after '{}'",
                first
            )));
        }
        let amount = parse_usize_arg(amount_text, "amount", command_text)?;
        let (id, name) = if first == '+' {
            (CommandId::Down, "down")
        } else {
            (CommandId::Up, "up")
        };
        return Ok(Some(CommandInvocation {
            id,
            raw_input: command_text.to_string(),
            command_name: name.to_string(),
            args: vec![CommandArgValue::UnsignedInt(amount)],
        }));
    }

    if command_text.chars().all(|c| c.is_ascii_digit()) {
        let index = parse_usize_arg(command_text, "index", command_text)?;
        return Ok(Some(CommandInvocation {
            id: CommandId::Seek,
            raw_input: command_text.to_string(),
            command_name: "seek".to_string(),
            args: vec![CommandArgValue::UnsignedInt(index)],
        }));
    }

    Ok(None)
}

fn legacy_descriptor_for_input(command_text: &str) -> Option<&'static CommandDescriptor> {
    let first = command_text.chars().next()?;
    if first == '+' {
        find_command_descriptor_by_id(CommandId::Down)
    } else if first == '-' {
        find_command_descriptor_by_id(CommandId::Up)
    } else if command_text.chars().all(|c| c.is_ascii_digit()) {
        find_command_descriptor_by_id(CommandId::Seek)
    } else {
        None
    }
}

fn parse_command_args(
    descriptor: &CommandDescriptor,
    tokens: &[CommandToken],
) -> Result<Vec<CommandArgValue>, AppError> {
    let required_args = descriptor.args.iter().filter(|arg| arg.required).count();
    if tokens.len() < required_args {
        return Err(AppError::InvalidCommand(format!(
            "Command '{}' expects {} argument(s)",
            descriptor.name, required_args
        )));
    }
    if tokens.len() > descriptor.args.len() {
        return Err(AppError::InvalidCommand(format!(
            "Command '{}' received too many arguments",
            descriptor.name
        )));
    }

    descriptor
        .args
        .iter()
        .zip(tokens.iter())
        .map(|(arg_spec, token)| match arg_spec.kind {
            CommandArgKind::UnsignedInt => {
                parse_usize_arg(&token.value, arg_spec.name, descriptor.name)
                    .map(CommandArgValue::UnsignedInt)
            }
            CommandArgKind::Word => Ok(CommandArgValue::Word(token.value.clone())),
        })
        .collect()
}

fn tokenize_command_text(command_text: &str) -> Result<Vec<CommandToken>, AppError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = None;
    let mut current_quoted = false;
    let mut escaped = false;

    for ch in command_text.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => {
                escaped = true;
            }
            '"' | '\'' => {
                if let Some(active_quote) = in_quote {
                    if ch == active_quote {
                        in_quote = None;
                    } else {
                        current.push(ch);
                    }
                } else {
                    in_quote = Some(ch);
                    current_quoted = true;
                }
            }
            c if c.is_whitespace() && in_quote.is_none() => {
                if !current.is_empty() {
                    tokens.push(CommandToken {
                        value: std::mem::take(&mut current),
                        quoted: current_quoted,
                    });
                    current_quoted = false;
                }
            }
            _ => current.push(ch),
        }
    }

    if escaped {
        current.push('\\');
    }

    if let Some(quote) = in_quote {
        return Err(AppError::InvalidCommand(format!(
            "Unterminated quoted argument starting with {}",
            quote
        )));
    }

    if !current.is_empty() {
        tokens.push(CommandToken {
            value: current,
            quoted: current_quoted,
        });
    }

    Ok(tokens)
}

fn handle_seek(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.set(command.usize_arg(0)?)
}

fn handle_goto(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.select_tree_node_by_path(command.word_arg(0)?)?;
    Ok(EventResult::Redraw)
}

fn handle_up(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.up(command.usize_arg_or(0, 1)?)
}

fn handle_down(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.down(command.usize_arg_or(0, 1)?)
}

fn handle_left(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.left(command.usize_arg_or(0, 1)? as isize)
}

fn handle_right(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.right(command.usize_arg_or(0, 1)? as isize)
}

fn handle_page_up(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.up(20)
}

fn handle_page_down(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.down(20)
}

fn handle_focus(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    match command.word_arg(0)?.to_ascii_lowercase().as_str() {
        "tree" => {
            if !state.show_tree_view {
                return Err(AppError::InvalidCommand(
                    "Cannot focus tree while the tree view is hidden".to_string(),
                ));
            }
            state.focus_tree_from_current();
            Ok(EventResult::Redraw)
        }
        "attributes" | "attrs" => {
            state.focus = super::state::Focus::Attributes;
            Ok(EventResult::Redraw)
        }
        "content" => {
            state.focus = super::state::Focus::Content;
            Ok(EventResult::Redraw)
        }
        target => Err(AppError::InvalidCommand(format!(
            "Unknown focus target '{}'. Expected tree, attributes, or content",
            target
        ))),
    }
}

fn handle_mode(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let requested_mode = match command.word_arg(0)?.to_ascii_lowercase().as_str() {
        "preview" => super::state::ContentShowMode::Preview,
        "matrix" => super::state::ContentShowMode::Matrix,
        mode => {
            return Err(AppError::InvalidCommand(format!(
                "Unknown content mode '{}'. Expected preview or matrix",
                mode
            )))
        }
    };

    let available = state.treeview[state.tree_view_cursor]
        .node
        .borrow_mut()
        .content_show_modes();
    if !available.contains(&requested_mode) {
        return Err(AppError::InvalidCommand(format!(
            "Mode '{}' is not available for the selected item",
            command.word_arg(0)?
        )));
    }
    state.content_mode = requested_mode;
    Ok(EventResult::Redraw)
}

fn handle_toggle_tree(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.toggle_tree_view();
    Ok(EventResult::Redraw)
}

fn handle_reload(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    Ok(EventResult::ReloadFile {
        write: !state.readonly,
    })
}

fn handle_x(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.change_x(parse_direction_delta(command.word_arg(0)?)?)
}

fn handle_row(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.change_row(parse_direction_delta(command.word_arg(0)?)?)
}

fn handle_col(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.change_col(parse_direction_delta(command.word_arg(0)?)?)
}

fn handle_dim(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.change_selected_dimension(parse_direction_delta(command.word_arg(0)?)?)
}

fn handle_index(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let delta = parse_direction_delta(command.word_arg(0)?)?;
    let amount = command.usize_arg_or(1, 1)? as isize;
    state.change_selected_index(delta.signum() * amount)
}

fn handle_mchart(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let action = command.word_arg(0)?.to_ascii_lowercase();
    match action.as_str() {
        "open" | "show" => {
            state.mode = super::state::Mode::MultiChart;
            Ok(EventResult::Redraw)
        }
        "close" | "hide" => {
            state.multi_chart.close_expression_prompt();
            state.mode = super::state::Mode::Normal;
            Ok(EventResult::Redraw)
        }
        "toggle" => {
            state.multi_chart.close_expression_prompt();
            state.mode = if matches!(state.mode, super::state::Mode::MultiChart) {
                super::state::Mode::Normal
            } else {
                super::state::Mode::MultiChart
            };
            Ok(EventResult::Redraw)
        }
        "add" => {
            if let Some(spec) = command.word_arg_optional(1)? {
                let file = state.file.clone();
                state
                    .multi_chart
                    .add_dataset_reference_command(spec, file.as_ref())
                    .map_err(AppError::InvalidCommand)?;
            } else {
                let Some((source, points)) = state.capture_multichart_item()? else {
                    return Err(AppError::InvalidCommand(
                        "The current selection is not previewable as a multichart dataset"
                            .to_string(),
                    ));
                };
                state.multi_chart.add_chart_item(source, points);
            }
            state.compute_tree_view();
            Ok(EventResult::Redraw)
        }
        "expr" | "expression" => {
            let expression = command.word_arg(1)?.to_string();
            let file = state.file.clone();
            state
                .multi_chart
                .create_expression_derived_command(expression, file.as_ref())
                .map_err(AppError::InvalidCommand)?;
            Ok(EventResult::Redraw)
        }
        "prompt" => {
            state.mode = super::state::Mode::MultiChart;
            state.multi_chart.open_expression_prompt();
            Ok(EventResult::Redraw)
        }
        "base" => match command.word_arg_optional(1)?.map(|arg| arg.to_ascii_lowercase()) {
            None => {
                state
                    .multi_chart
                    .toggle_marked_base()
                    .map_err(AppError::InvalidCommand)?;
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "toggle" => {
                state
                    .multi_chart
                    .toggle_marked_base()
                    .map_err(AppError::InvalidCommand)?;
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "clear" => {
                state.multi_chart.clear_marked_base();
                Ok(EventResult::Redraw)
            }
            Some(other) => Err(AppError::InvalidCommand(format!(
                "Unknown mchart base action '{}'. Expected toggle or clear",
                other
            ))),
        },
        "derive" => {
            let operation = parse_multichart_derived_op(command.word_arg(1)?)?;
            state
                .multi_chart
                .create_builtin_derived(operation)
                .map_err(AppError::InvalidCommand)?;
            Ok(EventResult::Redraw)
        }
        "select" | "move" => {
            let delta = parse_direction_delta(command.word_arg(1)?)?;
            let amount = parse_word_usize(command.word_arg_optional(2)?, 1, "mchart")?;
            for _ in 0..amount {
                if delta < 0 {
                    state.multi_chart.move_up();
                } else {
                    state.multi_chart.move_down();
                }
            }
            Ok(EventResult::Redraw)
        }
        "visible" | "visibility" => match command
            .word_arg_optional(1)?
            .map(|arg| arg.to_ascii_lowercase())
        {
            None => {
                state.multi_chart.toggle_selected_visible();
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "toggle" => {
                state.multi_chart.toggle_selected_visible();
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "show" => {
                state.multi_chart.set_selected_visible(true);
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "hide" => {
                state.multi_chart.set_selected_visible(false);
                Ok(EventResult::Redraw)
            }
            Some(other) => Err(AppError::InvalidCommand(format!(
                "Unknown mchart visibility action '{}'. Expected toggle, show, or hide",
                other
            ))),
        },
        "remove" | "delete" => {
            state.multi_chart.clear_selected();
            state.compute_tree_view();
            Ok(EventResult::Redraw)
        }
        "clear" => {
            match command.word_arg_optional(1)?.map(|arg| arg.to_ascii_lowercase()) {
                None => {
                    state.multi_chart.clear_all();
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }
                Some(action) if action == "all" => {
                    state.multi_chart.clear_all();
                    state.compute_tree_view();
                    Ok(EventResult::Redraw)
                }
                Some(action) if action == "zoom" => {
                    state.multi_chart.clear_zoom();
                    Ok(EventResult::Redraw)
                }
                Some(other) => Err(AppError::InvalidCommand(format!(
                    "Unknown mchart clear target '{}'. Expected all or zoom",
                    other
                ))),
            }
        }
        "zoom" => {
            let target = command.word_arg_optional(1)?.unwrap_or("reset");
            match target.to_ascii_lowercase().as_str() {
                "in" => {
                    state
                        .multi_chart
                        .zoom_in(parse_word_f64(command.word_arg_optional(2)?, 10.0, "mchart")?);
                    Ok(EventResult::Redraw)
                }
                "out" => {
                    state
                        .multi_chart
                        .zoom_out(parse_word_f64(command.word_arg_optional(2)?, 10.0, "mchart")?);
                    Ok(EventResult::Redraw)
                }
                "reset" | "clear" => {
                    state.multi_chart.clear_zoom();
                    Ok(EventResult::Redraw)
                }
                other => Err(AppError::InvalidCommand(format!(
                    "Unknown mchart zoom action '{}'. Expected in, out, or reset",
                    other
                ))),
            }
        }
        "pan" => {
            let direction = command.word_arg(1)?;
            let amount = parse_word_f64(command.word_arg_optional(2)?, 10.0, "mchart")?;
            match direction.to_ascii_lowercase().as_str() {
                "left" => {
                    state.multi_chart.pan_left(amount);
                    Ok(EventResult::Redraw)
                }
                "right" => {
                    state.multi_chart.pan_right(amount);
                    Ok(EventResult::Redraw)
                }
                other => Err(AppError::InvalidCommand(format!(
                    "Unknown mchart pan direction '{}'. Expected left or right",
                    other
                ))),
            }
        }
        other => Err(AppError::InvalidCommand(format!(
            "Unknown mchart action '{}'. Expected open, close, add, expr, derive, select, visible, remove, clear, zoom, or pan",
            other
        ))),
    }
}

fn handle_help(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let Some(CommandArgValue::Word(target)) = command.args.first() else {
        state.mode = super::state::Mode::Help;
        return Ok(EventResult::Redraw);
    };

    let descriptor = legacy_descriptor_for_input(target)
        .or_else(|| find_command_descriptor(target))
        .ok_or_else(|| {
            AppError::InvalidCommand(format!(
                "Unknown command '{}'. Try 'help' to open the command reference",
                target
            ))
        })?;

    Ok(EventResult::Toast(
        super::state::AppToast::Info(describe_command_descriptor(descriptor)),
        false,
    ))
}

fn handle_repeat(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let Some(last_command) = state.command_state.last_command.clone() else {
        return Err(AppError::InvalidCommand(
            "No previous command to repeat".to_string(),
        ));
    };
    execute_command(state, &last_command)
}

fn handle_press(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let mut last_result = EventResult::Redraw;
    let mut pressed_any = false;
    for index in 0..4 {
        let Some(key_spec) = command.word_arg_optional(index)? else {
            continue;
        };
        pressed_any = true;
        let key_event = parse_simulated_key(key_spec)?;
        let result = handle_input_event(state, Event::Key(key_event))?;
        match result {
            EventResult::Continue => {}
            EventResult::Quit
            | EventResult::ReloadFile { .. }
            | EventResult::Error(_)
            | EventResult::Toast(_, _)
            | EventResult::Copying => return Ok(result),
            EventResult::Redraw => last_result = EventResult::Redraw,
        }
    }

    if !pressed_any {
        return Err(AppError::InvalidCommand(
            "Command 'press' expects at least one key".to_string(),
        ));
    }

    Ok(last_result)
}

fn parse_usize_arg(token: &str, arg_name: &str, command_name: &str) -> Result<usize, AppError> {
    token.parse::<usize>().map_err(|_| {
        AppError::InvalidCommand(format!(
            "Invalid {} '{}' for command '{}'",
            arg_name, token, command_name
        ))
    })
}

fn parse_direction_delta(word: &str) -> Result<isize, AppError> {
    match word.to_ascii_lowercase().as_str() {
        "next" | "forward" | "right" | "down" => Ok(1),
        "prev" | "previous" | "back" | "left" | "up" => Ok(-1),
        other => Err(AppError::InvalidCommand(format!(
            "Unknown direction '{}'. Expected next or prev",
            other
        ))),
    }
}

fn parse_multichart_derived_op(word: &str) -> Result<BuiltinDerivedOp, AppError> {
    match word.to_ascii_lowercase().as_str() {
        "diff" | "difference" => Ok(BuiltinDerivedOp::Difference),
        "sum" => Ok(BuiltinDerivedOp::Sum),
        "ratio" => Ok(BuiltinDerivedOp::Ratio),
        "product" | "mul" | "multiply" => Ok(BuiltinDerivedOp::Product),
        "xy" | "x-y" | "pair" => Ok(BuiltinDerivedOp::Xy),
        other => Err(AppError::InvalidCommand(format!(
            "Unknown multichart derived operation '{}'. Expected difference, sum, ratio, product, or xy",
            other
        ))),
    }
}

fn parse_word_usize(
    word: Option<&str>,
    default: usize,
    command_name: &str,
) -> Result<usize, AppError> {
    match word {
        Some(word) => word.parse::<usize>().map_err(|_| {
            AppError::InvalidCommand(format!(
                "Invalid numeric argument '{}' for command '{}'",
                word, command_name
            ))
        }),
        None => Ok(default),
    }
}

fn parse_word_f64(word: Option<&str>, default: f64, command_name: &str) -> Result<f64, AppError> {
    match word {
        Some(word) => word.parse::<f64>().map_err(|_| {
            AppError::InvalidCommand(format!(
                "Invalid numeric argument '{}' for command '{}'",
                word, command_name
            ))
        }),
        None => Ok(default),
    }
}

fn parse_simulated_key(key_spec: &str) -> Result<KeyEvent, AppError> {
    let normalized = key_spec.trim();
    if normalized.is_empty() {
        return Err(AppError::InvalidCommand(
            "Key spec cannot be empty".to_string(),
        ));
    }

    let mut modifiers = KeyModifiers::NONE;
    let mut parts = normalized.split('+').peekable();
    let mut base = None;
    while let Some(part) = parts.next() {
        let part = part.trim();
        if part.is_empty() {
            return Err(AppError::InvalidCommand(format!(
                "Invalid key spec '{}'",
                key_spec
            )));
        }
        let lower = part.to_ascii_lowercase();
        if parts.peek().is_some() {
            match lower.as_str() {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" | "meta" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                _ => {
                    return Err(AppError::InvalidCommand(format!(
                        "Unknown key modifier '{}' in '{}'",
                        part, key_spec
                    )))
                }
            }
        } else {
            base = Some(part);
        }
    }

    let base = base
        .ok_or_else(|| AppError::InvalidCommand(format!("Missing base key in '{}'", key_spec)))?;
    let key_code = match base.to_ascii_lowercase().as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" | "shift-tab" => {
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
        "pageup" | "page-up" | "pgup" => KeyCode::PageUp,
        "pagedown" | "page-down" | "pgdown" => KeyCode::PageDown,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        value if value.len() == 1 => {
            let ch = base.chars().next().ok_or_else(|| {
                AppError::InvalidCommand(format!("Invalid key spec '{}'", key_spec))
            })?;
            KeyCode::Char(ch)
        }
        _ => {
            return Err(AppError::InvalidCommand(format!(
                "Unknown key '{}' in '{}'",
                base, key_spec
            )))
        }
    };

    let key_code = if key_code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
        KeyCode::BackTab
    } else {
        key_code
    };

    Ok(KeyEvent::new(key_code, modifiers))
}

fn first_token(buffer: &str) -> Option<&str> {
    buffer.split_whitespace().next()
}

fn format_command_arg(arg: &CommandArgValue) -> String {
    match arg {
        CommandArgValue::UnsignedInt(value) => value.to_string(),
        CommandArgValue::Word(value) => {
            if value.is_empty()
                || value
                    .chars()
                    .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\\' | ';'))
            {
                format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                value.clone()
            }
        }
    }
}

fn push_startup_command(
    commands: &mut Vec<StartupCommand>,
    current: &mut String,
    origin: &str,
    line: usize,
    segment: usize,
) {
    let trimmed = current.trim();
    if !trimmed.is_empty() && !trimmed.starts_with('#') {
        commands.push(StartupCommand {
            origin: format_startup_origin(origin, line, segment),
            command_text: trimmed.to_string(),
        });
    }
    current.clear();
}

fn format_startup_origin(origin: &str, line: usize, segment: usize) -> String {
    if segment <= 1 {
        format!("{origin}:{line}")
    } else {
        format!("{origin}:{line}[{segment}]")
    }
}

fn command_tail(buffer: &str) -> Option<&str> {
    let trimmed = buffer.trim_start();
    let token = first_token(trimmed)?;
    let rest = &trimmed[token.len()..];
    Some(rest)
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    use super::{
        describe_command_descriptor, describe_command_invocation, find_command_descriptor,
        format_command_invocation, parse_command_text, parse_startup_commands,
        tokenize_command_text, CommandArgValue, CommandId, CommandState,
    };

    #[test]
    fn parses_named_seek_command() {
        let command = parse_command_text("seek 42").expect("expected command to parse");
        assert_eq!(command.id, CommandId::Seek);
        assert_eq!(command.args, vec![CommandArgValue::UnsignedInt(42)]);
    }

    #[test]
    fn parses_goto_command_with_path_argument() {
        let command = parse_command_text("goto /group/dataset").expect("expected goto command");
        assert_eq!(command.id, CommandId::Goto);
        assert_eq!(
            command.args,
            vec![CommandArgValue::Word("/group/dataset".to_string())]
        );
    }

    #[test]
    fn parses_goto_command_with_quoted_path_argument() {
        let command = parse_command_text(r#"goto "/group/my dataset""#)
            .expect("expected quoted goto command");
        assert_eq!(command.id, CommandId::Goto);
        assert_eq!(
            command.args,
            vec![CommandArgValue::Word("/group/my dataset".to_string())]
        );
    }

    #[test]
    fn parses_legacy_relative_aliases() {
        let down = parse_command_text("+7").expect("expected +7 to parse");
        assert_eq!(down.id, CommandId::Down);
        assert_eq!(down.args, vec![CommandArgValue::UnsignedInt(7)]);

        let up = parse_command_text("-3").expect("expected -3 to parse");
        assert_eq!(up.id, CommandId::Up);
        assert_eq!(up.args, vec![CommandArgValue::UnsignedInt(3)]);
    }

    #[test]
    fn parses_legacy_absolute_alias() {
        let command = parse_command_text("9").expect("expected 9 to parse");
        assert_eq!(command.id, CommandId::Seek);
        assert_eq!(command.args, vec![CommandArgValue::UnsignedInt(9)]);
    }

    #[test]
    fn rejects_unknown_commands() {
        let error = parse_command_text("teleport 4").expect_err("expected parse error");
        assert!(error.to_string().contains("Unknown command"));
    }

    #[test]
    fn tokenizes_quoted_arguments() {
        let tokens =
            tokenize_command_text(r#"seek "42""#).expect("expected quoted tokens to parse");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].value, "seek");
        assert_eq!(tokens[1].value, "42");
        assert!(tokens[1].quoted);
    }

    #[test]
    fn rejects_unterminated_quotes() {
        let error = tokenize_command_text(r#"seek "42"#).expect_err("expected quote error");
        assert!(error.to_string().contains("Unterminated quoted argument"));
    }

    #[test]
    fn finds_legacy_descriptor_matches() {
        let matches = super::command_matches("+4");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "down");
    }

    #[test]
    fn parses_focus_command_with_word_argument() {
        let command = parse_command_text("focus content").expect("expected focus command");
        assert_eq!(command.id, CommandId::Focus);
        assert_eq!(
            command.args,
            vec![CommandArgValue::Word("content".to_string())]
        );
    }

    #[test]
    fn parses_index_command_with_optional_amount() {
        let command = parse_command_text("index prev 10").expect("expected index command");
        assert_eq!(command.id, CommandId::Index);
        assert_eq!(
            command.args,
            vec![
                CommandArgValue::Word("prev".to_string()),
                CommandArgValue::UnsignedInt(10)
            ]
        );
    }

    #[test]
    fn parses_multichart_add_command_with_dataset_spec() {
        let command =
            parse_command_text("mchart add !/group/dataset[..,0]").expect("expected mchart add");
        assert_eq!(command.id, CommandId::MultiChart);
        assert_eq!(
            command.args,
            vec![
                CommandArgValue::Word("add".to_string()),
                CommandArgValue::Word("!/group/dataset[..,0]".to_string()),
            ]
        );
    }

    #[test]
    fn parses_multichart_expression_command_with_quoted_expression() {
        let command = parse_command_text(r#"mchart expr "($1, !/ticks + #OFFSET)""#)
            .expect("expected mchart expr");
        assert_eq!(command.id, CommandId::MultiChart);
        assert_eq!(
            command.args,
            vec![
                CommandArgValue::Word("expr".to_string()),
                CommandArgValue::Word("($1, !/ticks + #OFFSET)".to_string()),
            ]
        );
    }

    #[test]
    fn parses_press_command_with_multiple_keys() {
        let command = parse_command_text("press ctrl+w o").expect("expected press command");
        assert_eq!(command.id, CommandId::Press);
        assert_eq!(
            command.args,
            vec![
                CommandArgValue::Word("ctrl+w".to_string()),
                CommandArgValue::Word("o".to_string()),
            ]
        );
    }

    #[test]
    fn parses_shift_tab_key_spec() {
        let key = super::parse_simulated_key("shift+tab").expect("shift+tab key");
        assert_eq!(key.code, KeyCode::BackTab);
        assert!(key.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn history_navigation_restores_draft() {
        let mut state = CommandState {
            command_buffer: "see".to_string(),
            cursor: 3,
            last_command: None,
            selected_suggestion: 0,
            history: std::collections::VecDeque::from(["seek 1".to_string(), "down 3".to_string()]),
            history_cursor: None,
            history_draft: None,
        };

        assert!(state.select_previous_history());
        assert_eq!(state.command_buffer, "down 3");
        assert!(state.select_previous_history());
        assert_eq!(state.command_buffer, "seek 1");
        assert!(state.select_next_history());
        assert_eq!(state.command_buffer, "down 3");
        assert!(state.select_next_history());
        assert_eq!(state.command_buffer, "see");
        assert!(state.history_cursor.is_none());
    }

    #[test]
    fn parses_startup_script_lines_with_comments() {
        let commands = parse_startup_commands("\n# comment\nseek 1\n  down 2  \n", "stdin");
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].origin, "stdin:3");
        assert_eq!(commands[0].command_text, "seek 1");
        assert_eq!(commands[1].origin, "stdin:4");
        assert_eq!(commands[1].command_text, "down 2");
    }

    #[test]
    fn parses_startup_script_semicolon_segments() {
        let commands = parse_startup_commands("seek 1; down 2\nmode preview", "script.h5v");
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].origin, "script.h5v:1");
        assert_eq!(commands[1].origin, "script.h5v:1[2]");
        assert_eq!(commands[1].command_text, "down 2");
        assert_eq!(commands[2].origin, "script.h5v:2");
    }

    #[test]
    fn keeps_semicolons_inside_quoted_startup_commands() {
        let commands = parse_startup_commands(r#"focus "content; pane"; down 2"#, "stdin");
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].command_text, r#"focus "content; pane""#);
        assert_eq!(commands[1].command_text, "down 2");
    }

    #[test]
    fn formats_alias_commands_using_canonical_names() {
        let command = parse_command_text("+7").expect("expected +7 to parse");
        assert_eq!(format_command_invocation(&command), "down 7");
        assert_eq!(
            describe_command_invocation(&command),
            Some("Move down by a relative amount")
        );
    }

    #[test]
    fn parses_help_command_with_optional_target() {
        let command = parse_command_text("help reload").expect("expected help command");
        assert_eq!(command.id, CommandId::Help);
        assert_eq!(
            command.args,
            vec![CommandArgValue::Word("reload".to_string())]
        );
    }

    #[test]
    fn describes_command_descriptor_with_aliases_and_keys() {
        let descriptor = find_command_descriptor("reload").expect("reload descriptor");
        let description = describe_command_descriptor(descriptor);
        assert!(description.contains("reload"));
        assert!(description.contains("refresh"));
        assert!(description.contains("Ctrl+r"));
    }

    #[test]
    fn repeat_does_not_replace_last_command() {
        let mut state = CommandState {
            command_buffer: String::new(),
            cursor: 0,
            last_command: Some(parse_command_text("down 3").expect("down command")),
            selected_suggestion: 0,
            history: std::collections::VecDeque::new(),
            history_cursor: None,
            history_draft: None,
        };

        let repeat = parse_command_text("repeat").expect("repeat command");
        state.record_successful_command(&repeat);
        assert_eq!(
            state.last_command.expect("last command").command_name,
            "down"
        );
    }
}
