use std::path::PathBuf;

use crate::configure::errors::ConfigureErrors;

pub fn config_path() -> Result<PathBuf, ConfigureErrors> {
    Ok(dirs::config_dir()
        .unwrap_or(std::env::current_dir().map_err(ConfigureErrors::NoCurrentDir)?)
        .join("h5v")
        .join("init.lua")
        .with_file_name("init.lua")
        .with_extension("lua"))
}

pub fn load_or_create_config() -> Result<String, ConfigureErrors> {
    let config_path = config_path()?;
    if !std::path::Path::new(&config_path).exists() {
        let parent_dir = config_path.parent().ok_or({
            ConfigureErrors::FailureCreateDefault(std::io::Error::other(
                "Failed to get parent directory of config path",
            ))
        })?;
        std::fs::create_dir_all(parent_dir).map_err(ConfigureErrors::FailureCreateDefault)?;
        std::fs::write(&config_path, "-- H5V Lua configuration file\n")
            .map_err(ConfigureErrors::FailureCreateDefault)?;
    }

    let init_lua_content =
        std::fs::read_to_string(&config_path).map_err(ConfigureErrors::FailureToReadConfig)?;

    Ok(init_lua_content)
}
