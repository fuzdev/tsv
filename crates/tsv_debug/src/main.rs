use std::env;

mod cli;
mod fixtures;

fn main() {
    let args: Vec<String> = env::args().collect();
    let registry = cli::build_registry();
    registry.run(args);
}
