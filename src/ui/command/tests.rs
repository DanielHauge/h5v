use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::handlers::parse_simulated_key;
use super::parsing::tokenize_command_text;
use super::{
    describe_command_descriptor, describe_command_invocation, find_command_descriptor,
    format_command_invocation, parse_command_text, parse_startup_commands, CommandArgValue,
    CommandId, CommandState,
};

#[test]
fn parses_named_seek_command() {
    let command = parse_command_text("seek 42").expect("expected command to parse");
    assert_eq!(command.id, CommandId::Seek);
    assert_eq!(command.args, vec![CommandArgValue::UnsignedInt(42)]);
}

#[test]
fn parses_named_seek_command_with_second_index() {
    let command = parse_command_text("seek 25 35").expect("expected 2d seek command to parse");
    assert_eq!(command.id, CommandId::Seek);
    assert_eq!(
        command.args,
        vec![
            CommandArgValue::UnsignedInt(25),
            CommandArgValue::UnsignedInt(35),
        ]
    );
}

#[test]
fn parses_seek_row_command() {
    let command = parse_command_text("seek-row 35").expect("expected seek-row command");
    assert_eq!(command.id, CommandId::SeekRow);
    assert_eq!(command.args, vec![CommandArgValue::UnsignedInt(35)]);
}

#[test]
fn parses_seek_col_command() {
    let command = parse_command_text("seek-col 25").expect("expected seek-col command");
    assert_eq!(command.id, CommandId::SeekCol);
    assert_eq!(command.args, vec![CommandArgValue::UnsignedInt(25)]);
}

#[test]
fn parses_seek_page_command() {
    let command = parse_command_text("seek-page 3").expect("expected seek-page command");
    assert_eq!(command.id, CommandId::SeekPage);
    assert_eq!(command.args, vec![CommandArgValue::UnsignedInt(3)]);
}

#[test]
fn parses_goto_command_with_path_argument() {
    let command = parse_command_text("goto /group/dataset").expect("expected goto command");
    assert_eq!(command.id, CommandId::Goto);
    assert_eq!(
        command.args,
        vec![CommandArgValue::Word("/group/dataset".to_string())]
    );
}

#[test]
fn parses_goto_command_with_quoted_path_argument() {
    let command =
        parse_command_text(r#"goto "/group/my dataset""#).expect("expected quoted goto command");
    assert_eq!(command.id, CommandId::Goto);
    assert_eq!(
        command.args,
        vec![CommandArgValue::Word("/group/my dataset".to_string())]
    );
}

#[test]
fn parses_legacy_relative_aliases() {
    let down = parse_command_text("+7").expect("expected +7 to parse");
    assert_eq!(down.id, CommandId::Down);
    assert_eq!(down.args, vec![CommandArgValue::UnsignedInt(7)]);

    let up = parse_command_text("-3").expect("expected -3 to parse");
    assert_eq!(up.id, CommandId::Up);
    assert_eq!(up.args, vec![CommandArgValue::UnsignedInt(3)]);
}

#[test]
fn parses_legacy_absolute_alias() {
    let command = parse_command_text("9").expect("expected 9 to parse");
    assert_eq!(command.id, CommandId::Seek);
    assert_eq!(command.args, vec![CommandArgValue::UnsignedInt(9)]);
}

#[test]
fn rejects_unknown_commands() {
    let error = parse_command_text("teleport 4").expect_err("expected parse error");
    assert!(error.to_string().contains("Unknown command"));
}

#[test]
fn tokenizes_quoted_arguments() {
    let tokens = tokenize_command_text(r#"seek "42""#).expect("expected quoted tokens to parse");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].value, "seek");
    assert_eq!(tokens[1].value, "42");
    assert!(tokens[1].quoted);
}

#[test]
fn rejects_unterminated_quotes() {
    let error = tokenize_command_text(r#"seek "42"#).expect_err("expected quote error");
    assert!(error.to_string().contains("Unterminated quoted argument"));
}

#[test]
fn finds_legacy_descriptor_matches() {
    let matches = super::command_matches("+4");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "down");
}

#[test]
fn parses_focus_command_with_word_argument() {
    let command = parse_command_text("focus content").expect("expected focus command");
    assert_eq!(command.id, CommandId::Focus);
    assert_eq!(
        command.args,
        vec![CommandArgValue::Word("content".to_string())]
    );
}

#[test]
fn parses_index_command_with_optional_amount() {
    let command = parse_command_text("index prev 10").expect("expected index command");
    assert_eq!(command.id, CommandId::Index);
    assert_eq!(
        command.args,
        vec![
            CommandArgValue::Word("prev".to_string()),
            CommandArgValue::UnsignedInt(10)
        ]
    );
}

#[test]
fn parses_multichart_add_command_with_dataset_spec() {
    let command =
        parse_command_text("mchart add load(/group/dataset)[..,0]").expect("expected mchart add");
    assert_eq!(command.id, CommandId::MultiChart);
    assert_eq!(
        command.args,
        vec![
            CommandArgValue::Word("add".to_string()),
            CommandArgValue::Word("load(/group/dataset)[..,0]".to_string()),
        ]
    );
}

#[test]
fn parses_multichart_expression_command_with_quoted_expression() {
    let command = parse_command_text(r#"mchart expr "($1, load(/ticks) + load(/OFFSET))""#)
        .expect("expected mchart expr");
    assert_eq!(command.id, CommandId::MultiChart);
    assert_eq!(
        command.args,
        vec![
            CommandArgValue::Word("expr".to_string()),
            CommandArgValue::Word("($1, load(/ticks) + load(/OFFSET))".to_string()),
        ]
    );
}

#[test]
fn parses_press_command_with_multiple_keys() {
    let command = parse_command_text("press ctrl+w o").expect("expected press command");
    assert_eq!(command.id, CommandId::Press);
    assert_eq!(
        command.args,
        vec![
            CommandArgValue::Word("ctrl+w".to_string()),
            CommandArgValue::Word("o".to_string()),
        ]
    );
}

#[test]
fn parses_shift_tab_key_spec() {
    let key = parse_simulated_key("shift+tab").expect("shift+tab key");
    assert_eq!(key.code, KeyCode::BackTab);
    assert!(key.modifiers.contains(KeyModifiers::SHIFT));
}

#[test]
fn history_navigation_restores_draft() {
    let mut state = CommandState {
        command_buffer: "see".to_string(),
        cursor: 3,
        last_command: None,
        selected_suggestion: 0,
        history: std::collections::VecDeque::from(["seek 1".to_string(), "down 3".to_string()]),
        history_cursor: None,
        history_draft: None,
    };

    assert!(state.select_previous_history());
    assert_eq!(state.command_buffer, "down 3");
    assert!(state.select_previous_history());
    assert_eq!(state.command_buffer, "seek 1");
    assert!(state.select_next_history());
    assert_eq!(state.command_buffer, "down 3");
    assert!(state.select_next_history());
    assert_eq!(state.command_buffer, "see");
    assert!(state.history_cursor.is_none());
}

#[test]
fn apply_selected_suggestion_completes_partial_command_name() {
    let mut state = CommandState {
        command_buffer: "mc".to_string(),
        cursor: 2,
        last_command: None,
        selected_suggestion: 0,
        history: std::collections::VecDeque::new(),
        history_cursor: None,
        history_draft: None,
    };

    assert!(state.apply_selected_suggestion());
    assert_eq!(state.command_buffer, "mchart ");
    assert_eq!(state.cursor, state.command_buffer.len());
}

#[test]
fn parses_startup_script_lines_with_comments() {
    let commands = parse_startup_commands("\n# comment\nseek 1\n  down 2  \n", "stdin");
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].origin, "stdin:3");
    assert_eq!(commands[0].command_text, "seek 1");
    assert_eq!(commands[1].origin, "stdin:4");
    assert_eq!(commands[1].command_text, "down 2");
}

#[test]
fn parses_startup_script_semicolon_segments() {
    let commands = parse_startup_commands("seek 1; down 2\nmode preview", "script.h5v");
    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].origin, "script.h5v:1");
    assert_eq!(commands[1].origin, "script.h5v:1[2]");
    assert_eq!(commands[1].command_text, "down 2");
    assert_eq!(commands[2].origin, "script.h5v:2");
}

#[test]
fn keeps_semicolons_inside_quoted_startup_commands() {
    let commands = parse_startup_commands(r#"focus "content; pane"; down 2"#, "stdin");
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].command_text, r#"focus "content; pane""#);
    assert_eq!(commands[1].command_text, "down 2");
}

#[test]
fn formats_alias_commands_using_canonical_names() {
    let command = parse_command_text("+7").expect("expected +7 to parse");
    assert_eq!(format_command_invocation(&command), "down 7");
    assert_eq!(
        describe_command_invocation(&command),
        Some("Move down by a relative amount")
    );
}

#[test]
fn parses_help_command_with_optional_target() {
    let command = parse_command_text("help reload").expect("expected help command");
    assert_eq!(command.id, CommandId::Help);
    assert_eq!(
        command.args,
        vec![CommandArgValue::Word("reload".to_string())]
    );
}

#[test]
fn parses_attr_create_command() {
    let command =
        parse_command_text(r#"attr create title string "hello world""#).expect("attr create");
    assert_eq!(command.id, CommandId::Attr);
    assert_eq!(
        command.args,
        vec![
            CommandArgValue::Word("create".to_string()),
            CommandArgValue::Word("title".to_string()),
            CommandArgValue::Word("string".to_string()),
            CommandArgValue::Word("hello world".to_string()),
        ]
    );
}

#[test]
fn parses_attr_delete_command() {
    let command = parse_command_text("attr delete title").expect("attr delete");
    assert_eq!(command.id, CommandId::Attr);
    assert_eq!(
        command.args,
        vec![
            CommandArgValue::Word("delete".to_string()),
            CommandArgValue::Word("title".to_string()),
        ]
    );
}

#[test]
fn describes_command_descriptor_with_aliases_and_keys() {
    let descriptor = find_command_descriptor("reload").expect("reload descriptor");
    let description = describe_command_descriptor(descriptor);
    assert!(description.contains("reload"));
    assert!(description.contains("refresh"));
    assert!(description.contains("Ctrl+r"));
}

#[test]
fn parses_configure_command() {
    let command = parse_command_text("configure").expect("configure command");
    assert_eq!(command.id, CommandId::Configure);
    assert!(command.args.is_empty());
}

#[test]
fn parses_configure_reset_command() {
    let command = parse_command_text("configure reset").expect("configure reset command");
    assert_eq!(command.id, CommandId::Configure);
    assert_eq!(
        command.args,
        vec![CommandArgValue::Word("reset".to_string())]
    );
}

#[test]
fn repeat_does_not_replace_last_command() {
    let mut state = CommandState {
        command_buffer: String::new(),
        cursor: 0,
        last_command: Some(parse_command_text("down 3").expect("down command")),
        selected_suggestion: 0,
        history: std::collections::VecDeque::new(),
        history_cursor: None,
        history_draft: None,
    };

    let repeat = parse_command_text("repeat").expect("repeat command");
    state.record_successful_command(&repeat);
    assert_eq!(
        state.last_command.expect("last command").command_name,
        "down"
    );
}
