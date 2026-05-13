use crate::{error::AppError, h5f::AttributeCreateType, ui::mchart::BuiltinDerivedOp};
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::super::{
    input::{handle_input_event, EventResult},
    state::{AppState, AppToast, ContentShowMode, Focus, Mode},
};
use super::{
    execute_command, find_command_descriptor,
    parsing::{describe_command_descriptor, legacy_descriptor_for_input},
    CommandArgValue, CommandInvocation,
};

pub(super) fn handle_seek(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.set(command.usize_arg(0)?)
}

pub(super) fn handle_goto(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.select_tree_node_by_path(command.word_arg(0)?)?;
    Ok(EventResult::Redraw)
}

pub(super) fn handle_up(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.up(command.usize_arg_or(0, 1)?)
}

pub(super) fn handle_down(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.down(command.usize_arg_or(0, 1)?)
}

pub(super) fn handle_left(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.left(command.usize_arg_or(0, 1)? as isize)
}

pub(super) fn handle_right(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.right(command.usize_arg_or(0, 1)? as isize)
}

pub(super) fn handle_page_up(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.up(20)
}

pub(super) fn handle_page_down(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.down(20)
}

pub(super) fn handle_focus(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    match command.word_arg(0)?.to_ascii_lowercase().as_str() {
        "tree" => {
            if !state.show_tree_view {
                return Err(AppError::InvalidCommand(
                    "Cannot focus tree while the tree view is hidden".to_string(),
                ));
            }
            state.focus_tree_from_current();
            Ok(EventResult::Redraw)
        }
        "attributes" | "attrs" => {
            state.focus = Focus::Attributes;
            Ok(EventResult::Redraw)
        }
        "content" => {
            state.focus = Focus::Content;
            Ok(EventResult::Redraw)
        }
        target => Err(AppError::InvalidCommand(format!(
            "Unknown focus target '{}'. Expected tree, attributes, or content",
            target
        ))),
    }
}

pub(super) fn handle_mode(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let requested_mode = match command.word_arg(0)?.to_ascii_lowercase().as_str() {
        "preview" => ContentShowMode::Preview,
        "matrix" => ContentShowMode::Matrix,
        mode => {
            return Err(AppError::InvalidCommand(format!(
                "Unknown content mode '{}'. Expected preview or matrix",
                mode
            )))
        }
    };

    let available = state.treeview[state.tree_view_cursor]
        .node
        .borrow_mut()
        .content_show_modes();
    if !available.contains(&requested_mode) {
        return Err(AppError::InvalidCommand(format!(
            "Mode '{}' is not available for the selected item",
            command.word_arg(0)?
        )));
    }
    state.set_content_mode(requested_mode);
    Ok(EventResult::Redraw)
}

pub(super) fn handle_toggle_tree(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.toggle_tree_view();
    Ok(EventResult::Redraw)
}

pub(super) fn handle_reload(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    Ok(EventResult::ReloadFile {
        write: !state.readonly,
    })
}

pub(super) fn handle_configure(
    _state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let reset = match command.word_arg_optional(0)? {
        None => false,
        Some(action) if action.eq_ignore_ascii_case("reset") => true,
        Some(action) => {
            return Err(AppError::InvalidCommand(format!(
                "Unknown configure action '{action}'. Supported actions: reset"
            )));
        }
    };
    Ok(EventResult::Configure { reset })
}

pub(super) fn handle_x(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.change_x(parse_direction_delta(command.word_arg(0)?)?)
}

pub(super) fn handle_row(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.change_row(parse_direction_delta(command.word_arg(0)?)?)
}

pub(super) fn handle_col(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.change_col(parse_direction_delta(command.word_arg(0)?)?)
}

pub(super) fn handle_dim(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    state.change_selected_dimension(parse_direction_delta(command.word_arg(0)?)?)
}

pub(super) fn handle_index(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let delta = parse_direction_delta(command.word_arg(0)?)?;
    let amount = command.usize_arg_or(1, 1)? as isize;
    state.change_selected_index(delta.signum() * amount)
}

pub(super) fn handle_mchart(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let action = command.word_arg(0)?.to_ascii_lowercase();
    match action.as_str() {
        "open" | "show" => {
            state.mode = Mode::MultiChart;
            Ok(EventResult::Redraw)
        }
        "close" | "hide" => {
            state.multi_chart.close_expression_prompt();
            state.mode = Mode::Normal;
            Ok(EventResult::Redraw)
        }
        "toggle" => {
            state.multi_chart.close_expression_prompt();
            state.mode = if matches!(state.mode, Mode::MultiChart) {
                Mode::Normal
            } else {
                Mode::MultiChart
            };
            Ok(EventResult::Redraw)
        }
        "add" => {
            if let Some(spec) = command.word_arg_optional(1)? {
                let file = state.file.clone();
                state
                    .multi_chart
                    .add_dataset_reference_command(spec, file.as_ref())
                    .map_err(AppError::InvalidCommand)?;
            } else {
                let Some((source, points)) = state.capture_multichart_item()? else {
                    return Err(AppError::InvalidCommand(
                        "The current selection is not previewable as a multichart chart item"
                            .to_string(),
                    ));
                };
                state.multi_chart.add_chart_item(source, points);
            }
            state.compute_tree_view();
            Ok(EventResult::Redraw)
        }
        "expr" | "expression" => {
            let expression = command.word_arg(1)?.to_string();
            let file = state.file.clone();
            state
                .multi_chart
                .create_expression_derived_command(expression, file.as_ref())
                .map_err(AppError::InvalidCommand)?;
            Ok(EventResult::Redraw)
        }
        "prompt" => {
            state.mode = Mode::MultiChart;
            state.multi_chart.open_expression_prompt();
            Ok(EventResult::Redraw)
        }
        "base" => match command.word_arg_optional(1)?.map(|arg| arg.to_ascii_lowercase()) {
            None => {
                state
                    .multi_chart
                    .toggle_marked_base()
                    .map_err(AppError::InvalidCommand)?;
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "toggle" => {
                state
                    .multi_chart
                    .toggle_marked_base()
                    .map_err(AppError::InvalidCommand)?;
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "clear" => {
                state.multi_chart.clear_marked_base();
                Ok(EventResult::Redraw)
            }
            Some(other) => Err(AppError::InvalidCommand(format!(
                "Unknown mchart base action '{}'. Expected toggle or clear",
                other
            ))),
        },
        "derive" => {
            let operation = parse_multichart_derived_op(command.word_arg(1)?)?;
            state
                .multi_chart
                .create_builtin_derived(operation)
                .map_err(AppError::InvalidCommand)?;
            Ok(EventResult::Redraw)
        }
        "select" | "move" => {
            let delta = parse_direction_delta(command.word_arg(1)?)?;
            let amount = parse_word_usize(command.word_arg_optional(2)?, 1, "mchart")?;
            for _ in 0..amount {
                if delta < 0 {
                    state.multi_chart.move_up();
                } else {
                    state.multi_chart.move_down();
                }
            }
            Ok(EventResult::Redraw)
        }
        "visible" | "visibility" => match command
            .word_arg_optional(1)?
            .map(|arg| arg.to_ascii_lowercase())
        {
            None => {
                state.multi_chart.toggle_selected_visible();
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "toggle" => {
                state.multi_chart.toggle_selected_visible();
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "show" => {
                state.multi_chart.set_selected_visible(true);
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "hide" => {
                state.multi_chart.set_selected_visible(false);
                Ok(EventResult::Redraw)
            }
            Some(other) => Err(AppError::InvalidCommand(format!(
                "Unknown mchart visibility action '{}'. Expected toggle, show, or hide",
                other
            ))),
        },
        "remove" | "delete" => {
            state.multi_chart.clear_selected();
            state.compute_tree_view();
            Ok(EventResult::Redraw)
        }
        "clear" => match command.word_arg_optional(1)?.map(|arg| arg.to_ascii_lowercase()) {
            None => {
                state.multi_chart.clear_all();
                state.compute_tree_view();
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "all" => {
                state.multi_chart.clear_all();
                state.compute_tree_view();
                Ok(EventResult::Redraw)
            }
            Some(action) if action == "zoom" => {
                state.multi_chart.clear_zoom();
                Ok(EventResult::Redraw)
            }
            Some(other) => Err(AppError::InvalidCommand(format!(
                "Unknown mchart clear target '{}'. Expected all or zoom",
                other
            ))),
        },
        "zoom" => {
            let target = command.word_arg_optional(1)?.unwrap_or("reset");
            match target.to_ascii_lowercase().as_str() {
                "in" => {
                    state
                        .multi_chart
                        .zoom_in(parse_word_f64(command.word_arg_optional(2)?, 10.0, "mchart")?);
                    Ok(EventResult::Redraw)
                }
                "out" => {
                    state
                        .multi_chart
                        .zoom_out(parse_word_f64(command.word_arg_optional(2)?, 10.0, "mchart")?);
                    Ok(EventResult::Redraw)
                }
                "reset" | "clear" => {
                    state.multi_chart.clear_zoom();
                    Ok(EventResult::Redraw)
                }
                other => Err(AppError::InvalidCommand(format!(
                    "Unknown mchart zoom action '{}'. Expected in, out, or reset",
                    other
                ))),
            }
        }
        "pan" => {
            let direction = command.word_arg(1)?;
            let amount = parse_word_f64(command.word_arg_optional(2)?, 10.0, "mchart")?;
            match direction.to_ascii_lowercase().as_str() {
                "left" => {
                    state.multi_chart.pan_left(amount);
                    Ok(EventResult::Redraw)
                }
                "right" => {
                    state.multi_chart.pan_right(amount);
                    Ok(EventResult::Redraw)
                }
                other => Err(AppError::InvalidCommand(format!(
                    "Unknown mchart pan direction '{}'. Expected left or right",
                    other
                ))),
            }
        }
        other => Err(AppError::InvalidCommand(format!(
            "Unknown mchart action '{}'. Expected open, close, add, expr, derive, select, visible, remove, clear, zoom, or pan",
            other
        ))),
    }
}

pub(super) fn handle_help(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let Some(CommandArgValue::Word(target)) = command.args.first() else {
        state.mode = Mode::Help;
        return Ok(EventResult::Redraw);
    };

    let descriptor = legacy_descriptor_for_input(target)
        .or_else(|| find_command_descriptor(target))
        .ok_or_else(|| {
            AppError::InvalidCommand(format!(
                "Unknown command '{}'. Try 'help' to open the command reference",
                target
            ))
        })?;

    Ok(EventResult::Toast(
        AppToast::Info(describe_command_descriptor(descriptor)),
        false,
    ))
}

pub(super) fn handle_attr(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    if state.readonly {
        return Err(AppError::EditError(
            "Cannot edit in read-only mode; reopen with -w to modify the file".to_string(),
        ));
    }

    let action = command.word_arg(0)?.to_ascii_lowercase();
    match action.as_str() {
        "create" | "add" | "new" => {
            let attr_name = command.word_arg(1)?;
            let attr_type = AttributeCreateType::parse(command.word_arg(2)?)?;
            let value = command.word_arg_optional(3)?.unwrap_or("");
            let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
            let created_type = selected_node.create_attribute(attr_name, attr_type, value)?;
            drop(selected_node);
            state.acknowledge_file_write();
            Ok(EventResult::Toast(
                AppToast::Info(format!(
                    "Created attribute '{}' ({})",
                    attr_name, created_type
                )),
                true,
            ))
        }
        "delete" | "remove" | "rm" => {
            let attr_name = command.word_arg(1)?;
            let mut selected_node = state.treeview[state.tree_view_cursor].node.borrow_mut();
            selected_node.delete_attribute(attr_name)?;
            drop(selected_node);
            state.acknowledge_file_write();
            Ok(EventResult::Toast(
                AppToast::Info(format!("Deleted attribute '{}'", attr_name)),
                true,
            ))
        }
        other => Err(AppError::InvalidCommand(format!(
            "Unknown attr action '{}'. Expected create or delete",
            other
        ))),
    }
}

pub(super) fn handle_repeat(
    state: &mut AppState<'_>,
    _command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let Some(last_command) = state.command_state.last_command.clone() else {
        return Err(AppError::InvalidCommand(
            "No previous command to repeat".to_string(),
        ));
    };
    execute_command(state, &last_command)
}

pub(super) fn handle_press(
    state: &mut AppState<'_>,
    command: &CommandInvocation,
) -> Result<EventResult, AppError> {
    let mut last_result = EventResult::Redraw;
    let mut pressed_any = false;
    for index in 0..4 {
        let Some(key_spec) = command.word_arg_optional(index)? else {
            continue;
        };
        pressed_any = true;
        let key_event = parse_simulated_key(key_spec)?;
        let result = handle_input_event(state, Event::Key(key_event))?;
        match result {
            EventResult::Continue => {}
            EventResult::Quit
            | EventResult::Configure { .. }
            | EventResult::ReloadFile { .. }
            | EventResult::Error(_)
            | EventResult::Toast(_, _)
            | EventResult::Copying => return Ok(result),
            EventResult::Redraw => last_result = EventResult::Redraw,
        }
    }

    if !pressed_any {
        return Err(AppError::InvalidCommand(
            "Command 'press' expects at least one key".to_string(),
        ));
    }

    Ok(last_result)
}

fn parse_direction_delta(word: &str) -> Result<isize, AppError> {
    match word.to_ascii_lowercase().as_str() {
        "next" | "forward" | "right" | "down" => Ok(1),
        "prev" | "previous" | "back" | "left" | "up" => Ok(-1),
        other => Err(AppError::InvalidCommand(format!(
            "Unknown direction '{}'. Expected next or prev",
            other
        ))),
    }
}

fn parse_multichart_derived_op(word: &str) -> Result<BuiltinDerivedOp, AppError> {
    match word.to_ascii_lowercase().as_str() {
        "diff" | "difference" => Ok(BuiltinDerivedOp::Difference),
        "sum" => Ok(BuiltinDerivedOp::Sum),
        "ratio" => Ok(BuiltinDerivedOp::Ratio),
        "product" | "mul" | "multiply" => Ok(BuiltinDerivedOp::Product),
        "xy" | "x-y" | "pair" => Ok(BuiltinDerivedOp::Xy),
        other => Err(AppError::InvalidCommand(format!(
            "Unknown multichart derived operation '{}'. Expected difference, sum, ratio, product, or xy",
            other
        ))),
    }
}

fn parse_word_usize(
    word: Option<&str>,
    default: usize,
    command_name: &str,
) -> Result<usize, AppError> {
    match word {
        Some(word) => word.parse::<usize>().map_err(|_| {
            AppError::InvalidCommand(format!(
                "Invalid numeric argument '{}' for command '{}'",
                word, command_name
            ))
        }),
        None => Ok(default),
    }
}

fn parse_word_f64(word: Option<&str>, default: f64, command_name: &str) -> Result<f64, AppError> {
    match word {
        Some(word) => word.parse::<f64>().map_err(|_| {
            AppError::InvalidCommand(format!(
                "Invalid numeric argument '{}' for command '{}'",
                word, command_name
            ))
        }),
        None => Ok(default),
    }
}

pub(super) fn parse_simulated_key(key_spec: &str) -> Result<KeyEvent, AppError> {
    let normalized = key_spec.trim();
    if normalized.is_empty() {
        return Err(AppError::InvalidCommand(
            "Key spec cannot be empty".to_string(),
        ));
    }

    let mut modifiers = KeyModifiers::NONE;
    let mut parts = normalized.split('+').peekable();
    let mut base = None;
    while let Some(part) = parts.next() {
        let part = part.trim();
        if part.is_empty() {
            return Err(AppError::InvalidCommand(format!(
                "Invalid key spec '{}'",
                key_spec
            )));
        }
        let lower = part.to_ascii_lowercase();
        if parts.peek().is_some() {
            match lower.as_str() {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" | "meta" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                _ => {
                    return Err(AppError::InvalidCommand(format!(
                        "Unknown key modifier '{}' in '{}'",
                        part, key_spec
                    )))
                }
            }
        } else {
            base = Some(part);
        }
    }

    let base = base
        .ok_or_else(|| AppError::InvalidCommand(format!("Missing base key in '{}'", key_spec)))?;
    let key_code = match base.to_ascii_lowercase().as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" | "shift-tab" => {
            modifiers |= KeyModifiers::SHIFT;
            KeyCode::BackTab
        }
        "space" => KeyCode::Char(' '),
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "page-up" | "pgup" => KeyCode::PageUp,
        "pagedown" | "page-down" | "pgdown" => KeyCode::PageDown,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        value if value.len() == 1 => {
            let ch = base.chars().next().ok_or_else(|| {
                AppError::InvalidCommand(format!("Invalid key spec '{}'", key_spec))
            })?;
            KeyCode::Char(ch)
        }
        _ => {
            return Err(AppError::InvalidCommand(format!(
                "Unknown key '{}' in '{}'",
                base, key_spec
            )))
        }
    };

    let key_code = if key_code == KeyCode::Tab && modifiers.contains(KeyModifiers::SHIFT) {
        KeyCode::BackTab
    } else {
        key_code
    };

    Ok(KeyEvent::new(key_code, modifiers))
}
