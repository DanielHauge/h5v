use crate::{
    configure::{
        builtin_registry_snapshot,
        registry::{
            CommandMetadata, CommandVisibility, MchartFunctionCategory, MchartFunctionMetadata,
            MchartParamMetadata, RegistryError, RegistryOwner, RegistryValueKind,
        },
        RegistryBuilder,
    },
    ui::{command::command_catalog, mchart::functions::mchart_functions},
};

#[test]
fn rejects_duplicate_command_ids() {
    let mut builder = RegistryBuilder::new();
    let metadata = CommandMetadata {
        handle: "builtin.command.custom".into(),
        name: "custom".to_string(),
        aliases: Vec::new(),
        summary: "summary".to_string(),
        category: "App".to_string(),
        keybindings: Vec::new(),
        callback_id: None,
        args: Vec::new(),
        examples: vec!["custom".to_string()],
        visibility: CommandVisibility::Visible,
        owner: RegistryOwner::Builtin,
    };
    builder
        .register_command(metadata.clone())
        .expect("first registration should succeed");
    let error = builder
        .register_command(metadata)
        .expect_err("second registration should fail");
    assert!(matches!(error, RegistryError::DuplicateId { .. }));
}

#[test]
fn rejects_duplicate_command_aliases_across_handles() {
    let mut builder = RegistryBuilder::new();
    builder
        .register_command(CommandMetadata {
            handle: "builtin.command.first".into(),
            name: "first".to_string(),
            aliases: vec!["shared".to_string()],
            summary: "summary".to_string(),
            category: "App".to_string(),
            keybindings: Vec::new(),
            callback_id: None,
            args: Vec::new(),
            examples: vec!["first".to_string()],
            visibility: CommandVisibility::Visible,
            owner: RegistryOwner::Builtin,
        })
        .expect("first command should register");
    builder
        .register_command(CommandMetadata {
            handle: "builtin.command.second".into(),
            name: "second".to_string(),
            aliases: vec!["shared".to_string()],
            summary: "summary".to_string(),
            category: "App".to_string(),
            keybindings: Vec::new(),
            callback_id: None,
            args: Vec::new(),
            examples: vec!["second".to_string()],
            visibility: CommandVisibility::Visible,
            owner: RegistryOwner::Builtin,
        })
        .expect("second command should register before freeze");

    let error = builder
        .freeze()
        .expect_err("freeze should reject alias collision");
    assert!(matches!(
        error,
        RegistryError::DuplicateCommandLookup { .. }
    ));
}

#[test]
fn builtin_snapshot_seeds_known_metadata() {
    let snapshot = builtin_registry_snapshot().expect("builtin registry snapshot");

    assert_eq!(snapshot.commands().count(), command_catalog().len());
    assert_eq!(
        snapshot.mchart_functions().count(),
        mchart_functions().len()
    );
    assert!(snapshot.find_command("reload").is_some());
    assert!(snapshot.find_command("refresh").is_some());
    assert!(snapshot
        .themes()
        .any(|theme| theme.handle.as_str() == "builtin.theme.dark"));
    assert!(snapshot
        .content_modes()
        .any(|mode| mode.handle.as_str() == "builtin.content_mode.preview"));
}

#[test]
fn builtin_snapshot_preserves_multichart_function_shapes() {
    let snapshot = builtin_registry_snapshot().expect("builtin registry snapshot");
    let function = snapshot
        .find_mchart_function("mean")
        .expect("mean function should be seeded");
    assert_eq!(function.category, MchartFunctionCategory::Reducer);
    assert_eq!(function.return_kind, RegistryValueKind::Scalar);
    assert!(!function.top_level_only);
    assert_eq!(function.owner, RegistryOwner::Builtin);
}

#[test]
fn rejects_duplicate_multichart_function_names_across_handles() {
    let mut builder = RegistryBuilder::new();
    builder
        .register_mchart_function(MchartFunctionMetadata {
            handle: "builtin.mchart_function.first".into(),
            name: "shared".to_string(),
            category: MchartFunctionCategory::Math,
            summary: "summary".to_string(),
            params: vec![MchartParamMetadata {
                name: "value".to_string(),
                value_kind: RegistryValueKind::Scalar,
                kind_label: "Scalar".to_string(),
                detail: "detail".to_string(),
            }],
            return_kind: RegistryValueKind::Scalar,
            example: "shared(1)".to_string(),
            completion_insert: "shared($1)".to_string(),
            callback_id: None,
            top_level_only: false,
            first_arg_direct_item_ref_only: false,
            owner: RegistryOwner::Builtin,
        })
        .expect("first function should register");
    builder
        .register_mchart_function(MchartFunctionMetadata {
            handle: "builtin.mchart_function.second".into(),
            name: "shared".to_string(),
            category: MchartFunctionCategory::Math,
            summary: "summary".to_string(),
            params: vec![MchartParamMetadata {
                name: "value".to_string(),
                value_kind: RegistryValueKind::Scalar,
                kind_label: "Scalar".to_string(),
                detail: "detail".to_string(),
            }],
            return_kind: RegistryValueKind::Scalar,
            example: "shared(2)".to_string(),
            completion_insert: "shared($1)".to_string(),
            callback_id: None,
            top_level_only: false,
            first_arg_direct_item_ref_only: false,
            owner: RegistryOwner::Builtin,
        })
        .expect("second function should register before freeze");

    let error = builder
        .freeze()
        .expect_err("freeze should reject function name collision");
    assert!(matches!(
        error,
        RegistryError::DuplicateMchartFunctionLookup { .. }
    ));
}
