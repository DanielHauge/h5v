#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
// #![deny(clippy::unreachable)]
use clap::{CommandFactory, Parser};
use ratatui::crossterm::style::{Color, Stylize};

mod cli;
mod color_consts;
mod compat;
mod configure;
mod data;
mod error;
mod h5f;
mod linking;
mod search;
mod sprint_attributes;
mod sprint_typedesc;
mod ui;
use git_version::git_version;

use crate::cli::{collect_startup_commands, normalize_cli_args, run_script_test, Args};
use crate::error::AppError;
pub const GIT_VERSION: &str =
    git_version!(args = ["--always", "--dirty=-modified", "--tags", "--abbrev=4"]);
// only major.minor.patch without commit hash or dirty state, for more concise display in the UI
pub const GIT_VERSION_SHORT: &str = git_version!(args = ["--tags", "--abbrev=0"]);

fn main() -> Result<(), AppError> {
    let args = Args::parse_from(normalize_cli_args(std::env::args_os()));
    let runtime_config = compat::resolve_runtime_config(
        args.compatibility,
        args.no_terminal_graphics,
        std::env::var_os("H5V_COMPATIBILITY_MODE").as_deref(),
    )?;
    compat::install_runtime_config(runtime_config)?;
    let startup = collect_startup_commands(&args)?;

    for warning in &startup.warnings {
        eprintln!("Warning: {warning}");
    }

    if args.script_test {
        run_script_test(&startup.commands)?;
        return Ok(());
    }

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
