use mlua::{Lua, Table, Value};

use crate::configure::{self, errors::ConfigureErrors, SymbolThemeName, ThemeName};

pub(super) fn build_empty_nested_table<'a>(
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

pub(super) fn build_theme_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
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

pub(super) fn build_symbol_theme_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
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
