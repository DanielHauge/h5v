use mlua::{Function, Lua, Table, Value};

use crate::{
    configure::{
        errors::ConfigureErrors,
        registry::{
            CommandArgMetadata, CommandArgValueKind, CommandHandle, CommandMetadata,
            CommandVisibility, RegistryBuilder, RegistryOwner,
        },
    },
    error::AppError,
};

use super::keymaps::with_config_lua_runtime;

const COMMANDS_CALLBACKS_FIELD: &str = "__lua_callbacks";
const COMMANDS_DEFINITIONS_FIELD: &str = "__definitions";
const COMMANDS_NEXT_ID_FIELD: &str = "__next_lua_callback_id";
const REGISTRY_OWNER_FIELD: &str = "__registry_owner";

pub(super) fn build_commands_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let commands = lua.create_table()?;
    commands.set(COMMANDS_CALLBACKS_FIELD, lua.create_table()?)?;
    commands.set(COMMANDS_DEFINITIONS_FIELD, lua.create_table()?)?;
    commands.set(COMMANDS_NEXT_ID_FIELD, 1)?;

    let register_table = commands.clone();
    let register_fn = lua.create_function(move |lua, definition: Table| {
        register_command_definition(lua, &register_table, definition)
    })?;
    commands.set("register", register_fn)?;
    Ok(commands)
}

pub(super) fn register_lua_commands(
    builder: &mut RegistryBuilder,
    h5v: &Table,
) -> Result<(), ConfigureErrors> {
    let commands = match h5v.get::<Value>("commands")? {
        Value::Nil => return Ok(()),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.commands must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };
    let definitions = match commands.get::<Value>(COMMANDS_DEFINITIONS_FIELD)? {
        Value::Table(table) => table,
        _ => return Err(mlua::Error::runtime("h5v.commands.__definitions must be a table").into()),
    };
    for pair in definitions.pairs::<String, Table>() {
        let (_id, definition) = pair?;
        if !super::plugins::definition_owner_is_enabled(h5v, &definition)? {
            continue;
        }
        builder
            .register_command(parse_command_metadata(&definition)?)
            .map_err(|error| mlua::Error::runtime(error.to_string()))?;
    }
    Ok(())
}

pub fn with_command_lua_callback<R>(
    callback_id: &str,
    run: impl FnOnce(&Lua, Function) -> Result<R, AppError>,
) -> Result<R, AppError> {
    with_config_lua_runtime(|lua| {
        let h5v: Table = lua
            .globals()
            .get("h5v")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let commands: Table = h5v
            .get("commands")
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let callbacks: Table = commands
            .get(COMMANDS_CALLBACKS_FIELD)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        let callback: Function = callbacks
            .get(callback_id)
            .map_err(|error| AppError::InvalidCommand(error.to_string()))?;
        run(lua, callback)
    })
}

fn register_command_definition(
    lua: &Lua,
    commands: &Table,
    definition: Table,
) -> Result<String, mlua::Error> {
    let id = required_string_field(&definition, "id", "h5v.commands.register")?;
    let owner = current_registry_owner(lua)?;
    let handle = command_handle_for_owner(&owner, &id).to_string();
    let title = optional_string_field(&definition, "title", "h5v.commands.register")?
        .unwrap_or_else(|| id.clone());
    let summary = optional_string_field(&definition, "summary", "h5v.commands.register")?
        .unwrap_or_else(|| title.clone());
    let category = optional_string_field(&definition, "category", "h5v.commands.register")?
        .unwrap_or_else(|| "App".to_string());
    let aliases = optional_string_list_field(&definition, "aliases", "h5v.commands.register")?;
    let args = optional_args_field(lua, &definition)?;
    let examples = optional_string_list_field(&definition, "examples", "h5v.commands.register")?;
    let visible =
        optional_bool_field(&definition, "visible", "h5v.commands.register")?.unwrap_or(true);
    let run: Function = definition.get("run").map_err(|error| {
        mlua::Error::runtime(format!("h5v.commands.register.run is required: {error}"))
    })?;

    let definitions: Table = commands.get(COMMANDS_DEFINITIONS_FIELD)?;
    if !matches!(definitions.get::<Value>(handle.as_str())?, Value::Nil) {
        return Err(mlua::Error::runtime(format!(
            "Command '{}' is already registered",
            id
        )));
    }

    let callbacks: Table = commands.get(COMMANDS_CALLBACKS_FIELD)?;
    let next_id = match commands.get::<Value>(COMMANDS_NEXT_ID_FIELD)? {
        Value::Integer(value) if value > 0 => value,
        _ => 1,
    };
    let callback_id = format!("command-{next_id}");
    callbacks.set(callback_id.as_str(), run)?;
    commands.set(COMMANDS_NEXT_ID_FIELD, next_id + 1)?;

    let stored = lua.create_table()?;
    stored.set("id", id.as_str())?;
    stored.set("title", title)?;
    stored.set("summary", summary)?;
    stored.set("category", category)?;
    stored.set("visible", visible)?;
    stored.set("callback_id", callback_id)?;
    stored.set("aliases", lua.create_sequence_from(aliases)?)?;
    stored.set("args", args)?;
    stored.set("examples", lua.create_sequence_from(examples)?)?;
    stored.set("owner", owner)?;
    definitions.set(handle.as_str(), stored)?;
    Ok(handle)
}

fn parse_command_metadata(definition: &Table) -> Result<CommandMetadata, ConfigureErrors> {
    let id = required_string_field(definition, "id", "h5v.commands.__definitions")?;
    let title = optional_string_field(definition, "title", "h5v.commands.__definitions")?
        .unwrap_or_else(|| id.clone());
    let summary = optional_string_field(definition, "summary", "h5v.commands.__definitions")?
        .unwrap_or(title.clone());
    let category = optional_string_field(definition, "category", "h5v.commands.__definitions")?
        .unwrap_or_else(|| "App".to_string());
    let aliases = optional_string_list_field(definition, "aliases", "h5v.commands.__definitions")?;
    let examples =
        optional_string_list_field(definition, "examples", "h5v.commands.__definitions")?;
    let callback_id =
        optional_string_field(definition, "callback_id", "h5v.commands.__definitions")?;
    let args = parse_command_args(definition)?;
    let visible =
        optional_bool_field(definition, "visible", "h5v.commands.__definitions")?.unwrap_or(true);
    let owner = parse_registry_owner(definition)?;

    Ok(CommandMetadata {
        handle: command_handle_for_registry_owner(&owner, &id),
        name: id,
        aliases,
        summary,
        category,
        keybindings: Vec::new(),
        callback_id,
        args,
        examples,
        visibility: if visible {
            CommandVisibility::Visible
        } else {
            CommandVisibility::Hidden
        },
        owner,
    })
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
    let owner = optional_string_field(definition, "owner", "h5v.commands.__definitions")?
        .unwrap_or_else(|| "config".to_string());
    if owner == "config" {
        return Ok(RegistryOwner::Config);
    }
    if let Some(handle) = owner.strip_prefix("plugin.") {
        return Ok(RegistryOwner::Plugin(format!("plugin.{handle}").into()));
    }
    Err(mlua::Error::runtime(format!(
        "Unsupported registry owner '{owner}' for h5v.commands.__definitions.owner"
    ))
    .into())
}

fn parse_command_args(definition: &Table) -> Result<Vec<CommandArgMetadata>, ConfigureErrors> {
    match definition.get::<Value>("args")? {
        Value::Nil => Ok(Vec::new()),
        Value::Table(entries) => {
            let mut args = Vec::new();
            for value in entries.sequence_values::<Table>() {
                let entry = value?;
                let name = required_string_field(&entry, "name", "h5v.commands.args")?;
                let kind = match required_string_field(&entry, "kind", "h5v.commands.args")?
                    .trim()
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "uint" | "unsigned-int" | "unsigned_int" => CommandArgValueKind::UnsignedInt,
                    "word" | "string" => CommandArgValueKind::Word,
                    other => {
                        return Err(mlua::Error::runtime(format!(
                            "Unsupported command arg kind '{other}'. Use 'word' or 'uint'"
                        ))
                        .into())
                    }
                };
                let required =
                    optional_bool_field(&entry, "required", "h5v.commands.args")?.unwrap_or(false);
                let help =
                    optional_string_field(&entry, "help", "h5v.commands.args")?.unwrap_or_default();
                let values = optional_string_list_field(&entry, "values", "h5v.commands.args")?;
                args.push(CommandArgMetadata {
                    name,
                    kind,
                    required,
                    help,
                    values,
                });
            }
            Ok(args)
        }
        other => Err(mlua::Error::runtime(format!(
            "h5v.commands.args must be an array of tables, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn optional_args_field(lua: &Lua, definition: &Table) -> Result<Table, mlua::Error> {
    match definition.get::<Value>("args")? {
        Value::Nil => lua.create_table(),
        Value::Table(table) => Ok(table),
        other => Err(mlua::Error::runtime(format!(
            "h5v.commands.register.args must be an array of tables, got {}",
            other.type_name()
        ))),
    }
}

fn required_string_field(table: &Table, field: &str, context: &str) -> Result<String, mlua::Error> {
    match table.get::<Value>(field)? {
        Value::String(value) => {
            let value = value.to_str()?.trim().to_string();
            if value.is_empty() {
                Err(mlua::Error::runtime(format!(
                    "{context}.{field} cannot be empty"
                )))
            } else {
                Ok(value)
            }
        }
        Value::Nil => Err(mlua::Error::runtime(format!(
            "{context}.{field} is required"
        ))),
        other => Err(mlua::Error::runtime(format!(
            "{context}.{field} must be a string, got {}",
            other.type_name()
        ))),
    }
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

fn optional_bool_field(
    table: &Table,
    field: &str,
    context: &str,
) -> Result<Option<bool>, mlua::Error> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(None),
        Value::Boolean(value) => Ok(Some(value)),
        other => Err(mlua::Error::runtime(format!(
            "{context}.{field} must be a boolean, got {}",
            other.type_name()
        ))),
    }
}

fn optional_string_list_field(
    table: &Table,
    field: &str,
    context: &str,
) -> Result<Vec<String>, mlua::Error> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(Vec::new()),
        Value::Table(values) => {
            let mut entries = Vec::new();
            for value in values.sequence_values::<String>() {
                entries.push(value?);
            }
            Ok(entries)
        }
        other => Err(mlua::Error::runtime(format!(
            "{context}.{field} must be an array of strings, got {}",
            other.type_name()
        ))),
    }
}

pub(crate) fn config_command_handle(id: &str) -> CommandHandle {
    CommandHandle::new(format!("config.command.{id}"))
}

pub(crate) fn command_handle_for_owner(owner: &str, id: &str) -> CommandHandle {
    if owner == "config" {
        return config_command_handle(id);
    }
    if let Some(plugin) = owner.strip_prefix("plugin.") {
        return CommandHandle::new(format!("plugin.{plugin}.command.{id}"));
    }
    config_command_handle(id)
}

fn command_handle_for_registry_owner(owner: &RegistryOwner, id: &str) -> CommandHandle {
    match owner {
        RegistryOwner::Builtin | RegistryOwner::Config => config_command_handle(id),
        RegistryOwner::Plugin(handle) => CommandHandle::new(format!("{}.command.{id}", handle)),
    }
}

pub(crate) fn find_registered_command_handle(
    lua: &Lua,
    value: &str,
) -> Result<Option<String>, mlua::Error> {
    let h5v = match lua.globals().get::<Value>("h5v")? {
        Value::Table(table) => table,
        _ => return Ok(None),
    };
    let commands = match h5v.get::<Value>("commands")? {
        Value::Table(table) => table,
        _ => return Ok(None),
    };
    let definitions = match commands.get::<Value>(COMMANDS_DEFINITIONS_FIELD)? {
        Value::Table(table) => table,
        _ => return Ok(None),
    };
    match definitions.get::<Value>(value)? {
        Value::Table(definition) => {
            if super::plugins::definition_owner_is_enabled(&h5v, &definition)? {
                Ok(Some(value.to_string()))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}
