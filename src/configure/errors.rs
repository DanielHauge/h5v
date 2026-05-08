use std::io;

#[derive(Debug)]
pub enum ConfigureErrors {
    FailureToReadConfig(io::Error),
    FailureCreateDefault(io::Error),
    NoCurrentDir(io::Error),
    LuaError(mlua::Error),
}

impl std::fmt::Display for ConfigureErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigureErrors::FailureToReadConfig(e) => write!(f, "Failed to read config: {}", e),
            ConfigureErrors::FailureCreateDefault(e) => {
                write!(f, "Failed to create default config: {}", e)
            }
            ConfigureErrors::NoCurrentDir(e) => write!(f, "Failed to get current directory: {}", e),
            ConfigureErrors::LuaError(e) => write!(f, "Lua error: {}", e),
        }
    }
}

impl From<mlua::Error> for ConfigureErrors {
    fn from(value: mlua::Error) -> Self {
        ConfigureErrors::LuaError(value)
    }
}
