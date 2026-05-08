use std::path::PathBuf;

use self::errors::ConfigureErrors;

pub mod errors;
mod loading;
mod lua;

pub fn ensure_config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::ensure_config_exists()
}

pub fn reset_config_path() -> Result<PathBuf, ConfigureErrors> {
    loading::reset_config_to_default()
}

pub use lua::run_lua_engine;
