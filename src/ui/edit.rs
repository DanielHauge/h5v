use std::{
    env,
    fs::File,
    io::{stdout, Read, Write},
    path::Path,
    process::Command,
};

use ratatui::crossterm::{
    cursor::{Hide, SetCursorStyle, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
#[cfg(not(target_os = "windows"))]
use shell_words::split as shell_split;
use tempfile::{Builder, NamedTempFile};

use crate::{error::AppError, ui::state::AppState};

pub fn leave_h5v() -> Result<(), AppError> {
    stdout().execute(Show)?;
    stdout().execute(SetCursorStyle::DefaultUserShape)?;
    stdout().execute(DisableMouseCapture)?;
    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    ratatui::restore();
    Ok(())
}

pub fn reenter_h5v() -> Result<(), AppError> {
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableMouseCapture)?;
    stdout().execute(SetCursorStyle::DefaultUserShape)?;
    stdout().execute(Hide)?;
    enable_raw_mode()?;
    Ok(())
}

fn drain_terminal_events() {
    while let Ok(true) = event::poll(std::time::Duration::from_millis(0)) {
        if event::read().is_err() {
            break;
        }
    }
}

fn sanitize_file_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if sanitized.is_empty() {
        "edit".to_string()
    } else {
        sanitized
    }
}

fn create_tmp_file(name_hint: Option<&str>) -> Result<NamedTempFile, AppError> {
    let hinted_component = name_hint
        .and_then(|hint| {
            let parts = hint
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>();
            parts
                .iter()
                .rev()
                .find(|part| Path::new(part).extension().is_some())
                .copied()
                .or_else(|| parts.last().copied())
        })
        .unwrap_or("edit");

    let hinted_path = Path::new(hinted_component);
    let prefix = format!(
        "h5v-{}-",
        sanitize_file_component(
            hinted_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(hinted_component)
        )
    );

    let mut builder = Builder::new();
    builder.prefix(&prefix);

    let suffix = if let Some(extension) = hinted_path.extension().and_then(|ext| ext.to_str()) {
        let extension = sanitize_file_component(extension);
        if extension.is_empty() {
            None
        } else {
            Some(format!(".{extension}"))
        }
    } else {
        None
    };
    if let Some(ref suffix) = suffix {
        builder.suffix(suffix);
    }

    builder.tempfile().map_err(AppError::from)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(not(target_os = "windows"))]
#[derive(Debug, PartialEq, Eq)]
struct ParsedEditorCommand {
    envs: Vec<(String, String)>,
    program: String,
    args: Vec<String>,
}

#[cfg(not(target_os = "windows"))]
fn is_shell_assignment(value: &str) -> bool {
    let Some((name, _)) = value.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    match chars.next() {
        Some('a'..='z' | 'A'..='Z' | '_') => {}
        _ => return false,
    }
    chars.all(|ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))
}

#[cfg(not(target_os = "windows"))]
fn is_shell_operator(value: &str) -> bool {
    matches!(
        value,
        "|" | "||" | "&" | "&&" | ";" | "<" | ">" | ">>" | "<<" | "<<<" | "<&" | ">&"
    )
}

#[cfg(not(target_os = "windows"))]
fn parse_editor_command(editor: &str) -> Option<ParsedEditorCommand> {
    let parts = shell_split(editor).ok()?;
    if parts.is_empty() {
        return None;
    }

    let mut envs = Vec::new();
    let mut index = 0;
    while index < parts.len() && is_shell_assignment(&parts[index]) {
        let (name, value) = parts[index].split_once('=').unwrap_or_default();
        envs.push((name.to_string(), value.to_string()));
        index += 1;
    }

    let program = parts.get(index)?.to_string();
    let args = parts[index + 1..]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if is_shell_operator(&program) || args.iter().any(|arg| is_shell_operator(arg)) {
        return None;
    }

    Some(ParsedEditorCommand {
        envs,
        program,
        args,
    })
}

#[cfg(target_os = "windows")]
fn launch_editor(editor: &str, path: &Path) -> Result<std::process::ExitStatus, AppError> {
    let path_str = path.to_string_lossy();
    Command::new("cmd")
        .arg("/C")
        .arg(format!("{editor} \"{path_str}\""))
        .status()
        .map_err(AppError::from)
}

#[cfg(not(target_os = "windows"))]
fn launch_editor(editor: &str, path: &Path) -> Result<std::process::ExitStatus, AppError> {
    if let Some(parsed) = parse_editor_command(editor) {
        let mut command = Command::new(parsed.program);
        command.args(parsed.args);
        command.arg(path);
        for (name, value) in parsed.envs {
            command.env(name, value);
        }
        return command.status().map_err(AppError::from);
    }

    let editor_cmd = format!("{editor} {}", shell_quote(&path.to_string_lossy()));
    Command::new("sh")
        .arg("-c")
        .arg(editor_cmd)
        .status()
        .map_err(AppError::from)
}

pub fn perform_edit(
    state: &mut AppState<'_>,
    content: String,
    name_hint: Option<&str>,
) -> Result<String, AppError> {
    if state.readonly {
        return Err(AppError::EditError(
            "Cannot edit in read-only mode, open file with -w flag".to_string(),
        ));
    }

    leave_h5v()?;
    let edit_pause = state.edit_pause.write()?;
    let edit_result = (|| -> Result<String, AppError> {
        let mut file = create_tmp_file(name_hint)?;
        file.write_all(content.as_bytes())?;
        file.flush()?;
        let path = file.path().to_path_buf();

        let editor = env::var("VISUAL")
            .or_else(|_| env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_string());
        let status = launch_editor(&editor, &path)?;
        if !status.success() {
            let status_label = status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string());
            return Err(AppError::EditError(format!(
                "Editor exited unsuccessfully with status {status_label}"
            )));
        }
        let mut new_content = String::new();
        File::open(&path)?.read_to_string(&mut new_content)?;
        Ok(normalize_edited_content(new_content))
    })();
    reenter_h5v()?;
    drain_terminal_events();
    drop(edit_pause);
    edit_result
}

pub fn edit_existing_file(state: &mut AppState<'_>, path: &Path) -> Result<(), AppError> {
    leave_h5v()?;
    let edit_pause = state.edit_pause.write()?;
    let edit_result = (|| -> Result<(), AppError> {
        let editor = env::var("VISUAL")
            .or_else(|_| env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_string());
        let status = launch_editor(&editor, path)?;
        if !status.success() {
            let status_label = status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string());
            return Err(AppError::EditError(format!(
                "Editor exited unsuccessfully with status {status_label}"
            )));
        }
        Ok(())
    })();
    reenter_h5v()?;
    drain_terminal_events();
    drop(edit_pause);
    edit_result
}

fn normalize_edited_content(mut content: String) -> String {
    if content.ends_with('\n') {
        content.pop();
        if content.ends_with('\r') {
            content.pop();
        }
    }
    content
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{normalize_edited_content, shell_quote};

    #[cfg(not(target_os = "windows"))]
    use super::{parse_editor_command, ParsedEditorCommand};

    #[test]
    fn shell_quotes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn parses_editor_args_without_shell() {
        assert_eq!(
            parse_editor_command("code --wait"),
            Some(ParsedEditorCommand {
                envs: vec![],
                program: "code".to_string(),
                args: vec!["--wait".to_string()],
            })
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn parses_leading_env_assignments() {
        assert_eq!(
            parse_editor_command("GIT_EDITOR=true code --reuse-window"),
            Some(ParsedEditorCommand {
                envs: vec![("GIT_EDITOR".to_string(), "true".to_string())],
                program: "code".to_string(),
                args: vec!["--reuse-window".to_string()],
            })
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn falls_back_for_shell_operators() {
        assert_eq!(parse_editor_command("code --wait && echo done"), None);
    }

    #[test]
    fn normalizes_single_trailing_newline_only() {
        assert_eq!(normalize_edited_content("value\n".to_string()), "value");
        assert_eq!(normalize_edited_content("value\n\n".to_string()), "value\n");
    }
}
