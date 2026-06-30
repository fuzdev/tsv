mod cli;
mod deno;
mod diff;
mod error;
mod fixtures;
mod render_normalize;
mod test262;

use std::process::ExitCode;

// EXPERIMENTAL perf A/B (off by default): swap the global allocator for mimalloc
// behind the `mimalloc` feature. Setting it in this binary crate routes every
// allocation in the linked workspace crates (parser/printer) through mimalloc.
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

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
