use std::{env, io::stdout};

use ratatui::{
    crossterm::{
        cursor::{Hide, Show},
        event::{DisableMouseCapture, EnableMouseCapture},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    prelude::CrosstermBackend,
    Terminal,
};

use crate::{compat::RuntimeConfig, error::AppError};

use super::config::should_use_alternate_screen;

pub(super) type AppTerminal = Terminal<CrosstermBackend<std::io::Stdout>>;

pub(super) enum RecoverLoopAction {
    Retry(String),
    Break(String),
}

pub(super) fn resolve_alternate_screen(runtime_config: RuntimeConfig) -> bool {
    should_use_alternate_screen(runtime_config, env::var("CROS_CONTAINER").ok().as_deref())
}

pub(super) fn init_terminal(use_alternate_screen: bool) -> Result<AppTerminal, AppError> {
    if use_alternate_screen {
        stdout().execute(EnterAlternateScreen)?;
    }
    stdout().execute(EnableMouseCapture)?;
    stdout().execute(Hide)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;
    Ok(terminal)
}

pub(super) fn restore_terminal(
    use_alternate_screen: bool,
    last_message: Option<String>,
) -> Result<(), AppError> {
    stdout().execute(Show)?;
    stdout().execute(DisableMouseCapture)?;
    if use_alternate_screen {
        stdout().execute(LeaveAlternateScreen)?;
    }
    disable_raw_mode()?;
    if let Some(message) = last_message {
        eprintln!("Unrecoverable AppError: {}", message);
    }
    Ok(())
}

pub(super) fn classify_recover_loop_error(error: AppError) -> RecoverLoopAction {
    match error {
        AppError::FileError(_) => RecoverLoopAction::Retry("No files given error".to_string()),
        AppError::Io(error) => RecoverLoopAction::Retry(format!("IO Error: - {error}")),
        AppError::Hdf5(error) => match error {
            hdf5_metno::Error::HDF5(_) => RecoverLoopAction::Break("HDF5 Error".to_string()),
            hdf5_metno::Error::Internal(error) => {
                RecoverLoopAction::Break(format!("HDF5 Internal: - {error}"))
            }
        },
        AppError::ChannelError(error) => {
            RecoverLoopAction::Retry(format!("Channel Error: - {error}"))
        }
        AppError::ClipboardError(message) => {
            RecoverLoopAction::Break(format!("Clipboard Error: - {message}"))
        }
        AppError::InvalidCommand(command) => {
            RecoverLoopAction::Break(format!("Invalid Command: - {command}"))
        }
        AppError::EditError(error) => RecoverLoopAction::Break(format!("Edit Error: - {error}")),
        AppError::EditWarning(error) => {
            RecoverLoopAction::Break(format!("Edit Warning: - {error}"))
        }
        AppError::FixedStringOverflow(error) => {
            RecoverLoopAction::Break(format!("Edit Error: - {error}"))
        }
        AppError::ChildNotFound(error) => {
            RecoverLoopAction::Break(format!("Child not found: - {error}"))
        }
        AppError::PoisonedLockError(error) => {
            RecoverLoopAction::Break(format!("Poisoned lock error: - {error}"))
        }
        AppError::DrawingError(error) => {
            RecoverLoopAction::Break(format!("Drawing error: - {error}"))
        }
        AppError::LuaError(error) => RecoverLoopAction::Break(format!("Lua error: - {error}")),
    }
}
