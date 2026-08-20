use argh::FromArgs;

/// The command name argh prints in usage, help, and error text.
///
/// Pinned rather than derived from `argv[0]` (what `argh::from_env` does): the
/// binary is `tsv.exe` on Windows and is invoked through the npm loader's `tsv`
/// bin everywhere, so a derived name would print `Usage: tsv.exe format` on one
/// platform and disagree with the JS mirror's hand-written help, which spells
/// the same contract as `tsv` on every platform. Running a renamed copy of the
/// binary is how `tests/cli_tests.rs` holds this — the property is otherwise
/// only observable on Windows.
const CMD_NAME: &str = "tsv";

fn main() {
    // `argh::from_env` with the command name fixed — same early-exit contract:
    // `--help` to stdout with exit 0, a parse error to stderr with exit 1.
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(std::ffi::OsString::into_string)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|arg| {
            eprintln!("Invalid utf8: {}", arg.to_string_lossy());
            std::process::exit(1)
        });
    let arg_strs: Vec<&str> = args.iter().map(String::as_str).collect();

    let cmd =
        tsv_cli::cli::TopLevel::from_args(&[CMD_NAME], &arg_strs).unwrap_or_else(|early_exit| {
            std::process::exit(match early_exit.status {
                Ok(()) => {
                    println!("{}", early_exit.output);
                    0
                }
                Err(()) => {
                    eprintln!(
                        "{}\nRun {CMD_NAME} --help for more information.",
                        early_exit.output
                    );
                    1
                }
            })
        });
    // Every subcommand runs on one explicitly sized stack — see `cli::stack`. Doing it
    // here rather than inside the recursive commands is what keeps the ceiling the
    // same for `parse` and for both of `format`'s routes.
    tsv_cli::cli::stack::run_on_sized_stack(move || cmd.run());
}
