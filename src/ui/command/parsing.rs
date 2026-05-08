use crate::error::AppError;

use super::{
    catalog::{command_catalog, find_command_descriptor, find_command_descriptor_by_id},
    CommandArgKind, CommandArgValue, CommandDescriptor, CommandId, CommandInvocation, CommandToken,
    StartupCommand,
};

pub fn parse_command_text(command_text: &str) -> Result<CommandInvocation, AppError> {
    let trimmed = command_text.trim();
    if trimmed.is_empty() {
        return Ok(CommandInvocation::noop(trimmed));
    }

    if let Some(invocation) = parse_legacy_numeric_alias(trimmed)? {
        return Ok(invocation);
    }

    let tokens = tokenize_command_text(trimmed)?;
    let command_name = tokens
        .first()
        .map(|token| token.value.as_str())
        .ok_or_else(|| AppError::InvalidCommand("Command was empty".to_string()))?;
    let descriptor = find_command_descriptor(command_name).ok_or_else(|| {
        let known = command_catalog()
            .iter()
            .map(|descriptor| descriptor.name)
            .collect::<Vec<_>>()
            .join(", ");
        AppError::InvalidCommand(format!(
            "Unknown command '{}'. Known commands: {}",
            command_name, known
        ))
    })?;

    let args = parse_command_args(descriptor, &tokens[1..])?;
    Ok(CommandInvocation {
        id: descriptor.id,
        raw_input: trimmed.to_string(),
        command_name: descriptor.name.to_string(),
        args,
    })
}

pub fn format_command_invocation(command: &CommandInvocation) -> String {
    if command.is_noop() {
        return String::new();
    }

    let command_name = find_command_descriptor_by_id(command.id)
        .map(|descriptor| descriptor.name)
        .unwrap_or(command.command_name.as_str());
    let args = command
        .args
        .iter()
        .map(format_command_arg)
        .collect::<Vec<_>>();

    if args.is_empty() {
        command_name.to_string()
    } else {
        format!("{} {}", command_name, args.join(" "))
    }
}

pub fn describe_command_invocation(command: &CommandInvocation) -> Option<&'static str> {
    find_command_descriptor_by_id(command.id).map(|descriptor| descriptor.description)
}

pub fn describe_command_descriptor(descriptor: &CommandDescriptor) -> String {
    let mut parts = vec![format!(
        "{} - {}",
        command_usage(descriptor),
        descriptor.description
    )];
    if !descriptor.aliases.is_empty() {
        parts.push(format!("aliases: {}", descriptor.aliases.join(", ")));
    }
    let keybindings = command_keybindings(descriptor);
    if !keybindings.is_empty() {
        parts.push(format!("keys: {keybindings}"));
    }
    parts.join(" | ")
}

pub fn parse_startup_commands(script: &str, origin: &str) -> Vec<StartupCommand> {
    let mut commands = Vec::new();
    let mut current = String::new();
    let mut line = 1usize;
    let mut segment = 1usize;
    let mut start_line = 1usize;
    let mut start_segment = 1usize;
    let mut in_quote = None;
    let mut escaped = false;

    for ch in script.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => {
                current.push(ch);
                escaped = true;
            }
            '"' | '\'' => {
                current.push(ch);
                if let Some(active_quote) = in_quote {
                    if active_quote == ch {
                        in_quote = None;
                    }
                } else {
                    in_quote = Some(ch);
                }
            }
            ';' if in_quote.is_none() => {
                push_startup_command(
                    &mut commands,
                    &mut current,
                    origin,
                    start_line,
                    start_segment,
                );
                segment += 1;
                start_line = line;
                start_segment = segment;
            }
            '\n' if in_quote.is_none() => {
                push_startup_command(
                    &mut commands,
                    &mut current,
                    origin,
                    start_line,
                    start_segment,
                );
                line += 1;
                segment = 1;
                start_line = line;
                start_segment = 1;
            }
            _ => current.push(ch),
        }
    }

    push_startup_command(
        &mut commands,
        &mut current,
        origin,
        start_line,
        start_segment,
    );
    commands
}

pub fn command_matches(buffer: &str) -> Vec<&'static CommandDescriptor> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return command_catalog().iter().collect();
    }

    if let Some(descriptor) = legacy_descriptor_for_input(trimmed) {
        return vec![descriptor];
    }

    let fragment = first_token(trimmed)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut matches = command_catalog()
        .iter()
        .filter(|descriptor| {
            descriptor.name.starts_with(&fragment)
                || descriptor
                    .aliases
                    .iter()
                    .any(|alias| alias.to_ascii_lowercase().starts_with(&fragment))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|descriptor| descriptor.name);
    matches
}

pub fn selected_command_descriptor(
    buffer: &str,
    selected_suggestion: usize,
) -> Option<&'static CommandDescriptor> {
    let matches = command_matches(buffer);
    if matches.is_empty() {
        None
    } else {
        Some(matches[selected_suggestion.min(matches.len() - 1)])
    }
}

pub fn current_command_descriptor(buffer: &str) -> Option<&'static CommandDescriptor> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return None;
    }
    legacy_descriptor_for_input(trimmed)
        .or_else(|| first_token(trimmed).and_then(find_command_descriptor))
}

pub fn command_usage(descriptor: &CommandDescriptor) -> String {
    let args = descriptor
        .args
        .iter()
        .map(|arg| {
            if arg.required {
                format!("<{}>", arg.name)
            } else {
                format!("[{}]", arg.name)
            }
        })
        .collect::<Vec<_>>();
    if args.is_empty() {
        descriptor.name.to_string()
    } else {
        format!("{} {}", descriptor.name, args.join(" "))
    }
}

pub fn command_keybindings(descriptor: &CommandDescriptor) -> String {
    descriptor.keybindings.join(", ")
}

pub(super) fn legacy_descriptor_for_input(
    command_text: &str,
) -> Option<&'static CommandDescriptor> {
    let first = command_text.chars().next()?;
    if first == '+' {
        find_command_descriptor_by_id(CommandId::Down)
    } else if first == '-' {
        find_command_descriptor_by_id(CommandId::Up)
    } else if command_text.chars().all(|c| c.is_ascii_digit()) {
        find_command_descriptor_by_id(CommandId::Seek)
    } else {
        None
    }
}

pub(super) fn tokenize_command_text(command_text: &str) -> Result<Vec<CommandToken>, AppError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = None;
    let mut current_quoted = false;
    let mut escaped = false;

    for ch in command_text.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => {
                escaped = true;
            }
            '"' | '\'' => {
                if let Some(active_quote) = in_quote {
                    if ch == active_quote {
                        in_quote = None;
                    } else {
                        current.push(ch);
                    }
                } else {
                    in_quote = Some(ch);
                    current_quoted = true;
                }
            }
            c if c.is_whitespace() && in_quote.is_none() => {
                if !current.is_empty() {
                    tokens.push(CommandToken {
                        value: std::mem::take(&mut current),
                        quoted: current_quoted,
                    });
                    current_quoted = false;
                }
            }
            _ => current.push(ch),
        }
    }

    if escaped {
        current.push('\\');
    }

    if let Some(quote) = in_quote {
        return Err(AppError::InvalidCommand(format!(
            "Unterminated quoted argument starting with {}",
            quote
        )));
    }

    if !current.is_empty() {
        tokens.push(CommandToken {
            value: current,
            quoted: current_quoted,
        });
    }

    Ok(tokens)
}

pub(super) fn command_tail(buffer: &str) -> Option<&str> {
    let trimmed = buffer.trim_start();
    let token = first_token(trimmed)?;
    let rest = &trimmed[token.len()..];
    Some(rest)
}

fn parse_legacy_numeric_alias(command_text: &str) -> Result<Option<CommandInvocation>, AppError> {
    let Some(first) = command_text.chars().next() else {
        return Ok(Some(CommandInvocation::noop(command_text)));
    };

    if first == '+' || first == '-' {
        let amount_text = command_text[1..].trim();
        if amount_text.is_empty() {
            return Err(AppError::InvalidCommand(format!(
                "Expected a number after '{}'",
                first
            )));
        }
        let amount = parse_usize_arg(amount_text, "amount", command_text)?;
        let (id, name) = if first == '+' {
            (CommandId::Down, "down")
        } else {
            (CommandId::Up, "up")
        };
        return Ok(Some(CommandInvocation {
            id,
            raw_input: command_text.to_string(),
            command_name: name.to_string(),
            args: vec![CommandArgValue::UnsignedInt(amount)],
        }));
    }

    if command_text.chars().all(|c| c.is_ascii_digit()) {
        let index = parse_usize_arg(command_text, "index", command_text)?;
        return Ok(Some(CommandInvocation {
            id: CommandId::Seek,
            raw_input: command_text.to_string(),
            command_name: "seek".to_string(),
            args: vec![CommandArgValue::UnsignedInt(index)],
        }));
    }

    Ok(None)
}

fn parse_command_args(
    descriptor: &CommandDescriptor,
    tokens: &[CommandToken],
) -> Result<Vec<CommandArgValue>, AppError> {
    let required_args = descriptor.args.iter().filter(|arg| arg.required).count();
    if tokens.len() < required_args {
        return Err(AppError::InvalidCommand(format!(
            "Command '{}' expects {} argument(s)",
            descriptor.name, required_args
        )));
    }
    if tokens.len() > descriptor.args.len() {
        return Err(AppError::InvalidCommand(format!(
            "Command '{}' received too many arguments",
            descriptor.name
        )));
    }

    descriptor
        .args
        .iter()
        .zip(tokens.iter())
        .map(|(arg_spec, token)| match arg_spec.kind {
            CommandArgKind::UnsignedInt => {
                parse_usize_arg(&token.value, arg_spec.name, descriptor.name)
                    .map(CommandArgValue::UnsignedInt)
            }
            CommandArgKind::Word => Ok(CommandArgValue::Word(token.value.clone())),
        })
        .collect()
}

fn parse_usize_arg(token: &str, arg_name: &str, command_name: &str) -> Result<usize, AppError> {
    token.parse::<usize>().map_err(|_| {
        AppError::InvalidCommand(format!(
            "Invalid {} '{}' for command '{}'",
            arg_name, token, command_name
        ))
    })
}

fn first_token(buffer: &str) -> Option<&str> {
    buffer.split_whitespace().next()
}

fn format_command_arg(arg: &CommandArgValue) -> String {
    match arg {
        CommandArgValue::UnsignedInt(value) => value.to_string(),
        CommandArgValue::Word(value) => {
            if value.is_empty()
                || value
                    .chars()
                    .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\\' | ';'))
            {
                format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                value.clone()
            }
        }
    }
}

fn push_startup_command(
    commands: &mut Vec<StartupCommand>,
    current: &mut String,
    origin: &str,
    line: usize,
    segment: usize,
) {
    let trimmed = current.trim();
    if !trimmed.is_empty() && !trimmed.starts_with('#') {
        commands.push(StartupCommand {
            origin: format_startup_origin(origin, line, segment),
            command_text: trimmed.to_string(),
        });
    }
    current.clear();
}

fn format_startup_origin(origin: &str, line: usize, segment: usize) -> String {
    if segment <= 1 {
        format!("{origin}:{line}")
    } else {
        format!("{origin}:{line}[{segment}]")
    }
}
