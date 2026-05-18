use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use serde_json::Value;

use crate::{configure, configure::registry::ContentModeHandle, error::AppError};

use super::state::AppState;

mod json;
mod lua;
mod render;
mod types;

pub(crate) use json::parse_ui_document;
pub(crate) use lua::build_ui_document_builder;
#[cfg(test)]
pub(crate) use lua::build_ui_table;
pub(crate) use render::render_ui_nodes;
#[cfg(test)]
pub(crate) use types::{LuaContentNode, LuaSplitDirection};

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
        let ctx = lua::build_render_context(lua, state)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        lua::collect_ui_nodes(lua, callback, Some(ctx))
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

pub(crate) fn render_serialized_ui_document(
    document: &str,
    width: usize,
) -> Result<Vec<Line<'static>>, String> {
    let value: Value = serde_json::from_str(document).map_err(|error| error.to_string())?;
    let nodes = json::nodes_from_json_value(&value)?;
    Ok(render_ui_nodes(&nodes, width))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::{build_ui_table, LuaContentNode, LuaSplitDirection};

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
        .expect("execute ui builder");

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

            ui.split(
              { direction = "horizontal", ratio = 0.25, gap = 3 },
              function(ui) ui.text("left") end,
              function(ui) ui.text("right") end
            )

            ui.table({
              { "name", "value" },
              { "alpha", "1" },
              { "beta", "2" },
            })
            "#,
        )
        .exec()
        .expect("execute ui builder");

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
                    direction: LuaSplitDirection::Horizontal,
                    ratio_millis: 250,
                    gap: 3,
                    left: vec![LuaContentNode::Text("left".to_string())],
                    right: vec![LuaContentNode::Text("right".to_string())],
                },
                LuaContentNode::Table {
                    rows: vec![
                        vec!["name".to_string(), "value".to_string()],
                        vec!["alpha".to_string(), "1".to_string()],
                        vec!["beta".to_string(), "2".to_string()],
                    ],
                },
            ]
        );
    }
}
