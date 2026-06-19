use clap::{Parser, ValueEnum};
use ratatui::crossterm::style::{Attribute, Color, Stylize};
use std::{
    ffi::OsString,
    fs,
    io::{self, IsTerminal, Read, Write},
    path::PathBuf,
};

use crate::{
    compat, configure,
    error::AppError,
    h5f::ReadOpenMode,
    ui::command::{
        describe_command_invocation, format_command_invocation, parse_command_text,
        parse_startup_commands, StartupCommand,
    },
    GIT_VERSION,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum CliReadMode {
    Standard,
    Swmr,
    Snapshot,
    Auto,
}

impl Default for CliReadMode {
    fn default() -> Self {
        Self::Auto
    }
}

impl From<CliReadMode> for ReadOpenMode {
    fn from(value: CliReadMode) -> Self {
        match value {
            CliReadMode::Standard => ReadOpenMode::Standard,
            CliReadMode::Swmr => ReadOpenMode::Swmr,
            CliReadMode::Snapshot => ReadOpenMode::Snapshot,
            CliReadMode::Auto => ReadOpenMode::Auto,
        }
    }
}

#[derive(Parser, Debug)]
#[clap(
    author = "Daniel F. Hauge animcuil@gmail.com",
    about = "HDF5 Viewer - TUI for inspecting, visualizing and manipulating HDF5 and imported tabular data",
    version = GIT_VERSION,
    help_template = "{about-with-newline}\nVersion: {version}\n\n{usage-heading} {usage}\n\n{all-args}"
)]
pub(crate) struct Args {
    /// Paths to HDF5 files or supported tabular files (.csv, .tsv, .xlsx, .parquet) to open
    pub(crate) files: Vec<String>,

    #[clap(short, long)]
    pub(crate) write: bool,

    /// Read-only open strategy for native HDF5 files.
    #[clap(long = "read-mode", value_enum, default_value_t = CliReadMode::Auto)]
    pub(crate) read_mode: CliReadMode,

    /// Execute a command at startup. Can be repeated.
    #[clap(short = 'c', long = "command", value_name = "COMMAND")]
    pub(crate) commands: Vec<String>,

    /// Execute commands from a script file at startup. Use '-' to read stdin.
    #[clap(long = "script", value_name = "PATH")]
    pub(crate) scripts: Vec<String>,

    /// Validate startup commands and print a summary without launching the UI.
    #[clap(long = "script-test")]
    pub(crate) script_test: bool,

    /// Enable compatibility mode: plain fallback symbols plus text-only previews.
    #[clap(long = "compatibility")]
    pub(crate) compatibility: bool,

    /// Disable terminal graphics probing without enabling other compatibility fallbacks.
    #[clap(long = "no-terminal-graphics")]
    pub(crate) no_terminal_graphics: bool,

    /// Use this Lua config path instead of the default config directory path.
    #[clap(long = "config", value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,

    /// Initialize a new h5v plugin scaffold at PATH and exit.
    #[clap(long = "init-plugin", value_name = "PATH")]
    pub(crate) init_plugin: Option<PathBuf>,
}

pub(crate) struct CollectedStartupCommands {
    pub(crate) commands: Vec<StartupCommand>,
    pub(crate) warnings: Vec<String>,
}

struct ScriptTestSummary {
    origin: String,
    command: String,
    action: String,
}

struct ScriptTestTheme {
    colors: bool,
}

pub(crate) fn normalize_cli_args(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
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

pub(crate) fn collect_startup_commands(args: &Args) -> Result<CollectedStartupCommands, AppError> {
    let should_read_stdin = should_read_startup_stdin(args);
    let stdin_content = if should_read_stdin {
        let mut content = String::new();
        io::stdin().read_to_string(&mut content)?;
        Some(content)
    } else {
        None
    };
    collect_startup_commands_from_inputs(args, stdin_content.as_deref())
}

fn should_read_startup_stdin(args: &Args) -> bool {
    args.scripts.iter().any(|script| script == "-")
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

pub(crate) fn run_script_test(startup_commands: &[StartupCommand]) -> Result<(), AppError> {
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

pub(crate) fn init_plugin_scaffold(path: &std::path::Path) -> Result<String, AppError> {
    let plugin_root = path;
    let plugin_name = plugin_root
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            AppError::FileError(format!(
                "Plugin path '{}' must end with a directory name",
                plugin_root.display()
            ))
        })?;
    if plugin_root.exists() {
        if !plugin_root.is_dir() {
            return Err(AppError::FileError(format!(
                "Plugin path '{}' already exists and is not a directory",
                plugin_root.display()
            )));
        }
    }

    fs::create_dir_all(plugin_root.join("lua"))?;
    let manifest_path = plugin_root.join("h5v-plugin.toml");
    let entry_path = plugin_root.join("lua").join("init.lua");
    let plugin_id = sanitize_plugin_id(plugin_name);
    let display_name = humanize_plugin_name(plugin_name);
    if !manifest_path.exists() {
        fs::write(
            &manifest_path,
            format!(
                "id = \"{plugin_id}\"\nname = \"{display_name}\"\nversion = \"0.1.0\"\napi_version = \"2\"\nentry = \"lua/init.lua\"\n"
            ),
        )?;
    }
    if !entry_path.exists() {
        fs::write(&entry_path, plugin_lua_template())?;
    }
    configure::refresh_plugin_lua_ls_support_files(plugin_root)?;

    Ok(format!(
        "Initialized plugin scaffold at {}\n- {}\n- {}",
        plugin_root.display(),
        manifest_path.display(),
        entry_path.display()
    ))
}

fn sanitize_plugin_id(name: &str) -> String {
    let mut id = String::new();
    let mut last_was_separator = false;
    for ch in name.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '.'
        };
        if mapped == '.' {
            if !last_was_separator && !id.is_empty() {
                id.push('.');
            }
            last_was_separator = true;
        } else {
            id.push(mapped);
            last_was_separator = false;
        }
    }
    let trimmed = id.trim_matches('.');
    if trimmed.is_empty() {
        "example.plugin".to_string()
    } else {
        trimmed.to_string()
    }
}

fn humanize_plugin_name(name: &str) -> String {
    let words = name
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!(
                    "{}{}",
                    first.to_ascii_uppercase(),
                    chars.as_str().to_ascii_lowercase()
                ),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();
    if words.is_empty() {
        "Example Plugin".to_string()
    } else {
        words.join(" ")
    }
}

fn plugin_lua_template() -> &'static str {
    r#"---@type H5vPluginModule
return {
  ---@param ctx H5vPluginHealthcheckContext
  health = function(ctx)
    return {
      status = ctx.health.healthy,
      message = ctx.ui.build(function(ui)
        ui.text("🟢 successfully loaded plugin")
      end),
    }
  end,

  ---@param h5v H5vConfig
  ---@param ctx H5vPluginInitContext
  init = function(h5v, ctx)
    ctx.toast.info("success :D")
  end,
}
"#
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
            action: describe_command_invocation(&invocation).unwrap_or_else(|| "No-op".to_string()),
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
            theme.action(&summary.action)
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
            &configure::configured_symbol(|symbols| symbols.tree.horizontal_rule).repeat(width),
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
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{
        build_script_test_summaries, collect_startup_commands_from_inputs, format_script_test_report,
        init_plugin_scaffold, normalize_cli_args, Args, CliReadMode, ScriptTestTheme,
    };
    use crate::GIT_VERSION;

    fn test_args() -> Args {
        Args {
            files: vec!["file.h5".to_string()],
            write: false,
            read_mode: CliReadMode::Auto,
            commands: Vec::new(),
            scripts: Vec::new(),
            script_test: false,
            compatibility: false,
            no_terminal_graphics: false,
            config: None,
            init_plugin: None,
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
    fn stdin_is_only_read_for_explicit_script_dash() {
        let args = test_args();
        assert!(!super::should_read_startup_stdin(&args));

        let mut args = test_args();
        args.scripts = vec!["-".to_string()];
        assert!(super::should_read_startup_stdin(&args));
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
    fn explicit_stdin_commands_are_collected() {
        let mut args = test_args();
        args.scripts = vec!["-".to_string()];
        let collected = collect_startup_commands_from_inputs(&args, Some("seek 1; down 2\n"))
            .expect("stdin commands");
        assert_eq!(collected.commands.len(), 2);
        assert_eq!(collected.commands[0].origin, "stdin:1");
        assert_eq!(collected.commands[1].origin, "stdin:1[2]");
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
        assert!(report.contains(
            "action  Jump to an absolute index, or to x/y coordinates in matrix and heatmap views"
        ));
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
        assert!(help.contains("--config"));
        assert!(help.contains("--read-mode"));
    }

    #[test]
    fn parses_custom_config_path_argument() {
        let args = Args::parse_from(["h5v", "--config", "/tmp/h5v/init.lua", "file.h5"]);
        assert_eq!(args.config, Some("/tmp/h5v/init.lua".into()));
    }

    #[test]
    fn parses_read_mode_argument() {
        let args = Args::parse_from(["h5v", "--read-mode", "swmr", "file.h5"]);
        assert_eq!(args.read_mode, CliReadMode::Swmr);
    }

    #[test]
    fn parses_init_plugin_argument() {
        let args = Args::parse_from(["h5v", "--init-plugin", "/tmp/demo-plugin"]);
        assert_eq!(args.init_plugin, Some("/tmp/demo-plugin".into()));
    }

    #[test]
    fn initializes_plugin_scaffold_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let plugin_root = temp.path().join("demo-analysis");

        let message = init_plugin_scaffold(&plugin_root).expect("init plugin");
        assert!(message.contains("Initialized plugin scaffold"));

        let manifest =
            std::fs::read_to_string(plugin_root.join("h5v-plugin.toml")).expect("read manifest");
        assert!(manifest.contains("id = \"demo.analysis\""));
        assert!(manifest.contains("entry = \"lua/init.lua\""));

        let entry =
            std::fs::read_to_string(plugin_root.join("lua/init.lua")).expect("read entry lua");
        let lua_rc = std::fs::read_to_string(plugin_root.join(".luarc.json")).expect("read lua rc");
        let stub =
            std::fs::read_to_string(plugin_root.join(".h5v-luals/h5v.lua")).expect("read lua stub");
        assert!(lua_rc.contains("\"mode\": \"plugin\""));
        assert!(stub.contains("---@class H5vPluginModule"));
        assert!(entry.contains("health = function(ctx)"));
        assert!(entry.contains("ctx.health.healthy"));
        assert!(entry.contains("ctx.ui.build(function(ui)"));
        assert!(entry.contains("successfully loaded plugin"));
        assert!(entry.contains("success :D"));
        assert!(entry.contains("---@param h5v H5vConfig"));
        assert!(entry.contains("---@param ctx H5vPluginInitContext"));
    }

    #[test]
    fn preserves_existing_plugin_files_but_refreshes_luals_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let plugin_root = temp.path().join("demo-analysis");
        std::fs::create_dir_all(plugin_root.join("lua")).expect("create plugin lua dir");
        std::fs::write(plugin_root.join("h5v-plugin.toml"), "custom manifest\n")
            .expect("write custom manifest");
        std::fs::write(plugin_root.join("lua/init.lua"), "-- keep me\n")
            .expect("write custom init");
        std::fs::create_dir_all(plugin_root.join(".h5v-luals")).expect("create luals dir");
        std::fs::write(plugin_root.join(".luarc.json"), "{\"stale\":true}\n")
            .expect("write stale luarc");
        std::fs::write(plugin_root.join(".h5v-luals/h5v.lua"), "-- stale\n")
            .expect("write stale stub");

        init_plugin_scaffold(&plugin_root).expect("refresh plugin scaffold");

        assert_eq!(
            std::fs::read_to_string(plugin_root.join("h5v-plugin.toml")).expect("read manifest"),
            "custom manifest\n"
        );
        assert_eq!(
            std::fs::read_to_string(plugin_root.join("lua/init.lua")).expect("read init"),
            "-- keep me\n"
        );
        let lua_rc = std::fs::read_to_string(plugin_root.join(".luarc.json")).expect("read luarc");
        let stub =
            std::fs::read_to_string(plugin_root.join(".h5v-luals/h5v.lua")).expect("read stub");
        assert!(lua_rc.contains("\"mode\": \"plugin\""));
        assert!(stub.contains("---@class H5vPluginModule"));
        assert_ne!(stub, "-- stale\n");
    }
}
