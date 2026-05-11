use std::path::PathBuf;

use crate::{
    configure::errors::ConfigureErrors,
    configure::{self, SymbolThemeName, ThemeName},
};

pub fn config_path() -> Result<PathBuf, ConfigureErrors> {
    Ok(dirs::config_dir()
        .unwrap_or(std::env::current_dir().map_err(ConfigureErrors::NoCurrentDir)?)
        .join("h5v")
        .join("init.lua")
        .with_file_name("init.lua")
        .with_extension("lua"))
}

pub fn load_or_create_config() -> Result<String, ConfigureErrors> {
    let config_path = ensure_config_exists()?;

    let init_lua_content =
        std::fs::read_to_string(&config_path).map_err(ConfigureErrors::FailureToReadConfig)?;

    Ok(init_lua_content)
}

pub fn ensure_config_exists() -> Result<PathBuf, ConfigureErrors> {
    let config_path = config_path()?;
    if !std::path::Path::new(&config_path).exists() {
        write_default_config(&config_path)?;
    }
    Ok(config_path)
}

pub fn reset_config_to_default() -> Result<PathBuf, ConfigureErrors> {
    let config_path = config_path()?;
    write_default_config(&config_path)?;
    Ok(config_path)
}

fn default_config_contents() -> String {
    let mut lines = vec![
        "-- H5V Lua configuration file".to_string(),
        "-- Pick built-in color/symbol themes, then override any named values you want."
            .to_string(),
        format!(
            "-- Available themes: {}",
            configure::available_theme_names().join(", ")
        ),
        format!("-- h5v.theme = \"{}\"", ThemeName::Dark.as_str()),
        format!(
            "-- Available symbol themes: {}",
            configure::available_symbol_theme_names().join(", ")
        ),
        format!(
            "-- h5v.symbol_theme = \"{}\"",
            SymbolThemeName::Rich.as_str()
        ),
        "-- Compatibility precedence: CLI flag > h5v.compatibility > H5V_COMPATIBILITY_MODE"
            .to_string(),
        "-- h5v.compatibility = false".to_string(),
        "--".to_string(),
        "-- Colors accept #RRGGBB or names like blue, magenta, lightgreen, darkgray.".to_string(),
        "-- h5v.colors = {".to_string(),
    ];
    append_grouped_color_examples(&mut lines);
    lines.push("-- }".to_string());
    lines.push("--".to_string());
    lines.push(
        "-- Symbols accept any Lua string; use simple ASCII fallbacks if your terminal needs them."
            .to_string(),
    );
    lines.push("-- h5v.symbols = {".to_string());
    append_grouped_symbol_examples(&mut lines);
    lines.push("-- }".to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn append_grouped_color_examples(lines: &mut Vec<String>) {
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (name, color) in configure::theme_named_colors(ThemeName::Dark) {
        let (group, key) = name.split_once('.').unwrap_or(("", name));
        let value = configure::color_to_lua_string(color);

        if let Some((_, entries)) = groups.iter_mut().find(|(existing, _)| existing == group) {
            entries.push((key.to_string(), value));
        } else {
            groups.push((group.to_string(), vec![(key.to_string(), value)]));
        }
    }

    for (group, entries) in groups {
        lines.push(format!("--   {group} = {{"));
        for (key, value) in entries {
            lines.push(format!("--     {key} = \"{value}\","));
        }
        lines.push("--   },".to_string());
    }
}

fn append_grouped_symbol_examples(lines: &mut Vec<String>) {
    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (name, value) in configure::theme_named_symbols(SymbolThemeName::Rich) {
        let (group, key) = name.split_once('.').unwrap_or(("", name));

        if let Some((_, entries)) = groups.iter_mut().find(|(existing, _)| existing == group) {
            entries.push((key.to_string(), value.to_string()));
        } else {
            groups.push((
                group.to_string(),
                vec![(key.to_string(), value.to_string())],
            ));
        }
    }

    for (group, entries) in groups {
        lines.push(format!("--   {group} = {{"));
        for (key, value) in entries {
            lines.push(format!("--     {key} = \"{value}\","));
        }
        lines.push("--   },".to_string());
    }
}

fn write_default_config(config_path: &PathBuf) -> Result<(), ConfigureErrors> {
    let parent_dir = config_path.parent().ok_or({
        ConfigureErrors::FailureCreateDefault(std::io::Error::other(
            "Failed to get parent directory of config path",
        ))
    })?;
    std::fs::create_dir_all(parent_dir).map_err(ConfigureErrors::FailureCreateDefault)?;
    std::fs::write(config_path, default_config_contents())
        .map_err(ConfigureErrors::FailureCreateDefault)
}

#[cfg(test)]
mod tests {
    use super::default_config_contents;

    #[test]
    fn reset_scaffold_groups_each_category_once() {
        let config = default_config_contents();

        assert_eq!(config.matches("--   text = {").count(), 1);
        assert_eq!(config.matches("--   content = {").count(), 1);
        assert_eq!(config.matches("--   command = {").count(), 1);
        assert_eq!(config.matches("--   help = {").count(), 1);
        assert_eq!(config.matches("--   metadata = {").count(), 1);
        assert_eq!(config.matches("--   file = {").count(), 1);
        assert_eq!(config.matches("--   mchart = {").count(), 1);
        assert_eq!(config.matches("--   surface = {").count(), 1);
        assert_eq!(config.matches("--   accent = {").count(), 1);
        assert_eq!(config.matches("--   tree = {").count(), 2);
        assert_eq!(config.matches("--   chart = {").count(), 2);
        assert_eq!(config.matches("--   status = {").count(), 1);
        assert_eq!(config.matches("--   toast = {").count(), 1);
        assert_eq!(config.matches("--   section = {").count(), 1);
        assert_eq!(config.matches("--   title = {").count(), 1);
        assert_eq!(config.matches("--   badge = {").count(), 1);
    }
}
