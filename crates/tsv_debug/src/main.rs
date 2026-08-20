use std::process::ExitCode;
use tsv_debug::cli;

/// The single exit point: dispatch the subcommand and map its outcome to a
/// process exit code. Every command threads its decision back here as a
/// [`cli::CliError`] instead of calling `std::process::exit` directly.
///
/// The dispatch runs on the same stack reservation the `tsv` binary states
/// (`tsv_cli::cli::stack`), for the same reason and one more: these commands run the
/// parser and printer over whole corpora, and a stack overflow is not a catchable
/// panic — so the `corpus` profile's `panic = "unwind"`, which turns a per-file panic
/// into a per-file *report*, can do nothing about an overflow. Unsized, one pathological
/// file kills an audit sweep outright, at whatever depth the machine's `RLIMIT_STACK`
/// happens to allow. The `tsc_conformance` sweep still spawns its own far larger stack
/// on top of this: its corpus is adversarial by construction, not incidentally deep.
fn main() -> ExitCode {
    let cmd: cli::TopLevel = argh::from_env();
    tsv_cli::cli::stack::run_on_sized_stack(move || match cmd.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => ExitCode::from(e.exit_code()),
    })
}
