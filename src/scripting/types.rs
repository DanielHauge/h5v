use hdf5_metno::SliceOrIndex;
use rhai::{Array, CustomType, TypeBuilder};

#[derive(Clone, Debug, PartialEq)]
pub struct DatasetLoad {
    pub path: String,
    pub selection: Vec<SliceOrIndex>,
}

impl DatasetLoad {
    pub fn dataset(path: String, selection: Vec<SliceOrIndex>) -> Self {
        Self { path, selection }
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
pub struct ContextLoad {
    selection: Vec<SliceOrIndex>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EntityLoad {
    Context(ContextLoad),
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
    pub fn dataset(path: String, selection: Vec<SliceOrIndex>) -> Self {
        Operation::Value(EntityLoad::Dataset(DatasetLoad::dataset(path, selection)))
    }

    pub fn attr(path: String, name: String) -> Self {
        Operation::Value(EntityLoad::Attribute(AttributeLoad::attr(path, name)))
    }
}

#[derive(Clone, Debug, CustomType, PartialEq, Default)]
pub struct LineSeries {
    pub title: Option<String>,
    pub x_label: Option<String>,
    pub y_label: Option<String>,
    pub x_data: Option<Operation>,
    pub y_data: Option<Operation>,
}

impl LineSeries {
    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    pub fn set_x_label(&mut self, x_label: String) {
        self.x_label = Some(x_label);
    }

    pub fn set_y_label(&mut self, y_label: String) {
        self.y_label = Some(y_label);
    }

    pub fn set_x_data(&mut self, x_data: Operation) {
        self.x_data = Some(x_data);
    }

    pub fn set_y_data(&mut self, y_data: Operation) {
        self.y_data = Some(y_data);
    }
}

#[derive(Clone, Debug, CustomType, PartialEq, Default)]
pub struct Chart {
    pub title: Option<String>,
    pub dpi: Option<i64>,
    pub series: Vec<LineSeries>,
    pub colors: Option<Vec<String>>,
}

impl Chart {
    pub fn set_title(&mut self, title: String) {
        self.title = Some(title);
    }

    pub fn set_dpi(&mut self, dpi: i64) {
        self.dpi = Some(dpi);
    }

    pub fn add_series(&mut self, series: LineSeries) {
        self.series.push(series);
    }

    pub fn set_colors(&mut self, colors: Vec<String>) {
        self.colors = Some(colors);
    }
}

pub fn register_load_types(engine: &mut rhai::Engine) {
    engine
        .register_type_with_name::<EntityLoad>("EntityLoad")
        .register_fn("attr", Operation::attr)
        .register_fn("dataset", |path: String, arr: Array| {
            let arr: Vec<SliceOrIndex> = arr
                .into_iter()
                .map(|x| {
                    x.try_cast::<SliceOrIndex>()
                        .unwrap_or(SliceOrIndex::Index(0))
                })
                .collect();
            Operation::dataset(path, arr.to_vec())
        })
        .register_type_with_name::<Operation>("Operation");

    engine.register_fn("ctx", |selection: Array| {
        let arr: Vec<SliceOrIndex> = selection
            .into_iter()
            .map(|x| {
                x.try_cast::<SliceOrIndex>()
                    .unwrap_or(SliceOrIndex::Unlimited {
                        start: 0,
                        step: 1,
                        block: 1,
                    })
            })
            .collect();
        Operation::Value(EntityLoad::Context(ContextLoad {
            selection: arr.to_vec(),
        }))
    });

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

    engine.register_fn("index", |x: i64| SliceOrIndex::Index(x as usize));

    engine.register_fn(
        "slice_adv",
        |start: i64, step: i64, end: i64, block: i64| SliceOrIndex::SliceTo {
            start: if start < 0 { 0 } else { start as usize },
            step: if step < 1 { 1 } else { step as usize },
            end: if end < 0 { 0 } else { end as usize },
            block: if block < 1 { 1 } else { block as usize },
        },
    );

    engine.register_fn("slice", |start: i64, end: i64| SliceOrIndex::SliceTo {
        start: if start < 0 { 0 } else { start as usize },
        step: 1,
        end: if end < 0 { 0 } else { end as usize },
        block: 1,
    });

    engine.register_fn("all", || SliceOrIndex::Unlimited {
        start: 0,
        step: 1,
        block: 1,
    });

    engine
        .register_type_with_name::<LineSeries>("line")
        .register_fn("line", LineSeries::default)
        .register_fn("set_title", LineSeries::set_title)
        .register_fn("set_x_label", LineSeries::set_x_label)
        .register_fn("set_y_label", LineSeries::set_y_label)
        .register_fn("set_x_data", LineSeries::set_x_data)
        .register_fn("set_y_data", LineSeries::set_y_data);

    engine
        .register_type_with_name::<Chart>("Chart")
        .register_fn("chart", Chart::default)
        .register_fn("set_colors", Chart::set_colors)
        .register_fn("set_title", Chart::set_title)
        .register_fn("set_dpi", Chart::set_dpi)
        .register_fn("add_series", Chart::add_series);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_engine_selection() {
        let mut engine = rhai::Engine::new();
        register_load_types(&mut engine);
        let script = r#"
        let sel = slice(0, 10);
        sel
    "#;
        let selection = engine.eval::<SliceOrIndex>(script).unwrap();
        assert_eq!(
            selection,
            SliceOrIndex::SliceTo {
                start: 0,
                step: 1,
                end: 10,
                block: 1
            }
        );
    }

    #[test]
    fn test_script_engine_line() {
        let mut engine = rhai::Engine::new();

        // Register external function as 'compute'
        register_load_types(&mut engine);

        let script = r#"
        let line = line();
        line.set_title("My line");
        line.set_x_label("X Axis");
        line.set_y_label("Y Axis");
        line.set_x_data(ctx([all()]);
        line.set_y_data(ctx([all()]);
        line.set_allow_zip(true);
        line
    "#;
        let plot_opts = engine.eval::<LineSeries>(script).unwrap();
        assert_eq!(plot_opts.title, Some("My line".to_string()));
        assert_eq!(plot_opts.x_label, Some("X Axis".to_string()));
        assert_eq!(plot_opts.y_label, Some("Y Axis".to_string()));
        assert_eq!(
            plot_opts.x_data,
            Some(Operation::Value(EntityLoad::Context(ContextLoad {
                selection: vec![SliceOrIndex::Unlimited {
                    start: 0,
                    step: 1,
                    block: 1
                }]
            })))
        );
        assert_eq!(
            plot_opts.y_data,
            Some(Operation::Value(EntityLoad::Context(ContextLoad {
                selection: vec![SliceOrIndex::Unlimited {
                    start: 0,
                    step: 1,
                    block: 1
                }]
            })))
        );
    }

    #[test]
    fn test_entity_load() {
        let mut engine = rhai::Engine::new();

        // Register external function as 'compute'
        register_load_types(&mut engine);

        let script = r#"
        let a = attr("path1", "name1");
        let selection = all();
        let b = dataset("path2", [selection]);
        let c = a + b;     // uses our registered "+"
        let d = ctx([all()]) - c; // uses our registered "ctx" and "-"
        d
    "#;
        let operation = engine.eval::<Operation>(script).unwrap();
        match operation {
            Operation::Subtract { left, right } => {
                assert_eq!(
                    *left,
                    Operation::Value(EntityLoad::Context(ContextLoad {
                        selection: vec![SliceOrIndex::Unlimited {
                            start: 0,
                            step: 1,
                            block: 1
                        }]
                    }))
                );
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
                                path: "path2".to_string(),
                                selection: vec![SliceOrIndex::Unlimited {
                                    start: 0,
                                    step: 1,
                                    block: 1
                                }]
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
