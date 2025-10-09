use clap::Parser;

mod color_consts;
mod data;
mod error;
mod h5f;
mod search;
mod sprint_attributes;
mod sprint_typedesc;
mod ui;
mod utils;

#[derive(Parser, Debug)]
#[clap(
    author = "Daniel F. Hauge animcuil@gmail.com",
    about = "HDF5 Terminal Viewer (h5v)",
    version
)]
struct Args {
    /// Path to the HDF5 file to open
    file: String,
}

fn main() {
    let args = Args::parse();
    let file = args.file;
    ui::app::init(file).unwrap();
}
