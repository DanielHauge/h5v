mod types;

#[cfg(test)]
mod tests {
    use rhai::Engine;

    use crate::scripting::types::AttributeLoad;

    #[test]
    fn test_rhai_engine_basic() {
        // Create scripting engine
        let mut engine = Engine::new();

        // Register external function as 'compute'
        engine
            .register_type::<AttributeLoad>()
            .register_fn("attr", AttributeLoad::new)
            .register_fn("read", AttributeLoad::get_as_f64);

        let result: AttributeLoad = engine
            .eval("let gg = attr(\"test\",\"hello\");\n gg")
            .unwrap();

        assert_eq!(result.path, "test");
    }

    #[test]
    fn test_rhai_engine_float() {
        // Create scripting engine
        let mut engine = Engine::new();

        // Register external function as 'compute'
        engine
            .register_type::<AttributeLoad>()
            .register_fn("attr", AttributeLoad::new)
            .register_fn("read", AttributeLoad::get_as_f64)
            .register_fn("read_unwrap", AttributeLoad::get_as_f64_unwrap);

        let result: Result<f64, String> = engine
            .eval("let gg = attr(\"test\",\"hello\");\n let floatf = gg.read(); floatf")
            .unwrap();
        assert_eq!(result, Err("Not implemented".into()))
    }
}
