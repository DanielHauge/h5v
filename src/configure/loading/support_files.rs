use std::path::Path;

use crate::configure::errors::ConfigureErrors;

use super::pathing::config_parent_dir;
use super::{
    lua_ls_config_contents, lua_ls_plugin_config_contents, lua_ls_stub_contents,
    should_refresh_lua_ls_config, LUA_LS_CONFIG_FILE, LUA_LS_LIBRARY_DIR, LUA_LS_STUB_FILE,
};

pub(super) fn ensure_lua_ls_support_files(config_path: &Path) -> Result<(), ConfigureErrors> {
    let parent_dir = config_parent_dir(config_path);
    ensure_lua_ls_support_files_in_dir(parent_dir, &lua_ls_config_contents(), false)
}

pub fn refresh_plugin_lua_ls_support_files(plugin_root: &Path) -> Result<(), ConfigureErrors> {
    ensure_lua_ls_support_files_in_dir(plugin_root, &lua_ls_plugin_config_contents(), true)
}

fn ensure_lua_ls_support_files_in_dir(
    root_dir: &Path,
    lua_rc_contents: &str,
    force_write: bool,
) -> Result<(), ConfigureErrors> {
    let lua_ls_dir = root_dir.join(LUA_LS_LIBRARY_DIR);
    std::fs::create_dir_all(&lua_ls_dir).map_err(ConfigureErrors::FailureCreateDefault)?;
    std::fs::write(lua_ls_dir.join(LUA_LS_STUB_FILE), lua_ls_stub_contents()?)
        .map_err(ConfigureErrors::FailureCreateDefault)?;

    let lua_rc_path = root_dir.join(LUA_LS_CONFIG_FILE);
    let should_write_config = if force_write || !lua_rc_path.exists() {
        true
    } else {
        let existing =
            std::fs::read_to_string(&lua_rc_path).map_err(ConfigureErrors::FailureCreateDefault)?;
        should_refresh_lua_ls_config(&existing)
    };
    if should_write_config {
        std::fs::write(lua_rc_path, lua_rc_contents)
            .map_err(ConfigureErrors::FailureCreateDefault)?;
    }
    Ok(())
}
