use std::{
    env,
    io::stdout,
    panic::PanicHookInfo,
    sync::{Arc, Mutex},
};

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

type PanicHook = Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>;

pub(super) struct PanicTerminalRestoreHook {
    previous: Arc<Mutex<Option<PanicHook>>>,
}

pub(super) fn install_panic_terminal_restore_hook(
    use_alternate_screen: bool,
) -> PanicTerminalRestoreHook {
    let previous = Arc::new(Mutex::new(Some(std::panic::take_hook())));
    let panic_previous = Arc::clone(&previous);

    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal_best_effort(use_alternate_screen);
        if let Ok(previous) = panic_previous.lock() {
            if let Some(previous) = previous.as_ref() {
                previous(panic_info);
            }
        }
    }));

    PanicTerminalRestoreHook { previous }
}

impl Drop for PanicTerminalRestoreHook {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }

        if let Ok(mut previous) = self.previous.lock() {
            if let Some(previous) = previous.take() {
                std::panic::set_hook(previous);
            }
        }
    }
}

pub(super) fn resolve_alternate_screen(runtime_config: RuntimeConfig) -> bool {
    should_use_alternate_screen(runtime_config, env::var("CROS_CONTAINER").ok().as_deref())
}

pub(super) fn init_terminal(use_alternate_screen: bool) -> Result<AppTerminal, AppError> {
    let result = (|| {
        if use_alternate_screen {
            stdout().execute(EnterAlternateScreen)?;
        }
        stdout().execute(EnableMouseCapture)?;
        stdout().execute(Hide)?;
        enable_raw_mode()?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        terminal.clear()?;
        super::startup_progress::set_startup_progress_enabled(true);
        Ok(terminal)
    })();

    if result.is_err() {
        restore_terminal_best_effort(use_alternate_screen);
    }
    result
}

pub(super) fn restore_terminal(
    use_alternate_screen: bool,
    last_message: Option<String>,
) -> Result<(), AppError> {
    restore_terminal_state(use_alternate_screen)?;
    if let Some(message) = last_message {
        eprintln!("Unrecoverable AppError: {}", message);
    }
    Ok(())
}

fn restore_terminal_best_effort(use_alternate_screen: bool) {
    let _ = restore_terminal_state(use_alternate_screen);
}

fn restore_terminal_state(use_alternate_screen: bool) -> std::io::Result<()> {
    super::startup_progress::set_startup_progress_enabled(false);
    let mut out = stdout();
    let mut first_error = None;
    if let Err(error) = out.execute(Show) {
        first_error.get_or_insert(error);
    }
    if let Err(error) = out.execute(DisableMouseCapture) {
        first_error.get_or_insert(error);
    }
    if use_alternate_screen {
        if let Err(error) = out.execute(LeaveAlternateScreen) {
            first_error.get_or_insert(error);
        }
    }
    if let Err(error) = disable_raw_mode() {
        first_error.get_or_insert(error);
    }
    first_error.map_or(Ok(()), Err)
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
