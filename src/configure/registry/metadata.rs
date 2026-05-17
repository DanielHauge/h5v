use crate::health::HealthStatus;

use super::ids::{
    ColorHandle, CommandHandle, ContentModeHandle, EventHandle, MchartFunctionHandle, PluginHandle,
    SettingHandle, SymbolHandle, ThemeHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryItemKind {
    Setting,
    Theme,
    Color,
    Symbol,
    Command,
    Event,
    MchartFunction,
    ContentMode,
    Plugin,
}

impl std::fmt::Display for RegistryItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Setting => "setting",
            Self::Theme => "theme",
            Self::Color => "color",
            Self::Symbol => "symbol",
            Self::Command => "command",
            Self::Event => "event",
            Self::MchartFunction => "multichart function",
            Self::ContentMode => "content mode",
            Self::Plugin => "plugin",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryOwner {
    Builtin,
    Config,
    Plugin(PluginHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryValueKind {
    Boolean,
    UnsignedInt,
    Float,
    String,
    Color,
    Symbol,
    Theme,
    SymbolTheme,
    ContentMode,
    Scalar,
    Series,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingScope {
    App,
    Component,
    ContentMode,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingMetadata {
    pub handle: SettingHandle,
    pub title: String,
    pub description: String,
    pub value_kind: RegistryValueKind,
    pub default_value: Option<String>,
    pub current_value: Option<String>,
    pub scope: SettingScope,
    pub examples: Vec<String>,
    pub owner: RegistryOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeMetadata {
    pub handle: ThemeHandle,
    pub title: String,
    pub summary: String,
    pub variant: Option<String>,
    pub color_overrides: Vec<(ColorHandle, String)>,
    pub symbol_overrides: Vec<(SymbolHandle, String)>,
    pub is_active: bool,
    pub owner: RegistryOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorMetadata {
    pub handle: ColorHandle,
    pub group: String,
    pub name: String,
    pub override_value: Option<String>,
    pub owner: RegistryOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolMetadata {
    pub handle: SymbolHandle,
    pub group: String,
    pub name: String,
    pub override_value: Option<String>,
    pub owner: RegistryOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandArgValueKind {
    UnsignedInt,
    Word,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandArgMetadata {
    pub name: String,
    pub kind: CommandArgValueKind,
    pub required: bool,
    pub help: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandVisibility {
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MchartFunctionCategory {
    Reducer,
    Math,
    Transform,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMetadata {
    pub handle: CommandHandle,
    pub name: String,
    pub aliases: Vec<String>,
    pub summary: String,
    pub category: String,
    pub keybindings: Vec<String>,
    pub callback_id: Option<String>,
    pub args: Vec<CommandArgMetadata>,
    pub examples: Vec<String>,
    pub visibility: CommandVisibility,
    pub owner: RegistryOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMetadata {
    pub handle: EventHandle,
    pub title: String,
    pub payload_schema: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MchartParamMetadata {
    pub name: String,
    pub value_kind: RegistryValueKind,
    pub kind_label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MchartFunctionMetadata {
    pub handle: MchartFunctionHandle,
    pub name: String,
    pub category: MchartFunctionCategory,
    pub summary: String,
    pub params: Vec<MchartParamMetadata>,
    pub return_kind: RegistryValueKind,
    pub example: String,
    pub completion_insert: String,
    pub callback_id: Option<String>,
    pub top_level_only: bool,
    pub first_arg_direct_item_ref_only: bool,
    pub owner: RegistryOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentModeMetadata {
    pub handle: ContentModeHandle,
    pub title: String,
    pub summary: String,
    pub callback_id: Option<String>,
    pub owner: RegistryOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMetadata {
    pub handle: PluginHandle,
    pub name: String,
    pub version: Option<String>,
    pub source: Option<String>,
    pub requested_ref: Option<String>,
    pub resolved_commit: Option<String>,
    pub auto_pull: bool,
    pub health_status: HealthStatus,
    pub health_message: Option<String>,
    pub health_ui_document: Option<String>,
}
