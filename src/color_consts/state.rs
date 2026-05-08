use std::sync::{LazyLock, RwLock};

use ratatui::prelude::Color;

use crate::compat;

use super::{
    catalog::available_color_names,
    types::{ThemeColors, ThemeName, ThemeSnapshot, ThemeState},
};

static THEME_STATE: LazyLock<RwLock<ThemeState>> = LazyLock::new(|| {
    RwLock::new(ThemeState {
        active_theme: ThemeName::Dark,
        colors: ThemeColors::for_theme(ThemeName::Dark),
    })
});

pub fn reset_theme(theme: ThemeName) {
    with_theme_write(|state| {
        state.active_theme = theme;
        state.colors = ThemeColors::for_theme(theme);
    });
}

pub fn snapshot_theme() -> ThemeSnapshot {
    with_theme_read(|state| ThemeSnapshot {
        active_theme: state.active_theme,
        colors: state.colors.clone(),
    })
}

pub fn restore_theme(snapshot: ThemeSnapshot) {
    with_theme_write(|state| {
        state.active_theme = snapshot.active_theme;
        state.colors = snapshot.colors;
    });
}

pub fn set_color_override(name: &str, color: Color) -> Result<(), String> {
    with_theme_write(|state| {
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

pub fn current_theme_name() -> ThemeName {
    with_theme_read(|state| state.active_theme)
}

pub fn prefers_strong_text() -> bool {
    matches!(current_theme_name(), ThemeName::Light)
}

pub(crate) fn themed_color(getter: impl FnOnce(&ThemeColors) -> Color) -> Color {
    with_theme_read(|state| getter(&state.colors))
}

pub(crate) fn compat_color(rich: Color, fallback: Color) -> Color {
    if compat::current().compatibility_mode {
        fallback
    } else {
        rich
    }
}

fn with_theme_read<R>(f: impl FnOnce(&ThemeState) -> R) -> R {
    let guard = match THEME_STATE.read() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    f(&guard)
}

fn with_theme_write<R>(f: impl FnOnce(&mut ThemeState) -> R) -> R {
    let mut guard = match THEME_STATE.write() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    f(&mut guard)
}
