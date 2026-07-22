mod audit;
mod cli;
mod compile_fixtures;
mod deno;
mod diff;
mod error;
mod fixtures;
mod render_browser;
mod render_normalize;
mod test262;

use std::process::ExitCode;

/// The single exit point: dispatch the subcommand and map its outcome to a
/// process exit code. Every command threads its decision back here as a
/// [`cli::CliError`] instead of calling `std::process::exit` directly.
fn main() -> ExitCode {
    let cmd: cli::TopLevel = argh::from_env();
    match cmd.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => ExitCode::from(e.exit_code()),
    }
}
