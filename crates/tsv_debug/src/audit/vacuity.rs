//! The **vacuity guard** — the two checks that stop a gate over nothing from
//! reading as a green gate.
//!
//! Every audit's happy path prints a `✓` and exits 0, and so does an audit that
//! graded nothing at all: an empty walk, a corpus the parser rejects end to end,
//! a fixture tree that silently shrank. The two layers here answer that at the
//! two scopes a run can have:
//!
//! - [`check_graded_nonzero`] — **scope-relative**, called unconditionally by
//!   every corpus-walking audit. Zero graded is vacuous whatever the paths, so it
//!   needs no pin.
//! - [`check_formatted_min`] + [`FIXTURES_FORMATTED_MIN`] — **default-corpus**,
//!   called only on a full default run. It grades a *shrink*, which only a run
//!   over the committed fixtures tree can be held to.
//!
//! Its own module rather than [`super::sweep`]'s (where it was born) because the
//! floor is a question about an audit's DENOMINATOR, not about that loop: seven
//! of its callers — `canonicalize`, `binding`, `neutrality`, `roundtrip`,
//! `authoring`, `render`, `fuzz` — drive no sweep at all. The pin above it is
//! sweep-shaped by coincidence of subject (its consumers all count formatted
//! files); the two belong together because they are two answers to one question.

use crate::cli::CliError;

/// REGRESSION PIN (minimum, at the exact measured value): files a default
/// (`tests/fixtures`) run formats. **One corpus, one pin** — every consumer
/// resolves the same seed list (`resolve_seed_files`) and skips the same three
/// classes (`input_invalid_*`, a parse rejection, a panic), so their counts are
/// equal by construction, not by coincidence — so all five [`super::sweep`]
/// consumers pass it, including `comment_audit`, which adds its own
/// `REGISTERED_MIN` over the same walk for the collapse a file count cannot see.
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

/// The **default-corpus** half of the vacuity guard: a default run that formatted
/// fewer than `min` files is not a passing gate, it is a collapsed corpus. It grades
/// a *shrink*, which only a default run can be held to — so every consumer gates it
/// behind `default_paths`, and [`check_graded_nonzero`] is the floor underneath that
/// no scope escapes.
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

/// The **scope-relative** half of the vacuity guard: an audit that graded nothing
/// proved nothing, whatever its scope. `subject` names what was counted ("files
/// formatted", "TypeScript-family files compared", "boundary sites probed").
///
/// ⚠️ **`graded` is the count the audit's VERDICT rests on, not the count its
/// resolution returned.** Seed resolution already refuses an empty walk
/// (`resolve_seed_files_named`), so passing it the file list back re-asks a
/// question already answered; the collapse this catches lives *below* a healthy
/// file list — every file parse-failing, every render skipped as compile-blind,
/// every seed unreadable. So each consumer passes its own denominator, and the
/// rule for choosing it is: count the outcomes that carry a VERDICT, exclude only
/// the ones that could not be evaluated. A trivially-clean outcome is a verdict (a
/// no-op format renders identically by identity), so it counts; "the parser
/// rejected it" is not, so it does not.
///
/// Complements [`check_formatted_min`], the **default-corpus** half: that one pins
/// a number only the committed fixtures tree can meet, so every consumer gates it
/// behind `default_paths` — which leaves every explicitly-pathed run (each corpus
/// sweep, each ad-hoc triage run, and `canonicalize:audit`, the one `deno task
/// check` leg invoked with paths) relying on this floor alone. Zero is the one
/// value that is vacuous at *every* scope, so this check needs no pin and is called
/// unconditionally.
///
/// # Errors
///
/// Returns [`CliError::Failed`] (after a user-facing message) when `graded` is 0.
pub(crate) fn check_graded_nonzero(graded: usize, subject: &str) -> Result<(), CliError> {
    if graded > 0 {
        return Ok(());
    }
    eprintln!(
        "Error: vacuous run — 0 {subject}. A gate over nothing proves nothing: check the \
         paths, and that the corpus still parses."
    );
    Err(CliError::Failed)
}
