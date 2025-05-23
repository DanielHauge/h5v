use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::{
    error::AppError,
    ui::state::{
        AppState,
        AttributeViewSelection::{Name, NameAndValue, Value},
    },
};

use super::EventResult;

pub fn handle_normal_attributes(
    state: &mut AppState<'_>,
    event: Event,
) -> Result<EventResult, AppError> {
    match event {
        Event::Key(key_event) => match key_event.kind {
            KeyEventKind::Press => match (key_event.code, key_event.modifiers) {
                (KeyCode::Up, _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut current_node = tree_item.node.borrow_mut();
                    let attributes_count =
                        current_node.read_attributes()?.rendered_attributes.len();
                    if state.attributes_view_cursor.attribute_index > 0 {
                        if state.attributes_view_cursor.attribute_index >= attributes_count {
                            state.attributes_view_cursor.attribute_index = attributes_count - 2;
                        } else {
                            state.attributes_view_cursor.attribute_index -= 1;
                        }
                        Ok(EventResult::Redraw)
                    } else {
                        state.attributes_view_cursor.attribute_index = 0;
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Down, _) => {
                    let tree_item = &state.treeview[state.tree_view_cursor];
                    let mut current_node = tree_item.node.borrow_mut();
                    let attributes_count =
                        current_node.read_attributes()?.rendered_attributes.len();

                    if state.attributes_view_cursor.attribute_index < attributes_count - 1 {
                        state.attributes_view_cursor.attribute_index += 1;
                        Ok(EventResult::Redraw)
                    } else {
                        state.attributes_view_cursor.attribute_index = attributes_count - 1;
                        Ok(EventResult::Continue)
                    }
                }
                (KeyCode::Left, _) => {
                    match state.attributes_view_cursor.attribute_view_selection {
                        Name => {}
                        NameAndValue => {
                            state.attributes_view_cursor.attribute_view_selection = Name;
                        }
                        Value => {
                            state.attributes_view_cursor.attribute_view_selection = NameAndValue;
                        }
                    }
                    Ok(EventResult::Redraw)
                }
                (KeyCode::Right, _) => {
                    match state.attributes_view_cursor.attribute_view_selection {
                        Name => {
                            state.attributes_view_cursor.attribute_view_selection = NameAndValue;
                        }
                        NameAndValue => {
                            state.attributes_view_cursor.attribute_view_selection = Value;
                        }
                        Value => {}
                    }
                    Ok(EventResult::Redraw)
                }

                _ => Ok(EventResult::Continue),
            },
            KeyEventKind::Repeat => Ok(EventResult::Continue),
            KeyEventKind::Release => Ok(EventResult::Continue),
        },
        Event::Resize(_, _) => Ok(EventResult::Redraw),
        _ => Ok(EventResult::Continue),
    }
}
