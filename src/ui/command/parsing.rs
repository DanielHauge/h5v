use crate::{
    configure::{
        self,
        registry::{CommandArgMetadata, CommandArgValueKind, CommandMetadata, CommandVisibility},
    },
    error::AppError,
};

use super::{
    builtin_command_handle,
    catalog::{command_catalog, find_command_descriptor},
    find_command_descriptor_by_handle, CommandArgKind, CommandArgValue, CommandDescriptor,
    CommandId, CommandInvocation, CommandToken, StartupCommand,
};

pub fn parse_command_text(command_text: &str) -> Result<CommandInvocation, AppError> {
    let trimmed = command_text.trim();
    if trimmed.is_empty() {
        return Ok(CommandInvocation::noop(trimmed));
    }

    let tokens = tokenize_command_text(trimmed)?;
    let command_name = tokens
        .first()
        .map(|token| token.value.as_str())
        .ok_or_else(|| AppError::InvalidCommand("Command was empty".to_string()))?;
    let metadata = command_metadata(command_name).ok_or_else(|| {
        let known = command_names().join(", ");
        AppError::InvalidCommand(format!(
            "Unknown command '{}'. Known commands: {}",
            command_name, known
        ))
    })?;

    let args = parse_command_args(&metadata, &tokens[1..])?;
    Ok(CommandInvocation {
        handle: metadata.handle.clone(),
        id: find_command_descriptor_by_handle(&metadata.handle)
            .map(|descriptor| descriptor.id)
            .unwrap_or(CommandId::Custom),
        raw_input: trimmed.to_string(),
        command_name: metadata.name,
        args,
    })
}

pub fn format_command_invocation(command: &CommandInvocation) -> String {
    if command.is_noop() {
        return String::new();
    }

    let command_name = find_command_descriptor_by_handle(&command.handle)
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

pub fn describe_command_invocation(command: &CommandInvocation) -> Option<String> {
    command_metadata_by_handle(&command.handle).map(|metadata| metadata.summary)
}

#[cfg(test)]
pub(super) fn describe_command_descriptor(descriptor: &CommandDescriptor) -> String {
    let metadata = builtin_descriptor_metadata(descriptor);
    describe_command_metadata(&metadata)
}

pub fn describe_command_metadata(metadata: &CommandMetadata) -> String {
    let mut parts = vec![format!(
        "{} - {}",
        command_usage_metadata(metadata),
        metadata.summary
    )];
    if !metadata.aliases.is_empty() {
        parts.push(format!("aliases: {}", metadata.aliases.join(", ")));
    }
    let keybindings = command_keybindings_metadata(metadata);
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

pub fn command_matches(buffer: &str) -> Vec<CommandMetadata> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        let mut commands = command_catalog_metadata();
        commands.sort_by(|left, right| left.name.cmp(&right.name));
        return commands;
    }

    let fragment = first_token(trimmed)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut matches = command_catalog_metadata()
        .into_iter()
        .filter(|metadata| {
            metadata.name.starts_with(&fragment)
                || metadata
                    .aliases
                    .iter()
                    .any(|alias| alias.to_ascii_lowercase().starts_with(&fragment))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.name.cmp(&right.name));
    matches
}

pub fn selected_command_metadata(
    buffer: &str,
    selected_suggestion: usize,
) -> Option<CommandMetadata> {
    let matches = command_matches(buffer);
    if matches.is_empty() {
        None
    } else {
        Some(matches[selected_suggestion.min(matches.len() - 1)].clone())
    }
}

pub fn current_command_metadata(buffer: &str) -> Option<CommandMetadata> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return None;
    }
    first_token(trimmed).and_then(command_metadata)
}

pub fn command_usage_metadata(metadata: &CommandMetadata) -> String {
    let args = metadata
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
        metadata.name.to_string()
    } else {
        format!("{} {}", metadata.name, args.join(" "))
    }
}

pub fn command_keybindings_metadata(metadata: &CommandMetadata) -> String {
    configure::current_registry_snapshot()
        .command(&metadata.handle)
        .map(|live| live.keybindings.join(", "))
        .filter(|keybindings| !keybindings.is_empty())
        .unwrap_or_else(|| metadata.keybindings.join(", "))
}

fn builtin_descriptor_metadata(descriptor: &CommandDescriptor) -> CommandMetadata {
    configure::current_registry_snapshot()
        .command(&builtin_command_handle(descriptor.name))
        .cloned()
        .unwrap_or_else(|| CommandMetadata {
            handle: builtin_command_handle(descriptor.name),
            name: descriptor.name.to_string(),
            aliases: descriptor
                .aliases
                .iter()
                .map(|alias| (*alias).to_string())
                .collect(),
            summary: descriptor.description.to_string(),
            category: format!("{:?}", descriptor.category),
            keybindings: descriptor
                .keybindings
                .iter()
                .map(|binding| (*binding).to_string())
                .collect(),
            callback_id: None,
            args: descriptor
                .args
                .iter()
                .map(|arg| CommandArgMetadata {
                    name: arg.name.to_string(),
                    kind: match arg.kind {
                        CommandArgKind::UnsignedInt => CommandArgValueKind::UnsignedInt,
                        CommandArgKind::Word => CommandArgValueKind::Word,
                    },
                    required: arg.required,
                    help: arg.help.to_string(),
                    values: arg
                        .values
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                })
                .collect(),
            examples: vec![descriptor.example.to_string()],
            visibility: CommandVisibility::Visible,
            owner: configure::registry::RegistryOwner::Builtin,
        })
}

fn command_catalog_metadata() -> Vec<CommandMetadata> {
    let snapshot = configure::current_registry_snapshot();
    let mut commands = snapshot.commands().cloned().collect::<Vec<_>>();
    if commands.is_empty() {
        commands = command_catalog()
            .iter()
            .map(builtin_descriptor_metadata)
            .collect();
    }
    commands
        .into_iter()
        .filter(|metadata| metadata.visibility == CommandVisibility::Visible)
        .collect()
}

pub fn command_metadata(name: &str) -> Option<CommandMetadata> {
    let snapshot = configure::current_registry_snapshot();
    snapshot
        .find_command(name)
        .cloned()
        .or_else(|| {
            snapshot
                .command(&crate::configure::registry::CommandHandle::new(name))
                .cloned()
        })
        .or_else(|| find_command_descriptor(name).map(builtin_descriptor_metadata))
}

pub fn command_metadata_by_handle(
    handle: &crate::configure::registry::CommandHandle,
) -> Option<CommandMetadata> {
    let snapshot = configure::current_registry_snapshot();
    snapshot
        .command(handle)
        .cloned()
        .or_else(|| find_command_descriptor_by_handle(handle).map(builtin_descriptor_metadata))
}

fn command_names() -> Vec<String> {
    let mut names = command_catalog_metadata()
        .into_iter()
        .map(|metadata| metadata.name)
        .collect::<Vec<_>>();
    names.sort();
    names
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

fn parse_command_args(
    metadata: &CommandMetadata,
    tokens: &[CommandToken],
) -> Result<Vec<CommandArgValue>, AppError> {
    let required_args = metadata.args.iter().filter(|arg| arg.required).count();
    if tokens.len() < required_args {
        return Err(AppError::InvalidCommand(format!(
            "Command '{}' expects {} argument(s)",
            metadata.name, required_args
        )));
    }
    if tokens.len() > metadata.args.len() {
        return Err(AppError::InvalidCommand(format!(
            "Command '{}' received too many arguments",
            metadata.name
        )));
    }

    metadata
        .args
        .iter()
        .zip(tokens.iter())
        .map(|(arg_spec, token)| match arg_spec.kind {
            CommandArgValueKind::UnsignedInt => {
                parse_usize_arg(&token.value, &arg_spec.name, &metadata.name)
                    .map(CommandArgValue::UnsignedInt)
            }
            CommandArgValueKind::Word => Ok(CommandArgValue::Word(token.value.clone())),
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
