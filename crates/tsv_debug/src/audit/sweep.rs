//! The shared **pristine-format sweep** — the corpus loop of the audits that
//! format each seed AS AUTHORED and compare something about the two sides:
//! per file, skip `input_invalid_*` fixtures, read, format under
//! `catch_unwind`, bucket the skips identically, and hand each successfully
//! formatted `(path, parser, source, output)` to the audit's own visitor.
//!
//! `fabrication_audit` and `census_audit` are the consumers — twins of each
//! other in exactly this loop (the injection audits mutate per site and the
//! armed sweeps drive instrumentation, so neither fits). Extracted when the
//! census arrived as the loop's second verbatim copy; the conserved-content
//! census v2 (decoded literal values) is the expected third.

use std::path::{Path, PathBuf};

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::cli::commands::profile::is_input_invalid_fixture;

/// The sweep's bookkeeping: how many files were formatted, and why the rest
/// were not. The consumer's vacuity pin (`FORMATTED_MIN`) reads `formatted`;
/// the report tail reads [`Self::skipped_note`].
pub(crate) struct PristineSweep {
    /// Files successfully formatted (the visitor ran on each).
    pub(crate) formatted: usize,
    /// Files the parser or formatter rejected — no format claim, sanctioned skip.
    pub(crate) parse_errors: usize,
    /// Files `read_to_string` failed on (vanished mid-walk, permissions,
    /// non-UTF-8) — never formatted, so outside every other bucket. Counted
    /// rather than silently skipped so the report's file totals add up.
    pub(crate) read_errors: usize,
    /// Files whose format PANICKED. Reported separately rather than folded into
    /// `parse_errors`: a crash is not a rejection, and a bucket that calls it
    /// one would launder the loudest possible finding into a routine skip line.
    /// Not gated by these audits — the panic gates (`fuzz`, `blank_audit`) own
    /// that class.
    pub(crate) panics: usize,
}

impl PristineSweep {
    /// The skipped-file tail of a report line. A read failure or a panic is
    /// named only when one happened, so the common line stays short — but
    /// neither is ever folded into the parse count (see the field docs).
    pub(crate) fn skipped_note(&self) -> String {
        let mut parts = vec![format!("{} parse-skipped", self.parse_errors)];
        if self.read_errors > 0 {
            parts.push(format!("{} read-skipped", self.read_errors));
        }
        if self.panics > 0 {
            parts.push(format!("⚠ {} PANICKED", self.panics));
        }
        parts.join(", ")
    }
}

/// Format every file as authored and hand each `(path, parser, source, output)`
/// to `visit`. Requires the corpus profile's `panic = "unwind"` for the panic
/// bucket to catch anything — under `panic = "abort"` a formatter panic still
/// kills the process.
pub(crate) fn sweep_pristine(
    files: &[PathBuf],
    mut visit: impl FnMut(&Path, ParserType, &str, &str),
) -> PristineSweep {
    let mut sweep = PristineSweep {
        formatted: 0,
        parse_errors: 0,
        read_errors: 0,
        panics: 0,
    };
    for path in files {
        // Skip fixtures expected to fail parsing.
        if is_input_invalid_fixture(path) {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            sweep.read_errors += 1;
            continue;
        };
        let parser = ParserType::from_extension(&path.to_string_lossy());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            format_source(&source, parser)
        }));
        let output = match result {
            Ok(Ok(output)) => output,
            Ok(Err(_)) => {
                sweep.parse_errors += 1;
                continue;
            }
            Err(_) => {
                sweep.panics += 1;
                continue;
            }
        };
        sweep.formatted += 1;
        visit(path, parser, &source, &output);
    }
    sweep
}
