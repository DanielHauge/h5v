use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use crate::{
    configure::errors::ConfigureErrors,
    configure::{self, SymbolThemeName, ThemeName},
};
use serde_json::{json, Value};

const LUA_LS_LIBRARY_DIR: &str = ".h5v-luals";
const LUA_LS_STUB_FILE: &str = "h5v.lua";
const LUA_LS_CONFIG_FILE: &str = ".luarc.json";
const LUA_LS_GENERATED_KIND: &str = "generated-luals-config";

pub fn config_path() -> Result<PathBuf, ConfigureErrors> {
    Ok(dirs::config_dir()
        .unwrap_or(std::env::current_dir().map_err(ConfigureErrors::NoCurrentDir)?)
        .join("h5v")
        .join("init.lua")
        .with_file_name("init.lua")
        .with_extension("lua"))
}

pub fn load_or_create_config() -> Result<String, ConfigureErrors> {
    let config_path = ensure_config_exists()?;

    let init_lua_content =
        std::fs::read_to_string(&config_path).map_err(ConfigureErrors::FailureToReadConfig)?;

    Ok(init_lua_content)
}

pub fn ensure_config_exists() -> Result<PathBuf, ConfigureErrors> {
    let config_path = config_path()?;
    if !std::path::Path::new(&config_path).exists() {
        write_default_config(&config_path)?;
    } else {
        ensure_lua_ls_support_files(&config_path)?;
    }
    Ok(config_path)
}

pub fn reset_config_to_default() -> Result<PathBuf, ConfigureErrors> {
    let config_path = config_path()?;
    write_default_config(&config_path)?;
    Ok(config_path)
}

fn default_config_contents() -> String {
    let mut lines = vec![
        "-- H5V Lua configuration file".to_string(),
        "-- Pick built-in color/symbol themes, then override any named values you want."
            .to_string(),
        format!(
            "-- Available themes: {}",
            configure::available_theme_names().join(", ")
        ),
        format!("-- h5v.theme = \"{}\"", ThemeName::Dark.as_str()),
        format!(
            "-- Available symbol themes: {}",
            configure::available_symbol_theme_names().join(", ")
        ),
        format!(
            "-- h5v.symbol_theme = \"{}\"",
            SymbolThemeName::Rich.as_str()
        ),
        "-- Compatibility precedence: CLI flag > h5v.compatibility > H5V_COMPATIBILITY_MODE"
            .to_string(),
        "-- h5v.compatibility = false".to_string(),
        "-- Content mode precedence/default: first available mode in this list wins".to_string(),
        "-- h5v.content_mode_order = { \"preview\", \"matrix\", \"heatmap\" }".to_string(),
        "-- Heatmap custom range presets can mix exact bounds and percentages.".to_string(),
        "-- h5v.heatmap = {".to_string(),
        "--   default_range = \"Auto\",".to_string(),
        "--   default_colormap = \"turbo\",".to_string(),
        "--   default_normalization = \"linear\",".to_string(),
        "--   default_invert_x = false,".to_string(),
        "--   default_invert_y = false,".to_string(),
        "--   default_invert_c = false,".to_string(),
        "--   range_modes = {".to_string(),
        "--     { label = \"5-80%\", min = \"5%\", max = \"80%\" },".to_string(),
        "--     { label = \"2.5..5.5\", min = 2.5, max = 5.5 },".to_string(),
        "--   },".to_string(),
        "-- }".to_string(),
        "-- Keymaps are layered: heatmap/content/tree/attributes/mchart > normal > global."
            .to_string(),
        "-- Use bind(mode, key, action[, description]), bind_command(...), bind_script(...), bind_commands(...), bind_lua(...), and unbind(mode, key)."
            .to_string(),
        "-- Constants live under h5v.modes and h5v.actions."
            .to_string(),
        "-- Scope tables still support clear_defaults, bind, unbind, and command-backed entries."
            .to_string(),
        "-- bind(h5v.modes.Global, \"ctrl+h\", h5v.actions.ShowHelp, \"Show help\")"
            .to_string(),
        "-- unbind(h5v.modes.Heatmap, \"v\")".to_string(),
        "-- bind_command(h5v.modes.Heatmap, \"ctrl+alt+r\", \"heatmap range use \\\"Clip 1-99%\\\"\")"
            .to_string(),
        "-- bind_commands(h5v.modes.Global, \"ctrl+k\", { \"down 2\", \"up 1\" })".to_string(),
        "-- bind_lua(h5v.modes.Global, \"ctrl+l\", function(ctx) ctx.command(\"help reload\") end)"
            .to_string(),
        "-- h5v.keymaps.heatmap = {".to_string(),
        "--   bind = {".to_string(),
        "--     { key = \"ctrl+z\", action = h5v.actions.HeatmapZoomIn },".to_string(),
        "--   },".to_string(),
        "-- }".to_string(),
        "--".to_string(),
        format!(
            "-- LuaLS support files are generated beside this config under {LUA_LS_LIBRARY_DIR}/."
        ),
        "-- If your editor uses LuaLS, opening this directory or file should pick them up automatically."
            .to_string(),
        String::new(),
        "-- Colors accept #RRGGBB or names like blue, magenta, lightgreen, darkgray.".to_string(),
        "-- h5v.colors = {".to_string(),
    ];
    append_grouped_color_examples(&mut lines);
    lines.push("-- }".to_string());
    lines.push("--".to_string());
    lines.push(
        "-- Symbols accept any Lua string; use simple ASCII fallbacks if your terminal needs them."
            .to_string(),
    );
    lines.push("-- h5v.symbols = {".to_string());
    append_grouped_symbol_examples(&mut lines);
    lines.push("-- }".to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn collect_grouped_entries<I>(entries: I) -> Vec<(String, Vec<(String, String)>)>
where
    I: IntoIterator<Item = (&'static str, String)>,
{
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (name, value) in entries {
        let (group, key) = name.split_once('.').unwrap_or(("", name));
        if let Some((_, entries)) = groups.iter_mut().find(|(existing, _)| existing == group) {
            entries.push((key.to_string(), value));
        } else {
            groups.push((group.to_string(), vec![(key.to_string(), value)]));
        }
    }
    groups
}

fn pascal_case(name: &str) -> String {
    name.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = String::new();
                    out.push(first.to_ascii_uppercase());
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn lua_string_union<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    values
        .into_iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join("|")
}

fn color_group_class_name(group: &str) -> String {
    format!("H5vColor{}", pascal_case(group))
}

fn symbol_group_class_name(group: &str) -> String {
    format!("H5vSymbol{}", pascal_case(group))
}

fn lua_ls_stub_contents() -> String {
    let color_groups = collect_grouped_entries(
        configure::theme_named_colors(ThemeName::Dark)
            .into_iter()
            .map(|(name, color)| (name, configure::color_to_lua_string(color))),
    );
    let symbol_groups = collect_grouped_entries(
        configure::theme_named_symbols(SymbolThemeName::Rich)
            .into_iter()
            .map(|(name, value)| (name, value.to_string())),
    );
    let mode_codes: Vec<&'static str> = crate::ui::input::keymap::exported_mode_codes()
        .iter()
        .map(|(_, code)| *code)
        .collect();
    let action_codes: Vec<&'static str> = crate::ui::input::keymap::exported_action_codes()
        .into_iter()
        .map(|action| action.code)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut lines = vec![
        "---@meta".to_string(),
        "---@diagnostic disable: undefined-global".to_string(),
        "---@alias H5vThemeName \"dark\"|\"light\"".to_string(),
        "---@alias H5vSymbolThemeName \"rich\"|\"compatibility\"".to_string(),
        "---@alias H5vContentMode \"preview\"|\"matrix\"|\"heatmap\"".to_string(),
        format!("---@alias H5vModeCode {}", lua_string_union(mode_codes.iter().copied())),
        format!(
            "---@alias H5vActionCode {}",
            lua_string_union(action_codes.iter().copied())
        ),
        "---@alias H5vHeatmapColormap \"turbo\"|\"grayscale\"|\"inferno\"".to_string(),
        "---@alias H5vHeatmapNormalization \"linear\"|\"log\"|\"sqrt\"".to_string(),
        "---@class H5vKeymapLuaContext".to_string(),
        "---@field command fun(command: string)".to_string(),
        "---@field commands fun(commands: string[])".to_string(),
        "---@field script fun(script: string)".to_string(),
        "---@class H5vHeatmapRangePreset".to_string(),
        "---@field label? string".to_string(),
        "---@field min string|number".to_string(),
        "---@field max string|number".to_string(),
        "---@class H5vHeatmapConfig".to_string(),
        "---@field default_range string".to_string(),
        "---@field default_colormap H5vHeatmapColormap".to_string(),
        "---@field default_normalization H5vHeatmapNormalization".to_string(),
        "---@field default_invert_x boolean".to_string(),
        "---@field default_invert_y boolean".to_string(),
        "---@field default_invert_c boolean".to_string(),
        "---@field range_modes H5vHeatmapRangePreset[]".to_string(),
        "---@class H5vKeymapBinding".to_string(),
        "---@field key string".to_string(),
        "---@field action? H5vActionCode".to_string(),
        "---@field command? string".to_string(),
        "---@field commands? string[]".to_string(),
        "---@field script? string".to_string(),
        "---@field lua? fun(ctx: H5vKeymapLuaContext)".to_string(),
        "---@field description? string".to_string(),
        "---@class H5vKeymapScope".to_string(),
        "---@field clear_defaults? boolean".to_string(),
        "---@field unbind? string[]".to_string(),
        "---@field bind? H5vKeymapBinding[]".to_string(),
        "---@class H5vKeymaps".to_string(),
        "---@field bind fun(mode: H5vModeCode, key: string, action: H5vActionCode, description?: string)"
            .to_string(),
        "---@field bind_command fun(mode: H5vModeCode, key: string, command: string, description?: string)"
            .to_string(),
        "---@field bind_commands fun(mode: H5vModeCode, key: string, commands: string[], description?: string)"
            .to_string(),
        "---@field bind_script fun(mode: H5vModeCode, key: string, script: string, description?: string)"
            .to_string(),
        "---@field bind_lua fun(mode: H5vModeCode, key: string, callback: fun(ctx: H5vKeymapLuaContext), description?: string)"
            .to_string(),
        "---@field unbind fun(mode: H5vModeCode, key: string)".to_string(),
        "---@field global? H5vKeymapScope".to_string(),
        "---@field normal? H5vKeymapScope".to_string(),
        "---@field window? H5vKeymapScope".to_string(),
        "---@field tree? H5vKeymapScope".to_string(),
        "---@field content? H5vKeymapScope".to_string(),
        "---@field heatmap? H5vKeymapScope".to_string(),
        "---@field attributes? H5vKeymapScope".to_string(),
        "---@field mchart? H5vKeymapScope".to_string(),
        "---@class H5vModes".to_string(),
    ];
    for (symbol, code) in crate::ui::input::keymap::exported_mode_codes() {
        lines.push(format!("---@field {} \"{}\"", symbol, code));
    }
    lines.push("---@class H5vColorOverrides".to_string());
    for (group, _) in &color_groups {
        lines.push(format!(
            "---@field {}? {}",
            group,
            color_group_class_name(group)
        ));
    }
    lines.push("---@class H5vActions".to_string());
    for action in crate::ui::input::keymap::exported_action_codes() {
        lines.push(format!("---@field {} \"{}\"", action.symbol, action.code));
    }
    lines.push("---@class H5vSymbolOverrides".to_string());
    for (group, _) in &symbol_groups {
        lines.push(format!(
            "---@field {}? {}",
            group,
            symbol_group_class_name(group)
        ));
    }
    lines.push("---@class H5vThemeCatalog".to_string());
    for theme_name in configure::available_theme_names() {
        lines.push(format!("---@field {} H5vColorOverrides", theme_name));
    }
    lines.push("---@class H5vSymbolThemeCatalog".to_string());
    for theme_name in configure::available_symbol_theme_names() {
        lines.push(format!("---@field {} H5vSymbolOverrides", theme_name));
    }
    lines.push("---@class H5vConfig".to_string());
    lines.push("---@field log fun(message: string)".to_string());
    lines.push("---@field compatibility boolean".to_string());
    lines.push("---@field content_mode_order H5vContentMode[]".to_string());
    lines.push("---@field theme H5vThemeName".to_string());
    lines.push("---@field symbol_theme H5vSymbolThemeName".to_string());
    lines.push("---@field heatmap H5vHeatmapConfig".to_string());
    lines.push("---@field keymaps H5vKeymaps".to_string());
    lines.push("---@field modes H5vModes".to_string());
    lines.push("---@field actions H5vActions".to_string());
    lines.push("---@field colors H5vColorOverrides".to_string());
    lines.push("---@field symbols H5vSymbolOverrides".to_string());
    lines.push("---@field themes H5vThemeCatalog".to_string());
    lines.push("---@field symbol_themes H5vSymbolThemeCatalog".to_string());

    for (group, entries) in color_groups {
        lines.push(format!("---@class {}", color_group_class_name(&group)));
        for (key, _) in entries {
            lines.push(format!("---@field {}? string", key));
        }
    }
    for (group, entries) in symbol_groups {
        lines.push(format!("---@class {}", symbol_group_class_name(&group)));
        for (key, _) in entries {
            lines.push(format!("---@field {}? string", key));
        }
    }
    lines.push("---@type H5vConfig".to_string());
    lines.push("h5v = h5v".to_string());
    lines.push(
        "---@type fun(mode: H5vModeCode, key: string, action: H5vActionCode, description?: string)"
            .to_string(),
    );
    lines.push("bind = bind".to_string());
    lines.push(
        "---@type fun(mode: H5vModeCode, key: string, command: string, description?: string)"
            .to_string(),
    );
    lines.push("bind_command = bind_command".to_string());
    lines.push(
        "---@type fun(mode: H5vModeCode, key: string, commands: string[], description?: string)"
            .to_string(),
    );
    lines.push("bind_commands = bind_commands".to_string());
    lines.push(
        "---@type fun(mode: H5vModeCode, key: string, script: string, description?: string)"
            .to_string(),
    );
    lines.push("bind_script = bind_script".to_string());
    lines.push(
        "---@type fun(mode: H5vModeCode, key: string, callback: fun(ctx: H5vKeymapLuaContext), description?: string)"
            .to_string(),
    );
    lines.push("bind_lua = bind_lua".to_string());
    lines.push("---@type fun(mode: H5vModeCode, key: string)".to_string());
    lines.push("unbind = unbind".to_string());
    lines.join("\n")
}

fn lua_ls_config_json() -> Value {
    json!({
        "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
        "workspace": {
            "library": [LUA_LS_LIBRARY_DIR]
        },
        "diagnostics": {
            "globals": ["h5v", "bind", "bind_command", "bind_commands", "bind_script", "bind_lua", "unbind"]
        },
        "h5v": {
            "kind": LUA_LS_GENERATED_KIND,
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn lua_ls_config_contents() -> String {
    #[allow(clippy::expect_used)]
    let mut rendered =
        serde_json::to_string_pretty(&lua_ls_config_json()).expect("serialize LuaLS config");
    rendered.push('\n');
    rendered
}

fn should_refresh_lua_ls_config(existing: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<Value>(existing) else {
        return false;
    };

    if parsed
        .get("h5v")
        .and_then(Value::as_object)
        .is_some_and(|h5v| h5v.get("kind").and_then(Value::as_str) == Some(LUA_LS_GENERATED_KIND))
    {
        return true;
    }

    parsed
        == json!({
            "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
            "workspace": {
                "library": [LUA_LS_LIBRARY_DIR]
            },
            "diagnostics": {
                "globals": ["h5v", "bind", "bind_command", "bind_commands", "bind_script", "bind_lua", "unbind"]
            }
        })
        || parsed
            == json!({
                "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
                "workspace": {
                    "library": [LUA_LS_LIBRARY_DIR]
                },
                "diagnostics": {
                    "globals": ["h5v"]
                }
            })
        || parsed
            == json!({
                "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
                "workspace": {
                    "library": [LUA_LS_LIBRARY_DIR]
                },
                "diagnostics": {
                    "globals": ["h5v", "h5v_modes", "h5v_actions"]
                }
            })
        || parsed
            == json!({
                "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
                "workspace": {
                    "library": [LUA_LS_LIBRARY_DIR]
                },
                "diagnostics": {
                    "globals": ["h5v", "bind", "bind_command", "unbind"]
                }
            })
}

fn append_grouped_color_examples(lines: &mut Vec<String>) {
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (name, color) in configure::theme_named_colors(ThemeName::Dark) {
        let (group, key) = name.split_once('.').unwrap_or(("", name));
        let value = configure::color_to_lua_string(color);

        if let Some((_, entries)) = groups.iter_mut().find(|(existing, _)| existing == group) {
            entries.push((key.to_string(), value));
        } else {
            groups.push((group.to_string(), vec![(key.to_string(), value)]));
        }
    }

    for (group, entries) in groups {
        lines.push(format!("--   {group} = {{"));
        for (key, value) in entries {
            lines.push(format!("--     {key} = \"{value}\","));
        }
        lines.push("--   },".to_string());
    }
}

fn append_grouped_symbol_examples(lines: &mut Vec<String>) {
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (name, value) in configure::theme_named_symbols(SymbolThemeName::Rich) {
        let (group, key) = name.split_once('.').unwrap_or(("", name));

        if let Some((_, entries)) = groups.iter_mut().find(|(existing, _)| existing == group) {
            entries.push((key.to_string(), value.to_string()));
        } else {
            groups.push((
                group.to_string(),
                vec![(key.to_string(), value.to_string())],
            ));
        }
    }

    for (group, entries) in groups {
        lines.push(format!("--   {group} = {{"));
        for (key, value) in entries {
            lines.push(format!("--     {key} = \"{value}\","));
        }
        lines.push("--   },".to_string());
    }
}

fn write_default_config(config_path: &PathBuf) -> Result<(), ConfigureErrors> {
    let parent_dir = config_path.parent().ok_or({
        ConfigureErrors::FailureCreateDefault(std::io::Error::other(
            "Failed to get parent directory of config path",
        ))
    })?;
    std::fs::create_dir_all(parent_dir).map_err(ConfigureErrors::FailureCreateDefault)?;
    std::fs::write(config_path, default_config_contents())
        .map_err(ConfigureErrors::FailureCreateDefault)?;
    ensure_lua_ls_support_files(config_path)
}

fn ensure_lua_ls_support_files(config_path: &Path) -> Result<(), ConfigureErrors> {
    let parent_dir = config_path.parent().ok_or({
        ConfigureErrors::FailureCreateDefault(std::io::Error::other(
            "Failed to get parent directory of config path",
        ))
    })?;
    let lua_ls_dir = parent_dir.join(".h5v-luals");
    std::fs::create_dir_all(&lua_ls_dir).map_err(ConfigureErrors::FailureCreateDefault)?;
    std::fs::write(lua_ls_dir.join(LUA_LS_STUB_FILE), lua_ls_stub_contents())
        .map_err(ConfigureErrors::FailureCreateDefault)?;

    let lua_rc_path = parent_dir.join(LUA_LS_CONFIG_FILE);
    let should_write_config = if !lua_rc_path.exists() {
        true
    } else {
        let existing =
            std::fs::read_to_string(&lua_rc_path).map_err(ConfigureErrors::FailureCreateDefault)?;
        should_refresh_lua_ls_config(&existing)
    };
    if should_write_config {
        std::fs::write(lua_rc_path, lua_ls_config_contents())
            .map_err(ConfigureErrors::FailureCreateDefault)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        default_config_contents, lua_ls_config_contents, lua_ls_stub_contents,
        should_refresh_lua_ls_config,
    };

    #[test]
    fn reset_scaffold_groups_each_category_once() {
        let config = default_config_contents();

        assert_eq!(config.matches("--   text = {").count(), 1);
        assert_eq!(config.matches("--   content = {").count(), 1);
        assert_eq!(config.matches("--   command = {").count(), 1);
        assert_eq!(config.matches("--   help = {").count(), 1);
        assert_eq!(config.matches("--   metadata = {").count(), 1);
        assert_eq!(config.matches("--   file = {").count(), 1);
        assert_eq!(config.matches("--   mchart = {").count(), 1);
        assert_eq!(config.matches("--   surface = {").count(), 1);
        assert_eq!(config.matches("--   accent = {").count(), 1);
        assert_eq!(config.matches("--   tree = {").count(), 2);
        assert_eq!(config.matches("--   chart = {").count(), 2);
        assert_eq!(config.matches("--   status = {").count(), 1);
        assert_eq!(config.matches("--   toast = {").count(), 1);
        assert_eq!(config.matches("--   section = {").count(), 1);
        assert_eq!(config.matches("--   title = {").count(), 1);
        assert_eq!(config.matches("--   badge = {").count(), 1);
    }

    #[test]
    fn scaffold_points_to_external_lua_ls_support() {
        let config = default_config_contents();
        assert!(config.contains(".h5v-luals"));
        assert!(!config.contains("---@class H5vConfig"));
    }

    #[test]
    fn lua_ls_stub_contains_h5v_config_types() {
        let stub = lua_ls_stub_contents();
        assert!(stub.contains("---@class H5vConfig"));
        assert!(stub.contains("---@field content_mode_order H5vContentMode[]"));
        assert!(stub.contains("h5v = h5v"));
    }

    #[test]
    fn lua_ls_stub_attaches_mode_fields_to_h5v_modes() {
        let stub = lua_ls_stub_contents();
        let modes_index = stub.find("---@class H5vModes").expect("H5vModes class");
        let global_index = stub
            .find("---@field Global \"global\"")
            .expect("Global mode field");
        let color_index = stub
            .find("---@class H5vColorOverrides")
            .expect("H5vColorOverrides class");

        assert!(modes_index < global_index);
        assert!(global_index < color_index);
    }

    #[test]
    fn lua_ls_config_references_support_library() {
        let config = lua_ls_config_contents();
        assert!(config.contains("\"workspace\""));
        assert!(config.contains(".h5v-luals"));
        assert!(config.contains("\"version\": \"0.1.0\""));
    }

    #[test]
    fn refreshes_existing_generated_lua_ls_config() {
        let old_generated = r#"{
  "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
  "workspace": {
    "library": [
      ".h5v-luals"
    ]
  },
  "diagnostics": {
    "globals": [
      "h5v"
    ]
  }
}
"#;
        assert!(should_refresh_lua_ls_config(old_generated));
    }

    #[test]
    fn preserves_user_managed_lua_ls_config() {
        let user_config = r#"{
  "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
  "workspace": {
    "library": [
      ".h5v-luals",
      "lua"
    ]
  },
  "diagnostics": {
    "globals": [
      "h5v",
      "vim"
    ]
  }
}
"#;
        assert!(!should_refresh_lua_ls_config(user_config));
    }
}
