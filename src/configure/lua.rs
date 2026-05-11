use std::sync::mpsc::Sender;

use mlua::{Lua, Table, Value};

use crate::{
    configure::{self, SymbolThemeName, ThemeName},
    ui::state::ContentShowMode,
    ui::{app::AppEvent, state::AppToast},
};

use super::{errors::ConfigureErrors, loading};

fn default_symbol_theme_for_compatibility(compatibility: bool) -> SymbolThemeName {
    if compatibility {
        SymbolThemeName::Compatibility
    } else {
        SymbolThemeName::Rich
    }
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
            configure::current_content_mode_order()
                .into_iter()
                .map(ContentShowMode::as_str),
        )?,
    )?;
    h5v.set(
        "symbol_theme",
        default_symbol_theme_for_compatibility(default_compatibility).as_str(),
    )?;
    h5v.set("colors", lua.create_table()?)?;
    h5v.set("symbols", lua.create_table()?)?;
    h5v.set("themes", build_theme_table(lua)?)?;
    h5v.set("symbol_themes", build_symbol_theme_table(lua)?)?;
    Ok(h5v)
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
                        "Unknown content mode '{value}'. Available modes: preview, matrix"
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

pub fn load_config_compatibility(
    default_compatibility: bool,
) -> Result<Option<bool>, ConfigureErrors> {
    let lua = Lua::new();
    let h5v = build_h5v_table(&lua, None, default_compatibility)?;
    lua.globals().set("h5v", h5v.clone())?;
    let config = loading::load_or_create_config()?;
    lua.load(&config).exec()?;
    parse_compatibility_override(&h5v)
}

pub fn run_lua_engine(
    events: Sender<AppEvent>,
    default_compatibility: bool,
) -> Result<(), ConfigureErrors> {
    let lua = Lua::new();
    let h5v = build_h5v_table(&lua, Some(events), default_compatibility)?;
    lua.globals().set("h5v", h5v.clone())?;
    let config = loading::load_or_create_config()?;
    let previous_config = configure::snapshot_config();

    configure::reset_config(ThemeName::Dark);
    let result = (|| -> Result<(), ConfigureErrors> {
        lua.load(&config).exec()?;
        apply_lua_config(&h5v)?;
        Ok(())
    })();
    if result.is_err() {
        configure::restore_config(previous_config);
    }
    result
}

fn build_theme_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let themes = lua.create_table()?;
    for theme_name in [ThemeName::Dark, ThemeName::Light, ThemeName::LightBlue] {
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
        apply_lua_config, build_symbol_theme_table, build_theme_table,
        parse_compatibility_override, parse_content_mode_order,
    };
    use crate::configure::{
        self, configured_symbol, current_content_mode_order, themed_color, SymbolThemeName,
        ThemeName,
    };
    use crate::ui::state::ContentShowMode;
    use mlua::{Lua, Table, Value};
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
            vec![ContentShowMode::Matrix, ContentShowMode::Preview]
        );

        configure::reset_config(ThemeName::Dark);
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
}
