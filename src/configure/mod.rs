use std::sync::mpsc::Sender;

use mlua::Lua;

use crate::{
    configure::errors::ConfigureErrors,
    ui::{app::AppEvent, state::AppToast},
};
pub mod errors;
mod loading;

pub fn run_lua_engine(events: Sender<AppEvent>) -> Result<(), ConfigureErrors> {
    let lua = Lua::new();

    let h5v = lua.create_table()?;

    let events_clone = events.clone();
    let log_fn = lua.create_function(move |_, msg: String| {
        let _ = events_clone
            .to_owned()
            .send(AppEvent::Toast(AppToast::Info(msg)));
        Ok(())
    })?;

    h5v.set("log", log_fn)?;

    lua.globals().set("h5v", h5v)?;

    let config = loading::load_or_create_config()?;

    lua.load(&config).exec()?;

    Ok(())
}
