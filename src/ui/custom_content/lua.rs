use std::{cell::RefCell, rc::Rc};

use crate::{configure, ui::state::AppState};

use super::{
    json::nodes_to_json_value,
    types::{LuaContentNode, LuaSplitDirection},
};

pub(super) fn build_render_context(
    lua: &mlua::Lua,
    state: &AppState<'_>,
) -> Result<mlua::Table, mlua::Error> {
    let ctx = lua.create_table()?;
    ctx.set("app", configure::build_lua_app_context(lua, state)?)?;
    ctx.set("config", configure::build_lua_config_context(lua)?)?;
    ctx.set("fs", configure::build_lua_fs_context(lua, state)?)?;
    ctx.set(
        "selection",
        configure::build_lua_selection_context(lua, state)?,
    )?;
    ctx.set("plugin", configure::build_lua_plugin_context(lua)?)?;

    let file = lua.create_table()?;
    file.set("path", state.file_watch.path.clone())?;
    if let Some(path) = state.selected_tree_path() {
        file.set("selected_path", path)?;
    }
    ctx.set("file", file)?;

    let render_state = lua.create_table()?;
    let h5v: mlua::Table = lua.globals().get("h5v")?;
    let plugin_store: mlua::Table = h5v.get("__plugin_store")?;
    for pair in plugin_store.pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair?;
        render_state.set(key, value)?;
    }
    ctx.set("state", render_state)?;
    Ok(ctx)
}

pub(crate) fn build_ui_document_builder(lua: &mlua::Lua) -> Result<mlua::Table, mlua::Error> {
    let builder = lua.create_table()?;
    builder.set(
        "build",
        lua.create_function(|lua, callback: mlua::Function| {
            let nodes = collect_ui_nodes(lua, callback, None)?;
            let document = lua.create_table()?;
            document.set(
                "__h5v_ui_document",
                serde_json::to_string(&nodes_to_json_value(&nodes))
                    .map_err(|error| mlua::Error::runtime(error.to_string()))?,
            )?;
            Ok(document)
        })?,
    )?;
    Ok(builder)
}

pub(crate) fn build_ui_table(
    lua: &mlua::Lua,
    nodes: Rc<RefCell<Vec<LuaContentNode>>>,
) -> Result<mlua::Table, mlua::Error> {
    let ui = lua.create_table()?;

    let text_nodes = nodes.clone();
    ui.set(
        "text",
        lua.create_function(move |_, text: String| {
            text_nodes.borrow_mut().push(LuaContentNode::Text(text));
            Ok(())
        })?,
    )?;

    let code_nodes = nodes.clone();
    ui.set(
        "code",
        lua.create_function(move |_, (body, kind): (String, Option<String>)| {
            code_nodes.borrow_mut().push(LuaContentNode::Code {
                body,
                kind: kind.map(|kind| kind.trim().to_ascii_lowercase()),
            });
            Ok(())
        })?,
    )?;

    let kv_nodes = nodes.clone();
    ui.set(
        "kv",
        lua.create_function(move |_, (key, value): (String, mlua::Value)| {
            kv_nodes.borrow_mut().push(LuaContentNode::KeyValue {
                key,
                value: lua_value_string(&value),
            });
            Ok(())
        })?,
    )?;

    let separator_nodes = nodes.clone();
    ui.set(
        "separator",
        lua.create_function(move |_, options: Option<mlua::Value>| {
            let (label, empty, height) = match options.unwrap_or(mlua::Value::Nil) {
                mlua::Value::Nil => (None, false, 1usize),
                mlua::Value::String(value) => (Some(value.to_str()?.trim().to_string()), false, 1),
                mlua::Value::Table(table) => (
                    optional_table_string(&table, "label")?,
                    optional_table_bool(&table, "empty")?.unwrap_or(false),
                    optional_table_usize(&table, "height")?.unwrap_or(1).max(1),
                ),
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "ui.separator options must be nil, a label string, or a table, got {}",
                        other.type_name()
                    )))
                }
            };
            separator_nodes
                .borrow_mut()
                .push(LuaContentNode::Separator {
                    label,
                    empty,
                    height,
                });
            Ok(())
        })?,
    )?;

    let badge_nodes = nodes.clone();
    ui.set(
        "badge",
        lua.create_function(move |_, text: String| {
            badge_nodes.borrow_mut().push(LuaContentNode::Badge(text));
            Ok(())
        })?,
    )?;

    let row_nodes = nodes.clone();
    ui.set(
        "row",
        lua.create_function(move |lua, callback: mlua::Function| {
            row_nodes.borrow_mut().push(LuaContentNode::Row {
                children: collect_child_nodes(lua, callback)?,
            });
            Ok(())
        })?,
    )?;

    let column_nodes = nodes.clone();
    ui.set(
        "column",
        lua.create_function(move |lua, callback: mlua::Function| {
            column_nodes.borrow_mut().push(LuaContentNode::Column {
                children: collect_child_nodes(lua, callback)?,
            });
            Ok(())
        })?,
    )?;

    let split_nodes = nodes.clone();
    ui.set(
        "split",
        lua.create_function(
            move |lua, (options, left, right): (mlua::Value, mlua::Function, mlua::Function)| {
                let (direction, ratio_millis, gap) = parse_split_options(&options)?;
                split_nodes.borrow_mut().push(LuaContentNode::Split {
                    direction,
                    ratio_millis,
                    gap,
                    left: collect_child_nodes(lua, left)?,
                    right: collect_child_nodes(lua, right)?,
                });
                Ok(())
            },
        )?,
    )?;

    let table_nodes = nodes.clone();
    ui.set(
        "table",
        lua.create_function(move |_, rows: mlua::Table| {
            table_nodes.borrow_mut().push(LuaContentNode::Table {
                rows: parse_table_rows(&rows)?,
            });
            Ok(())
        })?,
    )?;

    let block_nodes = nodes.clone();
    ui.set(
        "block",
        lua.create_function(
            move |lua, (options, callback): (mlua::Value, mlua::Function)| {
                let title = match options {
                    mlua::Value::Nil => None,
                    mlua::Value::String(value) => Some(value.to_str()?.trim().to_string()),
                    mlua::Value::Table(table) => optional_table_string(&table, "title")?,
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "ui.block options must be nil, a title string, or a table, got {}",
                            other.type_name()
                        )))
                    }
                };
                block_nodes.borrow_mut().push(LuaContentNode::Block {
                    title,
                    children: collect_child_nodes(lua, callback)?,
                });
                Ok(())
            },
        )?,
    )?;

    Ok(ui)
}

pub(super) fn collect_ui_nodes(
    lua: &mlua::Lua,
    callback: mlua::Function,
    ctx: Option<mlua::Table>,
) -> Result<Vec<LuaContentNode>, mlua::Error> {
    let nodes = Rc::new(RefCell::new(Vec::new()));
    let ui = build_ui_table(lua, nodes.clone())?;
    match ctx {
        Some(ctx) => callback.call::<()>((ctx, ui))?,
        None => callback.call::<()>(ui)?,
    }
    Ok(nodes.take())
}

fn optional_table_string(table: &mlua::Table, field: &str) -> Result<Option<String>, mlua::Error> {
    match table.get::<mlua::Value>(field)? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::String(value) => Ok(Some(value.to_str()?.trim().to_string())),
        other => Err(mlua::Error::runtime(format!(
            "ui.block.{} must be a string, got {}",
            field,
            other.type_name()
        ))),
    }
}

fn optional_table_bool(table: &mlua::Table, field: &str) -> Result<Option<bool>, mlua::Error> {
    match table.get::<mlua::Value>(field)? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::Boolean(value) => Ok(Some(value)),
        other => Err(mlua::Error::runtime(format!(
            "ui option '{}' must be a boolean, got {}",
            field,
            other.type_name()
        ))),
    }
}

fn optional_table_usize(table: &mlua::Table, field: &str) -> Result<Option<usize>, mlua::Error> {
    match table.get::<mlua::Value>(field)? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::Integer(value) if value >= 0 => Ok(Some(value as usize)),
        mlua::Value::Number(value) if value >= 0.0 => Ok(Some(value as usize)),
        other => Err(mlua::Error::runtime(format!(
            "ui option '{}' must be a non-negative number, got {}",
            field,
            other.type_name()
        ))),
    }
}

fn parse_split_options(
    options: &mlua::Value,
) -> Result<(LuaSplitDirection, u16, usize), mlua::Error> {
    let mut direction = LuaSplitDirection::Horizontal;
    let mut ratio = 0.5f64;
    let mut gap = 2usize;
    match options {
        mlua::Value::Nil => {}
        mlua::Value::String(value) => {
            direction = parse_split_direction(value.to_str()?.as_ref())?;
        }
        mlua::Value::Table(table) => {
            if let Some(value) = optional_table_string(table, "direction")? {
                direction = parse_split_direction(&value)?;
            }
            match table.get::<mlua::Value>("ratio")? {
                mlua::Value::Nil => {}
                mlua::Value::Integer(value) => ratio = value as f64,
                mlua::Value::Number(value) => ratio = value,
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "ui.split ratio must be a number, got {}",
                        other.type_name()
                    )))
                }
            }
            gap = optional_table_usize(table, "gap")?.unwrap_or(gap);
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "ui.split options must be nil, a direction string, or a table, got {}",
                other.type_name()
            )))
        }
    }
    if !(0.0..=1.0).contains(&ratio) {
        return Err(mlua::Error::runtime(
            "ui.split ratio must be between 0.0 and 1.0".to_string(),
        ));
    }
    Ok((direction, (ratio * 1000.0).round() as u16, gap))
}

fn parse_split_direction(value: &str) -> Result<LuaSplitDirection, mlua::Error> {
    match value.trim().to_ascii_lowercase().as_str() {
        "horizontal" | "row" => Ok(LuaSplitDirection::Horizontal),
        "vertical" | "column" | "col" => Ok(LuaSplitDirection::Vertical),
        other => Err(mlua::Error::runtime(format!(
            "ui.split direction must be 'horizontal' or 'vertical', got '{}'",
            other
        ))),
    }
}

fn collect_child_nodes(
    lua: &mlua::Lua,
    callback: mlua::Function,
) -> Result<Vec<LuaContentNode>, mlua::Error> {
    let child_nodes = Rc::new(RefCell::new(Vec::new()));
    let child_ui = build_ui_table(lua, child_nodes.clone())?;
    callback.call::<()>(child_ui)?;
    Ok(child_nodes.take())
}

fn parse_table_rows(rows: &mlua::Table) -> Result<Vec<Vec<String>>, mlua::Error> {
    let mut parsed = Vec::new();
    for row in rows.sequence_values::<mlua::Table>() {
        let row = row?;
        let mut cells = Vec::new();
        for value in row.sequence_values::<mlua::Value>() {
            cells.push(lua_value_string(&value?));
        }
        parsed.push(cells);
    }
    Ok(parsed)
}

fn lua_value_string(value: &mlua::Value) -> String {
    match value {
        mlua::Value::Nil => String::new(),
        mlua::Value::Boolean(value) => value.to_string(),
        mlua::Value::Integer(value) => value.to_string(),
        mlua::Value::Number(value) => value.to_string(),
        mlua::Value::String(value) => value
            .to_str()
            .map(|text| text.to_string())
            .unwrap_or_default(),
        other => format!("<{}>", other.type_name()),
    }
}
