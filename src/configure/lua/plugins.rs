use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

use mlua::{Lua, Table, Value};

use crate::configure::{
    errors::ConfigureErrors,
    registry::{PluginHandle, PluginMetadata, RegistryBuilder},
};
use crate::health::{HealthStatus, HealthcheckResult};

const PLUGINS_DEFINITIONS_FIELD: &str = "__definitions";
const REGISTRY_OWNER_FIELD: &str = "__registry_owner";

pub(super) fn build_plugins_table(lua: &Lua) -> Result<Table, ConfigureErrors> {
    let plugins = lua.create_table()?;
    plugins.set(PLUGINS_DEFINITIONS_FIELD, lua.create_table()?)?;

    let use_table = plugins.clone();
    let use_fn = lua.create_function(move |lua, (source, options): (String, Option<Table>)| {
        use_plugin(lua, &use_table, &source, options)
    })?;
    plugins.set("use", use_fn)?;
    Ok(plugins)
}

pub(super) fn register_lua_plugins(
    builder: &mut RegistryBuilder,
    h5v: &Table,
) -> Result<(), ConfigureErrors> {
    let plugins = match h5v.get::<Value>("plugins")? {
        Value::Nil => return Ok(()),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.plugins must be a table, got {}",
                other.type_name()
            ))
            .into())
        }
    };
    let definitions = match plugins.get::<Value>(PLUGINS_DEFINITIONS_FIELD)? {
        Value::Table(table) => table,
        _ => return Err(mlua::Error::runtime("h5v.plugins.__definitions must be a table").into()),
    };
    for pair in definitions.pairs::<String, Table>() {
        let (_handle, definition) = pair?;
        builder
            .register_plugin(parse_plugin_metadata(&definition)?)
            .map_err(|error| mlua::Error::runtime(error.to_string()))?;
    }
    Ok(())
}

fn use_plugin(
    lua: &Lua,
    plugins: &Table,
    source: &str,
    options: Option<Table>,
) -> Result<String, mlua::Error> {
    let auto_pull = parse_auto_pull(options.as_ref())?;
    let source = parse_plugin_source(source).map_err(|error| {
        report_h5v_plugin_issue(
            format!("plugin {}", source),
            format!("Failed to parse plugin source '{}': {error}", source),
        );
        mlua::Error::runtime(error)
    })?;
    let installed = install_plugin(&source, auto_pull).map_err(|error| {
        report_h5v_plugin_issue(
            format!("plugin {}", source.original),
            format!("Failed to load plugin '{}': {error}", source.original),
        );
        mlua::Error::runtime(error)
    })?;
    let handle = plugin_handle(&installed.manifest.id).to_string();

    let definitions: Table = plugins.get(PLUGINS_DEFINITIONS_FIELD)?;
    if matches!(definitions.get::<Value>(handle.as_str())?, Value::Nil) {
        let definition = lua.create_table()?;
        definition.set("id", installed.manifest.id.as_str())?;
        definition.set("name", installed.manifest.name.as_str())?;
        definition.set("version", installed.manifest.version.as_str())?;
        definition.set("source", installed.source.original.as_str())?;
        if let Some(requested_ref) = &installed.source.requested_ref {
            definition.set("requested_ref", requested_ref.as_str())?;
        }
        definition.set("resolved_commit", installed.resolved_commit.as_str())?;
        definition.set("auto_pull", installed.auto_pull)?;
        definitions.set(handle.as_str(), definition)?;

        let definition: Table = definitions.get(handle.as_str())?;
        set_definition_health(&definition, HealthcheckResult::healthy(String::new()))?;

        match load_plugin_module(lua, &installed) {
            Ok(module) => match run_plugin_healthcheck(lua, &installed, &module) {
                Ok(health) => {
                    let should_init = health.status != HealthStatus::Fail;
                    set_definition_health(&definition, health)?;
                    if should_init {
                        if let Err(error) = run_plugin_init(lua, &installed, &module) {
                            set_definition_failure(
                                &definition,
                                format!("Plugin init failed: {error}"),
                            )?;
                        }
                    }
                }
                Err(error) => {
                    set_definition_failure(
                        &definition,
                        format!("Plugin health check failed: {error}"),
                    )?;
                }
            },
            Err(error) => {
                set_definition_failure(&definition, format!("Plugin load failed: {error}"))?;
            }
        }
    }

    Ok(handle)
}

fn report_h5v_plugin_issue(source: String, message: String) {
    crate::health::push_reported_health_issue(crate::health::ReportedHealthIssue::new(
        source,
        crate::health::HealthcheckResult::fail(message),
    ));
}

fn set_definition_health(definition: &Table, health: HealthcheckResult) -> Result<(), mlua::Error> {
    definition.set("health_status", health.status.as_str())?;
    if health.message.trim().is_empty() {
        definition.set("health_message", Value::Nil)?;
    } else {
        definition.set("health_message", health.message)?;
    }
    if let Some(document) = health.ui_document {
        definition.set("health_ui_document", document)?;
    } else {
        definition.set("health_ui_document", Value::Nil)?;
    }
    Ok(())
}

fn set_definition_failure(definition: &Table, message: String) -> Result<(), mlua::Error> {
    set_definition_health(definition, HealthcheckResult::fail(message))
}

struct LoadedPluginModule {
    healthcheck: mlua::Function,
    init: mlua::Function,
}

fn load_plugin_module(
    lua: &Lua,
    installed: &InstalledPlugin,
) -> Result<LoadedPluginModule, mlua::Error> {
    let entry_path = resolve_plugin_entry_path(installed).map_err(mlua::Error::runtime)?;
    let globals = lua.globals();
    let previous_h5v = globals.get::<Value>("h5v")?;
    globals.raw_remove("h5v")?;
    let source = fs::read_to_string(&entry_path).map_err(mlua::Error::external)?;
    let chunk_name = format!("@{}", entry_path.display());
    let result = lua.load(source).set_name(&chunk_name).eval::<Value>();
    match previous_h5v {
        Value::Nil => globals.raw_remove("h5v")?,
        other => globals.set("h5v", other)?,
    }
    let module = match result? {
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "Plugin entry '{}' must return a table, got {}",
                entry_path.display(),
                other.type_name()
            )))
        }
    };
    let healthcheck = module.get::<mlua::Function>("health").map_err(|error| {
        mlua::Error::runtime(format!(
            "Plugin '{}' must return a 'health' function: {error}",
            installed.manifest.id
        ))
    })?;
    let init = module.get::<mlua::Function>("init").map_err(|error| {
        mlua::Error::runtime(format!(
            "Plugin '{}' must return an 'init' function: {error}",
            installed.manifest.id
        ))
    })?;
    Ok(LoadedPluginModule { healthcheck, init })
}

fn run_plugin_init(
    lua: &Lua,
    installed: &InstalledPlugin,
    module: &LoadedPluginModule,
) -> Result<(), mlua::Error> {
    let h5v: Table = lua.globals().get("h5v")?;
    let previous_owner = h5v.get::<Value>(REGISTRY_OWNER_FIELD)?;
    h5v.set(
        REGISTRY_OWNER_FIELD,
        plugin_handle(&installed.manifest.id).as_str(),
    )?;
    let ctx = build_plugin_init_context(lua, installed)?;
    let result = module.init.call::<()>((h5v.clone(), ctx));
    match previous_owner {
        Value::Nil => h5v.raw_remove(REGISTRY_OWNER_FIELD)?,
        other => h5v.set(REGISTRY_OWNER_FIELD, other)?,
    }
    result
}

fn parse_plugin_metadata(definition: &Table) -> Result<PluginMetadata, ConfigureErrors> {
    let id = required_string_field(definition, "id", "h5v.plugins.__definitions")?;
    let name = required_string_field(definition, "name", "h5v.plugins.__definitions")?;
    let version = optional_string_field(definition, "version", "h5v.plugins.__definitions")?;
    let source = optional_string_field(definition, "source", "h5v.plugins.__definitions")?;
    let requested_ref =
        optional_string_field(definition, "requested_ref", "h5v.plugins.__definitions")?;
    let resolved_commit =
        optional_string_field(definition, "resolved_commit", "h5v.plugins.__definitions")?;
    let auto_pull = match definition.get::<Value>("auto_pull")? {
        Value::Nil => true,
        Value::Boolean(value) => value,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.plugins.__definitions.auto_pull must be a boolean, got {}",
                other.type_name()
            ))
            .into())
        }
    };
    let health_status =
        optional_string_field(definition, "health_status", "h5v.plugins.__definitions")?
            .as_deref()
            .and_then(HealthStatus::parse)
            .unwrap_or(HealthStatus::Healthy);
    let health_message =
        optional_string_field(definition, "health_message", "h5v.plugins.__definitions")?;
    let health_ui_document = optional_string_field(
        definition,
        "health_ui_document",
        "h5v.plugins.__definitions",
    )?;
    Ok(PluginMetadata {
        handle: plugin_handle(&id),
        name,
        version,
        source,
        requested_ref,
        resolved_commit,
        auto_pull,
        health_status,
        health_message,
        health_ui_document,
    })
}

fn run_plugin_healthcheck(
    lua: &Lua,
    installed: &InstalledPlugin,
    module: &LoadedPluginModule,
) -> Result<HealthcheckResult, mlua::Error> {
    let ctx = build_plugin_healthcheck_context(lua, installed)?;
    let result = module.healthcheck.call::<Value>(ctx)?;
    parse_plugin_healthcheck_result(result)
}

fn build_plugin_healthcheck_context(
    lua: &Lua,
    installed: &InstalledPlugin,
) -> Result<Table, mlua::Error> {
    let ctx = lua.create_table()?;
    ctx.set("process", crate::configure::build_lua_process_context(lua)?)?;
    let health = lua.create_table()?;
    health.set("healthy", HealthStatus::Healthy.as_str())?;
    health.set("warning", HealthStatus::Warning.as_str())?;
    health.set("fail", HealthStatus::Fail.as_str())?;
    ctx.set("health", health)?;
    ctx.set(
        "ui",
        crate::ui::custom_content::build_ui_document_builder(lua)?,
    )?;
    ctx.set(
        "log",
        crate::configure::build_lua_log_context_with_handle(
            lua,
            plugin_handle(&installed.manifest.id).as_str(),
        )?,
    )?;
    ctx.set("config", crate::configure::build_lua_config_context(lua)?)?;
    ctx.set("fs", crate::configure::build_lua_plugin_fs_context(lua)?)?;
    ctx.set("plugin", crate::configure::build_lua_plugin_context(lua)?)?;
    Ok(ctx)
}

fn build_plugin_init_context(lua: &Lua, installed: &InstalledPlugin) -> Result<Table, mlua::Error> {
    let h5v: Table = lua.globals().get("h5v")?;
    let toast: Table = h5v.get("toast")?;

    let ctx = lua.create_table()?;
    ctx.set(
        "log",
        crate::configure::build_lua_log_context_with_handle(
            lua,
            plugin_handle(&installed.manifest.id).as_str(),
        )?,
    )?;
    ctx.set("toast", toast)?;
    ctx.set("config", crate::configure::build_lua_config_context(lua)?)?;
    ctx.set("fs", crate::configure::build_lua_plugin_fs_context(lua)?)?;
    ctx.set("plugin", crate::configure::build_lua_plugin_context(lua)?)?;
    Ok(ctx)
}

fn parse_plugin_healthcheck_result(value: Value) -> Result<HealthcheckResult, mlua::Error> {
    match value {
        Value::Table(table) => {
            let status = optional_string_field_lua(&table, "status", "plugin healthcheck result")?
                .as_deref()
                .and_then(HealthStatus::parse)
                .ok_or_else(|| {
                    mlua::Error::runtime(
                        "plugin healthcheck result.status must be one of healthy, warning, or fail",
                    )
                })?;
            let summary =
                optional_string_field_lua(&table, "summary", "plugin healthcheck result")?;
            let message = match table.get::<Value>("message")? {
                Value::Nil => summary.unwrap_or_default(),
                Value::String(value) => value.to_str()?.to_string(),
                value => {
                    if crate::ui::custom_content::parse_ui_document(
                        &value,
                        "plugin healthcheck result.message",
                    )?
                    .is_some()
                    {
                        summary.unwrap_or_default()
                    } else {
                        return Err(mlua::Error::runtime(format!(
                            "plugin healthcheck result.message must be a string or ctx.ui.build(...) document, got {}",
                            value.type_name()
                        )));
                    }
                }
            };
            let ui_document = crate::ui::custom_content::parse_ui_document(
                &table.get::<Value>("message")?,
                "plugin healthcheck result.message",
            )?
            .or(crate::ui::custom_content::parse_ui_document(
                &table.get::<Value>("ui")?,
                "plugin healthcheck result.ui",
            )?);
            Ok(HealthcheckResult {
                status,
                message,
                ui_document,
            })
        }
        Value::String(value) => {
            let status = HealthStatus::parse(value.to_str()?.as_ref()).ok_or_else(|| {
                mlua::Error::runtime(
                    "plugin healthcheck must return a table or a valid status string",
                )
            })?;
            Ok(HealthcheckResult {
                status,
                message: String::new(),
                ui_document: None,
            })
        }
        other => Err(mlua::Error::runtime(format!(
            "plugin healthcheck must return a table, got {}",
            other.type_name()
        ))),
    }
}

pub(crate) fn plugin_handle_is_enabled(h5v: &Table, handle: &str) -> Result<bool, mlua::Error> {
    let plugins = match h5v.get::<Value>("plugins")? {
        Value::Nil => return Ok(true),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.plugins must be a table, got {}",
                other.type_name()
            )))
        }
    };
    let definitions = match plugins.get::<Value>(PLUGINS_DEFINITIONS_FIELD)? {
        Value::Nil => return Ok(true),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "h5v.plugins.__definitions must be a table, got {}",
                other.type_name()
            )))
        }
    };
    let definition = match definitions.get::<Value>(handle)? {
        Value::Nil => return Ok(true),
        Value::Table(table) => table,
        other => {
            return Err(mlua::Error::runtime(format!(
                "Plugin definition '{}' must be a table, got {}",
                handle,
                other.type_name()
            )))
        }
    };
    let status =
        optional_string_field_lua(&definition, "health_status", "h5v.plugins.__definitions")?
            .as_deref()
            .and_then(HealthStatus::parse)
            .unwrap_or(HealthStatus::Healthy);
    Ok(status != HealthStatus::Fail)
}

pub(crate) fn definition_owner_is_enabled(
    h5v: &Table,
    definition: &Table,
) -> Result<bool, mlua::Error> {
    let owner = optional_string_field_lua(definition, "owner", "registry definition")?;
    match owner.as_deref() {
        Some(value) if value.starts_with("plugin.") => plugin_handle_is_enabled(h5v, value),
        _ => Ok(true),
    }
}

fn optional_string_field_lua(
    table: &Table,
    field: &str,
    context: &str,
) -> Result<Option<String>, mlua::Error> {
    optional_string_field(table, field, context)
        .map_err(|error| mlua::Error::runtime(error.to_string()))
}

fn parse_auto_pull(options: Option<&Table>) -> Result<bool, mlua::Error> {
    let Some(options) = options else {
        return Ok(true);
    };
    match options.get::<Value>("auto_pull")? {
        Value::Nil => Ok(true),
        Value::Boolean(value) => Ok(value),
        other => Err(mlua::Error::runtime(format!(
            "h5v.plugins.use(..., {{ auto_pull = ... }}) expects a boolean, got {}",
            other.type_name()
        ))),
    }
}

#[derive(Debug, Clone)]
struct PluginManifest {
    id: String,
    name: String,
    version: String,
    entry: String,
}

#[derive(Debug, Clone)]
enum PluginSourceKind {
    Github { owner_repo: String },
    GitUrl { url: String },
    LocalPath { path: PathBuf },
}

#[derive(Debug, Clone)]
struct PluginSourceSpec {
    original: String,
    requested_ref: Option<String>,
    kind: PluginSourceKind,
}

#[derive(Debug, Clone)]
struct InstalledPlugin {
    source: PluginSourceSpec,
    manifest: PluginManifest,
    resolved_commit: String,
    auto_pull: bool,
    plugin_root: PathBuf,
}

fn parse_plugin_source(source: &str) -> Result<PluginSourceSpec, String> {
    let source = source.trim();
    if source.is_empty() {
        return Err("Plugin source cannot be empty".to_string());
    }
    let (base, requested_ref) = split_source_ref(source);
    let path_candidate = Path::new(base);
    if path_candidate.is_absolute() || base.starts_with("./") || base.starts_with("../") {
        return Ok(PluginSourceSpec {
            original: source.to_string(),
            requested_ref,
            kind: PluginSourceKind::LocalPath {
                path: path_candidate.to_path_buf(),
            },
        });
    }
    if base.contains("://") {
        return Ok(PluginSourceSpec {
            original: source.to_string(),
            requested_ref,
            kind: PluginSourceKind::GitUrl {
                url: base.to_string(),
            },
        });
    }
    if base.split('/').count() == 2 {
        return Ok(PluginSourceSpec {
            original: source.to_string(),
            requested_ref,
            kind: PluginSourceKind::Github {
                owner_repo: base.to_string(),
            },
        });
    }
    if path_candidate.exists() {
        return Ok(PluginSourceSpec {
            original: source.to_string(),
            requested_ref: None,
            kind: PluginSourceKind::LocalPath {
                path: path_candidate.to_path_buf(),
            },
        });
    }
    Err(format!(
        "Unsupported plugin source '{source}'. Use owner/repo, a git URL, or a local path"
    ))
}

fn split_source_ref(source: &str) -> (&str, Option<String>) {
    let Some(index) = source.rfind('@') else {
        return (source, None);
    };
    let last_slash = source.rfind('/').unwrap_or(0);
    if index <= last_slash {
        return (source, None);
    }
    let (base, requested_ref) = source.split_at(index);
    (
        base,
        Some(requested_ref.trim_start_matches('@').to_string()),
    )
}

fn install_plugin(source: &PluginSourceSpec, auto_pull: bool) -> Result<InstalledPlugin, String> {
    crate::ui::app::render_startup_progress("Loading plugin...", Some(source.original.as_str()));
    let plugin_root = match &source.kind {
        PluginSourceKind::LocalPath { path } => {
            if source.requested_ref.is_some() {
                return Err("Local plugin paths do not support @ref selectors".to_string());
            }
            path.canonicalize().map_err(|error| {
                format!(
                    "Failed to resolve plugin path '{}': {error}",
                    path.display()
                )
            })?
        }
        PluginSourceKind::Github { owner_repo } => install_git_plugin(
            &format!("https://github.com/{owner_repo}.git"),
            &source.original,
            source.requested_ref.as_deref(),
            auto_pull,
        )?,
        PluginSourceKind::GitUrl { url } => install_git_plugin(
            url,
            &source.original,
            source.requested_ref.as_deref(),
            auto_pull,
        )?,
    };
    let manifest = read_plugin_manifest(&plugin_root)?;
    let resolved_commit = resolve_git_commit(&plugin_root).unwrap_or_else(|_| "local".to_string());
    Ok(InstalledPlugin {
        source: source.clone(),
        manifest,
        resolved_commit,
        auto_pull,
        plugin_root,
    })
}

fn resolve_plugin_entry_path(installed: &InstalledPlugin) -> Result<PathBuf, String> {
    let entry_path = installed.plugin_root.join(&installed.manifest.entry);
    let entry_path = entry_path.canonicalize().map_err(|error| {
        format!(
            "Failed to resolve plugin entry '{}': {error}",
            entry_path.display()
        )
    })?;
    if !entry_path.starts_with(&installed.plugin_root) {
        return Err(format!(
            "Plugin entry '{}' must stay inside the plugin root",
            installed.manifest.entry
        ));
    }
    if !entry_path.is_file() {
        return Err(format!(
            "Plugin entry '{}' does not exist",
            entry_path.display()
        ));
    }
    Ok(entry_path)
}

fn install_git_plugin(
    url: &str,
    source_key: &str,
    requested_ref: Option<&str>,
    auto_pull: bool,
) -> Result<PathBuf, String> {
    let plugins_dir = plugin_state_dir().map_err(|error| error.to_string())?;
    fs::create_dir_all(&plugins_dir).map_err(|error| {
        format!(
            "Failed to create plugin state directory '{}': {error}",
            plugins_dir.display()
        )
    })?;
    let checkout_dir = plugins_dir.join(plugin_checkout_dir_name(source_key));
    if !checkout_dir.exists() {
        crate::ui::app::render_startup_progress("Cloning plugin...", Some(source_key));
        run_git(
            None,
            &[
                "clone".to_string(),
                "--quiet".to_string(),
                url.to_string(),
                checkout_dir.display().to_string(),
            ],
        )?;
    }

    if let Some(requested_ref) = requested_ref {
        crate::ui::app::render_startup_progress(
            "Fetching plugin...",
            Some(&format!("{source_key}@{requested_ref}")),
        );
        run_git(
            Some(&checkout_dir),
            &[
                "fetch".to_string(),
                "--quiet".to_string(),
                "origin".to_string(),
                requested_ref.to_string(),
            ],
        )?;
        crate::ui::app::render_startup_progress(
            "Checking out plugin...",
            Some(&format!("{source_key}@{requested_ref}")),
        );
        run_git(
            Some(&checkout_dir),
            &[
                "checkout".to_string(),
                "--quiet".to_string(),
                "FETCH_HEAD".to_string(),
            ],
        )?;
    } else if auto_pull {
        crate::ui::app::render_startup_progress("Updating plugin...", Some(source_key));
        run_git(
            Some(&checkout_dir),
            &[
                "fetch".to_string(),
                "--quiet".to_string(),
                "origin".to_string(),
                "main".to_string(),
            ],
        )?;
        crate::ui::app::render_startup_progress("Checking out plugin...", Some(source_key));
        run_git(
            Some(&checkout_dir),
            &[
                "checkout".to_string(),
                "--quiet".to_string(),
                "FETCH_HEAD".to_string(),
            ],
        )?;
    }

    checkout_dir.canonicalize().map_err(|error| {
        format!(
            "Failed to resolve plugin checkout '{}': {error}",
            checkout_dir.display()
        )
    })
}

fn read_plugin_manifest(plugin_root: &Path) -> Result<PluginManifest, String> {
    let manifest_path = plugin_root.join("h5v-plugin.toml");
    let manifest_source = fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "Failed to read plugin manifest '{}': {error}",
            manifest_path.display()
        )
    })?;
    let manifest: toml::Value = manifest_source.parse().map_err(|error| {
        format!(
            "Invalid plugin manifest '{}': {error}",
            manifest_path.display()
        )
    })?;
    let id = required_toml_string(&manifest, "id", &manifest_path)?;
    let name = required_toml_string(&manifest, "name", &manifest_path)?;
    let version = required_toml_string(&manifest, "version", &manifest_path)?;
    let api_version = required_toml_string(&manifest, "api_version", &manifest_path)?;
    if api_version != "2" {
        return Err(format!(
            "Plugin manifest '{}' must set api_version = \"2\"",
            manifest_path.display()
        ));
    }
    let entry = required_toml_string(&manifest, "entry", &manifest_path)?;
    Ok(PluginManifest {
        id,
        name,
        version,
        entry,
    })
}

fn required_toml_string(
    manifest: &toml::Value,
    field: &str,
    manifest_path: &Path,
) -> Result<String, String> {
    match manifest.get(field) {
        Some(toml::Value::String(value)) if !value.trim().is_empty() => Ok(value.clone()),
        Some(other) => Err(format!(
            "Plugin manifest '{}' field '{}' must be a non-empty string, got {}",
            manifest_path.display(),
            field,
            other.type_str()
        )),
        None => Err(format!(
            "Plugin manifest '{}' is missing required field '{}'",
            manifest_path.display(),
            field
        )),
    }
}

fn run_git(cwd: Option<&Path>, args: &[String]) -> Result<(), String> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|error| format!("Failed to run git {}: {error}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(format!(
        "git {} failed: {}{}",
        args.join(" "),
        stderr,
        if stdout.is_empty() {
            String::new()
        } else {
            format!(" ({stdout})")
        }
    ))
}

fn resolve_git_commit(path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|error| {
            format!(
                "Failed to resolve git commit for '{}': {error}",
                path.display()
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "Failed to resolve git commit for '{}'",
            path.display()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn plugin_state_dir() -> Result<PathBuf, std::io::Error> {
    Ok(dirs::data_dir()
        .unwrap_or(std::env::current_dir()?)
        .join("h5v")
        .join("plugins"))
}

fn plugin_checkout_dir_name(source: &str) -> String {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    let hash = hasher.finish();
    let stem = source
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    format!("{stem}_{hash:016x}")
}

fn plugin_handle(id: &str) -> PluginHandle {
    PluginHandle::new(format!("plugin.{id}"))
}

fn required_string_field(
    table: &Table,
    field: &str,
    context: &str,
) -> Result<String, ConfigureErrors> {
    match table.get::<Value>(field)? {
        Value::String(value) => {
            let value = value.to_str()?.trim().to_string();
            if value.is_empty() {
                return Err(mlua::Error::runtime(format!(
                    "{context}.{field} must be a non-empty string"
                ))
                .into());
            }
            Ok(value)
        }
        Value::Nil => Err(mlua::Error::runtime(format!("{context}.{field} is required")).into()),
        other => Err(mlua::Error::runtime(format!(
            "{context}.{field} must be a string, got {}",
            other.type_name()
        ))
        .into()),
    }
}

fn optional_string_field(
    table: &Table,
    field: &str,
    context: &str,
) -> Result<Option<String>, ConfigureErrors> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(None),
        Value::String(value) => Ok(Some(value.to_str()?.trim().to_string())),
        other => Err(mlua::Error::runtime(format!(
            "{context}.{field} must be a string, got {}",
            other.type_name()
        ))
        .into()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{parse_plugin_source, plugin_checkout_dir_name, read_plugin_manifest};

    #[test]
    fn parses_github_and_ref_sources() {
        let source = parse_plugin_source("owner/repo@stable").expect("parse plugin source");
        assert_eq!(source.requested_ref.as_deref(), Some("stable"));
    }

    #[test]
    fn checkout_dir_name_is_stable() {
        assert_eq!(
            plugin_checkout_dir_name("owner/repo@stable"),
            plugin_checkout_dir_name("owner/repo@stable")
        );
    }

    #[test]
    fn parses_manifest_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("h5v-plugin.toml"),
            r#"
id = "demo.analysis"
name = "Demo"
version = "0.1.0"
api_version = "2"
entry = "lua/main.lua"
"#,
        )
        .expect("write manifest");
        let manifest = read_plugin_manifest(temp.path()).expect("read manifest");
        assert_eq!(manifest.id, "demo.analysis");
        assert_eq!(manifest.entry, "lua/main.lua");
    }
}
