use std::{
    io::Write,
    process::{Command, Stdio},
};

use mlua::{Lua, Table, Value};
use serde_json::Value as JsonValue;
use tracing::Level;

use crate::ui::state::{AppState, AppToast, Mode};

use super::ui::resolve_registered_content_mode_handle;

pub(crate) enum LuaToastLevel {
    Info,
    Warning,
    Error,
}

pub(crate) fn set_lua_toast(state: &mut AppState<'_>, level: LuaToastLevel, message: String) {
    crate::ui::toast::apply_app_toast(
        state,
        match level {
            LuaToastLevel::Info => AppToast::Info(message),
            LuaToastLevel::Warning => AppToast::Warning(message),
            LuaToastLevel::Error => AppToast::Error(message),
        },
    );
}

pub(crate) fn build_log_context(lua: &Lua) -> Result<Table, mlua::Error> {
    build_log_context_with_handle(lua, crate::logging::APP_LOG_HANDLE)
}

pub(crate) fn build_log_context_with_handle(lua: &Lua, handle: &str) -> Result<Table, mlua::Error> {
    let handle = handle.to_string();
    let log = lua.create_table()?;
    log.set(
        "info",
        lua.create_function({
            let handle = handle.clone();
            move |_, message: String| {
                crate::logging::log_lua_with_handle(Level::INFO, &handle, &message);
                Ok(())
            }
        })?,
    )?;
    log.set(
        "warning",
        lua.create_function({
            let handle = handle.clone();
            move |_, message: String| {
                crate::logging::log_lua_with_handle(Level::WARN, &handle, &message);
                Ok(())
            }
        })?,
    )?;
    log.set(
        "warn",
        lua.create_function({
            let handle = handle.clone();
            move |_, message: String| {
                crate::logging::log_lua_with_handle(Level::WARN, &handle, &message);
                Ok(())
            }
        })?,
    )?;
    log.set(
        "error",
        lua.create_function(move |_, message: String| {
            crate::logging::log_lua_with_handle(Level::ERROR, &handle, &message);
            Ok(())
        })?,
    )?;
    Ok(log)
}

pub(crate) fn build_process_context(lua: &Lua) -> Result<Table, mlua::Error> {
    let process = lua.create_table()?;
    process.set(
        "run",
        lua.create_function(|lua, spec: Table| run_process_spec(lua, spec, false))?,
    )?;
    process.set(
        "spawn",
        lua.create_function(|lua, spec: Table| run_process_spec(lua, spec, true))?,
    )?;
    process.set(
        "parse_json",
        lua.create_function(|lua, value: Value| parse_process_json_output(lua, value))?,
    )?;
    Ok(process)
}

pub(crate) fn build_app_context(lua: &Lua, state: &AppState<'_>) -> Result<Table, mlua::Error> {
    let app = lua.create_table()?;
    app.set("readonly", state.readonly)?;
    app.set("mode", mode_label(&state.mode))?;
    app.set("content_mode", content_mode_label(state))?;
    Ok(app)
}

pub(crate) fn build_fs_context(lua: &Lua, state: &AppState<'_>) -> Result<Table, mlua::Error> {
    let fs = lua.create_table()?;
    fs.set("file_path", state.file_watch.path.clone())?;
    populate_fs_context(&fs)?;
    Ok(fs)
}

pub(crate) fn build_plugin_fs_context(lua: &Lua) -> Result<Table, mlua::Error> {
    let fs = lua.create_table()?;
    populate_fs_context(&fs)?;
    Ok(fs)
}

fn populate_fs_context(fs: &Table) -> Result<(), mlua::Error> {
    if let Ok(config_path) = crate::configure::config_path() {
        fs.set("config_path", config_path.display().to_string())?;
    }
    if let Ok(cwd) = std::env::current_dir() {
        fs.set("cwd", cwd.display().to_string())?;
    }
    Ok(())
}

pub(crate) fn build_selection_context(
    lua: &Lua,
    state: &AppState<'_>,
) -> Result<Table, mlua::Error> {
    let selection = lua.create_table()?;
    if let Some(path) = state.selected_tree_path() {
        selection.set("path", path)?;
    }
    selection.set("content_mode", content_mode_label(state))?;
    Ok(selection)
}

pub(crate) fn build_config_context(lua: &Lua) -> Result<Table, mlua::Error> {
    let config = lua.create_table()?;
    config.set("theme", crate::configure::current_theme_handle())?;
    config.set(
        "symbol_theme",
        crate::configure::current_symbol_theme_name().as_str(),
    )?;
    config.set("compatibility", crate::compat::current().compatibility_mode)?;
    config.set(
        "content_mode_order",
        lua.create_sequence_from(
            crate::configure::current_content_mode_order_handles()
                .into_iter()
                .map(|handle| handle.to_string()),
        )?,
    )?;
    Ok(config)
}

pub(crate) fn build_plugin_context(lua: &Lua) -> Result<Table, mlua::Error> {
    let h5v: Table = lua.globals().get("h5v")?;
    let store: Table = h5v.get("__plugin_store")?;
    let store_get = {
        let store = store.clone();
        lua.create_function(move |_, key: String| store.get::<Value>(key))?
    };
    let store_set = {
        let store = store.clone();
        lua.create_function(move |_, (key, value): (String, Value)| store.set(key, value))?
    };
    let store_delete = {
        let store = store.clone();
        lua.create_function(move |_, key: String| store.set(key, Value::Nil))?
    };
    let store_table = lua.create_table()?;
    store_table.set("get", store_get)?;
    store_table.set("set", store_set)?;
    store_table.set("delete", store_delete)?;

    let plugin = lua.create_table()?;
    plugin.set("store", store_table)?;
    Ok(plugin)
}

pub(crate) fn open_content_mode_target(
    lua: &Lua,
    state: &mut AppState<'_>,
    target: &str,
) -> Result<(), mlua::Error> {
    let target = target.trim();
    let handle = if let Some(mode) = crate::ui::state::ContentShowMode::parse_handle(target) {
        mode.handle()
    } else {
        let h5v: Table = lua.globals().get("h5v")?;
        resolve_registered_content_mode_handle(&h5v, target)?
            .ok_or_else(|| mlua::Error::runtime(format!("Unknown content mode '{target}'")))?
    };
    let available = state.available_content_mode_handles(
        state.treeview[state.tree_view_cursor]
            .node
            .borrow_mut()
            .content_show_modes(),
    );
    if !available.contains(&handle) {
        return Err(mlua::Error::runtime(format!(
            "Content mode '{}' is not available for the selected item",
            target
        )));
    }
    state.set_content_mode_handle(handle);
    Ok(())
}

pub(crate) fn content_mode_label(state: &AppState<'_>) -> String {
    let handle = state.active_content_mode_handle();
    crate::ui::state::ContentShowMode::parse_handle(handle.as_str())
        .map(|mode| mode.as_str().to_string())
        .unwrap_or_else(|| handle.as_str().to_string())
}

pub(crate) fn run_process_spec(
    lua: &Lua,
    spec: Table,
    detached: bool,
) -> Result<Table, mlua::Error> {
    let command = parse_process_command(&spec)?;
    let stdin = parse_process_stdin(&spec)?;
    let mut process = Command::new(&command[0]);
    process.args(&command[1..]);
    if let Some(cwd) = optional_string_field(&spec, "cwd", "ctx.process")? {
        process.current_dir(cwd);
    }

    if detached {
        if stdin.is_some() {
            return Err(mlua::Error::runtime(
                "ctx.process.spawn does not support stdin input",
            ));
        }
        process.stdin(Stdio::null());
        process.stdout(Stdio::null());
        process.stderr(Stdio::null());
        let child = process
            .spawn()
            .map_err(|error| mlua::Error::runtime(error.to_string()))?;
        let result = lua.create_table()?;
        result.set("success", true)?;
        result.set("pid", child.id())?;
        return Ok(result);
    }

    let output = if let Some(stdin) = stdin {
        process.stdin(Stdio::piped());
        process.stdout(Stdio::piped());
        process.stderr(Stdio::piped());
        let mut child = process
            .spawn()
            .map_err(|error| mlua::Error::runtime(error.to_string()))?;
        if let Some(mut child_stdin) = child.stdin.take() {
            child_stdin
                .write_all(&stdin)
                .map_err(|error| mlua::Error::runtime(error.to_string()))?;
        }
        child
            .wait_with_output()
            .map_err(|error| mlua::Error::runtime(error.to_string()))?
    } else {
        process
            .output()
            .map_err(|error| mlua::Error::runtime(error.to_string()))?
    };
    let result = lua.create_table()?;
    result.set("success", output.status.success())?;
    result.set("status", output.status.code().unwrap_or(-1))?;
    result.set(
        "stdout",
        String::from_utf8_lossy(&output.stdout).to_string(),
    )?;
    result.set(
        "stderr",
        String::from_utf8_lossy(&output.stderr).to_string(),
    )?;
    Ok(result)
}

pub(crate) fn parse_process_json_output(lua: &Lua, value: Value) -> Result<Value, mlua::Error> {
    let stdout = parse_process_stdout_text(value, "ctx.process.parse_json")?;
    let parsed = serde_json::from_str::<JsonValue>(&stdout).map_err(|error| {
        mlua::Error::runtime(format!(
            "ctx.process.parse_json could not parse stdout as JSON: {error}"
        ))
    })?;
    json_value_to_lua(lua, &parsed)
}

fn parse_process_stdout_text(value: Value, context: &str) -> Result<String, mlua::Error> {
    match value {
        Value::String(value) => Ok(value.to_str()?.to_string()),
        Value::Table(table) => {
            match table.get::<Value>("success")? {
                Value::Boolean(true) | Value::Nil => {}
                Value::Boolean(false) => {
                    let status = match table.get::<Value>("status")? {
                        Value::Integer(value) => value.to_string(),
                        Value::Nil => "unknown".to_string(),
                        other => other.type_name().to_string(),
                    };
                    let stderr = match table.get::<Value>("stderr")? {
                        Value::String(value) => value.to_str()?.trim().to_string(),
                        Value::Nil => String::new(),
                        other => {
                            return Err(mlua::Error::runtime(format!(
                                "{context} expected stderr to be a string, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let message = if stderr.is_empty() {
                        format!("{context} requires a successful process result, got exit status {status}")
                    } else {
                        format!(
                            "{context} requires a successful process result, got exit status {status}: {stderr}"
                        )
                    };
                    return Err(mlua::Error::runtime(message));
                }
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "{context} expected result.success to be a boolean, got {}",
                        other.type_name()
                    )))
                }
            }
            match table.get::<Value>("stdout")? {
                Value::String(stdout) => Ok(stdout.to_str()?.to_string()),
                Value::Nil => Err(mlua::Error::runtime(format!(
                    "{context} expected a process result with stdout"
                ))),
                other => Err(mlua::Error::runtime(format!(
                    "{context} expected stdout to be a string, got {}",
                    other.type_name()
                ))),
            }
        }
        other => Err(mlua::Error::runtime(format!(
            "{context} expects either a process result table or a stdout string, got {}",
            other.type_name()
        ))),
    }
}

fn json_value_to_lua(lua: &Lua, value: &JsonValue) -> Result<Value, mlua::Error> {
    match value {
        JsonValue::Null => Ok(Value::Nil),
        JsonValue::Bool(value) => Ok(Value::Boolean(*value)),
        JsonValue::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(Value::Integer(value))
            } else if let Some(value) = value.as_u64() {
                match i64::try_from(value) {
                    Ok(value) => Ok(Value::Integer(value)),
                    Err(_) => Ok(Value::Number(value as f64)),
                }
            } else {
                Ok(Value::Number(value.as_f64().ok_or_else(|| {
                    mlua::Error::runtime("ctx.process.parse_json encountered an unsupported number")
                })?))
            }
        }
        JsonValue::String(value) => Ok(Value::String(lua.create_string(value)?)),
        JsonValue::Array(values) => {
            let table = lua.create_table()?;
            for (index, value) in values.iter().enumerate() {
                table.set(index + 1, json_value_to_lua(lua, value)?)?;
            }
            Ok(Value::Table(table))
        }
        JsonValue::Object(values) => {
            let table = lua.create_table()?;
            for (key, value) in values {
                table.set(key.as_str(), json_value_to_lua(lua, value)?)?;
            }
            Ok(Value::Table(table))
        }
    }
}

fn parse_process_stdin(spec: &Table) -> Result<Option<Vec<u8>>, mlua::Error> {
    match spec.get::<Value>("stdin")? {
        Value::Nil => Ok(None),
        Value::String(value) => Ok(Some(value.as_bytes().to_vec())),
        Value::Table(values) => {
            let mut lines = Vec::new();
            for value in values.sequence_values::<Value>() {
                match value? {
                    Value::String(value) => lines.push(value.to_str()?.to_string()),
                    Value::Integer(value) => lines.push(value.to_string()),
                    Value::Number(value) if value.is_finite() => lines.push(value.to_string()),
                    Value::Number(_) => {
                        return Err(mlua::Error::runtime(
                            "ctx.process.stdin values must be finite strings or numbers",
                        ))
                    }
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "ctx.process.stdin entries must be strings or numbers, got {}",
                            other.type_name()
                        )))
                    }
                }
            }
            Ok(Some(lines.join("\n").into_bytes()))
        }
        other => Err(mlua::Error::runtime(format!(
            "ctx.process.stdin must be a string or an array of strings/numbers, got {}",
            other.type_name()
        ))),
    }
}

fn parse_process_command(spec: &Table) -> Result<Vec<String>, mlua::Error> {
    match spec.get::<Value>("command")? {
        Value::Table(values) => {
            let mut command = Vec::new();
            for value in values.sequence_values::<String>() {
                command.push(value?);
            }
            if command.is_empty() {
                Err(mlua::Error::runtime(
                    "ctx.process.command must contain at least one string",
                ))
            } else {
                Ok(command)
            }
        }
        Value::Nil => Err(mlua::Error::runtime("ctx.process.command is required")),
        other => Err(mlua::Error::runtime(format!(
            "ctx.process.command must be an array of strings, got {}",
            other.type_name()
        ))),
    }
}

fn optional_string_field(
    table: &Table,
    field: &str,
    context: &str,
) -> Result<Option<String>, mlua::Error> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(None),
        Value::String(value) => Ok(Some(value.to_str()?.trim().to_string())),
        other => Err(mlua::Error::runtime(format!(
            "{context}.{field} must be a string, got {}",
            other.type_name()
        ))),
    }
}

fn mode_label(mode: &Mode) -> &'static str {
    match mode {
        Mode::Normal => "normal",
        Mode::Search => "search",
        Mode::Help => "help",
        Mode::Logs => "logs",
        Mode::Command => "command",
        Mode::MultiChart => "mchart",
        Mode::AttributeCreateDialog => "attribute-create-dialog",
        Mode::AttributeDeleteDialog => "attribute-delete-dialog",
        Mode::FixedStringOverflowDialog => "fixed-string-overflow-dialog",
        Mode::FixedStringResizeDialog => "fixed-string-resize-dialog",
    }
}
