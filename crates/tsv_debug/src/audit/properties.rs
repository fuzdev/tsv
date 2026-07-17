//! Per-input properties an audit checks — the panic-safe primitives that turn a
//! source string into a verdict.
//!
//! This is the shared home for the **input → property** layer that every audit
//! in the [`audit`](crate::audit) substrate builds on:
//!
//! - **reparse** — [`tsv_parse_to_value`] (parse to the wire `Value`) and
//!   [`structurally_equivalent`] (the structural-skeleton compare), the
//!   round-trip primitives the `roundtrip_audit` / `fuzz` commands share.
//! - **ledger** (behind the `comment_check` feature) — [`ledger_format`] /
//!   [`pristine_format`] drive `format_source` with the print-once comment
//!   ledger armed, and [`predict_comment_count`] plus the [`Verdict`] /
//!   [`VerifyOutcome`] / [`VerifySummary`] verdict types turn a ledger claim
//!   into a falsifiable, self-verified outcome. `gap_audit` is the only consumer
//!   today.
//!
//! It is the intended future home for the rest of the shared property set — the
//! no-panic guard, the F1 idempotency fixed point, the reparse-skeleton compare,
//! and the ledger-clean check — as the other audits (roundtrip / fuzz / F1 sweep)
//! migrate onto the substrate.

use serde_json::Value;

use tsv_cli::cli::input::ParserType;

use crate::diff::{DiffOptions, diff_to_string};
use crate::render_normalize::{normalize_pair, structural_skeleton};

/// Parse `source` with tsv's own parser and convert to the wire-JSON `Value`
/// (the same shape the canonical ASTs use). `None` on a tsv parse error.
///
/// The parse-to-wire primitive shared across the audit substrate — the
/// `roundtrip_audit` / `fuzz` round-trips and the gap audit's Svelte region walk
/// ([`sites::code_regions`](crate::audit::sites)) all reduce a source string to
/// this `Value`.
pub(crate) fn tsv_parse_to_value(source: &str, parser: ParserType) -> Option<Value> {
    let arena = bumpalo::Bump::new();
    match parser {
        ParserType::TypeScript => {
            let ast = tsv_ts::parse(source, &arena).ok()?;
            Some(tsv_ts::convert_ast_json(&ast, source))
        }
        ParserType::Svelte => {
            let ast = tsv_svelte::parse(source, &arena).ok()?;
            Some(tsv_svelte::convert_ast_json(&ast, source))
        }
        ParserType::Css => {
            let ast = tsv_css::parse(source, &arena).ok()?;
            Some(tsv_css::convert_ast_json(&ast, source))
        }
    }
}

/// Compare two ASTs for **structural** equivalence — the corruption-hunt basis.
///
/// Both are [`normalize_pair`]'d (render-normalized when `render`, then
/// location-stripped) and compared as [`structural_skeleton`]s, so legitimate
/// leaf reformatting doesn't read as corruption while an injected / dropped /
/// re-typed node still does (see `structural_skeleton` for what the skeleton
/// keeps vs erases). Char-dropping *value* corruption stays covered by the
/// complementary `corpus:compare:format` SAFETY (differential char-frequency),
/// which this deliberately does not duplicate.
///
/// Returns `(structurally_equal, diff)` — the diff (only with `verbose`) shows the
/// full location-stripped values, not the skeleton, so it's readable for triage.
///
/// Shared by the `roundtrip_audit` and `fuzz` commands.
pub(crate) fn structurally_equivalent(
    a: Value,
    b: Value,
    render: bool,
    verbose: bool,
) -> (bool, Option<String>) {
    let (a, b) = normalize_pair(a, b, render);
    if structural_skeleton(&a) == structural_skeleton(&b) {
        return (true, None);
    }
    let diff = if verbose {
        match (
            serde_json::to_string_pretty(&a),
            serde_json::to_string_pretty(&b),
        ) {
            (Ok(pa), Ok(pb)) => Some(diff_to_string(&pa, &pb, &DiffOptions::ast_diff())),
            _ => None,
        }
    } else {
        None
    };
    (false, diff)
}

// The ledger-driven property layer is only reachable through the `comment_check`
// feature (it arms `tsv_lang::comment_ledger`), so production and default
// `tsv_debug` builds compile it out entirely — the same gate the audits that
// consume it (`comment_audit`, `gap_audit`) sit behind.
#[cfg(feature = "comment_check")]
pub(crate) use ledger::*;

#[cfg(feature = "comment_check")]
mod ledger {
    use tsv_cli::cli::format_source::format_source;
    use tsv_cli::cli::input::ParserType;
    use tsv_lang::comment_ledger::{self, CommentFinding, CommentFindingKind};

    /// What one ledger-armed format did.
    pub(crate) enum Formatted {
        /// The parser or printer panicked — a finding in its own right (a comment in a gap
        /// must never crash the formatter).
        Panicked,
        /// The source did not parse, so the injection is not a legal comment here. The
        /// overwhelmingly common case, and **not** a finding: it means the offset names no gap.
        Rejected,
        /// Formatted.
        Ok {
            /// The ledger's findings — normally empty.
            findings: Vec<CommentFinding>,
            /// How many comments the document registered. Doubles as a needle-free "how many
            /// comments are in this text" measure: `ledger_format(text).parsed` counts them
            /// with the real lexer, so `verify_example` never has to string-match a comment
            /// whose text the printer may legitimately re-indent.
            parsed: usize,
            /// The formatted text, already built by `format_source` — free to carry.
            output: String,
        },
    }

    /// Format `src` with the ledger armed and drain it.
    ///
    /// Drains on every path, including the failing ones: the ledger is thread-local and keyed
    /// on source identity, so a straggler left by a rejected parse could otherwise be attributed
    /// to the next injection.
    pub(crate) fn ledger_format(src: &str, parser: ParserType) -> Formatted {
        let _ = comment_ledger::take_comment_ledger();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| format_source(src, parser)));
        match result {
            Err(_) => {
                let _ = comment_ledger::take_comment_ledger();
                Formatted::Panicked
            }
            Ok(Err(_)) => {
                let _ = comment_ledger::take_comment_ledger();
                Formatted::Rejected
            }
            Ok(Ok(output)) => {
                let ledger = comment_ledger::take_comment_ledger();
                Formatted::Ok {
                    findings: ledger.findings,
                    parsed: ledger.parsed,
                    output,
                }
            }
        }
    }

    /// The pristine-format outcome for a seed file: whether it is injectable, and if so the byte
    /// spans of the comments it already holds.
    ///
    /// The audit checks a file is clean *as authored* before injecting. `Clean` also carries the
    /// existing comment spans so `injection_sites` can skip a site that falls strictly *inside*
    /// one — injecting there mutilates the author's comment (a `line` payload terminates it
    /// early) rather than probing a gap, which reads as a false drop.
    pub(crate) enum Pristine {
        /// Rejected, panicked, or already dirty — not injected into. `dirty` distinguishes the
        /// already-had-findings case (reported) from the doesn't-parse case (silently skipped).
        Skip { dirty: bool },
        /// Clean; carries the byte spans of the comments the seed already holds (empty when it
        /// has none).
        Clean { comment_spans: Vec<tsv_lang::Span> },
    }

    /// Format `src` once to check it is clean AND capture its registered comment spans.
    ///
    /// Kept separate from [`ledger_format`] because it reads the spans **before** the drain (via
    /// [`comment_ledger::parsed_comment_spans`], which the drain discards). Only the once-per-file
    /// pristine check needs them; the per-injection hot path only ever drains and must not pay to
    /// collect them.
    pub(crate) fn pristine_format(src: &str, parser: ParserType) -> Pristine {
        let _ = comment_ledger::take_comment_ledger();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| format_source(src, parser)));
        match result {
            // A seed that panics or doesn't parse is not injectable — nothing to report, just skip.
            Err(_) | Ok(Err(_)) => {
                let _ = comment_ledger::take_comment_ledger();
                Pristine::Skip { dirty: false }
            }
            Ok(Ok(_output)) => {
                // Pass `src` itself, so `document_key(src)` matches the host document by pointer
                // identity and the spans are strictly host-absolute (a nested `<style>` island
                // registers under its own key and is excluded — see `parsed_comment_spans`).
                let comment_spans = comment_ledger::parsed_comment_spans(src);
                let ledger = comment_ledger::take_comment_ledger();
                if ledger.findings.is_empty() {
                    Pristine::Clean { comment_spans }
                } else {
                    Pristine::Skip { dirty: true }
                }
            }
        }
    }

    /// How many comments the output must hold, if the ledger's account of `findings` is true.
    ///
    /// Each dropped comment removes one. Each double-printed one adds `emitted - 1` — **not**
    /// one: `CommentFinding::emitted` is documented as `>= 2`, so a comment printed three times
    /// adds two, and assuming "double" means exactly twice would mispredict it as
    /// [`Verdict::Unconfirmed`].
    ///
    /// Split out and unit-tested because it is arithmetic: an off-by-one here changes a verdict
    /// and nothing else, and no corpus run would show it — the audit would simply file a
    /// confirmed finding under the wrong bucket.
    pub(crate) fn predict_comment_count(parsed: usize, findings: &[CommentFinding]) -> usize {
        let dropped = findings
            .iter()
            .filter(|f| f.kind == CommentFindingKind::Dropped)
            .count();
        let extra: usize = findings
            .iter()
            .filter(|f| f.kind == CommentFindingKind::DoublePrinted)
            .map(|f| f.emitted.saturating_sub(1))
            .sum();
        // `dropped` counts registered comments, so it can never exceed `parsed`; saturate
        // rather than risk a panic on a ledger that ever breaks that invariant.
        parsed.saturating_sub(dropped) + extra
    }

    /// Whether ONE of a shape's examples survives an **observational** re-check, independent of
    /// the ledger that reported it.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum Verdict {
        /// Re-formatting really does lose (or duplicate) a comment. The ledger's claim is
        /// visible in the output.
        Confirmed,
        /// The ledger says a comment was never emitted, yet the output holds just as many
        /// comments as its input. Something printed it without recording the emit — or printed
        /// a *mangled* rebuild of it (`/* a⏎b */` → `/* ab */`, one comment either way). Real
        /// either way, but not the plain drop it is filed as.
        Unconfirmed,
    }

    /// A shape's self-verification tally across its kept examples — the ratio that separates
    /// "uniformly an instrument gap" from "a mixed real drop".
    #[derive(Clone, Copy, Debug)]
    pub(crate) struct VerifyOutcome {
        /// Examples whose ledger claim was reproduced against the output.
        pub(crate) confirmed: usize,
        /// Examples verified — up to `VERIFY_EXAMPLES`, never zero for a recorded shape.
        pub(crate) total: usize,
    }

    impl VerifyOutcome {
        pub(crate) fn summary(self) -> VerifySummary {
            match self.confirmed {
                // A recorded shape always has ≥1 example, so `total == 0` is unreachable; treat
                // it as clean rather than risk a divide-by-nothing reading.
                _ if self.total == 0 => VerifySummary::Clean,
                0 => VerifySummary::Unconfirmed,
                c if c == self.total => VerifySummary::Clean,
                _ => VerifySummary::Partial,
            }
        }

        /// The report suffix — empty when every example confirmed (nothing to flag), else the
        /// `confirmed/total` ratio behind an `UNCONFIRMED` / `PARTIAL` label.
        pub(crate) fn report_label(self) -> String {
            let ratio = format!("({}/{} confirmed)", self.confirmed, self.total);
            match self.summary() {
                VerifySummary::Clean => String::new(),
                VerifySummary::Unconfirmed => format!("  ⚠ UNCONFIRMED {ratio}"),
                VerifySummary::Partial => format!("  ⚠ PARTIAL {ratio}"),
            }
        }
    }

    /// The three-way per-shape verdict once every kept example has been re-checked.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum VerifySummary {
        /// Every kept example reproduced — the finding is what it says it is.
        Clean,
        /// Some examples reproduced, some didn't — a mixed real drop.
        Partial,
        /// No kept example reproduced — uniformly an instrument gap (likely mangles, not drops).
        Unconfirmed,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tsv_lang::comment_ledger::{CommentFinding, CommentFindingKind};

        /// The ledger's falsifiable prediction. Arithmetic, so the corpus cannot grade it.
        #[test]
        fn predicted_comment_count_accounts_for_each_finding() {
            let f = |kind, emitted| CommentFinding {
                kind,
                text: "/* c */".to_string(),
                span: tsv_lang::Span { start: 0, end: 7 },
                emitted,
            };
            // No findings ⇒ the output keeps every comment.
            assert_eq!(predict_comment_count(5, &[]), 5);
            // A drop removes exactly one.
            assert_eq!(
                predict_comment_count(5, &[f(CommentFindingKind::Dropped, 0)]),
                4
            );
            // A comment printed TWICE adds one — but one printed THREE times adds two. This is
            // the case a "double means 2" reading gets wrong.
            assert_eq!(
                predict_comment_count(5, &[f(CommentFindingKind::DoublePrinted, 2)]),
                6
            );
            assert_eq!(
                predict_comment_count(5, &[f(CommentFindingKind::DoublePrinted, 3)]),
                7
            );
            // Mixed findings compose.
            assert_eq!(
                predict_comment_count(
                    5,
                    &[
                        f(CommentFindingKind::Dropped, 0),
                        f(CommentFindingKind::DoublePrinted, 2)
                    ]
                ),
                5
            );
            // A ledger that broke its own invariant must not panic the audit.
            assert_eq!(
                predict_comment_count(
                    0,
                    &[
                        f(CommentFindingKind::Dropped, 0),
                        f(CommentFindingKind::Dropped, 0)
                    ]
                ),
                0
            );
        }

        /// The verify ratio's three-way split, and the labels that carry it. Arithmetic, so no
        /// corpus run grades it.
        #[test]
        fn verify_outcome_splits_clean_partial_unconfirmed() {
            let out = |confirmed, total| VerifyOutcome { confirmed, total };
            assert_eq!(out(5, 5).summary(), VerifySummary::Clean);
            assert_eq!(out(1, 1).summary(), VerifySummary::Clean);
            assert_eq!(out(0, 5).summary(), VerifySummary::Unconfirmed);
            assert_eq!(out(2, 5).summary(), VerifySummary::Partial);

            assert_eq!(out(5, 5).report_label(), "", "clean flags nothing");
            assert_eq!(out(0, 5).report_label(), "  ⚠ UNCONFIRMED (0/5 confirmed)");
            assert_eq!(out(2, 5).report_label(), "  ⚠ PARTIAL (2/5 confirmed)");
        }
    }
}
