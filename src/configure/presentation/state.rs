use std::sync::{LazyLock, RwLock};

use ratatui::prelude::Color;

use crate::compat;
use crate::ui::state::ContentShowMode;

use super::{
    catalog::{available_color_names, available_symbol_names},
    palette::{SymbolThemeName, ThemeName},
    types::{ConfigSnapshot, ConfigState, ThemeColors, UiSymbols},
};

fn default_symbol_theme() -> SymbolThemeName {
    if compat::current().compatibility_mode {
        SymbolThemeName::Compatibility
    } else {
        SymbolThemeName::Rich
    }
}

fn default_content_mode_order() -> Vec<ContentShowMode> {
    vec![ContentShowMode::Preview, ContentShowMode::Matrix]
}

static CONFIG_STATE: LazyLock<RwLock<ConfigState>> = LazyLock::new(|| {
    let symbol_theme = default_symbol_theme();
    RwLock::new(ConfigState {
        active_theme: ThemeName::Dark,
        active_symbol_theme: symbol_theme,
        colors: ThemeColors::for_theme(ThemeName::Dark),
        symbols: UiSymbols::for_theme(symbol_theme),
        content_mode_order: default_content_mode_order(),
    })
});

pub fn reset_config(theme: ThemeName) {
    with_config_write(|state| {
        let symbol_theme = default_symbol_theme();
        state.active_theme = theme;
        state.active_symbol_theme = symbol_theme;
        state.colors = ThemeColors::for_theme(theme);
        state.symbols = UiSymbols::for_theme(symbol_theme);
        state.content_mode_order = default_content_mode_order();
    });
}

pub fn reset_symbol_theme(theme: SymbolThemeName) {
    with_config_write(|state| {
        state.active_symbol_theme = theme;
        state.symbols = UiSymbols::for_theme(theme);
    });
}

pub fn snapshot_config() -> ConfigSnapshot {
    with_config_read(|state| ConfigSnapshot {
        active_theme: state.active_theme,
        active_symbol_theme: state.active_symbol_theme,
        colors: state.colors.clone(),
        symbols: state.symbols.clone(),
        content_mode_order: state.content_mode_order.clone(),
    })
}

pub fn restore_config(snapshot: ConfigSnapshot) {
    with_config_write(|state| {
        state.active_theme = snapshot.active_theme;
        state.active_symbol_theme = snapshot.active_symbol_theme;
        state.colors = snapshot.colors;
        state.symbols = snapshot.symbols;
        state.content_mode_order = snapshot.content_mode_order;
    });
}

pub fn set_color_override(name: &str, color: Color) -> Result<(), String> {
    with_config_write(|state| {
        if state.colors.set_named_color(name, color) {
            Ok(())
        } else {
            Err(format!(
                "Unknown color '{name}'. Available colors: {}",
                available_color_names().join(", ")
            ))
        }
    })
}

pub fn set_symbol_override(name: &str, value: &str) -> Result<(), String> {
    with_config_write(|state| {
        if state.symbols.set_named_symbol(name, value) {
            Ok(())
        } else {
            Err(format!(
                "Unknown symbol '{name}'. Available symbols: {}",
                available_symbol_names().join(", ")
            ))
        }
    })
}

pub fn current_theme_name() -> ThemeName {
    with_config_read(|state| state.active_theme)
}

pub fn current_symbol_theme_name() -> SymbolThemeName {
    with_config_read(|state| state.active_symbol_theme)
}

pub fn set_content_mode_order(order: &[ContentShowMode]) {
    with_config_write(|state| {
        state.content_mode_order = normalize_content_mode_order(order);
    });
}

pub fn ordered_content_modes(available: &[ContentShowMode]) -> Vec<ContentShowMode> {
    with_config_read(|state| {
        let mut ordered = Vec::new();
        for preferred in &state.content_mode_order {
            if available.contains(preferred) && !ordered.contains(preferred) {
                ordered.push(*preferred);
            }
        }
        for mode in available {
            if !ordered.contains(mode) {
                ordered.push(*mode);
            }
        }
        ordered
    })
}

pub fn current_content_mode_order() -> Vec<ContentShowMode> {
    with_config_read(|state| state.content_mode_order.clone())
}

pub fn prefers_strong_text() -> bool {
    matches!(current_theme_name(), ThemeName::Light)
}

pub(crate) fn themed_color(getter: impl FnOnce(&ThemeColors) -> Color) -> Color {
    with_config_read(|state| getter(&state.colors))
}

pub(crate) fn configured_symbol(getter: impl FnOnce(&UiSymbols) -> &'static str) -> &'static str {
    with_config_read(|state| getter(&state.symbols))
}

fn normalize_content_mode_order(order: &[ContentShowMode]) -> Vec<ContentShowMode> {
    let mut normalized = Vec::new();
    for mode in order {
        if !normalized.contains(mode) {
            normalized.push(*mode);
        }
    }
    for mode in default_content_mode_order() {
        if !normalized.contains(&mode) {
            normalized.push(mode);
        }
    }
    normalized
}

fn with_config_read<R>(f: impl FnOnce(&ConfigState) -> R) -> R {
    let guard = match CONFIG_STATE.read() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    f(&guard)
}

fn with_config_write<R>(f: impl FnOnce(&mut ConfigState) -> R) -> R {
    let mut guard = match CONFIG_STATE.write() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    f(&mut guard)
}
