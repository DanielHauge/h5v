use mlua::{Function, Lua, Table, Value};

use crate::{
    configure::{
        errors::ConfigureErrors,
        registry::{ContentModeHandle, ContentModeMetadata, RegistryBuilder, RegistryOwner},
    },
    error::AppError,
    h5f::{HasAttributes, HasPath, Node},
    ui::state::AppState,
};

use super::keymaps::with_config_lua_runtime;

const CONTENT_MODES_CALLBACKS_FIELD: &str = "__lua_callbacks";
const CONTENT_MODES_DEFINITIONS_FIELD: &str = "__definitions";
const CONTENT_MODES_ATTACHMENTS_FIELD: &str = "__attachments";
const CONTENT_MODES_NEXT_ID_FIELD: &str = "__next_lua_callback_id";
const REGISTRY_OWNER_FIELD: &str = "__registry_owner";

pub(super) fn build_ui_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let ui = lua.create_table()?;
    ui.set("content_modes", build_content_modes_table(lua)?)?;
    Ok(ui)
}

pub(super) fn register_lua_content_modes(
    builder: &mut RegistryBuilder,
    h5v: &Table,
) -> Result<(), ConfigureErrors> {
    let ui = match h5v.get::<Value>("ui")? {
        Value::Nil => return Ok(()),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.ui must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };
    let content_modes = match ui.get::<Value>("content_modes")? {
        Value::Nil => return Ok(()),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.ui.content_modes must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };
    let definitions = match content_modes.get::<Value>(CONTENT_MODES_DEFINITIONS_FIELD)? {
        Value::Table(table) => table,
        _ => {
            return Err(
                mlua::Error::runtime("h5v.ui.content_modes.__definitions must be a table").into(),
            )
        }
    };
    for pair in definitions.pairs::<String, Table>() {
        let (_id, definition) = pair?;
        if !super::plugins::definition_owner_is_enabled(h5v, &definition)? {
            continue;
        }
        builder
            .register_content_mode(parse_content_mode_metadata(&definition)?)
            .map_err(|error| mlua::Error::runtime(error.to_string()))?;
    }
    Ok(())
}

pub fn with_content_mode_lua_callback<R>(
    callback_id: &str,
    run: impl FnOnce(&Lua, Function) -> Result<R, AppError>,
) -> Result<R, AppError> {
    with_config_lua_runtime(|lua| {
        let h5v: Table = lua
            .globals()
            .get("h5v")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let ui: Table = h5v
            .get("ui")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let content_modes: Table = ui
            .get("content_modes")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let callbacks: Table = content_modes
            .get(CONTENT_MODES_CALLBACKS_FIELD)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let callback: Function = callbacks
            .get(callback_id)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        run(lua, callback)
    })
}

pub(crate) fn available_content_mode_handles(
    state: &AppState<'_>,
) -> Result<Vec<ContentModeHandle>, AppError> {
    with_config_lua_runtime(|lua| {
        let h5v: Table = lua
            .globals()
            .get("h5v")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let content_modes = content_modes_table(&h5v)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let definitions: Table = content_modes
            .get(CONTENT_MODES_DEFINITIONS_FIELD)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let callbacks: Table = content_modes
            .get(CONTENT_MODES_CALLBACKS_FIELD)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let attachments: Table = content_modes
            .get(CONTENT_MODES_ATTACHMENTS_FIELD)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;

        let (selected_path, kind, attribute_names) = state
            .treeview
            .get(state.tree_view_cursor)
            .and_then(|item| {
                item.node.try_borrow().ok().map(|node| {
                    let kind = match &node.node {
                        Node::File(_) => "file",
                        Node::Group(_, _) => "group",
                        Node::Dataset(_, _) => "dataset",
                        Node::Broken(_) => "broken",
                    }
                    .to_string();
                    let attribute_names = node.node.attribute_names().unwrap_or_default();
                    (
                        normalize_content_mode_path(&node.node.path()),
                        kind,
                        attribute_names,
                    )
                })
            })
            .unwrap_or_else(|| ("/".to_string(), "broken".to_string(), Vec::new()));

        let mut handles = Vec::new();
        for pair in definitions.pairs::<String, Table>() {
            let (handle, definition) =
                pair.map_err(|error| AppError::InvalidCommand(error.to_string()))?;
            if !super::plugins::definition_owner_is_enabled(&h5v, &definition)
                .map_err(|error| AppError::InvalidCommand(error.to_string()))?
            {
                continue;
            }
            let predicate_callback_id = optional_string_field(
                &definition,
                "predicate_callback_id",
                "h5v.ui.content_modes.__definitions",
            )
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
            let linked_paths = linked_paths_for_handle(&attachments, handle.as_str())
                .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
            let linked_match = linked_paths.iter().any(|path| path == &selected_path);
            let predicate_match = if let Some(callback_id) = predicate_callback_id.as_deref() {
                let callback: Function = callbacks
                    .get(callback_id)
                    .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
                let item =
                    build_content_mode_item_context(lua, &selected_path, &kind, &attribute_names)
                        .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
                callback
                    .call::<bool>(item)
                    .map_err(|error| AppError::InvalidCommand(error.to_string()))?
            } else {
                false
            };
            let available = if !linked_paths.is_empty() {
                linked_match || predicate_match
            } else if predicate_callback_id.is_some() {
                predicate_match
            } else {
                true
            };
            if available {
                handles.push(ContentModeHandle::new(handle));
            }
        }
        Ok(handles)
    })
}

pub(crate) fn resolve_registered_content_mode_handle(
    h5v: &Table,
    target: &str,
) -> Result<Option<ContentModeHandle>, mlua::Error> {
    let Some(content_modes) = optional_content_modes_table(h5v)? else {
        return Ok(None);
    };
    let definitions = match content_modes.get::<Value>(CONTENT_MODES_DEFINITIONS_FIELD)? {
        Value::Nil => return Ok(None),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.ui.content_modes.__definitions must be a table, got {}",
                other.type_name()
            )))
        }
    };
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    resolve_registered_content_mode_handle_in_definitions(h5v, &definitions, trimmed)
}

fn build_content_modes_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let content_modes = lua.create_table()?;
    content_modes.set(CONTENT_MODES_CALLBACKS_FIELD, lua.create_table()?)?;
    content_modes.set(CONTENT_MODES_DEFINITIONS_FIELD, lua.create_table()?)?;
    content_modes.set(CONTENT_MODES_ATTACHMENTS_FIELD, lua.create_table()?)?;
    content_modes.set(CONTENT_MODES_NEXT_ID_FIELD, 1)?;

    let register_table = content_modes.clone();
    let register_fn = lua.create_function(move |lua, definition: Table| {
        register_content_mode_definition(lua, &register_table, definition)
    })?;
    content_modes.set("register", register_fn)?;
    let add_table = content_modes.clone();
    let add_fn = lua.create_function(move |lua, definition: Table| {
        add_content_mode_link(lua, &add_table, definition)
    })?;
    content_modes.set("add", add_fn)?;
    Ok(content_modes)
}

fn register_content_mode_definition(
    lua: &Lua,
    content_modes: &Table,
    definition: Table,
) -> Result<String, mlua::Error> {
    let id = required_string_field(&definition, "id", "h5v.ui.content_modes.register")?;
    let owner = current_registry_owner(lua)?;
    let handle = content_mode_handle_for_owner(&owner, &id).to_string();
    let title = optional_string_field(&definition, "title", "h5v.ui.content_modes.register")?
        .unwrap_or_else(|| id.clone());
    let summary = optional_string_field(&definition, "summary", "h5v.ui.content_modes.register")?
        .unwrap_or_else(|| title.clone());
    let render: Function = definition.get("render").map_err(|error| {
        mlua::Error::runtime(format!(
            "h5v.ui.content_modes.register.render is required: {error}"
        ))
    })?;
    let predicate = match definition.get::<Value>("predicate")? {
        Value::Nil => None,
        Value::Function(callback) => Some(callback),
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.ui.content_modes.register.predicate must be a function, got {}",
                other.type_name()
            )))
        }
    };

    let definitions: Table = content_modes.get(CONTENT_MODES_DEFINITIONS_FIELD)?;
    if !matches!(definitions.get::<Value>(handle.as_str())?, Value::Nil) {
        return Err(mlua::Error::runtime(format!(
            "Content mode '{}' is already registered",
            id
        )));
    }

    let callback_id = register_content_mode_callback(content_modes, render, "content-mode-render")?;
    let predicate_callback_id = predicate
        .map(|callback| {
            register_content_mode_callback(content_modes, callback, "content-mode-predicate")
        })
        .transpose()?;

    let stored = lua.create_table()?;
    stored.set("id", id.as_str())?;
    stored.set("title", title)?;
    stored.set("summary", summary)?;
    stored.set("callback_id", callback_id)?;
    if let Some(predicate_callback_id) = predicate_callback_id {
        stored.set("predicate_callback_id", predicate_callback_id)?;
    }
    stored.set("owner", owner)?;
    definitions.set(handle.as_str(), stored)?;
    Ok(handle)
}

fn parse_content_mode_metadata(definition: &Table) -> Result<ContentModeMetadata, ConfigureErrors> {
    let id = required_string_field(definition, "id", "h5v.ui.content_modes.__definitions")?;
    let title = optional_string_field(definition, "title", "h5v.ui.content_modes.__definitions")?
        .unwrap_or_else(|| id.clone());
    let summary =
        optional_string_field(definition, "summary", "h5v.ui.content_modes.__definitions")?
            .unwrap_or_else(|| title.clone());
    let callback_id = optional_string_field(
        definition,
        "callback_id",
        "h5v.ui.content_modes.__definitions",
    )?;
    let owner = parse_registry_owner(definition)?;

    Ok(ContentModeMetadata {
        handle: content_mode_handle_for_registry_owner(&owner, &id),
        title,
        summary,
        callback_id,
        owner,
    })
}

fn config_content_mode_handle(id: &str) -> ContentModeHandle {
    ContentModeHandle::new(format!("config.content_mode.{id}"))
}

fn content_mode_handle_for_owner(owner: &str, id: &str) -> ContentModeHandle {
    if owner == "config" {
        return config_content_mode_handle(id);
    }
    if let Some(plugin) = owner.strip_prefix("plugin.") {
        return ContentModeHandle::new(format!("plugin.{plugin}.content_mode.{id}"));
    }
    config_content_mode_handle(id)
}

fn content_mode_handle_for_registry_owner(owner: &RegistryOwner, id: &str) -> ContentModeHandle {
    match owner {
        RegistryOwner::Builtin | RegistryOwner::Config => config_content_mode_handle(id),
        RegistryOwner::Plugin(handle) => {
            ContentModeHandle::new(format!("{}.content_mode.{id}", handle))
        }
    }
}

fn content_modes_table(h5v: &Table) -> Result<Table, mlua::Error> {
    optional_content_modes_table(h5v)?
        .ok_or_else(|| mlua::Error::runtime("h5v.ui.content_modes is not available"))
}

fn optional_content_modes_table(h5v: &Table) -> Result<Option<Table>, mlua::Error> {
    let ui = match h5v.get::<Value>("ui")? {
        Value::Nil => return Ok(None),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.ui must be a table, got {}",
                other.type_name()
            )))
        }
    };
    match ui.get::<Value>("content_modes")? {
        Value::Table(table) => Ok(Some(table)),
        Value::Nil => Ok(None),
        other => Err(mlua::Error::runtime(format!(
            "h5v.ui.content_modes must be a table, got {}",
            other.type_name()
        ))),
    }
}

fn register_content_mode_callback(
    content_modes: &Table,
    callback: Function,
    prefix: &str,
) -> Result<String, mlua::Error> {
    let callbacks: Table = content_modes.get(CONTENT_MODES_CALLBACKS_FIELD)?;
    let next_id = match content_modes.get::<Value>(CONTENT_MODES_NEXT_ID_FIELD)? {
        Value::Integer(value) if value > 0 => value,
        _ => 1,
    };
    let callback_id = format!("{prefix}-{next_id}");
    callbacks.set(callback_id.as_str(), callback)?;
    content_modes.set(CONTENT_MODES_NEXT_ID_FIELD, next_id + 1)?;
    Ok(callback_id)
}

fn add_content_mode_link(
    lua: &Lua,
    content_modes: &Table,
    definition: Table,
) -> Result<(), mlua::Error> {
    let mode = required_string_field(&definition, "mode", "h5v.ui.content_modes.add")?;
    let path = required_string_field(&definition, "path", "h5v.ui.content_modes.add")?;
    let h5v: Table = lua.globals().get("h5v")?;
    let definitions: Table = content_modes.get(CONTENT_MODES_DEFINITIONS_FIELD)?;
    let handle = resolve_registered_content_mode_handle_in_definitions(&h5v, &definitions, &mode)?
        .ok_or_else(|| mlua::Error::runtime(format!("Unknown content mode '{mode}'")))?;
    let attachments: Table = content_modes.get(CONTENT_MODES_ATTACHMENTS_FIELD)?;
    let normalized_path = normalize_content_mode_path(&path);
    let links = match attachments.get::<Value>(handle.as_str())? {
        Value::Nil => {
            let created = lua.create_table()?;
            attachments.set(handle.as_str(), created.clone())?;
            created
        }
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.ui.content_modes.__attachments.{} must be a table, got {}",
                handle.as_str(),
                other.type_name()
            )))
        }
    };
    for value in links.sequence_values::<String>() {
        if normalize_content_mode_path(&value?) == normalized_path {
            return Ok(());
        }
    }
    links.set(links.raw_len() + 1, normalized_path)?;
    Ok(())
}

fn resolve_registered_content_mode_handle_in_definitions(
    h5v: &Table,
    definitions: &Table,
    target: &str,
) -> Result<Option<ContentModeHandle>, mlua::Error> {
    match definitions.get::<Value>(target)? {
        Value::Table(definition) => {
            if super::plugins::definition_owner_is_enabled(h5v, &definition)? {
                return Ok(Some(ContentModeHandle::new(target)));
            }
        }
        Value::Nil => {}
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.ui.content_modes.__definitions.{} must be a table, got {}",
                target,
                other.type_name()
            )))
        }
    }
    for pair in definitions.pairs::<String, Table>() {
        let (handle, definition) = pair?;
        if !super::plugins::definition_owner_is_enabled(h5v, &definition)? {
            continue;
        }
        let id = required_string_field(&definition, "id", "h5v.ui.content_modes.__definitions")?;
        if id == target {
            return Ok(Some(ContentModeHandle::new(handle)));
        }
    }
    Ok(None)
}

fn build_content_mode_item_context(
    lua: &Lua,
    path: &str,
    kind: &str,
    attribute_names: &[String],
) -> Result<Table, mlua::Error> {
    let item = lua.create_table()?;
    item.set("path", path)?;
    item.set("kind", kind)?;
    item.set(
        "attribute_names",
        lua.create_sequence_from(attribute_names.iter().cloned())?,
    )?;
    let names = attribute_names.to_vec();
    item.set(
        "has_attribute",
        lua.create_function(move |_, name: String| Ok(names.iter().any(|entry| entry == &name)))?,
    )?;
    Ok(item)
}

fn linked_paths_for_handle(attachments: &Table, handle: &str) -> Result<Vec<String>, mlua::Error> {
    match attachments.get::<Value>(handle)? {
        Value::Nil => Ok(Vec::new()),
        Value::Table(values) => {
            let mut paths = Vec::new();
            for value in values.sequence_values::<String>() {
                paths.push(normalize_content_mode_path(&value?));
            }
            Ok(paths)
        }
        other => Err(mlua::Error::runtime(format!(
            "h5v.ui.content_modes.__attachments.{handle} must be a table, got {}",
            other.type_name()
        ))),
    }
}

fn normalize_content_mode_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    let trimmed = trimmed.trim_end_matches('/');
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn current_registry_owner(lua: &Lua) -> Result<String, mlua::Error> {
    let h5v: Table = lua.globals().get("h5v")?;
    match h5v.get::<Value>(REGISTRY_OWNER_FIELD)? {
        Value::Nil => Ok("config".to_string()),
        Value::String(value) => Ok(value.to_str()?.trim().to_string()),
        other => Err(mlua::Error::runtime(format!(
            "h5v.{REGISTRY_OWNER_FIELD} must be a string, got {}",
            other.type_name()
        ))),
    }
}

fn parse_registry_owner(definition: &Table) -> Result<RegistryOwner, ConfigureErrors> {
    let owner = optional_string_field(definition, "owner", "h5v.ui.content_modes.__definitions")?
        .unwrap_or_else(|| "config".to_string());
    if owner == "config" {
        return Ok(RegistryOwner::Config);
    }
    if let Some(handle) = owner.strip_prefix("plugin.") {
        return Ok(RegistryOwner::Plugin(format!("plugin.{handle}").into()));
    }
    Err(mlua::Error::runtime(format!(
        "Unsupported registry owner '{owner}' for h5v.ui.content_modes.__definitions.owner"
    ))
    .into())
}

fn required_string_field(table: &Table, field: &str, context: &str) -> Result<String, mlua::Error> {
    optional_string_field(table, field, context)?
        .ok_or_else(|| mlua::Error::runtime(format!("{context}.{field} is required")))
}

fn optional_string_field(
    table: &Table,
    field: &str,
    context: &str,
) -> Result<Option<String>, mlua::Error> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(None),
        Value::String(value) => Ok(Some(value.to_str()?.trim().to_string())),
        other => Err(mlua::Error::runtime(format!(
            "{context}.{field} must be a string, got {}",
            other.type_name()
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        build_content_modes_table, CONTENT_MODES_ATTACHMENTS_FIELD, CONTENT_MODES_DEFINITIONS_FIELD,
    };

    #[test]
    fn register_stores_optional_predicate_callback() {
        let lua = mlua::Lua::new();
        let content_modes = build_content_modes_table(&lua).expect("build content modes table");
        lua.globals()
            .set("h5v", lua.create_table().expect("create h5v"))
            .expect("set h5v");
        lua.globals()
            .set("content_modes", content_modes.clone())
            .expect("set content modes");

        let handle = lua
            .load(
                r#"
                return content_modes.register({
                  id = "analysis.results",
                  predicate = function(item)
                    return item.kind == "dataset"
                  end,
                  render = function(ctx, ui)
                    ui.text("ok")
                  end,
                })
            "#,
            )
            .eval::<String>()
            .expect("register content mode");

        let definitions: mlua::Table = content_modes
            .get(CONTENT_MODES_DEFINITIONS_FIELD)
            .expect("definitions");
        let definition: mlua::Table = definitions.get(handle.as_str()).expect("definition");
        assert_eq!(
            definition.get::<String>("id").expect("id"),
            "analysis.results"
        );
        assert!(definition.get::<String>("callback_id").is_ok());
        assert!(definition.get::<String>("predicate_callback_id").is_ok());
    }

    #[test]
    fn add_links_registered_mode_by_handle_or_id() {
        let lua = mlua::Lua::new();
        let content_modes = build_content_modes_table(&lua).expect("build content modes table");
        lua.globals()
            .set("h5v", lua.create_table().expect("create h5v"))
            .expect("set h5v");
        lua.globals()
            .set("content_modes", content_modes.clone())
            .expect("set content modes");

        let handle = lua
            .load(
                r#"
                return content_modes.register({
                  id = "analysis.results",
                  render = function(ctx, ui)
                    ui.text("ok")
                  end,
                })
            "#,
            )
            .eval::<String>()
            .expect("register content mode");

        lua.load(
            r#"
            content_modes.add({ mode = "analysis.results", path = "group/ds/" })
            content_modes.add({ mode = "config.content_mode.analysis.results", path = "/group/ds" })
        "#,
        )
        .exec()
        .expect("link content mode");

        let attachments: mlua::Table = content_modes
            .get(CONTENT_MODES_ATTACHMENTS_FIELD)
            .expect("attachments");
        let links: mlua::Table = attachments.get(handle.as_str()).expect("linked paths");
        assert_eq!(links.raw_len(), 1);
        assert_eq!(links.get::<String>(1).expect("first link"), "/group/ds");
    }

    #[test]
    fn plugin_owned_registration_returns_plugin_scoped_handle() {
        let lua = mlua::Lua::new();
        let h5v = lua.create_table().expect("create h5v");
        h5v.set("__registry_owner", "plugin.demo.analysis")
            .expect("set registry owner");
        lua.globals().set("h5v", h5v).expect("set h5v");

        let content_modes = build_content_modes_table(&lua).expect("build content modes table");
        lua.globals()
            .set("content_modes", content_modes)
            .expect("set content modes");

        let handle = lua
            .load(
                r#"
                return content_modes.register({
                  id = "analysis.results",
                  render = function(ctx, ui)
                    ui.text("ok")
                  end,
                })
            "#,
            )
            .eval::<String>()
            .expect("register plugin content mode");

        assert_eq!(handle, "plugin.demo.analysis.content_mode.analysis.results");
    }
}
