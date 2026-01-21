use std::env;

mod cli;
mod deno;
mod diff;
mod error;
mod fixtures;
mod subprocess;
mod test262;

fn main() {
    let args: Vec<String> = env::args().collect();
    let registry = cli::build_registry();
    registry.run(args);
}
