#![cfg_attr(not(test), warn(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(clippy::expect_used))]
#![cfg_attr(not(test), warn(clippy::panic))]
#![cfg_attr(not(test), warn(clippy::todo))]
#![cfg_attr(not(test), warn(clippy::unimplemented))]
use clap::{CommandFactory, Parser};
use ratatui::crossterm::style::{Color, Stylize};
use std::time::Instant;

mod cli;
mod compat;
mod configure;
mod data;
mod error;
mod h5f;
mod health;
mod linking;
mod logging;
mod search;
#[cfg(test)]
mod test_support;
mod ui;
use git_version::git_version;

use crate::cli::{
    collect_startup_commands, init_plugin_scaffold, normalize_cli_args, run_script_test, Args,
};
use crate::error::{log_error, AppError};
pub const GIT_VERSION: &str =
    git_version!(args = ["--always", "--dirty=-modified", "--tags", "--abbrev=4"]);
// only major.minor.patch without commit hash or dirty state, for more concise display in the UI
pub const GIT_VERSION_SHORT: &str = git_version!(args = ["--tags", "--abbrev=0"]);

fn main() -> Result<(), AppError> {
    let startup_started = Instant::now();
    let args = Args::parse_from(normalize_cli_args(std::env::args_os()));
    if let Err(error) = logging::initialize() {
        eprintln!("Warning: Failed to initialize logging: {error}");
    } else if let Some(log_path) = logging::log_path() {
        tracing::info!(
            kind = "launch",
            phase = "start",
            args = ?std::env::args_os().collect::<Vec<_>>(),
            init_plugin = args.init_plugin.as_ref().map(|path| path.display().to_string()),
            config_override = args.config.as_ref().map(|path| path.display().to_string()),
            file_count = args.files.len(),
            script_test = args.script_test,
            compatibility_flag = args.compatibility,
            terminal_graphics_disabled = args.no_terminal_graphics,
            log_path = %log_path.display(),
            startup_elapsed_ms = startup_started.elapsed().as_millis() as u64,
            message = "launch started"
        );
    }
    if let Some(path) = &args.init_plugin {
        tracing::info!(
            kind = "launch",
            phase = "init_plugin",
            path = %path.display(),
            startup_elapsed_ms = startup_started.elapsed().as_millis() as u64,
            message = "initializing plugin scaffold"
        );
        println!("{}", init_plugin_scaffold(path)?);
        return Ok(());
    }
    configure::set_config_path_override(args.config.clone())?;
    let compatibility_from_env =
        compat::compatibility_from_env(std::env::var_os("H5V_COMPATIBILITY_MODE").as_deref())?;
    let default_compatibility = compatibility_from_env.unwrap_or(false);
    let compatibility_from_config = if args.compatibility {
        None
    } else {
        match configure::load_config_compatibility(default_compatibility) {
            Ok(value) => value,
            Err(error) => {
                log_error(format!("Configuration error: {error}\n"));
                eprintln!("Warning: Configuration error: {error}");
                None
            }
        }
    };
    let runtime_config = compat::resolve_runtime_config(
        args.compatibility,
        args.no_terminal_graphics,
        compatibility_from_config,
        compatibility_from_env,
    );
    compat::install_runtime_config(runtime_config)?;
    let startup = collect_startup_commands(&args)?;

    for warning in &startup.warnings {
        eprintln!("Warning: {warning}");
    }

    if args.script_test {
        tracing::info!(
            kind = "launch",
            phase = "script_test",
            startup_elapsed_ms = startup_started.elapsed().as_millis() as u64,
            startup_command_count = startup.commands.len(),
            message = "running script test"
        );
        run_script_test(&startup.commands)?;
        return Ok(());
    }

    tracing::info!(
        kind = "launch",
        phase = "ui_handoff",
        startup_elapsed_ms = startup_started.elapsed().as_millis() as u64,
        startup_command_count = startup.commands.len(),
        linked_file_count = args.files.len(),
        message = "starting UI"
    );
    match &args.files[..] {
        // [] => Err(AppError::FileError(String::from(
        //     "No files given.\n Usage: h5v /path/to/file.h5",
        // ))),
        [] => {
            eprintln!("{}", "Error: No files given.\n".with(Color::Red));
            Args::command().print_long_help()?;
            std::process::exit(1);
        }
        [single] => ui::app::init(
            single.clone(),
            false,
            args.write,
            runtime_config,
            &startup.commands,
        ),
        multiple => ui::app::init(
            linking::link(multiple)?,
            true,
            args.write,
            runtime_config,
            &startup.commands,
        ),
    }
}
