use std::{sync::mpsc::Sender, time::Instant};

use mlua::Error as LuaError;

use crate::{
    configure::{self, install_registry_snapshot, ThemeName},
    ui::{app::AppEvent, command::sync_command_registry_keybindings},
};

use super::super::errors::ConfigureErrors;
use super::bootstrap::{execute_config_chunk, prepare_lua_config};
use super::keymaps::store_config_lua_runtime;
use super::registration::{
    apply_lua_config_with_snapshot, parse_compatibility_override, register_lua_config,
};
use super::{set_last_config_load_metrics, ConfigLoadMetrics};

pub fn load_config_compatibility(
    default_compatibility: bool,
) -> Result<Option<bool>, ConfigureErrors> {
    let prepare_started = Instant::now();
    let prepared = prepare_lua_config(None, default_compatibility)?;
    tracing::info!(
        kind = "config",
        phase = "compatibility_prepare",
        duration_ms = prepare_started.elapsed().as_millis() as u64,
        chunk_name = prepared.chunk_name.as_str(),
        message = "prepared compatibility config load"
    );
    let _registry_builder = prepared.registry_builder;
    let lua = prepared.lua;
    let h5v = prepared.h5v;
    let chunk_name = prepared.chunk_name;
    let config = prepared.config;
    let execute_started = Instant::now();
    execute_config_chunk(&lua, &chunk_name, &config)?;
    tracing::info!(
        kind = "config",
        phase = "compatibility_execute",
        duration_ms = execute_started.elapsed().as_millis() as u64,
        chunk_name = chunk_name.as_str(),
        message = "executed compatibility config chunk"
    );
    parse_compatibility_override(&h5v)
}

pub fn run_lua_engine(
    events: Sender<AppEvent>,
    default_compatibility: bool,
) -> Result<(), ConfigureErrors> {
    crate::health::clear_reported_health_issues();
    let prepare_started = Instant::now();
    let prepared = match prepare_lua_config(Some(events), default_compatibility) {
        Ok(prepared) => prepared,
        Err(error) => {
            crate::health::push_reported_health_issue(crate::health::ReportedHealthIssue::new(
                "configuration",
                crate::health::HealthcheckResult::fail(error.to_string()),
            ));
            return Err(error);
        }
    };
    tracing::info!(
        kind = "config",
        phase = "prepare",
        duration_ms = prepare_started.elapsed().as_millis() as u64,
        chunk_name = prepared.chunk_name.as_str(),
        message = "prepared Lua config runtime"
    );
    let mut registry_builder = prepared.registry_builder;
    let lua = prepared.lua;
    let h5v = prepared.h5v;
    let chunk_name = prepared.chunk_name;
    let config = prepared.config;
    let previous_config = configure::snapshot_config();

    configure::reset_config(ThemeName::Dark);
    let apply_started = Instant::now();
    let result = (|| -> Result<(), ConfigureErrors> {
        let execute_started = Instant::now();
        execute_config_chunk(&lua, &chunk_name, &config)?;
        tracing::info!(
            kind = "config",
            phase = "execute",
            duration_ms = execute_started.elapsed().as_millis() as u64,
            chunk_name = chunk_name.as_str(),
            message = "executed Lua config chunk"
        );
        let register_started = Instant::now();
        register_lua_config(&mut registry_builder, &h5v)?;
        let registry_snapshot = registry_builder
            .freeze()
            .map_err(|error| LuaError::runtime(error.to_string()))?;
        tracing::info!(
            kind = "config",
            phase = "register",
            duration_ms = register_started.elapsed().as_millis() as u64,
            message = "registered and froze Lua config registry"
        );
        let apply_state_started = Instant::now();
        apply_lua_config_with_snapshot(&registry_snapshot, &h5v)?;
        install_registry_snapshot(registry_snapshot);
        sync_command_registry_keybindings(&configure::current_keymaps());
        store_config_lua_runtime(lua);
        tracing::info!(
            kind = "config",
            phase = "apply",
            duration_ms = apply_state_started.elapsed().as_millis() as u64,
            total_duration_ms = apply_started.elapsed().as_millis() as u64,
            message = "applied Lua config runtime"
        );
        Ok(())
    })();
    if result.is_err() {
        configure::restore_config(previous_config);
        if let Err(error) = &result {
            if crate::health::reported_health_issues().is_empty() {
                crate::health::push_reported_health_issue(crate::health::ReportedHealthIssue::new(
                    "configuration",
                    crate::health::HealthcheckResult::fail(error.to_string()),
                ));
            }
            tracing::error!(
                kind = "config",
                phase = "apply",
                total_duration_ms = apply_started.elapsed().as_millis() as u64,
                error = %error,
                message = "Lua config runtime failed"
            );
        }
    }
    set_last_config_load_metrics(ConfigLoadMetrics {
        total_duration_ms: apply_started.elapsed().as_millis() as u64,
    });
    result
}
