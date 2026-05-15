use std::sync::mpsc::Sender;

use mlua::{Lua, Table};

use crate::{
    configure::{self, current_content_mode_order, SymbolThemeName, ThemeName},
    ui::{
        app::AppEvent,
        state::{AppToast, ContentShowMode},
    },
};

use super::{
    heatmap::build_heatmap_table,
    keymaps::{build_action_constants_table, build_keymaps_table, build_mode_constants_table},
    layout::build_layout_table,
    mchart::build_multichart_table,
    themes::{build_empty_nested_table, build_symbol_theme_table, build_theme_table},
};
use crate::configure::{errors::ConfigureErrors, loading};

pub(super) struct PreparedLuaConfig {
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
    h5v.set("layout", build_layout_table(lua)?)?;
    h5v.set("multichart", build_multichart_table(lua)?)?;
    h5v.set("modes", build_mode_constants_table(lua)?)?;
    h5v.set("actions", build_action_constants_table(lua)?)?;
    h5v.set("keymaps", build_keymaps_table(lua)?)?;
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
    let lua = Lua::new();
    let h5v = build_h5v_table(&lua, events, default_compatibility)?;
    install_h5v_globals(&lua, &h5v)?;
    let config_path = loading::config_path()?;
    let chunk_name = format!("@{}", config_path.display());
    let config = loading::load_or_create_config()?;
    Ok(PreparedLuaConfig {
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
