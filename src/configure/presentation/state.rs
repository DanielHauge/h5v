use std::sync::{LazyLock, RwLock};

use ratatui::prelude::Color;

use crate::compat;
use crate::configure::registry::{ContentModeHandle, RegistrySnapshot, SettingHandle};
use crate::ui::{
    input::keymap::{merge_keymap_config, EffectiveKeymaps, KeymapConfig},
    state::{
        ContentShowMode, HeatmapColormap, HeatmapNormalization, HeatmapRangeMode, HeatmapSettings,
    },
};

use super::{
    catalog::{available_color_names, available_symbol_names},
    palette::{SymbolThemeName, ThemeName},
    parsing::parse_color,
    types::{ConfigSnapshot, ConfigState, ThemeColors, UiSymbols},
};

const THEME_SETTING: &str = "builtin.setting.theme";
const SYMBOL_THEME_SETTING: &str = "builtin.setting.symbol_theme";
const CONTENT_MODE_ORDER_SETTING: &str = "builtin.setting.content_mode_order";
const HEATMAP_DEFAULT_RANGE_SETTING: &str = "builtin.setting.heatmap.default_range";
const HEATMAP_DEFAULT_COLORMAP_SETTING: &str = "builtin.setting.heatmap.default_colormap";
const HEATMAP_DEFAULT_NORMALIZATION_SETTING: &str = "builtin.setting.heatmap.default_normalization";
const HEATMAP_DEFAULT_INVERT_X_SETTING: &str = "builtin.setting.heatmap.default_invert_x";
const HEATMAP_DEFAULT_INVERT_Y_SETTING: &str = "builtin.setting.heatmap.default_invert_y";
const HEATMAP_DEFAULT_INVERT_C_SETTING: &str = "builtin.setting.heatmap.default_invert_c";
const MCHART_OVERVIEW_MAX_SAMPLES_SETTING: &str = "builtin.setting.multichart.overview_max_samples";
const MCHART_DETAIL_ENABLED_SETTING: &str = "builtin.setting.multichart.detail_enabled";
const MCHART_DETAIL_SAMPLES_PER_COLUMN_SETTING: &str =
    "builtin.setting.multichart.detail_samples_per_column";
const MCHART_DETAIL_MIN_SAMPLES_SETTING: &str = "builtin.setting.multichart.detail_min_samples";
const MCHART_DETAIL_MAX_SAMPLES_SETTING: &str = "builtin.setting.multichart.detail_max_samples";
const MCHART_DETAIL_PADDING_RATIO_SETTING: &str = "builtin.setting.multichart.detail_padding_ratio";
const MCHART_DERIVED_DETAIL_ENABLED_SETTING: &str =
    "builtin.setting.multichart.derived_detail_enabled";

fn default_symbol_theme() -> SymbolThemeName {
    if compat::current().compatibility_mode {
        SymbolThemeName::Compatibility
    } else {
        SymbolThemeName::Rich
    }
}

fn default_content_mode_order() -> Vec<ContentModeHandle> {
    vec![
        ContentShowMode::Preview.handle(),
        ContentShowMode::Matrix.handle(),
        ContentShowMode::Heatmap.handle(),
    ]
}

static CONFIG_STATE: LazyLock<RwLock<ConfigState>> = LazyLock::new(|| {
    let symbol_theme = default_symbol_theme();
    RwLock::new(ConfigState {
        config_generation: 0,
        active_theme_handle: "builtin.theme.dark".to_string(),
        active_theme_variant: ThemeName::Dark,
        active_symbol_theme: symbol_theme,
        colors: ThemeColors::for_theme(ThemeName::Dark),
        symbols: UiSymbols::for_theme(symbol_theme),
        content_mode_order: default_content_mode_order(),
        auto_layout: super::types::AutoLayoutSettings::default(),
        heatmap_range_modes: Vec::new(),
        heatmap_default_settings: HeatmapSettings::default(),
        multichart_settings: super::types::MultiChartSettings::default(),
        keymap_config: KeymapConfig::default(),
        keymaps: EffectiveKeymaps::default(),
    })
});

pub fn reset_config(theme: ThemeName) {
    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        let symbol_theme = default_symbol_theme();
        state.active_theme_handle = format!("builtin.theme.{}", theme.as_str());
        state.active_theme_variant = theme;
        state.active_symbol_theme = symbol_theme;
        state.colors = ThemeColors::for_theme(theme);
        state.symbols = UiSymbols::for_theme(symbol_theme);
        state.content_mode_order = default_content_mode_order();
        state.auto_layout = super::types::AutoLayoutSettings::default();
        state.heatmap_range_modes = Vec::new();
        state.heatmap_default_settings = HeatmapSettings::default();
        state.multichart_settings = super::types::MultiChartSettings::default();
        state.keymap_config = KeymapConfig::default();
        state.keymaps = EffectiveKeymaps::default();
    });
}

#[allow(dead_code)]
pub fn reset_symbol_theme(theme: SymbolThemeName) {
    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        state.active_symbol_theme = theme;
        state.symbols = UiSymbols::for_theme(theme);
    });
}

pub fn snapshot_config() -> ConfigSnapshot {
    with_config_read(|state| ConfigSnapshot {
        active_theme_handle: state.active_theme_handle.clone(),
        active_theme_variant: state.active_theme_variant,
        active_symbol_theme: state.active_symbol_theme,
        colors: state.colors.clone(),
        symbols: state.symbols.clone(),
        content_mode_order: state.content_mode_order.clone(),
        auto_layout: state.auto_layout.clone(),
        heatmap_range_modes: state.heatmap_range_modes.clone(),
        heatmap_default_settings: state.heatmap_default_settings.clone(),
        multichart_settings: state.multichart_settings.clone(),
        keymap_config: state.keymap_config.clone(),
        keymaps: state.keymaps.clone(),
    })
}

pub fn restore_config(snapshot: ConfigSnapshot) {
    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        state.active_theme_handle = snapshot.active_theme_handle;
        state.active_theme_variant = snapshot.active_theme_variant;
        state.active_symbol_theme = snapshot.active_symbol_theme;
        state.colors = snapshot.colors;
        state.symbols = snapshot.symbols;
        state.content_mode_order = snapshot.content_mode_order;
        state.auto_layout = snapshot.auto_layout;
        state.heatmap_range_modes = snapshot.heatmap_range_modes;
        state.heatmap_default_settings = snapshot.heatmap_default_settings;
        state.multichart_settings = snapshot.multichart_settings;
        state.keymap_config = snapshot.keymap_config;
        state.keymaps = snapshot.keymaps;
    });
}

pub fn apply_registry_snapshot(snapshot: &RegistrySnapshot) -> Result<(), String> {
    let theme = parse_registry_theme(snapshot)?;
    let symbol_theme = parse_registry_symbol_theme(snapshot)?;
    let content_mode_order = parse_registry_content_mode_order(snapshot)?;
    let mut colors = ThemeColors::for_theme(theme.variant);
    let mut symbols = UiSymbols::for_theme(symbol_theme);
    let heatmap_default_settings = parse_registry_heatmap_settings(snapshot)?;
    let multichart_settings = parse_registry_multichart_settings(snapshot)?;

    for (name, value) in &theme.color_overrides {
        let color = parse_color(value)
            .ok_or_else(|| format!("Invalid theme color '{value}' for '{name}'"))?;
        if !colors.set_named_color(name, color) {
            return Err(format!("Unknown theme color '{name}'"));
        }
    }

    for (name, value) in &theme.symbol_overrides {
        if !symbols.set_named_symbol(name, value) {
            return Err(format!("Unknown theme symbol '{name}'"));
        }
    }

    for metadata in snapshot.colors() {
        let Some(value) = metadata.override_value.as_deref() else {
            continue;
        };
        let name = registry_entry_name(&metadata.group, &metadata.name);
        let color = parse_color(value)
            .ok_or_else(|| format!("Invalid registry color '{value}' for '{name}'"))?;
        if !colors.set_named_color(&name, color) {
            return Err(format!("Unknown registry color '{name}'"));
        }
    }

    for metadata in snapshot.symbols() {
        let Some(value) = metadata.override_value.as_deref() else {
            continue;
        };
        let name = registry_entry_name(&metadata.group, &metadata.name);
        if !symbols.set_named_symbol(&name, value) {
            return Err(format!("Unknown registry symbol '{name}'"));
        }
    }

    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        state.active_theme_handle = theme.handle;
        state.active_theme_variant = theme.variant;
        state.active_symbol_theme = symbol_theme;
        state.colors = colors;
        state.symbols = symbols;
        state.content_mode_order = content_mode_order;
        state.auto_layout = super::types::AutoLayoutSettings::default();
        state.heatmap_range_modes = Vec::new();
        state.heatmap_default_settings = heatmap_default_settings;
        state.multichart_settings = multichart_settings;
        state.keymap_config = KeymapConfig::default();
        state.keymaps = EffectiveKeymaps::default();
    });
    Ok(())
}

#[allow(dead_code)]
pub fn set_color_override(name: &str, color: Color) -> Result<(), String> {
    with_config_write(|state| {
        if state.colors.set_named_color(name, color) {
            state.config_generation = state.config_generation.wrapping_add(1);
            Ok(())
        } else {
            Err(format!(
                "Unknown color '{name}'. Available colors: {}",
                available_color_names().join(", ")
            ))
        }
    })
}

#[allow(dead_code)]
pub fn set_symbol_override(name: &str, value: &str) -> Result<(), String> {
    with_config_write(|state| {
        if state.symbols.set_named_symbol(name, value) {
            state.config_generation = state.config_generation.wrapping_add(1);
            Ok(())
        } else {
            Err(format!(
                "Unknown symbol '{name}'. Available symbols: {}",
                available_symbol_names().join(", ")
            ))
        }
    })
}

pub fn current_theme_name() -> ThemeName {
    with_config_read(|state| state.active_theme_variant)
}

pub fn current_theme_handle() -> String {
    with_config_read(|state| state.active_theme_handle.clone())
}

pub fn current_symbol_theme_name() -> SymbolThemeName {
    with_config_read(|state| state.active_symbol_theme)
}

#[allow(dead_code)]
pub fn set_content_mode_order(order: &[ContentShowMode]) {
    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        state.content_mode_order = normalize_content_mode_order(
            &order
                .iter()
                .copied()
                .map(ContentShowMode::handle)
                .collect::<Vec<_>>(),
        );
    });
}

pub fn ordered_content_modes(available: &[ContentShowMode]) -> Vec<ContentShowMode> {
    ordered_content_mode_handles(
        &available
            .iter()
            .copied()
            .map(ContentShowMode::handle)
            .collect::<Vec<_>>(),
    )
    .into_iter()
    .filter_map(|handle| ContentShowMode::parse_handle(handle.as_str()))
    .collect()
}

pub fn ordered_content_mode_handles(available: &[ContentModeHandle]) -> Vec<ContentModeHandle> {
    with_config_read(|state| {
        let mut ordered = Vec::new();
        for preferred in &state.content_mode_order {
            if available.contains(preferred) && !ordered.contains(preferred) {
                ordered.push(preferred.clone());
            }
        }
        for handle in available {
            if !ordered.contains(handle) {
                ordered.push(handle.clone());
            }
        }
        ordered
    })
}

pub fn current_content_mode_order() -> Vec<ContentShowMode> {
    with_config_read(|state| {
        state
            .content_mode_order
            .iter()
            .filter_map(|handle| ContentShowMode::parse_handle(handle.as_str()))
            .collect()
    })
}

pub fn current_content_mode_order_handles() -> Vec<ContentModeHandle> {
    with_config_read(|state| state.content_mode_order.clone())
}

pub fn set_auto_layout_settings(settings: &super::types::AutoLayoutSettings) {
    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        state.auto_layout = settings.clone();
    });
}

pub fn current_auto_layout_settings() -> super::types::AutoLayoutSettings {
    with_config_read(|state| state.auto_layout.clone())
}

pub fn set_heatmap_ranges(range_modes: &[HeatmapRangeMode], default_range: &HeatmapRangeMode) {
    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        state.heatmap_range_modes = range_modes.to_vec();
        state.heatmap_default_settings.range = default_range.clone();
    });
}

pub fn current_heatmap_range_modes() -> Vec<HeatmapRangeMode> {
    with_config_read(|state| state.heatmap_range_modes.clone())
}

#[allow(dead_code)]
pub fn set_heatmap_default_settings(default_settings: &HeatmapSettings) {
    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        state.heatmap_default_settings = default_settings.clone();
    });
}

pub fn current_heatmap_default_settings() -> HeatmapSettings {
    with_config_read(|state| state.heatmap_default_settings.clone())
}

pub fn current_heatmap_default_range() -> HeatmapRangeMode {
    with_config_read(|state| state.heatmap_default_settings.range.clone())
}

#[allow(dead_code)]
pub fn set_multichart_settings(settings: &super::types::MultiChartSettings) {
    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        state.multichart_settings = settings.clone();
    });
}

pub fn current_multichart_settings() -> super::types::MultiChartSettings {
    with_config_read(|state| state.multichart_settings.clone())
}

pub fn set_keymap_config(keymap_config: &KeymapConfig) -> Result<(), String> {
    let keymaps = merge_keymap_config(keymap_config)?;
    with_config_write(|state| {
        state.config_generation = state.config_generation.wrapping_add(1);
        state.keymap_config = keymap_config.clone();
        state.keymaps = keymaps;
    });
    Ok(())
}

pub fn current_keymaps() -> EffectiveKeymaps {
    with_config_read(|state| state.keymaps.clone())
}

pub fn prefers_strong_text() -> bool {
    matches!(current_theme_name(), ThemeName::Light)
}

pub fn current_config_generation() -> u64 {
    with_config_read(|state| state.config_generation)
}

pub(crate) fn themed_color(getter: impl FnOnce(&ThemeColors) -> Color) -> Color {
    with_config_read(|state| getter(&state.colors))
}

pub(crate) fn configured_symbol(getter: impl FnOnce(&UiSymbols) -> &'static str) -> &'static str {
    with_config_read(|state| getter(&state.symbols))
}

fn normalize_content_mode_order(order: &[ContentModeHandle]) -> Vec<ContentModeHandle> {
    let mut normalized = Vec::new();
    for mode in order {
        if !normalized.contains(mode) {
            normalized.push(mode.clone());
        }
    }
    for mode in default_content_mode_order() {
        if !normalized.contains(&mode) {
            normalized.push(mode);
        }
    }
    normalized
}

fn parse_registry_theme(snapshot: &RegistrySnapshot) -> Result<ResolvedTheme, String> {
    let selected = registry_setting_value(snapshot, THEME_SETTING)
        .map(str::to_string)
        .or_else(|| {
            snapshot
                .themes()
                .find(|metadata| metadata.is_active)
                .map(|metadata| metadata.handle.to_string())
        })
        .unwrap_or_else(|| "builtin.theme.dark".to_string());
    let metadata = resolve_registry_theme(snapshot, &selected)
        .ok_or_else(|| format!("Unknown registry theme '{selected}'"))?;
    Ok(ResolvedTheme {
        handle: metadata.handle.to_string(),
        variant: resolve_theme_variant(metadata),
        color_overrides: metadata
            .color_overrides
            .iter()
            .map(|(handle, value)| -> Result<(String, String), String> {
                Ok((registry_color_name(snapshot, handle)?, value.clone()))
            })
            .collect::<Result<Vec<_>, String>>()?,
        symbol_overrides: metadata
            .symbol_overrides
            .iter()
            .map(|(handle, value)| -> Result<(String, String), String> {
                Ok((registry_symbol_name(snapshot, handle)?, value.clone()))
            })
            .collect::<Result<Vec<_>, String>>()?,
    })
}

struct ResolvedTheme {
    handle: String,
    variant: ThemeName,
    color_overrides: Vec<(String, String)>,
    symbol_overrides: Vec<(String, String)>,
}

fn resolve_registry_theme<'a>(
    snapshot: &'a RegistrySnapshot,
    value: &str,
) -> Option<&'a crate::configure::registry::ThemeMetadata> {
    snapshot
        .theme(&crate::configure::registry::ThemeHandle::new(value))
        .or_else(|| {
            snapshot.theme(&crate::configure::registry::ThemeHandle::new(format!(
                "builtin.theme.{value}"
            )))
        })
        .or_else(|| {
            snapshot
                .themes()
                .find(|metadata| metadata.handle.as_str().rsplit('.').next() == Some(value.trim()))
        })
}

fn resolve_theme_variant(metadata: &crate::configure::registry::ThemeMetadata) -> ThemeName {
    metadata
        .variant
        .as_deref()
        .and_then(ThemeName::parse)
        .unwrap_or(ThemeName::Dark)
}

fn registry_color_name(
    snapshot: &RegistrySnapshot,
    handle: &crate::configure::registry::ColorHandle,
) -> Result<String, String> {
    let metadata = snapshot
        .color(handle)
        .ok_or_else(|| format!("Unknown theme color handle '{}'", handle.as_str()))?;
    Ok(registry_entry_name(&metadata.group, &metadata.name))
}

fn registry_symbol_name(
    snapshot: &RegistrySnapshot,
    handle: &crate::configure::registry::SymbolHandle,
) -> Result<String, String> {
    let metadata = snapshot
        .symbol(handle)
        .ok_or_else(|| format!("Unknown theme symbol handle '{}'", handle.as_str()))?;
    Ok(registry_entry_name(&metadata.group, &metadata.name))
}

fn parse_registry_symbol_theme(snapshot: &RegistrySnapshot) -> Result<SymbolThemeName, String> {
    let symbol_theme_value = registry_setting_value(snapshot, SYMBOL_THEME_SETTING)
        .unwrap_or_else(|| default_symbol_theme().as_str());
    SymbolThemeName::parse(symbol_theme_value)
        .ok_or_else(|| format!("Unknown registry symbol theme '{symbol_theme_value}'"))
}

fn parse_registry_content_mode_order(
    snapshot: &RegistrySnapshot,
) -> Result<Vec<ContentModeHandle>, String> {
    let Some(value) = registry_setting_value(snapshot, CONTENT_MODE_ORDER_SETTING) else {
        return Ok(default_content_mode_order());
    };
    let mut order = Vec::new();
    for entry in value.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let handle = if let Some(mode) = ContentShowMode::parse_handle(trimmed) {
            mode.handle()
        } else {
            let handle = ContentModeHandle::new(trimmed);
            if snapshot.content_mode(&handle).is_none() {
                return Err(format!("Unknown registry content mode '{trimmed}'"));
            }
            handle
        };
        if !order.contains(&handle) {
            order.push(handle);
        }
    }
    if order.is_empty() {
        return Err(
            "Registry content mode order must include at least one content mode".to_string(),
        );
    }
    Ok(normalize_content_mode_order(&order))
}

fn parse_registry_heatmap_settings(snapshot: &RegistrySnapshot) -> Result<HeatmapSettings, String> {
    let mut settings = HeatmapSettings::default();
    if let Some(value) = registry_setting_value(snapshot, HEATMAP_DEFAULT_RANGE_SETTING) {
        if let Some(range) = HeatmapRangeMode::default_modes()
            .into_iter()
            .find(|mode| mode.selector_matches(value))
        {
            settings.range = range;
        }
    }
    if let Some(value) = registry_setting_value(snapshot, HEATMAP_DEFAULT_COLORMAP_SETTING) {
        settings.colormap = HeatmapColormap::parse(value)
            .ok_or_else(|| format!("Unknown registry heatmap colormap '{value}'"))?;
    }
    if let Some(value) = registry_setting_value(snapshot, HEATMAP_DEFAULT_NORMALIZATION_SETTING) {
        settings.normalization = HeatmapNormalization::parse(value)
            .ok_or_else(|| format!("Unknown registry heatmap normalization '{value}'"))?;
    }
    if let Some(value) = registry_setting_value(snapshot, HEATMAP_DEFAULT_INVERT_X_SETTING) {
        settings.invert_x = parse_registry_bool(value, "heatmap.default_invert_x")?;
    }
    if let Some(value) = registry_setting_value(snapshot, HEATMAP_DEFAULT_INVERT_Y_SETTING) {
        settings.invert_y = parse_registry_bool(value, "heatmap.default_invert_y")?;
    }
    if let Some(value) = registry_setting_value(snapshot, HEATMAP_DEFAULT_INVERT_C_SETTING) {
        settings.invert_c = parse_registry_bool(value, "heatmap.default_invert_c")?;
    }
    Ok(settings)
}

fn parse_registry_multichart_settings(
    snapshot: &RegistrySnapshot,
) -> Result<super::types::MultiChartSettings, String> {
    let mut settings = super::types::MultiChartSettings::default();
    if let Some(value) = registry_setting_value(snapshot, MCHART_OVERVIEW_MAX_SAMPLES_SETTING) {
        settings.overview_max_samples =
            parse_registry_usize(value, "multichart.overview_max_samples")?;
    }
    if let Some(value) = registry_setting_value(snapshot, MCHART_DETAIL_ENABLED_SETTING) {
        settings.detail_enabled = parse_registry_bool(value, "multichart.detail_enabled")?;
    }
    if let Some(value) = registry_setting_value(snapshot, MCHART_DETAIL_SAMPLES_PER_COLUMN_SETTING)
    {
        settings.detail_samples_per_column =
            parse_registry_usize(value, "multichart.detail_samples_per_column")?;
    }
    if let Some(value) = registry_setting_value(snapshot, MCHART_DETAIL_MIN_SAMPLES_SETTING) {
        settings.detail_min_samples = parse_registry_usize(value, "multichart.detail_min_samples")?;
    }
    if let Some(value) = registry_setting_value(snapshot, MCHART_DETAIL_MAX_SAMPLES_SETTING) {
        settings.detail_max_samples = parse_registry_usize(value, "multichart.detail_max_samples")?;
    }
    if let Some(value) = registry_setting_value(snapshot, MCHART_DETAIL_PADDING_RATIO_SETTING) {
        settings.detail_padding_ratio =
            parse_registry_f64(value, "multichart.detail_padding_ratio")?;
    }
    if let Some(value) = registry_setting_value(snapshot, MCHART_DERIVED_DETAIL_ENABLED_SETTING) {
        settings.derived_detail_enabled =
            parse_registry_bool(value, "multichart.derived_detail_enabled")?;
    }
    Ok(settings)
}

fn registry_setting_value<'a>(snapshot: &'a RegistrySnapshot, handle: &str) -> Option<&'a str> {
    snapshot
        .setting(&SettingHandle::new(handle))
        .and_then(|metadata| {
            metadata
                .current_value
                .as_deref()
                .or(metadata.default_value.as_deref())
        })
}

fn registry_entry_name(group: &str, name: &str) -> String {
    if group.is_empty() {
        name.to_string()
    } else {
        format!("{group}.{name}")
    }
}

fn parse_registry_bool(value: &str, field: &str) -> Result<bool, String> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!(
            "Registry setting '{field}' must be 'true' or 'false', got '{value}'"
        )),
    }
}

fn parse_registry_usize(value: &str, field: &str) -> Result<usize, String> {
    let parsed = value.trim().parse().map_err(|_| {
        format!("Registry setting '{field}' must be a positive integer, got '{value}'")
    })?;
    if parsed == 0 {
        return Err(format!(
            "Registry setting '{field}' must be a positive integer, got '{value}'"
        ));
    }
    Ok(parsed)
}

fn parse_registry_f64(value: &str, field: &str) -> Result<f64, String> {
    let parsed = value
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("Registry setting '{field}' must be a number, got '{value}'"))?;
    if !parsed.is_finite() {
        return Err(format!(
            "Registry setting '{field}' must be finite, got '{value}'"
        ));
    }
    Ok(parsed)
}

fn with_config_read<R>(f: impl FnOnce(&ConfigState) -> R) -> R {
    let guard = match CONFIG_STATE.read() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    f(&guard)
}

fn with_config_write<R>(f: impl FnOnce(&mut ConfigState) -> R) -> R {
    let mut guard = match CONFIG_STATE.write() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    f(&mut guard)
}
