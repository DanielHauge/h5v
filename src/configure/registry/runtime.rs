use std::{
    collections::BTreeMap,
    sync::{LazyLock, RwLock},
};

use super::{
    ids::{
        ColorHandle, CommandHandle, ContentModeHandle, EventHandle, MchartFunctionHandle,
        PluginHandle, SettingHandle, SymbolHandle, ThemeHandle,
    },
    metadata::{
        ColorMetadata, CommandMetadata, ContentModeMetadata, EventMetadata, MchartFunctionMetadata,
        PluginMetadata, SettingMetadata, SymbolMetadata, ThemeMetadata,
    },
};

#[derive(Debug, Clone)]
pub struct RegistrySnapshot {
    pub(crate) settings: BTreeMap<SettingHandle, SettingMetadata>,
    pub(crate) themes: BTreeMap<ThemeHandle, ThemeMetadata>,
    pub(crate) colors: BTreeMap<ColorHandle, ColorMetadata>,
    pub(crate) symbols: BTreeMap<SymbolHandle, SymbolMetadata>,
    pub(crate) commands: BTreeMap<CommandHandle, CommandMetadata>,
    pub(crate) events: BTreeMap<EventHandle, EventMetadata>,
    pub(crate) mchart_functions: BTreeMap<MchartFunctionHandle, MchartFunctionMetadata>,
    pub(crate) content_modes: BTreeMap<ContentModeHandle, ContentModeMetadata>,
    pub(crate) plugins: BTreeMap<PluginHandle, PluginMetadata>,
    pub(crate) command_lookup: BTreeMap<String, CommandHandle>,
    pub(crate) mchart_function_lookup: BTreeMap<String, MchartFunctionHandle>,
}

static REGISTRY_SNAPSHOT: LazyLock<RwLock<RegistrySnapshot>> =
    LazyLock::new(|| RwLock::new(RegistrySnapshot::empty()));

impl RegistrySnapshot {
    pub(crate) fn new(parts: RegistrySnapshotParts) -> Self {
        Self {
            settings: parts.settings,
            themes: parts.themes,
            colors: parts.colors,
            symbols: parts.symbols,
            commands: parts.commands,
            events: parts.events,
            mchart_functions: parts.mchart_functions,
            content_modes: parts.content_modes,
            plugins: parts.plugins,
            command_lookup: parts.command_lookup,
            mchart_function_lookup: parts.mchart_function_lookup,
        }
    }

    pub(crate) fn empty() -> Self {
        Self::new(RegistrySnapshotParts::default())
    }

    pub fn settings(&self) -> impl Iterator<Item = &SettingMetadata> {
        self.settings.values()
    }

    pub fn themes(&self) -> impl Iterator<Item = &ThemeMetadata> {
        self.themes.values()
    }

    pub fn colors(&self) -> impl Iterator<Item = &ColorMetadata> {
        self.colors.values()
    }

    pub fn symbols(&self) -> impl Iterator<Item = &SymbolMetadata> {
        self.symbols.values()
    }

    pub fn commands(&self) -> impl Iterator<Item = &CommandMetadata> {
        self.commands.values()
    }

    pub fn events(&self) -> impl Iterator<Item = &EventMetadata> {
        self.events.values()
    }

    pub fn mchart_functions(&self) -> impl Iterator<Item = &MchartFunctionMetadata> {
        self.mchart_functions.values()
    }

    pub fn content_modes(&self) -> impl Iterator<Item = &ContentModeMetadata> {
        self.content_modes.values()
    }

    pub fn plugins(&self) -> impl Iterator<Item = &PluginMetadata> {
        self.plugins.values()
    }

    pub fn setting(&self, handle: &SettingHandle) -> Option<&SettingMetadata> {
        self.settings.get(handle)
    }

    pub fn theme(&self, handle: &ThemeHandle) -> Option<&ThemeMetadata> {
        self.themes.get(handle)
    }

    pub fn color(&self, handle: &ColorHandle) -> Option<&ColorMetadata> {
        self.colors.get(handle)
    }

    pub fn symbol(&self, handle: &SymbolHandle) -> Option<&SymbolMetadata> {
        self.symbols.get(handle)
    }

    pub fn command(&self, handle: &CommandHandle) -> Option<&CommandMetadata> {
        self.commands.get(handle)
    }

    pub fn event(&self, handle: &EventHandle) -> Option<&EventMetadata> {
        self.events.get(handle)
    }

    pub fn find_command(&self, name_or_alias: &str) -> Option<&CommandMetadata> {
        let key = name_or_alias.trim().to_ascii_lowercase();
        let handle = self.command_lookup.get(&key)?;
        self.commands.get(handle)
    }

    pub fn mchart_function(
        &self,
        handle: &MchartFunctionHandle,
    ) -> Option<&MchartFunctionMetadata> {
        self.mchart_functions.get(handle)
    }

    pub fn find_mchart_function(&self, name: &str) -> Option<&MchartFunctionMetadata> {
        let key = name.trim().to_ascii_lowercase();
        let handle = self.mchart_function_lookup.get(&key)?;
        self.mchart_functions.get(handle)
    }

    pub fn content_mode(&self, handle: &ContentModeHandle) -> Option<&ContentModeMetadata> {
        self.content_modes.get(handle)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RegistrySnapshotParts {
    pub(crate) settings: BTreeMap<SettingHandle, SettingMetadata>,
    pub(crate) themes: BTreeMap<ThemeHandle, ThemeMetadata>,
    pub(crate) colors: BTreeMap<ColorHandle, ColorMetadata>,
    pub(crate) symbols: BTreeMap<SymbolHandle, SymbolMetadata>,
    pub(crate) commands: BTreeMap<CommandHandle, CommandMetadata>,
    pub(crate) events: BTreeMap<EventHandle, EventMetadata>,
    pub(crate) mchart_functions: BTreeMap<MchartFunctionHandle, MchartFunctionMetadata>,
    pub(crate) content_modes: BTreeMap<ContentModeHandle, ContentModeMetadata>,
    pub(crate) plugins: BTreeMap<PluginHandle, PluginMetadata>,
    pub(crate) command_lookup: BTreeMap<String, CommandHandle>,
    pub(crate) mchart_function_lookup: BTreeMap<String, MchartFunctionHandle>,
}

pub fn install_registry_snapshot(snapshot: RegistrySnapshot) {
    let mut guard = match REGISTRY_SNAPSHOT.write() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    *guard = snapshot;
}

pub fn current_registry_snapshot() -> RegistrySnapshot {
    let guard = match REGISTRY_SNAPSHOT.read() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    guard.clone()
}

pub fn with_registry_snapshot<R>(f: impl FnOnce(&RegistrySnapshot) -> R) -> R {
    let guard = match REGISTRY_SNAPSHOT.read() {
        Ok(guard) => guard,
        Err(error) => error.into_inner(),
    };
    f(&guard)
}
