use std::sync::mpsc::Sender;

use mlua::{Table, Value};

use crate::{
    configure::{self, SymbolThemeName, ThemeName},
    ui::{app::AppEvent, state::ContentShowMode},
};

use super::errors::ConfigureErrors;
mod bootstrap;
mod heatmap;
mod keymaps;
mod layout;
mod mchart;
mod themes;
use bootstrap::{default_symbol_theme_for_compatibility, execute_config_chunk, prepare_lua_config};
use heatmap::parse_heatmap_config;
pub use keymaps::with_keymap_lua_callback;
use keymaps::{parse_keymaps_config, store_keymap_lua_runtime};
use layout::parse_layout_config;
use mchart::parse_multichart_config;

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

pub fn load_config_compatibility(
    default_compatibility: bool,
) -> Result<Option<bool>, ConfigureErrors> {
    let prepared = prepare_lua_config(None, default_compatibility)?;
    let lua = prepared.lua;
    let h5v = prepared.h5v;
    let chunk_name = prepared.chunk_name;
    let config = prepared.config;
    execute_config_chunk(&lua, &chunk_name, &config)?;
    parse_compatibility_override(&h5v)
}

pub fn run_lua_engine(
    events: Sender<AppEvent>,
    default_compatibility: bool,
) -> Result<(), ConfigureErrors> {
    let prepared = prepare_lua_config(Some(events), default_compatibility)?;
    let lua = prepared.lua;
    let h5v = prepared.h5v;
    let chunk_name = prepared.chunk_name;
    let config = prepared.config;
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

fn apply_lua_config(h5v: &Table) -> Result<(), ConfigureErrors> {
    let compatibility_override = parse_compatibility_override(h5v)?;
    let content_mode_order = parse_content_mode_order(h5v)?;
    let heatmap_config = parse_heatmap_config(h5v)?;
    let layout_config = parse_layout_config(h5v)?;
    let multichart_config = parse_multichart_config(h5v)?;
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
    if let Some(layout_settings) = layout_config {
        configure::set_auto_layout_settings(&layout_settings);
    }
    if let Some((range_modes, default_settings)) = heatmap_config {
        configure::set_heatmap_ranges(&range_modes, &default_settings.range);
        configure::set_heatmap_default_settings(&default_settings);
    }
    if let Some(multichart_settings) = multichart_config {
        configure::set_multichart_settings(&multichart_settings);
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
        apply_lua_config,
        bootstrap::{build_h5v_table, execute_config_chunk},
        heatmap::parse_heatmap_config,
        layout::parse_layout_config,
        parse_compatibility_override, parse_content_mode_order,
        themes::{build_symbol_theme_table, build_theme_table},
    };
    use crate::configure::{
        self, configured_symbol, current_auto_layout_settings, current_content_mode_order,
        current_heatmap_default_settings, current_heatmap_range_modes, current_keymaps,
        themed_color, AutoLayoutSettings, LayoutSize, PanelLayoutSizes, SymbolThemeName, ThemeName,
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
    fn parses_layout_configuration() {
        let lua = Lua::new();
        let h5v = lua.create_table().expect("create h5v table");
        let layout = lua.create_table().expect("create layout table");
        let tree = lua.create_table().expect("create tree table");
        tree.set("focused", "32%").expect("set tree focused");
        tree.set("unfocused", "18%").expect("set tree unfocused");
        layout.set("tree", tree).expect("set tree config");
        let attributes = lua.create_table().expect("create attributes table");
        attributes
            .set("focused", 14)
            .expect("set attributes focused");
        attributes
            .set("unfocused", 6)
            .expect("set attributes unfocused");
        layout
            .set("attributes", attributes)
            .expect("set attributes config");
        let content = lua.create_table().expect("create content table");
        content.set("focused", "*").expect("set content focused");
        content
            .set("unfocused", "*")
            .expect("set content unfocused");
        layout.set("content", content).expect("set content config");
        h5v.set("layout", layout).expect("set layout");

        let parsed = parse_layout_config(&h5v)
            .expect("parse layout config")
            .expect("layout config present");
        assert_eq!(
            parsed,
            AutoLayoutSettings {
                tree: PanelLayoutSizes::new(LayoutSize::percent(32), LayoutSize::percent(18)),
                attributes: PanelLayoutSizes::new(LayoutSize::cells(14), LayoutSize::cells(6)),
                content: PanelLayoutSizes::new(LayoutSize::fill(), LayoutSize::fill()),
            }
        );
    }

    #[test]
    fn applies_layout_configuration() {
        let lua = Lua::new();
        let h5v = build_h5v_table(&lua, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            h5v.layout.tree.focused = "32%"
            h5v.layout.tree.unfocused = "18%"
            h5v.layout.attributes.focused = 14
            h5v.layout.attributes.unfocused = 6
            h5v.layout.content.focused = "*"
            h5v.layout.content.unfocused = "*"
        "#,
        )
        .exec()
        .expect("assign layout config");

        apply_lua_config(&h5v).expect("apply config");
        assert_eq!(
            current_auto_layout_settings(),
            AutoLayoutSettings {
                tree: PanelLayoutSizes::new(LayoutSize::percent(32), LayoutSize::percent(18)),
                attributes: PanelLayoutSizes::new(LayoutSize::cells(14), LayoutSize::cells(6)),
                content: PanelLayoutSizes::new(LayoutSize::fill(), LayoutSize::fill()),
            }
        );
        configure::reset_config(ThemeName::Dark);
    }

    #[test]
    fn applies_max_layout_constraint_configuration() {
        let lua = Lua::new();
        let h5v = build_h5v_table(&lua, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            h5v.layout.attributes.focused = "max(12)"
        "#,
        )
        .exec()
        .expect("assign layout config");

        apply_lua_config(&h5v).expect("apply config");
        assert_eq!(
            current_auto_layout_settings().attributes.focused,
            LayoutSize::max(12)
        );
        configure::reset_config(ThemeName::Dark);
    }

    #[test]
    fn layout_configuration_rejects_invalid_pairing() {
        let lua = Lua::new();
        let h5v = build_h5v_table(&lua, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            h5v.layout.attributes.focused = "61%"
            h5v.layout.content.unfocused = "30%"
        "#,
        )
        .exec()
        .expect("assign invalid layout config");

        let error = apply_lua_config(&h5v).expect_err("invalid layout should error");
        assert!(error.to_string().contains(
            "h5v.layout.attributes.focused (61%) + h5v.layout.content.unfocused (30%) must equal 100% when both sides use percentages"
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
