use std::collections::VecDeque;

use crate::{
    configure,
    configure::registry::CommandHandle,
    error::AppError,
    ui::input::keymap::{
        AttributesAction, BoundAction, ContentAction, EffectiveKeymaps, GlobalAction, KeyBinding,
        MultiChartAction, NormalAction, TreeAction, WindowAction,
    },
};

use super::{
    input::EventResult,
    state::{AppState, Mode},
};

mod catalog;
mod handlers;
mod parsing;
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests;
mod view;

#[allow(unused_imports)]
pub use catalog::{command_catalog, find_command_descriptor};
#[allow(unused_imports)]
pub use parsing::{
    command_keybindings, command_keybindings_metadata, command_matches, command_metadata,
    command_metadata_by_handle, command_usage, command_usage_metadata, current_command_metadata,
    describe_command_descriptor, describe_command_invocation, describe_command_metadata,
    format_command_invocation, parse_command_text, parse_startup_commands,
    selected_command_metadata,
};
pub use view::render_command_dialog;

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
    SeekRow,
    SeekCol,
    SeekPage,
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
    Quit,
    Logs,
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
    Heatmap,
    Custom,
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
    pub help: &'static str,
    pub values: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub struct CommandDescriptor {
    pub id: CommandId,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub category: CommandCategory,
    pub keybindings: &'static [&'static str],
    pub args: &'static [CommandArgSpec],
    pub example: &'static str,
    pub handler: CommandHandler,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandArgValue {
    UnsignedInt(usize),
    Word(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub handle: CommandHandle,
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

pub fn builtin_command_handle(name: &str) -> CommandHandle {
    CommandHandle::new(format!("builtin.command.{name}"))
}

pub fn command_lua_id_symbol(name: &str) -> String {
    name.replace('-', "_")
}

pub fn find_command_descriptor_by_handle(
    handle: &CommandHandle,
) -> Option<&'static CommandDescriptor> {
    command_catalog()
        .iter()
        .find(|descriptor| builtin_command_handle(descriptor.name) == *handle)
}

impl CommandInvocation {
    pub fn noop(raw_input: impl Into<String>) -> Self {
        Self {
            handle: builtin_command_handle("noop"),
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

    pub fn usize_arg_optional(&self, index: usize) -> Result<Option<usize>, AppError> {
        match self.args.get(index) {
            Some(CommandArgValue::UnsignedInt(value)) => Ok(Some(*value)),
            Some(CommandArgValue::Word(_)) => Err(AppError::InvalidCommand(format!(
                "Argument {} for command '{}' must be a number",
                index + 1,
                self.command_name
            ))),
            None => Ok(None),
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

    pub fn apply_selected_suggestion(&mut self) -> bool {
        let trimmed = self.command_buffer.trim_start();
        if trimmed.is_empty() {
            return false;
        }
        let Some(descriptor) =
            selected_command_metadata(&self.command_buffer, self.selected_suggestion)
        else {
            return false;
        };
        let leading_len = self.command_buffer.len() - trimmed.len();
        let leading = self.command_buffer[..leading_len].to_string();
        let suffix = trimmed
            .find(char::is_whitespace)
            .map(|index| trimmed[index..].to_string())
            .unwrap_or_default();
        self.command_buffer = format!("{leading}{}{}", descriptor.name, suffix);
        if suffix.is_empty() {
            self.command_buffer.push(' ');
        }
        self.cursor = self.command_buffer.len();
        self.note_buffer_edited();
        true
    }

    pub fn record_successful_command(&mut self, invocation: &CommandInvocation) {
        if invocation.is_noop() {
            return;
        }

        if !matches!(
            find_command_descriptor_by_handle(&invocation.handle).map(|descriptor| descriptor.id),
            Some(CommandId::Repeat)
        ) {
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

    if let Some(descriptor) = find_command_descriptor_by_handle(&command.handle) {
        return (descriptor.handler)(state, command);
    }

    let snapshot = configure::current_registry_snapshot();
    let metadata = snapshot.command(&command.handle).ok_or_else(|| {
        AppError::InvalidCommand(format!(
            "Command '{}' is not registered",
            command.command_name
        ))
    })?;
    let Some(callback_id) = metadata.callback_id.as_deref() else {
        return Err(AppError::InvalidCommand(format!(
            "Command '{}' is not executable",
            command.command_name
        )));
    };
    execute_registered_command_callback(state, callback_id)
}

fn execute_registered_command_callback(
    state: &mut AppState<'_>,
    callback_id: &str,
) -> Result<EventResult, AppError> {
    if state.binding_command_depth >= 8 {
        return Err(AppError::InvalidCommand(
            "Command recursion limit reached".to_string(),
        ));
    }
    state.binding_command_depth += 1;
    let result = configure::with_command_lua_callback(callback_id, |lua, callback| {
        let callback_result = std::cell::RefCell::new(EventResult::Continue);
        let state_cell = std::cell::RefCell::new(&mut *state);
        lua.scope(|scope| {
            let command_fn = scope.create_function_mut(|_, command: String| {
                let mut state = state_cell.borrow_mut();
                *callback_result.borrow_mut() =
                    crate::ui::input::execute_bound_command(*state, &command)
                        .map_err(|error| mlua::Error::runtime(error.to_string()))?;
                Ok(())
            })?;
            let commands_fn = scope.create_function_mut(|_, commands: mlua::Table| {
                let mut parts = Vec::new();
                for value in commands.sequence_values::<String>() {
                    parts.push(value?);
                }
                let script = parts.join("\n");
                let mut state = state_cell.borrow_mut();
                *callback_result.borrow_mut() =
                    crate::ui::input::execute_bound_script(*state, &script, "lua command callback")
                        .map_err(|error| mlua::Error::runtime(error.to_string()))?;
                Ok(())
            })?;
            let script_fn = scope.create_function_mut(|_, script: String| {
                let mut state = state_cell.borrow_mut();
                *callback_result.borrow_mut() =
                    crate::ui::input::execute_bound_script(*state, &script, "lua command callback")
                        .map_err(|error| mlua::Error::runtime(error.to_string()))?;
                Ok(())
            })?;
            let toast_info_fn = scope.create_function_mut(|_, message: String| {
                let mut state = state_cell.borrow_mut();
                configure::set_lua_toast(*state, configure::LuaToastLevel::Info, message);
                if matches!(*callback_result.borrow(), EventResult::Continue) {
                    *callback_result.borrow_mut() = EventResult::Redraw;
                }
                Ok(())
            })?;
            let toast_warn_fn = scope.create_function_mut(|_, message: String| {
                let mut state = state_cell.borrow_mut();
                configure::set_lua_toast(*state, configure::LuaToastLevel::Warning, message);
                if matches!(*callback_result.borrow(), EventResult::Continue) {
                    *callback_result.borrow_mut() = EventResult::Redraw;
                }
                Ok(())
            })?;
            let toast_error_fn = scope.create_function_mut(|_, message: String| {
                let mut state = state_cell.borrow_mut();
                configure::set_lua_toast(*state, configure::LuaToastLevel::Error, message);
                if matches!(*callback_result.borrow(), EventResult::Continue) {
                    *callback_result.borrow_mut() = EventResult::Redraw;
                }
                Ok(())
            })?;
            let process_run_fn = scope.create_function_mut(|lua, spec: mlua::Table| {
                configure::run_lua_process_spec(lua, spec, false)
            })?;
            let process_spawn_fn = scope.create_function_mut(|lua, spec: mlua::Table| {
                configure::run_lua_process_spec(lua, spec, true)
            })?;
            let content_open_fn = scope.create_function_mut(|lua, target: String| {
                let mut state = state_cell.borrow_mut();
                configure::open_lua_content_mode_target(lua, *state, &target)?;
                if matches!(*callback_result.borrow(), EventResult::Continue) {
                    *callback_result.borrow_mut() = EventResult::Redraw;
                }
                Ok(())
            })?;
            let content_toggle_fn = scope.create_function_mut(|_, ()| {
                let mut state = state_cell.borrow_mut();
                let available = state.treeview[state.tree_view_cursor]
                    .node
                    .borrow_mut()
                    .content_show_modes();
                let available = state.available_content_mode_handles(available);
                state.swap_content_mode_handle(available);
                if matches!(*callback_result.borrow(), EventResult::Continue) {
                    *callback_result.borrow_mut() = EventResult::Redraw;
                }
                Ok(())
            })?;
            let mchart_open_fn = scope.create_function_mut(|_, ()| {
                let mut state = state_cell.borrow_mut();
                state.mode = Mode::MultiChart;
                if matches!(*callback_result.borrow(), EventResult::Continue) {
                    *callback_result.borrow_mut() = EventResult::Redraw;
                }
                Ok(())
            })?;
            let mchart_close_fn = scope.create_function_mut(|_, ()| {
                let mut state = state_cell.borrow_mut();
                state.multi_chart.close_expression_prompt();
                state.mode = Mode::Normal;
                if matches!(*callback_result.borrow(), EventResult::Continue) {
                    *callback_result.borrow_mut() = EventResult::Redraw;
                }
                Ok(())
            })?;
            let mchart_toggle_fn = scope.create_function_mut(|_, ()| {
                let mut state = state_cell.borrow_mut();
                state.multi_chart.close_expression_prompt();
                state.mode = if matches!(state.mode, Mode::MultiChart) {
                    Mode::Normal
                } else {
                    Mode::MultiChart
                };
                if matches!(*callback_result.borrow(), EventResult::Continue) {
                    *callback_result.borrow_mut() = EventResult::Redraw;
                }
                Ok(())
            })?;
            let ctx = lua.create_table()?;
            let toast = lua.create_table()?;
            toast.set("info", toast_info_fn)?;
            toast.set("warning", toast_warn_fn.clone())?;
            toast.set("warn", toast_warn_fn)?;
            toast.set("error", toast_error_fn)?;
            let process = lua.create_table()?;
            process.set("run", process_run_fn)?;
            process.set("spawn", process_spawn_fn)?;
            let content = lua.create_table()?;
            content.set("open", content_open_fn)?;
            content.set("toggle", content_toggle_fn)?;
            let mchart = lua.create_table()?;
            mchart.set("open", mchart_open_fn)?;
            mchart.set("close", mchart_close_fn)?;
            mchart.set("toggle", mchart_toggle_fn)?;
            ctx.set("command", command_fn)?;
            ctx.set("commands", commands_fn)?;
            ctx.set("script", script_fn)?;
            ctx.set("toast", toast)?;
            ctx.set("process", process)?;
            ctx.set("content", content)?;
            ctx.set("mchart", mchart)?;
            {
                let state = state_cell.borrow();
                ctx.set("app", configure::build_lua_app_context(lua, &state)?)?;
                ctx.set("config", configure::build_lua_config_context(lua)?)?;
                ctx.set("fs", configure::build_lua_fs_context(lua, &state)?)?;
                ctx.set(
                    "selection",
                    configure::build_lua_selection_context(lua, &state)?,
                )?;
                ctx.set("plugin", configure::build_lua_plugin_context(lua)?)?;
            }
            callback.call::<()>(ctx)?;
            Ok(())
        })
        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        Ok(callback_result.into_inner())
    });
    state.binding_command_depth = state.binding_command_depth.saturating_sub(1);
    result
}

pub fn sync_command_registry_keybindings(keymaps: &EffectiveKeymaps) {
    let mut snapshot = configure::current_registry_snapshot();
    let mut command_targets = std::collections::BTreeMap::<String, Vec<String>>::new();
    extend_registry_command_targets(&snapshot, &mut command_targets, &keymaps.global);
    extend_registry_command_targets(&snapshot, &mut command_targets, &keymaps.normal);
    extend_registry_command_targets(&snapshot, &mut command_targets, &keymaps.window);
    extend_registry_command_targets(&snapshot, &mut command_targets, &keymaps.tree);
    extend_registry_command_targets(&snapshot, &mut command_targets, &keymaps.content);
    extend_registry_command_targets(&snapshot, &mut command_targets, &keymaps.heatmap);
    extend_registry_command_targets(&snapshot, &mut command_targets, &keymaps.attributes);
    extend_registry_command_targets(&snapshot, &mut command_targets, &keymaps.multichart);
    for metadata in snapshot.commands.values_mut() {
        metadata.keybindings = if matches!(
            metadata.owner,
            crate::configure::registry::RegistryOwner::Builtin
        ) {
            derived_builtin_command_keybindings(&metadata.handle, keymaps)
        } else {
            Vec::new()
        };
        if let Some(extra) = command_targets.get(metadata.handle.as_str()) {
            for label in extra {
                if !metadata.keybindings.contains(label) {
                    metadata.keybindings.push(label.clone());
                }
            }
        }
    }
    configure::install_registry_snapshot(snapshot);
}

fn extend_registry_command_targets<T>(
    _snapshot: &configure::RegistrySnapshot,
    targets: &mut std::collections::BTreeMap<String, Vec<String>>,
    bindings: &[KeyBinding<T>],
) {
    for binding in bindings {
        let BoundAction::Command(command_text) = &binding.target else {
            continue;
        };
        let Some(tokens) = parsing::tokenize_command_text(command_text).ok() else {
            continue;
        };
        let Some(command_name) = tokens.first().map(|token| token.value.as_str()) else {
            continue;
        };
        let Some(handle) = crate::ui::command::command_metadata(command_name)
            .map(|metadata| metadata.handle.to_string())
        else {
            continue;
        };
        let entry = targets.entry(handle).or_default();
        let label = binding.key.to_string();
        if !entry.contains(&label) {
            entry.push(label);
        }
    }
}

fn derived_builtin_command_keybindings(
    handle: &CommandHandle,
    keymaps: &EffectiveKeymaps,
) -> Vec<String> {
    let mut labels = Vec::new();
    match handle.as_str() {
        "builtin.command.up" => {
            extend_action_keybindings(&mut labels, &keymaps.tree, &TreeAction::MoveUp(1));
            extend_action_keybindings(
                &mut labels,
                &keymaps.content,
                &ContentAction::Move(crate::ui::input::keymap::Direction::Up, 1),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.attributes,
                &AttributesAction::Move(crate::ui::input::keymap::Direction::Up, 1),
            );
        }
        "builtin.command.down" => {
            extend_action_keybindings(&mut labels, &keymaps.tree, &TreeAction::MoveDown(1));
            extend_action_keybindings(
                &mut labels,
                &keymaps.content,
                &ContentAction::Move(crate::ui::input::keymap::Direction::Down, 1),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.attributes,
                &AttributesAction::Move(crate::ui::input::keymap::Direction::Down, 1),
            );
        }
        "builtin.command.left" => {
            extend_action_keybindings(
                &mut labels,
                &keymaps.content,
                &ContentAction::Move(crate::ui::input::keymap::Direction::Left, 1),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.attributes,
                &AttributesAction::Move(crate::ui::input::keymap::Direction::Left, 1),
            );
        }
        "builtin.command.right" => {
            extend_action_keybindings(
                &mut labels,
                &keymaps.content,
                &ContentAction::Move(crate::ui::input::keymap::Direction::Right, 1),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.attributes,
                &AttributesAction::Move(crate::ui::input::keymap::Direction::Right, 1),
            );
        }
        "builtin.command.page-up" => {
            extend_action_keybindings(&mut labels, &keymaps.tree, &TreeAction::MoveUp(10));
            extend_action_keybindings(
                &mut labels,
                &keymaps.content,
                &ContentAction::Move(crate::ui::input::keymap::Direction::Up, 10),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.attributes,
                &AttributesAction::Move(crate::ui::input::keymap::Direction::Up, 10),
            );
        }
        "builtin.command.page-down" => {
            extend_action_keybindings(&mut labels, &keymaps.tree, &TreeAction::MoveDown(10));
            extend_action_keybindings(
                &mut labels,
                &keymaps.content,
                &ContentAction::Move(crate::ui::input::keymap::Direction::Down, 10),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.attributes,
                &AttributesAction::Move(crate::ui::input::keymap::Direction::Down, 10),
            );
        }
        "builtin.command.focus" => {
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::Focus(crate::ui::input::keymap::Direction::Left),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::Focus(crate::ui::input::keymap::Direction::Right),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::Focus(crate::ui::input::keymap::Direction::Up),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::Focus(crate::ui::input::keymap::Direction::Down),
            );
        }
        "builtin.command.mode" => {
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::ToggleContentMode,
            );
        }
        "builtin.command.toggle-tree" => {
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::ToggleTreeView);
            extend_action_keybindings(&mut labels, &keymaps.window, &WindowAction::ToggleTreeView);
        }
        "builtin.command.reload" => {
            extend_action_keybindings(&mut labels, &keymaps.global, &GlobalAction::ReloadFile);
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::ReloadFile);
        }
        "builtin.command.x" => {
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::ChangeX(1));
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::ChangeX(-1));
        }
        "builtin.command.row" => {
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::ChangeRow(1));
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::ChangeRow(-1));
        }
        "builtin.command.col" => {
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::ChangeCol(1));
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::ChangeCol(-1));
        }
        "builtin.command.dim" => {
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::ChangeSelectedDimension(1),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::ChangeSelectedDimension(-1),
            );
        }
        "builtin.command.index" => {
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::ChangeSelectedIndex(1),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::ChangeSelectedIndex(-1),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::ChangeSelectedIndex(10),
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::ChangeSelectedIndex(-10),
            );
        }
        "builtin.command.help" => {
            extend_action_keybindings(&mut labels, &keymaps.global, &GlobalAction::ShowHelp);
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::ShowHelp);
            extend_action_keybindings(
                &mut labels,
                &keymaps.multichart,
                &MultiChartAction::ShowHelp,
            );
        }
        "builtin.command.attr" => {
            extend_action_keybindings(&mut labels, &keymaps.attributes, &AttributesAction::Create);
            extend_action_keybindings(&mut labels, &keymaps.attributes, &AttributesAction::Delete);
        }
        "builtin.command.repeat" => {
            extend_action_keybindings(&mut labels, &keymaps.normal, &NormalAction::RepeatCommand);
        }
        "builtin.command.mchart" => {
            extend_action_keybindings(
                &mut labels,
                &keymaps.global,
                &GlobalAction::ToggleMultiChart,
            );
            extend_action_keybindings(
                &mut labels,
                &keymaps.normal,
                &NormalAction::ToggleMultiChart,
            );
        }
        _ => {}
    }
    labels
}

fn extend_action_keybindings<T: PartialEq>(
    labels: &mut Vec<String>,
    bindings: &[KeyBinding<T>],
    action: &T,
) {
    for binding in bindings {
        if matches!(&binding.target, BoundAction::Action(target) if target == action) {
            let label = binding.key.to_string();
            if !labels.contains(&label) {
                labels.push(label);
            }
        }
    }
}
