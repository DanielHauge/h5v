use clap::Parser;

mod color_consts;
mod data;
mod error;
mod h5f;
mod search;
mod sprint_attributes;
mod sprint_typedesc;
mod ui;

use git_version::git_version;
const GIR_VERSION: &str =
    git_version!(args = ["--always", "--dirty=-modified", "--tags", "--abbrev=4"]);

#[derive(Parser, Debug)]
#[clap(
    version = GIR_VERSION,
    author = "Your Name animcuil@gmail.com",
    about = "HDF5 Terminal Viewer (h5v)"
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
