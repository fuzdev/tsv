//! The **audit substrate** — the reusable machinery an audit command builds on.
//!
//! A tsv audit takes a corpus of sources, checks a property of each, and grades
//! the findings against a policy. The pieces that recur across audits live here so
//! a new audit instantiates them rather than copying them:
//!
//! - [`ratchet`] — the "every line is a known bug, the file shrinking is the goal"
//!   snapshot gate (the shape a large, churny finding set needs instead of a count).
//! - [`census`] — the raw-text comment-trivia scanners behind the comment census
//!   (`census_audit`): per-language extraction that never consults the parser's
//!   own comment carrying, so parse-time drops are visible.
//! - [`sweep`] — the pristine-format corpus loop the as-authored ratchet audits
//!   (`fabrication_audit`, `census_audit`, `width_audit`) share: skip/read/format
//!   bookkeeping with a per-file visitor, plus the vacuity guard that refuses to
//!   grade a collapsed corpus.
//! - [`panic_hook`] — the suppressed-default-hook bracket every corpus walk that
//!   formats under `catch_unwind` installs, so N crashing files do not print N
//!   backtraces over the report.
//! - [`shape`] — the markup arm of the line-shape alphabet those snapshots key
//!   on, shared so two ratchets cannot drift into meaning different things by
//!   the same key.
//! - [`examples`] — the bounded, `--jobs`-deterministic example set every audit's
//!   per-shape aggregate keeps its reproducers in.
//! - [`tally`] — run-level tally primitives (the capped path-sample bucket the
//!   injection audits' skipped files and the sweep's panicking inputs share).
//! - [`node_edge`] — the wire-tree walker keying an injection offset to its
//!   enclosing AST node + child-role edge, the coarse companion to `sites`'
//!   file-independent shape keying.
//! - [`sites`] — site enumeration + file-independent shape keying for the
//!   gap-injection and blank-injection audits.
//! - [`properties`] — the per-input property layer: the panic-safe ledger format
//!   and verify verdicts, plus the shared parse-to-wire / reparse-skeleton
//!   primitives.
//! - [`report`] — the shared reporting envelope (`{severity, confidence, site,
//!   example, detail}`) and the human / JSON printers.
//!
//! `gap_audit`, `blank_audit`, and `ignore_audit` are the consumers of the whole
//! substrate; [`ratchet`] additionally serves `fabrication_audit`, `census_audit`,
//! and `compile_corpus_compare --ratchet`, which drive no ledger. The modules are
//! written generic where a second consumer would actually reuse them, and no
//! further.

// Always compiled: `properties` hosts the reparse primitives the `roundtrip_audit`
// / `fuzz` commands share, which are not behind `comment_check`. (Its ledger /
// verify layer is internally feature-gated.)
pub(crate) mod properties;

// The census scanners are NOT gated: `census_audit` lexes comment trivia off raw
// text (its independence from the parser's comment carrying is the whole point),
// so it drives no ledger and must exist in a default build.
pub(crate) mod census;

// The ratchet is NOT gated: `compile_corpus_compare --ratchet` (the validation-suite
// gate) drives no comment ledger, so it must exist in a default build. It is pure
// generic snapshot plumbing with no ledger dependency of its own.
pub(crate) mod ratchet;

// The pristine-format sweep is NOT gated: its consumers (`fabrication_audit`,
// `census_audit`, `width_audit`) drive no instrumentation seam.
pub(crate) mod sweep;

// The panic-hook bracket is NOT gated: `sweep` installs one in a default build.
// `ArmedRun` (gated, in `parallel`) holds one too — one definition, so the two
// corpus-walk shapes cannot drift into silencing panics differently.
pub(crate) mod panic_hook;

// The capped path sample is NOT gated: `sweep`'s panic bucket is one, and that
// bucket is what makes suppressing the hook lossless rather than silencing.
pub(crate) mod tally;

// The line-shape alphabet is NOT gated, for the same reason and the same
// consumers — it is pure text keying with no seam of its own.
pub(crate) mod shape;

// The injection machinery is only reachable through `gap_audit` / `blank_audit`,
// both themselves behind the `comment_check` feature (they arm
// `tsv_lang::comment_ledger`), so gate these too — otherwise they read as dead
// code in a default build.
#[cfg(feature = "comment_check")]
pub(crate) mod examples;
#[cfg(feature = "comment_check")]
pub(crate) mod node_edge;
#[cfg(feature = "comment_check")]
pub(crate) mod parallel;
#[cfg(feature = "comment_check")]
pub(crate) mod report;
#[cfg(feature = "comment_check")]
pub(crate) mod sites;
