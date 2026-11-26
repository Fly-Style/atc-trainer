use std::path::PathBuf;

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    atc_client::sector_viewer::run(path);
}
