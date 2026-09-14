//! Top-level argv and dispatch: an unknown command, a bare invocation, `--version`'s
//! placement, the command name's independence from `argv[0]`, and the help text.

use std::fs;
use std::process::Command;

use crate::common::{built_tsv, temp_dir, tsv};

#[test]
fn test_unknown_command() {
    let output = tsv(&["unknown-command"]);

    assert!(!output.status.success(), "Unknown command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unrecognized argument"),
        "Should report unknown command"
    );
}

/// `--help`'s extension list IS [`tsv_discover::FORMATTABLE_EXTENSIONS`], not a copy.
///
/// argh renders `--help` from doc comments, and a doc comment is a string literal — so
/// `FormatCommand`'s `paths` doc cannot build its list from the const the way
/// `unsupported_extension_error` and the nothing-in-scope error do
/// ([`tsv_discover::formattable_extension_list`]). That leaves two spellings of one fact,
/// and `--help` is the one that drifts silently: nothing else reads it, so a ninth
/// language would ship with a refusal naming nine and a help text naming eight. This is
/// what fails instead.
///
/// The comparison strips ALL whitespace from the help text rather than matching the line:
/// argh wraps at word boundaries, so a longer list moves to its own line — a real
/// rendering, not a drift — and the rendered list holds no whitespace of its own, which
/// makes the stripped `contains` exact rather than merely lenient.
#[test]
fn test_format_help_extension_list_is_rendered_from_the_const() {
    let output = tsv(&["format", "--help"]);
    let help: String = String::from_utf8_lossy(&output.stdout)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let list = tsv_discover::formattable_extension_list("/");
    assert!(
        help.contains(&list),
        "`tsv format --help` must name the formattable extensions as `{list}` (rendered from \
         tsv_discover::FORMATTABLE_EXTENSIONS); the `paths` doc comment in \
         crates/tsv_cli/src/cli/commands/format.rs has drifted from the const"
    );
}

/// `--version` beside a subcommand is refused rather than printing the version and
/// dropping the rest — the one argv shape argh accepts that means two things at once.
#[test]
fn test_version_with_a_subcommand_is_refused() {
    let output = tsv(&["--version", "format", "--check", "/nonexistent"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error: --version cannot be combined with a subcommand"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("Run tsv --help for more information."));
    let alone = tsv(&["--version"]);
    assert_eq!(alone.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&alone.stdout).starts_with("tsv "));
}

/// A bare `tsv` is argh's own required-subcommand refusal — `main` answers only a bare
/// `--version` ahead of argh, so the subcommand stays required and nothing restates this
/// text by hand on the native side. The bytes are pinned because the JS mirror prints
/// the same ones (`cli.js`'s `main`), exit 1 on both.
#[test]
fn test_no_command() {
    let output = tsv(&[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "One of the following subcommands must be present:\n    help\n    parse\n    format\n\nRun tsv --help for more information.\n"
    );
}

/// `--version` is declared on the top-level struct rather than only intercepted in
/// `main`, so that `--help` lists it; and a repeated switch is the one argh counts
/// (`cli.js`'s transcription does too), so `--version --version` prints the version once.
#[test]
fn test_help_lists_the_version_switch_and_a_repeat_still_prints_it() {
    let help = tsv(&["--help"]);
    assert_eq!(help.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(
        stdout.starts_with("Usage: tsv [--version] <command> [<args>]\n"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("--version         print the tsv version"),
        "stdout: {stdout}"
    );

    let repeated = tsv(&["--version", "--version"]);
    assert_eq!(repeated.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&repeated.stdout),
        format!("tsv {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn test_version_flag() {
    let output = tsv(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("tsv {}\n", env!("CARGO_PKG_VERSION")),
        "exact `tsv <workspace version>` line — the npm cli.js mirrors it from its package.json"
    );
    assert!(output.stderr.is_empty());
}

/// The command name in usage/help/error text is pinned to `tsv`, never derived
/// from `argv[0]` — which is what `argh::from_env` does, and what printed
/// `Usage: tsv.exe format` on Windows. Running a renamed copy varies exactly
/// that dimension on any platform, so the property is provable here rather than
/// only in a Windows CI leg.
#[test]
#[allow(clippy::expect_used)]
fn test_command_name_is_independent_of_argv0() {
    let dir = temp_dir("argv0_name");
    let renamed = dir.join(format!("tsv_renamed{}", std::env::consts::EXE_SUFFIX));
    fs::copy(built_tsv(), &renamed).expect("Failed to copy the tsv binary");

    // ⚠️ Retry on ETXTBSY. `fs::copy` above closes its destination handle before
    // returning, but this harness runs tests on parallel threads and most of them
    // spawn a child process: between another thread's fork and its exec the child
    // holds a *copy* of every open descriptor, so a fork that straddles the copy
    // leaves the new inode write-open in that child until it execs. The kernel's
    // check is on the inode's writecount, so exec'ing the fresh binary in that
    // window fails with `ExecutableFileBusy` — a race in the harness, never in
    // the binary. The window closes with the other child's exec, so a bounded
    // retry is the fix; the copy itself cannot avoid it.
    let run = |args: &[&str]| {
        for _ in 0..50 {
            match Command::new(&renamed).args(args).output() {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                result => return result.expect("Failed to execute the renamed tsv binary"),
            }
        }
        panic!("the renamed tsv binary stayed ExecutableFileBusy for a second");
    };

    let help = run(&["help", "format"]);
    assert_eq!(help.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(
        stdout.starts_with("Usage: tsv format"),
        "renamed binary must still name itself `tsv`: {stdout}"
    );

    // The error path carries the same name (argh's `Run <cmd> --help` line).
    let bad = run(&["format", "--parser", "bogus", "--content", "x"]);
    assert_eq!(bad.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        stderr.contains("Run tsv --help for more information."),
        "stderr: {stderr}"
    );
}

#[test]
fn test_version_is_top_level_only() {
    // Subcommands don't take --version — argh's unrecognized-argument error, which
    // the JS mirror's transcription of argh repeats word for word (exit 1).
    let output = tsv(&["format", "--version"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Unrecognized argument: --version"),
        "should fail as an unknown subcommand argument, not print a version"
    );
}
