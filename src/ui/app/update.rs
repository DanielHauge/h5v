use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};
use update_informer::{registry, Check};

use crate::{error::log_error, GIT_VERSION_SHORT};

pub(super) const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60 * 24);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UpdateCheckCache {
    pub(super) current_version: String,
    pub(super) checked_at_unix_secs: u64,
    pub(super) available_version: Option<String>,
}

pub(super) fn update_check_cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|path| path.join("h5v").join("update-check.json"))
}

fn parse_update_check_cache(value: Value) -> Option<UpdateCheckCache> {
    let object = value.as_object()?;
    Some(UpdateCheckCache {
        current_version: object.get("current_version")?.as_str()?.to_string(),
        checked_at_unix_secs: object.get("checked_at_unix_secs")?.as_u64()?,
        available_version: object
            .get("available_version")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    })
}

fn load_update_check_cache(path: &Path) -> Option<UpdateCheckCache> {
    let contents = fs::read_to_string(path).ok()?;
    let value = serde_json::from_str(&contents).ok()?;
    parse_update_check_cache(value)
}

pub(super) fn write_update_check_cache(
    path: &Path,
    cache: &UpdateCheckCache,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string(&json!({
        "current_version": cache.current_version,
        "checked_at_unix_secs": cache.checked_at_unix_secs,
        "available_version": cache.available_version,
    }))
    .map_err(std::io::Error::other)?;
    fs::write(path, contents)
}

pub(super) fn update_check_cache_is_fresh(
    cache: &UpdateCheckCache,
    current_version: &str,
    now: SystemTime,
) -> bool {
    if cache.current_version != current_version {
        return false;
    }
    let now_unix_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    now_unix_secs.saturating_sub(cache.checked_at_unix_secs) < UPDATE_CHECK_INTERVAL.as_secs()
}

fn fetch_latest_h5v_version() -> std::result::Result<Option<String>, String> {
    let informer =
        update_informer::new(registry::Crates, "h5v", GIT_VERSION_SHORT).interval(Duration::ZERO);
    informer
        .check_version()
        .map(|version| version.map(|version| version.to_string()))
        .map_err(|error| error.to_string())
}

pub(super) fn resolve_available_update<F>(
    cache_path: Option<&Path>,
    current_version: &str,
    now: SystemTime,
    fetch_latest: F,
) -> Option<String>
where
    F: FnOnce() -> std::result::Result<Option<String>, String>,
{
    let existing_cache = cache_path.and_then(load_update_check_cache);
    if let Some(cache) = existing_cache.as_ref() {
        if update_check_cache_is_fresh(cache, current_version, now) {
            return cache.available_version.clone();
        }
    }

    let fallback_version = existing_cache
        .as_ref()
        .filter(|cache| cache.current_version == current_version)
        .and_then(|cache| cache.available_version.clone());
    let available_version = match fetch_latest() {
        Ok(version) => version,
        Err(error) => {
            log_error(format!("Update check failed: {error}"));
            fallback_version
        }
    };

    if let Some(cache_path) = cache_path {
        let checked_at_unix_secs = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let cache = UpdateCheckCache {
            current_version: current_version.to_string(),
            checked_at_unix_secs,
            available_version: available_version.clone(),
        };
        if let Err(error) = write_update_check_cache(cache_path, &cache) {
            log_error(format!(
                "Failed to write update-check cache '{}': {error}",
                cache_path.display()
            ));
        }
    }

    available_version
}

pub(super) fn check_for_available_update(now: SystemTime) -> Option<String> {
    resolve_available_update(
        update_check_cache_path().as_deref(),
        GIT_VERSION_SHORT,
        now,
        fetch_latest_h5v_version,
    )
}
