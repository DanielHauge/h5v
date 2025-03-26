use std::{any::Any, io::Read, process::exit, u8};

use clap::Parser;
use hdf5_metno::{
    types::{dyn_value::DynVarLenString, FixedAscii, VarLenAscii},
    Error, H5Type,
};
use sprint_attributes::{sprint_attribute, Stringer};

mod h5f;
mod sprint_attributes;
mod sprint_typedesc;
mod ui;

#[derive(Parser, Debug)]
struct Args {
    file: String,
}

fn main() {
    let args = Args::parse();
    let file = args.file;
    let mut h5f = h5f::H5F::open(file).unwrap();
    ui::init(&mut h5f).unwrap();
}
