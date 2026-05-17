use std::{cell::RefCell, rc::Rc};

use ratatui::{
    style::Style,
    symbols::border,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use serde_json::{Map, Value};

use crate::{
    configure, configure::registry::ContentModeHandle, error::AppError,
    ui::std_comp_render::highlighted_lines,
};

use super::state::AppState;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LuaContentNode {
    Text(String),
    Code {
        body: String,
        kind: Option<String>,
    },
    Badge(String),
    KeyValue {
        key: String,
        value: String,
    },
    Separator {
        label: Option<String>,
        empty: bool,
        height: usize,
    },
    Row {
        children: Vec<LuaContentNode>,
    },
    Column {
        children: Vec<LuaContentNode>,
    },
    Split {
        direction: LuaSplitDirection,
        ratio_millis: u16,
        gap: usize,
        left: Vec<LuaContentNode>,
        right: Vec<LuaContentNode>,
    },
    Table {
        rows: Vec<Vec<String>>,
    },
    Block {
        title: Option<String>,
        children: Vec<LuaContentNode>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LuaSplitDirection {
    Horizontal,
    Vertical,
}

pub(crate) fn render_custom_content_mode(
    f: &mut Frame,
    area: &ratatui::layout::Rect,
    state: &AppState<'_>,
    handle: &ContentModeHandle,
) -> Result<(), AppError> {
    let snapshot = configure::current_registry_snapshot();
    let metadata = snapshot.content_mode(handle).ok_or_else(|| {
        AppError::InvalidCommand(format!(
            "Content mode '{}' is not registered",
            handle.as_str()
        ))
    })?;
    let callback_id = metadata.callback_id.as_deref().ok_or_else(|| {
        AppError::InvalidCommand(format!(
            "Content mode '{}' is not renderable",
            handle.as_str()
        ))
    })?;

    let nodes = configure::with_content_mode_lua_callback(callback_id, |lua, callback| {
        let ctx = build_render_context(lua, state)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        collect_ui_nodes(lua, callback, Some(ctx))
            .map_err(|error| AppError::InvalidCommand(error.to_string()))
    })?;

    let mut lines = render_ui_nodes(&nodes, area.width as usize);
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No content".to_string(),
            Style::default().fg(configure::themed_color(|colors| colors.help.muted)),
        )));
    }
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(configure::themed_color(|colors| colors.surface.bg)))
            .wrap(Wrap { trim: false }),
        *area,
    );
    Ok(())
}

fn build_render_context(lua: &mlua::Lua, state: &AppState<'_>) -> Result<mlua::Table, mlua::Error> {
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

pub(crate) fn parse_ui_document(
    value: &mlua::Value,
    context: &str,
) -> Result<Option<String>, mlua::Error> {
    let mlua::Value::Table(table) = value else {
        return Ok(None);
    };
    match table.get::<mlua::Value>("__h5v_ui_document")? {
        mlua::Value::Nil => Ok(None),
        mlua::Value::String(value) => Ok(Some(value.to_str()?.to_string())),
        other => Err(mlua::Error::runtime(format!(
            "{context}.__h5v_ui_document must be a string, got {}",
            other.type_name()
        ))),
    }
}

pub(crate) fn render_serialized_ui_document(
    document: &str,
    width: usize,
) -> Result<Vec<Line<'static>>, String> {
    let value: Value = serde_json::from_str(document).map_err(|error| error.to_string())?;
    let nodes = nodes_from_json_value(&value)?;
    Ok(render_ui_nodes(&nodes, width))
}

fn build_ui_table(
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

fn collect_ui_nodes(
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

pub(crate) fn render_ui_nodes(nodes: &[LuaContentNode], width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for node in nodes {
        render_node(node, 0, width, &mut lines);
    }
    lines
}

fn nodes_to_json_value(nodes: &[LuaContentNode]) -> Value {
    Value::Array(nodes.iter().map(node_to_json_value).collect())
}

fn node_to_json_value(node: &LuaContentNode) -> Value {
    let mut object = Map::new();
    match node {
        LuaContentNode::Text(text) => {
            object.insert("type".to_string(), Value::String("text".to_string()));
            object.insert("text".to_string(), Value::String(text.clone()));
        }
        LuaContentNode::Code { body, kind } => {
            object.insert("type".to_string(), Value::String("code".to_string()));
            object.insert("body".to_string(), Value::String(body.clone()));
            object.insert(
                "kind".to_string(),
                kind.clone().map(Value::String).unwrap_or(Value::Null),
            );
        }
        LuaContentNode::Badge(text) => {
            object.insert("type".to_string(), Value::String("badge".to_string()));
            object.insert("text".to_string(), Value::String(text.clone()));
        }
        LuaContentNode::KeyValue { key, value } => {
            object.insert("type".to_string(), Value::String("key_value".to_string()));
            object.insert("key".to_string(), Value::String(key.clone()));
            object.insert("value".to_string(), Value::String(value.clone()));
        }
        LuaContentNode::Separator {
            label,
            empty,
            height,
        } => {
            object.insert("type".to_string(), Value::String("separator".to_string()));
            object.insert(
                "label".to_string(),
                label.clone().map(Value::String).unwrap_or(Value::Null),
            );
            object.insert("empty".to_string(), Value::Bool(*empty));
            object.insert(
                "height".to_string(),
                Value::Number(serde_json::Number::from(*height as u64)),
            );
        }
        LuaContentNode::Row { children } => {
            object.insert("type".to_string(), Value::String("row".to_string()));
            object.insert("children".to_string(), nodes_to_json_value(children));
        }
        LuaContentNode::Column { children } => {
            object.insert("type".to_string(), Value::String("column".to_string()));
            object.insert("children".to_string(), nodes_to_json_value(children));
        }
        LuaContentNode::Split {
            direction,
            ratio_millis,
            gap,
            left,
            right,
        } => {
            object.insert("type".to_string(), Value::String("split".to_string()));
            object.insert(
                "direction".to_string(),
                Value::String(match direction {
                    LuaSplitDirection::Horizontal => "horizontal".to_string(),
                    LuaSplitDirection::Vertical => "vertical".to_string(),
                }),
            );
            object.insert(
                "ratio".to_string(),
                serde_json::Number::from_f64((*ratio_millis as f64) / 1000.0)
                    .map(Value::Number)
                    .unwrap_or(Value::Null),
            );
            object.insert(
                "gap".to_string(),
                Value::Number(serde_json::Number::from(*gap as u64)),
            );
            object.insert("left".to_string(), nodes_to_json_value(left));
            object.insert("right".to_string(), nodes_to_json_value(right));
        }
        LuaContentNode::Table { rows } => {
            object.insert("type".to_string(), Value::String("table".to_string()));
            object.insert(
                "rows".to_string(),
                Value::Array(
                    rows.iter()
                        .map(|row| Value::Array(row.iter().cloned().map(Value::String).collect()))
                        .collect(),
                ),
            );
        }
        LuaContentNode::Block { title, children } => {
            object.insert("type".to_string(), Value::String("block".to_string()));
            match title {
                Some(title) => {
                    object.insert("title".to_string(), Value::String(title.clone()));
                }
                None => {
                    object.insert("title".to_string(), Value::Null);
                }
            }
            object.insert("children".to_string(), nodes_to_json_value(children));
        }
    }
    Value::Object(object)
}

fn nodes_from_json_value(value: &Value) -> Result<Vec<LuaContentNode>, String> {
    let Value::Array(values) = value else {
        return Err("health UI document must be an array".to_string());
    };
    values.iter().map(node_from_json_value).collect()
}

fn node_from_json_value(value: &Value) -> Result<LuaContentNode, String> {
    let Value::Object(object) = value else {
        return Err("health UI node must be an object".to_string());
    };
    let node_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "health UI node type is required".to_string())?;
    match node_type {
        "text" => Ok(LuaContentNode::Text(required_json_string(object, "text")?)),
        "code" => Ok(LuaContentNode::Code {
            body: required_or_legacy_json_string(object, "body", "text")?,
            kind: optional_json_string(object, "kind")?,
        }),
        "badge" => Ok(LuaContentNode::Badge(required_json_string(object, "text")?)),
        "key_value" => Ok(LuaContentNode::KeyValue {
            key: required_json_string(object, "key")?,
            value: required_json_string(object, "value")?,
        }),
        "separator" => Ok(LuaContentNode::Separator {
            label: optional_json_string(object, "label")?,
            empty: object
                .get("empty")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            height: object.get("height").and_then(Value::as_u64).unwrap_or(1) as usize,
        }),
        "row" => Ok(LuaContentNode::Row {
            children: nodes_from_json_value(
                object
                    .get("children")
                    .ok_or_else(|| "health UI row children are required".to_string())?,
            )?,
        }),
        "column" => Ok(LuaContentNode::Column {
            children: nodes_from_json_value(
                object
                    .get("children")
                    .ok_or_else(|| "health UI column children are required".to_string())?,
            )?,
        }),
        "split" => Ok(LuaContentNode::Split {
            direction: parse_json_split_direction(
                object
                    .get("direction")
                    .and_then(Value::as_str)
                    .unwrap_or("horizontal"),
            )?,
            ratio_millis: parse_json_ratio_millis(object.get("ratio"))?,
            gap: object.get("gap").and_then(Value::as_u64).unwrap_or(2) as usize,
            left: nodes_from_json_value(
                object
                    .get("left")
                    .ok_or_else(|| "health UI split left children are required".to_string())?,
            )?,
            right: nodes_from_json_value(
                object
                    .get("right")
                    .ok_or_else(|| "health UI split right children are required".to_string())?,
            )?,
        }),
        "table" => Ok(LuaContentNode::Table {
            rows: parse_json_rows(
                object
                    .get("rows")
                    .ok_or_else(|| "health UI table rows are required".to_string())?,
            )?,
        }),
        "block" => Ok(LuaContentNode::Block {
            title: optional_json_string(object, "title")?,
            children: nodes_from_json_value(
                object
                    .get("children")
                    .ok_or_else(|| "health UI block children are required".to_string())?,
            )?,
        }),
        other => Err(format!("unknown health UI node type '{other}'")),
    }
}

fn required_json_string(object: &Map<String, Value>, field: &str) -> Result<String, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("health UI field '{field}' must be a string"))
}

fn required_or_legacy_json_string(
    object: &Map<String, Value>,
    field: &str,
    legacy_field: &str,
) -> Result<String, String> {
    required_json_string(object, field).or_else(|_| required_json_string(object, legacy_field))
}

fn optional_json_string(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("health UI field '{field}' must be a string")),
    }
}

fn parse_json_ratio_millis(value: Option<&Value>) -> Result<u16, String> {
    let ratio = value.and_then(Value::as_f64).unwrap_or(0.5);
    if !(0.0..=1.0).contains(&ratio) {
        return Err("health UI split ratio must be between 0.0 and 1.0".to_string());
    }
    Ok((ratio * 1000.0).round() as u16)
}

fn parse_json_split_direction(value: &str) -> Result<LuaSplitDirection, String> {
    match value {
        "horizontal" | "row" => Ok(LuaSplitDirection::Horizontal),
        "vertical" | "column" | "col" => Ok(LuaSplitDirection::Vertical),
        other => Err(format!("unknown health UI split direction '{other}'")),
    }
}

fn parse_json_rows(value: &Value) -> Result<Vec<Vec<String>>, String> {
    let Value::Array(rows) = value else {
        return Err("health UI table rows must be an array".to_string());
    };
    rows.iter()
        .map(|row| {
            let Value::Array(cells) = row else {
                return Err("health UI table row must be an array".to_string());
            };
            cells
                .iter()
                .map(|cell| {
                    cell.as_str()
                        .map(ToString::to_string)
                        .ok_or_else(|| "health UI table cells must be strings".to_string())
                })
                .collect()
        })
        .collect()
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

fn render_node(node: &LuaContentNode, indent: usize, width: usize, lines: &mut Vec<Line<'static>>) {
    match node {
        LuaContentNode::Text(text) => {
            append_multiline_text(text, indent, text_style(), lines);
        }
        LuaContentNode::Code { body, kind } => {
            render_code_block(body, kind.as_deref(), indent, width, lines);
        }
        LuaContentNode::Badge(text) => {
            lines.push(Line::from(Span::styled(
                format!("{}[{}]", " ".repeat(indent), text),
                badge_style(),
            )));
        }
        LuaContentNode::KeyValue { key, value } => {
            let prefix = format!("{}{}: ", " ".repeat(indent), key);
            lines.push(Line::from(vec![
                Span::styled(prefix, key_style()),
                Span::styled(value.clone(), text_style()),
            ]));
        }
        LuaContentNode::Separator {
            label,
            empty,
            height,
        } => {
            render_separator(label.as_deref(), *empty, *height, indent, width, lines);
        }
        LuaContentNode::Row { children } => {
            if !render_inline_row(children, indent, lines) {
                for child in children {
                    render_node(child, indent, width, lines);
                }
            }
        }
        LuaContentNode::Column { children } => {
            for child in children {
                render_node(child, indent, width, lines);
            }
        }
        LuaContentNode::Split {
            direction,
            ratio_millis,
            gap,
            left,
            right,
        } => render_split(
            *direction,
            *ratio_millis,
            *gap,
            left,
            right,
            indent,
            width,
            lines,
        ),
        LuaContentNode::Table { rows } => render_table(rows, indent, lines),
        LuaContentNode::Block { title, children } => {
            let inner_width = width.saturating_sub(indent + 4).max(1);
            let mut body = render_ui_nodes(children, inner_width);
            if body.is_empty() {
                body.push(Line::from(Span::styled(String::new(), text_style())));
            }
            render_framed_lines(title.as_deref(), body, indent, width, false, lines);
        }
    }
}

fn render_inline_row(
    children: &[LuaContentNode],
    indent: usize,
    lines: &mut Vec<Line<'static>>,
) -> bool {
    let mut spans = vec![Span::raw(" ".repeat(indent))];
    for (index, child) in children.iter().enumerate() {
        let Some(mut inline) = inline_spans(child) else {
            return false;
        };
        if index > 0 {
            spans.push(Span::raw("  ".to_string()));
        }
        spans.append(&mut inline);
    }
    lines.push(Line::from(spans));
    true
}

fn inline_spans(node: &LuaContentNode) -> Option<Vec<Span<'static>>> {
    match node {
        LuaContentNode::Text(text) => Some(vec![Span::styled(text.clone(), text_style())]),
        LuaContentNode::Code { .. } => None,
        LuaContentNode::Badge(text) => {
            Some(vec![Span::styled(format!("[{}]", text), badge_style())])
        }
        LuaContentNode::KeyValue { key, value } => Some(vec![
            Span::styled(format!("{key}: "), key_style()),
            Span::styled(value.clone(), text_style()),
        ]),
        LuaContentNode::Separator { .. }
        | LuaContentNode::Row { .. }
        | LuaContentNode::Column { .. }
        | LuaContentNode::Split { .. }
        | LuaContentNode::Table { .. }
        | LuaContentNode::Block { .. } => None,
    }
}

fn render_table(rows: &[Vec<String>], indent: usize, lines: &mut Vec<Line<'static>>) {
    if rows.is_empty() {
        return;
    }
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths = (0..column_count)
        .map(|index| {
            rows.iter()
                .filter_map(|row| row.get(index))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();

    for (row_index, row) in rows.iter().enumerate() {
        let mut spans = vec![Span::raw(" ".repeat(indent))];
        for (index, width) in widths.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw(" ".to_string()));
            }
            let cell = row.get(index).cloned().unwrap_or_default();
            let padding = width.saturating_sub(cell.chars().count());
            let style = table_cell_style(row_index, index);
            spans.push(Span::styled(" ".to_string(), style));
            spans.push(Span::styled(
                format!("{cell}{}", " ".repeat(padding)),
                style,
            ));
            spans.push(Span::styled(" ".to_string(), style));
        }
        lines.push(Line::from(spans));
    }
}

fn render_framed_lines(
    title: Option<&str>,
    body: Vec<Line<'static>>,
    indent: usize,
    width: usize,
    code_block: bool,
    lines: &mut Vec<Line<'static>>,
) {
    let inner_width = width.saturating_sub(indent + 4).max(1);
    lines.push(framed_top_line(title, indent, inner_width, code_block));
    for line in body {
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            Span::styled(
                border::ROUNDED.vertical_left.to_string(),
                frame_border_style(code_block),
            ),
            Span::styled(" ".to_string(), frame_fill_style(code_block)),
        ];
        let line_width = line.width();
        spans.extend(line.spans);
        if line_width < inner_width {
            spans.push(Span::styled(
                " ".repeat(inner_width - line_width),
                frame_fill_style(code_block),
            ));
        }
        spans.push(Span::styled(" ".to_string(), frame_fill_style(code_block)));
        spans.push(Span::styled(
            border::ROUNDED.vertical_right.to_string(),
            frame_border_style(code_block),
        ));
        lines.push(Line::from(spans));
    }
    lines.push(framed_bottom_line(indent, inner_width, code_block));
}

fn framed_top_line(
    title: Option<&str>,
    indent: usize,
    inner_width: usize,
    code_block: bool,
) -> Line<'static> {
    let total_width = inner_width.saturating_add(2);
    let title = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| format!(" {title} "));
    let mut spans = vec![
        Span::raw(" ".repeat(indent)),
        Span::styled(
            border::ROUNDED.top_left.to_string(),
            frame_border_style(code_block),
        ),
    ];
    if let Some(title) = title {
        let title_width = title.chars().count().min(total_width);
        let left = total_width.saturating_sub(title_width) / 2;
        let right = total_width.saturating_sub(title_width + left);
        spans.push(Span::styled(
            border::ROUNDED.horizontal_top.repeat(left),
            frame_border_style(code_block),
        ));
        spans.push(Span::styled(title, frame_title_style(code_block)));
        spans.push(Span::styled(
            border::ROUNDED.horizontal_top.repeat(right),
            frame_border_style(code_block),
        ));
    } else {
        spans.push(Span::styled(
            border::ROUNDED.horizontal_top.repeat(total_width),
            frame_border_style(code_block),
        ));
    }
    spans.push(Span::styled(
        border::ROUNDED.top_right.to_string(),
        frame_border_style(code_block),
    ));
    Line::from(spans)
}

fn framed_bottom_line(indent: usize, inner_width: usize, code_block: bool) -> Line<'static> {
    Line::from(vec![
        Span::raw(" ".repeat(indent)),
        Span::styled(
            border::ROUNDED.bottom_left.to_string(),
            frame_border_style(code_block),
        ),
        Span::styled(
            border::ROUNDED
                .horizontal_bottom
                .repeat(inner_width.saturating_add(2)),
            frame_border_style(code_block),
        ),
        Span::styled(
            border::ROUNDED.bottom_right.to_string(),
            frame_border_style(code_block),
        ),
    ])
}

fn separator_line(label: Option<&str>, indent: usize, line_width: usize) -> Line<'static> {
    let mut spans = vec![Span::raw(" ".repeat(indent))];
    if let Some(label) = label {
        let text = format!(" {label} ");
        let label_width = text.chars().count().min(line_width);
        let left = line_width.saturating_sub(label_width) / 2;
        let right = line_width.saturating_sub(label_width + left);
        spans.push(Span::styled("─".repeat(left), separator_style()));
        spans.push(Span::styled(text, frame_title_style(false)));
        spans.push(Span::styled("─".repeat(right), separator_style()));
    } else {
        spans.push(Span::styled("─".repeat(line_width), separator_style()));
    }
    Line::from(spans)
}

fn padded_line_spans(
    line: Option<&Line<'static>>,
    width: usize,
    filler_style: Style,
) -> Vec<Span<'static>> {
    match line {
        Some(line) => {
            let mut spans = line.spans.clone();
            let line_width = line.width();
            if line_width < width {
                spans.push(Span::styled(" ".repeat(width - line_width), filler_style));
            }
            spans
        }
        None => vec![Span::styled(" ".repeat(width), filler_style)],
    }
}

fn table_cell_style(row_index: usize, column_index: usize) -> Style {
    let bg = if row_index == 0 {
        configure::themed_color(|colors| colors.surface.bg_val3)
    } else if (row_index + column_index) % 2 == 0 {
        configure::themed_color(|colors| colors.surface.bg_val2)
    } else {
        configure::themed_color(|colors| colors.surface.bg_val1)
    };
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(bg)
}

fn render_code_block(
    body: &str,
    kind: Option<&str>,
    indent: usize,
    width: usize,
    lines: &mut Vec<Line<'static>>,
) {
    let mut body_lines = kind
        .and_then(|kind| highlighted_lines(body, kind))
        .unwrap_or_else(|| {
            body.lines()
                .map(|line| Line::from(Span::styled(line.to_string(), code_style())))
                .collect()
        });
    if body_lines.is_empty() {
        body_lines.push(Line::from(Span::styled(String::new(), code_style())));
    }
    render_framed_lines(kind, body_lines, indent, width, true, lines);
}

fn render_separator(
    label: Option<&str>,
    empty: bool,
    height: usize,
    indent: usize,
    width: usize,
    lines: &mut Vec<Line<'static>>,
) {
    let line_width = width.saturating_sub(indent).max(1);
    for _ in 0..height {
        if empty {
            lines.push(Line::from(Span::styled(
                " ".repeat(indent + line_width),
                separator_fill_style(),
            )));
            continue;
        }
        let text = label.filter(|label| !label.trim().is_empty());
        lines.push(separator_line(text, indent, line_width));
    }
}

fn render_split(
    direction: LuaSplitDirection,
    ratio_millis: u16,
    gap: usize,
    left: &[LuaContentNode],
    right: &[LuaContentNode],
    indent: usize,
    width: usize,
    lines: &mut Vec<Line<'static>>,
) {
    match direction {
        LuaSplitDirection::Vertical => {
            for child in left {
                render_node(child, indent, width, lines);
            }
            for _ in 0..gap {
                lines.push(Line::from(Span::raw(" ".repeat(indent))));
            }
            for child in right {
                render_node(child, indent, width, lines);
            }
        }
        LuaSplitDirection::Horizontal => {
            let available = width.saturating_sub(indent);
            if available <= gap + 2 {
                for child in left {
                    render_node(child, indent, width, lines);
                }
                for child in right {
                    render_node(child, indent, width, lines);
                }
                return;
            }
            let content_width = available.saturating_sub(gap);
            let left_width = ((content_width as u32 * ratio_millis as u32) / 1000) as usize;
            let left_width = left_width.clamp(1, content_width.saturating_sub(1));
            let right_width = content_width.saturating_sub(left_width);
            let left_lines = render_ui_nodes(left, left_width);
            let right_lines = render_ui_nodes(right, right_width);
            let total_lines = left_lines.len().max(right_lines.len());
            for index in 0..total_lines {
                let mut spans = vec![Span::raw(" ".repeat(indent))];
                spans.extend(padded_line_spans(
                    left_lines.get(index),
                    left_width,
                    text_style(),
                ));
                spans.push(Span::raw(" ".repeat(gap)));
                spans.extend(padded_line_spans(
                    right_lines.get(index),
                    right_width,
                    text_style(),
                ));
                lines.push(Line::from(spans));
            }
        }
    }
}

fn append_multiline_text(text: &str, indent: usize, style: Style, lines: &mut Vec<Line<'static>>) {
    if text.is_empty() {
        lines.push(Line::from(Span::styled(" ".repeat(indent), style)));
        return;
    }
    for line in text.lines() {
        lines.push(Line::from(Span::styled(
            format!("{}{}", " ".repeat(indent), line),
            style,
        )));
    }
}

fn frame_title_style(code_block: bool) -> Style {
    if code_block {
        Style::default()
            .fg(configure::themed_color(|colors| colors.help.section))
            .bg(configure::themed_color(|colors| colors.surface.bg_val3))
            .bold()
    } else {
        Style::default()
            .fg(configure::themed_color(|colors| colors.content.app_brand))
            .bold()
    }
}

fn frame_border_style(code_block: bool) -> Style {
    let mut style = Style::default()
        .fg(configure::themed_color(|colors| colors.help.muted))
        .dim();
    if code_block {
        style = style.bg(configure::themed_color(|colors| colors.surface.bg_val3));
    }
    style
}

fn frame_fill_style(code_block: bool) -> Style {
    if code_block {
        Style::default().bg(configure::themed_color(|colors| colors.surface.bg_val3))
    } else {
        Style::default()
    }
}

fn key_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.help.section))
        .bold()
}

fn text_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| colors.text.primary))
}

fn code_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.text.primary))
        .bg(configure::themed_color(|colors| colors.surface.bg_val3))
}

fn badge_style() -> Style {
    Style::default()
        .fg(configure::themed_color(|colors| colors.accent.selection_fg))
        .bg(configure::themed_color(|colors| colors.accent.selection_bg))
        .bold()
}

fn separator_style() -> Style {
    Style::default().fg(configure::themed_color(|colors| {
        colors.surface.panel_border
    }))
}

fn separator_fill_style() -> Style {
    separator_style().bg(configure::themed_color(|colors| colors.surface.bg_val3))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{build_ui_table, LuaContentNode};

    #[test]
    fn ui_block_collects_nested_content_nodes() {
        let lua = mlua::Lua::new();
        let nodes = Rc::new(RefCell::new(Vec::new()));
        let ui = build_ui_table(&lua, nodes.clone()).expect("build ui table");
        lua.globals().set("ui", ui).expect("set ui");

        lua.load(
            r#"
            ui.block({ title = "Analysis" }, function(ui)
              ui.text("hello")
              ui.kv("File", "demo.h5")
              ui.separator({ label = "status" })
              ui.code("done", "lua")
            end)
        "#,
        )
        .exec()
        .expect("build ui nodes");

        assert_eq!(
            nodes.take(),
            vec![LuaContentNode::Block {
                title: Some("Analysis".to_string()),
                children: vec![
                    LuaContentNode::Text("hello".to_string()),
                    LuaContentNode::KeyValue {
                        key: "File".to_string(),
                        value: "demo.h5".to_string(),
                    },
                    LuaContentNode::Separator {
                        label: Some("status".to_string()),
                        empty: false,
                        height: 1,
                    },
                    LuaContentNode::Code {
                        body: "done".to_string(),
                        kind: Some("lua".to_string()),
                    },
                ],
            }]
        );
    }

    #[test]
    fn ui_collects_row_column_split_and_table_nodes() {
        let lua = mlua::Lua::new();
        let nodes = Rc::new(RefCell::new(Vec::new()));
        let ui = build_ui_table(&lua, nodes.clone()).expect("build ui table");
        lua.globals().set("ui", ui).expect("set ui");

        lua.load(
            r#"
            ui.row(function(ui)
              ui.badge("ok")
              ui.text("ready")
            end)
            ui.column(function(ui)
              ui.text("line 1")
              ui.text("line 2")
            end)
            ui.split({ direction = "horizontal", ratio = 0.25, gap = 3 }, function(ui)
              ui.text("left")
            end, function(ui)
              ui.text("right")
            end)
            ui.table({
              { "Name", "Value" },
              { "alpha", 1 },
            })
        "#,
        )
        .exec()
        .expect("build ui nodes");

        assert_eq!(
            nodes.take(),
            vec![
                LuaContentNode::Row {
                    children: vec![
                        LuaContentNode::Badge("ok".to_string()),
                        LuaContentNode::Text("ready".to_string()),
                    ],
                },
                LuaContentNode::Column {
                    children: vec![
                        LuaContentNode::Text("line 1".to_string()),
                        LuaContentNode::Text("line 2".to_string()),
                    ],
                },
                LuaContentNode::Split {
                    direction: super::LuaSplitDirection::Horizontal,
                    ratio_millis: 250,
                    gap: 3,
                    left: vec![LuaContentNode::Text("left".to_string())],
                    right: vec![LuaContentNode::Text("right".to_string())],
                },
                LuaContentNode::Table {
                    rows: vec![
                        vec!["Name".to_string(), "Value".to_string()],
                        vec!["alpha".to_string(), "1".to_string()],
                    ],
                },
            ]
        );
    }
}
