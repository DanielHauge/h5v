use crate::scripting::types::register_load_types;

pub fn create_engine() -> rhai::Engine {
    let mut engine = rhai::Engine::new();

    register_load_types(&mut engine);

    engine
}
