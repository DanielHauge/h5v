use std::sync::mpsc::Sender;

use mlua::{Lua, Table};
use tracing::Level;

use crate::{
    configure::{
        builtin_registry_builder, current_content_mode_order, RegistryBuilder, RegistrySnapshot,
        SymbolThemeName, ThemeName,
    },
    ui::{
        app::AppEvent,
        state::{AppToast, ContentShowMode},
        toast::send_app_toast,
    },
};

use super::{
    commands::build_commands_table,
    events::build_events_table,
    heatmap::build_heatmap_table,
    keymaps::{build_action_constants_table, build_keymaps_table, build_mode_constants_table},
    layout::build_layout_table,
    mchart::build_multichart_table,
    plugins::build_plugins_table,
    themes::{build_empty_nested_table, build_symbol_theme_table, build_theme_table},
    ui::build_ui_table,
};
use crate::configure::{errors::ConfigureErrors, loading};

pub(super) struct PreparedLuaConfig {
    pub registry_builder: RegistryBuilder,
    pub lua: Lua,
    pub h5v: Table,
    pub chunk_name: String,
    pub config: String,
}

pub(super) fn default_symbol_theme_for_compatibility(compatibility: bool) -> SymbolThemeName {
    if compatibility {
        SymbolThemeName::Compatibility
    } else {
        SymbolThemeName::Rich
    }
}

pub(super) fn build_h5v_table(
    lua: &Lua,
    registry: &RegistrySnapshot,
    events: Option<Sender<AppEvent>>,
    default_compatibility: bool,
) -> Result<Table, ConfigureErrors> {
    let h5v = lua.create_table()?;
    let modes = build_mode_constants_table(lua)?;
    let actions = build_action_constants_table(lua)?;
    let keymaps = build_keymaps_table(lua)?;
    let ids = lua.create_table()?;
    let keymap_modes = lua.create_table()?;
    for (_, code) in crate::ui::input::keymap::exported_mode_codes() {
        keymap_modes.set(*code, *code)?;
    }
    ids.set("keymap_modes", keymap_modes)?;
    ids.set("commands", build_command_ids_table(lua, registry)?)?;
    ids.set("events", build_event_ids_table(lua, registry)?)?;
    ids.set("settings", build_setting_ids_table(lua, registry)?)?;
    ids.set("themes", build_theme_ids_table(lua, registry)?)?;
    ids.set("components", build_component_ids_table(lua)?)?;
    ids.set("health", build_health_ids_table(lua)?)?;
    ids.set(
        "symbol_themes",
        build_symbol_theme_ids_table(lua, registry)?,
    )?;
    ids.set(
        "content_modes",
        build_content_mode_ids_table(lua, registry)?,
    )?;
    ids.set("colors", build_color_ids_table(lua, registry)?)?;
    ids.set("symbols", build_symbol_ids_table(lua, registry)?)?;
    ids.set("value_kinds", build_value_kind_ids_table(lua)?)?;
    let color_names = registry
        .colors()
        .map(registry_color_name)
        .collect::<Vec<_>>();
    let symbol_names = registry
        .symbols()
        .map(registry_symbol_name)
        .collect::<Vec<_>>();

    let logs = lua.create_table()?;
    logs.set(
        "info",
        lua.create_function(|_, message: String| {
            crate::logging::log_lua(Level::INFO, &message);
            Ok(())
        })?,
    )?;
    logs.set(
        "warning",
        lua.create_function(|_, message: String| {
            crate::logging::log_lua(Level::WARN, &message);
            Ok(())
        })?,
    )?;
    logs.set(
        "warn",
        lua.create_function(|_, message: String| {
            crate::logging::log_lua(Level::WARN, &message);
            Ok(())
        })?,
    )?;
    logs.set(
        "error",
        lua.create_function(|_, message: String| {
            crate::logging::log_lua(Level::ERROR, &message);
            Ok(())
        })?,
    )?;
    let log_fn: mlua::Function = logs.get("info")?;
    h5v.set("log", log_fn)?;
    h5v.set("logs", logs)?;

    let toast = lua.create_table()?;
    match events {
        Some(events) => {
            let info_events = events.clone();
            toast.set(
                "info",
                lua.create_function(move |_, message: String| {
                    send_app_toast(&info_events, AppToast::Info(message));
                    Ok(())
                })?,
            )?;
            let warn_events = events.clone();
            toast.set(
                "warning",
                lua.create_function(move |_, message: String| {
                    send_app_toast(&warn_events, AppToast::Warning(message));
                    Ok(())
                })?,
            )?;
            let warn_alias_events = events.clone();
            toast.set(
                "warn",
                lua.create_function(move |_, message: String| {
                    send_app_toast(&warn_alias_events, AppToast::Warning(message));
                    Ok(())
                })?,
            )?;
            toast.set(
                "error",
                lua.create_function(move |_, message: String| {
                    send_app_toast(&events, AppToast::Error(message));
                    Ok(())
                })?,
            )?;
        }
        None => {
            let noop = lua.create_function(|_, _: String| Ok(()))?;
            toast.set("info", noop.clone())?;
            toast.set("warning", noop.clone())?;
            toast.set("warn", noop.clone())?;
            toast.set("error", noop)?;
        }
    }
    h5v.set("toast", toast)?;
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
    let colors = build_empty_nested_table(lua, color_names.iter().map(String::as_str))?;
    let themes = build_theme_table(lua, &h5v)?;
    colors.set("themes", themes.clone())?;
    h5v.set("colors", colors)?;
    h5v.set(
        "symbols",
        build_empty_nested_table(lua, symbol_names.iter().map(String::as_str))?,
    )?;
    h5v.set("themes", themes)?;
    h5v.set("symbol_themes", build_symbol_theme_table(lua)?)?;
    h5v.set("heatmap", build_heatmap_table(lua)?)?;
    h5v.set("layout", build_layout_table(lua)?)?;
    let multichart = build_multichart_table(lua)?;
    h5v.set("multichart", multichart.clone())?;
    h5v.set("mchart", multichart)?;
    h5v.set("modes", modes)?;
    h5v.set("actions", actions)?;
    h5v.set("ids", ids)?;
    h5v.set("commands", build_commands_table(lua)?)?;
    h5v.set("events", build_events_table(lua)?)?;
    h5v.set("plugins", build_plugins_table(lua)?)?;
    h5v.set("ui", build_ui_table(lua)?)?;
    h5v.set("keymaps", keymaps.clone())?;
    h5v.set("keys", keymaps)?;
    h5v.set("__plugin_store", lua.create_table()?)?;
    h5v.set("__registry_owner", "config")?;
    Ok(h5v)
}

pub(super) fn execute_config_chunk(
    lua: &Lua,
    chunk_name: &str,
    config: &str,
) -> Result<(), ConfigureErrors> {
    lua.load(config).set_name(chunk_name).exec()?;
    Ok(())
}

pub(super) fn prepare_lua_config(
    events: Option<Sender<AppEvent>>,
    default_compatibility: bool,
) -> Result<PreparedLuaConfig, ConfigureErrors> {
    let registry_builder =
        builtin_registry_builder().map_err(|error| mlua::Error::runtime(error.to_string()))?;
    let registry = crate::configure::builtin_registry_snapshot()
        .map_err(|error| mlua::Error::runtime(error.to_string()))?;
    let lua = Lua::new();
    let h5v = build_h5v_table(&lua, &registry, events, default_compatibility)?;
    install_h5v_globals(&lua, &h5v)?;
    let config_path = loading::config_path()?;
    let chunk_name = format!("@{}", config_path.display());
    let config = loading::load_or_create_config()?;
    Ok(PreparedLuaConfig {
        registry_builder,
        lua,
        h5v,
        chunk_name,
        config,
    })
}

fn install_h5v_globals(lua: &Lua, h5v: &Table) -> Result<(), ConfigureErrors> {
    lua.globals().set("h5v", h5v.clone())?;
    lua.globals().set("h5v_modes", h5v.get::<Table>("modes")?)?;
    lua.globals()
        .set("h5v_actions", h5v.get::<Table>("actions")?)?;
    Ok(())
}

fn registry_color_name(metadata: &crate::configure::registry::ColorMetadata) -> String {
    registry_catalog_name(&metadata.group, &metadata.name)
}

fn registry_symbol_name(metadata: &crate::configure::registry::SymbolMetadata) -> String {
    registry_catalog_name(&metadata.group, &metadata.name)
}

fn registry_catalog_name(group: &str, name: &str) -> String {
    if group.is_empty() {
        name.to_string()
    } else {
        format!("{group}.{name}")
    }
}

fn build_command_ids_table(
    lua: &Lua,
    registry: &RegistrySnapshot,
) -> Result<Table, ConfigureErrors> {
    let commands = lua.create_table()?;
    for metadata in registry.commands() {
        let symbol = crate::ui::command::command_lua_id_symbol(&metadata.name);
        if commands.contains_key(symbol.as_str())? {
            return Err(mlua::Error::runtime(format!(
                "Duplicate Lua command id symbol '{}' for builtin commands",
                symbol
            ))
            .into());
        }
        commands.set(symbol, metadata.handle.as_str())?;
    }
    Ok(commands)
}

fn build_setting_ids_table(
    lua: &Lua,
    registry: &RegistrySnapshot,
) -> Result<Table, ConfigureErrors> {
    let settings = lua.create_table()?;
    for metadata in registry.settings() {
        let Some(suffix) = metadata.handle.as_str().strip_prefix("builtin.setting.") else {
            continue;
        };
        insert_nested_id(
            lua,
            &settings,
            &split_id_segments(suffix),
            metadata.handle.as_str(),
        )?;
    }
    Ok(settings)
}

fn build_event_ids_table(lua: &Lua, registry: &RegistrySnapshot) -> Result<Table, ConfigureErrors> {
    let events = lua.create_table()?;
    for metadata in registry.events() {
        let Some(suffix) = metadata.handle.as_str().strip_prefix("builtin.event.") else {
            continue;
        };
        events.set(lua_id_symbol(suffix), metadata.handle.as_str())?;
    }
    Ok(events)
}

fn build_theme_ids_table(lua: &Lua, registry: &RegistrySnapshot) -> Result<Table, ConfigureErrors> {
    let themes = lua.create_table()?;
    for metadata in registry.themes() {
        let Some(suffix) = metadata.handle.as_str().strip_prefix("builtin.theme.") else {
            continue;
        };
        themes.set(lua_id_symbol(suffix), metadata.handle.as_str())?;
    }
    Ok(themes)
}

fn build_component_ids_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let components = lua.create_table()?;
    for (name, id) in [
        ("tree", "tree"),
        ("attributes", "attributes"),
        ("content", "content"),
        ("help", "help"),
        ("heatmap", "heatmap"),
        ("mchart", "mchart"),
        ("preview", "preview"),
        ("matrix", "matrix"),
        ("command", "command"),
        ("status", "status"),
    ] {
        components.set(name, id)?;
    }
    Ok(components)
}

fn build_health_ids_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let health = lua.create_table()?;
    health.set("healthy", "healthy")?;
    health.set("warning", "warning")?;
    health.set("fail", "fail")?;
    Ok(health)
}

fn build_symbol_theme_ids_table(
    lua: &Lua,
    _registry: &RegistrySnapshot,
) -> Result<Table, ConfigureErrors> {
    let symbol_themes = lua.create_table()?;
    for theme_name in crate::configure::available_symbol_theme_names() {
        symbol_themes.set(
            lua_id_symbol(theme_name),
            format!("builtin.symbol_theme.{theme_name}"),
        )?;
    }
    Ok(symbol_themes)
}

fn build_content_mode_ids_table(
    lua: &Lua,
    registry: &RegistrySnapshot,
) -> Result<Table, ConfigureErrors> {
    let content_modes = lua.create_table()?;
    for metadata in registry.content_modes() {
        let Some(suffix) = metadata
            .handle
            .as_str()
            .strip_prefix("builtin.content_mode.")
        else {
            continue;
        };
        content_modes.set(lua_id_symbol(suffix), metadata.handle.as_str())?;
    }
    Ok(content_modes)
}

fn build_color_ids_table(lua: &Lua, registry: &RegistrySnapshot) -> Result<Table, ConfigureErrors> {
    let colors = lua.create_table()?;
    for metadata in registry.colors() {
        let name = registry_catalog_name(&metadata.group, &metadata.name);
        let Some(suffix) = metadata.handle.as_str().strip_prefix("builtin.color.") else {
            continue;
        };
        debug_assert_eq!(suffix, name);
        insert_nested_id(
            lua,
            &colors,
            &split_id_segments(&name),
            metadata.handle.as_str(),
        )?;
    }
    Ok(colors)
}

fn build_symbol_ids_table(
    lua: &Lua,
    registry: &RegistrySnapshot,
) -> Result<Table, ConfigureErrors> {
    let symbols = lua.create_table()?;
    for metadata in registry.symbols() {
        let name = registry_catalog_name(&metadata.group, &metadata.name);
        let Some(suffix) = metadata.handle.as_str().strip_prefix("builtin.symbol.") else {
            continue;
        };
        debug_assert_eq!(suffix, name);
        insert_nested_id(
            lua,
            &symbols,
            &split_id_segments(&name),
            metadata.handle.as_str(),
        )?;
    }
    Ok(symbols)
}

fn build_value_kind_ids_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let value_kinds = lua.create_table()?;
    value_kinds.set("scalar", "scalar")?;
    value_kinds.set("series", "series")?;
    value_kinds.set("unknown", "unknown")?;
    Ok(value_kinds)
}

fn insert_nested_id(
    lua: &Lua,
    root: &Table,
    segments: &[String],
    value: &str,
) -> Result<(), ConfigureErrors> {
    let Some((last, parents)) = segments.split_last() else {
        return Ok(());
    };
    let mut current = root.clone();
    for segment in parents {
        let key = lua_id_symbol(segment);
        match current.get::<mlua::Value>(key.as_str())? {
            mlua::Value::Nil => {
                let created = lua.create_table()?;
                current.set(key.as_str(), created.clone())?;
                current = created;
            }
            mlua::Value::Table(existing) => current = existing,
            other => {
                return Err(mlua::Error::runtime(format!(
                    "Cannot create nested id namespace '{}': found {}",
                    key,
                    other.type_name()
                ))
                .into())
            }
        }
    }
    current.set(lua_id_symbol(last), value)?;
    Ok(())
}

fn split_id_segments(value: &str) -> Vec<String> {
    value
        .split('.')
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn lua_id_symbol(value: &str) -> String {
    value.replace('-', "_")
}
