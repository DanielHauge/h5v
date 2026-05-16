use mlua::{Lua, Table, Value};

use crate::{configure, configure::errors::ConfigureErrors};

pub(super) fn build_layout_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let layout = lua.create_table()?;
    let settings = configure::current_auto_layout_settings();
    layout.set("tree", build_panel_table(lua, &settings.tree)?)?;
    layout.set("attributes", build_panel_table(lua, &settings.attributes)?)?;
    layout.set("content", build_panel_table(lua, &settings.content)?)?;
    Ok(layout)
}

pub(super) fn parse_layout_config(
    h5v: &Table,
) -> Result<Option<configure::AutoLayoutSettings>, ConfigureErrors> {
    let layout = match h5v.get::<Value>("layout")? {
        Value::Nil => return Ok(None),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.layout must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let defaults = configure::AutoLayoutSettings::default();
    let settings = configure::AutoLayoutSettings {
        tree: parse_panel_config(&layout, "tree", &defaults.tree)?,
        attributes: parse_panel_config(&layout, "attributes", &defaults.attributes)?,
        content: parse_panel_config(&layout, "content", &defaults.content)?,
    };
    validate_layout_settings(&settings)?;
    Ok(Some(settings))
}

fn build_panel_table(
    lua: &Lua,
    sizes: &configure::PanelLayoutSizes,
) -> Result<Table, ConfigureErrors> {
    let table = lua.create_table()?;
    table.set("focused", layout_size_to_lua(lua, &sizes.focused)?)?;
    table.set("unfocused", layout_size_to_lua(lua, &sizes.unfocused)?)?;
    Ok(table)
}

fn parse_panel_config(
    layout: &Table,
    field: &str,
    default: &configure::PanelLayoutSizes,
) -> Result<configure::PanelLayoutSizes, ConfigureErrors> {
    let panel = match layout.get::<Value>(field)? {
        Value::Nil => return Ok(default.clone()),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.layout.{field} must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    Ok(configure::PanelLayoutSizes::new(
        parse_size_field(&panel, field, "focused", &default.focused)?,
        parse_size_field(&panel, field, "unfocused", &default.unfocused)?,
    ))
}

fn layout_size_to_lua(lua: &Lua, size: &configure::LayoutSize) -> Result<Value, ConfigureErrors> {
    Ok(match size {
        configure::LayoutSize::Cells(value) => Value::Integer(i64::from(*value)),
        configure::LayoutSize::Min(value) => {
            Value::String(lua.create_string(format!("min({value})"))?)
        }
        configure::LayoutSize::Max(value) => {
            Value::String(lua.create_string(format!("max({value})"))?)
        }
        configure::LayoutSize::Percent(value) => {
            Value::String(lua.create_string(format!("{value}%"))?)
        }
        configure::LayoutSize::Ratio(numerator, denominator) => {
            Value::String(lua.create_string(format!("ratio({numerator},{denominator})"))?)
        }
        configure::LayoutSize::Fill(1) => Value::String(lua.create_string("*")?),
        configure::LayoutSize::Fill(weight) => {
            Value::String(lua.create_string(format!("fill({weight})"))?)
        }
    })
}

fn parse_size_field(
    panel: &Table,
    panel_name: &str,
    field_name: &str,
    default: &configure::LayoutSize,
) -> Result<configure::LayoutSize, ConfigureErrors> {
    match panel.get::<Value>(field_name)? {
        Value::Nil => Ok(default.clone()),
        Value::Integer(value) if (0..=i64::from(u16::MAX)).contains(&value) => {
            Ok(configure::LayoutSize::cells(value as u16))
        }
        Value::Number(value)
            if value.is_finite()
                && value.fract() == 0.0
                && (0.0..=f64::from(u16::MAX)).contains(&value) =>
        {
            Ok(configure::LayoutSize::cells(value as u16))
        }
        Value::Integer(_) | Value::Number(_) => Err(mlua::Error::runtime(format!(
            "h5v.layout.{panel_name}.{field_name} must be a non-negative integer, a percentage string like \"30%\", \"*\", or a constraint like \"max(12)\""
        ))
        .into()),
        Value::String(value) => parse_layout_string(value.to_str()?.as_ref(), panel_name, field_name),
        other => Err(mlua::Error::runtime(format!(
            "h5v.layout.{panel_name}.{field_name} must be an integer or string, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_layout_string(
    value: &str,
    panel_name: &str,
    field_name: &str,
) -> Result<configure::LayoutSize, ConfigureErrors> {
    let value = value.trim();
    if value == "*" {
        return Ok(configure::LayoutSize::fill());
    }
    if let Some(parsed) = parse_constraint_call(value, panel_name, field_name)? {
        return Ok(parsed);
    }
    let Some(percent) = value.strip_suffix('%') else {
        return Err(mlua::Error::runtime(format!(
            "h5v.layout.{panel_name}.{field_name} must use \"*\", an integer percentage string like \"30%\", or a constraint like \"max(12)\""
        ))
        .into());
    };
    let percent = percent.trim().parse::<u16>().map_err(|_| {
        mlua::Error::runtime(format!(
            "h5v.layout.{panel_name}.{field_name} has invalid percentage '{value}'"
        ))
    })?;
    if percent > 100 {
        return Err(mlua::Error::runtime(format!(
            "h5v.layout.{panel_name}.{field_name} percentage must be between 0% and 100%"
        ))
        .into());
    }
    Ok(configure::LayoutSize::percent(percent))
}

fn parse_constraint_call(
    value: &str,
    panel_name: &str,
    field_name: &str,
) -> Result<Option<configure::LayoutSize>, ConfigureErrors> {
    let Some(open_index) = value.find('(') else {
        return Ok(None);
    };
    let Some(close_index) = value.rfind(')') else {
        return Err(mlua::Error::runtime(format!(
            "h5v.layout.{panel_name}.{field_name} has invalid constraint '{value}'"
        ))
        .into());
    };
    if close_index <= open_index || close_index + 1 != value.len() {
        return Err(mlua::Error::runtime(format!(
            "h5v.layout.{panel_name}.{field_name} has invalid constraint '{value}'"
        ))
        .into());
    }
    let name = value[..open_index].trim().to_ascii_lowercase();
    let args = value[open_index + 1..close_index]
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let parse_u16 = |raw: &str| {
        raw.parse::<u16>().map_err(|_| {
            mlua::Error::runtime(format!(
                "h5v.layout.{panel_name}.{field_name} has invalid constraint '{value}'"
            ))
        })
    };
    let parse_u32 = |raw: &str| {
        raw.parse::<u32>().map_err(|_| {
            mlua::Error::runtime(format!(
                "h5v.layout.{panel_name}.{field_name} has invalid constraint '{value}'"
            ))
        })
    };

    let parsed = match name.as_str() {
        "min" if args.len() == 1 => configure::LayoutSize::min(parse_u16(args[0])?),
        "max" if args.len() == 1 => configure::LayoutSize::max(parse_u16(args[0])?),
        "length" | "cells" if args.len() == 1 => configure::LayoutSize::cells(parse_u16(args[0])?),
        "fill" if args.len() == 1 => configure::LayoutSize::fill_weight(parse_u16(args[0])?),
        "ratio" if args.len() == 2 => {
            let numerator = parse_u32(args[0])?;
            let denominator = parse_u32(args[1])?;
            if denominator == 0 {
                return Err(mlua::Error::runtime(format!(
                    "h5v.layout.{panel_name}.{field_name} ratio denominator must be non-zero"
                ))
                .into());
            }
            configure::LayoutSize::ratio(numerator, denominator)
        }
        _ => {
            return Err(mlua::Error::runtime(format!(
                "h5v.layout.{panel_name}.{field_name} has invalid constraint '{value}'"
            ))
            .into())
        }
    };
    Ok(Some(parsed))
}

fn validate_layout_settings(
    settings: &configure::AutoLayoutSettings,
) -> Result<(), ConfigureErrors> {
    validate_percent_pair(
        "h5v.layout.attributes.focused",
        &settings.attributes.focused,
        "h5v.layout.content.unfocused",
        &settings.content.unfocused,
    )?;
    validate_percent_pair(
        "h5v.layout.attributes.unfocused",
        &settings.attributes.unfocused,
        "h5v.layout.content.focused",
        &settings.content.focused,
    )?;
    Ok(())
}

fn validate_percent_pair(
    left_name: &str,
    left: &configure::LayoutSize,
    right_name: &str,
    right: &configure::LayoutSize,
) -> Result<(), ConfigureErrors> {
    if let (configure::LayoutSize::Percent(left), configure::LayoutSize::Percent(right)) =
        (left, right)
    {
        if left + right != 100 {
            return Err(mlua::Error::runtime(format!(
                "{left_name} ({left}%) + {right_name} ({right}%) must equal 100% when both sides use percentages"
            ))
            .into());
        }
    }
    Ok(())
}
