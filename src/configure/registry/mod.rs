#![allow(dead_code)]

mod builder;
mod ids;
mod metadata;
mod runtime;
mod seed;

#[allow(unused_imports)]
pub use builder::{RegistryBuilder, RegistryError};
#[allow(unused_imports)]
pub use ids::{
    ColorHandle, CommandHandle, ContentModeHandle, EventHandle, MchartFunctionHandle, PluginHandle,
    SettingHandle, SymbolHandle, ThemeHandle,
};
#[allow(unused_imports)]
pub use metadata::{
    ColorMetadata, CommandArgMetadata, CommandArgValueKind, CommandMetadata, CommandVisibility,
    ContentModeMetadata, EventMetadata, MchartFunctionCategory, MchartFunctionMetadata,
    MchartParamMetadata, PluginMetadata, RegistryItemKind, RegistryOwner, RegistryValueKind,
    SettingMetadata, SettingScope, SymbolMetadata, ThemeMetadata,
};
pub use runtime::{
    current_registry_snapshot, install_registry_snapshot, with_registry_snapshot, RegistrySnapshot,
};
pub use seed::{builtin_registry_builder, builtin_registry_snapshot};

#[cfg(test)]
mod tests;
