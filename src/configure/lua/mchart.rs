use mlua::{Lua, Table, Value};

use crate::{configure, configure::errors::ConfigureErrors};

pub(super) fn build_multichart_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let multichart = lua.create_table()?;
    let settings = configure::current_multichart_settings();
    multichart.set("overview_max_samples", settings.overview_max_samples)?;
    multichart.set("detail_enabled", settings.detail_enabled)?;
    multichart.set(
        "detail_samples_per_column",
        settings.detail_samples_per_column,
    )?;
    multichart.set("detail_min_samples", settings.detail_min_samples)?;
    multichart.set("detail_max_samples", settings.detail_max_samples)?;
    multichart.set("detail_padding_ratio", settings.detail_padding_ratio)?;
    multichart.set("derived_detail_enabled", settings.derived_detail_enabled)?;
    Ok(multichart)
}

pub(super) fn parse_multichart_config(
    h5v: &Table,
) -> Result<Option<configure::MultiChartSettings>, ConfigureErrors> {
    let multichart = match h5v.get::<Value>("multichart")? {
        Value::Nil => return Ok(None),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.multichart must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let mut settings = configure::MultiChartSettings::default();
    settings.overview_max_samples = parse_usize_field(
        &multichart,
        "overview_max_samples",
        settings.overview_max_samples,
    )?;
    settings.detail_enabled =
        parse_bool_field(&multichart, "detail_enabled", settings.detail_enabled)?;
    settings.detail_samples_per_column = parse_usize_field(
        &multichart,
        "detail_samples_per_column",
        settings.detail_samples_per_column,
    )?;
    settings.detail_min_samples = parse_usize_field(
        &multichart,
        "detail_min_samples",
        settings.detail_min_samples,
    )?;
    settings.detail_max_samples = parse_usize_field(
        &multichart,
        "detail_max_samples",
        settings.detail_max_samples,
    )?;
    settings.detail_padding_ratio = parse_f64_field(
        &multichart,
        "detail_padding_ratio",
        settings.detail_padding_ratio,
    )?;
    settings.derived_detail_enabled = parse_bool_field(
        &multichart,
        "derived_detail_enabled",
        settings.derived_detail_enabled,
    )?;

    if settings.detail_min_samples > settings.detail_max_samples {
        return Err(mlua::Error::runtime(
            "h5v.multichart.detail_min_samples cannot exceed detail_max_samples",
        )
        .into());
    }
    if !settings.detail_padding_ratio.is_finite() || settings.detail_padding_ratio < 0.0 {
        return Err(mlua::Error::runtime(
            "h5v.multichart.detail_padding_ratio must be a non-negative finite number",
        )
        .into());
    }

    Ok(Some(settings))
}

fn parse_usize_field(table: &Table, field: &str, default: usize) -> Result<usize, ConfigureErrors> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(default),
        Value::Integer(value) if value > 0 => Ok(value as usize),
        Value::Number(value) if value.is_finite() && value.fract() == 0.0 && value > 0.0 => {
            Ok(value as usize)
        }
        Value::Integer(_) | Value::Number(_) => Err(mlua::Error::runtime(format!(
            "h5v.multichart.{field} must be a positive integer"
        ))
        .into()),
        other => Err(mlua::Error::runtime(format!(
            "h5v.multichart.{field} must be a positive integer, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_bool_field(table: &Table, field: &str, default: bool) -> Result<bool, ConfigureErrors> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(default),
        Value::Boolean(value) => Ok(value),
        other => Err(mlua::Error::runtime(format!(
            "h5v.multichart.{field} must be a boolean, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_f64_field(table: &Table, field: &str, default: f64) -> Result<f64, ConfigureErrors> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(default),
        Value::Integer(value) => Ok(value as f64),
        Value::Number(value) => Ok(value),
        other => Err(mlua::Error::runtime(format!(
            "h5v.multichart.{field} must be a number, got {}",
            other.type_name()
        ))
        .into()),
    }
}
