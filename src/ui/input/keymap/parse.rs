use std::fmt;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::error::AppError;

use super::KeyPattern;

impl KeyPattern {
    pub fn to_key_event(self) -> KeyEvent {
        KeyEvent::new(self.code, self.modifiers)
    }

    pub fn matches(self, key: &KeyEvent) -> bool {
        let (code, modifiers) = normalize_key_parts(key.code, key.modifiers);
        if self.code != code {
            return false;
        }
        let required_non_shift = self.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT);
        let actual_non_shift = modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT);
        if required_non_shift != actual_non_shift {
            return false;
        }
        if matches!(self.code, KeyCode::Char(_)) {
            !self.modifiers.contains(KeyModifiers::SHIFT) || modifiers.contains(KeyModifiers::SHIFT)
        } else {
            self.modifiers == modifiers
        }
    }
}

impl fmt::Display for KeyPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt".to_string());
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift".to_string());
        }
        parts.push(key_code_name(self.code));
        write!(f, "{}", parts.join("+"))
    }
}

pub fn parse_key_pattern(key_spec: &str) -> Result<KeyPattern, String> {
    let normalized = key_spec.trim();
    if normalized.is_empty() {
        return Err("Key spec cannot be empty".to_string());
    }
    if normalized == "+" {
        return Ok(KeyPattern {
            code: KeyCode::Char('+'),
            modifiers: KeyModifiers::NONE,
        });
    }

    let parts = normalized
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(format!("Invalid key spec '{key_spec}'"));
    }

    let mut modifiers = KeyModifiers::NONE;
    let mut key_name = None;
    for part in parts {
        let lowered = part.to_ascii_lowercase();
        match lowered.as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" | "meta" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ if key_name.is_none() => key_name = Some(part.to_string()),
            _ => return Err(format!("Invalid key spec '{key_spec}'")),
        }
    }

    let Some(key_name) = key_name else {
        return Err(format!("Invalid key spec '{key_spec}'"));
    };

    let lowered = key_name.to_ascii_lowercase();
    let code = match lowered.as_str() {
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "shift-tab" | "backtab" => {
            modifiers |= KeyModifiers::SHIFT;
            KeyCode::BackTab
        }
        "space" => KeyCode::Char(' '),
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "page-up" => KeyCode::PageUp,
        "pagedown" | "page-down" => KeyCode::PageDown,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        _ if key_name.chars().count() == 1 => {
            let ch = key_name
                .chars()
                .next()
                .ok_or_else(|| format!("Invalid key spec '{key_spec}'"))?;
            KeyCode::Char(ch)
        }
        _ => return Err(format!("Invalid key spec '{key_spec}'")),
    };

    let (code, modifiers) = normalize_key_parts(code, modifiers);
    Ok(KeyPattern { code, modifiers })
}

pub fn parse_simulated_key(key_spec: &str) -> Result<KeyEvent, AppError> {
    parse_key_pattern(key_spec)
        .map(KeyPattern::to_key_event)
        .map_err(AppError::InvalidCommand)
}

fn normalize_key_parts(code: KeyCode, modifiers: KeyModifiers) -> (KeyCode, KeyModifiers) {
    let mut modifiers =
        modifiers & (KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT);
    let code = match code {
        KeyCode::BackTab => {
            modifiers |= KeyModifiers::SHIFT;
            KeyCode::BackTab
        }
        KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
        other => other,
    };
    (code, modifiers)
}

fn key_code_name(code: KeyCode) -> String {
    match code {
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "Tab".to_string(),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        _ => "Key".to_string(),
    }
}
