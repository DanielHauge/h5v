use mlua::{Lua, Table, Value};

use crate::configure::errors::ConfigureErrors;
use crate::configure::{self, AxisNumberFormat, ChartSettings};

pub(super) fn build_chart_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let chart = lua.create_table()?;
    let settings = configure::current_chart_settings();
    chart.set(
        "axis_numbers",
        match settings.axis_numbers {
            AxisNumberFormat::Exact => "exact",
            AxisNumberFormat::Auto => "auto",
            AxisNumberFormat::Scientific => "scientific",
        },
    )?;
    chart.set(
        "scientific_lower_exponent",
        settings.scientific_lower_exponent,
    )?;
    chart.set(
        "scientific_upper_exponent",
        settings.scientific_upper_exponent,
    )?;
    Ok(chart)
}

pub(super) fn parse_chart_config(h5v: &Table) -> Result<Option<ChartSettings>, ConfigureErrors> {
    let chart = match h5v.get::<Value>("chart")? {
        Value::Nil => return Ok(None),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.chart must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };
    let mut settings = ChartSettings::default();
    settings.axis_numbers = match chart.get::<Value>("axis_numbers")? {
        Value::Nil => settings.axis_numbers,
        Value::String(value) => match value.to_str()?.trim().to_ascii_lowercase().as_str() {
            "exact" => AxisNumberFormat::Exact,
            "auto" => AxisNumberFormat::Auto,
            "scientific" => AxisNumberFormat::Scientific,
            _ => {
                return Err(mlua::Error::runtime(
                    "h5v.chart.axis_numbers must be exact, auto, or scientific",
                )
                .into())
            }
        },
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.chart.axis_numbers must be a string, got {}",
                other.type_name()
            ))
            .into())
        }
    };
    settings.scientific_lower_exponent = parse_i32(
        &chart,
        "scientific_lower_exponent",
        settings.scientific_lower_exponent,
    )?;
    settings.scientific_upper_exponent = parse_i32(
        &chart,
        "scientific_upper_exponent",
        settings.scientific_upper_exponent,
    )?;
    if settings.scientific_lower_exponent > settings.scientific_upper_exponent {
        return Err(mlua::Error::runtime(
            "h5v.chart.scientific_lower_exponent cannot exceed scientific_upper_exponent",
        )
        .into());
    }
    Ok(Some(settings))
}

fn parse_i32(table: &Table, field: &str, default: i32) -> Result<i32, ConfigureErrors> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(default),
        Value::Integer(value) => i32::try_from(value).map_err(|_| {
            mlua::Error::runtime(format!("h5v.chart.{field} must be an integer")).into()
        }),
        Value::Number(value)
            if value.is_finite()
                && value.fract() == 0.0
                && value >= i32::MIN as f64
                && value <= i32::MAX as f64 =>
        {
            Ok(value as i32)
        }
        Value::Number(_) => {
            Err(mlua::Error::runtime(format!("h5v.chart.{field} must be an integer")).into())
        }
        other => Err(mlua::Error::runtime(format!(
            "h5v.chart.{field} must be an integer, got {}",
            other.type_name()
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_chart_config;

    #[test]
    fn rejects_reversed_scientific_thresholds() {
        let lua = mlua::Lua::new();
        let h5v = lua.create_table().expect("create h5v table");
        let chart = lua.create_table().expect("create chart table");
        chart
            .set("scientific_lower_exponent", 2)
            .expect("set lower threshold");
        chart
            .set("scientific_upper_exponent", -2)
            .expect("set upper threshold");
        h5v.set("chart", chart).expect("set chart table");
        assert!(parse_chart_config(&h5v).is_err());
    }
}
