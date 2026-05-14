use mlua::{Lua, Table, Value};

use crate::{
    configure,
    configure::errors::ConfigureErrors,
    ui::state::{
        HeatmapColormap, HeatmapNormalization, HeatmapRangeBound, HeatmapRangeMode, HeatmapSettings,
    },
};

pub(super) fn build_heatmap_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let heatmap = lua.create_table()?;
    let defaults = configure::current_heatmap_default_settings();
    heatmap.set("default_range", defaults.range.label())?;
    heatmap.set("default_colormap", defaults.colormap.as_str())?;
    heatmap.set("default_normalization", defaults.normalization.as_str())?;
    heatmap.set("default_invert_x", defaults.invert_x)?;
    heatmap.set("default_invert_y", defaults.invert_y)?;
    heatmap.set("default_invert_c", defaults.invert_c)?;
    let range_modes = lua.create_table()?;
    for (index, mode) in configure::current_heatmap_range_modes()
        .into_iter()
        .enumerate()
    {
        let HeatmapRangeMode::Custom(custom) = mode else {
            continue;
        };
        let entry = lua.create_table()?;
        entry.set("label", custom.label)?;
        match custom.lower {
            HeatmapRangeBound::Exact(value) => entry.set("min", value.to_f64())?,
            HeatmapRangeBound::Percentile(bps) => {
                entry.set("min", format!("{}%", format_heatmap_percent(bps)))?
            }
        }
        match custom.upper {
            HeatmapRangeBound::Exact(value) => entry.set("max", value.to_f64())?,
            HeatmapRangeBound::Percentile(bps) => {
                entry.set("max", format!("{}%", format_heatmap_percent(bps)))?
            }
        }
        range_modes.set(index + 1, entry)?;
    }
    heatmap.set("range_modes", range_modes)?;
    Ok(heatmap)
}

pub(super) fn parse_heatmap_config(
    h5v: &Table,
) -> Result<Option<(Vec<HeatmapRangeMode>, HeatmapSettings)>, ConfigureErrors> {
    let heatmap = match h5v.get::<Value>("heatmap")? {
        Value::Nil => return Ok(None),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let custom_modes = match heatmap.get::<Value>("range_modes")? {
        Value::Nil => Vec::new(),
        Value::Table(values) => {
            let mut modes = Vec::new();
            for value in values.sequence_values::<Value>() {
                let value = value?;
                let Value::Table(entry) = value else {
                    return Err(mlua::Error::runtime(
                        "h5v.heatmap.range_modes entries must be tables",
                    )
                    .into());
                };
                let lower = parse_heatmap_bound_value(entry.get::<Value>("min")?, "min")?;
                let upper = parse_heatmap_bound_value(entry.get::<Value>("max")?, "max")?;
                let label = match entry.get::<Value>("label")? {
                    Value::Nil => None,
                    Value::String(value) => Some(value.to_str()?.to_string()),
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "h5v.heatmap.range_modes.label must be a string, got {}",
                            other.type_name()
                        ))
                        .into())
                    }
                };
                modes.push(HeatmapRangeMode::custom(lower, upper, label));
            }
            modes
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap.range_modes must be an array of tables, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let mut available = HeatmapRangeMode::default_modes();
    for mode in &custom_modes {
        let label = mode.label();
        if available
            .iter()
            .any(|existing| existing.label().eq_ignore_ascii_case(&label))
        {
            return Err(
                mlua::Error::runtime(format!("Duplicate heatmap range label '{}'", label)).into(),
            );
        }
        available.push(mode.clone());
    }

    let mut default_settings = HeatmapSettings::default();

    default_settings.range = match heatmap.get::<Value>("default_range")? {
        Value::Nil => default_settings.range,
        Value::String(value) => {
            let selector = value.to_str()?;
            available
                .iter()
                .find(|mode| mode.selector_matches(selector.as_ref()))
                .cloned()
                .ok_or_else(|| {
                    mlua::Error::runtime(format!(
                        "Unknown heatmap default range '{}'. Expected one of: {}",
                        selector,
                        available
                            .iter()
                            .map(|mode| mode.label())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                })?
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap.default_range must be a string, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    default_settings.colormap = match heatmap.get::<Value>("default_colormap")? {
        Value::Nil => default_settings.colormap,
        Value::String(value) => {
            HeatmapColormap::parse(value.to_str()?.as_ref()).ok_or_else(|| {
                mlua::Error::runtime(
                    "Unknown heatmap colormap. Expected one of: turbo, grayscale, inferno",
                )
            })?
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap.default_colormap must be a string, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    default_settings.normalization = match heatmap.get::<Value>("default_normalization")? {
        Value::Nil => default_settings.normalization,
        Value::String(value) => {
            HeatmapNormalization::parse(value.to_str()?.as_ref()).ok_or_else(|| {
                mlua::Error::runtime(
                    "Unknown heatmap normalization. Expected one of: linear, log, sqrt",
                )
            })?
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.heatmap.default_normalization must be a string, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    default_settings.invert_x = parse_heatmap_bool_field(&heatmap, "default_invert_x")?
        .unwrap_or(default_settings.invert_x);
    default_settings.invert_y = parse_heatmap_bool_field(&heatmap, "default_invert_y")?
        .unwrap_or(default_settings.invert_y);
    default_settings.invert_c = parse_heatmap_bool_field(&heatmap, "default_invert_c")?
        .unwrap_or(default_settings.invert_c);

    Ok(Some((custom_modes, default_settings)))
}

fn parse_heatmap_bound_value(
    value: Value,
    field_name: &str,
) -> Result<HeatmapRangeBound, ConfigureErrors> {
    match value {
        Value::String(value) => HeatmapRangeBound::parse(value.to_str()?.as_ref())
            .map_err(mlua::Error::runtime)
            .map_err(Into::into),
        Value::Integer(value) => HeatmapRangeBound::parse(&value.to_string())
            .map_err(mlua::Error::runtime)
            .map_err(Into::into),
        Value::Number(value) => HeatmapRangeBound::parse(&value.to_string())
            .map_err(mlua::Error::runtime)
            .map_err(Into::into),
        Value::Nil => Err(mlua::Error::runtime(format!(
            "h5v.heatmap.range_modes.{field_name} is required"
        ))
        .into()),
        other => Err(mlua::Error::runtime(format!(
            "h5v.heatmap.range_modes.{field_name} must be a string or number, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_heatmap_bool_field(
    heatmap: &Table,
    field_name: &str,
) -> Result<Option<bool>, ConfigureErrors> {
    match heatmap.get::<Value>(field_name)? {
        Value::Nil => Ok(None),
        Value::Boolean(value) => Ok(Some(value)),
        other => Err(mlua::Error::runtime(format!(
            "h5v.heatmap.{field_name} must be a boolean, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn format_heatmap_percent(bps: u16) -> String {
    let whole = bps / 100;
    let frac = bps % 100;
    if frac == 0 {
        whole.to_string()
    } else if frac.is_multiple_of(10) {
        format!("{whole}.{}", frac / 10)
    } else {
        format!("{whole}.{frac:02}")
    }
}
