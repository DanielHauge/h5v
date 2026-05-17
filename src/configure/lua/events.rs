use mlua::{Function, Lua, Table, Value};

use crate::{
    configure::{errors::ConfigureErrors, registry::EventHandle},
    error::AppError,
    ui::{input::EventResult, state::AppState},
};

use super::keymaps::with_config_lua_runtime;

const EVENTS_CALLBACKS_FIELD: &str = "__lua_callbacks";
const EVENTS_HANDLERS_FIELD: &str = "__handlers";
const EVENTS_NEXT_ID_FIELD: &str = "__next_lua_callback_id";
const REGISTRY_OWNER_FIELD: &str = "__registry_owner";

pub(super) fn build_events_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let events = lua.create_table()?;
    events.set(EVENTS_CALLBACKS_FIELD, lua.create_table()?)?;
    events.set(EVENTS_HANDLERS_FIELD, lua.create_table()?)?;
    events.set(EVENTS_NEXT_ID_FIELD, 1)?;

    let register_table = events.clone();
    let on_fn = lua.create_function(move |lua, (event_handle, callback): (String, Function)| {
        register_event_handler(lua, &register_table, &event_handle, callback)
    })?;
    events.set("on", on_fn)?;
    Ok(events)
}

pub(crate) fn dispatch_lua_event(
    state: &mut AppState<'_>,
    event_handle: &str,
    payload: impl FnOnce(&Lua) -> Result<Table, mlua::Error>,
) -> Result<EventResult, AppError> {
    if state.binding_command_depth >= 8 {
        return Err(AppError::InvalidCommand(
            "Lua event recursion limit reached".to_string(),
        ));
    }
    state.binding_command_depth += 1;
    let result = with_config_lua_runtime(|lua| {
        let h5v: Table = lua
            .globals()
            .get("h5v")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let events: Table = h5v
            .get("events")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let handlers: Table = events
            .get(EVENTS_HANDLERS_FIELD)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let callbacks: Table = events
            .get(EVENTS_CALLBACKS_FIELD)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let handler_ids = match handlers
            .get::<Value>(event_handle)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?
        {
            Value::Nil => return Ok(EventResult::Continue),
            Value::Table(table) => table,
            other => {
                return Err(AppError::InvalidCommand(format!(
                    "Lua event handlers for '{}' must be a table, got {}",
                    event_handle,
                    other.type_name()
                )))
            }
        };
        let payload = payload(lua).map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let mut last_result = EventResult::Continue;
        for entry in handler_ids.sequence_values::<Value>() {
            let entry = entry.map_err(|error| AppError::InvalidCommand(error.to_string()))?;
            let (callback_id, owner) = match entry {
                Value::String(value) => (
                    value
                        .to_str()
                        .map_err(|error| AppError::InvalidCommand(error.to_string()))?
                        .to_string(),
                    None,
                ),
                Value::Table(table) => {
                    let callback_id: String = table
                        .get("callback_id")
                        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
                    let owner: Option<String> = table
                        .get("owner")
                        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
                    (callback_id, owner)
                }
                other => {
                    return Err(AppError::InvalidCommand(format!(
                        "Lua event handler entry for '{}' must be a string or table, got {}",
                        event_handle,
                        other.type_name()
                    )))
                }
            };
            if owner
                .as_deref()
                .is_some_and(|owner| owner.starts_with("plugin."))
                && !super::plugins::plugin_handle_is_enabled(
                    &h5v,
                    owner.as_deref().unwrap_or_default(),
                )
                .map_err(|error| AppError::InvalidCommand(error.to_string()))?
            {
                continue;
            }
            let callback: Function = callbacks
                .get(callback_id.as_str())
                .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
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
                    *callback_result.borrow_mut() = crate::ui::input::execute_bound_script(
                        *state,
                        &script,
                        "lua event callback",
                    )
                    .map_err(|error| mlua::Error::runtime(error.to_string()))?;
                    Ok(())
                })?;
                let script_fn = scope.create_function_mut(|_, script: String| {
                    let mut state = state_cell.borrow_mut();
                    *callback_result.borrow_mut() = crate::ui::input::execute_bound_script(
                        *state,
                        &script,
                        "lua event callback",
                    )
                    .map_err(|error| mlua::Error::runtime(error.to_string()))?;
                    Ok(())
                })?;
                let toast_info_fn = scope.create_function_mut(|_, message: String| {
                    let mut state = state_cell.borrow_mut();
                    crate::configure::set_lua_toast(
                        *state,
                        crate::configure::LuaToastLevel::Info,
                        message,
                    );
                    if matches!(*callback_result.borrow(), EventResult::Continue) {
                        *callback_result.borrow_mut() = EventResult::Redraw;
                    }
                    Ok(())
                })?;
                let toast_warn_fn = scope.create_function_mut(|_, message: String| {
                    let mut state = state_cell.borrow_mut();
                    crate::configure::set_lua_toast(
                        *state,
                        crate::configure::LuaToastLevel::Warning,
                        message,
                    );
                    if matches!(*callback_result.borrow(), EventResult::Continue) {
                        *callback_result.borrow_mut() = EventResult::Redraw;
                    }
                    Ok(())
                })?;
                let toast_error_fn = scope.create_function_mut(|_, message: String| {
                    let mut state = state_cell.borrow_mut();
                    crate::configure::set_lua_toast(
                        *state,
                        crate::configure::LuaToastLevel::Error,
                        message,
                    );
                    if matches!(*callback_result.borrow(), EventResult::Continue) {
                        *callback_result.borrow_mut() = EventResult::Redraw;
                    }
                    Ok(())
                })?;
                let content_open_fn = scope.create_function_mut(|lua, target: String| {
                    let mut state = state_cell.borrow_mut();
                    crate::configure::open_lua_content_mode_target(lua, *state, &target)?;
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
                    state.mode = crate::ui::state::Mode::MultiChart;
                    if matches!(*callback_result.borrow(), EventResult::Continue) {
                        *callback_result.borrow_mut() = EventResult::Redraw;
                    }
                    Ok(())
                })?;
                let mchart_close_fn = scope.create_function_mut(|_, ()| {
                    let mut state = state_cell.borrow_mut();
                    state.multi_chart.close_expression_prompt();
                    state.mode = crate::ui::state::Mode::Normal;
                    if matches!(*callback_result.borrow(), EventResult::Continue) {
                        *callback_result.borrow_mut() = EventResult::Redraw;
                    }
                    Ok(())
                })?;
                let mchart_toggle_fn = scope.create_function_mut(|_, ()| {
                    let mut state = state_cell.borrow_mut();
                    state.multi_chart.close_expression_prompt();
                    state.mode = if matches!(state.mode, crate::ui::state::Mode::MultiChart) {
                        crate::ui::state::Mode::Normal
                    } else {
                        crate::ui::state::Mode::MultiChart
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
                ctx.set("log", crate::configure::build_lua_log_context(lua)?)?;
                ctx.set("toast", toast)?;
                ctx.set("process", crate::configure::build_lua_process_context(lua)?)?;
                ctx.set("content", content)?;
                ctx.set("mchart", mchart)?;
                {
                    let state = state_cell.borrow();
                    ctx.set("app", crate::configure::build_lua_app_context(lua, &state)?)?;
                    ctx.set("config", crate::configure::build_lua_config_context(lua)?)?;
                    ctx.set("fs", crate::configure::build_lua_fs_context(lua, &state)?)?;
                    ctx.set(
                        "selection",
                        crate::configure::build_lua_selection_context(lua, &state)?,
                    )?;
                    ctx.set("plugin", crate::configure::build_lua_plugin_context(lua)?)?;
                }
                callback.call::<()>((ctx, payload.clone()))?;
                Ok::<(), mlua::Error>(())
            })
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
            let callback_outcome = callback_result.into_inner();
            match callback_outcome {
                EventResult::Continue => {}
                EventResult::Redraw | EventResult::Copying | EventResult::Toast(_, _) => {
                    last_result = callback_outcome;
                }
                other => {
                    state.binding_command_depth = state.binding_command_depth.saturating_sub(1);
                    return Ok(other);
                }
            }
        }
        Ok(last_result)
    });
    state.binding_command_depth = state.binding_command_depth.saturating_sub(1);
    result
}

fn register_event_handler(
    lua: &Lua,
    events: &Table,
    event_handle: &str,
    callback: Function,
) -> Result<String, mlua::Error> {
    let snapshot = crate::configure::current_registry_snapshot();
    let event_handle = EventHandle::new(event_handle);
    let known = snapshot.event(&event_handle).is_some()
        || crate::configure::builtin_registry_snapshot()
            .ok()
            .and_then(|builtin| builtin.event(&event_handle).cloned())
            .is_some();
    if !known {
        return Err(mlua::Error::runtime(format!(
            "Unknown event handle '{}'",
            event_handle.as_str()
        )));
    }
    let callbacks: Table = events.get(EVENTS_CALLBACKS_FIELD)?;
    let next_id = match events.get::<Value>(EVENTS_NEXT_ID_FIELD)? {
        Value::Integer(value) if value > 0 => value,
        _ => 1,
    };
    let callback_id = format!("event-{next_id}");
    callbacks.set(callback_id.as_str(), callback)?;
    events.set(EVENTS_NEXT_ID_FIELD, next_id + 1)?;
    let h5v: Table = lua.globals().get("h5v")?;
    let owner = match h5v.get::<Value>(REGISTRY_OWNER_FIELD)? {
        Value::String(value) => value.to_str()?.to_string(),
        Value::Nil => "config".to_string(),
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.__registry_owner must be a string, got {}",
                other.type_name()
            )))
        }
    };

    let handlers: Table = events.get(EVENTS_HANDLERS_FIELD)?;
    let handler_list = match handlers.get::<Value>(event_handle.as_str())? {
        Value::Nil => {
            let created = lua.create_table()?;
            handlers.set(event_handle.as_str(), created.clone())?;
            created
        }
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.events handlers for '{}' must be a table, got {}",
                event_handle.as_str(),
                other.type_name()
            )))
        }
    };
    let stored = lua.create_table()?;
    stored.set("callback_id", callback_id.as_str())?;
    stored.set("owner", owner)?;
    handler_list.set(handler_list.raw_len() + 1, stored)?;
    Ok(callback_id)
}
