use clap::Parser;

mod color_consts;
mod data;
mod h5f;
mod search;
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
    ui::app::init(file).unwrap();
}
