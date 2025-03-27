use clap::Parser;

mod h5f;
mod sprint_attributes;
mod sprint_typedesc;
mod ui;
mod ui_tree_view;

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
