//! The **validation-suite ratchet**: a path-keyed known-bug snapshot over Svelte's own
//! `compiler-errors` + `validator` suites, turning the compiler's over-acceptance debt
//! into a standing gate.
//!
//! # Why these suites, and why a ratchet
//!
//! `compile_corpus_compare`'s ordinary corpus is real components — overwhelmingly
//! *valid* Svelte, so it exercises the compiler's emission and barely touches its
//! **validation** surface. The two suites here are the inverse: ~2/3 of their files are
//! deliberately INVALID, each written to trip exactly one oracle rule. They sit in no
//! other gated corpus, and every file the oracle rejects that tsv nevertheless compiles
//! is an OVER-ACCEPTANCE — a refusal-contract bug (see the crate's refusal contract:
//! nothing invalid in runes mode may compile).
//!
//! There are enough of those today that a green gate is not reachable, so the shape is a
//! [`Ratchet`] rather than a pass/fail: every line is a known bug and the file shrinking
//! is the goal. A *new* over-acceptance fails; a pinned one that stops firing fails
//! (so fixing one forces removing its line, and the list cannot rot).
//!
//! # Why a PATH key works here
//!
//! `compile_fuzz`'s findings are *generated* mutants — a path key would be meaningless
//! there, since a corpus edit rewrites which mutants exist. These files are **authored
//! and committed upstream**, at stable paths, so the path IS the finding's identity. The
//! oracle error code rides along, so a pin bump that changes which rule rejects a file
//! re-triages rather than silently matching.
//!
//! # The four key kinds
//!
//! - `OVER-ACCEPT` — **pinnable**. The debt this gate exists to ratchet down.
//! - `MISMATCH` — **never pinnable**. Both sides compiled and the canonical code differs;
//!   by the refusal contract that is always a bug, so it can never be laundered into the
//!   list whose shrinking is the goal. It fails unconditionally, exactly as it does in
//!   [`exit_verdict`](super::exit_verdict).
//! - `HARNESS-ERROR` — **never pinnable**. Every harness failure that is not the oracle
//!   itself rejecting-by-throwing: a tsv-side compiler bug (`tsv-corrupt-output`,
//!   `tsv-type-erasure-leak`, `tsv-parse`), a canonicalizer bug (`canonicalize-ours`,
//!   `canonicalize-oracle`, `oracle-recanonicalize`, `oracle-non-idempotent`), or an
//!   environment failure (`read`, `oracle-sidecar`). None of those are upstream's, so
//!   none may be laundered into a list whose header tells the next reader that an
//!   errored line is *the oracle's* bug. A `CorruptOutput` or a `TypeErasureLeak` is a
//!   compiler bug that fails its run everywhere else in this repo; it fails here too.
//! - `ORACLE-ERROR` — **pinnable**, and deliberately so rather than an exclusion list.
//!   One upstream file (`validator/samples/silence-warnings-2`) makes the pinned oracle
//!   *throw* rather than reject: it carries a `svelte-ignore` for a warning whose
//!   construction path dereferences an unset source locator, so
//!   `compile(src, { runes: true })` dies in `state.js`'s `locator` with "An impossible
//!   situation occurred". Verified against the pin (svelte 5.56.4): the same source
//!   compiles fine at the DEFAULT (auto-detected) mode and at `runes: false`, under both
//!   `generate` targets — Svelte's own harness does not force runes, which is why it is
//!   green upstream. It is not tsv's bug, but the sidecar forces `runes: true` (the
//!   oracle is runes-only by design), so it is permanent here until the pin moves.
//!
//!   Pinning it as its own kind keeps it VISIBLE (a line in the snapshot, a paragraph in
//!   its header) instead of a silent skip, and — unlike an exclusion list — it cannot
//!   suppress a *different* future harness error: any other errored file, or a different
//!   failure on this one, is a key the snapshot has never seen and FAILS.
//!
//!   ⚠️ The cost, stated plainly: an errored file gets no oracle verdict, so
//!   [`classify`](super::classify) never probes tsv on it. A pinned `ORACLE-ERROR` file
//!   could therefore be hiding an over-acceptance of its own. That is inherent to "the
//!   oracle cannot speak here", not something the ratchet chooses to ignore.
//!
//! # What is deliberately NOT pinned
//!
//! `refused` and `fenced` counts. A refusal is not a defect — it is the honest "not yet"
//! the whole contract rests on — and its bucket churns with every compiler slice, so
//! pinning it would fail on ordinary forward progress and get the gate turned off. The
//! refusal surface is already reported by the ordinary run and re-priced by `--census`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::audit::ratchet::{Ratchet, SnapshotKey};
use crate::cli::CliError;

use super::{Bucket, FileOutcome, GroupInfo, Report};

/// The corpus the ratchet grades — Svelte's own validation suites, fixed in code.
///
/// Fixed rather than passed in because the snapshot is PATH-keyed: grading a
/// path-keyed snapshot against a different corpus would read every pinned line as stale
/// and every found one as new. A run given explicit positional paths is therefore
/// narrowed (see [`RatchetArgs::narrowing_flags`]) and neither graded nor pinnable.
///
/// Fail-closed either way a checkout can be missing, by two different mechanisms:
///
/// - the root path does **not exist** (no `../svelte` at all) — discovery fails first,
///   with `path not found: …`, before a single file is walked. The run never reaches the
///   ratchet;
/// - the roots exist but are **empty** (a partial/sparse checkout) — zero files are
///   walked, the run does reach the ratchet, and every pinned line grades as STALE.
///
/// Neither passes vacuously; only the second is the wall-of-stale-entries case.
pub(super) const RATCHET_ROOTS: [&str; 2] = [
    "../svelte/packages/svelte/tests/compiler-errors",
    "../svelte/packages/svelte/tests/validator",
];

/// The command that re-pins the snapshot — quoted by the ratchet's read-failure message.
const REPIN_HINT: &str = "deno task compile:validation:update";

/// The `#`-comment header the snapshot opens with. Owned here rather than by the
/// [`Ratchet`] because it documents *this* gate: what a line means, which kinds are
/// pinnable, and why the one `ORACLE-ERROR` line is upstream's bug rather than tsv's.
const SNAPSHOT_HEADER: &str = "# Generated by `deno task compile:validation:update` — do NOT hand-edit.\n\
     #\n\
     # A known-bug RATCHET over Svelte's own validation suites\n\
     # (../svelte/packages/svelte/tests/{compiler-errors,validator}). Every OVER-ACCEPT\n\
     # line is a KNOWN BUG: the oracle REJECTS that component and tsv compiles it anyway,\n\
     # which the refusal contract forbids. The file shrinking is the goal — the gate fails\n\
     # on a line that is NOT here (a new over-acceptance) and on a line here that no longer\n\
     # fires (delete it when you fix one, so the list cannot rot).\n\
     #\n\
     # A MISMATCH is NEVER listed: both sides compiled and the code differs, an absolute\n\
     # bug, so it always fails the gate rather than being pinned.\n\
     #\n\
     # An ORACLE-ERROR line is an UPSTREAM defect, not tsv's. The sidecar forces\n\
     # `runes: true` (the oracle is runes-only by design) and one upstream sample makes\n\
     # svelte's own warning constructor throw under that flag; it compiles fine at the\n\
     # default mode. It is pinned rather than skipped so it stays visible AND so a\n\
     # different future harness error still fails. Caveat: an errored file gets no oracle\n\
     # verdict, so tsv is never probed on it — a pinned one could hide an over-acceptance.\n\
     #\n\
     # ORACLE-ERROR is the ONLY pinnable harness failure. Every other one — a tsv compiler\n\
     # self-check, a tsv parse over-rejection, a canonicalizer failure, an unreadable file,\n\
     # a dead sidecar — is tsv's or the machine's, never upstream's, so it is UNPINNABLE\n\
     # and fails the gate outright rather than appearing here.\n\
     #\n\
     # Refusal / fenced counts are deliberately NOT pinned: a refusal is not a defect, and\n\
     # its buckets churn with every compiler slice.\n\
     #\n\
     # Format: KIND<TAB>ORACLE_CODE<TAB>PATH (path relative to its suite root)\n";

/// Where the committed snapshot lives — colocated with the code that owns it, as with
/// `gap_audit_known.txt` / `blank_audit_known.txt`. Only the path is compile-time; the
/// [`Ratchet`] reads the file at runtime.
pub(super) fn known_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/commands/compile_validation_known.txt")
}

/// The ratchet over [`known_path`], carrying this gate's header + re-pin hint.
fn ratchet() -> Ratchet {
    Ratchet::new(known_path(), SNAPSHOT_HEADER, REPIN_HINT)
}

/// What a snapshot line records. Leads [`RatchetKey`]'s derived [`Ord`], so the file
/// renders grouped by kind.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Kind {
    /// The oracle rejected the source and tsv compiled it anyway — the pinned debt.
    OverAcceptance,
    /// The **oracle itself** threw where it should have rejected (no `svelte.dev/e/`
    /// code) — upstream's bug, and the only harness failure that is pinnable.
    OracleError,
    /// Both compiled, canonical code differs. Never pinnable.
    Mismatch,
    /// Any other harness failure — tsv-side, canonicalizer-side, or environmental.
    /// Never pinnable; see the module docs.
    HarnessError,
}

impl Kind {
    /// Which kind a [`Bucket::Error`]'s `kind` string is.
    ///
    /// The split is *whose bug is it*, and only one answer is "upstream's": the oracle
    /// throwing instead of rejecting. Everything else — a tsv compiler self-check
    /// (`tsv-corrupt-output` / `tsv-type-erasure-leak`), a tsv parser over-rejection
    /// (`tsv-parse`), a canonicalizer failure (`canonicalize-*` / `oracle-recanonicalize`
    /// / `oracle-non-idempotent`), or an environment failure (`read` / `oracle-sidecar`)
    /// — is ours or the machine's, and must fail rather than be pinnable.
    ///
    /// ⚠️ The default is the UNPINNABLE side deliberately: a harness error kind added
    /// later fails the gate until someone decides it belongs upstream, rather than
    /// becoming quietly pinnable.
    fn for_error(kind: &str) -> Self {
        if kind == "oracle-tool" {
            Self::OracleError
        } else {
            Self::HarnessError
        }
    }

    /// Whether a key of this kind may be written into the snapshot.
    ///
    /// A pure function of the enum — never of the error-detail string — so the rule is
    /// checkable by reading three arms, and the string classification happens exactly
    /// once, in [`Self::for_error`].
    fn is_pinnable(self) -> bool {
        matches!(self, Self::OverAcceptance | Self::OracleError)
    }

    fn label(self) -> &'static str {
        match self {
            Self::OverAcceptance => "OVER-ACCEPT",
            Self::OracleError => "ORACLE-ERROR",
            Self::Mismatch => "MISMATCH",
            Self::HarnessError => "HARNESS-ERROR",
        }
    }

    fn from_label(s: &str) -> Option<Self> {
        match s {
            "OVER-ACCEPT" => Some(Self::OverAcceptance),
            "ORACLE-ERROR" => Some(Self::OracleError),
            "MISMATCH" => Some(Self::Mismatch),
            "HARNESS-ERROR" => Some(Self::HarnessError),
            _ => None,
        }
    }
}

/// One snapshot line: a finding at a stable upstream path.
///
/// `code` is part of the key, not decoration. For an over-acceptance it is the oracle's
/// own error code, so a file that starts being rejected by a DIFFERENT rule (an oracle
/// pin bump, or a rewritten upstream sample) reads as one retired line plus one new one
/// and gets re-triaged, rather than silently matching the old entry.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) struct RatchetKey {
    pub(super) kind: Kind,
    pub(super) code: String,
    pub(super) path: String,
}

impl SnapshotKey for RatchetKey {
    fn to_line(&self) -> String {
        format!("{}\t{}\t{}", self.kind.label(), self.code, self.path)
    }

    fn from_line(line: &str) -> Option<Self> {
        let mut cols = line.split('\t');
        let kind = Kind::from_label(cols.next()?)?;
        let code = cols.next()?.to_string();
        let path = cols.next()?.to_string();
        Some(Self { kind, code, path })
    }

    /// A MISMATCH and a HARNESS-ERROR are never pinnable — each breaks an invariant that
    /// is absolute *for tsv*, so `--update` must never launder one into the list whose
    /// shrinking is the goal (and whose header says an errored line is upstream's bug).
    fn is_pinnable(&self) -> bool {
        self.kind.is_pinnable()
    }
}

/// Render a corpus path as its suite-relative key: the root's final component plus the
/// path below it (`validator/samples/foo/input.svelte`).
///
/// Keyed relative to the suite so the snapshot does not encode how the root was spelled
/// on the machine that pinned it. A path that does not sit under its own group root
/// (impossible for a walked file, but not worth panicking over) falls back to verbatim.
fn key_path(root: &str, path: &str) -> String {
    let root_name = Path::new(root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root);
    match path
        .strip_prefix(root)
        .and_then(|rest| rest.strip_prefix('/'))
    {
        Some(rest) => format!("{root_name}/{rest}"),
        None => path.to_string(),
    }
}

/// A TAB-free, single-line projection of an error detail — the snapshot is TAB-delimited
/// and line-oriented, so an un-sanitized detail would split a key across columns/lines.
fn error_code(kind: &str, detail: &str) -> String {
    let detail = detail.replace(['\t', '\n', '\r'], " ");
    let detail = detail.trim();
    if detail.is_empty() {
        kind.to_string()
    } else {
        format!("{kind}: {detail}")
    }
}

/// Every gradable finding in the run, as keys. Parity / refused / plain oracle-rejected
/// outcomes contribute nothing — they are not findings.
pub(super) fn keys(groups: &[GroupInfo], outcomes: &[FileOutcome]) -> BTreeSet<RatchetKey> {
    let mut set = BTreeSet::new();
    for o in outcomes {
        let root = groups[o.group].root.as_str();
        let path = key_path(root, &o.path.display().to_string());
        match &o.bucket {
            Bucket::OracleRejected {
                code,
                tsv_over_accepts: true,
            } => {
                set.insert(RatchetKey {
                    kind: Kind::OverAcceptance,
                    code: code.clone(),
                    path,
                });
            }
            Bucket::Error(kind, detail) => {
                set.insert(RatchetKey {
                    kind: Kind::for_error(kind),
                    code: error_code(kind, detail),
                    path,
                });
            }
            Bucket::Mismatch(_) => {
                set.insert(RatchetKey {
                    kind: Kind::Mismatch,
                    code: "-".to_string(),
                    path,
                });
            }
            Bucket::Parity { .. }
            | Bucket::Refused { .. }
            | Bucket::OracleRejected {
                tsv_over_accepts: false,
                ..
            } => {}
        }
    }
    set
}

/// The ratchet-mode inputs, split out of the command so the narrowing rule is testable
/// without constructing a whole `argh` command (and so it reads as one thing).
pub(super) struct RatchetArgs {
    /// Positional paths the user supplied, if any.
    pub(super) paths: Vec<String>,
}

impl RatchetArgs {
    /// The inputs in effect that make this run something other than the one the snapshot
    /// describes — i.e. not the full [`RATCHET_ROOTS`] corpus.
    ///
    /// Empty ⇒ the run is both gradable against the snapshot and pinnable into it. One
    /// list, two uses (the `--update` refusal and the grade), so the two cannot drift
    /// into disagreeing about what a full run is.
    ///
    /// ⚠️ Anything added that changes WHICH files are compared — a `--limit`, a filter,
    /// a second corpus mode — must be listed here, or `--update` will pin a partial set
    /// and silently unpin real bugs. `every_narrowing_input_disqualifies_a_run` is the
    /// backstop; extend it alongside.
    pub(super) fn narrowing_flags(&self) -> Vec<&'static str> {
        let mut flags = Vec::new();
        if !self.paths.is_empty() {
            flags.push("<paths>");
        }
        flags
    }
}

/// Grade `report`/`outcomes` against the snapshot and print the verdict.
///
/// Returns the gate's exit status; see [`ratchet_verdict`].
pub(super) fn grade_and_report(
    groups: &[GroupInfo],
    outcomes: &[FileOutcome],
    report: &Report,
) -> Result<(), CliError> {
    let found = keys(groups, outcomes);
    let diff = ratchet().grade(&found)?;

    println!("\nVALIDATION RATCHET — {}", known_path().display());
    if diff.holds() {
        println!(
            "  ✓ {} pinned finding(s) reproduced exactly; no new, none stale, no mismatch.",
            diff.known
        );
    } else {
        for key in &diff.new {
            println!("  ✗ NEW      {}", key.to_line());
        }
        for key in &diff.stale {
            println!("  ✗ STALE    {}", key.to_line());
        }
        for key in found.iter().filter(|k| !k.is_pinnable()) {
            println!("  ✗ UNPINNABLE {}", key.to_line());
        }
        if diff.unpinnable > 0 {
            println!(
                "  ✗ {} unpinnable finding(s) — a MISMATCH (both sides compiled, the code \
                 differs) or a HARNESS-ERROR that is tsv's or the machine's, not \
                 upstream's. Neither may be pinned.",
                diff.unpinnable
            );
        }
        println!(
            "\n  A NEW line is a regression. A STALE line is a bug you fixed (or a corpus \
             that moved) — re-pin with `{REPIN_HINT}`."
        );
    }
    ratchet_verdict(report, &diff)
}

/// The ratchet run's exit verdict, as a pure function of the report and the grade.
///
/// Deliberately NOT [`exit_verdict`](super::exit_verdict): that one fails on any
/// over-acceptance, which is the very debt this gate ratchets, so reusing it would make
/// the gate permanently red and useless. Two rules instead:
///
/// - a MISMATCH fails, unconditionally and by name. It is also unpinnable, so the grade
///   below would catch it anyway — the redundancy is deliberate, so that relaxing the
///   pinnability rule can never silently ungate mismatches;
/// - the grade must hold: no new key, no stale key, no unpinnable key.
///
/// A harness `error` is deliberately absent: errors are pinnable keys here (see the
/// module docs), so an expected one must not also trip a blanket error term, while an
/// unexpected one already fails as a NEW key.
///
/// Extracted so it is TESTABLE — the caller is async and needs a live sidecar pool.
fn ratchet_verdict<K>(
    report: &Report,
    diff: &crate::audit::ratchet::GateDiff<K>,
) -> Result<(), CliError> {
    if report.totals.mismatch > 0 || !diff.holds() {
        Err(CliError::Failed)
    } else {
        Ok(())
    }
}

/// The exit verdict for a **narrowed** ratchet run — one given explicit paths, whose
/// finding set is a subset of the snapshot's and so cannot be graded.
///
/// ⭐ **Not-graded is not not-gated.** Skipping the ratchet *comparison* is correct (the
/// found set is truncated, so every unreached line would read as stale), but the
/// absolute terms need no snapshot to be true: a MISMATCH is a compiler bug wherever it
/// is found, and so is an unpinnable HARNESS-ERROR. Returning `Ok(())` for the whole
/// branch made `--ratchet <subtree>` exit 0 on findings that the same paths WITHOUT
/// `--ratchet` exit 1 on — i.e. narrowing was a way to get a green exit on a real bug.
///
/// What it deliberately does NOT gate is the over-acceptance debt. That is the ratcheted
/// balance; a subtree spot-check reaches an arbitrary slice of it and would be
/// permanently red, which is precisely why this gate does not reuse
/// [`exit_verdict`](super::exit_verdict).
fn narrowed_verdict(
    report: &Report,
    found: &BTreeSet<RatchetKey>,
    print: bool,
) -> Result<(), CliError> {
    let unpinnable: Vec<&RatchetKey> = found.iter().filter(|k| !k.is_pinnable()).collect();
    if report.totals.mismatch == 0 && unpinnable.is_empty() {
        return Ok(());
    }
    if print {
        for key in &unpinnable {
            println!("  ✗ UNPINNABLE {}", key.to_line());
        }
        println!(
            "  ✗ this narrowed run is not GRADED, but a MISMATCH and a HARNESS-ERROR are \
             absolute and gate anyway."
        );
    }
    Err(CliError::Failed)
}

/// Report a narrowed run and return its verdict. The narrowing notice is printed here so
/// the skip and the still-live gate read as one message.
pub(super) fn report_narrowed(
    groups: &[GroupInfo],
    outcomes: &[FileOutcome],
    report: &Report,
    narrowed: &[&'static str],
) -> Result<(), CliError> {
    println!(
        "\nVALIDATION RATCHET — not graded: this run is narrowed by {}, so its finding \
         set is a SUBSET of the snapshot's. Re-run without it to grade.",
        narrowed.join(" / ")
    );
    narrowed_verdict(report, &keys(groups, outcomes), true)
}

/// Re-pin the snapshot from a full run's findings.
///
/// The caller has already refused a narrowed run (see [`RatchetArgs::narrowing_flags`]),
/// so `found` is the whole key set the snapshot means. A MISMATCH is filtered out by the
/// [`Ratchet`] itself and reported here, since pinning cannot absorb it.
pub(super) fn update(groups: &[GroupInfo], outcomes: &[FileOutcome]) -> Result<(), CliError> {
    let previous: BTreeSet<RatchetKey> = if known_path().exists() {
        ratchet().parse_known().unwrap_or_default()
    } else {
        BTreeSet::new()
    };
    let found = keys(groups, outcomes);
    ratchet().write(&found)?;

    let pinned: BTreeSet<RatchetKey> = found.iter().filter(|k| k.is_pinnable()).cloned().collect();
    println!(
        "✓ wrote {} finding(s) to {}",
        pinned.len(),
        known_path().display()
    );
    // Counted per KIND, not over the whole pinned set: an ORACLE-ERROR line retiring is
    // not an over-acceptance win, and folding the two made the yield line miscount the
    // number the label names.
    let over = |s: &BTreeSet<RatchetKey>| -> BTreeSet<RatchetKey> {
        s.iter()
            .filter(|k| k.kind == Kind::OverAcceptance)
            .cloned()
            .collect()
    };
    let (prev_over, now_over) = (over(&previous), over(&pinned));
    let retired = prev_over.difference(&now_over).count();
    let added = now_over.difference(&prev_over).count();
    println!(
        "  yield: over-acceptances −{retired} +{added} (net {:+})",
        added as isize - retired as isize
    );
    let other_retired = previous.len() - prev_over.len();
    let other_now = pinned.len() - now_over.len();
    if other_retired != other_now {
        println!("  other pinned kinds: {other_retired} → {other_now}");
    }

    refuse_unpinnable(&found)
}

/// Report the findings `--update` could not pin, and fail if there were any.
///
/// Split out of [`update`] so the refusal is testable: `update` writes to the committed
/// snapshot before it reaches this point, so a test that drove it end to end would
/// clobber the real `compile_validation_known.txt`.
fn refuse_unpinnable(found: &BTreeSet<RatchetKey>) -> Result<(), CliError> {
    let unpinnable: Vec<&RatchetKey> = found.iter().filter(|k| !k.is_pinnable()).collect();
    if unpinnable.is_empty() {
        return Ok(());
    }
    eprintln!(
        "\n✗ {} finding(s) were NOT pinned — a MISMATCH (both sides compiled and the \
         canonical code differs) and a HARNESS-ERROR (tsv's or the machine's, not \
         upstream's) are both unpinnable, so the gate will keep failing until they are \
         fixed:",
        unpinnable.len()
    );
    for key in unpinnable {
        eprintln!("    {}", key.to_line());
    }
    Err(CliError::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::ratchet::GateDiff;

    fn outcome(group: usize, path: &str, bucket: Bucket) -> FileOutcome {
        FileOutcome {
            group,
            path: PathBuf::from(path),
            bucket,
        }
    }

    fn groups() -> Vec<GroupInfo> {
        RATCHET_ROOTS
            .iter()
            .map(|root| GroupInfo {
                root: (*root).to_string(),
                file_count: 0,
            })
            .collect()
    }

    /// Only findings become keys, and each carries its suite-relative path.
    #[test]
    fn keys_cover_findings_and_nothing_else() {
        let g = groups();
        let found = keys(
            &g,
            &[
                outcome(
                    1,
                    "../svelte/packages/svelte/tests/validator/samples/a/input.svelte",
                    Bucket::OracleRejected {
                        code: "constant_assignment".to_string(),
                        tsv_over_accepts: true,
                    },
                ),
                // A plain oracle rejection is the expected case, not a finding.
                outcome(
                    1,
                    "../svelte/packages/svelte/tests/validator/samples/b/input.svelte",
                    Bucket::OracleRejected {
                        code: "constant_assignment".to_string(),
                        tsv_over_accepts: false,
                    },
                ),
                outcome(
                    0,
                    "../svelte/packages/svelte/tests/compiler-errors/samples/c/input.svelte",
                    Bucket::Parity { tolerated: false },
                ),
                outcome(
                    0,
                    "../svelte/packages/svelte/tests/compiler-errors/samples/d/input.svelte",
                    Bucket::Refused {
                        reason: "css at-rule in <style>".to_string(),
                        fenced: false,
                    },
                ),
            ],
        );
        let lines: Vec<String> = found.iter().map(SnapshotKey::to_line).collect();
        assert_eq!(
            lines,
            vec!["OVER-ACCEPT\tconstant_assignment\tvalidator/samples/a/input.svelte"]
        );
    }

    /// A MISMATCH is a key (so it is counted and fails) but never a pinned line.
    ///
    /// The corpus cannot grade this: the suites produce zero mismatches today, so both
    /// arms are vacuously green there and would stay green if the filter were dropped.
    #[test]
    fn a_mismatch_is_never_pinned() {
        let g = groups();
        let found = keys(
            &g,
            &[
                outcome(
                    1,
                    "../svelte/packages/svelte/tests/validator/samples/a/input.svelte",
                    Bucket::OracleRejected {
                        code: "constant_assignment".to_string(),
                        tsv_over_accepts: true,
                    },
                ),
                outcome(
                    1,
                    "../svelte/packages/svelte/tests/validator/samples/m/input.svelte",
                    Bucket::Mismatch("diff".to_string()),
                ),
            ],
        );
        let r = Ratchet::new(PathBuf::from("/unused"), SNAPSHOT_HEADER, REPIN_HINT);
        let rendered = r.render(&found);
        let data: Vec<&str> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(
            data,
            vec!["OVER-ACCEPT\tconstant_assignment\tvalidator/samples/a/input.svelte"],
            "only the pinnable finding is written"
        );
        assert_eq!(found.iter().filter(|k| !k.is_pinnable()).count(), 1);
    }

    /// A tsv-side (or environmental) harness failure must be UNPINNABLE, while the
    /// oracle's own throw stays pinnable — the two share one `Bucket::Error` variant, so
    /// only the error KIND string tells them apart.
    ///
    /// Without this split every `Bucket::Error` mapped to the pinnable `ORACLE-ERROR`,
    /// so a `tsv-corrupt-output` — a compiler bug that fails its run everywhere else in
    /// this repo — could be laundered by the next `--update` into a list whose header
    /// tells the reader an errored line is UPSTREAM's bug.
    ///
    /// The corpus cannot grade this: the suites produce exactly one harness error today
    /// (the pinned upstream `oracle-tool` throw), so every tsv-side arm is vacuously
    /// green there and would stay green if the classification were dropped.
    #[test]
    fn a_tsv_side_harness_error_is_never_pinned() {
        let g = groups();
        let found = keys(
            &g,
            &[
                outcome(
                    1,
                    "../svelte/packages/svelte/tests/validator/samples/oracle/input.svelte",
                    Bucket::Error(
                        "oracle-tool",
                        "An impossible situation occurred".to_string(),
                    ),
                ),
                outcome(
                    1,
                    "../svelte/packages/svelte/tests/validator/samples/corrupt/input.svelte",
                    Bucket::Error("tsv-corrupt-output", "unparseable JS".to_string()),
                ),
            ],
        );

        // Only the oracle's own throw reaches the file.
        let r = Ratchet::new(PathBuf::from("/unused"), SNAPSHOT_HEADER, REPIN_HINT);
        let rendered = r.render(&found);
        let data: Vec<&str> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(
            data,
            vec![
                "ORACLE-ERROR\toracle-tool: An impossible situation occurred\t\
                 validator/samples/oracle/input.svelte"
            ],
            "a tsv-side harness error must never be written into the snapshot"
        );

        // And `--update` refuses the run rather than silently dropping it.
        assert!(matches!(refuse_unpinnable(&found), Err(CliError::Failed)));

        // Every non-`oracle-tool` kind the classify path can emit is unpinnable; the
        // list mirrors `Bucket::Error(...)` call sites in the parent module.
        for kind in [
            "read",
            "oracle-sidecar",
            "tsv-parse",
            "tsv-corrupt-output",
            "tsv-type-erasure-leak",
            "tsv-generated-name-missing",
            "canonicalize-oracle",
            "oracle-non-idempotent",
            "oracle-recanonicalize",
            "canonicalize-ours",
        ] {
            assert_eq!(
                Kind::for_error(kind),
                Kind::HarnessError,
                "{kind} must not be pinnable"
            );
            assert!(!Kind::for_error(kind).is_pinnable());
        }
        assert_eq!(Kind::for_error("oracle-tool"), Kind::OracleError);
        assert!(Kind::for_error("oracle-tool").is_pinnable());
    }

    /// `--update` must refuse a run carrying an unpinnable finding, and say so.
    ///
    /// The refusal is split out of [`update`] precisely so it can be driven here:
    /// `update` writes the committed snapshot before reaching it, so an end-to-end test
    /// would clobber the real `compile_validation_known.txt`.
    #[test]
    fn update_refuses_to_leave_an_unpinnable_finding_unreported() {
        let clean: BTreeSet<RatchetKey> = std::iter::once(RatchetKey {
            kind: Kind::OverAcceptance,
            code: "constant_assignment".to_string(),
            path: "validator/samples/a/input.svelte".to_string(),
        })
        .collect();
        assert!(refuse_unpinnable(&clean).is_ok());

        for kind in [Kind::Mismatch, Kind::HarnessError] {
            let mut found = clean.clone();
            found.insert(RatchetKey {
                kind,
                code: "-".to_string(),
                path: "validator/samples/m/input.svelte".to_string(),
            });
            assert!(
                matches!(refuse_unpinnable(&found), Err(CliError::Failed)),
                "{kind:?} must fail `--update`"
            );
        }
    }

    /// ⭐ NOT GRADED IS NOT UN-GATED. A narrowed run skips the ratchet COMPARISON (its
    /// found set is a subset, so every unreached line would read as stale) — but the
    /// absolute terms need no snapshot and must still fire.
    ///
    /// Regression: the narrowed branch returned `Ok(())` outright, so
    /// `--ratchet <subtree>` exited 0 on a MISMATCH that the same paths WITHOUT
    /// `--ratchet` exit 1 on — narrowing was a way to get a green exit on a real bug.
    ///
    /// Unit-tested on the verdict rather than driven end to end: a live narrowed run
    /// needs the sidecar, and the suites produce no mismatch to drive it with.
    #[test]
    fn a_narrowed_run_still_gates_the_absolute_terms() {
        let g = [GroupInfo {
            root: "r".to_string(),
            file_count: 1,
        }];
        let over = |path: &str| {
            outcome(
                0,
                path,
                Bucket::OracleRejected {
                    code: "constant_assignment".to_string(),
                    tsv_over_accepts: true,
                },
            )
        };

        // The ratcheted debt alone is NOT gated on a narrowed run: a subtree reaches an
        // arbitrary slice of it, so gating it would be permanently red.
        let debt = [over("a.svelte")];
        let debt_report = Report::build(&g, &debt);
        assert_eq!(debt_report.over_acceptance_total(), 1);
        assert!(
            super::super::exit_verdict(&debt_report).is_err(),
            "precondition: the ordinary verdict does fail on the debt"
        );
        assert!(narrowed_verdict(&debt_report, &keys(&g, &debt), false).is_ok());

        // A MISMATCH gates — by the report total AND as an unpinnable key.
        let mismatch = [outcome(0, "m.svelte", Bucket::Mismatch("d".into()))];
        let mismatch_report = Report::build(&g, &mismatch);
        assert_eq!(mismatch_report.totals.mismatch, 1);
        assert!(matches!(
            narrowed_verdict(&mismatch_report, &keys(&g, &mismatch), false),
            Err(CliError::Failed)
        ));

        // So does an unpinnable HARNESS-ERROR, which the report totals do NOT carry —
        // the key set is what sees it.
        let harness = [outcome(
            0,
            "c.svelte",
            Bucket::Error("tsv-corrupt-output", "boom".to_string()),
        )];
        let harness_report = Report::build(&g, &harness);
        assert_eq!(harness_report.totals.mismatch, 0);
        assert!(matches!(
            narrowed_verdict(&harness_report, &keys(&g, &harness), false),
            Err(CliError::Failed)
        ));

        // A pinnable ORACLE-ERROR does not gate a narrowed run: it is the upstream line
        // the full gate pins, so failing on it would make every spot-check red.
        let oracle = [outcome(
            0,
            "o.svelte",
            Bucket::Error("oracle-tool", "impossible".to_string()),
        )];
        let oracle_report = Report::build(&g, &oracle);
        assert!(narrowed_verdict(&oracle_report, &keys(&g, &oracle), false).is_ok());
    }

    /// render → parse must round-trip, and the file must group by KIND-enum order
    /// (`OVER-ACCEPT` before `ORACLE-ERROR`), which is NOT label-string order
    /// (`'O','R'` vs `'O','V'` would put ORACLE-ERROR first). Two facts carry that:
    /// `kind` is the [`Kind`] enum, and it is the FIRST field of the derived `Ord`.
    #[test]
    fn render_and_parse_round_trip_in_kind_enum_order() {
        let found: BTreeSet<RatchetKey> = [
            RatchetKey {
                kind: Kind::OracleError,
                code: "oracle-tool: An impossible situation occurred".to_string(),
                path: "validator/samples/silence-warnings-2/input.svelte".to_string(),
            },
            RatchetKey {
                kind: Kind::OverAcceptance,
                code: "constant_assignment".to_string(),
                path: "validator/samples/a/input.svelte".to_string(),
            },
        ]
        .into_iter()
        .collect();
        let r = Ratchet::new(PathBuf::from("/unused"), SNAPSHOT_HEADER, REPIN_HINT);
        let rendered = r.render(&found);
        let data: Vec<&str> = rendered
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(
            data,
            vec![
                "OVER-ACCEPT\tconstant_assignment\tvalidator/samples/a/input.svelte",
                "ORACLE-ERROR\toracle-tool: An impossible situation occurred\t\
                 validator/samples/silence-warnings-2/input.svelte",
            ]
        );
        let parsed: BTreeSet<RatchetKey> = data
            .iter()
            .filter_map(|l| RatchetKey::from_line(l))
            .collect();
        assert_eq!(parsed, found, "render → parse must round-trip");
    }

    /// An error detail must never carry a TAB or a newline into the TAB-delimited,
    /// line-oriented snapshot — either would split one key across columns or lines, and
    /// the key would silently stop round-tripping.
    #[test]
    fn error_detail_is_sanitized_into_one_column() {
        let code = error_code("oracle-tool", "boom\there\nand more\r");
        assert_eq!(code, "oracle-tool: boom here and more");
        assert!(!code.contains('\t') && !code.contains('\n'));
        // An empty detail degrades to the bare kind rather than a dangling separator.
        assert_eq!(error_code("read", "   "), "read");
    }

    /// The narrowing list decides BOTH whether `--update` may write and whether the run
    /// is graded, so a narrowing input missing from it pins a set that isn't the one the
    /// snapshot means. Every input that changes which files are compared gets its own
    /// named assertion here.
    #[test]
    fn every_narrowing_input_disqualifies_a_run() {
        let full = RatchetArgs { paths: Vec::new() };
        assert!(
            full.narrowing_flags().is_empty(),
            "the default run is the one the snapshot describes"
        );
        let narrowed = RatchetArgs {
            paths: vec!["../svelte/packages/svelte/tests/validator".to_string()],
        };
        assert_eq!(narrowed.narrowing_flags(), vec!["<paths>"]);
    }

    /// The verdict must gate on the RATCHET, not on the raw over-acceptance count —
    /// and must still fail a mismatch unconditionally.
    #[test]
    fn ratchet_verdict_gates_the_grade_not_the_debt() {
        let g = [GroupInfo {
            root: "r".to_string(),
            file_count: 1,
        }];
        let holds: GateDiff<RatchetKey> = GateDiff {
            known: 49,
            new: Vec::new(),
            stale: Vec::new(),
            unpinnable: 0,
        };

        // 49 over-acceptances, all pinned ⇒ GREEN. `exit_verdict` would fail here, which
        // is exactly why this gate has its own verdict.
        let debt = Report::build(
            &g,
            &[outcome(
                0,
                "a.svelte",
                Bucket::OracleRejected {
                    code: "constant_assignment".to_string(),
                    tsv_over_accepts: true,
                },
            )],
        );
        assert_eq!(debt.over_acceptance_total(), 1);
        assert!(super::super::exit_verdict(&debt).is_err(), "precondition");
        assert!(ratchet_verdict(&debt, &holds).is_ok());

        // A new key fails.
        let new: GateDiff<RatchetKey> = GateDiff {
            known: 49,
            new: vec![RatchetKey {
                kind: Kind::OverAcceptance,
                code: "x".to_string(),
                path: "p".to_string(),
            }],
            stale: Vec::new(),
            unpinnable: 0,
        };
        assert!(matches!(
            ratchet_verdict(&debt, &new),
            Err(CliError::Failed)
        ));

        // A stale key fails.
        let stale: GateDiff<RatchetKey> = GateDiff {
            known: 49,
            new: Vec::new(),
            stale: vec![RatchetKey {
                kind: Kind::OverAcceptance,
                code: "x".to_string(),
                path: "p".to_string(),
            }],
            unpinnable: 0,
        };
        assert!(matches!(
            ratchet_verdict(&debt, &stale),
            Err(CliError::Failed)
        ));

        // A mismatch fails by its own name, even with a holding grade — the redundancy
        // that keeps mismatches gated if the pinnability rule is ever relaxed.
        let mismatch = Report::build(&g, &[outcome(0, "m.svelte", Bucket::Mismatch("d".into()))]);
        assert_eq!(mismatch.totals.mismatch, 1);
        assert!(matches!(
            ratchet_verdict(&mismatch, &holds),
            Err(CliError::Failed)
        ));
    }
}
