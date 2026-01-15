use clap::Parser;

mod color_consts;
mod data;
mod error;
mod h5f;
mod linking;
// mod scripting;
mod search;
mod sprint_attributes;
mod sprint_typedesc;
mod ui;
mod utils;

use git_version::git_version;

use crate::error::AppError;
const GIR_VERSION: &str =
    git_version!(args = ["--always", "--dirty=-modified", "--tags", "--abbrev=4"]);

#[derive(Parser, Debug)]
#[clap(
    author = "Daniel F. Hauge animcuil@gmail.com",
    about = "HDF5 Terminal Viewer (h5v)",
    version = GIR_VERSION
)]
struct Args {
    /// Path to the HDF5 file to open
    files: Vec<String>,
}

fn main() -> Result<(), AppError> {
    let args = Args::parse();

    match &args.files[..] {
        [] => Err(AppError::FileError(String::from("No files given"))), // TODO: Provide some usage tip etc.
        [single] => ui::app::init(single.clone()),
        multiple => ui::app::init(linking::link(multiple)?),
    }
}
