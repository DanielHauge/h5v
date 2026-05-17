use std::collections::BTreeMap;

use super::{
    ids::{
        ColorHandle, CommandHandle, ContentModeHandle, EventHandle, MchartFunctionHandle,
        PluginHandle, SettingHandle, SymbolHandle, ThemeHandle,
    },
    metadata::{
        ColorMetadata, CommandMetadata, ContentModeMetadata, EventMetadata, MchartFunctionMetadata,
        PluginMetadata, RegistryItemKind, SettingMetadata, SymbolMetadata, ThemeMetadata,
    },
    runtime::{RegistrySnapshot, RegistrySnapshotParts},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateId {
        kind: RegistryItemKind,
        id: String,
    },
    DuplicateCommandLookup {
        lookup: String,
        existing: String,
        candidate: String,
    },
    DuplicateMchartFunctionLookup {
        lookup: String,
        existing: String,
        candidate: String,
    },
    UnknownHandle {
        kind: RegistryItemKind,
        id: String,
    },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId { kind, id } => {
                write!(f, "Duplicate {kind} id '{id}'")
            }
            Self::DuplicateCommandLookup {
                lookup,
                existing,
                candidate,
            } => write!(
                f,
                "Duplicate command lookup '{lookup}' for '{candidate}' conflicts with '{existing}'"
            ),
            Self::DuplicateMchartFunctionLookup {
                lookup,
                existing,
                candidate,
            } => write!(
                f,
                "Duplicate multichart function lookup '{lookup}' for '{candidate}' conflicts with '{existing}'"
            ),
            Self::UnknownHandle { kind, id } => write!(f, "Unknown {kind} handle '{id}'"),
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Debug, Default)]
pub struct RegistryBuilder {
    settings: BTreeMap<SettingHandle, SettingMetadata>,
    themes: BTreeMap<ThemeHandle, ThemeMetadata>,
    colors: BTreeMap<ColorHandle, ColorMetadata>,
    symbols: BTreeMap<SymbolHandle, SymbolMetadata>,
    commands: BTreeMap<CommandHandle, CommandMetadata>,
    events: BTreeMap<EventHandle, EventMetadata>,
    mchart_functions: BTreeMap<MchartFunctionHandle, MchartFunctionMetadata>,
    content_modes: BTreeMap<ContentModeHandle, ContentModeMetadata>,
    plugins: BTreeMap<PluginHandle, PluginMetadata>,
}

impl RegistryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_setting(
        &mut self,
        metadata: SettingMetadata,
    ) -> Result<SettingHandle, RegistryError> {
        let handle = metadata.handle.clone();
        Self::insert_unique(
            RegistryItemKind::Setting,
            handle.to_string(),
            &mut self.settings,
            handle.clone(),
            metadata,
        )?;
        Ok(handle)
    }

    pub fn update_setting(
        &mut self,
        handle: &SettingHandle,
        update: impl FnOnce(&mut SettingMetadata),
    ) -> Result<(), RegistryError> {
        let metadata =
            self.settings
                .get_mut(handle)
                .ok_or_else(|| RegistryError::UnknownHandle {
                    kind: RegistryItemKind::Setting,
                    id: handle.to_string(),
                })?;
        update(metadata);
        Ok(())
    }

    pub fn register_theme(
        &mut self,
        metadata: ThemeMetadata,
    ) -> Result<ThemeHandle, RegistryError> {
        let handle = metadata.handle.clone();
        Self::insert_unique(
            RegistryItemKind::Theme,
            handle.to_string(),
            &mut self.themes,
            handle.clone(),
            metadata,
        )?;
        Ok(handle)
    }

    pub fn update_theme(
        &mut self,
        handle: &ThemeHandle,
        update: impl FnOnce(&mut ThemeMetadata),
    ) -> Result<(), RegistryError> {
        let metadata = self
            .themes
            .get_mut(handle)
            .ok_or_else(|| RegistryError::UnknownHandle {
                kind: RegistryItemKind::Theme,
                id: handle.to_string(),
            })?;
        update(metadata);
        Ok(())
    }

    pub fn register_color(
        &mut self,
        metadata: ColorMetadata,
    ) -> Result<ColorHandle, RegistryError> {
        let handle = metadata.handle.clone();
        Self::insert_unique(
            RegistryItemKind::Color,
            handle.to_string(),
            &mut self.colors,
            handle.clone(),
            metadata,
        )?;
        Ok(handle)
    }

    pub fn update_color(
        &mut self,
        handle: &ColorHandle,
        update: impl FnOnce(&mut ColorMetadata),
    ) -> Result<(), RegistryError> {
        let metadata = self
            .colors
            .get_mut(handle)
            .ok_or_else(|| RegistryError::UnknownHandle {
                kind: RegistryItemKind::Color,
                id: handle.to_string(),
            })?;
        update(metadata);
        Ok(())
    }

    pub fn register_symbol(
        &mut self,
        metadata: SymbolMetadata,
    ) -> Result<SymbolHandle, RegistryError> {
        let handle = metadata.handle.clone();
        Self::insert_unique(
            RegistryItemKind::Symbol,
            handle.to_string(),
            &mut self.symbols,
            handle.clone(),
            metadata,
        )?;
        Ok(handle)
    }

    pub fn update_symbol(
        &mut self,
        handle: &SymbolHandle,
        update: impl FnOnce(&mut SymbolMetadata),
    ) -> Result<(), RegistryError> {
        let metadata =
            self.symbols
                .get_mut(handle)
                .ok_or_else(|| RegistryError::UnknownHandle {
                    kind: RegistryItemKind::Symbol,
                    id: handle.to_string(),
                })?;
        update(metadata);
        Ok(())
    }

    pub fn register_command(
        &mut self,
        metadata: CommandMetadata,
    ) -> Result<CommandHandle, RegistryError> {
        let handle = metadata.handle.clone();
        Self::insert_unique(
            RegistryItemKind::Command,
            handle.to_string(),
            &mut self.commands,
            handle.clone(),
            metadata,
        )?;
        Ok(handle)
    }

    pub fn register_event(
        &mut self,
        metadata: EventMetadata,
    ) -> Result<EventHandle, RegistryError> {
        let handle = metadata.handle.clone();
        Self::insert_unique(
            RegistryItemKind::Event,
            handle.to_string(),
            &mut self.events,
            handle.clone(),
            metadata,
        )?;
        Ok(handle)
    }

    pub fn register_mchart_function(
        &mut self,
        metadata: MchartFunctionMetadata,
    ) -> Result<MchartFunctionHandle, RegistryError> {
        let handle = metadata.handle.clone();
        Self::insert_unique(
            RegistryItemKind::MchartFunction,
            handle.to_string(),
            &mut self.mchart_functions,
            handle.clone(),
            metadata,
        )?;
        Ok(handle)
    }

    pub fn register_content_mode(
        &mut self,
        metadata: ContentModeMetadata,
    ) -> Result<ContentModeHandle, RegistryError> {
        let handle = metadata.handle.clone();
        Self::insert_unique(
            RegistryItemKind::ContentMode,
            handle.to_string(),
            &mut self.content_modes,
            handle.clone(),
            metadata,
        )?;
        Ok(handle)
    }

    pub fn register_plugin(
        &mut self,
        metadata: PluginMetadata,
    ) -> Result<PluginHandle, RegistryError> {
        let handle = metadata.handle.clone();
        Self::insert_unique(
            RegistryItemKind::Plugin,
            handle.to_string(),
            &mut self.plugins,
            handle.clone(),
            metadata,
        )?;
        Ok(handle)
    }

    pub fn freeze(self) -> Result<RegistrySnapshot, RegistryError> {
        let mut command_lookup = BTreeMap::new();
        let mut mchart_function_lookup = BTreeMap::new();
        for metadata in self.commands.values() {
            Self::insert_command_lookup(
                &mut command_lookup,
                metadata.name.as_str(),
                metadata.handle.as_str(),
            )?;
            for alias in &metadata.aliases {
                if alias.eq_ignore_ascii_case(&metadata.name) {
                    continue;
                }
                Self::insert_command_lookup(&mut command_lookup, alias, metadata.handle.as_str())?;
            }
        }
        for metadata in self.mchart_functions.values() {
            Self::insert_mchart_function_lookup(
                &mut mchart_function_lookup,
                metadata.name.as_str(),
                metadata.handle.as_str(),
            )?;
        }

        Ok(RegistrySnapshot::new(RegistrySnapshotParts {
            settings: self.settings,
            themes: self.themes,
            colors: self.colors,
            symbols: self.symbols,
            commands: self.commands,
            events: self.events,
            mchart_functions: self.mchart_functions,
            content_modes: self.content_modes,
            plugins: self.plugins,
            command_lookup,
            mchart_function_lookup,
        }))
    }

    fn insert_command_lookup(
        lookup: &mut BTreeMap<String, CommandHandle>,
        value: &str,
        handle: &str,
    ) -> Result<(), RegistryError> {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Ok(());
        }
        if let Some(existing) = lookup.get(&normalized) {
            if existing.as_str() != handle {
                return Err(RegistryError::DuplicateCommandLookup {
                    lookup: value.to_string(),
                    existing: existing.to_string(),
                    candidate: handle.to_string(),
                });
            }
            return Ok(());
        }
        lookup.insert(normalized, CommandHandle::new(handle));
        Ok(())
    }

    fn insert_mchart_function_lookup(
        lookup: &mut BTreeMap<String, MchartFunctionHandle>,
        value: &str,
        handle: &str,
    ) -> Result<(), RegistryError> {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Ok(());
        }
        if let Some(existing) = lookup.get(&normalized) {
            if existing.as_str() != handle {
                return Err(RegistryError::DuplicateMchartFunctionLookup {
                    lookup: value.to_string(),
                    existing: existing.to_string(),
                    candidate: handle.to_string(),
                });
            }
            return Ok(());
        }
        lookup.insert(normalized, MchartFunctionHandle::new(handle));
        Ok(())
    }

    fn insert_unique<K: Ord, V>(
        kind: RegistryItemKind,
        id: String,
        map: &mut BTreeMap<K, V>,
        key: K,
        value: V,
    ) -> Result<(), RegistryError> {
        match map.entry(key) {
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(RegistryError::DuplicateId { kind, id })
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(())
            }
        }
    }
}
