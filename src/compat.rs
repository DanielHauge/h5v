use std::{ffi::OsStr, sync::OnceLock};

use crate::error::AppError;

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

pub fn resolve_runtime_config(
    compatibility_flag: bool,
    no_terminal_graphics_flag: bool,
    compatibility_env: Option<&OsStr>,
) -> Result<RuntimeConfig, AppError> {
    let compatibility_from_env = compatibility_env
        .map(parse_bool_env)
        .transpose()?
        .unwrap_or(false);
    let compatibility_mode = compatibility_flag || compatibility_from_env;
    Ok(RuntimeConfig {
        compatibility_mode,
        terminal_graphics: !compatibility_mode && !no_terminal_graphics_flag,
    })
}

pub fn install_runtime_config(config: RuntimeConfig) -> Result<(), AppError> {
    RUNTIME_CONFIG.set(config).map_err(|_| {
        AppError::FileError("Runtime compatibility config was initialized twice".to_string())
    })
}

pub fn current() -> RuntimeConfig {
    RUNTIME_CONFIG.get().copied().unwrap_or_default()
}

pub fn horizontal_rule(width: usize) -> String {
    std::iter::repeat_n(horizontal_rule_char(), width).collect()
}

pub fn horizontal_rule_char() -> char {
    if current().compatibility_mode {
        '-'
    } else {
        '─'
    }
}

pub fn tree_connector(is_last_child: bool) -> &'static str {
    if current().compatibility_mode {
        if is_last_child {
            "`-"
        } else {
            "|-"
        }
    } else if is_last_child {
        "└─"
    } else {
        "├─"
    }
}

pub fn tree_vertical_guide() -> &'static str {
    if current().compatibility_mode {
        "|   "
    } else {
        "│   "
    }
}

pub fn collapse_icon(expanded: bool) -> &'static str {
    if current().compatibility_mode {
        if expanded {
            "v "
        } else {
            "> "
        }
    } else if expanded {
        " "
    } else {
        " "
    }
}

pub fn folder_icon(expanded: bool, has_children: bool) -> &'static str {
    if current().compatibility_mode {
        match (expanded, has_children) {
            (true, _) => "G",
            (false, true) => "G",
            (false, false) => "g",
        }
    } else {
        match (expanded, has_children) {
            (true, true) => "",
            (true, false) => "",
            (false, true) => "",
            (false, false) => "",
        }
    }
}

pub fn root_file_icon() -> &'static str {
    if current().compatibility_mode {
        "F "
    } else {
        "󰈚 "
    }
}

pub fn dataset_icon() -> &'static str {
    if current().compatibility_mode {
        "D "
    } else {
        "󰈚 "
    }
}

pub fn dataset_link_icon() -> &'static str {
    if current().compatibility_mode {
        "D@"
    } else {
        "󰈚🔗"
    }
}

pub fn compound_container_icon() -> &'static str {
    if current().compatibility_mode {
        "C "
    } else {
        "󰆼 "
    }
}

pub fn compound_leaf_icon() -> &'static str {
    if current().compatibility_mode {
        "c "
    } else {
        "󰈚 "
    }
}

pub fn link_marker() -> &'static str {
    if current().compatibility_mode {
        "@"
    } else {
        "🔗"
    }
}

pub fn readonly_badge(readonly: bool) -> &'static str {
    if current().compatibility_mode {
        if readonly {
            " [ro] read-only "
        } else {
            " [rw] write "
        }
    } else if readonly {
        " 🔒 read-only "
    } else {
        " ✏ write "
    }
}

pub fn linked_badge() -> &'static str {
    if current().compatibility_mode {
        " linked "
    } else {
        " 🔗 linked "
    }
}

pub fn linked_root_suffix(member_count: usize) -> String {
    if current().compatibility_mode {
        format!(" ({member_count}) linked ")
    } else {
        format!(" ({member_count}) 🔗 linked ")
    }
}

pub fn app_brand() -> &'static str {
    if current().compatibility_mode {
        " h5v "
    } else {
        " 🔬 h5v "
    }
}

pub fn load_more_label() -> &'static str {
    if current().compatibility_mode {
        "Load more"
    } else {
        "⤵ Load more"
    }
}

pub fn section_title(title: &str) -> String {
    if current().compatibility_mode {
        return title.to_string();
    }
    match title {
        "Properties" => "󰜉 Properties".to_string(),
        "Attributes" => "󰠱 Attributes".to_string(),
        other => other.to_string(),
    }
}

pub fn tree_title() -> &'static str {
    if current().compatibility_mode {
        "Tree"
    } else {
        " 🔍 Tree "
    }
}

pub fn meta_title() -> &'static str {
    if current().compatibility_mode {
        "Meta"
    } else {
        " 🧾 Meta "
    }
}

pub fn file_metadata_title() -> &'static str {
    if current().compatibility_mode {
        " File metadata "
    } else {
        " 📄 File metadata "
    }
}

pub fn empty_group_title() -> &'static str {
    if current().compatibility_mode {
        " Empty group preview "
    } else {
        " 📁 Empty group preview "
    }
}

pub fn empty_dataset_title() -> &'static str {
    if current().compatibility_mode {
        " Empty dataset "
    } else {
        " 🧮 Empty dataset "
    }
}

pub fn error_title() -> &'static str {
    if current().compatibility_mode {
        "Error"
    } else {
        " ⚠ Error "
    }
}

pub fn create_attribute_title() -> &'static str {
    if current().compatibility_mode {
        " Create attribute "
    } else {
        " ✨ Create attribute "
    }
}

pub fn delete_attribute_title() -> &'static str {
    if current().compatibility_mode {
        " Delete attribute "
    } else {
        " 🗑 Delete attribute "
    }
}

pub fn fixed_string_overflow_title() -> &'static str {
    if current().compatibility_mode {
        " Fixed string overflow "
    } else {
        " 🧵 Fixed string overflow "
    }
}

pub fn fixed_string_resize_title() -> &'static str {
    if current().compatibility_mode {
        " Change fixed string size "
    } else {
        " 📏 Change fixed string size "
    }
}

pub fn help_title() -> &'static str {
    if current().compatibility_mode {
        " Help "
    } else {
        " ❔ Help "
    }
}

pub fn matrix_tab_title() -> &'static str {
    if current().compatibility_mode {
        "Matrix"
    } else {
        "Matrix🧮"
    }
}

pub fn chart_membership_marker() -> &'static str {
    if current().compatibility_mode {
        "*"
    } else {
        "●"
    }
}

pub fn chart_visibility_marker(visible: bool) -> &'static str {
    if current().compatibility_mode {
        if visible {
            "*"
        } else {
            "o"
        }
    } else if visible {
        "●"
    } else {
        "○"
    }
}

pub fn enum_symbol(slot: usize) -> &'static str {
    const RICH_SYMBOLS: [&str; 8] = ["●", "■", "▲", "◆", "✦", "✚", "⬢", "◉"];
    const COMPAT_SYMBOLS: [&str; 8] = ["*", "+", "^", "#", "x", "%", "@", "o"];
    let symbols = if current().compatibility_mode {
        &COMPAT_SYMBOLS
    } else {
        &RICH_SYMBOLS
    };
    symbols[slot % symbols.len()]
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
#[allow(clippy::expect_used)]
mod tests {
    use std::ffi::OsString;

    use super::resolve_runtime_config;

    #[test]
    fn compatibility_env_enables_compatibility_mode() {
        let config = resolve_runtime_config(false, false, Some(OsString::from("true").as_os_str()))
            .expect("config");
        assert!(config.compatibility_mode);
        assert!(!config.terminal_graphics);
    }

    #[test]
    fn compatibility_flag_takes_effect_without_env() {
        let config = resolve_runtime_config(true, false, None).expect("config");
        assert!(config.compatibility_mode);
        assert!(!config.terminal_graphics);
    }

    #[test]
    fn no_terminal_graphics_only_disables_graphics() {
        let config = resolve_runtime_config(false, true, None).expect("config");
        assert!(!config.compatibility_mode);
        assert!(!config.terminal_graphics);
    }

    #[test]
    fn invalid_compatibility_env_errors() {
        let error = resolve_runtime_config(false, false, Some(OsString::from("maybe").as_os_str()))
            .expect_err("invalid env should fail");
        assert!(error
            .to_string()
            .contains("Invalid H5V_COMPATIBILITY_MODE value"));
    }
}
