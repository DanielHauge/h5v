use std::{fs, path::PathBuf};

use ratatui_image::{
    picker::{Picker, ProtocolType},
    FontSize,
};
use serde::{Deserialize, Serialize};

const PICKER_CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PickerCacheKey {
    term: Option<String>,
    colorterm: Option<String>,
    term_program: Option<String>,
    tmux: Option<String>,
    kitty_window_id_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PickerCacheEntry {
    version: u32,
    key: PickerCacheKey,
    protocol: String,
    cell_width: u16,
    cell_height: u16,
}

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|path| path.join("h5v").join("picker.json"))
}

fn current_key() -> PickerCacheKey {
    PickerCacheKey {
        term: std::env::var("TERM").ok(),
        colorterm: std::env::var("COLORTERM").ok(),
        term_program: std::env::var("TERM_PROGRAM").ok(),
        tmux: std::env::var("TMUX").ok(),
        kitty_window_id_present: std::env::var_os("KITTY_WINDOW_ID").is_some(),
    }
}

fn protocol_to_str(protocol: ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::Halfblocks => "halfblocks",
        ProtocolType::Sixel => "sixel",
        ProtocolType::Kitty => "kitty",
        ProtocolType::Iterm2 => "iterm2",
    }
}

fn protocol_from_str(value: &str) -> Option<ProtocolType> {
    match value {
        "halfblocks" => Some(ProtocolType::Halfblocks),
        "sixel" => Some(ProtocolType::Sixel),
        "kitty" => Some(ProtocolType::Kitty),
        "iterm2" => Some(ProtocolType::Iterm2),
        _ => None,
    }
}

#[allow(deprecated)]
fn picker_from_cache(entry: &PickerCacheEntry) -> Option<Picker> {
    if entry.version != PICKER_CACHE_VERSION || entry.key != current_key() {
        return None;
    }
    if entry.cell_width == 0 || entry.cell_height == 0 {
        return None;
    }
    let protocol = protocol_from_str(&entry.protocol)?;
    let mut picker = Picker::from_fontsize(FontSize::new(entry.cell_width, entry.cell_height));
    picker.set_protocol_type(protocol);
    Some(picker)
}

pub(super) fn load_cached_picker() -> Option<Picker> {
    let path = cache_path()?;
    let contents = fs::read_to_string(path).ok()?;
    let entry: PickerCacheEntry = serde_json::from_str(&contents).ok()?;
    picker_from_cache(&entry)
}

pub(super) fn store_picker(picker: &Picker) {
    let Some(path) = cache_path() else {
        return;
    };
    let font_size = picker.font_size();
    if font_size.width == 0 || font_size.height == 0 {
        return;
    }
    let entry = PickerCacheEntry {
        version: PICKER_CACHE_VERSION,
        key: current_key(),
        protocol: protocol_to_str(picker.protocol_type()).to_string(),
        cell_width: font_size.width,
        cell_height: font_size.height,
    };
    let Ok(contents) = serde_json::to_string(&entry) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, contents);
}

#[cfg(test)]
mod tests {
    use super::{picker_from_cache, PickerCacheEntry, PickerCacheKey, PICKER_CACHE_VERSION};
    use ratatui_image::picker::ProtocolType;

    #[test]
    fn cached_picker_rejects_unknown_protocol() {
        let entry = PickerCacheEntry {
            version: PICKER_CACHE_VERSION,
            key: PickerCacheKey {
                term: std::env::var("TERM").ok(),
                colorterm: std::env::var("COLORTERM").ok(),
                term_program: std::env::var("TERM_PROGRAM").ok(),
                tmux: std::env::var("TMUX").ok(),
                kitty_window_id_present: std::env::var_os("KITTY_WINDOW_ID").is_some(),
            },
            protocol: "bogus".to_string(),
            cell_width: 10,
            cell_height: 20,
        };
        assert!(picker_from_cache(&entry).is_none());
    }

    #[test]
    fn cached_picker_restores_protocol_and_font_size() {
        let entry = PickerCacheEntry {
            version: PICKER_CACHE_VERSION,
            key: PickerCacheKey {
                term: std::env::var("TERM").ok(),
                colorterm: std::env::var("COLORTERM").ok(),
                term_program: std::env::var("TERM_PROGRAM").ok(),
                tmux: std::env::var("TMUX").ok(),
                kitty_window_id_present: std::env::var_os("KITTY_WINDOW_ID").is_some(),
            },
            protocol: "kitty".to_string(),
            cell_width: 9,
            cell_height: 18,
        };
        let picker = picker_from_cache(&entry).expect("picker");
        assert_eq!(picker.protocol_type(), ProtocolType::Kitty);
        let font_size = picker.font_size();
        assert_eq!(font_size.width, 9);
        assert_eq!(font_size.height, 18);
    }
}
