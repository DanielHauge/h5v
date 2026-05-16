use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::{
    catalogs::{
        attributes_action_codes, content_action_codes, global_action_codes,
        multichart_action_codes, normal_action_codes, tree_action_codes, window_action_codes,
    },
    parse_key_pattern, ActionCode, BoundAction, KeyBinding, KeyPattern,
};

fn binding<T>(key: &str, target: BoundAction<T>) -> KeyBinding<T> {
    KeyBinding {
        key: match parse_key_pattern(key) {
            Ok(key) => key,
            Err(error) => {
                eprintln!("Invalid built-in key binding '{key}': {error}");
                KeyPattern {
                    code: KeyCode::F(24),
                    modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT,
                }
            }
        },
        target,
        description: None,
    }
}

fn bindings_from_action_codes<T: Copy>(codes: &[ActionCode<T>]) -> Vec<KeyBinding<T>> {
    codes
        .iter()
        .flat_map(|entry| {
            entry
                .default_keys
                .iter()
                .map(move |key| binding(key, BoundAction::Action(entry.action)))
        })
        .collect()
}

pub(super) fn default_global_bindings() -> Vec<KeyBinding<super::GlobalAction>> {
    bindings_from_action_codes(global_action_codes())
}

pub(super) fn default_normal_bindings() -> Vec<KeyBinding<super::NormalAction>> {
    bindings_from_action_codes(normal_action_codes())
}

pub(super) fn default_window_bindings() -> Vec<KeyBinding<super::WindowAction>> {
    bindings_from_action_codes(window_action_codes())
}

pub(super) fn default_tree_bindings() -> Vec<KeyBinding<super::TreeAction>> {
    bindings_from_action_codes(tree_action_codes())
}

pub(super) fn default_content_bindings() -> Vec<KeyBinding<super::ContentAction>> {
    bindings_from_action_codes(content_action_codes())
}

pub(super) fn default_heatmap_bindings() -> Vec<KeyBinding<super::ContentAction>> {
    bindings_from_action_codes(
        content_action_codes()
            .iter()
            .filter(|entry| entry.code.starts_with("heatmap-"))
            .copied()
            .collect::<Vec<_>>()
            .as_slice(),
    )
}

pub(super) fn default_attributes_bindings() -> Vec<KeyBinding<super::AttributesAction>> {
    bindings_from_action_codes(attributes_action_codes())
}

pub(super) fn default_multichart_bindings() -> Vec<KeyBinding<super::MultiChartAction>> {
    bindings_from_action_codes(multichart_action_codes())
}
