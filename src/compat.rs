use std::{ffi::OsStr, io::IsTerminal, sync::OnceLock};

use crate::error::AppError;
use crate::health::{HealthStatus, HealthcheckResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub compatibility_mode: bool,
    pub terminal_graphics: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            compatibility_mode: false,
            terminal_graphics: true,
        }
    }
}

static RUNTIME_CONFIG: OnceLock<RuntimeConfig> = OnceLock::new();

pub fn compatibility_from_env(compatibility_env: Option<&OsStr>) -> Result<Option<bool>, AppError> {
    compatibility_env.map(parse_bool_env).transpose()
}

pub fn resolve_runtime_config(
    compatibility_flag: bool,
    no_terminal_graphics_flag: bool,
    compatibility_from_config: Option<bool>,
    compatibility_from_env: Option<bool>,
) -> RuntimeConfig {
    let compatibility_mode = if compatibility_flag {
        true
    } else {
        compatibility_from_config.unwrap_or_else(|| compatibility_from_env.unwrap_or(false))
    };
    RuntimeConfig {
        compatibility_mode,
        terminal_graphics: !compatibility_mode && !no_terminal_graphics_flag,
    }
}

pub fn install_runtime_config(config: RuntimeConfig) -> Result<(), AppError> {
    RUNTIME_CONFIG.set(config).map_err(|_| {
        AppError::FileError("Runtime compatibility config was initialized twice".to_string())
    })
}

pub fn current() -> RuntimeConfig {
    RUNTIME_CONFIG.get().copied().unwrap_or_default()
}

pub fn run_runtime_healthcheck(
    config: RuntimeConfig,
    image_protocol_enabled: bool,
) -> Vec<HealthcheckResult> {
    let mut results = Vec::new();
    if !std::io::stdout().is_terminal() {
        results.push(HealthcheckResult::fail(
            "stdout is not a terminal; h5v expects an interactive terminal session",
        ));
    }

    let term = std::env::var("TERM").unwrap_or_default();
    if term.trim().eq_ignore_ascii_case("dumb") {
        results.push(HealthcheckResult::fail(
            "TERM is set to 'dumb'; terminal capabilities are too limited for h5v",
        ));
    }

    if !config.compatibility_mode && std::env::var_os("COLORTERM").is_none() && term.is_empty() {
        results.push(HealthcheckResult::warning(
            "terminal color capabilities could not be detected",
        ));
    }

    if !config.terminal_graphics {
        results.push(HealthcheckResult::warning(
            "terminal graphics are disabled; image previews will fall back to text rendering",
        ));
    } else if !image_protocol_enabled {
        results.push(HealthcheckResult::warning(
            "terminal graphics probing fell back to halfblocks; graphics previews are unavailable",
        ));
    }

    if results.is_empty() {
        results.push(HealthcheckResult::healthy(
            "terminal capabilities look good",
        ));
    }
    results
}

pub fn summarize_runtime_healthcheck(results: &[HealthcheckResult]) -> Option<String> {
    let summary = results
        .iter()
        .filter(|result| result.status != HealthStatus::Healthy)
        .map(|result| format!("{}: {}", result.status.as_str(), result.message))
        .collect::<Vec<_>>();
    if summary.is_empty() {
        None
    } else {
        Some(format!("Healthcheck warning: {}", summary.join("; ")))
    }
}

fn parse_bool_env(value: &OsStr) -> Result<bool, AppError> {
    let raw = value.to_str().ok_or_else(|| {
        AppError::FileError("H5V_COMPATIBILITY_MODE must be valid UTF-8".to_string())
    })?;
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(AppError::FileError(format!(
            "Invalid H5V_COMPATIBILITY_MODE value `{raw}`; expected true/false"
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use std::ffi::OsString;

    use super::{compatibility_from_env, resolve_runtime_config};

    #[test]
    fn compatibility_env_enables_compatibility_mode() {
        let env = compatibility_from_env(Some(OsString::from("true").as_os_str())).expect("env");
        let config = resolve_runtime_config(false, false, None, env);
        assert!(config.compatibility_mode);
        assert!(!config.terminal_graphics);
    }

    #[test]
    fn compatibility_flag_takes_effect_without_env() {
        let config = resolve_runtime_config(true, false, Some(false), Some(false));
        assert!(config.compatibility_mode);
        assert!(!config.terminal_graphics);
    }

    #[test]
    fn no_terminal_graphics_only_disables_graphics() {
        let config = resolve_runtime_config(false, true, None, None);
        assert!(!config.compatibility_mode);
        assert!(!config.terminal_graphics);
    }

    #[test]
    fn invalid_compatibility_env_errors() {
        let error = compatibility_from_env(Some(OsString::from("maybe").as_os_str()))
            .expect_err("invalid env should fail");
        assert!(error
            .to_string()
            .contains("Invalid H5V_COMPATIBILITY_MODE value"));
    }

    #[test]
    fn config_overrides_environment_without_cli_flag() {
        let config = resolve_runtime_config(false, false, Some(false), Some(true));
        assert!(!config.compatibility_mode);
        assert!(config.terminal_graphics);
    }
}
