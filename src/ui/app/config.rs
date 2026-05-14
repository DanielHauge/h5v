use std::sync::mpsc::Sender;

use super::AppEvent;
use crate::{
    compat::RuntimeConfig,
    configure,
    configure::{ensure_config_path, reset_config_path, run_lua_engine},
    error::{log_error, AppError},
    ui::{
        edit::edit_existing_file,
        state::{AppState, AppToast},
    },
};

type Result<T> = std::result::Result<T, AppError>;

pub(super) fn configuration_warning_message(
    error: &impl std::fmt::Display,
    keeping_previous: bool,
) -> String {
    if keeping_previous {
        format!("Configuration warning: {error}. Keeping previous settings.")
    } else {
        format!("Configuration warning: {error}. Using built-in settings.")
    }
}

pub(super) fn open_configuration_and_reload(
    state: &mut AppState<'_>,
    tx_events: Sender<AppEvent>,
    reset: bool,
) -> Result<AppToast> {
    let config_path = if reset {
        reset_config_path()?
    } else {
        let config_path = ensure_config_path()?;
        state.editing = true;
        let edit_result = edit_existing_file(state, &config_path);
        state.editing = false;
        edit_result?;
        config_path
    };

    if let Err(error) = run_lua_engine(tx_events, state.compatibility_mode) {
        log_configuration_error(&error);
        let message = configuration_warning_message(&error, true);
        state.configuration_warning = Some(message.clone());
        return Ok(AppToast::Warning(message));
    }
    if let Ok(Some(compatibility_mode)) =
        configure::load_config_compatibility(state.compatibility_mode)
    {
        state.compatibility_mode = compatibility_mode;
        if compatibility_mode {
            state.image_protocol_enabled = false;
        }
    }
    state.configuration_warning = None;
    if let Some(preferred_mode) = configure::current_content_mode_order().first().copied() {
        state.content_mode = preferred_mode;
    }
    state.sync_heatmap_configuration();
    state.compute_tree_view();
    let config_path = config_path.display();
    if reset {
        Ok(AppToast::Info(format!(
            "Reset configuration to default at {config_path}"
        )))
    } else {
        Ok(AppToast::Info(format!(
            "Reloaded configuration from {config_path}"
        )))
    }
}

pub(super) fn log_configuration_error(error: &impl std::fmt::Display) {
    log_error(format!("Configuration error: {error}\n"));
}

pub(super) fn should_use_alternate_screen(
    runtime_config: RuntimeConfig,
    cros_container: Option<&str>,
) -> bool {
    runtime_config.terminal_graphics || !is_crostini_env(cros_container)
}

pub(super) fn is_crostini_env(cros_container: Option<&str>) -> bool {
    cros_container.map(str::trim).is_some_and(|value| {
        !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
    })
}

#[cfg(test)]
mod tests {
    use super::{configuration_warning_message, is_crostini_env, should_use_alternate_screen};
    use crate::compat::RuntimeConfig;

    #[test]
    fn detects_crostini_from_cros_container() {
        assert!(is_crostini_env(Some("1")));
        assert!(is_crostini_env(Some("penguin")));
    }

    #[test]
    fn ignores_empty_or_false_cros_container() {
        assert!(!is_crostini_env(None));
        assert!(!is_crostini_env(Some("")));
        assert!(!is_crostini_env(Some("0")));
        assert!(!is_crostini_env(Some("false")));
    }

    #[test]
    fn keeps_alternate_screen_without_safe_flag() {
        assert!(should_use_alternate_screen(
            RuntimeConfig::default(),
            Some("1")
        ));
    }

    #[test]
    fn disables_alternate_screen_for_crostini_safe_mode() {
        assert!(!should_use_alternate_screen(
            RuntimeConfig {
                compatibility_mode: true,
                terminal_graphics: false,
            },
            Some("1"),
        ));
    }

    #[test]
    fn formats_configuration_warning_by_context() {
        assert_eq!(
            configuration_warning_message(&"bad config", true),
            "Configuration warning: bad config. Keeping previous settings."
        );
        assert_eq!(
            configuration_warning_message(&"bad config", false),
            "Configuration warning: bad config. Using built-in settings."
        );
    }
}
