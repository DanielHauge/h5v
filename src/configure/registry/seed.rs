use crate::{
    configure,
    ui::{
        command::{builtin_command_handle, command_catalog, CommandArgKind},
        mchart::functions::{mchart_functions, MchartFunctionCategory as BuiltinMchartCategory},
        state::ContentShowMode,
    },
};

use super::{
    builder::{RegistryBuilder, RegistryError},
    ids::{
        ColorHandle, ContentModeHandle, EventHandle, MchartFunctionHandle, SettingHandle,
        SymbolHandle, ThemeHandle,
    },
    metadata::{
        ColorMetadata, CommandArgMetadata, CommandArgValueKind, CommandMetadata, CommandVisibility,
        ContentModeMetadata, EventMetadata, MchartFunctionCategory, MchartFunctionMetadata,
        MchartParamMetadata, RegistryOwner, RegistryValueKind, SettingMetadata, SettingScope,
        SymbolMetadata, ThemeMetadata,
    },
    runtime::RegistrySnapshot,
};

struct BuiltinSettingSeed {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    value_kind: RegistryValueKind,
    default_value: Option<&'static str>,
}

const BUILTIN_SETTINGS: &[BuiltinSettingSeed] = &[
    BuiltinSettingSeed {
        id: "builtin.setting.theme",
        title: "Theme",
        description: "Selected application color theme.",
        value_kind: RegistryValueKind::Theme,
        default_value: Some("dark"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.symbol_theme",
        title: "Symbol theme",
        description: "Selected application symbol theme.",
        value_kind: RegistryValueKind::SymbolTheme,
        default_value: Some("rich"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.compatibility",
        title: "Compatibility mode",
        description: "Compatibility mode toggle for terminals and rendering.",
        value_kind: RegistryValueKind::Boolean,
        default_value: Some("false"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.content_mode_order",
        title: "Content mode order",
        description: "Preferred order for preview, matrix, and heatmap content modes.",
        value_kind: RegistryValueKind::ContentMode,
        default_value: Some("preview,matrix,heatmap"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.layout",
        title: "Layout",
        description: "Automatic panel layout settings.",
        value_kind: RegistryValueKind::Unknown,
        default_value: None,
    },
    BuiltinSettingSeed {
        id: "builtin.setting.heatmap.default_range",
        title: "Heatmap default range",
        description: "Default heatmap range mode.",
        value_kind: RegistryValueKind::String,
        default_value: Some("Auto"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.heatmap.default_colormap",
        title: "Heatmap default colormap",
        description: "Default heatmap colormap name.",
        value_kind: RegistryValueKind::String,
        default_value: Some("turbo"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.heatmap.default_normalization",
        title: "Heatmap default normalization",
        description: "Default heatmap normalization strategy.",
        value_kind: RegistryValueKind::String,
        default_value: Some("linear"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.heatmap.default_invert_x",
        title: "Heatmap invert x",
        description: "Default heatmap x-axis inversion.",
        value_kind: RegistryValueKind::Boolean,
        default_value: Some("false"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.heatmap.default_invert_y",
        title: "Heatmap invert y",
        description: "Default heatmap y-axis inversion.",
        value_kind: RegistryValueKind::Boolean,
        default_value: Some("false"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.heatmap.default_invert_c",
        title: "Heatmap invert color scale",
        description: "Default heatmap color-scale inversion.",
        value_kind: RegistryValueKind::Boolean,
        default_value: Some("false"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.multichart.overview_max_samples",
        title: "Multichart overview max samples",
        description: "Maximum overview samples for multichart.",
        value_kind: RegistryValueKind::UnsignedInt,
        default_value: Some("4096"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.multichart.detail_enabled",
        title: "Multichart detail enabled",
        description: "Whether multichart detail sampling is enabled.",
        value_kind: RegistryValueKind::Boolean,
        default_value: Some("true"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.multichart.detail_samples_per_column",
        title: "Multichart detail samples per column",
        description: "Samples per rendered column in multichart detail mode.",
        value_kind: RegistryValueKind::UnsignedInt,
        default_value: Some("4"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.multichart.detail_min_samples",
        title: "Multichart detail minimum samples",
        description: "Minimum multichart detail samples.",
        value_kind: RegistryValueKind::UnsignedInt,
        default_value: Some("512"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.multichart.detail_max_samples",
        title: "Multichart detail maximum samples",
        description: "Maximum multichart detail samples.",
        value_kind: RegistryValueKind::UnsignedInt,
        default_value: Some("16384"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.multichart.detail_padding_ratio",
        title: "Multichart detail padding ratio",
        description: "Padding ratio for multichart detail windows.",
        value_kind: RegistryValueKind::Float,
        default_value: Some("0.2"),
    },
    BuiltinSettingSeed {
        id: "builtin.setting.multichart.derived_detail_enabled",
        title: "Multichart derived detail enabled",
        description: "Whether detail sampling is enabled for derived multichart series.",
        value_kind: RegistryValueKind::Boolean,
        default_value: Some("true"),
    },
];

pub fn builtin_registry_snapshot() -> Result<RegistrySnapshot, RegistryError> {
    let builder = builtin_registry_builder()?;
    builder.freeze()
}

pub fn builtin_registry_builder() -> Result<RegistryBuilder, RegistryError> {
    let mut builder = RegistryBuilder::new();
    seed_builtin_registry(&mut builder)?;
    Ok(builder)
}

pub fn seed_builtin_registry(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    seed_builtin_commands(builder)?;
    seed_builtin_mchart_functions(builder)?;
    seed_builtin_themes(builder)?;
    seed_builtin_colors(builder)?;
    seed_builtin_symbols(builder)?;
    seed_builtin_settings(builder)?;
    seed_builtin_content_modes(builder)?;
    seed_builtin_events(builder)?;
    Ok(())
}

fn seed_builtin_commands(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    for descriptor in command_catalog() {
        builder.register_command(CommandMetadata {
            handle: builtin_command_handle(descriptor.name),
            name: descriptor.name.to_string(),
            aliases: descriptor
                .aliases
                .iter()
                .map(|alias| (*alias).to_string())
                .collect(),
            summary: descriptor.description.to_string(),
            category: format!("{:?}", descriptor.category),
            keybindings: descriptor
                .keybindings
                .iter()
                .map(|binding| (*binding).to_string())
                .collect(),
            callback_id: None,
            args: descriptor
                .args
                .iter()
                .map(|arg| CommandArgMetadata {
                    name: arg.name.to_string(),
                    kind: match arg.kind {
                        CommandArgKind::UnsignedInt => CommandArgValueKind::UnsignedInt,
                        CommandArgKind::Word => CommandArgValueKind::Word,
                    },
                    required: arg.required,
                    help: arg.help.to_string(),
                    values: arg
                        .values
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                })
                .collect(),
            examples: vec![descriptor.example.to_string()],
            visibility: CommandVisibility::Visible,
            owner: RegistryOwner::Builtin,
        })?;
    }
    Ok(())
}

fn seed_builtin_mchart_functions(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    for function in mchart_functions() {
        builder.register_mchart_function(MchartFunctionMetadata {
            handle: MchartFunctionHandle::new(format!("builtin.mchart_function.{}", function.name)),
            name: function.name.to_string(),
            category: match function.category {
                BuiltinMchartCategory::Reducer => MchartFunctionCategory::Reducer,
                BuiltinMchartCategory::Math => MchartFunctionCategory::Math,
                BuiltinMchartCategory::Transform => MchartFunctionCategory::Transform,
            },
            summary: function.summary.to_string(),
            params: function
                .params
                .iter()
                .map(|param| MchartParamMetadata {
                    name: param.name.to_string(),
                    value_kind: match param.kind_label {
                        "Scalar" => RegistryValueKind::Scalar,
                        "Series" | "X/Y Series" => RegistryValueKind::Series,
                        _ => RegistryValueKind::Unknown,
                    },
                    kind_label: param.kind_label.to_string(),
                    detail: param.detail.to_string(),
                })
                .collect(),
            return_kind: match function.return_label {
                "scalar" => RegistryValueKind::Scalar,
                "series" => RegistryValueKind::Series,
                _ => RegistryValueKind::Unknown,
            },
            example: function.example.to_string(),
            completion_insert: function.completion_insert.to_string(),
            callback_id: None,
            top_level_only: function.top_level_only,
            first_arg_direct_item_ref_only: function.first_arg_direct_item_ref_only,
            owner: RegistryOwner::Builtin,
        })?;
    }
    Ok(())
}

fn seed_builtin_themes(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    for theme_name in configure::available_theme_names() {
        let variant = match *theme_name {
            "dark" => Some("dark".to_string()),
            "light" => Some("light".to_string()),
            _ => None,
        };
        builder.register_theme(ThemeMetadata {
            handle: ThemeHandle::new(format!("builtin.theme.{theme_name}")),
            title: theme_name.to_ascii_uppercase(),
            summary: format!("Built-in {theme_name} theme"),
            variant,
            color_overrides: Vec::new(),
            symbol_overrides: Vec::new(),
            is_active: *theme_name == "dark",
            owner: RegistryOwner::Builtin,
        })?;
    }
    Ok(())
}

fn seed_builtin_colors(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    for name in configure::available_color_names() {
        let (group, short_name) = split_grouped_name(name);
        builder.register_color(ColorMetadata {
            handle: ColorHandle::new(format!("builtin.color.{name}")),
            group,
            name: short_name,
            override_value: None,
            owner: RegistryOwner::Builtin,
        })?;
    }
    Ok(())
}

fn seed_builtin_symbols(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    for name in configure::available_symbol_names() {
        let (group, short_name) = split_grouped_name(name);
        builder.register_symbol(SymbolMetadata {
            handle: SymbolHandle::new(format!("builtin.symbol.{name}")),
            group,
            name: short_name,
            override_value: None,
            owner: RegistryOwner::Builtin,
        })?;
    }
    Ok(())
}

fn seed_builtin_settings(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    for setting in BUILTIN_SETTINGS {
        builder.register_setting(SettingMetadata {
            handle: SettingHandle::new(setting.id),
            title: setting.title.to_string(),
            description: setting.description.to_string(),
            value_kind: setting.value_kind,
            default_value: setting.default_value.map(str::to_string),
            current_value: setting.default_value.map(str::to_string),
            scope: SettingScope::App,
            examples: Vec::new(),
            owner: RegistryOwner::Builtin,
        })?;
    }
    Ok(())
}

fn seed_builtin_content_modes(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    for mode in [
        ContentShowMode::Preview,
        ContentShowMode::Matrix,
        ContentShowMode::Heatmap,
    ] {
        builder.register_content_mode(ContentModeMetadata {
            handle: ContentModeHandle::new(format!("builtin.content_mode.{}", mode.as_str())),
            title: mode.as_str().to_string(),
            summary: format!("Built-in {} content mode", mode.as_str()),
            callback_id: None,
            owner: RegistryOwner::Builtin,
        })?;
    }
    Ok(())
}

fn seed_builtin_events(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    for (name, title, payload_schema) in [
        (
            "file_opened",
            "File opened",
            Some("{ path: string, readonly: boolean }"),
        ),
        (
            "file_reloaded",
            "File reloaded",
            Some("{ path: string, readonly: boolean }"),
        ),
        ("dataset_opened", "Dataset opened", Some("{ path: string }")),
        (
            "multichart_opened",
            "Multichart opened",
            Some("{ selected_path?: string }"),
        ),
        (
            "content_mode_changed",
            "Content mode changed",
            Some("{ mode: string, path?: string }"),
        ),
        (
            "selection_changed",
            "Selection changed",
            Some(
                "{ path?: string, previous_path?: string, kind?: string, previous_kind?: string }",
            ),
        ),
        (
            "focus_changed",
            "Focus changed",
            Some("{ focus: string, previous_focus: string }"),
        ),
        (
            "mode_changed",
            "Mode changed",
            Some("{ mode: string, previous_mode: string }"),
        ),
        (
            "help_opened",
            "Help opened",
            Some("{ return_mode: string }"),
        ),
        ("help_closed", "Help closed", Some("{ mode: string }")),
        (
            "logs_opened",
            "Logs opened",
            Some("{ return_mode: string }"),
        ),
        ("logs_closed", "Logs closed", Some("{ mode: string }")),
        (
            "command_opened",
            "Command opened",
            Some("{ return_mode: string }"),
        ),
        ("command_closed", "Command closed", Some("{ mode: string }")),
        (
            "search_opened",
            "Search opened",
            Some("{ previous_mode: string }"),
        ),
        ("search_closed", "Search closed", Some("{ mode: string }")),
        (
            "multichart_closed",
            "Multichart closed",
            Some("{ selected_path?: string }"),
        ),
        (
            "tree_view_toggled",
            "Tree view toggled",
            Some("{ visible: boolean }"),
        ),
        (
            "app_started",
            "App started",
            Some("{ path: string, readonly: boolean }"),
        ),
        (
            "app_shutting_down",
            "App shutting down",
            Some("{ path: string, readonly: boolean }"),
        ),
    ] {
        builder.register_event(EventMetadata {
            handle: EventHandle::new(format!("builtin.event.{name}")),
            title: title.to_string(),
            payload_schema: payload_schema.map(str::to_string),
        })?;
    }
    Ok(())
}

fn split_grouped_name(name: &str) -> (String, String) {
    match name.rsplit_once('.') {
        Some((group, short_name)) => (group.to_string(), short_name.to_string()),
        None => ("".to_string(), name.to_string()),
    }
}
