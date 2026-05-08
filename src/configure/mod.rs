use std::{path::PathBuf, sync::mpsc::Sender};

use mlua::{Lua, Table, Value};

use crate::{
    color_consts::{self, ThemeName},
    configure::errors::ConfigureErrors,
    ui::{app::AppEvent, state::AppToast},
};
pub mod errors;
mod loading;

pub fn ensure_config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::ensure_config_exists()
}

pub fn reset_config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::reset_config_to_default()
}

pub fn run_lua_engine(events: Sender<AppEvent>) -> Result<(), ConfigureErrors> {
    let lua = Lua::new();

    let h5v = lua.create_table()?;

    let events_clone = events.clone();
    let log_fn = lua.create_function(move |_, msg: String| {
        let _ = events_clone
            .to_owned()
            .send(AppEvent::Toast(AppToast::Info(msg)));
        Ok(())
    })?;

    h5v.set("log", log_fn)?;
    h5v.set("theme", ThemeName::Dark.as_str())?;
    h5v.set("colors", lua.create_table()?)?;
    h5v.set("themes", build_theme_table(&lua)?)?;

    lua.globals().set("h5v", h5v.clone())?;

    let config = loading::load_or_create_config()?;
    let previous_theme = color_consts::snapshot_theme();

    color_consts::reset_theme(ThemeName::Dark);
    let result = (|| -> Result<(), ConfigureErrors> {
        lua.load(&config).exec()?;
        apply_theme_config(&h5v)?;
        Ok(())
    })();
    if result.is_err() {
        color_consts::restore_theme(previous_theme);
    }
    result
}

fn build_theme_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let themes = lua.create_table()?;
    for theme_name in [ThemeName::Dark, ThemeName::Light] {
        let theme_table = lua.create_table()?;
        for (name, color) in color_consts::theme_named_colors(theme_name) {
            insert_color_value(
                lua,
                &theme_table,
                name,
                color_consts::color_to_lua_string(color),
            )?;
        }
        themes.set(theme_name.as_str(), theme_table)?;
    }
    Ok(themes)
}

fn apply_theme_config(h5v: &Table) -> Result<(), ConfigureErrors> {
    let selected_theme = match h5v.get::<Value>("theme")? {
        Value::Nil => ThemeName::Dark,
        Value::String(value) => {
            let value = value.to_str()?;
            ThemeName::parse(value.as_ref()).ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "Unknown theme '{value}'. Available themes: {}",
                    color_consts::available_theme_names().join(", ")
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
    color_consts::reset_theme(selected_theme);

    match h5v.get::<Value>("colors")? {
        Value::Nil => Ok(()),
        Value::Table(table) => apply_color_overrides(&table, None),
        other => Err(mlua::Error::runtime(format!(
            "h5v.colors must be a table, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn insert_color_value(
    lua: &Lua,
    root: &Table,
    dotted_name: &str,
    value: String,
) -> Result<(), ConfigureErrors> {
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
                let color = color_consts::parse_color(value.as_ref()).ok_or_else(|| {
                    mlua::Error::runtime(format!(
                        "Invalid color '{value}' for '{full_name}'. Use #RRGGBB or a named color."
                    ))
                })?;
                color_consts::set_color_override(&full_name, color)
                    .map_err(mlua::Error::runtime)?;
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

#[cfg(test)]
mod tests {
    use super::{apply_theme_config, build_theme_table};
    use crate::color_consts::{self, title_bg_color, title_color, ThemeName};
    use mlua::{Lua, Table};
    use ratatui::style::Color;

    #[test]
    fn applies_nested_lua_color_overrides() {
        let lua = Lua::new();
        let h5v = lua.create_table().expect("create h5v table");
        h5v.set("theme", ThemeName::Light.as_str())
            .expect("set theme");

        let colors = lua.create_table().expect("create colors table");
        let text = lua.create_table().expect("create text table");
        text.set("title", "#010203").expect("set text.title");
        colors.set("text", text).expect("set text table");
        colors
            .set("title_bg", "#040506")
            .expect("set legacy title_bg");
        h5v.set("colors", colors).expect("set colors");

        apply_theme_config(&h5v).expect("apply config");

        assert_eq!(title_color(), Color::Rgb(1, 2, 3));
        assert_eq!(title_bg_color(), Color::Rgb(4, 5, 6));

        color_consts::reset_theme(ThemeName::Dark);
    }

    #[test]
    fn exports_nested_theme_tables() {
        let lua = Lua::new();
        let themes = build_theme_table(&lua).expect("build themes");
        let dark: Table = themes.get("dark").expect("get dark theme");
        let text: Table = dark.get("text").expect("get dark text table");
        let surface: Table = dark.get("surface").expect("get dark surface table");

        assert_eq!(
            text.get::<String>("title").expect("get text.title"),
            color_consts::color_to_lua_string(Color::Yellow)
        );
        assert_eq!(
            surface
                .get::<String>("panel_border")
                .expect("get surface.panel_border"),
            color_consts::color_to_lua_string(
                color_consts::theme_named_colors(ThemeName::Dark)
                    .into_iter()
                    .find(|(name, _)| *name == "surface.panel_border")
                    .expect("surface.panel_border exists")
                    .1
            )
        );
    }
}
