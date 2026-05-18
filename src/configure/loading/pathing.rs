use std::{
    path::{Path, PathBuf},
    sync::{LazyLock, RwLock},
};

use crate::configure::errors::ConfigureErrors;

static CONFIG_PATH_OVERRIDE: LazyLock<RwLock<Option<PathBuf>>> =
    LazyLock::new(|| RwLock::new(None));

pub fn set_config_path_override(path: Option<PathBuf>) -> Result<Option<PathBuf>, ConfigureErrors> {
    let normalized = path.map(normalize_config_path).transpose()?;
    *CONFIG_PATH_OVERRIDE.write().map_err(|error| {
        ConfigureErrors::FailureCreateDefault(std::io::Error::other(format!(
            "Config path override lock poisoned: {error}"
        )))
    })? = normalized.clone();
    Ok(normalized)
}

pub fn config_path() -> Result<PathBuf, ConfigureErrors> {
    if let Some(path) = CONFIG_PATH_OVERRIDE
        .read()
        .map_err(|error| {
            ConfigureErrors::FailureCreateDefault(std::io::Error::other(format!(
                "Config path override lock poisoned: {error}"
            )))
        })?
        .clone()
    {
        return Ok(path);
    }
    Ok(dirs::config_dir()
        .unwrap_or(std::env::current_dir().map_err(ConfigureErrors::NoCurrentDir)?)
        .join("h5v")
        .join("init.lua")
        .with_file_name("init.lua")
        .with_extension("lua"))
}

fn normalize_config_path(path: PathBuf) -> Result<PathBuf, ConfigureErrors> {
    let path_str = path.to_string_lossy();
    if path_str == "~" || path_str.starts_with("~/") {
        let home = std::env::var_os("HOME").ok_or_else(|| {
            ConfigureErrors::FailureCreateDefault(std::io::Error::other(
                "Cannot expand '~' without HOME set",
            ))
        })?;
        let mut expanded = PathBuf::from(home);
        if let Some(suffix) = path_str.strip_prefix("~/") {
            expanded.push(suffix);
        }
        Ok(expanded)
    } else {
        Ok(path)
    }
}

pub(super) fn config_parent_dir(config_path: &Path) -> &Path {
    config_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}
