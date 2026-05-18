use serde_json::{Map, Value};

use crate::ui::custom_content::types::{LuaContentNode, LuaSplitDirection};

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

pub(super) fn nodes_to_json_value(nodes: &[LuaContentNode]) -> Value {
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

pub(crate) fn nodes_from_json_value(value: &Value) -> Result<Vec<LuaContentNode>, String> {
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
