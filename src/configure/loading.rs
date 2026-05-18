use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Instant,
};

use crate::{
    configure::errors::ConfigureErrors,
    configure::{self, SymbolThemeName, ThemeName},
};
use serde_json::{json, Value};

mod pathing;
mod support_files;

const LUA_LS_LIBRARY_DIR: &str = ".h5v-luals";
const LUA_LS_STUB_FILE: &str = "h5v.lua";
const LUA_LS_CONFIG_FILE: &str = ".luarc.json";
const LUA_LS_GENERATED_KIND: &str = "generated-luals-config";
use pathing::config_parent_dir;
pub use pathing::{config_path, set_config_path_override};
use support_files::ensure_lua_ls_support_files;
pub use support_files::refresh_plugin_lua_ls_support_files;

pub fn load_or_create_config() -> Result<String, ConfigureErrors> {
    let config_path = ensure_config_exists()?;
    let read_started = Instant::now();

    let init_lua_content =
        std::fs::read_to_string(&config_path).map_err(ConfigureErrors::FailureToReadConfig)?;
    tracing::info!(
        kind = "config",
        phase = "read",
        config_path = %config_path.display(),
        duration_ms = read_started.elapsed().as_millis() as u64,
        bytes = init_lua_content.len(),
        message = "loaded config file"
    );

    Ok(init_lua_content)
}

pub fn ensure_config_exists() -> Result<PathBuf, ConfigureErrors> {
    let config_path = config_path()?;
    if !std::path::Path::new(&config_path).exists() {
        write_default_config(&config_path)?;
    } else {
        ensure_lua_ls_support_files(&config_path)?;
        tracing::info!(
            kind = "config",
            phase = "exists",
            config_path = %config_path.display(),
            message = "using existing config path"
        );
    }
    Ok(config_path)
}

pub fn reset_config_to_default() -> Result<PathBuf, ConfigureErrors> {
    let config_path = config_path()?;
    write_default_config(&config_path)?;
    tracing::info!(
        kind = "config",
        phase = "reset",
        config_path = %config_path.display(),
        message = "reset config to default"
    );
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
        "-- Main panels resize automatically based on focus.".to_string(),
        "-- Layout values accept exact cell counts (12), percentages (\"28%\"), or fill (\"*\").".to_string(),
        "-- h5v.layout = {".to_string(),
        "--   tree = { focused = \"28%\", unfocused = \"20%\" },".to_string(),
        "--   attributes = { focused = 12, unfocused = 5 },".to_string(),
        "--   content = { focused = \"*\", unfocused = \"*\" },".to_string(),
        "-- }".to_string(),
        "-- Multichart large-series tuning controls overview sampling and viewport refinement."
            .to_string(),
        "-- h5v.multichart = {".to_string(),
        "--   overview_max_samples = 4096,".to_string(),
        "--   detail_enabled = true,".to_string(),
        "--   detail_samples_per_column = 4,".to_string(),
        "--   detail_min_samples = 512,".to_string(),
        "--   detail_max_samples = 16384,".to_string(),
        "--   detail_padding_ratio = 0.2,".to_string(),
        "--   derived_detail_enabled = true,".to_string(),
        "-- }".to_string(),
        "-- Keymaps are layered: heatmap/content/tree/attributes/mchart > normal > global."
            .to_string(),
        "-- Use h5v.keys.bind({ mode = h5v.ids.keymap_modes.global, key = \"ctrl+h\", target = h5v.actions.ShowHelp }) for keybindings."
            .to_string(),
        "-- Built-in command handles also live under h5v.ids.commands, for example h5v.ids.commands.reload."
            .to_string(),
        "-- Persistent logging is available through h5v.logs.info(\"message\") / warning / error."
            .to_string(),
        "-- Use h5v.toast.info(\"message\") when the message should also be shown in the UI."
            .to_string(),
        "-- Mode constants live under both h5v.modes and h5v.ids.keymap_modes; actions live under h5v.actions."
            .to_string(),
        "-- Scope tables still support clear_defaults, bind, unbind, and command-backed entries."
            .to_string(),
        "-- local refresh = h5v.commands.register({ id = \"analysis.refresh\", run = function(ctx) ctx.command(\"help reload\") end })"
            .to_string(),
        "-- h5v.keys.bind({ mode = h5v.ids.keymap_modes.global, key = \"ctrl+r\", target = refresh, description = \"Refresh analysis\" })"
            .to_string(),
        "-- h5v.plugins.use(\"owner/repo\")".to_string(),
        "-- h5v.plugins.use(\"./plugins/my-plugin\")"
            .to_string(),
        "-- h5v.keys.unbind({ mode = h5v.ids.keymap_modes.heatmap, key = \"v\" })".to_string(),
        "-- h5v.keys.bind({ mode = h5v.ids.keymap_modes.heatmap, key = \"ctrl+alt+r\", command = \"heatmap range use \\\"Clip 1-99%\\\"\" })"
            .to_string(),
        "-- h5v.keys.bind({ mode = h5v.ids.keymap_modes.global, key = \"ctrl+k\", commands = { \"down 2\", \"up 1\" } })".to_string(),
        "-- h5v.keys.bind({ mode = h5v.ids.keymap_modes.global, key = \"ctrl+l\", lua = function(ctx) ctx.command(\"help reload\") end })"
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

#[derive(Default)]
struct IdClassNode {
    handles: Vec<(String, String)>,
    children: Vec<(String, IdClassNode)>,
}

fn lua_id_symbol(name: &str) -> String {
    name.replace('-', "_")
}

fn insert_id_path(node: &mut IdClassNode, segments: &[String], handle_type: &str) {
    let Some((last, parents)) = segments.split_last() else {
        return;
    };
    let mut current = node;
    for segment in parents {
        let symbol = lua_id_symbol(segment);
        if let Some(index) = current
            .children
            .iter()
            .position(|(name, _)| name == &symbol)
        {
            current = &mut current.children[index].1;
        } else {
            current
                .children
                .push((symbol.clone(), IdClassNode::default()));
            let index = current.children.len() - 1;
            current = &mut current.children[index].1;
        }
    }
    current
        .handles
        .push((lua_id_symbol(last), handle_type.to_string()));
}

fn emit_id_class(lines: &mut Vec<String>, class_name: &str, node: &IdClassNode) {
    lines.push(format!("---@class {class_name}"));
    for (field, child) in &node.children {
        let child_class = format!("{class_name}{}", pascal_case(field));
        lines.push(format!("---@field {} {}", field, child_class));
        emit_id_class(lines, &child_class, child);
    }
    for (field, handle_type) in &node.handles {
        lines.push(format!("---@field {} {}", field, handle_type));
    }
}

fn registry_color_name(metadata: &configure::registry::ColorMetadata) -> String {
    if metadata.group.is_empty() {
        metadata.name.clone()
    } else {
        format!("{}.{}", metadata.group, metadata.name)
    }
}

fn registry_symbol_name(metadata: &configure::registry::SymbolMetadata) -> String {
    if metadata.group.is_empty() {
        metadata.name.clone()
    } else {
        format!("{}.{}", metadata.group, metadata.name)
    }
}

fn grouped_registry_color_examples(
    registry: &configure::RegistrySnapshot,
) -> Vec<(String, Vec<(String, String)>)> {
    let sample_values = configure::theme_named_colors(ThemeName::Dark)
        .into_iter()
        .map(|(name, color)| (name.to_string(), configure::color_to_lua_string(color)))
        .collect::<BTreeMap<_, _>>();
    let mut groups = BTreeMap::<String, Vec<(String, String)>>::new();
    for metadata in registry.colors() {
        let name = registry_color_name(metadata);
        let (group, key) = name.split_once('.').unwrap_or(("", name.as_str()));
        let value = sample_values.get(&name).cloned().unwrap_or_default();
        groups
            .entry(group.to_string())
            .or_default()
            .push((key.to_string(), value));
    }
    groups.into_iter().collect()
}

fn grouped_registry_symbol_examples(
    registry: &configure::RegistrySnapshot,
) -> Vec<(String, Vec<(String, String)>)> {
    let sample_values = configure::theme_named_symbols(SymbolThemeName::Rich)
        .into_iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();
    let mut groups = BTreeMap::<String, Vec<(String, String)>>::new();
    for metadata in registry.symbols() {
        let name = registry_symbol_name(metadata);
        let (group, key) = name.split_once('.').unwrap_or(("", name.as_str()));
        let value = sample_values.get(&name).cloned().unwrap_or_default();
        groups
            .entry(group.to_string())
            .or_default()
            .push((key.to_string(), value));
    }
    groups.into_iter().collect()
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

fn lua_ls_stub_contents() -> Result<String, ConfigureErrors> {
    let registry = configure::builtin_registry_snapshot().map_err(|error| {
        ConfigureErrors::FailureCreateDefault(std::io::Error::other(error.to_string()))
    })?;
    let color_groups = grouped_registry_color_examples(&registry);
    let symbol_groups = grouped_registry_symbol_examples(&registry);
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
    let theme_names = registry
        .themes()
        .filter_map(|metadata| metadata.handle.as_str().strip_prefix("builtin.theme."))
        .collect::<Vec<_>>();
    let mut command_id_tree = IdClassNode::default();
    for metadata in registry.commands() {
        if let Some(suffix) = metadata.handle.as_str().strip_prefix("builtin.command.") {
            insert_id_path(
                &mut command_id_tree,
                &[suffix.to_string()],
                "H5vCommandHandle",
            );
        }
    }
    let mut setting_id_tree = IdClassNode::default();
    for metadata in registry.settings() {
        if let Some(suffix) = metadata.handle.as_str().strip_prefix("builtin.setting.") {
            insert_id_path(
                &mut setting_id_tree,
                &suffix
                    .split('.')
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                "H5vSettingHandle",
            );
        }
    }
    let mut theme_id_tree = IdClassNode::default();
    for metadata in registry.themes() {
        if let Some(suffix) = metadata.handle.as_str().strip_prefix("builtin.theme.") {
            insert_id_path(&mut theme_id_tree, &[suffix.to_string()], "H5vThemeHandle");
        }
    }
    let mut symbol_theme_id_tree = IdClassNode::default();
    for theme_name in configure::available_symbol_theme_names() {
        insert_id_path(
            &mut symbol_theme_id_tree,
            &[(*theme_name).to_string()],
            "H5vSymbolThemeHandle",
        );
    }
    let mut content_mode_id_tree = IdClassNode::default();
    for metadata in registry.content_modes() {
        if let Some(suffix) = metadata
            .handle
            .as_str()
            .strip_prefix("builtin.content_mode.")
        {
            insert_id_path(
                &mut content_mode_id_tree,
                &[suffix.to_string()],
                "H5vContentModeHandle",
            );
        }
    }
    let mut event_id_tree = IdClassNode::default();
    for metadata in registry.events() {
        if let Some(suffix) = metadata.handle.as_str().strip_prefix("builtin.event.") {
            insert_id_path(&mut event_id_tree, &[suffix.to_string()], "H5vEventHandle");
        }
    }
    let mut color_id_tree = IdClassNode::default();
    for metadata in registry.colors() {
        insert_id_path(
            &mut color_id_tree,
            &registry_color_name(metadata)
                .split('.')
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            "H5vColorHandle",
        );
    }
    let mut symbol_id_tree = IdClassNode::default();
    for metadata in registry.symbols() {
        insert_id_path(
            &mut symbol_id_tree,
            &registry_symbol_name(metadata)
                .split('.')
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            "H5vSymbolHandle",
        );
    }

    let mut lines = vec![
        "---@meta".to_string(),
        "---@diagnostic disable: undefined-global".to_string(),
        format!("---@alias H5vThemeName {}", lua_string_union(theme_names.iter().copied())),
        "---@alias H5vSymbolThemeName \"rich\"|\"compatibility\"".to_string(),
        "---@alias H5vContentMode \"preview\"|\"matrix\"|\"heatmap\"".to_string(),
        format!("---@alias H5vModeCode {}", lua_string_union(mode_codes.iter().copied())),
        format!(
            "---@alias H5vActionCode {}",
            lua_string_union(action_codes.iter().copied())
        ),
        "---@alias H5vHeatmapColormap \"turbo\"|\"grayscale\"|\"inferno\"".to_string(),
        "---@alias H5vHeatmapNormalization \"linear\"|\"log\"|\"sqrt\"".to_string(),
        "---@class H5vToastContext".to_string(),
        "---@field info fun(message: string)".to_string(),
        "---@field warning fun(message: string)".to_string(),
        "---@field warn fun(message: string)".to_string(),
        "---@field error fun(message: string)".to_string(),
        "---@class H5vLogContext".to_string(),
        "---@field info fun(message: string)".to_string(),
        "---@field warning fun(message: string)".to_string(),
        "---@field warn fun(message: string)".to_string(),
        "---@field error fun(message: string)".to_string(),
        "---@class H5vProcessSpec".to_string(),
        "---@field command string[]".to_string(),
        "---@field cwd? string".to_string(),
        "---@field stdin? string|(string|number)[]".to_string(),
        "---@class H5vProcessResult".to_string(),
        "---@field success boolean".to_string(),
        "---@field status? integer".to_string(),
        "---@field stdout? string".to_string(),
        "---@field stderr? string".to_string(),
        "---@field pid? integer".to_string(),
        "---@class H5vProcessContext".to_string(),
        "---@field run fun(spec: H5vProcessSpec): H5vProcessResult".to_string(),
        "---@field spawn fun(spec: H5vProcessSpec): H5vProcessResult".to_string(),
        "---@field parse_json fun(result: H5vProcessResult|string): any".to_string(),
        "---@class H5vMchartProcessContext: H5vProcessContext".to_string(),
        "---@field parse_scalar fun(result: H5vProcessResult|string): number".to_string(),
        "---@field parse_series fun(result: H5vProcessResult|string): number[]".to_string(),
        "---@class H5vAppContext".to_string(),
        "---@field readonly boolean".to_string(),
        "---@field mode string".to_string(),
        "---@field content_mode H5vContentModeHandle".to_string(),
        "---@class H5vFsContext".to_string(),
        "---@field file_path string".to_string(),
        "---@field config_path? string".to_string(),
        "---@field cwd? string".to_string(),
        "---@class H5vSelectionContext".to_string(),
        "---@field path? string".to_string(),
        "---@field content_mode H5vContentModeHandle".to_string(),
        "---@class H5vConfigContext".to_string(),
        "---@field theme H5vThemeName".to_string(),
        "---@field symbol_theme H5vSymbolThemeName".to_string(),
        "---@field compatibility boolean".to_string(),
        "---@field content_mode_order H5vContentModeHandle[]".to_string(),
        "---@class H5vContentContext".to_string(),
        "---@field open fun(mode: H5vContentModeHandle|H5vContentMode)".to_string(),
        "---@field toggle fun()".to_string(),
        "---@class H5vMchartContext".to_string(),
        "---@field open fun()".to_string(),
        "---@field close fun()".to_string(),
        "---@field toggle fun()".to_string(),
        "---@class H5vPluginStoreContext".to_string(),
        "---@field get fun(key: string): any".to_string(),
        "---@field set fun(key: string, value: any)".to_string(),
        "---@field delete fun(key: string)".to_string(),
        "---@class H5vPluginContext".to_string(),
        "---@field store H5vPluginStoreContext".to_string(),
        "---@class H5vPluginUseOptions".to_string(),
        "---@field auto_pull? boolean".to_string(),
        "---@class H5vEvents".to_string(),
        "---@field on fun(event: H5vEventHandle, callback: fun(ctx: H5vKeymapLuaContext, ev: table)): string".to_string(),
        "---@class H5vKeymapLuaContext".to_string(),
        "---@field command fun(command: string)".to_string(),
        "---@field commands fun(commands: string[])".to_string(),
        "---@field script fun(script: string)".to_string(),
        "---@field log H5vLogContext".to_string(),
        "---@field toast H5vToastContext".to_string(),
        "---@field process H5vProcessContext".to_string(),
        "---@field app H5vAppContext".to_string(),
        "---@field config H5vConfigContext".to_string(),
        "---@field fs H5vFsContext".to_string(),
        "---@field selection H5vSelectionContext".to_string(),
        "---@field content H5vContentContext".to_string(),
        "---@field mchart H5vMchartContext".to_string(),
        "---@field plugin H5vPluginContext".to_string(),
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
        "---@alias H5vLayoutSize integer|string".to_string(),
        "---@class H5vLayoutPanelConfig".to_string(),
        "---@field focused H5vLayoutSize".to_string(),
        "---@field unfocused H5vLayoutSize".to_string(),
        "---@class H5vLayoutConfig".to_string(),
        "---@field tree H5vLayoutPanelConfig".to_string(),
        "---@field attributes H5vLayoutPanelConfig".to_string(),
        "---@field content H5vLayoutPanelConfig".to_string(),
        "---@class H5vMultiChartConfig".to_string(),
        "---@field overview_max_samples integer".to_string(),
        "---@field detail_enabled boolean".to_string(),
        "---@field detail_samples_per_column integer".to_string(),
        "---@field detail_min_samples integer".to_string(),
        "---@field detail_max_samples integer".to_string(),
        "---@field detail_padding_ratio number".to_string(),
        "---@field derived_detail_enabled boolean".to_string(),
        "---@field functions H5vMchartFunctions".to_string(),
        "---@class H5vMchartPoint".to_string(),
        "---@field x number".to_string(),
        "---@field y number".to_string(),
        "---@class H5vMchartSeriesArgument".to_string(),
        "---@field len integer".to_string(),
        "---@field to_array fun(): number[]".to_string(),
        "---@field points fun(): H5vMchartPoint[]".to_string(),
        "---@field iter fun(): fun(): number|nil".to_string(),
        "---@field to_lines fun(): string".to_string(),
        "---@class H5vMchartFunctionParam".to_string(),
        "---@field name string".to_string(),
        "---@field kind H5vValueKindId".to_string(),
        "---@field detail? string".to_string(),
        "---@class H5vMchartFunctionDefinition".to_string(),
        "---@field id string".to_string(),
        "---@field name? string".to_string(),
        "---@field summary? string".to_string(),
        "---@field category? \"reducer\"|\"math\"|\"transform\"".to_string(),
        "---@field params? H5vMchartFunctionParam[]".to_string(),
        "---@field returns H5vValueKindId".to_string(),
        "---@field example? string".to_string(),
        "---@field completion_insert? string".to_string(),
        "---@field top_level_only? boolean".to_string(),
        "---@field first_arg_direct_item_ref_only? boolean".to_string(),
        "---@field eval fun(...: any): number|number[]".to_string(),
        "---@alias H5vCommandArgKind \"word\"|\"uint\"".to_string(),
        "---@class H5vCommandArgSpec".to_string(),
        "---@field name string".to_string(),
        "---@field kind H5vCommandArgKind".to_string(),
        "---@field required? boolean".to_string(),
        "---@field help? string".to_string(),
        "---@field values? string[]".to_string(),
        "---@class H5vCommandDefinition".to_string(),
        "---@field id string".to_string(),
        "---@field title? string".to_string(),
        "---@field summary? string".to_string(),
        "---@field category? string".to_string(),
        "---@field aliases? string[]".to_string(),
        "---@field args? H5vCommandArgSpec[]".to_string(),
        "---@field examples? string[]".to_string(),
        "---@field visible? boolean".to_string(),
        "---@field run fun(ctx: H5vKeymapLuaContext)".to_string(),
        "---@class H5vContentModeRenderFileContext".to_string(),
        "---@field path string".to_string(),
        "---@field selected_path? string".to_string(),
        "---@class H5vContentModeRenderState".to_string(),
        "---@class H5vContentModeItemContext".to_string(),
        "---@field path string".to_string(),
        "---@field kind \"file\"|\"group\"|\"dataset\"|\"broken\"".to_string(),
        "---@field attribute_names string[]".to_string(),
        "---@field has_attribute fun(name: string): boolean".to_string(),
        "---@class H5vContentUiBlockOptions".to_string(),
        "---@field title? string".to_string(),
        "---@class H5vContentUiSeparatorOptions".to_string(),
        "---@field label? string".to_string(),
        "---@field empty? boolean".to_string(),
        "---@field height? integer".to_string(),
        "---@alias H5vContentUiSplitDirection \"horizontal\"|\"vertical\"".to_string(),
        "---@class H5vContentUiSplitOptions".to_string(),
        "---@field direction? H5vContentUiSplitDirection".to_string(),
        "---@field ratio? number".to_string(),
        "---@field gap? integer".to_string(),
        "---@class H5vUiDocument".to_string(),
        "---@class H5vUiDocumentBuilder".to_string(),
        "---@field build fun(render: fun(ui: H5vContentUi)): H5vUiDocument".to_string(),
        "---@class H5vContentUi".to_string(),
        "---@field text fun(text: string)".to_string(),
        "---@field code fun(body: string, kind?: string)".to_string(),
        "---@field badge fun(text: string)".to_string(),
        "---@field kv fun(key: string, value: any)".to_string(),
        "---@field separator fun(options?: H5vContentUiSeparatorOptions|string)".to_string(),
        "---@field row fun(render: fun(ui: H5vContentUi))".to_string(),
        "---@field column fun(render: fun(ui: H5vContentUi))".to_string(),
        "---@field split fun(options: H5vContentUiSplitOptions|H5vContentUiSplitDirection|nil, left: fun(ui: H5vContentUi), right: fun(ui: H5vContentUi))".to_string(),
        "---@field table fun(rows: any[][])".to_string(),
        "---@field block fun(options: H5vContentUiBlockOptions|string|nil, render: fun(ui: H5vContentUi))".to_string(),
        "---@class H5vContentModeRenderContext".to_string(),
        "---@field app H5vAppContext".to_string(),
        "---@field config H5vConfigContext".to_string(),
        "---@field fs H5vFsContext".to_string(),
        "---@field selection H5vSelectionContext".to_string(),
        "---@field plugin H5vPluginContext".to_string(),
        "---@field file H5vContentModeRenderFileContext".to_string(),
        "---@field state H5vContentModeRenderState".to_string(),
        "---@class H5vContentModeDefinition".to_string(),
        "---@field id string".to_string(),
        "---@field title? string".to_string(),
        "---@field summary? string".to_string(),
        "---@field predicate? fun(item: H5vContentModeItemContext): boolean".to_string(),
        "---@field render fun(ctx: H5vContentModeRenderContext, ui: H5vContentUi)".to_string(),
        "---@alias H5vCommandHandle string".to_string(),
        "---@alias H5vSettingHandle string".to_string(),
        "---@alias H5vThemeHandle string".to_string(),
        "---@alias H5vSymbolThemeHandle string".to_string(),
        "---@alias H5vContentModeHandle string".to_string(),
        "---@alias H5vEventHandle string".to_string(),
        "---@alias H5vPluginHandle string".to_string(),
        "---@alias H5vColorHandle string".to_string(),
        "---@alias H5vSymbolHandle string".to_string(),
        "---@alias H5vValueKindId string".to_string(),
        "---@class H5vCommands".to_string(),
        "---@field register fun(definition: H5vCommandDefinition): H5vCommandHandle".to_string(),
        "---@class H5vMchartFunctions".to_string(),
        "---@field register fun(definition: H5vMchartFunctionDefinition): string".to_string(),
        "---@field process H5vMchartProcessContext".to_string(),
        "---@class H5vUiContentModes".to_string(),
        "---@field register fun(definition: H5vContentModeDefinition): H5vContentModeHandle".to_string(),
        "---@field add fun(definition: { mode: H5vContentModeHandle|string, path: string })".to_string(),
        "---@class H5vUi".to_string(),
        "---@field content_modes H5vUiContentModes".to_string(),
        "---@class H5vPlugins".to_string(),
        "---@field use fun(source: string, options?: H5vPluginUseOptions): H5vPluginHandle"
            .to_string(),
        "---@alias H5vKeyBindTarget H5vActionCode|H5vCommandHandle|string".to_string(),
        "---@class H5vKeymapBinding".to_string(),
        "---@field key string".to_string(),
        "---@field action? H5vActionCode".to_string(),
        "---@field command? string".to_string(),
        "---@field commands? string[]".to_string(),
        "---@field script? string".to_string(),
        "---@field lua? fun(ctx: H5vKeymapLuaContext)".to_string(),
        "---@field description? string".to_string(),
        "---@class H5vKeyBindingDefinition".to_string(),
        "---@field mode H5vModeCode".to_string(),
        "---@field key string".to_string(),
        "---@field target? H5vKeyBindTarget".to_string(),
        "---@field action? H5vActionCode".to_string(),
        "---@field command? string".to_string(),
        "---@field commands? string[]".to_string(),
        "---@field script? string".to_string(),
        "---@field lua? fun(ctx: H5vKeymapLuaContext)".to_string(),
        "---@field description? string".to_string(),
        "---@class H5vKeyUnbindDefinition".to_string(),
        "---@field mode H5vModeCode".to_string(),
        "---@field key string".to_string(),
        "---@class H5vKeymapScope".to_string(),
        "---@field clear_defaults? boolean".to_string(),
        "---@field unbind? string[]".to_string(),
        "---@field bind? H5vKeymapBinding[]".to_string(),
        "---@class H5vKeys".to_string(),
        "---@field bind fun(binding: H5vKeyBindingDefinition)"
            .to_string(),
        "---@field unbind fun(binding: H5vKeyUnbindDefinition)"
            .to_string(),
        "---@class H5vKeymaps".to_string(),
        "---@field bind fun(binding: H5vKeyBindingDefinition)"
            .to_string(),
        "---@field unbind fun(binding: H5vKeyUnbindDefinition)".to_string(),
        "---@field global? H5vKeymapScope".to_string(),
        "---@field normal? H5vKeymapScope".to_string(),
        "---@field window? H5vKeymapScope".to_string(),
        "---@field tree? H5vKeymapScope".to_string(),
        "---@field content? H5vKeymapScope".to_string(),
        "---@field heatmap? H5vKeymapScope".to_string(),
        "---@field attributes? H5vKeymapScope".to_string(),
        "---@field mchart? H5vKeymapScope".to_string(),
        "---@class H5vKeymapModeIds".to_string(),
        "---@field global \"global\"".to_string(),
        "---@field normal \"normal\"".to_string(),
        "---@field window \"window\"".to_string(),
        "---@field tree \"tree\"".to_string(),
        "---@field content \"content\"".to_string(),
        "---@field heatmap \"heatmap\"".to_string(),
        "---@field attributes \"attributes\"".to_string(),
        "---@field mchart \"mchart\"".to_string(),
        "---@class H5vValueKindIds".to_string(),
        "---@field scalar H5vValueKindId".to_string(),
        "---@field series H5vValueKindId".to_string(),
        "---@field unknown H5vValueKindId".to_string(),
        "---@class H5vComponentIds".to_string(),
        "---@field tree \"tree\"".to_string(),
        "---@field attributes \"attributes\"".to_string(),
        "---@field content \"content\"".to_string(),
        "---@field help \"help\"".to_string(),
        "---@field heatmap \"heatmap\"".to_string(),
        "---@field mchart \"mchart\"".to_string(),
        "---@field preview \"preview\"".to_string(),
        "---@field matrix \"matrix\"".to_string(),
        "---@field command \"command\"".to_string(),
        "---@field status \"status\"".to_string(),
        "---@class H5vHealthIds".to_string(),
        "---@field healthy H5vHealthStatusId".to_string(),
        "---@field warning H5vHealthStatusId".to_string(),
        "---@field fail H5vHealthStatusId".to_string(),
        "---@class H5vIds".to_string(),
        "---@field commands H5vCommandIds".to_string(),
        "---@field settings H5vSettingIds".to_string(),
        "---@field themes H5vThemeIds".to_string(),
        "---@field components H5vComponentIds".to_string(),
        "---@field symbol_themes H5vSymbolThemeIds".to_string(),
        "---@field content_modes H5vContentModeIds".to_string(),
        "---@field events H5vEventIds".to_string(),
        "---@field colors H5vColorIds".to_string(),
        "---@field symbols H5vSymbolIds".to_string(),
        "---@field health H5vHealthIds".to_string(),
        "---@field keymap_modes H5vKeymapModeIds".to_string(),
        "---@field value_kinds H5vValueKindIds".to_string(),
    ];
    emit_id_class(&mut lines, "H5vCommandIds", &command_id_tree);
    emit_id_class(&mut lines, "H5vSettingIds", &setting_id_tree);
    emit_id_class(&mut lines, "H5vThemeIds", &theme_id_tree);
    emit_id_class(&mut lines, "H5vSymbolThemeIds", &symbol_theme_id_tree);
    emit_id_class(&mut lines, "H5vContentModeIds", &content_mode_id_tree);
    emit_id_class(&mut lines, "H5vEventIds", &event_id_tree);
    emit_id_class(&mut lines, "H5vColorIds", &color_id_tree);
    emit_id_class(&mut lines, "H5vSymbolIds", &symbol_id_tree);
    lines.push("---@class H5vModes".to_string());
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
    for theme_name in &theme_names {
        lines.push(format!(
            "---@field {} H5vColorOverrides",
            lua_id_symbol(theme_name)
        ));
    }
    lines.push("---@class H5vSymbolThemeCatalog".to_string());
    for theme_name in configure::available_symbol_theme_names() {
        lines.push(format!(
            "---@field {} H5vSymbolOverrides",
            lua_id_symbol(theme_name)
        ));
    }
    lines.push("---@class H5vConfig".to_string());
    lines.push("---@field log fun(message: string)".to_string());
    lines.push("---@field logs H5vLogContext".to_string());
    lines.push("---@field toast H5vToastContext".to_string());
    lines.push("---@field compatibility boolean".to_string());
    lines.push("---@field content_mode_order H5vContentModeHandle[]".to_string());
    lines.push("---@field theme H5vThemeName".to_string());
    lines.push("---@field symbol_theme H5vSymbolThemeName".to_string());
    lines.push("---@field heatmap H5vHeatmapConfig".to_string());
    lines.push("---@field layout H5vLayoutConfig".to_string());
    lines.push("---@field multichart H5vMultiChartConfig".to_string());
    lines.push("---@field mchart H5vMultiChartConfig".to_string());
    lines.push("---@field ids H5vIds".to_string());
    lines.push("---@field commands H5vCommands".to_string());
    lines.push("---@field events H5vEvents".to_string());
    lines.push("---@field plugins H5vPlugins".to_string());
    lines.push("---@field ui H5vUi".to_string());
    lines.push("---@field keys H5vKeys".to_string());
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
    lines.push("---@alias H5vHealthStatusId \"healthy\"|\"warning\"|\"fail\"".to_string());
    lines.push("---@class H5vHealthcheckResult".to_string());
    lines.push("---@field status H5vHealthStatusId".to_string());
    lines.push("---@field message string|H5vUiDocument".to_string());
    lines.push("---@field summary? string".to_string());
    lines.push("---@field ui? H5vUiDocument".to_string());
    lines.push("---@class H5vPluginHealthcheckContext".to_string());
    lines.push("---@field process H5vProcessContext".to_string());
    lines.push("---@field health H5vHealthIds".to_string());
    lines.push("---@field ui H5vUiDocumentBuilder".to_string());
    lines.push("---@field log H5vLogContext".to_string());
    lines.push("---@field config H5vRuntimeConfigContext".to_string());
    lines.push("---@field fs H5vFsContext".to_string());
    lines.push("---@field plugin H5vPluginContext".to_string());
    lines.push("---@class H5vPluginInitContext".to_string());
    lines.push("---@field log H5vLogContext".to_string());
    lines.push("---@field toast H5vToastContext".to_string());
    lines.push("---@field config H5vRuntimeConfigContext".to_string());
    lines.push("---@field fs H5vFsContext".to_string());
    lines.push("---@field plugin H5vPluginContext".to_string());
    lines.push("---@class H5vPluginModule".to_string());
    lines.push(
        "---@field health fun(ctx: H5vPluginHealthcheckContext): H5vHealthcheckResult|H5vHealthStatusId"
            .to_string(),
    );
    lines.push("---@field init fun(h5v: H5vConfig, ctx: H5vPluginInitContext)".to_string());
    lines.push("---@type H5vConfig".to_string());
    lines.push("h5v = h5v".to_string());
    Ok(lines.join("\n"))
}

fn lua_ls_config_json() -> Value {
    json!({
        "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
        "workspace": {
            "library": [LUA_LS_LIBRARY_DIR]
        },
        "diagnostics": {
            "globals": ["h5v"]
        },
        "h5v": {
            "kind": LUA_LS_GENERATED_KIND,
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn lua_ls_plugin_config_json() -> Value {
    json!({
        "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
        "workspace": {
            "library": [LUA_LS_LIBRARY_DIR]
        },
        "h5v": {
            "kind": LUA_LS_GENERATED_KIND,
            "mode": "plugin",
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

fn lua_ls_plugin_config_contents() -> String {
    #[allow(clippy::expect_used)]
    let mut rendered = serde_json::to_string_pretty(&lua_ls_plugin_config_json())
        .expect("serialize plugin LuaLS config");
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
                "globals": ["h5v"]
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
    let write_started = Instant::now();
    let parent_dir = config_parent_dir(config_path);
    std::fs::create_dir_all(parent_dir).map_err(ConfigureErrors::FailureCreateDefault)?;
    std::fs::write(config_path, default_config_contents())
        .map_err(ConfigureErrors::FailureCreateDefault)?;
    ensure_lua_ls_support_files(config_path)?;
    tracing::info!(
        kind = "config",
        phase = "write_default",
        config_path = %config_path.display(),
        duration_ms = write_started.elapsed().as_millis() as u64,
        message = "wrote default config"
    );
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        default_config_contents, ensure_config_exists, load_or_create_config,
        lua_ls_config_contents, lua_ls_stub_contents, set_config_path_override,
        should_refresh_lua_ls_config,
    };
    use std::sync::MutexGuard;

    fn test_guard() -> MutexGuard<'static, ()> {
        crate::test_support::serial_test_guard()
    }

    #[test]
    fn reset_scaffold_groups_each_category_once() {
        let config = default_config_contents();

        assert_eq!(config.matches("--   text = {").count(), 1);
        assert_eq!(config.matches("--   command = {").count(), 1);
        assert_eq!(config.matches("--   help = {").count(), 1);
        assert_eq!(config.matches("--   metadata = {").count(), 1);
        assert_eq!(config.matches("--   file = {").count(), 1);
        assert_eq!(config.matches("--   mchart = {").count(), 1);
        assert_eq!(config.matches("--   surface = {").count(), 1);
        assert_eq!(config.matches("--   accent = {").count(), 1);
        assert_eq!(config.matches("--   tree = {").count(), 3);
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
        assert!(config.contains("-- h5v.layout = {"));
        assert!(!config.contains("---@class H5vConfig"));
    }

    #[test]
    fn lua_ls_stub_contains_h5v_config_types() {
        let stub = lua_ls_stub_contents().expect("generate stub");
        assert!(stub.contains("---@class H5vConfig"));
        assert!(stub.contains("---@class H5vLogContext"));
        assert!(stub.contains("---@class H5vToastContext"));
        assert!(stub.contains("---@class H5vProcessContext"));
        assert!(stub.contains("---@class H5vMchartProcessContext"));
        assert!(stub.contains("---@class H5vAppContext"));
        assert!(stub.contains("---@class H5vFsContext"));
        assert!(stub.contains("---@class H5vSelectionContext"));
        assert!(stub.contains("---@class H5vConfigContext"));
        assert!(stub.contains("---@class H5vContentContext"));
        assert!(stub.contains("---@class H5vMchartContext"));
        assert!(stub.contains("---@class H5vPluginContext"));
        assert!(stub.contains("---@class H5vPlugins"));
        assert!(stub.contains("---@class H5vUi"));
        assert!(stub.contains("---@class H5vMchartFunctions"));
        assert!(stub.contains("---@field functions H5vMchartFunctions"));
        assert!(stub.contains("---@field stdin? string|(string|number)[]"));
        assert!(stub.contains("---@field to_lines fun(): string"));
        assert!(stub.contains("---@field parse_json fun(result: H5vProcessResult|string): any"));
        assert!(stub.contains("---@class H5vUiDocumentBuilder"));
        assert!(stub.contains("---@field message string|H5vUiDocument"));
        assert!(stub.contains("---@field ui H5vUiDocumentBuilder"));
        assert!(stub.contains("---@class H5vContentUiSeparatorOptions"));
        assert!(stub.contains("---@class H5vContentUiSplitOptions"));
        assert!(
            stub.contains("---@field parse_scalar fun(result: H5vProcessResult|string): number")
        );
        assert!(
            stub.contains("---@field parse_series fun(result: H5vProcessResult|string): number[]")
        );
        assert!(stub
            .contains("---@field register fun(definition: H5vMchartFunctionDefinition): string"));
        assert!(stub.contains("---@class H5vComponentIds"));
        assert!(stub.contains("---@field components H5vComponentIds"));
        assert!(stub.contains("---@field app H5vAppContext"));
        assert!(stub.contains("---@field config H5vConfigContext"));
        assert!(stub.contains("---@field fs H5vFsContext"));
        assert!(stub.contains("---@field logs H5vLogContext"));
        assert!(stub.contains("---@field log H5vLogContext"));
        assert!(stub.contains("---@field selection H5vSelectionContext"));
        assert!(stub.contains("---@field content H5vContentContext"));
        assert!(stub.contains("---@field mchart H5vMchartContext"));
        assert!(stub.contains("---@field plugin H5vPluginContext"));
        assert!(stub.contains("---@field plugins H5vPlugins"));
        assert!(stub.contains("---@field ui H5vUi"));
        assert!(stub.contains("---@field content_mode_order H5vContentModeHandle[]"));
        assert!(stub.contains("---@field code fun(body: string, kind?: string)"));
        assert!(
            stub.contains("---@field separator fun(options?: H5vContentUiSeparatorOptions|string)")
        );
        assert!(stub.contains("---@field split fun(options: H5vContentUiSplitOptions|H5vContentUiSplitDirection|nil, left: fun(ui: H5vContentUi), right: fun(ui: H5vContentUi))"));
        assert!(stub.contains("---@field badge fun(text: string)"));
        assert!(stub.contains("h5v = h5v"));
    }

    #[test]
    fn lua_ls_stub_attaches_mode_fields_to_h5v_modes() {
        let stub = lua_ls_stub_contents().expect("generate stub");
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
    fn lua_ls_stub_contains_registry_id_namespaces() {
        let stub = lua_ls_stub_contents().expect("generate stub");
        assert!(stub.contains("---@field settings H5vSettingIds"));
        assert!(stub.contains("---@field theme H5vSettingHandle"));
        assert!(stub.contains("---@field heatmap H5vSettingIdsHeatmap"));
        assert!(stub.contains("---@field default_colormap H5vSettingHandle"));
        assert!(stub.contains("---@field themes H5vThemeIds"));
        assert!(stub.contains("---@field dark H5vThemeHandle"));
        assert!(stub.contains("---@field colors H5vColorIds"));
        assert!(stub.contains("---@field surface H5vColorIdsSurface"));
        assert!(stub.contains("---@field panel_border H5vColorHandle"));
        assert!(stub.contains("---@field symbols H5vSymbolIds"));
        assert!(stub.contains("---@field tree H5vSymbolIdsTree"));
        assert!(stub.contains("---@field root_file_icon H5vSymbolHandle"));
        assert!(stub.contains("---@field content_modes H5vContentModeIds"));
        assert!(stub.contains("---@field preview H5vContentModeHandle"));
        assert!(stub.contains("---@field events H5vEventIds"));
        assert!(stub.contains("---@field file_opened H5vEventHandle"));
        assert!(stub.contains("---@field value_kinds H5vValueKindIds"));
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

    #[test]
    fn custom_config_path_override_creates_missing_config_in_selected_location() {
        let _guard = test_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("my_dots/h5v/init.lua");

        set_config_path_override(Some(config_path.clone())).expect("set config path override");
        let created_path = ensure_config_exists().expect("create config");
        let loaded = load_or_create_config().expect("load config");

        assert_eq!(created_path, config_path);
        assert!(created_path.exists());
        assert!(loaded.contains("H5V Lua configuration file"));
        assert!(temp.path().join("my_dots/h5v/.h5v-luals/h5v.lua").exists());
        assert!(temp.path().join("my_dots/h5v/.luarc.json").exists());

        set_config_path_override(None).expect("clear config path override");
    }
}
