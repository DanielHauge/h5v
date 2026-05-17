use mlua::{Lua, Result as LuaResult, Table, Value};

use crate::configure::presentation::{
    available_symbol_theme_names, available_theme_names, theme_named_colors, theme_named_symbols,
    SymbolThemeName, ThemeName,
};
use crate::configure::registry::{
    ColorHandle, RegistryBuilder, RegistryOwner, SymbolHandle, ThemeHandle, ThemeMetadata,
};

const CUSTOM_THEME_DEFINITIONS_KEY: &str = "__definitions";
const CUSTOM_THEME_REGISTER_FN_KEY: &str = "register";
const CUSTOM_THEME_OWNER_KEY: &str = "__owner";

pub(crate) fn register_lua_themes(builder: &mut RegistryBuilder, h5v: &Table) -> LuaResult<()> {
    let Some(definitions) = theme_definitions(h5v) else {
        return Ok(());
    };
    for pair in definitions.pairs::<String, Table>() {
        let (handle, definition) = pair?;
        if !super::plugins::definition_owner_is_enabled(h5v, &definition)? {
            continue;
        }
        builder
            .register_theme(parse_theme_definition(
                h5v,
                &ThemeHandle::new(handle),
                theme_owner(&definition),
                &definition,
            )?)
            .map_err(mlua::Error::external)?;
    }
    Ok(())
}

pub(crate) fn parse_selected_theme(h5v: &Table) -> LuaResult<ThemeHandle> {
    let theme_value = h5v.get::<Value>("theme")?;
    let theme_value = match theme_value {
        Value::String(value) => value.to_str()?.to_string(),
        Value::Nil => ThemeName::Dark.as_str().to_string(),
        _ => {
            return Err(mlua::Error::external(
                "h5v.theme must be a theme handle or built-in theme name",
            ));
        }
    };
    resolve_theme_handle(h5v, &theme_value).ok_or_else(|| {
        let mut available = available_theme_names()
            .into_iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>();
        if let Some(definitions) = theme_definitions(h5v) {
            available.extend(
                definitions
                    .pairs::<String, Table>()
                    .filter_map(|pair| pair.ok().map(|(handle, _)| handle)),
            );
        }
        mlua::Error::external(format!(
            "Unknown theme '{theme_value}'. Available themes: {}",
            available.join(", ")
        ))
    })
}

pub(crate) fn activate_theme(
    builder: &mut RegistryBuilder,
    h5v: &Table,
    selected: &ThemeHandle,
) -> LuaResult<()> {
    for name in available_theme_names() {
        let handle = ThemeHandle::new(format!("builtin.theme.{name}"));
        builder
            .update_theme(&handle, |metadata| {
                metadata.is_active = metadata.handle == *selected;
            })
            .map_err(mlua::Error::external)?;
    }
    if let Some(definitions) = theme_definitions(h5v) {
        for pair in definitions.pairs::<String, Table>() {
            let (handle, _) = pair?;
            let handle = ThemeHandle::new(handle);
            builder
                .update_theme(&handle, |metadata| {
                    metadata.is_active = metadata.handle == *selected;
                })
                .map_err(mlua::Error::external)?;
        }
    }
    Ok(())
}

pub(crate) fn build_theme_table(lua: &Lua, h5v: &Table) -> LuaResult<Table> {
    let themes = lua.create_table()?;
    let definitions = lua.create_table()?;
    themes.raw_set(CUSTOM_THEME_DEFINITIONS_KEY, definitions.clone())?;

    let h5v = h5v.clone();
    let register_definitions = definitions.clone();
    themes.set(
        CUSTOM_THEME_REGISTER_FN_KEY,
        lua.create_function(move |lua, definition: Table| {
            let handle = theme_definition_handle(&definition)?;
            match current_theme_owner(&h5v)? {
                RegistryOwner::Plugin(plugin) => {
                    definition.raw_set(CUSTOM_THEME_OWNER_KEY, plugin.as_str())?
                }
                RegistryOwner::Builtin | RegistryOwner::Config => {
                    definition.raw_set(CUSTOM_THEME_OWNER_KEY, Value::Nil)?
                }
            }
            register_definitions.raw_set(handle.as_str(), definition.clone())?;
            Ok(lua.create_string(handle.as_str())?)
        })?,
    )?;

    for name in available_theme_names() {
        themes.set(*name, build_builtin_theme_entry(lua, name)?)?;
    }

    Ok(themes)
}

fn theme_definitions(h5v: &Table) -> Option<Table> {
    let colors: Table = h5v.get("colors").ok()?;
    let themes: Table = colors.get("themes").ok()?;
    themes.get(CUSTOM_THEME_DEFINITIONS_KEY).ok()
}

fn build_builtin_theme_entry(lua: &Lua, name: &str) -> LuaResult<Table> {
    let table = lua.create_table()?;
    let theme = ThemeName::parse(name)
        .ok_or_else(|| mlua::Error::external(format!("Unknown built-in theme '{name}'")))?;
    let colors = lua.create_table()?;
    for (name, value) in theme_named_colors(theme) {
        let value = Value::String(lua.create_string(crate::configure::color_to_lua_string(value))?);
        insert_string_value(lua, &colors, name, value.clone())?;
        insert_string_value(lua, &table, name, value)?;
    }
    table.set("colors", colors)?;
    Ok(table)
}

fn theme_definition_handle(definition: &Table) -> LuaResult<ThemeHandle> {
    let id: String = definition
        .get("id")
        .map_err(|_| mlua::Error::external("theme definition requires id"))?;
    let handle = ThemeHandle::new(id);
    if handle.as_str().trim().is_empty() {
        return Err(mlua::Error::external("theme id cannot be empty"));
    }
    Ok(handle)
}

fn current_theme_owner(h5v: &Table) -> LuaResult<RegistryOwner> {
    let owner = match h5v.get::<Value>("__registry_owner")? {
        Value::String(value) => Some(value.to_str()?.to_string()),
        Value::Nil => None,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.__registry_owner must be a string, got {}",
                other.type_name()
            )))
        }
    };
    Ok(match owner.as_deref() {
        Some(value) if value.starts_with("plugin.") => {
            RegistryOwner::Plugin(crate::configure::registry::PluginHandle::new(value))
        }
        _ => RegistryOwner::Config,
    })
}

fn theme_owner(definition: &Table) -> RegistryOwner {
    definition
        .get::<Option<String>>(CUSTOM_THEME_OWNER_KEY)
        .ok()
        .flatten()
        .map(|id| RegistryOwner::Plugin(crate::configure::registry::PluginHandle::new(id)))
        .unwrap_or(RegistryOwner::Config)
}

fn parse_theme_definition(
    h5v: &Table,
    handle: &ThemeHandle,
    owner: RegistryOwner,
    definition: &Table,
) -> LuaResult<ThemeMetadata> {
    let title = definition
        .get::<Option<String>>("title")?
        .unwrap_or_else(|| {
            handle
                .as_str()
                .rsplit('.')
                .next()
                .unwrap_or("theme")
                .to_string()
        });
    let summary = definition
        .get::<Option<String>>("summary")?
        .unwrap_or_default();
    let variant = definition
        .get::<Option<String>>("variant")?
        .or_else(|| infer_theme_variant(h5v, handle));
    let color_overrides = parse_theme_color_overrides(h5v, definition)?;
    let symbol_overrides = parse_theme_symbol_overrides(h5v, definition)?;
    Ok(ThemeMetadata {
        handle: handle.clone(),
        title,
        summary,
        variant,
        color_overrides,
        symbol_overrides,
        is_active: false,
        owner,
    })
}

pub(crate) fn build_empty_nested_table<I>(lua: &Lua, entries: I) -> LuaResult<Table>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let root = lua.create_table()?;
    for entry in entries {
        insert_string_value(lua, &root, entry.as_ref(), Value::Nil)?;
    }
    Ok(root)
}

pub(crate) fn build_symbol_theme_table(lua: &Lua) -> LuaResult<Table> {
    let table = lua.create_table()?;
    for name in available_symbol_theme_names() {
        let theme = SymbolThemeName::parse(name)
            .ok_or_else(|| mlua::Error::external(format!("Unknown symbol theme '{name}'")))?;
        let entry = lua.create_table()?;
        for (symbol_name, value) in theme_named_symbols(theme) {
            insert_string_value(
                lua,
                &entry,
                symbol_name,
                Value::String(lua.create_string(value)?),
            )?;
        }
        table.set(*name, entry)?;
    }
    Ok(table)
}

fn infer_theme_variant(h5v: &Table, handle: &ThemeHandle) -> Option<String> {
    if handle.as_str().starts_with("builtin.theme.") {
        return handle
            .as_str()
            .rsplit('.')
            .next()
            .map(str::to_string)
            .filter(|value| ThemeName::parse(value).is_some());
    }
    resolve_theme_handle(h5v, handle.as_str()).and_then(|resolved| {
        if resolved.as_str().starts_with("builtin.theme.") {
            resolved.as_str().rsplit('.').next().map(str::to_string)
        } else {
            None
        }
    })
}

fn parse_theme_color_overrides(
    h5v: &Table,
    definition: &Table,
) -> LuaResult<Vec<(ColorHandle, String)>> {
    let Some(colors) = definition.get::<Option<Table>>("colors")? else {
        return Ok(Vec::new());
    };
    let mut overrides = Vec::new();
    for pair in colors.pairs::<String, String>() {
        let (key, value) = pair?;
        let handle = resolve_color_handle(h5v, &key)
            .ok_or_else(|| mlua::Error::external(format!("Unknown theme color '{key}'")))?;
        overrides.push((handle, value));
    }
    Ok(overrides)
}

fn parse_theme_symbol_overrides(
    h5v: &Table,
    definition: &Table,
) -> LuaResult<Vec<(SymbolHandle, String)>> {
    let Some(symbols) = definition.get::<Option<Table>>("symbols")? else {
        return Ok(Vec::new());
    };
    let mut overrides = Vec::new();
    for pair in symbols.pairs::<String, String>() {
        let (key, value) = pair?;
        let handle = resolve_symbol_handle(h5v, &key)
            .ok_or_else(|| mlua::Error::external(format!("Unknown theme symbol '{key}'")))?;
        overrides.push((handle, value));
    }
    Ok(overrides)
}

fn resolve_theme_handle(h5v: &Table, value: &str) -> Option<ThemeHandle> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("builtin.theme.")
        || trimmed.starts_with("config.theme.")
        || trimmed.starts_with("plugin.")
    {
        return Some(ThemeHandle::new(trimmed));
    }
    if ThemeName::parse(trimmed).is_some() {
        return Some(ThemeHandle::new(format!("builtin.theme.{trimmed}")));
    }
    let definitions = theme_definitions(h5v)?;
    if definitions.contains_key(trimmed).ok()? {
        return Some(ThemeHandle::new(trimmed));
    }
    None
}

fn resolve_color_handle(h5v: &Table, value: &str) -> Option<ColorHandle> {
    resolve_handle_from_id_table(h5v, "colors", value).map(ColorHandle::new)
}

fn resolve_symbol_handle(h5v: &Table, value: &str) -> Option<SymbolHandle> {
    resolve_handle_from_id_table(h5v, "symbols", value).map(SymbolHandle::new)
}

fn resolve_handle_from_id_table(h5v: &Table, group: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("builtin.")
        || trimmed.starts_with("config.")
        || trimmed.starts_with("plugin.")
    {
        return Some(trimmed.to_string());
    }
    let ids: Table = h5v.get("ids").ok()?;
    let entries: Table = ids.get(group).ok()?;
    if entries.contains_key(trimmed).ok()? {
        return entries.get(trimmed).ok();
    }
    Some(format!("builtin.{group}.{trimmed}"))
}

fn insert_string_value(lua: &Lua, table: &Table, dotted_name: &str, value: Value) -> LuaResult<()> {
    let mut current = table.clone();
    let mut parts = dotted_name.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            current.set(part, value.clone())?;
            return Ok(());
        }
        let next = match current.get::<Value>(part)? {
            Value::Table(table) => table,
            Value::Nil => {
                let created = lua.create_table()?;
                current.set(part, created.clone())?;
                created
            }
            other => {
                return Err(mlua::Error::external(format!(
                    "Cannot nest key '{dotted_name}' through {}",
                    other.type_name()
                )));
            }
        };
        current = next;
    }
    Ok(())
}
