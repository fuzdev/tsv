use std::env;
use tsv_cli::cli;

fn main() {
    let args: Vec<String> = env::args().collect();
    let registry = cli::build_registry();
    registry.run(args);
}
