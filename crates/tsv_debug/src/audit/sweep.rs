//! The shared **pristine-format sweep** — the corpus loop of the audits that
//! format each seed AS AUTHORED and compare something about the two sides:
//! per file, skip `input_invalid_*` fixtures, read, format under
//! `catch_unwind`, bucket the skips identically, and hand each successfully
//! formatted `(path, parser, source, output)` to the audit's own visitor.
//!
//! `fabrication_audit`, `census_audit`, `width_audit` and `swallow_audit` are
//! the consumers — the injection audits mutate per site, so none of those fit.
//! Extracted when the census arrived as the loop's second verbatim copy; the
//! conserved-content census v2 (decoded literal values) is the expected next.
//!
//! Every consumer walks the same seed list and skips the same classes, so they
//! all pin the same [`FIXTURES_FORMATTED_MIN`].

use std::path::{Path, PathBuf};

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::audit::panic_hook::{SuppressedPanicHook, panic_message};
use crate::audit::tally::CappedPaths;
use crate::cli::CliError;
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
    /// Files whose format PANICKED — an exact count plus a bounded
    /// `path: message` sample. Reported separately rather than folded into
    /// `parse_errors`: a crash is not a rejection, and a bucket that calls it
    /// one would launder the loudest possible finding into a routine skip line.
    /// Not gated by these audits — the panic gates (`fuzz`, `blank_audit`) own
    /// that class.
    ///
    /// The sample is what makes suppressing the default panic hook sound (see
    /// [`sweep_pristine_armed`]): the hook's per-file backtrace was the ONLY
    /// record of which input crashed, so dropping it without recording here
    /// would trade noise for silence.
    pub(crate) panics: CappedPaths,
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
        if !self.panics.is_empty() {
            parts.push(format!("⚠ {} PANICKED", self.panics.count()));
        }
        parts.join(", ")
    }

    /// The panicking inputs, `path: message`, bounded at [`CappedPaths::CAP`] —
    /// printed after the report so a crash names its reproducer even with the
    /// default hook suppressed.
    ///
    /// Prints rather than returning a string, and always to STDERR, so a
    /// `--json` run keeps a parseable stdout without each of the four consumers
    /// having to remember that.
    pub(crate) fn print_panic_sample(&self) {
        if self.panics.is_empty() {
            return;
        }
        eprintln!(
            "⚠ {} file(s) PANICKED while formatting (not gated here — the panic gates own that class):",
            self.panics.count()
        );
        for line in self.panics.sample_lines("    ") {
            eprintln!("{line}");
        }
    }
}

/// REGRESSION PIN (minimum, at the exact measured value): files a default
/// (`tests/fixtures`) run formats. **One corpus, one pin** — every consumer
/// resolves the same seed list (`resolve_seed_files`) and skips the same three
/// classes (`input_invalid_*`, a parse rejection, a panic), so their counts are
/// equal by construction, not by coincidence.
///
/// Shared rather than one const per audit because a private pin accumulates
/// SLACK: each is re-pinned at a different time, and the gap between a stale pin
/// and the live count is exactly the collapse [`check_formatted_min`] exists to
/// catch — a per-audit spread of a few hundred to ~1,700 files silently absorbed
/// a corpus collapse of that size.
///
/// The objection — "a future audit may legitimately skip differently" — argues
/// FOR sharing rather than against it. A divergent audit formats fewer files, so
/// it drops below this minimum and its gate FAILS, announcing the divergence at
/// the moment it is introduced; the remedy is then a deliberate private const for
/// that one audit. Private pins everywhere absorb the same divergence in silence.
pub(crate) const FIXTURES_FORMATTED_MIN: usize = 7_429;

/// The **vacuity guard** every corpus gate applies before grading: a default run
/// that formatted fewer than `min` files is not a passing gate, it is a
/// collapsed corpus — an empty walk or a parser that started rejecting
/// everything would otherwise report "0 findings across 0 files" and exit 0.
///
/// A minimum rather than a two-sided pin because the fixtures tree is COMMITTED
/// and grows with ordinary fixture PRs (`deno task check` must not fail per
/// added fixture); only shrinkage fails. Consumers pass
/// [`FIXTURES_FORMATTED_MIN`] and call this only on a full default run.
///
/// # Errors
///
/// Returns [`CliError::Failed`] (after a user-facing message) when fewer than
/// `min` files were formatted.
pub(crate) fn check_formatted_min(formatted: usize, min: usize) -> Result<(), CliError> {
    if formatted >= min {
        return Ok(());
    }
    eprintln!(
        "Error: pinned minimum — formatted {formatted} files < pinned {min}. \
         The fixtures walk shrank (or parsing collapsed), or this audit started skipping a \
         class the others do not; if deliberate, re-pin FIXTURES_FORMATTED_MIN."
    );
    Err(CliError::Failed)
}

/// Format every file as authored and hand each `(path, parser, source, output)`
/// to `visit`. Requires the corpus profile's `panic = "unwind"` for the panic
/// bucket to catch anything — under `panic = "abort"` a formatter panic still
/// kills the process.
pub(crate) fn sweep_pristine(
    files: &[PathBuf],
    visit: impl FnMut(&Path, ParserType, &str, &str),
) -> PristineSweep {
    sweep_pristine_armed(files, || {}, visit)
}

/// [`sweep_pristine`] with a per-file **arming** hook, for a consumer whose
/// instrumentation is a process-wide sink rather than a return value.
///
/// `arm` runs after the read succeeds and immediately before the format, which
/// is the only point that works: a sink drained *after* the visitor still holds
/// the reports of a seed the sweep skipped (a parse rejection and a panic never
/// reach the visitor), and those would then be attributed to the next file.
/// Draining ahead of every format makes what the visitor takes provably this
/// file's.
///
/// The hook exists so the one audit that interleaves drain/collect around the
/// format (`swallow_audit`) shares this loop's SKIP BUCKETING rather than
/// hand-rolling a fourth copy of it — a copy that dropped read failures on the
/// floor, so the file left every total it should have appeared in.
///
/// ⚠️ The DEFAULT PANIC HOOK IS SUPPRESSED for the whole walk, exactly as
/// `ArmedRun` (`audit::parallel`, gated with the injection audits, hence no
/// intra-doc link) does for those.  Catching a panic does not stop the hook from running first, so
/// without this a corpus with N crashing files printed N full backtraces over
/// the report — and here, unlike the injection audits, it stays latent until
/// the sweep is pointed at real code. The panicking input is recorded in
/// [`PristineSweep::panics`] instead, so the fix costs no information: it lives
/// in this shared loop rather than in any one consumer, because all four
/// (`fabrication_audit`, `census_audit`, `width_audit`, `swallow_audit`) format
/// under the same `catch_unwind`.
pub(crate) fn sweep_pristine_armed(
    files: &[PathBuf],
    mut arm: impl FnMut(),
    mut visit: impl FnMut(&Path, ParserType, &str, &str),
) -> PristineSweep {
    let _hook = SuppressedPanicHook::install();
    let mut sweep = PristineSweep {
        formatted: 0,
        parse_errors: 0,
        read_errors: 0,
        panics: CappedPaths::default(),
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
        arm();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            format_source(&source, parser)
        }));
        let output = match result {
            Ok(Ok(output)) => output,
            Ok(Err(_)) => {
                sweep.parse_errors += 1;
                continue;
            }
            Err(payload) => {
                sweep.panics.push(format!(
                    "{}: {}",
                    path.display(),
                    panic_message(payload.as_ref())
                ));
                continue;
            }
        };
        sweep.formatted += 1;
        visit(path, parser, &source, &output);
    }
    sweep
}
