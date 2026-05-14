use std::cell::RefCell;

use mlua::{Function, Lua, Table, Value};

use crate::{
    configure::errors::ConfigureErrors,
    error::AppError,
    ui::input::keymap::{
        exported_action_codes, exported_mode_codes, is_valid_action_name_for_scope,
        parse_attributes_action_name, parse_content_action_name, parse_global_action_name,
        parse_key_pattern, parse_multichart_action_name, parse_normal_action_name,
        parse_tree_action_name, parse_window_action_name, BoundAction, KeyBinding, KeymapConfig,
        KeymapScope, ScopeKeymapConfig,
    },
};

thread_local! {
    static KEYMAP_LUA_RUNTIME: RefCell<Option<Lua>> = const { RefCell::new(None) };
}

pub(super) fn build_mode_constants_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let modes = lua.create_table()?;
    for (symbol, code) in exported_mode_codes() {
        modes.set(*symbol, *code)?;
    }
    Ok(modes)
}

pub(super) fn build_action_constants_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let actions = lua.create_table()?;
    for action in exported_action_codes() {
        actions.set(action.symbol, action.code)?;
    }
    Ok(actions)
}

fn register_lua_keymap_callback(
    keymaps: &Table,
    callback: Function,
) -> Result<String, ConfigureErrors> {
    let callbacks = match keymaps.get::<Value>("__lua_callbacks")? {
        Value::Table(table) => table,
        _ => {
            return Err(mlua::Error::runtime("h5v.keymaps.__lua_callbacks must be a table").into())
        }
    };
    let next_id = match keymaps.get::<Value>("__next_lua_callback_id")? {
        Value::Integer(value) if value > 0 => value,
        _ => 1,
    };
    let callback_id = format!("callback-{next_id}");
    callbacks.set(callback_id.as_str(), callback)?;
    keymaps.set("__next_lua_callback_id", next_id + 1)?;
    Ok(callback_id)
}

fn parse_lua_keymap_scope(mode: &str) -> Result<KeymapScope, mlua::Error> {
    KeymapScope::parse(mode)
        .ok_or_else(|| mlua::Error::runtime(format!("Unknown keymap scope '{mode}'")))
}

fn ensure_keymap_scope_table(
    lua: &Lua,
    keymaps: &Table,
    scope: KeymapScope,
) -> Result<Table, mlua::Error> {
    match keymaps.get::<Value>(scope.as_str())? {
        Value::Nil => {
            let created = lua.create_table()?;
            keymaps.set(scope.as_str(), created.clone())?;
            Ok(created)
        }
        Value::Table(table) => Ok(table),
        other => Err(mlua::Error::runtime(format!(
            "h5v.keymaps.{} must be a table, got {}",
            scope.as_str(),
            other.type_name()
        ))),
    }
}

fn ensure_keymap_scope_array_field(
    lua: &Lua,
    keymaps: &Table,
    scope: KeymapScope,
    field_name: &str,
    expected: &str,
) -> Result<Table, mlua::Error> {
    let scope_table = ensure_keymap_scope_table(lua, keymaps, scope)?;
    match scope_table.get::<Value>(field_name)? {
        Value::Nil => {
            let created = lua.create_table()?;
            scope_table.set(field_name, created.clone())?;
            Ok(created)
        }
        Value::Table(table) => Ok(table),
        other => Err(mlua::Error::runtime(format!(
            "h5v.keymaps.{}.{} must be {}, got {}",
            scope.as_str(),
            field_name,
            expected,
            other.type_name()
        ))),
    }
}

fn append_keymap_bind_entry(
    lua: &Lua,
    keymaps: &Table,
    scope: KeymapScope,
    entry: Table,
) -> Result<(), mlua::Error> {
    let bind = ensure_keymap_scope_array_field(lua, keymaps, scope, "bind", "an array of tables")?;
    bind.set(bind.raw_len() + 1, entry)?;
    Ok(())
}

pub(super) fn store_keymap_lua_runtime(lua: Lua) {
    KEYMAP_LUA_RUNTIME.with(|runtime| {
        *runtime.borrow_mut() = Some(lua);
    });
}

pub fn with_keymap_lua_callback<R>(
    callback_id: &str,
    run: impl FnOnce(&Lua, Function) -> Result<R, AppError>,
) -> Result<R, AppError> {
    KEYMAP_LUA_RUNTIME.with(|runtime| {
        let runtime = runtime.borrow();
        let lua = runtime.as_ref().ok_or_else(|| {
            AppError::InvalidCommand("Lua keymap runtime is not available".to_string())
        })?;
        let h5v: Table = lua
            .globals()
            .get("h5v")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let keymaps: Table = h5v
            .get("keymaps")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let callbacks: Table = keymaps
            .get("__lua_callbacks")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let callback: Function = callbacks
            .get(callback_id)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        run(lua, callback)
    })
}

fn append_keymap_binding(
    lua: &Lua,
    keymaps: &Table,
    scope: KeymapScope,
    key: &str,
    field_name: &str,
    field_value: &str,
    description: Option<String>,
) -> Result<(), mlua::Error> {
    parse_key_pattern(key).map_err(mlua::Error::runtime)?;

    let entry = lua.create_table()?;
    entry.set("key", key)?;
    entry.set(field_name, field_value)?;
    if let Some(description) = description {
        entry.set("description", description)?;
    }
    append_keymap_bind_entry(lua, keymaps, scope, entry)
}

fn append_keymap_unbind(
    lua: &Lua,
    keymaps: &Table,
    scope: KeymapScope,
    key: &str,
) -> Result<(), mlua::Error> {
    parse_key_pattern(key).map_err(mlua::Error::runtime)?;
    let unbind =
        ensure_keymap_scope_array_field(lua, keymaps, scope, "unbind", "an array of strings")?;
    unbind.set(unbind.raw_len() + 1, lua.create_string(key)?)?;
    Ok(())
}

pub(super) fn build_keymaps_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let keymaps = lua.create_table()?;
    keymaps.set("__lua_callbacks", lua.create_table()?)?;
    keymaps.set("__next_lua_callback_id", 1)?;

    let bind_table = keymaps.clone();
    let bind_fn = lua.create_function(
        move |lua, (mode, key, action, description): (String, String, String, Option<String>)| {
            let scope = parse_lua_keymap_scope(&mode)?;
            if !is_valid_action_name_for_scope(scope, &action) {
                return Err(mlua::Error::runtime(format!(
                    "Unknown action '{action}' for bind({}, ...)",
                    scope.as_str()
                )));
            }
            append_keymap_binding(
                lua,
                &bind_table,
                scope,
                &key,
                "action",
                &action,
                description,
            )
        },
    )?;
    keymaps.set("bind", bind_fn.clone())?;

    let bind_command_table = keymaps.clone();
    let bind_command_fn = lua.create_function(
        move |lua, (mode, key, command, description): (String, String, String, Option<String>)| {
            let scope = parse_lua_keymap_scope(&mode)?;
            if command.trim().is_empty() {
                return Err(mlua::Error::runtime("bind_command command cannot be empty"));
            }
            append_keymap_binding(
                lua,
                &bind_command_table,
                scope,
                &key,
                "command",
                &command,
                description,
            )
        },
    )?;
    keymaps.set("bind_command", bind_command_fn.clone())?;

    let bind_commands_table = keymaps.clone();
    let bind_commands_fn = lua.create_function(
        move |lua, (mode, key, commands, description): (String, String, Table, Option<String>)| {
            let scope = parse_lua_keymap_scope(&mode)?;
            let entry = lua.create_table()?;
            entry.set("key", key)?;
            entry.set("commands", commands)?;
            if let Some(description) = description {
                entry.set("description", description)?;
            }
            append_keymap_bind_entry(lua, &bind_commands_table, scope, entry)
        },
    )?;
    keymaps.set("bind_commands", bind_commands_fn.clone())?;

    let bind_script_table = keymaps.clone();
    let bind_script_fn = lua.create_function(
        move |lua, (mode, key, script, description): (String, String, String, Option<String>)| {
            let scope = KeymapScope::parse(&mode)
                .ok_or_else(|| mlua::Error::runtime(format!("Unknown keymap scope '{mode}'")))?;
            if script.trim().is_empty() {
                return Err(mlua::Error::runtime("bind_script script cannot be empty"));
            }
            append_keymap_binding(
                lua,
                &bind_script_table,
                scope,
                &key,
                "script",
                &script,
                description,
            )
        },
    )?;
    keymaps.set("bind_script", bind_script_fn.clone())?;

    let bind_lua_table = keymaps.clone();
    let bind_lua_fn =
        lua.create_function(
            move |lua,
                  (mode, key, callback, description): (
                String,
                String,
                Function,
                Option<String>,
            )| {
                let scope = parse_lua_keymap_scope(&mode)?;
                parse_key_pattern(&key).map_err(mlua::Error::runtime)?;
                let entry = lua.create_table()?;
                entry.set("key", key)?;
                entry.set("lua", callback)?;
                if let Some(description) = description {
                    entry.set("description", description)?;
                }
                append_keymap_bind_entry(lua, &bind_lua_table, scope, entry)
            },
        )?;
    keymaps.set("bind_lua", bind_lua_fn.clone())?;

    let unbind_table = keymaps.clone();
    let unbind_fn = lua.create_function(move |lua, (mode, key): (String, String)| {
        let scope = parse_lua_keymap_scope(&mode)?;
        append_keymap_unbind(lua, &unbind_table, scope, &key)
    })?;
    keymaps.set("unbind", unbind_fn.clone())?;

    lua.globals().set("bind", bind_fn)?;
    lua.globals().set("bind_command", bind_command_fn)?;
    lua.globals().set("bind_commands", bind_commands_fn)?;
    lua.globals().set("bind_script", bind_script_fn)?;
    lua.globals().set("bind_lua", bind_lua_fn)?;
    lua.globals().set("unbind", unbind_fn)?;

    Ok(keymaps)
}

pub(super) fn parse_keymaps_config(h5v: &Table) -> Result<Option<KeymapConfig>, ConfigureErrors> {
    let keymaps = match h5v.get::<Value>("keymaps")? {
        Value::Nil => return Ok(None),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.keymaps must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    Ok(Some(KeymapConfig {
        global: parse_scope_keymap_config(&keymaps, "global", parse_global_action_name)?,
        normal: parse_scope_keymap_config(&keymaps, "normal", parse_normal_action_name)?,
        window: parse_scope_keymap_config(&keymaps, "window", parse_window_action_name)?,
        tree: parse_scope_keymap_config(&keymaps, "tree", parse_tree_action_name)?,
        content: parse_scope_keymap_config(&keymaps, "content", parse_content_action_name)?,
        heatmap: parse_scope_keymap_config(&keymaps, "heatmap", parse_content_action_name)?,
        attributes: parse_scope_keymap_config(
            &keymaps,
            "attributes",
            parse_attributes_action_name,
        )?,
        multichart: parse_scope_keymap_config(&keymaps, "mchart", parse_multichart_action_name)?,
    }))
}

fn parse_scope_keymap_config<T: Clone>(
    keymaps: &Table,
    scope_name: &str,
    parse_action: impl Fn(&str) -> Option<T>,
) -> Result<ScopeKeymapConfig<T>, ConfigureErrors> {
    let scope = match keymaps.get::<Value>(scope_name)? {
        Value::Nil => return Ok(ScopeKeymapConfig::default()),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.keymaps.{scope_name} must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let clear_defaults = match scope.get::<Value>("clear_defaults")? {
        Value::Nil => false,
        Value::Boolean(value) => value,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.keymaps.{scope_name}.clear_defaults must be a boolean, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let unbind = match scope.get::<Value>("unbind")? {
        Value::Nil => Vec::new(),
        Value::Table(values) => {
            let mut patterns = Vec::new();
            for value in values.sequence_values::<Value>() {
                let value = value?;
                let Value::String(value) = value else {
                    return Err(mlua::Error::runtime(format!(
                        "h5v.keymaps.{scope_name}.unbind entries must be strings"
                    ))
                    .into());
                };
                let pattern = parse_key_pattern(value.to_str()?.as_ref())
                    .map_err(mlua::Error::runtime)
                    .map_err(ConfigureErrors::from)?;
                patterns.push(pattern);
            }
            patterns
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.keymaps.{scope_name}.unbind must be an array of strings, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let bind = match scope.get::<Value>("bind")? {
        Value::Nil => Vec::new(),
        Value::Table(values) => {
            let mut bindings = Vec::new();
            for value in values.sequence_values::<Value>() {
                let value = value?;
                let Value::Table(entry) = value else {
                    return Err(mlua::Error::runtime(format!(
                        "h5v.keymaps.{scope_name}.bind entries must be tables"
                    ))
                    .into());
                };
                let key = match entry.get::<Value>("key")? {
                    Value::String(value) => parse_key_pattern(value.to_str()?.as_ref())
                        .map_err(mlua::Error::runtime)
                        .map_err(ConfigureErrors::from)?,
                    Value::Nil => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.key is required"
                        ))
                        .into())
                    }
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.key must be a string, got {}",
                            other.type_name()
                        ))
                        .into())
                    }
                };
                let action = match entry.get::<Value>("action")? {
                    Value::Nil => None,
                    Value::String(value) => {
                        let value = value.to_str()?;
                        let parsed = parse_action(value.as_ref()).ok_or_else(|| {
                            mlua::Error::runtime(format!(
                                "Unknown action '{}' for h5v.keymaps.{scope_name}.bind",
                                value
                            ))
                        })?;
                        Some(parsed)
                    }
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.action must be a string, got {}",
                            other.type_name()
                        ))
                        .into())
                    }
                };
                let command = match entry.get::<Value>("command")? {
                    Value::Nil => None,
                    Value::String(value) => Some(value.to_str()?.to_string()),
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.command must be a string, got {}",
                            other.type_name()
                        ))
                        .into())
                    }
                };
                let script = match entry.get::<Value>("script")? {
                    Value::Nil => None,
                    Value::String(value) => Some(value.to_str()?.to_string()),
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.script must be a string, got {}",
                            other.type_name()
                        ))
                        .into())
                    }
                };
                let commands = match entry.get::<Value>("commands")? {
                    Value::Nil => None,
                    Value::Table(values) => {
                        let mut commands = Vec::new();
                        for value in values.sequence_values::<Value>() {
                            let value = value?;
                            let Value::String(value) = value else {
                                return Err(mlua::Error::runtime(format!(
                                    "h5v.keymaps.{scope_name}.bind.commands entries must be strings"
                                ))
                                .into());
                            };
                            commands.push(value.to_str()?.to_string());
                        }
                        Some(commands)
                    }
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.commands must be an array of strings, got {}",
                            other.type_name()
                        ))
                        .into())
                    }
                };
                let lua_callback = match entry.get::<Value>("lua")? {
                    Value::Nil => None,
                    Value::String(value) => Some(value.to_str()?.to_string()),
                    Value::Function(callback) => Some(register_lua_keymap_callback(keymaps, callback)?),
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.lua must be a function or callback id string, got {}",
                            other.type_name()
                        ))
                        .into())
                    }
                };
                let description = match entry.get::<Value>("description")? {
                    Value::Nil => None,
                    Value::String(value) => Some(value.to_str()?.to_string()),
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.description must be a string, got {}",
                            other.type_name()
                        ))
                        .into())
                    }
                };

                if let Some(commands) = &commands {
                    if commands.is_empty() {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.commands cannot be empty"
                        ))
                        .into());
                    }
                }

                let selected = [
                    action.is_some(),
                    command.is_some(),
                    script.is_some(),
                    commands.is_some(),
                    lua_callback.is_some(),
                ]
                .into_iter()
                .filter(|selected| *selected)
                .count();
                if selected == 0 {
                    return Err(mlua::Error::runtime(format!(
                        "h5v.keymaps.{scope_name}.bind entries must set action, command, script, commands, or lua"
                    ))
                    .into());
                }
                if selected > 1 {
                    return Err(mlua::Error::runtime(format!(
                        "h5v.keymaps.{scope_name}.bind entries must set exactly one of action, command, script, commands, or lua"
                    ))
                    .into());
                }

                let target = match (action, command, script, commands, lua_callback) {
                    (Some(action), None, None, None, None) => BoundAction::Action(action),
                    (None, Some(command), None, None, None) => {
                        if command.trim().is_empty() {
                            return Err(mlua::Error::runtime(format!(
                                "h5v.keymaps.{scope_name}.bind.command cannot be empty"
                            ))
                            .into());
                        }
                        BoundAction::Command(command)
                    }
                    (None, None, Some(script), None, None) => {
                        if script.trim().is_empty() {
                            return Err(mlua::Error::runtime(format!(
                                "h5v.keymaps.{scope_name}.bind.script cannot be empty"
                            ))
                            .into());
                        }
                        BoundAction::Script(script)
                    }
                    (None, None, None, Some(commands), None) => {
                        BoundAction::Script(commands.join("\n"))
                    }
                    (None, None, None, None, Some(callback_id)) => {
                        BoundAction::LuaCallback(callback_id)
                    }
                    _ => unreachable!("validated exactly one keymap binding payload"),
                };

                bindings.push(KeyBinding {
                    key,
                    target,
                    description,
                });
            }
            bindings
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.keymaps.{scope_name}.bind must be an array of tables, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    Ok(ScopeKeymapConfig {
        clear_defaults,
        unbind,
        bind,
    })
}
