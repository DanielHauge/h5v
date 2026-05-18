use mlua::{Table, Value};

use crate::{
    configure::registry::{ColorHandle, ContentModeHandle, SettingHandle, SymbolHandle},
    configure::{self, RegistryBuilder, SymbolThemeName},
    ui::state::ContentShowMode,
};

use super::super::errors::ConfigureErrors;
use super::bootstrap::default_symbol_theme_for_compatibility;
use super::commands::register_lua_commands;
use super::heatmap::parse_heatmap_config;
use super::keymaps::parse_keymaps_config;
use super::layout::parse_layout_config;
use super::mchart::{parse_multichart_config, register_lua_mchart_functions};
use super::plugins::register_lua_plugins;
use super::themes::{activate_theme, parse_selected_theme, register_lua_themes};
use super::ui::{register_lua_content_modes, resolve_registered_content_mode_handle};

pub(super) fn parse_compatibility_override(h5v: &Table) -> Result<Option<bool>, ConfigureErrors> {
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

pub(super) fn parse_content_mode_order(
    h5v: &Table,
) -> Result<Option<Vec<ContentModeHandle>>, ConfigureErrors> {
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
                let handle = if let Some(mode) = ContentShowMode::parse_handle(value.as_ref()) {
                    mode.handle()
                } else if let Some(handle) =
                    resolve_registered_content_mode_handle(h5v, value.as_ref())?
                {
                    handle
                } else {
                    return Err(mlua::Error::runtime(format!(
                        "Unknown content mode '{value}'. Use a builtin mode name/handle or a registered custom content mode"
                    ))
                    .into());
                };
                if !order.contains(&handle) {
                    order.push(handle);
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

pub(super) fn register_lua_config(
    builder: &mut RegistryBuilder,
    h5v: &Table,
) -> Result<(), ConfigureErrors> {
    let compatibility_override = parse_compatibility_override(h5v)?;
    let heatmap_config = parse_heatmap_config(h5v)?;
    let multichart_config = parse_multichart_config(h5v)?;

    register_lua_plugins(builder, h5v)?;
    register_lua_themes(builder, h5v)?;
    register_lua_content_modes(builder, h5v)?;
    register_lua_mchart_functions(builder, h5v)?;
    let selected_theme = parse_selected_theme(h5v)?;
    let selected_symbol_theme = parse_selected_symbol_theme(h5v, compatibility_override)?;
    let content_mode_order = parse_content_mode_order(h5v)?;

    set_registry_setting(
        builder,
        "builtin.setting.compatibility",
        compatibility_override.unwrap_or(false).to_string(),
    )?;
    set_registry_setting(
        builder,
        "builtin.setting.theme",
        selected_theme.as_str().to_string(),
    )?;
    set_registry_setting(
        builder,
        "builtin.setting.symbol_theme",
        selected_symbol_theme.as_str().to_string(),
    )?;
    if let Some(order) = content_mode_order {
        set_registry_setting(
            builder,
            "builtin.setting.content_mode_order",
            order
                .iter()
                .map(|mode| mode.as_str())
                .collect::<Vec<_>>()
                .join(","),
        )?;
    }

    activate_theme(builder, h5v, &selected_theme)?;

    if let Some((_, default_settings)) = heatmap_config {
        set_registry_setting(
            builder,
            "builtin.setting.heatmap.default_range",
            default_settings.range.label(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.heatmap.default_colormap",
            default_settings.colormap.as_str().to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.heatmap.default_normalization",
            default_settings.normalization.as_str().to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.heatmap.default_invert_x",
            default_settings.invert_x.to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.heatmap.default_invert_y",
            default_settings.invert_y.to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.heatmap.default_invert_c",
            default_settings.invert_c.to_string(),
        )?;
    }

    if let Some(settings) = multichart_config {
        set_registry_setting(
            builder,
            "builtin.setting.multichart.overview_max_samples",
            settings.overview_max_samples.to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.multichart.detail_enabled",
            settings.detail_enabled.to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.multichart.detail_samples_per_column",
            settings.detail_samples_per_column.to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.multichart.detail_min_samples",
            settings.detail_min_samples.to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.multichart.detail_max_samples",
            settings.detail_max_samples.to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.multichart.detail_padding_ratio",
            settings.detail_padding_ratio.to_string(),
        )?;
        set_registry_setting(
            builder,
            "builtin.setting.multichart.derived_detail_enabled",
            settings.derived_detail_enabled.to_string(),
        )?;
    }

    match h5v.get::<Value>("colors")? {
        Value::Nil => {}
        Value::Table(table) => register_color_overrides(builder, &table, None)?,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.colors must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    }

    match h5v.get::<Value>("symbols")? {
        Value::Nil => {}
        Value::Table(table) => register_symbol_overrides(builder, &table, None)?,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.symbols must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    }

    register_lua_commands(builder, h5v)?;

    Ok(())
}

#[cfg(test)]
pub(super) fn apply_lua_config(h5v: &Table) -> Result<(), ConfigureErrors> {
    let mut builder = configure::builtin_registry_builder()
        .map_err(|error| mlua::Error::runtime(error.to_string()))?;
    register_lua_config(&mut builder, h5v)?;
    let registry_snapshot = builder
        .freeze()
        .map_err(|error| mlua::Error::runtime(error.to_string()))?;
    apply_lua_config_with_snapshot(&registry_snapshot, h5v)
}

pub(super) fn apply_lua_config_with_snapshot(
    registry_snapshot: &configure::RegistrySnapshot,
    h5v: &Table,
) -> Result<(), ConfigureErrors> {
    configure::apply_registry_snapshot(registry_snapshot).map_err(mlua::Error::runtime)?;
    apply_non_registry_lua_config(h5v)
}

pub(super) fn apply_non_registry_lua_config(h5v: &Table) -> Result<(), ConfigureErrors> {
    let heatmap_config = parse_heatmap_config(h5v)?;
    let layout_config = parse_layout_config(h5v)?;
    let keymap_config = parse_keymaps_config(h5v)?;
    if let Some(layout_settings) = layout_config {
        configure::set_auto_layout_settings(&layout_settings);
    }
    if let Some((range_modes, default_settings)) = heatmap_config {
        configure::set_heatmap_ranges(&range_modes, &default_settings.range);
    }
    if let Some(keymap_config) = keymap_config {
        configure::set_keymap_config(&keymap_config).map_err(mlua::Error::runtime)?;
    }
    Ok(())
}

fn parse_selected_symbol_theme(
    h5v: &Table,
    compatibility_override: Option<bool>,
) -> Result<SymbolThemeName, ConfigureErrors> {
    match h5v.get::<Value>("symbol_theme")? {
        Value::Nil => Ok(compatibility_override
            .map(default_symbol_theme_for_compatibility)
            .unwrap_or_else(configure::current_symbol_theme_name)),
        Value::String(value) => {
            let value = value.to_str()?;
            SymbolThemeName::parse(value.as_ref())
                .ok_or_else(|| {
                    mlua::Error::runtime(format!(
                        "Unknown symbol theme '{value}'. Available symbol themes: {}",
                        configure::available_symbol_theme_names().join(", ")
                    ))
                })
                .map_err(Into::into)
        }
        other => Err(mlua::Error::runtime(format!(
            "h5v.symbol_theme must be a string, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn set_registry_setting(
    builder: &mut RegistryBuilder,
    handle: &str,
    value: String,
) -> Result<(), ConfigureErrors> {
    builder
        .update_setting(&SettingHandle::new(handle), move |metadata| {
            metadata.current_value = Some(value);
        })
        .map_err(|error| mlua::Error::runtime(error.to_string()).into())
}

fn register_color_overrides(
    builder: &mut RegistryBuilder,
    table: &Table,
    prefix: Option<&str>,
) -> Result<(), ConfigureErrors> {
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
        if prefix.is_none() && full_name == "themes" {
            continue;
        }
        match value {
            Value::String(value) => {
                let override_value = value.to_str()?.to_string();
                builder
                    .update_color(
                        &ColorHandle::new(format!("builtin.color.{full_name}")),
                        move |metadata| {
                            metadata.override_value = Some(override_value);
                        },
                    )
                    .map_err(|error| mlua::Error::runtime(error.to_string()))?;
            }
            Value::Table(child) => register_color_overrides(builder, &child, Some(&full_name))?,
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

fn register_symbol_overrides(
    builder: &mut RegistryBuilder,
    table: &Table,
    prefix: Option<&str>,
) -> Result<(), ConfigureErrors> {
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
                let override_value = value.to_str()?.to_string();
                builder
                    .update_symbol(
                        &SymbolHandle::new(format!("builtin.symbol.{full_name}")),
                        move |metadata| {
                            metadata.override_value = Some(override_value);
                        },
                    )
                    .map_err(|error| mlua::Error::runtime(error.to_string()))?;
            }
            Value::Table(child) => register_symbol_overrides(builder, &child, Some(&full_name))?,
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
