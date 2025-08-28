use rhai::{CustomType, TypeBuilder};

#[derive(Clone, CustomType, Debug)]
pub struct AttributeLoad {
    pub path: String,
    pub name: String,
}

impl AttributeLoad {
    pub fn new(path: String, name: String) -> Self {
        AttributeLoad { path, name }
    }

    // Signatures must be &mut self first
    pub fn get_as_f64(&mut self) -> Result<f64, String> {
        Err("Not implemented".into())
    }

    pub fn get_as_f64_unwrap(&mut self) -> f64 {
        self.get_as_f64().unwrap()
    }
}

#[derive(Clone, CustomType)]
struct DatasetLoad {
    path: String,
}

