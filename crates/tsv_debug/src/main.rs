mod cli;
mod deno;
mod diff;
mod error;
mod fixtures;
mod render_normalize;
mod test262;

fn main() {
    let cmd: cli::TopLevel = argh::from_env();
    cmd.run();
}
