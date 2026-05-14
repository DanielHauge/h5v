use std::{cell::RefCell, sync::mpsc::Sender};

use mlua::{Function, Lua, Table, Value};

use crate::{
    configure::{self, current_content_mode_order, SymbolThemeName, ThemeName},
    error::AppError,
    ui::{
        app::AppEvent,
        input::keymap::{
            exported_action_codes, exported_mode_codes, parse_attributes_action_name,
            parse_content_action_name, parse_global_action_name, parse_key_pattern,
            parse_multichart_action_name, parse_normal_action_name, parse_tree_action_name,
            parse_window_action_name, BoundAction, KeyBinding, KeymapConfig, KeymapScope,
            ScopeKeymapConfig,
        },
        state::{
            AppToast, ContentShowMode, HeatmapColormap, HeatmapNormalization, HeatmapRangeBound,
            HeatmapRangeMode, HeatmapSettings,
        },
    },
};

use super::{errors::ConfigureErrors, loading};

thread_local! {
    static KEYMAP_LUA_RUNTIME: RefCell<Option<Lua>> = const { RefCell::new(None) };
}

fn default_symbol_theme_for_compatibility(compatibility: bool) -> SymbolThemeName {
    if compatibility {
        SymbolThemeName::Compatibility
    } else {
        SymbolThemeName::Rich
    }
}

fn build_mode_constants_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let modes = lua.create_table()?;
    for (symbol, code) in exported_mode_codes() {
        modes.set(*symbol, *code)?;
    }
    Ok(modes)
}

fn build_action_constants_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let actions = lua.create_table()?;
    for action in exported_action_codes() {
        actions.set(action.symbol, action.code)?;
    }
    Ok(actions)
}

fn parse_action_for_scope(scope: KeymapScope, action: &str) -> bool {
    match scope {
        KeymapScope::Global => parse_global_action_name(action).is_some(),
        KeymapScope::Normal => parse_normal_action_name(action).is_some(),
        KeymapScope::Window => parse_window_action_name(action).is_some(),
        KeymapScope::Tree => parse_tree_action_name(action).is_some(),
        KeymapScope::Content | KeymapScope::Heatmap => parse_content_action_name(action).is_some(),
        KeymapScope::Attributes => parse_attributes_action_name(action).is_some(),
        KeymapScope::MultiChart => parse_multichart_action_name(action).is_some(),
    }
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

pub fn store_keymap_lua_runtime(lua: Lua) {
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

    let scope_table = match keymaps.get::<Value>(scope.as_str())? {
        Value::Nil => {
            let created = lua.create_table()?;
            keymaps.set(scope.as_str(), created.clone())?;
            created
        }
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.keymaps.{} must be a table, got {}",
                scope.as_str(),
                other.type_name()
            )))
        }
    };
    let bind = match scope_table.get::<Value>("bind")? {
        Value::Nil => {
            let created = lua.create_table()?;
            scope_table.set("bind", created.clone())?;
            created
        }
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.keymaps.{}.bind must be an array of tables, got {}",
                scope.as_str(),
                other.type_name()
            )))
        }
    };

    let entry = lua.create_table()?;
    entry.set("key", key)?;
    entry.set(field_name, field_value)?;
    if let Some(description) = description {
        entry.set("description", description)?;
    }
    bind.set(bind.raw_len() + 1, entry)?;
    Ok(())
}

fn append_keymap_unbind(
    lua: &Lua,
    keymaps: &Table,
    scope: KeymapScope,
    key: &str,
) -> Result<(), mlua::Error> {
    parse_key_pattern(key).map_err(mlua::Error::runtime)?;

    let scope_table = match keymaps.get::<Value>(scope.as_str())? {
        Value::Nil => {
            let created = lua.create_table()?;
            keymaps.set(scope.as_str(), created.clone())?;
            created
        }
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.keymaps.{} must be a table, got {}",
                scope.as_str(),
                other.type_name()
            )))
        }
    };
    let unbind = match scope_table.get::<Value>("unbind")? {
        Value::Nil => {
            let created = lua.create_table()?;
            scope_table.set("unbind", created.clone())?;
            created
        }
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.keymaps.{}.unbind must be an array of strings, got {}",
                scope.as_str(),
                other.type_name()
            )))
        }
    };
    unbind.set(unbind.raw_len() + 1, lua.create_string(key)?)?;
    Ok(())
}

fn build_keymaps_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let keymaps = lua.create_table()?;
    keymaps.set("__lua_callbacks", lua.create_table()?)?;
    keymaps.set("__next_lua_callback_id", 1)?;

    let bind_table = keymaps.clone();
    let bind_fn = lua.create_function(
        move |lua, (mode, key, action, description): (String, String, String, Option<String>)| {
            let scope = KeymapScope::parse(&mode)
                .ok_or_else(|| mlua::Error::runtime(format!("Unknown keymap scope '{mode}'")))?;
            if !parse_action_for_scope(scope, &action) {
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
            let scope = KeymapScope::parse(&mode)
                .ok_or_else(|| mlua::Error::runtime(format!("Unknown keymap scope '{mode}'")))?;
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
            let scope = KeymapScope::parse(&mode)
                .ok_or_else(|| mlua::Error::runtime(format!("Unknown keymap scope '{mode}'")))?;
            let entry = lua.create_table()?;
            entry.set("key", key)?;
            entry.set("commands", commands)?;
            if let Some(description) = description {
                entry.set("description", description)?;
            }
            let scope_table = match bind_commands_table.get::<Value>(scope.as_str())? {
                Value::Nil => {
                    let created = lua.create_table()?;
                    bind_commands_table.set(scope.as_str(), created.clone())?;
                    created
                }
                Value::Table(table) => table,
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "h5v.keymaps.{} must be a table, got {}",
                        scope.as_str(),
                        other.type_name()
                    )))
                }
            };
            let bind = match scope_table.get::<Value>("bind")? {
                Value::Nil => {
                    let created = lua.create_table()?;
                    scope_table.set("bind", created.clone())?;
                    created
                }
                Value::Table(table) => table,
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "h5v.keymaps.{}.bind must be an array of tables, got {}",
                        scope.as_str(),
                        other.type_name()
                    )))
                }
            };
            bind.set(bind.raw_len() + 1, entry)?;
            Ok(())
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
                let scope = KeymapScope::parse(&mode).ok_or_else(|| {
                    mlua::Error::runtime(format!("Unknown keymap scope '{mode}'"))
                })?;
                parse_key_pattern(&key).map_err(mlua::Error::runtime)?;

                let scope_table = match bind_lua_table.get::<Value>(scope.as_str())? {
                    Value::Nil => {
                        let created = lua.create_table()?;
                        bind_lua_table.set(scope.as_str(), created.clone())?;
                        created
                    }
                    Value::Table(table) => table,
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{} must be a table, got {}",
                            scope.as_str(),
                            other.type_name()
                        )))
                    }
                };
                let bind = match scope_table.get::<Value>("bind")? {
                    Value::Nil => {
                        let created = lua.create_table()?;
                        scope_table.set("bind", created.clone())?;
                        created
                    }
                    Value::Table(table) => table,
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{}.bind must be an array of tables, got {}",
                            scope.as_str(),
                            other.type_name()
                        )))
                    }
                };
                let entry = lua.create_table()?;
                entry.set("key", key)?;
                entry.set("lua", callback)?;
                if let Some(description) = description {
                    entry.set("description", description)?;
                }
                bind.set(bind.raw_len() + 1, entry)?;
                Ok(())
            },
        )?;
    keymaps.set("bind_lua", bind_lua_fn.clone())?;

    let unbind_table = keymaps.clone();
    let unbind_fn = lua.create_function(move |lua, (mode, key): (String, String)| {
        let scope = KeymapScope::parse(&mode)
            .ok_or_else(|| mlua::Error::runtime(format!("Unknown keymap scope '{mode}'")))?;
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

fn build_h5v_table(
    lua: &Lua,
    events: Option<Sender<AppEvent>>,
    default_compatibility: bool,
) -> Result<Table, ConfigureErrors> {
    let h5v = lua.create_table()?;

    let log_fn = match events {
        Some(events) => lua.create_function(move |_, msg: String| {
            let _ = events.send(AppEvent::Toast(AppToast::Info(msg)));
            Ok(())
        })?,
        None => lua.create_function(|_, _: String| Ok(()))?,
    };

    h5v.set("log", log_fn)?;
    h5v.set("compatibility", default_compatibility)?;
    h5v.set("theme", ThemeName::Dark.as_str())?;
    h5v.set(
        "content_mode_order",
        lua.create_sequence_from(
            current_content_mode_order()
                .into_iter()
                .map(ContentShowMode::as_str),
        )?,
    )?;
    h5v.set(
        "symbol_theme",
        default_symbol_theme_for_compatibility(default_compatibility).as_str(),
    )?;
    h5v.set(
        "colors",
        build_empty_nested_table(lua, configure::available_color_names().iter().copied())?,
    )?;
    h5v.set(
        "symbols",
        build_empty_nested_table(lua, configure::available_symbol_names().iter().copied())?,
    )?;
    h5v.set("themes", build_theme_table(lua)?)?;
    h5v.set("symbol_themes", build_symbol_theme_table(lua)?)?;
    h5v.set("heatmap", build_heatmap_table(lua)?)?;
    h5v.set("modes", build_mode_constants_table(lua)?)?;
    h5v.set("actions", build_action_constants_table(lua)?)?;
    h5v.set("keymaps", build_keymaps_table(lua)?)?;
    Ok(h5v)
}

fn build_empty_nested_table<'a>(
    lua: &Lua,
    dotted_names: impl IntoIterator<Item = &'a str>,
) -> Result<Table, ConfigureErrors> {
    let root = lua.create_table()?;
    for dotted_name in dotted_names {
        let mut table = root.clone();
        let mut parts = dotted_name.split('.').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                break;
            }
            let next = match table.get::<Value>(part)? {
                Value::Table(existing) => existing,
                Value::Nil => {
                    let created = lua.create_table()?;
                    table.set(part, created.clone())?;
                    created
                }
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "Theme export conflict at '{dotted_name}': expected table before '{part}', got {}",
                        other.type_name()
                    ))
                    .into());
                }
            };
            table = next;
        }
    }
    Ok(root)
}

fn parse_compatibility_override(h5v: &Table) -> Result<Option<bool>, ConfigureErrors> {
    match h5v.get::<Value>("compatibility")? {
        Value::Nil => Ok(None),
        Value::Boolean(value) => Ok(Some(value)),
        other => Err(mlua::Error::runtime(format!(
            "h5v.compatibility must be a boolean, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_content_mode_order(h5v: &Table) -> Result<Option<Vec<ContentShowMode>>, ConfigureErrors> {
    match h5v.get::<Value>("content_mode_order")? {
        Value::Nil => Ok(None),
        Value::Table(values) => {
            let mut order = Vec::new();
            for value in values.sequence_values::<Value>() {
                let value = value?;
                let Value::String(value) = value else {
                    return Err(mlua::Error::runtime(
                        "h5v.content_mode_order entries must be strings",
                    )
                    .into());
                };
                let value = value.to_str()?;
                let mode = ContentShowMode::parse(value.as_ref()).ok_or_else(|| {
                    mlua::Error::runtime(format!(
                        "Unknown content mode '{value}'. Available modes: preview, matrix, heatmap"
                    ))
                })?;
                if !order.contains(&mode) {
                    order.push(mode);
                }
            }
            if order.is_empty() {
                return Err(mlua::Error::runtime(
                    "h5v.content_mode_order must include at least one content mode",
                )
                .into());
            }
            Ok(Some(order))
        }
        other => Err(mlua::Error::runtime(format!(
            "h5v.content_mode_order must be an array of strings, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn build_heatmap_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let heatmap = lua.create_table()?;
    let defaults = configure::current_heatmap_default_settings();
    heatmap.set("default_range", defaults.range.label())?;
    heatmap.set("default_colormap", defaults.colormap.as_str())?;
    heatmap.set("default_normalization", defaults.normalization.as_str())?;
    heatmap.set("default_invert_x", defaults.invert_x)?;
    heatmap.set("default_invert_y", defaults.invert_y)?;
    heatmap.set("default_invert_c", defaults.invert_c)?;
    let range_modes = lua.create_table()?;
    for (index, mode) in configure::current_heatmap_range_modes()
        .into_iter()
        .enumerate()
    {
        let HeatmapRangeMode::Custom(custom) = mode else {
            continue;
        };
        let entry = lua.create_table()?;
        entry.set("label", custom.label)?;
        match custom.lower {
            HeatmapRangeBound::Exact(value) => entry.set("min", value.to_f64())?,
            HeatmapRangeBound::Percentile(bps) => {
                entry.set("min", format!("{}%", format_heatmap_percent(bps)))?
            }
        }
        match custom.upper {
            HeatmapRangeBound::Exact(value) => entry.set("max", value.to_f64())?,
            HeatmapRangeBound::Percentile(bps) => {
                entry.set("max", format!("{}%", format_heatmap_percent(bps)))?
            }
        }
        range_modes.set(index + 1, entry)?;
    }
    heatmap.set("range_modes", range_modes)?;
    Ok(heatmap)
}

fn parse_heatmap_config(
    h5v: &Table,
) -> Result<Option<(Vec<HeatmapRangeMode>, HeatmapSettings)>, ConfigureErrors> {
    let heatmap = match h5v.get::<Value>("heatmap")? {
        Value::Nil => return Ok(None),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let custom_modes = match heatmap.get::<Value>("range_modes")? {
        Value::Nil => Vec::new(),
        Value::Table(values) => {
            let mut modes = Vec::new();
            for value in values.sequence_values::<Value>() {
                let value = value?;
                let Value::Table(entry) = value else {
                    return Err(mlua::Error::runtime(
                        "h5v.heatmap.range_modes entries must be tables",
                    )
                    .into());
                };
                let lower = parse_heatmap_bound_value(entry.get::<Value>("min")?, "min")?;
                let upper = parse_heatmap_bound_value(entry.get::<Value>("max")?, "max")?;
                let label = match entry.get::<Value>("label")? {
                    Value::Nil => None,
                    Value::String(value) => Some(value.to_str()?.to_string()),
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.heatmap.range_modes.label must be a string, got {}",
                            other.type_name()
                        ))
                        .into())
                    }
                };
                modes.push(HeatmapRangeMode::custom(lower, upper, label));
            }
            modes
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap.range_modes must be an array of tables, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let mut available = HeatmapRangeMode::default_modes();
    for mode in &custom_modes {
        let label = mode.label();
        if available
            .iter()
            .any(|existing| existing.label().eq_ignore_ascii_case(&label))
        {
            return Err(
                mlua::Error::runtime(format!("Duplicate heatmap range label '{}'", label)).into(),
            );
        }
        available.push(mode.clone());
    }

    let mut default_settings = HeatmapSettings::default();

    default_settings.range = match heatmap.get::<Value>("default_range")? {
        Value::Nil => default_settings.range,
        Value::String(value) => {
            let selector = value.to_str()?;
            available
                .iter()
                .find(|mode| mode.selector_matches(selector.as_ref()))
                .cloned()
                .ok_or_else(|| {
                    mlua::Error::runtime(format!(
                        "Unknown heatmap default range '{}'. Expected one of: {}",
                        selector,
                        available
                            .iter()
                            .map(|mode| mode.label())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                })?
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap.default_range must be a string, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    default_settings.colormap = match heatmap.get::<Value>("default_colormap")? {
        Value::Nil => default_settings.colormap,
        Value::String(value) => {
            HeatmapColormap::parse(value.to_str()?.as_ref()).ok_or_else(|| {
                mlua::Error::runtime(
                    "Unknown heatmap colormap. Expected one of: turbo, grayscale, inferno",
                )
            })?
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap.default_colormap must be a string, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    default_settings.normalization = match heatmap.get::<Value>("default_normalization")? {
        Value::Nil => default_settings.normalization,
        Value::String(value) => {
            HeatmapNormalization::parse(value.to_str()?.as_ref()).ok_or_else(|| {
                mlua::Error::runtime(
                    "Unknown heatmap normalization. Expected one of: linear, log, sqrt",
                )
            })?
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap.default_normalization must be a string, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    default_settings.invert_x = parse_heatmap_bool_field(&heatmap, "default_invert_x")?
        .unwrap_or(default_settings.invert_x);
    default_settings.invert_y = parse_heatmap_bool_field(&heatmap, "default_invert_y")?
        .unwrap_or(default_settings.invert_y);
    default_settings.invert_c = parse_heatmap_bool_field(&heatmap, "default_invert_c")?
        .unwrap_or(default_settings.invert_c);

    Ok(Some((custom_modes, default_settings)))
}

fn parse_heatmap_bound_value(
    value: Value,
    field_name: &str,
) -> Result<HeatmapRangeBound, ConfigureErrors> {
    match value {
        Value::String(value) => HeatmapRangeBound::parse(value.to_str()?.as_ref())
            .map_err(mlua::Error::runtime)
            .map_err(Into::into),
        Value::Integer(value) => HeatmapRangeBound::parse(&value.to_string())
            .map_err(mlua::Error::runtime)
            .map_err(Into::into),
        Value::Number(value) => HeatmapRangeBound::parse(&value.to_string())
            .map_err(mlua::Error::runtime)
            .map_err(Into::into),
        Value::Nil => Err(mlua::Error::runtime(format!(
            "h5v.heatmap.range_modes.{field_name} is required"
        ))
        .into()),
        other => Err(mlua::Error::runtime(format!(
            "h5v.heatmap.range_modes.{field_name} must be a string or number, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_heatmap_bool_field(
    heatmap: &Table,
    field_name: &str,
) -> Result<Option<bool>, ConfigureErrors> {
    match heatmap.get::<Value>(field_name)? {
        Value::Nil => Ok(None),
        Value::Boolean(value) => Ok(Some(value)),
        other => Err(mlua::Error::runtime(format!(
            "h5v.heatmap.{field_name} must be a boolean, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_keymaps_config(h5v: &Table) -> Result<Option<KeymapConfig>, ConfigureErrors> {
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
                    Value::Function(callback) => Some(register_lua_keymap_callback(keymaps, callback)?),
                    Value::String(value) => Some(value.to_str()?.to_string()),
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
                let commands_script = match commands {
                    Some(commands) if commands.is_empty() => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.commands cannot be empty"
                        ))
                        .into())
                    }
                    Some(commands) => Some(commands.join("\n")),
                    None => None,
                };
                let target_count = usize::from(action.is_some())
                    + usize::from(command.is_some())
                    + usize::from(script.is_some())
                    + usize::from(commands_script.is_some())
                    + usize::from(lua_callback.is_some());
                if target_count == 0 {
                    return Err(mlua::Error::runtime(format!(
                        "h5v.keymaps.{scope_name}.bind entries must set action, command, script, commands, or lua"
                    ))
                    .into());
                }
                if target_count > 1 {
                    return Err(mlua::Error::runtime(format!(
                        "h5v.keymaps.{scope_name}.bind entries must set exactly one of action, command, script, commands, or lua"
                    ))
                    .into());
                }
                let target = match (action, command, script, commands_script, lua_callback) {
                    (Some(action), None, None, None, None) => BoundAction::Action(action),
                    (None, Some(command), None, None, None) if !command.trim().is_empty() => {
                        BoundAction::Command(command)
                    }
                    (None, Some(_), None, None, None) => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.command cannot be empty"
                        ))
                        .into())
                    }
                    (None, None, Some(script), None, None) if !script.trim().is_empty() => {
                        BoundAction::Script(script)
                    }
                    (None, None, Some(_), None, None) => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.keymaps.{scope_name}.bind.script cannot be empty"
                        ))
                        .into())
                    }
                    (None, None, None, Some(script), None) => BoundAction::Script(script),
                    (None, None, None, None, Some(callback_id)) => {
                        BoundAction::LuaCallback(callback_id)
                    }
                    _ => unreachable!("validated exactly one target"),
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

fn format_heatmap_percent(bps: u16) -> String {
    let whole = bps / 100;
    let frac = bps % 100;
    if frac == 0 {
        whole.to_string()
    } else if frac.is_multiple_of(10) {
        format!("{whole}.{}", frac / 10)
    } else {
        format!("{whole}.{frac:02}")
    }
}

fn execute_config_chunk(lua: &Lua, chunk_name: &str, config: &str) -> Result<(), ConfigureErrors> {
    lua.load(config).set_name(chunk_name).exec()?;
    Ok(())
}

pub fn load_config_compatibility(
    default_compatibility: bool,
) -> Result<Option<bool>, ConfigureErrors> {
    let lua = Lua::new();
    let h5v = build_h5v_table(&lua, None, default_compatibility)?;
    lua.globals().set("h5v", h5v.clone())?;
    lua.globals().set("h5v_modes", h5v.get::<Table>("modes")?)?;
    lua.globals()
        .set("h5v_actions", h5v.get::<Table>("actions")?)?;
    let config_path = loading::config_path()?;
    let chunk_name = format!("@{}", config_path.display());
    let config = loading::load_or_create_config()?;
    execute_config_chunk(&lua, &chunk_name, &config)?;
    parse_compatibility_override(&h5v)
}

pub fn run_lua_engine(
    events: Sender<AppEvent>,
    default_compatibility: bool,
) -> Result<(), ConfigureErrors> {
    let lua = Lua::new();
    let h5v = build_h5v_table(&lua, Some(events), default_compatibility)?;
    lua.globals().set("h5v", h5v.clone())?;
    lua.globals().set("h5v_modes", h5v.get::<Table>("modes")?)?;
    lua.globals()
        .set("h5v_actions", h5v.get::<Table>("actions")?)?;
    let config_path = loading::config_path()?;
    let chunk_name = format!("@{}", config_path.display());
    let config = loading::load_or_create_config()?;
    let previous_config = configure::snapshot_config();

    configure::reset_config(ThemeName::Dark);
    let result = (|| -> Result<(), ConfigureErrors> {
        execute_config_chunk(&lua, &chunk_name, &config)?;
        apply_lua_config(&h5v)?;
        Ok(())
    })();
    if result.is_err() {
        configure::restore_config(previous_config);
    } else {
        store_keymap_lua_runtime(lua);
    }
    result
}

fn build_theme_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let themes = lua.create_table()?;
    for theme_name in [ThemeName::Dark, ThemeName::Light] {
        let theme_table = lua.create_table()?;
        for (name, color) in configure::theme_named_colors(theme_name) {
            insert_string_value(
                lua,
                &theme_table,
                name,
                configure::color_to_lua_string(color),
            )?;
        }
        themes.set(theme_name.as_str(), theme_table)?;
    }
    Ok(themes)
}

fn build_symbol_theme_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let themes = lua.create_table()?;
    for theme_name in [SymbolThemeName::Rich, SymbolThemeName::Compatibility] {
        let theme_table = lua.create_table()?;
        for (name, value) in configure::theme_named_symbols(theme_name) {
            insert_string_value(lua, &theme_table, name, value)?;
        }
        themes.set(theme_name.as_str(), theme_table)?;
    }
    Ok(themes)
}

fn apply_lua_config(h5v: &Table) -> Result<(), ConfigureErrors> {
    let compatibility_override = parse_compatibility_override(h5v)?;
    let content_mode_order = parse_content_mode_order(h5v)?;
    let heatmap_config = parse_heatmap_config(h5v)?;
    let keymap_config = parse_keymaps_config(h5v)?;
    let selected_theme = match h5v.get::<Value>("theme")? {
        Value::Nil => ThemeName::Dark,
        Value::String(value) => {
            let value = value.to_str()?;
            ThemeName::parse(value.as_ref()).ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "Unknown theme '{value}'. Available themes: {}",
                    configure::available_theme_names().join(", ")
                ))
            })?
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.theme must be a string, got {}",
                other.type_name()
            ))
            .into());
        }
    };
    configure::reset_config(selected_theme);
    if let Some(order) = content_mode_order {
        configure::set_content_mode_order(&order);
    }
    if let Some((range_modes, default_settings)) = heatmap_config {
        configure::set_heatmap_ranges(&range_modes, &default_settings.range);
        configure::set_heatmap_default_settings(&default_settings);
    }
    if let Some(keymap_config) = keymap_config {
        configure::set_keymap_config(&keymap_config).map_err(mlua::Error::runtime)?;
    }

    let selected_symbol_theme = match h5v.get::<Value>("symbol_theme")? {
        Value::Nil => compatibility_override
            .map(default_symbol_theme_for_compatibility)
            .unwrap_or_else(configure::current_symbol_theme_name),
        Value::String(value) => {
            let value = value.to_str()?;
            SymbolThemeName::parse(value.as_ref()).ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "Unknown symbol theme '{value}'. Available symbol themes: {}",
                    configure::available_symbol_theme_names().join(", ")
                ))
            })?
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.symbol_theme must be a string, got {}",
                other.type_name()
            ))
            .into());
        }
    };
    configure::reset_symbol_theme(selected_symbol_theme);

    match h5v.get::<Value>("colors")? {
        Value::Nil => Ok(()),
        Value::Table(table) => apply_color_overrides(&table, None),
        other => Err(mlua::Error::runtime(format!(
            "h5v.colors must be a table, got {}",
            other.type_name()
        ))
        .into()),
    }?;

    match h5v.get::<Value>("symbols")? {
        Value::Nil => Ok(()),
        Value::Table(table) => apply_symbol_overrides(&table, None),
        other => Err(mlua::Error::runtime(format!(
            "h5v.symbols must be a table, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn insert_string_value(
    lua: &Lua,
    root: &Table,
    dotted_name: &str,
    value: impl Into<String>,
) -> Result<(), ConfigureErrors> {
    let value = value.into();
    let mut table = root.clone();
    let mut parts = dotted_name.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            table.set(part, value.clone())?;
        } else {
            let next = match table.get::<Value>(part)? {
                Value::Table(existing) => existing,
                Value::Nil => {
                    let created = lua.create_table()?;
                    table.set(part, created.clone())?;
                    created
                }
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "Theme export conflict at '{dotted_name}': expected table before '{part}', got {}",
                        other.type_name()
                    ))
                    .into());
                }
            };
            table = next;
        }
    }
    Ok(())
}

fn apply_color_overrides(table: &Table, prefix: Option<&str>) -> Result<(), ConfigureErrors> {
    for pair in table.pairs::<Value, Value>() {
        let (key, value) = pair?;
        let key = match key {
            Value::String(value) => value.to_str()?.to_string(),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "h5v.colors keys must be strings, got {}",
                    other.type_name()
                ))
                .into());
            }
        };

        let full_name = match prefix {
            Some(prefix) => format!("{prefix}.{key}"),
            None => key,
        };

        match value {
            Value::String(value) => {
                let value = value.to_str()?;
                let color = configure::parse_color(value.as_ref()).ok_or_else(|| {
                    mlua::Error::runtime(format!(
                        "Invalid color '{value}' for '{full_name}'. Use #RRGGBB or a named color."
                    ))
                })?;
                configure::set_color_override(&full_name, color).map_err(mlua::Error::runtime)?;
            }
            Value::Table(child) => apply_color_overrides(&child, Some(&full_name))?,
            other => {
                return Err(mlua::Error::runtime(format!(
                    "h5v.colors.{full_name} must be a string or table, got {}",
                    other.type_name()
                ))
                .into());
            }
        }
    }
    Ok(())
}

fn apply_symbol_overrides(table: &Table, prefix: Option<&str>) -> Result<(), ConfigureErrors> {
    for pair in table.pairs::<Value, Value>() {
        let (key, value) = pair?;
        let key = match key {
            Value::String(value) => value.to_str()?.to_string(),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "h5v.symbols keys must be strings, got {}",
                    other.type_name()
                ))
                .into());
            }
        };

        let full_name = match prefix {
            Some(prefix) => format!("{prefix}.{key}"),
            None => key,
        };

        match value {
            Value::String(value) => {
                let value = value.to_str()?;
                configure::set_symbol_override(&full_name, value.as_ref())
                    .map_err(mlua::Error::runtime)?;
            }
            Value::Table(child) => apply_symbol_overrides(&child, Some(&full_name))?,
            other => {
                return Err(mlua::Error::runtime(format!(
                    "h5v.symbols.{full_name} must be a string or table, got {}",
                    other.type_name()
                ))
                .into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        apply_lua_config, build_h5v_table, build_symbol_theme_table, build_theme_table,
        execute_config_chunk, parse_compatibility_override, parse_content_mode_order,
        parse_heatmap_config,
    };
    use crate::configure::{
        self, configured_symbol, current_content_mode_order, current_heatmap_default_settings,
        current_heatmap_range_modes, current_keymaps, themed_color, SymbolThemeName, ThemeName,
    };
    use crate::ui::input::keymap::{
        global_action, heatmap_action, BoundAction, ContentAction, GlobalAction,
    };
    use crate::ui::state::{
        ContentShowMode, HeatmapColormap, HeatmapNormalization, HeatmapRangeBound,
        HeatmapRangeMode, HeatmapStoredFloat,
    };
    use mlua::{Lua, Table, Value};
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::style::Color;

    #[test]
    fn applies_nested_lua_config_overrides() {
        let lua = Lua::new();
        let h5v = lua.create_table().expect("create h5v table");
        h5v.set("theme", ThemeName::Light.as_str())
            .expect("set theme");
        h5v.set("symbol_theme", SymbolThemeName::Compatibility.as_str())
            .expect("set symbol theme");

        let colors = lua.create_table().expect("create colors table");
        let content = lua.create_table().expect("create content table");
        content
            .set("app_brand", "#010203")
            .expect("set content.app_brand");
        colors.set("content", content).expect("set content table");
        let surface = lua.create_table().expect("create surface table");
        surface
            .set("title_bg", "#040506")
            .expect("set surface.title_bg");
        colors.set("surface", surface).expect("set surface table");
        h5v.set("colors", colors).expect("set colors");

        let symbols = lua.create_table().expect("create symbols table");
        let tree = lua.create_table().expect("create tree table");
        tree.set("root_file_icon", "FILE ")
            .expect("set tree.root_file_icon");
        symbols.set("tree", tree).expect("set tree symbol table");
        h5v.set("symbols", symbols).expect("set symbols");
        let order = lua.create_table().expect("create order table");
        order.set(1, "matrix").expect("set order");
        h5v.set("content_mode_order", order)
            .expect("set content mode order");

        apply_lua_config(&h5v).expect("apply config");

        assert_eq!(
            themed_color(|colors| colors.content.app_brand),
            Color::Rgb(1, 2, 3)
        );
        assert_eq!(
            themed_color(|colors| colors.surface.title_bg),
            Color::Rgb(4, 5, 6)
        );
        assert_eq!(
            configured_symbol(|symbols| symbols.tree.root_file_icon),
            "FILE "
        );
        assert_eq!(
            current_content_mode_order(),
            vec![
                ContentShowMode::Matrix,
                ContentShowMode::Preview,
                ContentShowMode::Heatmap
            ]
        );

        configure::reset_config(ThemeName::Dark);
    }

    #[test]
    fn applies_keymap_configuration() {
        let lua = Lua::new();
        let h5v = build_h5v_table(&lua, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            bind(h5v.modes.Global, "ctrl+h", h5v.actions.ShowHelp, "Show help")
            bind_commands(h5v.modes.Global, "ctrl+k", { "down 2", "up 1" }, "Run commands")
            unbind(h5v.modes.Heatmap, "v")
            bind(h5v.modes.Heatmap, "ctrl+z", h5v.actions.HeatmapZoomIn)
            bind_lua(h5v.modes.Heatmap, "ctrl+l", function(ctx)
              ctx.command("help reload")
            end, "Run lua")
        "#,
        )
        .exec()
        .expect("run keymap config");

        apply_lua_config(&h5v).expect("apply config");

        let keymaps = current_keymaps();
        assert_eq!(
            global_action(
                &KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
                &keymaps
            ),
            Some(BoundAction::Action(GlobalAction::ShowHelp))
        );
        assert!(matches!(
            global_action(
                &KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
                &keymaps
            ),
            Some(BoundAction::Script(script)) if script == "down 2\nup 1"
        ));
        assert_eq!(
            heatmap_action(
                &KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
                &keymaps
            ),
            Some(BoundAction::Action(ContentAction::HeatmapZoomIn))
        );
        assert!(matches!(
            heatmap_action(
                &KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
                &keymaps
            ),
            Some(BoundAction::LuaCallback(_))
        ));

        configure::reset_config(ThemeName::Dark);
    }

    #[test]
    fn named_config_chunk_reports_lua_path_and_line() {
        let lua = Lua::new();
        let error = execute_config_chunk(&lua, "@/tmp/init.lua", "h5v.theme =\n")
            .expect_err("invalid Lua should error");

        let rendered = error.to_string();
        assert!(rendered.contains("/tmp/init.lua:2"), "{rendered}");
        assert!(!rendered.contains("src/configure/lua.rs"));
    }

    #[test]
    fn exports_nested_theme_tables() {
        let lua = Lua::new();
        let themes = build_theme_table(&lua).expect("build themes");
        let dark: Table = themes.get("dark").expect("get dark theme");
        let content: Table = dark.get("content").expect("get dark content table");
        let surface: Table = dark.get("surface").expect("get dark surface table");

        assert_eq!(
            content
                .get::<String>("app_brand")
                .expect("get content.app_brand"),
            configure::color_to_lua_string(Color::Yellow)
        );
        assert_eq!(
            surface
                .get::<String>("panel_border")
                .expect("get surface.panel_border"),
            configure::color_to_lua_string(
                configure::theme_named_colors(ThemeName::Dark)
                    .into_iter()
                    .find(|(name, _)| *name == "surface.panel_border")
                    .expect("surface.panel_border exists")
                    .1
            )
        );

        let symbol_themes = build_symbol_theme_table(&lua).expect("build symbol themes");
        let rich: Table = symbol_themes.get("rich").expect("get rich symbol theme");
        let tree: Table = rich.get("tree").expect("get tree symbols");
        assert_eq!(
            tree.get::<String>("root_file_icon")
                .expect("get tree.root_file_icon"),
            "󰈚 "
        );
    }

    #[test]
    fn compatibility_override_drives_default_symbol_theme() {
        let lua = Lua::new();
        let h5v = lua.create_table().expect("create h5v table");
        h5v.set("compatibility", true).expect("set compatibility");
        h5v.set("theme", ThemeName::Dark.as_str())
            .expect("set theme");
        h5v.set("colors", lua.create_table().expect("create colors"))
            .expect("set colors");
        h5v.set("symbols", lua.create_table().expect("create symbols"))
            .expect("set symbols");

        apply_lua_config(&h5v).expect("apply config");

        assert_eq!(
            configure::current_symbol_theme_name(),
            SymbolThemeName::Compatibility
        );
        configure::reset_config(ThemeName::Dark);
    }

    #[test]
    fn compatibility_override_requires_boolean() {
        let lua = Lua::new();
        let h5v = lua.create_table().expect("create h5v table");
        h5v.set(
            "compatibility",
            Value::String(lua.create_string("yes").expect("create string")),
        )
        .expect("set compatibility");

        let error = parse_compatibility_override(&h5v).expect_err("non-bool should error");
        assert!(error
            .to_string()
            .contains("h5v.compatibility must be a boolean"));
    }

    #[test]
    fn content_mode_order_requires_known_modes() {
        let lua = Lua::new();
        let h5v = lua.create_table().expect("create h5v table");
        let order = lua.create_table().expect("create order");
        order.set(1, "bogus").expect("set order");
        h5v.set("content_mode_order", order).expect("set order");

        let error = parse_content_mode_order(&h5v).expect_err("unknown mode should error");
        assert!(error.to_string().contains("Unknown content mode"));
    }

    #[test]
    fn direct_nested_color_assignment_works_without_manual_table_setup() {
        let lua = Lua::new();
        let h5v = build_h5v_table(&lua, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(r#"h5v.colors.accent.selection_bg = "green""#)
            .exec()
            .expect("assign nested color override");

        apply_lua_config(&h5v).expect("apply config");
        assert_eq!(
            themed_color(|colors| colors.accent.selection_bg),
            Color::Green
        );
        configure::reset_config(ThemeName::Dark);
    }

    #[test]
    fn parses_heatmap_range_configuration() {
        let lua = Lua::new();
        let h5v = lua.create_table().expect("create h5v table");
        let heatmap = lua.create_table().expect("create heatmap table");
        let ranges = lua.create_table().expect("create ranges table");
        let entry = lua.create_table().expect("create range entry");
        entry.set("label", "5-80%").expect("set label");
        entry.set("min", "5%").expect("set min");
        entry.set("max", "80%").expect("set max");
        ranges.set(1, entry).expect("set range entry");
        heatmap.set("range_modes", ranges).expect("set range modes");
        heatmap
            .set("default_range", "5-80%")
            .expect("set default range");
        heatmap
            .set("default_colormap", "inferno")
            .expect("set default colormap");
        heatmap
            .set("default_normalization", "log")
            .expect("set default normalization");
        heatmap
            .set("default_invert_x", true)
            .expect("set default invert x");
        heatmap
            .set("default_invert_y", true)
            .expect("set default invert y");
        heatmap
            .set("default_invert_c", true)
            .expect("set default invert c");
        h5v.set("heatmap", heatmap).expect("set heatmap");
        let (range_modes, default_settings) = parse_heatmap_config(&h5v)
            .expect("parse heatmap config")
            .expect("heatmap config present");
        assert_eq!(
            range_modes,
            vec![HeatmapRangeMode::Custom(
                crate::ui::state::HeatmapCustomRangeMode {
                    label: "5-80%".to_string(),
                    lower: HeatmapRangeBound::Percentile(500),
                    upper: HeatmapRangeBound::Percentile(8000),
                }
            )]
        );
        assert_eq!(default_settings.range.label(), "5-80%");
        assert_eq!(default_settings.colormap, HeatmapColormap::Inferno);
        assert_eq!(default_settings.normalization, HeatmapNormalization::Log);
        assert!(default_settings.invert_x);
        assert!(default_settings.invert_y);
        assert!(default_settings.invert_c);
    }

    #[test]
    fn applies_heatmap_range_configuration() {
        let lua = Lua::new();
        let h5v = build_h5v_table(&lua, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            h5v.heatmap.range_modes = {
                { label = "2.5..5.5", min = 2.5, max = 5.5 },
            }
            h5v.heatmap.default_range = "2.5..5.5"
            h5v.heatmap.default_colormap = "inferno"
            h5v.heatmap.default_normalization = "sqrt"
            h5v.heatmap.default_invert_x = true
            h5v.heatmap.default_invert_c = true
        "#,
        )
        .exec()
        .expect("assign heatmap config");

        apply_lua_config(&h5v).expect("apply config");
        assert_eq!(
            current_heatmap_range_modes(),
            vec![HeatmapRangeMode::Custom(
                crate::ui::state::HeatmapCustomRangeMode {
                    label: "2.5..5.5".to_string(),
                    lower: HeatmapRangeBound::Exact(HeatmapStoredFloat::from_f64(2.5).unwrap()),
                    upper: HeatmapRangeBound::Exact(HeatmapStoredFloat::from_f64(5.5).unwrap()),
                }
            )]
        );
        let defaults = current_heatmap_default_settings();
        assert_eq!(defaults.range.label(), "2.5..5.5");
        assert_eq!(defaults.colormap, HeatmapColormap::Inferno);
        assert_eq!(defaults.normalization, HeatmapNormalization::Sqrt);
        assert!(defaults.invert_x);
        assert!(!defaults.invert_y);
        assert!(defaults.invert_c);
        configure::reset_config(ThemeName::Dark);
    }
}
