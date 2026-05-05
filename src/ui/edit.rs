use std::{
    env,
    fs::File,
    io::{stdout, Read, Write},
    path::Path,
    process::Command,
};

use ratatui::crossterm::{
    cursor::{Hide, Show},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use tempfile::{Builder, NamedTempFile};

use crate::{error::AppError, ui::state::AppState};

pub fn leave_h5v() -> Result<(), AppError> {
    stdout().execute(Show)?;
    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    ratatui::restore();
    Ok(())
}

pub fn reenter_h5v() -> Result<(), AppError> {
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(Hide)?;
    enable_raw_mode()?;
    Ok(())
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
    let editor_cmd = format!("{editor} {}", shell_quote(&path.to_string_lossy()));
    Command::new("sh")
        .arg("-lc")
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
    drop(edit_pause);
    reenter_h5v()?;
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
mod tests {
    use super::{normalize_edited_content, shell_quote};

    #[test]
    fn shell_quotes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
    }

    #[test]
    fn normalizes_single_trailing_newline_only() {
        assert_eq!(normalize_edited_content("value\n".to_string()), "value");
        assert_eq!(normalize_edited_content("value\n\n".to_string()), "value\n");
    }
}
