use std::cell::RefCell;

use mlua::{Function, Lua, Table, Value};

use crate::{
    compat,
    configure::{
        self,
        errors::ConfigureErrors,
        registry::{
            MchartFunctionCategory, MchartFunctionHandle, MchartFunctionMetadata,
            MchartParamMetadata, RegistryBuilder, RegistryOwner, RegistryValueKind,
        },
        MultiChartSettings,
    },
    ui::mchart::Point,
};

use super::{
    bootstrap::{execute_config_chunk, prepare_lua_config},
    context::run_process_spec,
};

const MCHART_FUNCTIONS_CALLBACKS_FIELD: &str = "__lua_callbacks";
const MCHART_FUNCTIONS_DEFINITIONS_FIELD: &str = "__definitions";
const MCHART_FUNCTIONS_NEXT_ID_FIELD: &str = "__next_lua_callback_id";
const REGISTRY_OWNER_FIELD: &str = "__registry_owner";

thread_local! {
    static MCHART_WORKER_RUNTIME: RefCell<Option<WorkerLuaRuntime>> = const { RefCell::new(None) };
}

struct WorkerLuaRuntime {
    generation: u64,
    lua: Lua,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LuaMchartArgValue {
    Scalar(f64),
    Series(Vec<Point>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LuaMchartReturnValue {
    Scalar(f64),
    Series(Vec<f64>),
}

#[cfg(test)]
pub(crate) fn reset_mchart_worker_runtime() {
    MCHART_WORKER_RUNTIME.with(|slot| {
        *slot.borrow_mut() = None;
    });
}

pub(super) fn build_multichart_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let multichart = lua.create_table()?;
    let settings = configure::current_multichart_settings();
    multichart.set("overview_max_samples", settings.overview_max_samples)?;
    multichart.set("detail_enabled", settings.detail_enabled)?;
    multichart.set(
        "detail_samples_per_column",
        settings.detail_samples_per_column,
    )?;
    multichart.set("detail_min_samples", settings.detail_min_samples)?;
    multichart.set("detail_max_samples", settings.detail_max_samples)?;
    multichart.set("detail_padding_ratio", settings.detail_padding_ratio)?;
    multichart.set("derived_detail_enabled", settings.derived_detail_enabled)?;
    multichart.set("functions", build_functions_table(lua)?)?;
    Ok(multichart)
}

pub(super) fn register_lua_mchart_functions(
    builder: &mut RegistryBuilder,
    h5v: &Table,
) -> Result<(), ConfigureErrors> {
    let multichart = match h5v.get::<Value>("multichart")? {
        Value::Nil => return Ok(()),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.multichart must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };
    let functions = match multichart.get::<Value>("functions")? {
        Value::Nil => return Ok(()),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.multichart.functions must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };
    let definitions = match functions.get::<Value>(MCHART_FUNCTIONS_DEFINITIONS_FIELD)? {
        Value::Table(table) => table,
        _ => {
            return Err(mlua::Error::runtime(
                "h5v.multichart.functions.__definitions must be a table",
            )
            .into())
        }
    };
    for pair in definitions.pairs::<String, Table>() {
        let (_id, definition) = pair?;
        if !super::plugins::definition_owner_is_enabled(h5v, &definition)? {
            continue;
        }
        builder
            .register_mchart_function(parse_mchart_function_metadata(&definition)?)
            .map_err(|error| mlua::Error::runtime(error.to_string()))?;
    }
    Ok(())
}

pub(crate) fn run_registered_mchart_function(
    callback_id: &str,
    args: &[LuaMchartArgValue],
    return_kind: RegistryValueKind,
) -> Result<LuaMchartReturnValue, String> {
    with_worker_mchart_callback(callback_id, |lua, callback| {
        let lua_args = args
            .iter()
            .map(|arg| build_lua_mchart_arg(lua, arg))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let result = callback
            .call::<Value>(mlua::MultiValue::from_vec(lua_args))
            .map_err(|error| error.to_string())?;
        parse_lua_mchart_return(lua, result, return_kind, args)
    })
}

pub(super) fn parse_multichart_config(
    h5v: &Table,
) -> Result<Option<MultiChartSettings>, ConfigureErrors> {
    let multichart = match h5v.get::<Value>("multichart")? {
        Value::Nil => return Ok(None),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.multichart must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };

    let mut settings = MultiChartSettings::default();
    settings.overview_max_samples = parse_usize_field(
        &multichart,
        "overview_max_samples",
        settings.overview_max_samples,
    )?;
    settings.detail_enabled =
        parse_bool_field(&multichart, "detail_enabled", settings.detail_enabled)?;
    settings.detail_samples_per_column = parse_usize_field(
        &multichart,
        "detail_samples_per_column",
        settings.detail_samples_per_column,
    )?;
    settings.detail_min_samples = parse_usize_field(
        &multichart,
        "detail_min_samples",
        settings.detail_min_samples,
    )?;
    settings.detail_max_samples = parse_usize_field(
        &multichart,
        "detail_max_samples",
        settings.detail_max_samples,
    )?;
    settings.detail_padding_ratio = parse_f64_field(
        &multichart,
        "detail_padding_ratio",
        settings.detail_padding_ratio,
    )?;
    settings.derived_detail_enabled = parse_bool_field(
        &multichart,
        "derived_detail_enabled",
        settings.derived_detail_enabled,
    )?;

    if settings.detail_min_samples > settings.detail_max_samples {
        return Err(mlua::Error::runtime(
            "h5v.multichart.detail_min_samples cannot exceed detail_max_samples",
        )
        .into());
    }
    if !settings.detail_padding_ratio.is_finite() || settings.detail_padding_ratio < 0.0 {
        return Err(mlua::Error::runtime(
            "h5v.multichart.detail_padding_ratio must be a non-negative finite number",
        )
        .into());
    }

    Ok(Some(settings))
}

fn build_functions_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let functions = lua.create_table()?;
    functions.set(MCHART_FUNCTIONS_CALLBACKS_FIELD, lua.create_table()?)?;
    functions.set(MCHART_FUNCTIONS_DEFINITIONS_FIELD, lua.create_table()?)?;
    functions.set(MCHART_FUNCTIONS_NEXT_ID_FIELD, 1)?;

    let register_table = functions.clone();
    let register_fn = lua.create_function(move |lua, definition: Table| {
        register_mchart_function_definition(lua, &register_table, definition)
    })?;
    functions.set("register", register_fn)?;

    let process = lua.create_table()?;
    process.set(
        "run",
        lua.create_function(|lua, spec: Table| run_process_spec(lua, spec, false))?,
    )?;
    process.set(
        "spawn",
        lua.create_function(|lua, spec: Table| run_process_spec(lua, spec, true))?,
    )?;
    process.set(
        "parse_json",
        lua.create_function(|lua, value: Value| {
            crate::configure::parse_lua_process_json(lua, value)
        })?,
    )?;
    process.set(
        "parse_scalar",
        lua.create_function(|_, value: Value| parse_process_scalar_output(value))?,
    )?;
    process.set(
        "parse_series",
        lua.create_function(|lua, value: Value| parse_process_series_output(lua, value))?,
    )?;
    functions.set("process", process)?;
    Ok(functions)
}

fn register_mchart_function_definition(
    lua: &Lua,
    functions: &Table,
    definition: Table,
) -> Result<String, mlua::Error> {
    let id = required_string_field(&definition, "id", "h5v.mchart.functions.register")?;
    let owner = current_registry_owner(lua)?;
    let handle = mchart_function_handle_for_owner(&owner, &id).to_string();
    let name = optional_string_field(&definition, "name", "h5v.mchart.functions.register")?
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| id.clone());
    let summary = optional_string_field(&definition, "summary", "h5v.mchart.functions.register")?
        .unwrap_or_else(|| name.clone());
    let category = optional_string_field(&definition, "category", "h5v.mchart.functions.register")?
        .unwrap_or_else(|| "math".to_string());
    let params = optional_params_field(lua, &definition)?;
    let returns = required_string_field(&definition, "returns", "h5v.mchart.functions.register")?;
    parse_value_kind_id(&returns, "h5v.mchart.functions.register.returns")
        .map_err(|error| mlua::Error::runtime(error.to_string()))?;
    let example = optional_string_field(&definition, "example", "h5v.mchart.functions.register")?;
    let completion_insert = optional_string_field(
        &definition,
        "completion_insert",
        "h5v.mchart.functions.register",
    )?;
    let eval: Function = definition.get("eval").map_err(|error| {
        mlua::Error::runtime(format!(
            "h5v.mchart.functions.register.eval is required: {error}"
        ))
    })?;
    let top_level_only = optional_bool_field(
        &definition,
        "top_level_only",
        "h5v.mchart.functions.register",
    )?
    .unwrap_or(false);
    let first_arg_direct_item_ref_only = optional_bool_field(
        &definition,
        "first_arg_direct_item_ref_only",
        "h5v.mchart.functions.register",
    )?
    .unwrap_or(false);

    let definitions: Table = functions.get(MCHART_FUNCTIONS_DEFINITIONS_FIELD)?;
    if !matches!(definitions.get::<Value>(handle.as_str())?, Value::Nil) {
        return Err(mlua::Error::runtime(format!(
            "Multichart function '{}' is already registered",
            id
        )));
    }

    let callback_id = register_mchart_callback(functions, eval)?;
    let stored = lua.create_table()?;
    stored.set("id", id.clone())?;
    stored.set("name", name.clone())?;
    stored.set("summary", summary)?;
    stored.set("category", category.to_ascii_lowercase())?;
    stored.set("params", params.clone())?;
    stored.set("returns", returns)?;
    stored.set(
        "example",
        example
            .clone()
            .unwrap_or_else(|| default_completion_insert(&name, &params)),
    )?;
    stored.set(
        "completion_insert",
        completion_insert.unwrap_or_else(|| default_completion_insert(&name, &params)),
    )?;
    stored.set("callback_id", callback_id)?;
    stored.set("owner", owner)?;
    stored.set("top_level_only", top_level_only)?;
    stored.set(
        "first_arg_direct_item_ref_only",
        first_arg_direct_item_ref_only,
    )?;
    definitions.set(handle.as_str(), stored)?;
    Ok(handle)
}

fn parse_mchart_function_metadata(
    definition: &Table,
) -> Result<MchartFunctionMetadata, ConfigureErrors> {
    let id = required_string_field(definition, "id", "h5v.mchart.functions.__definitions")?;
    let owner = parse_registry_owner(definition)?;
    let name = required_string_field(definition, "name", "h5v.mchart.functions.__definitions")?;
    let summary =
        optional_string_field(definition, "summary", "h5v.mchart.functions.__definitions")?
            .unwrap_or_else(|| name.clone());
    let category = match optional_string_field(
        definition,
        "category",
        "h5v.mchart.functions.__definitions",
    )?
    .unwrap_or_else(|| "math".to_string())
    .trim()
    .to_ascii_lowercase()
    .as_str()
    {
        "reducer" => MchartFunctionCategory::Reducer,
        "math" => MchartFunctionCategory::Math,
        "transform" => MchartFunctionCategory::Transform,
        other => {
            return Err(mlua::Error::runtime(format!(
                "Unsupported multichart function category '{other}'. Use 'reducer', 'math', or 'transform'"
            ))
            .into())
        }
    };
    let params = parse_mchart_params(definition)?;
    let return_kind = parse_value_kind_id(
        &required_string_field(definition, "returns", "h5v.mchart.functions.__definitions")?,
        "h5v.mchart.functions.__definitions.returns",
    )?;
    let example =
        optional_string_field(definition, "example", "h5v.mchart.functions.__definitions")?
            .unwrap_or_else(|| default_completion_insert_from_metadata(&name, &params));
    let completion_insert = optional_string_field(
        definition,
        "completion_insert",
        "h5v.mchart.functions.__definitions",
    )?
    .unwrap_or_else(|| default_completion_insert_from_metadata(&name, &params));
    let callback_id = optional_string_field(
        definition,
        "callback_id",
        "h5v.mchart.functions.__definitions",
    )?;
    let top_level_only = optional_bool_field(
        definition,
        "top_level_only",
        "h5v.mchart.functions.__definitions",
    )?
    .unwrap_or(false);
    let first_arg_direct_item_ref_only = optional_bool_field(
        definition,
        "first_arg_direct_item_ref_only",
        "h5v.mchart.functions.__definitions",
    )?
    .unwrap_or(false);

    Ok(MchartFunctionMetadata {
        handle: mchart_function_handle_for_registry_owner(&owner, &id),
        name,
        category,
        summary,
        params,
        return_kind,
        example,
        completion_insert,
        callback_id,
        top_level_only,
        first_arg_direct_item_ref_only,
        owner,
    })
}

fn parse_mchart_params(definition: &Table) -> Result<Vec<MchartParamMetadata>, ConfigureErrors> {
    match definition.get::<Value>("params")? {
        Value::Nil => Ok(Vec::new()),
        Value::Table(entries) => {
            let mut params = Vec::new();
            for value in entries.sequence_values::<Table>() {
                let entry = value?;
                let name = required_string_field(&entry, "name", "h5v.mchart.functions.params")?;
                let kind = parse_value_kind_id(
                    &required_string_field(&entry, "kind", "h5v.mchart.functions.params")?,
                    "h5v.mchart.functions.params.kind",
                )?;
                if !matches!(kind, RegistryValueKind::Scalar | RegistryValueKind::Series) {
                    return Err(mlua::Error::runtime(
                        "Custom multichart params must use h5v.ids.value_kinds.scalar or h5v.ids.value_kinds.series",
                    )
                    .into());
                }
                let detail =
                    optional_string_field(&entry, "detail", "h5v.mchart.functions.params")?
                        .unwrap_or_else(|| match kind {
                            RegistryValueKind::Scalar => "The input scalar.".to_string(),
                            RegistryValueKind::Series => "The input series.".to_string(),
                            _ => String::new(),
                        });
                params.push(MchartParamMetadata {
                    name,
                    value_kind: kind,
                    kind_label: value_kind_label(kind).to_string(),
                    detail,
                });
            }
            Ok(params)
        }
        other => Err(mlua::Error::runtime(format!(
            "h5v.mchart.functions.params must be an array of tables, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_value_kind_id(value: &str, context: &str) -> Result<RegistryValueKind, ConfigureErrors> {
    match value.trim() {
        "scalar" => Ok(RegistryValueKind::Scalar),
        "series" => Ok(RegistryValueKind::Series),
        other => Err(mlua::Error::runtime(format!(
            "{context} must be h5v.ids.value_kinds.scalar or h5v.ids.value_kinds.series, got '{other}'"
        ))
        .into()),
    }
}

fn build_lua_mchart_arg(lua: &Lua, arg: &LuaMchartArgValue) -> Result<Value, mlua::Error> {
    Ok(match arg {
        LuaMchartArgValue::Scalar(value) => Value::Number(*value),
        LuaMchartArgValue::Series(points) => {
            Value::Table(build_series_argument_table(lua, points)?)
        }
    })
}

fn build_series_argument_table(lua: &Lua, points: &[Point]) -> Result<Table, mlua::Error> {
    let series = lua.create_table()?;
    series.set("len", points.len())?;
    series.set("to_array", {
        let values = points.iter().map(|(_, y)| *y).collect::<Vec<_>>();
        lua.create_function(move |lua, ()| lua.create_sequence_from(values.iter().copied()))?
    })?;
    series.set("points", {
        let points = points.to_vec();
        lua.create_function(move |lua, ()| {
            let rows = points
                .iter()
                .map(|(x, y)| {
                    let row = lua.create_table()?;
                    row.set("x", *x)?;
                    row.set("y", *y)?;
                    Ok(row)
                })
                .collect::<Result<Vec<_>, mlua::Error>>()?;
            lua.create_sequence_from(rows)
        })?
    })?;
    series.set("iter", {
        let values = points.iter().map(|(_, y)| *y).collect::<Vec<_>>();
        lua.create_function(move |lua, ()| {
            let mut index = 0usize;
            let values = values.clone();
            lua.create_function_mut(move |_, ()| {
                if let Some(value) = values.get(index).copied() {
                    index += 1;
                    Ok(Value::Number(value))
                } else {
                    Ok(Value::Nil)
                }
            })
        })?
    })?;
    series.set("to_lines", {
        let values = points.iter().map(|(_, y)| *y).collect::<Vec<_>>();
        lua.create_function(move |_, ()| {
            Ok(values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"))
        })?
    })?;
    Ok(series)
}

fn parse_lua_mchart_return(
    lua: &Lua,
    value: Value,
    return_kind: RegistryValueKind,
    args: &[LuaMchartArgValue],
) -> Result<LuaMchartReturnValue, String> {
    match return_kind {
        RegistryValueKind::Scalar => match value {
            Value::Integer(number) => {
                let number = number as f64;
                if number.is_finite() {
                    Ok(LuaMchartReturnValue::Scalar(number))
                } else {
                    Err("Custom multichart function returned a non-finite scalar".to_string())
                }
            }
            Value::Number(number) if number.is_finite() => Ok(LuaMchartReturnValue::Scalar(number)),
            Value::Number(_) => {
                Err("Custom multichart function returned a non-finite scalar".to_string())
            }
            other => Err(format!(
                "Custom multichart function must return a number, got {}",
                other.type_name()
            )),
        },
        RegistryValueKind::Series => {
            let expected_len = first_series_arg(args)
                .map(|points| points.len())
                .ok_or_else(|| {
                    "Series-returning custom multichart functions require at least one series argument"
                        .to_string()
                })?;
            let values = match value {
                Value::Table(values) => parse_series_return_values(lua, &values)?,
                other => {
                    return Err(format!(
                        "Custom multichart function must return an array of numbers for a series result, got {}",
                        other.type_name()
                    ))
                }
            };
            if values.len() != expected_len {
                return Err(format!(
                    "Custom multichart function returned {} samples, expected {}",
                    values.len(),
                    expected_len
                ));
            }
            Ok(LuaMchartReturnValue::Series(values))
        }
        other => Err(format!(
            "Unsupported custom multichart return kind '{}'",
            value_kind_label(other)
        )),
    }
}

fn parse_series_return_values(_lua: &Lua, values: &Table) -> Result<Vec<f64>, String> {
    let mut result = Vec::new();
    for value in values.sequence_values::<Value>() {
        match value.map_err(|error| error.to_string())? {
            Value::Integer(number) => result.push(number as f64),
            Value::Number(number) if number.is_finite() => result.push(number),
            Value::Number(_) => {
                return Err(
                    "Custom multichart function returned a non-finite series value".to_string(),
                )
            }
            other => {
                return Err(format!(
                    "Custom multichart function series results must contain only numbers, got {}",
                    other.type_name()
                ))
            }
        }
    }
    Ok(result)
}

fn first_series_arg(args: &[LuaMchartArgValue]) -> Option<&[Point]> {
    args.iter().find_map(|arg| match arg {
        LuaMchartArgValue::Scalar(_) => None,
        LuaMchartArgValue::Series(points) => Some(points.as_slice()),
    })
}

fn parse_process_scalar_output(value: Value) -> Result<f64, mlua::Error> {
    let stdout = parse_process_stdout_text(value, "h5v.mchart.functions.process.parse_scalar")?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(mlua::Error::runtime(
            "h5v.mchart.functions.process.parse_scalar expected stdout to contain a number",
        ));
    }
    let parsed = trimmed.parse::<f64>().map_err(|error| {
        mlua::Error::runtime(format!(
            "h5v.mchart.functions.process.parse_scalar could not parse '{trimmed}' as a number: {error}"
        ))
    })?;
    if !parsed.is_finite() {
        return Err(mlua::Error::runtime(
            "h5v.mchart.functions.process.parse_scalar requires a finite number",
        ));
    }
    Ok(parsed)
}

fn parse_process_series_output(lua: &Lua, value: Value) -> Result<Table, mlua::Error> {
    let stdout = parse_process_stdout_text(value, "h5v.mchart.functions.process.parse_series")?;
    let mut values = Vec::new();
    for token in stdout
        .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
        .filter(|token| !token.trim().is_empty())
    {
        let parsed = token.trim().parse::<f64>().map_err(|error| {
            mlua::Error::runtime(format!(
                "h5v.mchart.functions.process.parse_series could not parse '{token}' as a number: {error}"
            ))
        })?;
        if !parsed.is_finite() {
            return Err(mlua::Error::runtime(
                "h5v.mchart.functions.process.parse_series requires finite numeric stdout values",
            ));
        }
        values.push(parsed);
    }
    lua.create_sequence_from(values)
}

fn parse_process_stdout_text(value: Value, context: &str) -> Result<String, mlua::Error> {
    match value {
        Value::String(value) => Ok(value.to_str()?.to_string()),
        Value::Table(table) => {
            match table.get::<Value>("success")? {
                Value::Boolean(true) | Value::Nil => {}
                Value::Boolean(false) => {
                    let status = match table.get::<Value>("status")? {
                        Value::Integer(value) => value.to_string(),
                        Value::Nil => "unknown".to_string(),
                        other => other.type_name().to_string(),
                    };
                    let stderr = match table.get::<Value>("stderr")? {
                        Value::String(value) => value.to_str()?.trim().to_string(),
                        Value::Nil => String::new(),
                        other => {
                            return Err(mlua::Error::runtime(format!(
                                "{context} expected stderr to be a string, got {}",
                                other.type_name()
                            )))
                        }
                    };
                    let message = if stderr.is_empty() {
                        format!("{context} requires a successful process result, got exit status {status}")
                    } else {
                        format!("{context} requires a successful process result, got exit status {status}: {stderr}")
                    };
                    return Err(mlua::Error::runtime(message));
                }
                other => {
                    return Err(mlua::Error::runtime(format!(
                        "{context} expected result.success to be a boolean, got {}",
                        other.type_name()
                    )))
                }
            }
            match table.get::<Value>("stdout")? {
                Value::String(stdout) => Ok(stdout.to_str()?.to_string()),
                Value::Nil => Err(mlua::Error::runtime(format!(
                    "{context} expected a process result with stdout"
                ))),
                other => Err(mlua::Error::runtime(format!(
                    "{context} expected stdout to be a string, got {}",
                    other.type_name()
                ))),
            }
        }
        other => Err(mlua::Error::runtime(format!(
            "{context} expects either a process result table or a stdout string, got {}",
            other.type_name()
        ))),
    }
}

fn with_worker_mchart_callback<R>(
    callback_id: &str,
    run: impl FnOnce(&Lua, Function) -> Result<R, String>,
) -> Result<R, String> {
    with_worker_mchart_runtime(|lua| {
        let h5v: Table = lua
            .globals()
            .get("h5v")
            .map_err(|error| error.to_string())?;
        let multichart: Table = h5v.get("multichart").map_err(|error| error.to_string())?;
        let functions: Table = multichart
            .get("functions")
            .map_err(|error| error.to_string())?;
        let callbacks: Table = functions
            .get(MCHART_FUNCTIONS_CALLBACKS_FIELD)
            .map_err(|error| error.to_string())?;
        let callback: Function = callbacks
            .get(callback_id)
            .map_err(|error| error.to_string())?;
        run(lua, callback)
    })
}

fn with_worker_mchart_runtime<R>(run: impl FnOnce(&Lua) -> Result<R, String>) -> Result<R, String> {
    MCHART_WORKER_RUNTIME.with(|slot| {
        let generation = configure::current_config_generation();
        let needs_rebuild = slot
            .borrow()
            .as_ref()
            .is_none_or(|runtime| runtime.generation != generation);
        if needs_rebuild {
            let lua = build_worker_mchart_lua_runtime()?;
            *slot.borrow_mut() = Some(WorkerLuaRuntime { generation, lua });
        }
        let runtime = slot.borrow();
        let runtime = runtime
            .as_ref()
            .ok_or_else(|| "Lua multichart worker runtime is not available".to_string())?;
        run(&runtime.lua)
    })
}

fn build_worker_mchart_lua_runtime() -> Result<Lua, String> {
    let prepared = prepare_lua_config(None, compat::current().compatibility_mode)
        .map_err(|error| error.to_string())?;
    execute_config_chunk(&prepared.lua, &prepared.chunk_name, &prepared.config)
        .map_err(|error| error.to_string())?;
    Ok(prepared.lua)
}

fn register_mchart_callback(functions: &Table, callback: Function) -> Result<String, mlua::Error> {
    let callbacks: Table = functions.get(MCHART_FUNCTIONS_CALLBACKS_FIELD)?;
    let next_id = match functions.get::<Value>(MCHART_FUNCTIONS_NEXT_ID_FIELD)? {
        Value::Integer(value) if value > 0 => value,
        _ => 1,
    };
    let callback_id = format!("mchart-function-{next_id}");
    callbacks.set(callback_id.as_str(), callback)?;
    functions.set(MCHART_FUNCTIONS_NEXT_ID_FIELD, next_id + 1)?;
    Ok(callback_id)
}

fn default_completion_insert(name: &str, params: &Table) -> String {
    let placeholders = params
        .sequence_values::<Table>()
        .enumerate()
        .filter_map(|(index, value)| {
            let entry = value.ok()?;
            let kind = entry.get::<String>("kind").ok()?;
            Some(default_arg_placeholder(index, kind.trim()))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}({placeholders})")
}

fn default_completion_insert_from_metadata(name: &str, params: &[MchartParamMetadata]) -> String {
    let placeholders = params
        .iter()
        .enumerate()
        .map(|(index, param)| default_arg_placeholder(index, param.kind_label.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}({placeholders})")
}

fn default_arg_placeholder(index: usize, kind: &str) -> String {
    if matches!(kind, "series" | "Series") {
        format!("${}", index + 1)
    } else {
        "0".to_string()
    }
}

fn value_kind_label(kind: RegistryValueKind) -> &'static str {
    match kind {
        RegistryValueKind::Scalar => "Scalar",
        RegistryValueKind::Series => "Series",
        RegistryValueKind::Unknown => "Unknown",
        RegistryValueKind::Boolean => "Boolean",
        RegistryValueKind::UnsignedInt => "UnsignedInt",
        RegistryValueKind::Float => "Float",
        RegistryValueKind::String => "String",
        RegistryValueKind::Color => "Color",
        RegistryValueKind::Symbol => "Symbol",
        RegistryValueKind::Theme => "Theme",
        RegistryValueKind::SymbolTheme => "SymbolTheme",
        RegistryValueKind::ContentMode => "ContentMode",
    }
}

fn optional_params_field(lua: &Lua, definition: &Table) -> Result<Table, mlua::Error> {
    match definition.get::<Value>("params")? {
        Value::Nil => lua.create_table(),
        Value::Table(table) => Ok(table),
        other => Err(mlua::Error::runtime(format!(
            "h5v.mchart.functions.register.params must be an array of tables, got {}",
            other.type_name()
        ))),
    }
}

fn parse_usize_field(table: &Table, field: &str, default: usize) -> Result<usize, ConfigureErrors> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(default),
        Value::Integer(value) if value > 0 => Ok(value as usize),
        Value::Number(value) if value.is_finite() && value.fract() == 0.0 && value > 0.0 => {
            Ok(value as usize)
        }
        Value::Integer(_) | Value::Number(_) => Err(mlua::Error::runtime(format!(
            "h5v.multichart.{field} must be a positive integer"
        ))
        .into()),
        other => Err(mlua::Error::runtime(format!(
            "h5v.multichart.{field} must be a positive integer, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_bool_field(table: &Table, field: &str, default: bool) -> Result<bool, ConfigureErrors> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(default),
        Value::Boolean(value) => Ok(value),
        other => Err(mlua::Error::runtime(format!(
            "h5v.multichart.{field} must be a boolean, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn parse_f64_field(table: &Table, field: &str, default: f64) -> Result<f64, ConfigureErrors> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(default),
        Value::Integer(value) => Ok(value as f64),
        Value::Number(value) => Ok(value),
        other => Err(mlua::Error::runtime(format!(
            "h5v.multichart.{field} must be a number, got {}",
            other.type_name()
        ))
        .into()),
    }
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
        Value::String(value) => {
            let value = value.to_str()?.trim().to_string();
            if value.is_empty() {
                Err(mlua::Error::runtime(format!(
                    "{context}.{field} cannot be empty"
                )))
            } else {
                Ok(Some(value))
            }
        }
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
    let owner = optional_string_field(definition, "owner", "h5v.mchart.functions.__definitions")?
        .unwrap_or_else(|| "config".to_string());
    if owner == "config" {
        return Ok(RegistryOwner::Config);
    }
    if let Some(handle) = owner.strip_prefix("plugin.") {
        return Ok(RegistryOwner::Plugin(format!("plugin.{handle}").into()));
    }
    Err(mlua::Error::runtime(format!(
        "Unsupported registry owner '{owner}' for h5v.mchart.functions.__definitions.owner"
    ))
    .into())
}

fn config_mchart_function_handle(id: &str) -> MchartFunctionHandle {
    MchartFunctionHandle::new(format!("config.mchart_function.{id}"))
}

fn mchart_function_handle_for_owner(owner: &str, id: &str) -> MchartFunctionHandle {
    if owner == "config" {
        return config_mchart_function_handle(id);
    }
    if let Some(plugin) = owner.strip_prefix("plugin.") {
        return MchartFunctionHandle::new(format!("plugin.{plugin}.mchart_function.{id}"));
    }
    config_mchart_function_handle(id)
}

fn mchart_function_handle_for_registry_owner(
    owner: &RegistryOwner,
    id: &str,
) -> MchartFunctionHandle {
    match owner {
        RegistryOwner::Builtin | RegistryOwner::Config => config_mchart_function_handle(id),
        RegistryOwner::Plugin(handle) => {
            MchartFunctionHandle::new(format!("{}.mchart_function.{id}", handle))
        }
    }
}
