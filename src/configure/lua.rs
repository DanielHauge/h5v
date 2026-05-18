mod bootstrap;
mod commands;
mod context;
mod events;
mod heatmap;
mod keymaps;
mod layout;
mod loader;
mod mchart;
mod plugins;
mod registration;
mod themes;
mod ui;
use std::sync::{LazyLock, RwLock};

pub use commands::with_command_lua_callback;
pub(crate) use context::{
    build_app_context, build_config_context, build_fs_context, build_log_context,
    build_log_context_with_handle, build_plugin_context, build_plugin_fs_context,
    build_process_context, build_selection_context, open_content_mode_target,
    parse_process_json_output, run_process_spec, set_lua_toast, LuaToastLevel,
};
pub(crate) use events::dispatch_lua_event;
pub use keymaps::with_keymap_lua_callback;
pub use loader::{load_config_compatibility, run_lua_engine};
#[cfg(test)]
pub(crate) use mchart::reset_mchart_worker_runtime;
pub(crate) use mchart::{run_registered_mchart_function, LuaMchartArgValue, LuaMchartReturnValue};
#[cfg(test)]
use registration::{
    apply_lua_config, apply_non_registry_lua_config, parse_compatibility_override,
    parse_content_mode_order, register_lua_config,
};
pub(crate) use ui::available_content_mode_handles as available_lua_content_mode_handles;
pub use ui::with_content_mode_lua_callback;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigLoadMetrics {
    pub total_duration_ms: u64,
}

static CONFIG_LOAD_METRICS: LazyLock<RwLock<Option<ConfigLoadMetrics>>> =
    LazyLock::new(|| RwLock::new(None));

pub fn last_config_load_metrics() -> Option<ConfigLoadMetrics> {
    match CONFIG_LOAD_METRICS.read() {
        Ok(guard) => *guard,
        Err(error) => *error.into_inner(),
    }
}

fn set_last_config_load_metrics(metrics: ConfigLoadMetrics) {
    match CONFIG_LOAD_METRICS.write() {
        Ok(mut guard) => *guard = Some(metrics),
        Err(error) => *error.into_inner() = Some(metrics),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use std::sync::MutexGuard;

    use super::{
        apply_lua_config,
        bootstrap::{build_h5v_table, execute_config_chunk},
        heatmap::parse_heatmap_config,
        layout::parse_layout_config,
        parse_compatibility_override, parse_content_mode_order, register_lua_config,
        themes::{build_symbol_theme_table, build_theme_table},
    };
    use crate::configure::registry::{
        ColorHandle, CommandArgValueKind, CommandHandle, ContentModeHandle, MchartFunctionHandle,
        SettingHandle, SymbolHandle, ThemeHandle,
    };
    use crate::configure::{
        self, configured_symbol, current_auto_layout_settings, current_content_mode_order,
        current_content_mode_order_handles, current_heatmap_default_settings,
        current_heatmap_range_modes, current_keymaps, themed_color, AutoLayoutSettings, LayoutSize,
        PanelLayoutSizes, SymbolThemeName, ThemeName,
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

    fn test_guard() -> MutexGuard<'static, ()> {
        crate::test_support::serial_test_guard()
    }

    #[test]
    fn applies_nested_lua_config_overrides() {
        let _guard = test_guard();
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
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            h5v.keys.bind({
              mode = h5v.ids.keymap_modes.global,
              key = "ctrl+h",
              target = h5v.actions.ShowHelp,
              description = "Show help",
            })
            h5v.keys.bind({
              mode = h5v.ids.keymap_modes.global,
              key = "ctrl+k",
              commands = { "down 2", "up 1" },
              description = "Run commands",
            })
            h5v.keys.unbind({
              mode = h5v.ids.keymap_modes.heatmap,
              key = "v",
            })
            h5v.keys.bind({
              mode = h5v.ids.keymap_modes.heatmap,
              key = "ctrl+z",
              target = h5v.actions.HeatmapZoomIn,
            })
            h5v.keys.bind({
              mode = h5v.ids.keymap_modes.heatmap,
              key = "ctrl+l",
              lua = function(ctx)
                ctx.command("help reload")
              end,
              description = "Run lua",
            })
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
    fn bind_accepts_registered_command_handles() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            local refresh = h5v.commands.register({
              id = "analysis.refresh",
              run = function(ctx)
                ctx.command("help reload")
              end,
            })
            h5v.keys.bind({
              mode = h5v.ids.keymap_modes.global,
              key = "ctrl+r",
              target = refresh,
              description = "Refresh analysis",
            })
        "#,
        )
        .exec()
        .expect("register command handle binding");

        let keymaps = super::keymaps::parse_keymaps_config(&h5v)
            .expect("parse keymaps")
            .expect("keymaps config");
        assert!(matches!(
            keymaps.global.bind.first().map(|binding| &binding.target),
            Some(BoundAction::Command(command)) if command == "config.command.analysis.refresh"
        ));
    }

    #[test]
    fn declarative_keys_bind_accepts_command_targets() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            local refresh = h5v.commands.register({
              id = "analysis.refresh",
              run = function(ctx)
                ctx.command("help reload")
              end,
            })
            h5v.keys.bind({
              mode = h5v.ids.keymap_modes.global,
              key = "ctrl+r",
              target = refresh,
              description = "Refresh analysis",
            })
            h5v.keys.bind({
              mode = h5v.ids.keymap_modes.heatmap,
              key = "ctrl+z",
              target = h5v.actions.HeatmapZoomIn,
            })
        "#,
        )
        .exec()
        .expect("register declarative key bindings");

        let keymaps = super::keymaps::parse_keymaps_config(&h5v)
            .expect("parse keymaps")
            .expect("keymaps config");
        assert!(matches!(
            keymaps.global.bind.first().map(|binding| &binding.target),
            Some(BoundAction::Command(command)) if command == "config.command.analysis.refresh"
        ));
        assert!(matches!(
            keymaps.heatmap.bind.first().map(|binding| &binding.target),
            Some(BoundAction::Action(ContentAction::HeatmapZoomIn))
        ));
    }

    #[test]
    fn builtin_command_ids_are_exposed_under_h5v_ids_commands() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            h5v.keys.bind({
              mode = h5v.ids.keymap_modes.global,
              key = "ctrl+h",
              target = h5v.ids.commands.help,
            })
            h5v.keys.bind({
              mode = h5v.ids.keymap_modes.global,
              key = "ctrl+r",
              target = h5v.ids.commands.reload,
            })
        "#,
        )
        .exec()
        .expect("bind builtin command ids");

        let keymaps = super::keymaps::parse_keymaps_config(&h5v)
            .expect("parse keymaps")
            .expect("keymaps config");
        assert!(matches!(
            keymaps.global.bind.first().map(|binding| &binding.target),
            Some(BoundAction::Command(command)) if command == "builtin.command.help"
        ));
        assert!(matches!(
            keymaps.global.bind.get(1).map(|binding| &binding.target),
            Some(BoundAction::Command(command)) if command == "builtin.command.reload"
        ));
    }

    #[test]
    fn builtin_registry_ids_are_exposed_under_h5v_ids() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
        let ids: Table = h5v.get("ids").expect("get ids");
        let settings: Table = ids.get("settings").expect("get settings");
        let themes: Table = ids.get("themes").expect("get themes");
        let components: Table = ids.get("components").expect("get components");
        let symbol_themes: Table = ids.get("symbol_themes").expect("get symbol themes");
        let content_modes: Table = ids.get("content_modes").expect("get content modes");
        let events: Table = ids.get("events").expect("get events");
        let colors: Table = ids.get("colors").expect("get colors");
        let surface_colors: Table = colors.get("surface").expect("get colors.surface");
        let symbols: Table = ids.get("symbols").expect("get symbols");
        let tree_symbols: Table = symbols.get("tree").expect("get symbols.tree");
        let heatmap_settings: Table = settings.get("heatmap").expect("get settings.heatmap");
        let multichart_settings: Table =
            settings.get("multichart").expect("get settings.multichart");
        let value_kinds: Table = ids.get("value_kinds").expect("get value kinds");

        assert_eq!(
            settings.get::<String>("theme").expect("settings.theme"),
            "builtin.setting.theme"
        );
        assert_eq!(
            heatmap_settings
                .get::<String>("default_colormap")
                .expect("settings.heatmap.default_colormap"),
            "builtin.setting.heatmap.default_colormap"
        );
        assert_eq!(
            multichart_settings
                .get::<String>("detail_enabled")
                .expect("settings.multichart.detail_enabled"),
            "builtin.setting.multichart.detail_enabled"
        );
        assert_eq!(
            themes.get::<String>("dark").expect("themes.dark"),
            "builtin.theme.dark"
        );
        assert_eq!(
            components
                .get::<String>("heatmap")
                .expect("components.heatmap"),
            "heatmap"
        );
        assert_eq!(
            symbol_themes
                .get::<String>("compatibility")
                .expect("symbol_themes.compatibility"),
            "builtin.symbol_theme.compatibility"
        );
        assert_eq!(
            content_modes
                .get::<String>("heatmap")
                .expect("content_modes.heatmap"),
            "builtin.content_mode.heatmap"
        );
        assert_eq!(
            events
                .get::<String>("file_opened")
                .expect("events.file_opened"),
            "builtin.event.file_opened"
        );
        assert_eq!(
            surface_colors
                .get::<String>("panel_border")
                .expect("colors.surface.panel_border"),
            "builtin.color.surface.panel_border"
        );
        assert_eq!(
            tree_symbols
                .get::<String>("root_file_icon")
                .expect("symbols.tree.root_file_icon"),
            "builtin.symbol.tree.root_file_icon"
        );
        assert_eq!(
            value_kinds
                .get::<String>("series")
                .expect("value_kinds.series"),
            "series"
        );
    }

    #[test]
    fn events_on_registers_handlers_for_builtin_event_ids() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            h5v.events.on(h5v.ids.events.file_opened, function(ctx, ev)
              ctx.toast.info(ev.path)
            end)
        "#,
        )
        .exec()
        .expect("register file_opened handler");

        let events: Table = h5v.get("events").expect("get events");
        let handlers: Table = events.get("__handlers").expect("get handlers");
        let file_opened: Table = handlers
            .get("builtin.event.file_opened")
            .expect("get file_opened handlers");
        assert_eq!(file_opened.raw_len(), 1);
    }

    #[test]
    fn declarative_keys_unbind_accepts_table_form() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        lua.load(
            r#"
            h5v.keys.unbind({
              mode = h5v.ids.keymap_modes.heatmap,
              key = "v",
            })
        "#,
        )
        .exec()
        .expect("register declarative key unbind");

        let keymaps = super::keymaps::parse_keymaps_config(&h5v)
            .expect("parse keymaps")
            .expect("keymaps config");
        assert_eq!(keymaps.heatmap.unbind.len(), 1);
        assert_eq!(keymaps.heatmap.unbind[0].to_string(), "v");
    }

    #[test]
    fn parses_layout_configuration() {
        let _guard = test_guard();
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
    fn lua_registration_populates_registry_snapshot() {
        let _guard = test_guard();
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");

        h5v.set("theme", ThemeName::Light.as_str())
            .expect("set theme");
        h5v.set("symbol_theme", SymbolThemeName::Compatibility.as_str())
            .expect("set symbol theme");
        h5v.set("compatibility", true).expect("set compatibility");

        let order = lua.create_table().expect("create order table");
        order.set(1, "matrix").expect("set order");
        order.set(2, "preview").expect("set order");
        h5v.set("content_mode_order", order)
            .expect("set content mode order");

        let colors: Table = h5v.get("colors").expect("get colors");
        let content: Table = colors.get("content").expect("get colors.content");
        content
            .set("app_brand", "#010203")
            .expect("set content.app_brand");

        let symbols: Table = h5v.get("symbols").expect("get symbols");
        let tree: Table = symbols.get("tree").expect("get symbols.tree");
        tree.set("root_file_icon", "FILE ")
            .expect("set tree.root_file_icon");

        let heatmap: Table = h5v.get("heatmap").expect("get heatmap");
        heatmap
            .set("default_colormap", "inferno")
            .expect("set heatmap default colormap");
        heatmap
            .set("default_invert_x", true)
            .expect("set heatmap invert x");

        let multichart: Table = h5v.get("multichart").expect("get multichart");
        multichart
            .set("detail_max_samples", 8192)
            .expect("set multichart detail max samples");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");

        assert_eq!(
            snapshot
                .setting(&SettingHandle::new("builtin.setting.theme"))
                .and_then(|metadata| metadata.current_value.clone()),
            Some("builtin.theme.light".to_string())
        );
        assert_eq!(
            snapshot
                .setting(&SettingHandle::new("builtin.setting.symbol_theme"))
                .and_then(|metadata| metadata.current_value.clone()),
            Some("compatibility".to_string())
        );
        assert_eq!(
            snapshot
                .setting(&SettingHandle::new("builtin.setting.compatibility"))
                .and_then(|metadata| metadata.current_value.clone()),
            Some("true".to_string())
        );
        assert_eq!(
            snapshot
                .setting(&SettingHandle::new(
                    "builtin.setting.heatmap.default_colormap"
                ))
                .and_then(|metadata| metadata.current_value.clone()),
            Some("inferno".to_string())
        );
        assert_eq!(
            snapshot
                .setting(&SettingHandle::new(
                    "builtin.setting.multichart.detail_max_samples",
                ))
                .and_then(|metadata| metadata.current_value.clone()),
            Some("8192".to_string())
        );
        assert_eq!(
            snapshot
                .theme(&ThemeHandle::new("builtin.theme.light"))
                .map(|metadata| metadata.is_active),
            Some(true)
        );
        assert_eq!(
            snapshot
                .color(&ColorHandle::new("builtin.color.content.app_brand"))
                .and_then(|metadata| metadata.override_value.clone()),
            Some("#010203".to_string())
        );
        assert_eq!(
            snapshot
                .symbol(&SymbolHandle::new("builtin.symbol.tree.root_file_icon"))
                .and_then(|metadata| metadata.override_value.clone()),
            Some("FILE ".to_string())
        );
    }

    #[test]
    fn registry_snapshot_applies_runtime_visual_and_setting_state() {
        let _guard = test_guard();
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");

        h5v.set("theme", ThemeName::Light.as_str())
            .expect("set theme");
        h5v.set("symbol_theme", SymbolThemeName::Compatibility.as_str())
            .expect("set symbol theme");

        let order = lua.create_table().expect("create order table");
        order.set(1, "matrix").expect("set order");
        h5v.set("content_mode_order", order)
            .expect("set content mode order");

        let colors: Table = h5v.get("colors").expect("get colors");
        let content: Table = colors.get("content").expect("get colors.content");
        content
            .set("app_brand", "#010203")
            .expect("set content.app_brand");

        let symbols: Table = h5v.get("symbols").expect("get symbols");
        let tree: Table = symbols.get("tree").expect("get symbols.tree");
        tree.set("root_file_icon", "FILE ")
            .expect("set tree.root_file_icon");

        let heatmap: Table = h5v.get("heatmap").expect("get heatmap");
        heatmap
            .set("default_colormap", "inferno")
            .expect("set heatmap default colormap");
        heatmap
            .set("default_invert_x", true)
            .expect("set heatmap invert x");

        let multichart: Table = h5v.get("multichart").expect("get multichart");
        multichart
            .set("detail_max_samples", 8192)
            .expect("set multichart detail max samples");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        configure::apply_registry_snapshot(&snapshot).expect("apply registry snapshot");

        assert_eq!(configure::current_theme_name(), ThemeName::Light);
        assert_eq!(
            configure::current_symbol_theme_name(),
            SymbolThemeName::Compatibility
        );
        assert_eq!(
            themed_color(|colors| colors.content.app_brand),
            Color::Rgb(1, 2, 3)
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
        assert_eq!(
            current_heatmap_default_settings().colormap,
            HeatmapColormap::Inferno
        );
        assert!(current_heatmap_default_settings().invert_x);
        assert_eq!(
            configure::current_multichart_settings().detail_max_samples,
            8192
        );

        configure::reset_config(ThemeName::Dark);
    }

    #[test]
    fn custom_theme_registration_applies_theme_bundles() {
        let _guard = test_guard();
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");

        lua.load(
            r##"
            local theme = h5v.colors.themes.register({
              id = "config.theme.demo",
              title = "Demo",
              variant = "light",
              colors = {
                [h5v.ids.colors.content.app_brand] = "#010203",
              },
              symbols = {
                [h5v.ids.symbols.tree.root_file_icon] = "FILE ",
              },
            })
            h5v.theme = theme
        "##,
        )
        .exec()
        .expect("register custom theme");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        configure::apply_registry_snapshot(&snapshot).expect("apply registry snapshot");

        let metadata = snapshot
            .theme(&ThemeHandle::new("config.theme.demo"))
            .expect("custom theme metadata");
        assert!(metadata.is_active);
        assert_eq!(metadata.variant.as_deref(), Some("light"));
        assert_eq!(configure::current_theme_handle(), "config.theme.demo");
        assert_eq!(configure::current_theme_name(), ThemeName::Light);
        assert_eq!(
            themed_color(|colors| colors.content.app_brand),
            Color::Rgb(1, 2, 3)
        );
        assert_eq!(
            configured_symbol(|symbols| symbols.tree.root_file_icon),
            "FILE "
        );
    }

    #[test]
    fn lua_registration_registers_custom_commands() {
        let _guard = test_guard();
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");

        lua.load(
            r#"
            h5v.commands.register({
              id = "analysis.refresh",
              title = "Refresh analysis",
              summary = "Refresh generated analysis output",
              args = {
                { name = "count", kind = "uint", required = true, help = "Refresh count" },
              },
              examples = { "analysis.refresh 2" },
              run = function(ctx)
                ctx.command("help reload")
              end,
            })
        "#,
        )
        .exec()
        .expect("register custom command");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        let command = snapshot
            .command(&CommandHandle::new("config.command.analysis.refresh"))
            .expect("custom command metadata");

        assert_eq!(command.name, "analysis.refresh");
        assert_eq!(command.summary, "Refresh generated analysis output");
        assert_eq!(command.callback_id.as_deref(), Some("command-1"));
        assert_eq!(command.examples, vec!["analysis.refresh 2".to_string()]);
        assert_eq!(command.args.len(), 1);
        assert_eq!(command.args[0].kind, CommandArgValueKind::UnsignedInt);
    }

    #[test]
    fn lua_registration_registers_custom_mchart_functions() {
        let _guard = test_guard();
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");

        lua.load(
            r#"
            h5v.mchart.functions.register({
              id = "analysis.signal_to_noise",
              name = "signal_to_noise",
              summary = "Return a weighted sum for test coverage",
              params = {
                { name = "series", kind = h5v.ids.value_kinds.series, detail = "Input series" },
                { name = "scale", kind = h5v.ids.value_kinds.scalar, detail = "Scale factor" },
              },
              returns = h5v.ids.value_kinds.scalar,
              eval = function(series, scale)
                local total = 0
                for _, value in ipairs(series.to_array()) do
                  total = total + value
                end
                return total * scale
              end,
            })
        "#,
        )
        .exec()
        .expect("register custom multichart function");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        let function = snapshot
            .mchart_function(&MchartFunctionHandle::new(
                "config.mchart_function.analysis.signal_to_noise",
            ))
            .expect("custom multichart function metadata");

        assert_eq!(function.name, "signal_to_noise");
        assert_eq!(
            function.return_kind,
            crate::configure::registry::RegistryValueKind::Scalar
        );
        assert_eq!(function.callback_id.as_deref(), Some("mchart-function-1"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(
            function.params[0].value_kind,
            crate::configure::registry::RegistryValueKind::Series
        );
        assert_eq!(
            function.params[1].value_kind,
            crate::configure::registry::RegistryValueKind::Scalar
        );
    }

    #[test]
    fn custom_mchart_worker_runtime_executes_registered_functions_without_ctx_arg() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("init.lua");
        std::fs::write(
            &config_path,
            r#"
h5v.mchart.functions.register({
  id = "analysis.signal_to_noise",
  name = "signal_to_noise",
  params = {
    { name = "series", kind = h5v.ids.value_kinds.series },
    { name = "scale", kind = h5v.ids.value_kinds.scalar },
  },
  returns = h5v.ids.value_kinds.scalar,
  eval = function(series, scale)
    local total = 0
    for _, value in ipairs(series.to_array()) do
      total = total + value
    end
    return total * scale
  end,
})
"#,
        )
        .expect("write config");
        let previous_override =
            configure::set_config_path_override(Some(config_path)).expect("override config");
        configure::reset_mchart_worker_runtime();

        let result = crate::configure::run_registered_mchart_function(
            "mchart-function-1",
            &[
                crate::configure::LuaMchartArgValue::Series(vec![
                    (0.0, 2.0),
                    (1.0, 4.0),
                    (2.0, 6.0),
                ]),
                crate::configure::LuaMchartArgValue::Scalar(0.5),
            ],
            crate::configure::registry::RegistryValueKind::Scalar,
        )
        .expect("run custom mchart function");

        assert_eq!(result, crate::configure::LuaMchartReturnValue::Scalar(6.0));

        configure::set_config_path_override(previous_override).expect("restore config path");
        configure::reset_mchart_worker_runtime();
    }

    #[test]
    fn custom_mchart_worker_runtime_supports_process_backed_scalar_and_series_results() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("init.lua");
        std::fs::write(
            &config_path,
            r#"
h5v.mchart.functions.register({
  id = "analysis.process_scalar",
  name = "process_scalar",
  params = {
    { name = "value", kind = h5v.ids.value_kinds.scalar },
  },
  returns = h5v.ids.value_kinds.scalar,
  eval = function(value)
    local result = h5v.mchart.functions.process.run({
      command = {"cat"},
      stdin = tostring(value),
    })
    return h5v.mchart.functions.process.parse_scalar(result)
  end,
})

h5v.mchart.functions.register({
  id = "analysis.process_series",
  name = "process_series",
  params = {
    { name = "series", kind = h5v.ids.value_kinds.series },
  },
  returns = h5v.ids.value_kinds.series,
  eval = function(series)
    local result = h5v.mchart.functions.process.run({
      command = {"cat"},
      stdin = series.to_lines(),
    })
    return h5v.mchart.functions.process.parse_series(result)
  end,
})
"#,
        )
        .expect("write config");
        let previous_override =
            configure::set_config_path_override(Some(config_path)).expect("override config");
        configure::reset_mchart_worker_runtime();

        let scalar_result = crate::configure::run_registered_mchart_function(
            "mchart-function-1",
            &[crate::configure::LuaMchartArgValue::Scalar(3.5)],
            crate::configure::registry::RegistryValueKind::Scalar,
        )
        .expect("run process-backed scalar function");
        assert_eq!(
            scalar_result,
            crate::configure::LuaMchartReturnValue::Scalar(3.5)
        );

        let series_result = crate::configure::run_registered_mchart_function(
            "mchart-function-2",
            &[crate::configure::LuaMchartArgValue::Series(vec![
                (0.0, 2.0),
                (1.0, 4.0),
                (2.0, 6.0),
            ])],
            crate::configure::registry::RegistryValueKind::Series,
        )
        .expect("run process-backed series function");
        assert_eq!(
            series_result,
            crate::configure::LuaMchartReturnValue::Series(vec![2.0, 4.0, 6.0])
        );

        configure::set_config_path_override(previous_override).expect("restore config path");
        configure::reset_mchart_worker_runtime();
    }

    #[test]
    fn process_context_parse_json_converts_stdout_into_lua_tables() {
        let lua = Lua::new();
        let process = crate::configure::build_lua_process_context(&lua).expect("build process");
        let parse_json: mlua::Function = process.get("parse_json").expect("parse_json");
        let parsed: mlua::Table = parse_json
            .call(r#"{"ok":true,"items":[1,2,3],"nested":{"name":"demo"}}"#)
            .expect("parse json");
        assert!(parsed.get::<bool>("ok").expect("ok"));
        let items: mlua::Table = parsed.get("items").expect("items");
        assert_eq!(items.get::<i64>(1).expect("item1"), 1);
        assert_eq!(items.get::<i64>(3).expect("item3"), 3);
        let nested: mlua::Table = parsed.get("nested").expect("nested");
        assert_eq!(nested.get::<String>("name").expect("name"), "demo");
    }

    #[test]
    fn plugins_use_loads_local_plugin_and_registers_metadata() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("lua")).expect("create plugin lua dir");
        std::fs::write(
            temp.path().join("h5v-plugin.toml"),
            r#"
id = "demo.analysis"
name = "Demo analysis"
version = "0.1.0"
api_version = "2"
entry = "lua/analysis.lua"
"#,
        )
        .expect("write plugin manifest");
        std::fs::write(
            temp.path().join("lua/analysis.lua"),
            r#"
return {
  health = function(ctx)
    return {
      status = ctx.health.healthy,
      summary = "ready",
      message = ctx.ui.build(function(ui)
        ui.block({ title = "Plugin health" }, function(ui)
          ui.badge("ok")
          ui.text("ready")
        end)
      end),
    }
  end,
  init = function(h5v)
    h5v.commands.register({
      id = "analysis.refresh",
      title = "Refresh analysis",
      summary = "Refresh plugin-provided analysis",
      run = function(ctx)
        ctx.command("help reload")
      end,
    })
  end,
}
"#,
        )
        .expect("write plugin entry");

        let plugin_path = temp.path().display().to_string().replace('\\', "\\\\");
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");

        lua.load(format!(r#"loaded = h5v.plugins.use("{plugin_path}")"#))
            .exec()
            .expect("load local plugin");

        let loaded: String = lua.globals().get("loaded").expect("plugin handle");
        assert_eq!(loaded, "plugin.demo.analysis");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        let plugin = snapshot
            .plugins()
            .find(|plugin| plugin.handle.as_str() == "plugin.demo.analysis")
            .expect("plugin metadata");
        assert_eq!(plugin.name, "Demo analysis");
        assert_eq!(plugin.version.as_deref(), Some("0.1.0"));
        assert_eq!(plugin.source.as_deref(), Some(plugin_path.as_str()));
        assert_eq!(plugin.resolved_commit.as_deref(), Some("local"));
        assert!(plugin.auto_pull);
        assert_eq!(plugin.health_status, crate::health::HealthStatus::Healthy);
        assert_eq!(plugin.health_message.as_deref(), Some("ready"));
        assert!(plugin.health_ui_document.is_some());

        let command = snapshot
            .command(&CommandHandle::new(
                "plugin.demo.analysis.command.analysis.refresh",
            ))
            .expect("plugin command metadata");
        assert_eq!(
            command.owner,
            crate::configure::registry::RegistryOwner::Plugin(
                crate::configure::registry::PluginHandle::new("plugin.demo.analysis")
            )
        );
    }

    #[test]
    fn run_lua_engine_records_configuration_failures_as_health_issues() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("init.lua");
        std::fs::write(&config_path, "this is not valid lua").expect("write config");
        let previous_override =
            configure::set_config_path_override(Some(config_path)).expect("override config");
        crate::health::clear_reported_health_issues();

        let (tx, _rx) = std::sync::mpsc::channel();
        let error = configure::run_lua_engine(tx, false).expect_err("config should fail");
        let issues = crate::health::reported_health_issues();

        assert!(error.to_string().contains("Lua error"));
        assert!(issues.iter().any(|issue| {
            issue.source == "configuration"
                && issue.result.status == crate::health::HealthStatus::Fail
                && issue.result.message.contains("Lua error")
        }));

        configure::set_config_path_override(previous_override).expect("restore config path");
        crate::health::clear_reported_health_issues();
    }

    #[test]
    fn run_lua_engine_records_plugin_load_failures_as_health_issues() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("init.lua");
        std::fs::write(
            &config_path,
            r#"
h5v.plugins.use("/definitely/missing/h5v-plugin")
"#,
        )
        .expect("write config");
        let previous_override =
            configure::set_config_path_override(Some(config_path)).expect("override config");
        crate::health::clear_reported_health_issues();

        let (tx, _rx) = std::sync::mpsc::channel();
        let error = configure::run_lua_engine(tx, false).expect_err("plugin load should fail");
        let issues = crate::health::reported_health_issues();

        assert!(error.to_string().contains("Failed to resolve plugin path"));
        assert!(issues.iter().any(|issue| {
            issue
                .source
                .contains("plugin /definitely/missing/h5v-plugin")
                && issue.result.status == crate::health::HealthStatus::Fail
                && issue.result.message.contains("Failed to load plugin")
        }));

        configure::set_config_path_override(previous_override).expect("restore config path");
        crate::health::clear_reported_health_issues();
    }

    #[test]
    fn failing_plugin_healthcheck_disables_plugin_registrations_and_keymaps() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("lua")).expect("create plugin lua dir");
        std::fs::write(
            temp.path().join("h5v-plugin.toml"),
            r#"
id = "demo.unhealthy"
name = "Demo unhealthy"
version = "0.1.0"
api_version = "2"
entry = "lua/init.lua"
"#,
        )
        .expect("write plugin manifest");
        std::fs::write(
            temp.path().join("lua/init.lua"),
            r#"
return {
  health = function(ctx)
    return {
      status = ctx.health.fail,
      message = "tool 'cat' not found",
    }
  end,
  init = function(h5v)
    local refresh = h5v.commands.register({
      id = "analysis.refresh",
      run = function(ctx)
        ctx.toast.info("refreshed")
      end,
    })

    h5v.keys.bind({
      mode = h5v.ids.keymap_modes.global,
      key = "ctrl+r",
      target = refresh,
      description = "Refresh unhealthy plugin output",
    })
  end,
}
"#,
        )
        .expect("write plugin entry");

        let plugin_path = temp.path().display().to_string().replace('\\', "\\\\");
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");

        lua.load(format!(r#"h5v.plugins.use("{plugin_path}")"#))
            .exec()
            .expect("load unhealthy plugin");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        configure::apply_registry_snapshot(&snapshot).expect("apply registry snapshot");
        super::apply_non_registry_lua_config(&h5v).expect("apply non-registry config");

        let plugin = snapshot
            .plugins()
            .find(|plugin| plugin.handle.as_str() == "plugin.demo.unhealthy")
            .expect("plugin metadata");
        assert_eq!(plugin.health_status, crate::health::HealthStatus::Fail);
        assert_eq!(
            plugin.health_message.as_deref(),
            Some("tool 'cat' not found")
        );
        assert!(snapshot
            .command(&CommandHandle::new(
                "plugin.demo.unhealthy.command.analysis.refresh"
            ))
            .is_none());
        assert!(current_keymaps().global.iter().all(
            |binding| binding.description.as_deref() != Some("Refresh unhealthy plugin output")
        ));
    }

    #[test]
    fn plugin_missing_health_function_is_modeled_as_unhealthy_plugin() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("lua")).expect("create plugin lua dir");
        std::fs::write(
            temp.path().join("h5v-plugin.toml"),
            r#"
id = "demo.missing-health"
name = "Demo missing health"
version = "0.1.0"
api_version = "2"
entry = "lua/init.lua"
"#,
        )
        .expect("write plugin manifest");
        std::fs::write(
            temp.path().join("lua/init.lua"),
            r#"
return {
  init = function(h5v, ctx)
    ctx.toast.info("loaded")
  end,
}
"#,
        )
        .expect("write plugin entry");

        let plugin_path = temp.path().display().to_string().replace('\\', "\\\\");
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");
        crate::health::clear_reported_health_issues();

        lua.load(format!(r#"h5v.plugins.use("{plugin_path}")"#))
            .exec()
            .expect("load plugin with missing health");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        let plugin = snapshot
            .plugins()
            .find(|plugin| plugin.handle.as_str() == "plugin.demo.missing-health")
            .expect("plugin metadata");

        assert_eq!(plugin.health_status, crate::health::HealthStatus::Fail);
        assert!(plugin
            .health_message
            .as_deref()
            .expect("health message")
            .contains("must return a 'health' function"));
        assert!(crate::health::reported_health_issues().is_empty());
        crate::health::clear_reported_health_issues();
    }

    #[test]
    fn plugin_init_failure_marks_plugin_unhealthy_without_h5v_issue() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("lua")).expect("create plugin lua dir");
        std::fs::write(
            temp.path().join("h5v-plugin.toml"),
            r#"
id = "demo.init-fail"
name = "Demo init fail"
version = "0.1.0"
api_version = "2"
entry = "lua/init.lua"
"#,
        )
        .expect("write plugin manifest");
        std::fs::write(
            temp.path().join("lua/init.lua"),
            r#"
return {
  health = function(ctx)
    return {
      status = ctx.health.healthy,
      message = "ready",
    }
  end,
  init = function(h5v, ctx)
    error("boom")
  end,
}
"#,
        )
        .expect("write plugin entry");

        let plugin_path = temp.path().display().to_string().replace('\\', "\\\\");
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");
        crate::health::clear_reported_health_issues();

        lua.load(format!(r#"h5v.plugins.use("{plugin_path}")"#))
            .exec()
            .expect("load plugin with init failure");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        let plugin = snapshot
            .plugins()
            .find(|plugin| plugin.handle.as_str() == "plugin.demo.init-fail")
            .expect("plugin metadata");

        assert_eq!(plugin.health_status, crate::health::HealthStatus::Fail);
        assert!(plugin
            .health_message
            .as_deref()
            .expect("health message")
            .contains("Plugin init failed"));
        assert!(crate::health::reported_health_issues().is_empty());
        crate::health::clear_reported_health_issues();
    }

    #[test]
    fn acceptance_plugin_command_keymap_event_and_content_order_work_together() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("lua")).expect("create plugin lua dir");
        std::fs::write(
            temp.path().join("h5v-plugin.toml"),
            r#"
id = "demo.acceptance"
name = "Demo acceptance"
version = "0.1.0"
api_version = "2"
entry = "lua/init.lua"
"#,
        )
        .expect("write plugin manifest");
        std::fs::write(
            temp.path().join("lua/init.lua"),
            r#"
return {
  health = function(ctx)
    return {
      status = ctx.health.healthy,
      message = "ready",
    }
  end,
  init = function(h5v)
    local refresh = h5v.commands.register({
      id = "analysis.refresh",
      title = "Refresh analysis",
      summary = "Refresh plugin analysis output",
      run = function(ctx)
        ctx.toast.info("refreshed")
      end,
    })

    h5v.__test_plugin_command = refresh
  end,
}
"#,
        )
        .expect("write plugin entry");

        let plugin_path = temp.path().display().to_string().replace('\\', "\\\\");
        let lua = Lua::new();
        let bootstrap_registry = configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &bootstrap_registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");

        lua.load(format!(
            r#"
            h5v.plugins.use("{plugin_path}")
            h5v.theme = "light"
            h5v.symbol_theme = "compatibility"
            h5v.content_mode_order = {{ "heatmap", "preview" }}
            h5v.keys.bind({{
              mode = h5v.ids.keymap_modes.global,
              key = "ctrl+r",
              target = h5v.__test_plugin_command,
              description = "Refresh plugin analysis",
            }})
            h5v.events.on(h5v.ids.events.file_opened, function(ctx, ev)
              ctx.toast.info(ev.path)
            end)
        "#
        ))
        .exec()
        .expect("configure plugin-backed acceptance flow");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        configure::apply_registry_snapshot(&snapshot).expect("apply registry snapshot");
        super::apply_non_registry_lua_config(&h5v).expect("apply non-registry config");
        configure::install_registry_snapshot(snapshot);
        crate::ui::command::sync_command_registry_keybindings(&current_keymaps());

        let registry = configure::current_registry_snapshot();
        let command = registry
            .command(&CommandHandle::new(
                "plugin.demo.acceptance.command.analysis.refresh",
            ))
            .expect("plugin command metadata");
        assert_eq!(command.name, "analysis.refresh");
        assert_eq!(
            command.owner,
            crate::configure::registry::RegistryOwner::Plugin(
                crate::configure::registry::PluginHandle::new("plugin.demo.acceptance")
            )
        );
        assert!(command.keybindings.contains(&"Ctrl+r".to_string()));
        assert_eq!(configure::current_theme_name(), ThemeName::Light);
        assert_eq!(
            configure::current_symbol_theme_name(),
            SymbolThemeName::Compatibility
        );
        assert_eq!(
            current_content_mode_order(),
            vec![
                ContentShowMode::Heatmap,
                ContentShowMode::Preview,
                ContentShowMode::Matrix
            ]
        );
        assert_eq!(
            current_content_mode_order_handles(),
            vec![
                ContentShowMode::Heatmap.handle(),
                ContentShowMode::Preview.handle(),
                ContentShowMode::Matrix.handle()
            ]
        );

        let events: Table = h5v.get("events").expect("get events");
        let handlers: Table = events.get("__handlers").expect("get handlers");
        let file_opened: Table = handlers
            .get("builtin.event.file_opened")
            .expect("get file_opened handlers");
        assert_eq!(file_opened.raw_len(), 1);

        configure::install_registry_snapshot(
            configure::builtin_registry_snapshot().expect("restore builtin registry"),
        );
        configure::reset_config(ThemeName::Dark);
    }

    #[test]
    fn applies_layout_configuration() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
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
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
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
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
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
        let _guard = test_guard();
        let lua = Lua::new();
        let error = execute_config_chunk(&lua, "@/tmp/init.lua", "h5v.theme =\n")
            .expect_err("invalid Lua should error");

        let rendered = error.to_string();
        assert!(rendered.contains("/tmp/init.lua:2"), "{rendered}");
        assert!(!rendered.contains("src/configure/lua.rs"));
    }

    #[test]
    fn exports_nested_theme_tables() {
        let _guard = test_guard();
        let lua = Lua::new();
        let h5v = lua.create_table().expect("create h5v");
        let colors = lua.create_table().expect("create colors");
        h5v.set("colors", colors).expect("set colors");
        let plugins = lua.create_table().expect("create plugins");
        h5v.set("plugins", plugins).expect("set plugins");
        let themes = build_theme_table(&lua, &h5v).expect("build themes");
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
        let _guard = test_guard();
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
        let _guard = test_guard();
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
        let _guard = test_guard();
        let lua = Lua::new();
        let h5v = lua.create_table().expect("create h5v table");
        let order = lua.create_table().expect("create order");
        order.set(1, "bogus").expect("set order");
        h5v.set("content_mode_order", order).expect("set order");

        let error = parse_content_mode_order(&h5v).expect_err("unknown mode should error");
        assert!(error.to_string().contains("Unknown content mode"));
    }

    #[test]
    fn registers_custom_content_mode_metadata() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");
        let mut builder = configure::builtin_registry_builder().expect("build registry builder");

        lua.load(
            r#"
            h5v.ui.content_modes.register({
              id = "analysis.results",
              title = "Analysis",
              summary = "Show analysis results",
              render = function(ctx, ui)
                ui.text(ctx.file.path)
              end,
            })
        "#,
        )
        .exec()
        .expect("register custom content mode");

        register_lua_config(&mut builder, &h5v).expect("register lua config");
        let snapshot = builder.freeze().expect("freeze registry");
        let content_mode = snapshot
            .content_mode(&ContentModeHandle::new(
                "config.content_mode.analysis.results",
            ))
            .expect("content mode metadata");
        assert_eq!(content_mode.title, "Analysis");
        assert_eq!(content_mode.summary, "Show analysis results");
        assert!(content_mode.callback_id.is_some());
    }

    #[test]
    fn content_mode_order_accepts_registered_custom_mode_handles() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
        lua.globals().set("h5v", h5v.clone()).expect("set h5v");

        lua.load(
            r#"
            local analysis = h5v.ui.content_modes.register({
              id = "analysis.results",
              title = "Analysis",
              render = function(ctx, ui)
                ui.text("ok")
              end,
            })
            h5v.content_mode_order = {
              analysis,
              h5v.ids.content_modes.preview,
            }
        "#,
        )
        .exec()
        .expect("configure custom content mode order");

        assert_eq!(
            parse_content_mode_order(&h5v).expect("parse order"),
            Some(vec![
                ContentModeHandle::new("config.content_mode.analysis.results"),
                ContentShowMode::Preview.handle(),
            ])
        );
    }

    #[test]
    fn direct_nested_color_assignment_works_without_manual_table_setup() {
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
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
        let _guard = test_guard();
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
        let _guard = test_guard();
        let lua = Lua::new();
        let registry = crate::configure::builtin_registry_snapshot().expect("build registry");
        let h5v = build_h5v_table(&lua, &registry, None, false).expect("build h5v");
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
