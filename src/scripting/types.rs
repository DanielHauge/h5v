use rhai::{CustomType, TypeBuilder};

#[derive(Clone, Debug, PartialEq)]
pub struct DatasetLoad {
    pub path: String,
}

impl DatasetLoad {
    pub fn dataset(path: String) -> Self {
        Self { path }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeLoad {
    pub path: String,
    pub name: String,
}

impl AttributeLoad {
    pub fn attr(path: String, name: String) -> Self {
        Self { path, name }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum EntityLoad {
    CurrentDataset,
    Dataset(DatasetLoad),
    Attribute(AttributeLoad),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Addition {
        left: Box<Operation>,
        right: Box<Operation>,
    },
    Subtract {
        left: Box<Operation>,
        right: Box<Operation>,
    },
    Multiply {
        left: Box<Operation>,
        right: Box<Operation>,
    },
    Divide {
        left: Box<Operation>,
        right: Box<Operation>,
    },
    Value(EntityLoad),
}

impl Operation {
    pub fn dataset(path: String) -> Self {
        Operation::Value(EntityLoad::Dataset(DatasetLoad::dataset(path)))
    }

    pub fn attr(path: String, name: String) -> Self {
        Operation::Value(EntityLoad::Attribute(AttributeLoad::attr(path, name)))
    }
}

#[derive(Clone, Debug, CustomType, PartialEq)]
pub struct Plot {
    pub title: Option<String>,
    pub x_label: Option<String>,
    pub y_label: Option<String>,
    pub dpi: Option<i64>,
    pub x_data: Option<Operation>,
    pub y_data: Vec<Operation>,
    pub allow_zip: bool,
}

impl Default for Plot {
    fn default() -> Self {
        Self {
            title: None,
            x_label: None,
            y_label: None,
            dpi: Some(600),
            x_data: None,
            y_data: vec![],
            allow_zip: false,
        }
    }
}

impl Plot {
    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    pub fn set_x_label(&mut self, x_label: String) {
        self.x_label = Some(x_label);
    }

    pub fn set_y_label(&mut self, y_label: String) {
        self.y_label = Some(y_label);
    }

    pub fn set_dpi(&mut self, dpi: i64) {
        self.dpi = Some(dpi);
    }

    pub fn set_x_data(&mut self, x_data: Operation) {
        self.x_data = Some(x_data);
    }

    pub fn set_y_data(&mut self, y_data: Operation) {
        self.y_data.push(y_data);
    }

    pub fn set_allow_zip(&mut self, allow_zip: bool) {
        self.allow_zip = allow_zip;
    }
}

pub fn register_load_types(engine: &mut rhai::Engine) {
    engine
        .register_type_with_name::<EntityLoad>("EntityLoad")
        .register_fn("attr", Operation::attr)
        .register_fn("dataset", Operation::dataset)
        .register_type_with_name::<Operation>("Operation");

    engine.register_fn("ctx", || Operation::Value(EntityLoad::CurrentDataset));

    engine.register_fn("+", |left: Operation, right: Operation| {
        Operation::Addition {
            left: Box::new(left),
            right: Box::new(right),
        }
    });

    engine.register_fn("-", |left: Operation, right: Operation| {
        Operation::Subtract {
            left: Box::new(left),
            right: Box::new(right),
        }
    });

    engine.register_fn("*", |left: Operation, right: Operation| {
        Operation::Multiply {
            left: Box::new(left),
            right: Box::new(right),
        }
    });

    engine.register_fn("/", |left: Operation, right: Operation| Operation::Divide {
        left: Box::new(left),
        right: Box::new(right),
    });

    engine
        .register_type_with_name::<Plot>("Plot")
        .register_fn("plot", Plot::default)
        .register_fn("set_title", Plot::set_title)
        .register_fn("set_x_label", Plot::set_x_label)
        .register_fn("set_y_label", Plot::set_y_label)
        .register_fn("set_dpi", Plot::set_dpi)
        .register_fn("set_x_data", Plot::set_x_data)
        .register_fn("set_allow_zip", Plot::set_allow_zip)
        .register_fn("set_y_data", Plot::set_y_data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_engine_plot() {
        let mut engine = rhai::Engine::new();

        // Register external function as 'compute'
        register_load_types(&mut engine);

        let script = r#"
        let plot = plot();
        plot.set_title("My Plot");
        plot.set_x_label("X Axis");
        plot.set_y_label("Y Axis");
        plot.set_dpi(300);
        plot.set_x_data(ctx());
        plot.set_y_data(ctx());
        plot.set_allow_zip(true);
        plot
    "#;
        let plot_opts = engine.eval::<Plot>(script).unwrap();
        assert_eq!(plot_opts.title, Some("My Plot".to_string()));
        assert_eq!(plot_opts.x_label, Some("X Axis".to_string()));
        assert_eq!(plot_opts.y_label, Some("Y Axis".to_string()));
        assert!(plot_opts.allow_zip);
        assert_eq!(plot_opts.dpi, Some(300));
        assert_eq!(
            plot_opts.x_data,
            Some(Operation::Value(EntityLoad::CurrentDataset))
        );
        assert_eq!(plot_opts.y_data.len(), 1);
        assert_eq!(
            plot_opts.y_data[0],
            Operation::Value(EntityLoad::CurrentDataset)
        );
    }

    #[test]
    fn test_entity_load() {
        let mut engine = rhai::Engine::new();

        // Register external function as 'compute'
        register_load_types(&mut engine);

        let script = r#"
        let a = attr("path1", "name1");
        let b = dataset("path2");
        let c = a + b;     // uses our registered "+"
        let d = ctx() - c; // uses our registered "ctx" and "-"
        d
    "#;
        let operation = engine.eval::<Operation>(script).unwrap();
        match operation {
            Operation::Subtract { left, right } => {
                assert_eq!(*left, Operation::Value(EntityLoad::CurrentDataset));
                match *right {
                    Operation::Addition { left, right } => {
                        assert_eq!(
                            *left,
                            Operation::Value(EntityLoad::Attribute(AttributeLoad {
                                path: "path1".to_string(),
                                name: "name1".to_string()
                            }))
                        );
                        assert_eq!(
                            *right,
                            Operation::Value(EntityLoad::Dataset(DatasetLoad {
                                path: "path2".to_string()
                            }))
                        );
                    }
                    _ => panic!("Expected Addition operation"),
                }
            }
            _ => panic!("Expected Subtract operation"),
        }
    }
}
