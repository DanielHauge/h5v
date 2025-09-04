use hdf5_metno::{Group, Hyperslab, Selection};
use ndarray::ArrayD;

use crate::scripting::types::{Chart, Operation};

pub fn chartup(chart: Chart, file: Group, context: Group) -> Result<(), String> {
    for lineserie in chart.series {
        let y_data = match lineserie.y_data {
            Some(op) => eval_operation(&file, &context, op)?,
            None => return Err("LineSeries is missing y_data".to_string()),
        };
        let _x = match lineserie.x_data {
            Some(op) => eval_operation(&file, &context, op)?,
            None => make_linspace(0.0, (y_data.len() - 1) as f64, y_data.len()),
        };
    }
    Ok(())
}

fn make_linspace(start: f64, stop: f64, num: usize) -> ArrayD<f64> {
    let step = (stop - start) / (num - 1) as f64;
    let mut data = Vec::with_capacity(num);
    for i in 0..num {
        data.push(start + step * i as f64);
    }
    ArrayD::from_shape_vec(ndarray::IxDyn(&[num]), data).unwrap()
}

pub fn eval_operation(file: &Group, context: &Group, op: Operation) -> Result<ArrayD<f64>, String> {
    match op {
        Operation::Addition { left, right } => {
            let left = eval_operation(file, context, *left)?;
            let right = eval_operation(file, context, *right)?;
            Ok(&left + &right)
        }
        Operation::Subtract { left, right } => {
            let left = eval_operation(file, context, *left)?;
            let right = eval_operation(file, context, *right)?;
            Ok(&left - &right)
        }
        Operation::Multiply { left, right } => {
            let left = eval_operation(file, context, *left)?;
            let right = eval_operation(file, context, *right)?;
            Ok(&left * &right)
        }
        Operation::Divide { left, right } => {
            let left = eval_operation(file, context, *left)?;
            let right = eval_operation(file, context, *right)?;
            Ok(&left / &right)
        }
        Operation::Value(entity_load) => match entity_load {
            super::types::EntityLoad::Context(context_load) => {
                let ds = context.as_dataset().map_err(|e| e.to_string())?;
                let selection = Selection::Hyperslab(Hyperslab::from(context_load.selection));
                Ok(ds.read_slice(selection).map_err(|e| e.to_string())?)
            }
            super::types::EntityLoad::Dataset(dataset_load) => {
                let ds = match file.dataset(&dataset_load.path) {
                    Ok(ds) => ds,
                    Err(_) => context
                        .dataset(&dataset_load.path)
                        .map_err(|e| e.to_string())?,
                };
                let selection = Selection::Hyperslab(Hyperslab::from(dataset_load.selection));
                Ok(ds.read_slice(selection).map_err(|e| e.to_string())?)
            }
            super::types::EntityLoad::Attribute(attribute_load) => {
                let g = match file.group(&attribute_load.path) {
                    Ok(g) => g,
                    Err(_) => context
                        .group(&attribute_load.path)
                        .map_err(|e| e.to_string())?,
                };
                let attr = g.attr(&attribute_load.name).map_err(|e| e.to_string())?;
                Ok(match attr.read_scalar::<f64>() {
                    Ok(v) => ArrayD::from_shape_vec(ndarray::IxDyn(&[1]), vec![v]).unwrap(),
                    Err(_) => match attr.read_raw::<f64>() {
                        Ok(v) => {
                            ArrayD::from_shape_vec(ndarray::IxDyn(&[v.len()]), v.to_vec()).unwrap()
                        }
                        Err(e) => return Err(e.to_string()),
                    },
                })
            }
        },
    }
}
