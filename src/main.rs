use clap::Parser;

mod color_consts;
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
    let h5f = h5f::H5F::open(file).unwrap();
    ui::app::init(h5f).unwrap();
}
