#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
// #![deny(clippy::unreachable)]
use clap::{CommandFactory, Parser};
use ratatui::crossterm::style::{Attribute, Color, Stylize};
use std::{
    ffi::OsString,
    io::{self, IsTerminal, Read, Write},
};

mod color_consts;
mod compat;
mod data;
mod error;
mod h5f;
mod linking;
mod search;
mod sprint_attributes;
mod sprint_typedesc;
mod ui;
use git_version::git_version;

use crate::error::AppError;
use crate::ui::command::{
    describe_command_invocation, format_command_invocation, parse_command_text,
    parse_startup_commands, StartupCommand,
};
pub const GIT_VERSION: &str =
    git_version!(args = ["--always", "--dirty=-modified", "--tags", "--abbrev=4"]);

#[derive(Parser, Debug)]
#[clap(
    author = "Daniel F. Hauge animcuil@gmail.com",
    about = "HDF5 Viewer - TUI for inspecting, visualizing and manipulating HDF5 files",
    version = GIT_VERSION,
    help_template = "{about-with-newline}\nVersion: {version}\n\n{usage-heading} {usage}\n\n{all-args}"
)]
struct Args {
    /// Path to the HDF5 file to open
    files: Vec<String>,

    #[clap(short, long)]
    write: bool,

    /// Execute a command at startup. Can be repeated.
    #[clap(short = 'c', long = "command", value_name = "COMMAND")]
    commands: Vec<String>,

    /// Execute commands from a script file at startup. Use '-' to read stdin.
    #[clap(long = "script", value_name = "PATH")]
    scripts: Vec<String>,

    /// Validate startup commands and print a summary without launching the UI.
    #[clap(long = "script-test")]
    script_test: bool,

    /// Enable compatibility mode: plain fallback symbols plus text-only previews.
    #[clap(long = "compatibility")]
    compatibility: bool,

    /// Disable terminal graphics probing without enabling other compatibility fallbacks.
    #[clap(long = "no-terminal-graphics")]
    no_terminal_graphics: bool,
}

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

struct CollectedStartupCommands {
    commands: Vec<StartupCommand>,
    warnings: Vec<String>,
}

struct ScriptTestSummary {
    origin: String,
    command: String,
    action: &'static str,
}

struct ScriptTestTheme {
    colors: bool,
}

fn normalize_cli_args(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    args.into_iter()
        .map(|arg| {
            if arg == "-ct" {
                OsString::from("--script-test")
            } else {
                arg
            }
        })
        .collect()
}

fn collect_startup_commands(args: &Args) -> Result<CollectedStartupCommands, AppError> {
    let should_read_stdin =
        args.scripts.iter().any(|script| script == "-") || !io::stdin().is_terminal();
    let stdin_content = if should_read_stdin {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        Some(content)
    } else {
        None
    };
    collect_startup_commands_from_inputs(args, stdin_content.as_deref())
}

fn collect_startup_commands_from_inputs(
    args: &Args,
    stdin_content: Option<&str>,
) -> Result<CollectedStartupCommands, AppError> {
    let mut startup_commands = Vec::new();
    let mut warnings = Vec::new();
    let explicit_stdin = args.scripts.iter().any(|script| script == "-");

    for script in &args.scripts {
        let (content, origin) = if script == "-" {
            (
                stdin_content.unwrap_or_default().to_string(),
                "stdin".to_string(),
            )
        } else {
            let content = std::fs::read_to_string(script).map_err(|error| {
                AppError::FileError(format!("Failed to read script '{script}': {error}"))
            })?;
            (content, script.clone())
        };
        let parsed = parse_startup_commands(&content, &origin);
        if script == "-" && parsed.is_empty() {
            warnings.push("--script - reached EOF without any commands".to_string());
        }
        startup_commands.extend(parsed);
    }

    if !explicit_stdin {
        if let Some(stdin_content) = stdin_content {
            startup_commands.extend(parse_startup_commands(stdin_content, "stdin"));
        }
    }

    for (idx, command) in args.commands.iter().enumerate() {
        startup_commands.extend(parse_startup_commands(
            command,
            &format!("--command[{}]", idx + 1),
        ));
    }

    Ok(CollectedStartupCommands {
        commands: startup_commands,
        warnings,
    })
}

fn run_script_test(startup_commands: &[StartupCommand]) -> Result<(), AppError> {
    let summaries = build_script_test_summaries(startup_commands)?;
    let report = format_script_test_report(
        &summaries,
        ScriptTestTheme {
            colors: io::stdout().is_terminal() && !compat::current().compatibility_mode,
        },
    );
    io::stdout().write_all(report.as_bytes())?;
    Ok(())
}

fn build_script_test_summaries(
    startup_commands: &[StartupCommand],
) -> Result<Vec<ScriptTestSummary>, AppError> {
    let mut summaries = Vec::with_capacity(startup_commands.len());
    for startup_command in startup_commands {
        let invocation = parse_command_text(&startup_command.command_text).map_err(|error| {
            AppError::InvalidCommand(format!("{}: {}", startup_command.origin, error))
        })?;
        summaries.push(ScriptTestSummary {
            origin: startup_command.origin.clone(),
            command: format_command_invocation(&invocation),
            action: describe_command_invocation(&invocation).unwrap_or("No-op"),
        });
    }
    Ok(summaries)
}

fn format_script_test_report(summaries: &[ScriptTestSummary], theme: ScriptTestTheme) -> String {
    if summaries.is_empty() {
        return format!("{}\n", theme.muted("No startup commands found."));
    }

    let origin_width = summaries
        .iter()
        .map(|summary| summary.origin.len())
        .max()
        .unwrap_or(0);
    let command_width = summaries
        .iter()
        .map(|summary| summary.command.len())
        .max()
        .unwrap_or(0);
    let rule_width = origin_width.max(command_width).max(36);
    let mut lines = Vec::with_capacity(summaries.len() * 4 + 4);

    lines.push(theme.heading("Startup command dry run"));
    lines.push(format!(
        "{} {}",
        theme.badge("OK"),
        theme.muted(&format!("Validated {} startup command(s)", summaries.len()))
    ));
    lines.push(theme.rule(rule_width));

    for (idx, summary) in summaries.iter().enumerate() {
        lines.push(format!(
            "{} {}",
            theme.index(idx + 1),
            theme.origin(&summary.origin)
        ));
        lines.push(format!(
            "   {} {}",
            theme.label("command"),
            theme.command(&format!(
                "{:width$}",
                summary.command,
                width = command_width
            ))
        ));
        lines.push(format!(
            "   {} {}",
            theme.label("action "),
            theme.action(summary.action)
        ));
        if idx + 1 != summaries.len() {
            lines.push(theme.rule(rule_width));
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

impl ScriptTestTheme {
    fn heading(&self, text: &str) -> String {
        self.paint(text, Color::Cyan, true, false)
    }

    fn badge(&self, text: &str) -> String {
        self.paint(text, Color::Green, true, false)
    }

    fn index(&self, value: usize) -> String {
        self.paint(&format!("[{value:02}]"), Color::DarkGrey, true, false)
    }

    fn label(&self, text: &str) -> String {
        self.paint(text, Color::DarkGrey, true, false)
    }

    fn origin(&self, text: &str) -> String {
        self.paint(text, Color::Blue, true, false)
    }

    fn command(&self, text: &str) -> String {
        self.paint(text, Color::Yellow, true, false)
    }

    fn action(&self, text: &str) -> String {
        self.paint(text, Color::White, false, false)
    }

    fn muted(&self, text: &str) -> String {
        self.paint(text, Color::DarkGrey, false, false)
    }

    fn rule(&self, width: usize) -> String {
        self.paint(
            &compat::horizontal_rule(width),
            Color::DarkGrey,
            false,
            true,
        )
    }

    fn paint(&self, text: &str, color: Color, bold: bool, dim: bool) -> String {
        if !self.colors {
            return text.to_string();
        }

        let mut styled = text.with(color);
        if bold {
            styled = styled.attribute(Attribute::Bold);
        }
        if dim {
            styled = styled.attribute(Attribute::Dim);
        }
        format!("{styled}")
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::{
        build_script_test_summaries, collect_startup_commands_from_inputs,
        format_script_test_report, normalize_cli_args, Args, ScriptTestTheme,
    };
    use crate::GIT_VERSION;

    fn test_args() -> Args {
        Args {
            files: vec!["file.h5".to_string()],
            write: false,
            commands: Vec::new(),
            scripts: Vec::new(),
            script_test: false,
            compatibility: false,
            no_terminal_graphics: false,
        }
    }

    #[test]
    fn normalizes_ct_alias_to_script_test() {
        let normalized = normalize_cli_args(vec![
            "h5v".into(),
            "-ct".into(),
            "--command".into(),
            "down 2".into(),
        ]);
        assert_eq!(normalized[1], "--script-test");
    }

    #[test]
    fn collects_implicit_stdin_commands_without_warning() {
        let args = test_args();
        let collected = collect_startup_commands_from_inputs(&args, Some("seek 1; down 2\n"))
            .expect("stdin commands");
        assert_eq!(collected.commands.len(), 2);
        assert!(collected.warnings.is_empty());
        assert_eq!(collected.commands[0].origin, "stdin:1");
        assert_eq!(collected.commands[1].origin, "stdin:1[2]");
    }

    #[test]
    fn warns_when_explicit_stdin_has_no_commands() {
        let mut args = test_args();
        args.scripts = vec!["-".to_string()];
        let collected = collect_startup_commands_from_inputs(&args, Some(" \n# comment\n"))
            .expect("stdin parse");
        assert_eq!(collected.commands.len(), 0);
        assert_eq!(
            collected.warnings,
            vec!["--script - reached EOF without any commands"]
        );
    }

    #[test]
    fn formats_plain_script_test_report() {
        let startup_commands =
            collect_startup_commands_from_inputs(&test_args(), Some("seek 1; down 2"))
                .expect("startup commands")
                .commands;
        let summaries = build_script_test_summaries(&startup_commands).expect("summaries");
        let report = format_script_test_report(&summaries, ScriptTestTheme { colors: false });
        assert!(report.contains("Startup command dry run"));
        assert!(report.contains("[01] stdin:1"));
        assert!(report.contains("command seek 1"));
        assert!(report.contains("action  Jump to an absolute index in the current content view"));
        assert!(report.contains("[02] stdin:1[2]"));
    }

    #[test]
    fn help_includes_resolved_version() {
        let mut command = Args::command();
        let mut help = Vec::new();
        command.write_long_help(&mut help).expect("write help");
        let help = String::from_utf8(help).expect("utf8 help");
        assert!(help.contains(&format!("Version: {GIT_VERSION}")));
        assert!(help.contains("--compatibility"));
    }
}
