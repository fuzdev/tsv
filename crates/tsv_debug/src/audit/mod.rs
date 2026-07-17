//! The **audit substrate** — the reusable machinery an audit command builds on.
//!
//! A tsv audit takes a corpus of sources, checks a property of each, and grades
//! the findings against a policy. The pieces that recur across audits live here so
//! a new audit instantiates them rather than copying them:
//!
//! - [`ratchet`] — the "every line is a known bug, the file shrinking is the goal"
//!   snapshot gate (the shape a large, churny finding set needs instead of a count).
//! - [`sites`] — site enumeration + file-independent shape keying for a
//!   gap-injection audit.
//! - [`properties`] — the per-input property layer: the panic-safe ledger format
//!   and verify verdicts, plus the shared parse-to-wire / reparse-skeleton
//!   primitives.
//! - [`report`] — the shared reporting envelope (`{severity, confidence, site,
//!   example, detail}`) and the human / JSON printers.
//!
//! `gap_audit` is the first and (today) only consumer; the modules are written
//! generic where a second audit would reuse them, and no further.

// Always compiled: `properties` hosts the reparse primitives the `roundtrip_audit`
// / `fuzz` commands share, which are not behind `comment_check`. (Its ledger /
// verify layer is internally feature-gated.)
pub(crate) mod properties;

// The gap-injection machinery is only reachable through `gap_audit`, which is
// itself behind the `comment_check` feature (it arms `tsv_lang::comment_ledger`),
// so gate these too — otherwise they read as dead code in a default build.
#[cfg(feature = "comment_check")]
pub(crate) mod ratchet;
#[cfg(feature = "comment_check")]
pub(crate) mod report;
#[cfg(feature = "comment_check")]
pub(crate) mod sites;
