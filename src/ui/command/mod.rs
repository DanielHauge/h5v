use std::collections::VecDeque;

use crate::error::AppError;

use super::{input::EventResult, state::AppState};

mod catalog;
mod handlers;
mod parsing;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use catalog::{command_catalog, find_command_descriptor, find_command_descriptor_by_id};
#[allow(unused_imports)]
pub use parsing::{
    command_keybindings, command_matches, command_usage, current_command_descriptor,
    describe_command_descriptor, describe_command_invocation, format_command_invocation,
    parse_command_text, parse_startup_commands, selected_command_descriptor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Navigation,
    View,
    Selection,
    Attributes,
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
    Configure,
    X,
    Row,
    Col,
    Dim,
    Index,
    Help,
    Attr,
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

        let remainder = parsing::command_tail(&self.command_buffer)
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
