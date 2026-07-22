use crate::cli::CliError;
use crate::deno::{self, SvelteGenerate};
use crate::diff::{DiffOptions, diff_to_string};
use argh::FromArgs;
use tsv_cli::cli::input::{InputArgs, ParserType};
use tsv_cli::json_utils::to_json_with_tabs;
use tsv_svelte_compile::{
    CompileError, CompileOptions, Generate, Parity, canonicalize_js, compare_canonical, compile,
};

/// Compare tsv's Svelte compile output against the canonical Svelte compiler.
///
/// Both sides' compiled JS is canonicalized (an intent-erased reprint) before
/// comparison, so a diff reflects a real code difference, not incidental
/// whitespace. Exit codes: 0 = parity, 1 = a real difference, 2 = an error
/// (including a component shape tsv's compiler doesn't cover yet).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compile_compare")]
pub struct CompileCompareCommand {
    /// compile target: server | client (default: server)
    #[argh(option, default = "SvelteGenerate::Server")]
    target: SvelteGenerate,

    /// emit a JSON report instead of a human diff
    #[argh(switch)]
    json: bool,

    /// content to compile
    #[argh(option)]
    content: Option<String>,

    /// read from stdin
    #[argh(switch)]
    stdin: bool,

    /// file path
    #[argh(positional)]
    file: Option<String>,
}

/// Machine-readable `--json` report.
#[derive(serde::Serialize)]
struct CompareReport {
    /// The compile target ("server" | "client").
    target: &'static str,
    /// Whether the two canonical forms match (byte-exact OR comment-position-tolerated).
    parity: bool,
    /// True when parity was reached only by tolerating a comment-position difference
    /// (`compare_canonical` → `CommentPosition`), not byte-exactness.
    comment_position_tolerated: bool,
    /// The tsv side's outcome ("ok" | "unsupported").
    ours_status: &'static str,
    /// The unified diff of the two canonical forms, when both sides exist and differ
    /// (present for a tolerated position difference too, to show what moved).
    hunks: Option<String>,
}

impl CompileCompareCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let json = self.json;
        let target = self.target;

        // Compile is Svelte-only, so force the parser (matching `canonical_compile`).
        let input_args = InputArgs {
            content: self.content,
            stdin: self.stdin,
            parser: Some(ParserType::Svelte),
            file: self.file,
        };
        let input = match input_args.resolve() {
            Ok((input, _parser)) => input,
            Err(e) => {
                eprintln!("Error: {e}");
                return Err(CliError::Errored);
            }
        };
        let source = input.content();

        // Oracle side: compile with the canonical compiler, then canonicalize.
        let rt = super::create_runtime();
        let oracle_output = match rt.block_on(deno::svelte_compile(source, target, false)) {
            Ok(o) => o,
            Err(err) => {
                eprintln!("Error: oracle compile failed: {err}");
                return Err(CliError::Errored);
            }
        };
        let oracle_canonical = match canonicalize_js(&oracle_output.js) {
            Ok(c) => c,
            Err(err) => {
                eprintln!("Error: could not canonicalize oracle output: {err}");
                return Err(CliError::Errored);
            }
        };
        // Self-check: the canonicalizer must be idempotent on the oracle output.
        // A violation is a real bug in the canonicalizer, so surface it loudly.
        match canonicalize_js(&oracle_canonical) {
            Ok(again) if again == oracle_canonical => {}
            Ok(_) => {
                eprintln!("Error: canonicalizer is not idempotent on the oracle output (bug)");
                return Err(CliError::Errored);
            }
            Err(err) => {
                eprintln!("Error: re-canonicalizing oracle output failed: {err}");
                return Err(CliError::Errored);
            }
        }

        // Ours side: compile with tsv.
        let options = CompileOptions {
            generate: to_generate(target),
            dev: false,
        };
        match compile(source, &options) {
            Ok(ours_output) => match canonicalize_js(&ours_output.js) {
                Ok(ours_canonical) => report_both(target, &ours_canonical, &oracle_canonical, json),
                Err(err) => {
                    eprintln!("Error: could not canonicalize tsv output: {err}");
                    Err(CliError::Errored)
                }
            },
            Err(err @ CompileError::Unsupported(_)) => {
                report_unsupported(target, &err, &oracle_canonical, json)
            }
            Err(err) => {
                eprintln!("Error: tsv compile failed: {err}");
                Err(CliError::Errored)
            }
        }
    }
}

/// Report when both sides produced output: parity (exit 0) or a real diff (exit 1).
fn report_both(
    target: SvelteGenerate,
    ours: &str,
    oracle: &str,
    json: bool,
) -> Result<(), CliError> {
    // The parity bar tolerates comment-POSITION differences (tsv's comment placement
    // vs the oracle's esrap). A byte diff still displays — for a tolerated position
    // difference it shows what moved. See `compare_canonical`.
    let verdict = compare_canonical(ours, oracle);
    let parity = verdict.is_parity();
    let tolerated = verdict == Parity::CommentPosition;
    let differs = ours != oracle;
    if json {
        let hunks = differs.then(|| diff_to_string(ours, oracle, &DiffOptions::compile_compare()));
        let report = CompareReport {
            target: target_name(target),
            parity,
            comment_position_tolerated: tolerated,
            ours_status: "ok",
            hunks,
        };
        print_json(&report)?;
    } else if parity {
        let note = if tolerated {
            " (comment-position tolerated)"
        } else {
            ""
        };
        println!("compile_compare [{}] parity{note}", target_name(target));
        if tolerated {
            print!(
                "{}",
                diff_to_string(ours, oracle, &DiffOptions::compile_compare())
            );
        }
    } else {
        println!(
            "compile_compare [{}] canonical outputs differ",
            target_name(target)
        );
        print!(
            "{}",
            diff_to_string(ours, oracle, &DiffOptions::compile_compare())
        );
    }
    if parity {
        Ok(())
    } else {
        Err(CliError::Failed)
    }
}

/// Report a shape tsv's compiler doesn't cover yet (exit 2). The oracle
/// canonical form is shown so it's visible what tsv must reproduce.
fn report_unsupported(
    target: SvelteGenerate,
    err: &CompileError,
    oracle: &str,
    json: bool,
) -> Result<(), CliError> {
    if json {
        let report = CompareReport {
            target: target_name(target),
            parity: false,
            comment_position_tolerated: false,
            ours_status: "unsupported",
            hunks: None,
        };
        print_json(&report)?;
    } else {
        println!(
            "compile_compare [{}] {err} — oracle canonical form:",
            target_name(target)
        );
        print_block(oracle);
    }
    Err(CliError::Errored)
}

/// Serialize `report` as tab-indented JSON to stdout.
fn print_json(report: &CompareReport) -> Result<(), CliError> {
    match to_json_with_tabs(report) {
        Ok(json) => {
            println!("{json}");
            Ok(())
        }
        Err(err) => {
            eprintln!("Error serializing report: {err}");
            Err(CliError::Errored)
        }
    }
}

/// Print `text`, ensuring exactly one trailing newline.
fn print_block(text: &str) {
    print!("{text}");
    if !text.ends_with('\n') {
        println!();
    }
}

/// Map the oracle target to the tsv compiler's own target enum.
fn to_generate(target: SvelteGenerate) -> Generate {
    match target {
        SvelteGenerate::Server => Generate::Server,
        SvelteGenerate::Client => Generate::Client,
    }
}

/// The target's canonical name for reporting.
fn target_name(target: SvelteGenerate) -> &'static str {
    match target {
        SvelteGenerate::Server => "server",
        SvelteGenerate::Client => "client",
    }
}
