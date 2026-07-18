//! The shared reporting **envelope** for the audit substrate — one schema that
//! carries any audit's findings to the human / JSON printers.
//!
//! A finding is `{ audit, severity, confidence, site, example, verdict_string,
//! detail }`:
//!
//! - **severity** ([`Severity`]) — `GateFailing` (an absolute break the gate
//!   fails on regardless of any ratchet) vs `Informational` (a finding whose
//!   fatality the run decides elsewhere — e.g. via the [`ratchet`](crate::audit::ratchet)).
//! - **confidence** ([`Confidence`], optional) — the "did the observation
//!   reproduce" axis: gap's verify pass generalizes here (`Confirmed` /
//!   `Partial` / `Unconfirmed`).
//! - **site / example** — where the finding is, and a by-hand reproducer.
//! - **verdict_string** — the human one-line verdict suffix.
//! - **detail** ([`Detail`]) — the audit-specific payload, carried **verbatim**
//!   and rendered by the audit's own arm. Enum-dispatched, one variant per audit;
//!   [`gap_audit`](crate::cli::commands) ([`Detail::Gap`]) is the only one today.
//!
//! `gap_audit` is the only consumer, so the envelope is concrete (no generics /
//! `dyn`), but the skeleton is audit-agnostic and the detail slot is where each
//! audit's own vocabulary lives.
//!
//! ## Validation — does the schema hold other audits' vocabularies?
//!
//! Sketched here (not migrated) to prove `{skeleton + detail}` doesn't flatten a
//! load-bearing distinction:
//!
//! - **`roundtrip_audit`** — 7 buckets (`clean`, `format_error`,
//!   `canonical_rejects_input`, `{canonical,tsv}_unreparseable`,
//!   `{canonical,tsv}_divergent`). The two `*_unreparseable` (the reliable half,
//!   `--gate`-fatal) → `severity: GateFailing`; the two `*_divergent`
//!   (render-model noise, informational under `--gate`) → `Informational`;
//!   `clean` / `format_error` / `*_rejects_input` are non-findings, not emitted.
//!   The two-phase oracle → `confidence`: a canonical-confirmed finding is
//!   `Confirmed`, a tsv-self-only suspect (canonical didn't run) `Unconfirmed`.
//!   The exact bucket label rides `detail` verbatim, so the
//!   divergent-vs-unreparseable distinction survives in *both* severity and
//!   detail — no flattening. Site = the file; the AST diff is the reproducer.
//! - **`authoring_audit`** — the pure-Rust 3-way (converge / diverge-dual-stable /
//!   diverge-NON-IDEMPOTENT) plus the `--prettier` bug/pin/sanctioned 2×2.
//!   NON-IDEMPOTENT / a real bug → `GateFailing`; a `_prettier_divergence` to pin
//!   and a sanctioned divergence → `Informational`. `confidence` carries whether
//!   the prettier triage ran (`Confirmed`) or only the pure-Rust verdict is in
//!   hand (`Unconfirmed`). The bug/pin/sanctioned **classification** is audit-
//!   specific and rides `detail` — it is *not* a confidence, so it does not
//!   collide with that axis. No flattening.
//! - **`binding_audit`** — HARD (a parser-owned glued comment re-binds, `--gate`-
//!   fatal) → `GateFailing`; SOFT (an unowned glued block comment, informational)
//!   → `Informational`. A direct map — the whole audit is a severity split. The
//!   in→out bound subtree rides `detail`.
//!
//! **Where it would widen** (checked against every audit command, not just the
//! three sketched):
//!
//! - [`Finding::example`] is a per-offset injection reproducer. Roundtrip /
//!   binding / swallow findings are file-level, and a fuzz reproducer is a whole
//!   mutant input (or seed + iteration) — so the real widening is
//!   reproducer-*shape*, not merely `Option`. A reproducer-shape generalization,
//!   not a flattened distinction — noted rather than done, since gap always has
//!   an example.
//! - [`RunSummary`] is injection-shaped (`sites` / `injections` / `accepted` /
//!   `payload_labels`). The three audits sketched above happen to fit it, but
//!   `comment_audit`'s run level carries `registered` and `unregistered_emits`,
//!   and `fuzz` carries its pristine-reflow path aggregate — none has a slot. A
//!   migration gives the run level its own per-audit detail slot, exactly as
//!   [`Finding`] already has one.
//!
//! `build_fanout_audit` (synthetic growth curves) and `scan_audit` (a static
//! source lint over an allow-list) produce no corpus findings at all — outside
//! the envelope's scope by shape, not flattened by it.

use serde_json::Value;

/// How the gate treats a finding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Severity {
    /// An absolute invariant break — fails the gate on its own, regardless of any
    /// ratchet (gap's `PANIC`: a comment in a gap must never crash the formatter).
    GateFailing,
    /// A finding whose fatality the run decides elsewhere — for gap, whether the
    /// ratchet has seen its shape.
    Informational,
}

impl Severity {
    /// The `--json` label — the scriptable-triage key.
    fn label(self) -> &'static str {
        match self {
            Self::GateFailing => "gate-failing",
            Self::Informational => "informational",
        }
    }
}

/// The "did the observation reproduce" axis — gap's verify pass, generalized.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Confidence {
    /// The finding reproduced (every kept example, for gap).
    Confirmed,
    /// It reproduced for some kept examples but not others.
    Partial,
    /// No kept example reproduced — likely an instrument artifact, not the plain
    /// finding it is filed as.
    Unconfirmed,
}

/// A by-hand reproducer for a finding — everything needed to re-create it.
pub(crate) struct ReportExample {
    pub(crate) payload: &'static str,
    pub(crate) path: String,
    /// The byte offset the payload was **injected** at — the splice that reproduces the
    /// finding. Equals `attribution_offset` for an injected hit.
    pub(crate) injection_offset: usize,
    /// The byte offset the finding is **attributed** to: the victim comment's own site for a
    /// bystander, the injection site for the injected comment. Keys the shape + snippet.
    pub(crate) attribution_offset: usize,
    pub(crate) snippet: String,
    pub(crate) text: String,
    pub(crate) injected: bool,
}

/// `gap_audit`'s audit-specific detail — the per-shape aggregate the envelope
/// carries verbatim.
pub(crate) struct GapDetail {
    /// The verbatim finding-kind label — `DROPPED` / `DOUBLE-PRINTED` / `PANIC`.
    pub(crate) kind_label: &'static str,
    /// How many injections hit this shape.
    pub(crate) count: usize,
    /// Distinct seed files the shape fired in.
    pub(crate) files: usize,
    /// Which payloads reached this shape.
    pub(crate) payloads: Vec<&'static str>,
    /// Hits where the offending comment is a bystander (not the injected one).
    pub(crate) bystander_hits: usize,
    /// The verify pass's `confirmed` / `total`, for the JSON view (`None` when
    /// the shape was not verified).
    pub(crate) verify_confirmed: Option<usize>,
    pub(crate) verify_total: Option<usize>,
}

/// The audit-specific detail slot — one variant per audit (enum-dispatch, the
/// fuz-stack idiom over `dyn`). Adding an audit adds a variant and a printer arm.
pub(crate) enum Detail {
    Gap(GapDetail),
}

/// One finding in the shared envelope.
pub(crate) struct Finding {
    pub(crate) audit: &'static str,
    pub(crate) severity: Severity,
    pub(crate) confidence: Option<Confidence>,
    /// The site key (gap: the abstract [`site_shape`](crate::audit::sites::site_shape)).
    pub(crate) site: String,
    pub(crate) example: ReportExample,
    /// The human one-line verdict suffix (gap: the verify report label, or empty).
    pub(crate) verdict_string: String,
    pub(crate) detail: Detail,
}

impl Finding {
    /// The per-shape instance count, read out of the audit-specific detail — the
    /// worst-first sort key of the human report.
    fn count(&self) -> usize {
        let Detail::Gap(d) = &self.detail;
        d.count
    }
}

/// Run-level totals — the header line and the JSON envelope's top level.
pub(crate) struct RunSummary {
    pub(crate) audit: &'static str,
    pub(crate) files_done: usize,
    pub(crate) sites: usize,
    pub(crate) injections: usize,
    pub(crate) accepted: usize,
    pub(crate) parse_skipped: usize,
    /// Files already non-clean before injection — reported, never injected into.
    pub(crate) dirty_files: Vec<String>,
    pub(crate) payload_labels: Vec<&'static str>,
}

/// The header every report opens with — totals, then any file that was
/// **skipped**.
///
/// The skip notice lives here, not in [`print_report`], because it is a statement
/// about COVERAGE, not a finding: a dirty file is one the audit never probed. Quiet
/// modes may drop findings the snapshot already pins; they must never drop the fact
/// that a file went unprobed, or a shrinking corpus reads as a passing gate.
fn print_header(s: &RunSummary) {
    println!(
        "{} — {} files · {} sites · {} injections ({} accepted) · payloads: {}\n",
        s.audit,
        s.files_done,
        s.sites,
        s.injections,
        s.accepted,
        s.payload_labels.join(", ")
    );

    if !s.dirty_files.is_empty() {
        println!(
            "○ {} file(s) already had ledger findings AS AUTHORED — reported by \
             `comments:audit`, not injected into here:",
            s.dirty_files.len()
        );
        for p in s.dirty_files.iter().take(10) {
            println!("    {p}");
        }
        if s.dirty_files.len() > 10 {
            println!("    … and {} more", s.dirty_files.len() - 10);
        }
        println!();
    }
}

/// What a run with nothing to act on prints: the header, the totals, and nothing
/// else.
///
/// The per-shape report is for shapes you might *do* something about. When the
/// ratchet holds, every one is already pinned — printing them all buries the `✓`
/// under thousands of lines. `--report` brings them back.
pub(crate) fn print_summary(s: &RunSummary, findings: &[Finding]) {
    print_header(s);
    let total: usize = findings.iter().map(Finding::count).sum();
    println!(
        "○ {total} finding(s) across {} known site shape(s) — all pinned; re-run with \
         --report for the per-shape detail",
        findings.len()
    );
}

pub(crate) fn print_report(s: &RunSummary, findings: &[Finding]) {
    print_header(s);

    if findings.is_empty() {
        println!(
            "✓ every injected comment printed exactly once — no gap drops a comment across \
             {} injections",
            s.accepted
        );
        return;
    }

    let total: usize = findings.iter().map(Finding::count).sum();
    println!(
        "✗ {total} finding(s) across {} distinct site shape(s)\n",
        findings.len()
    );

    // Worst-first: a shape firing everywhere is one bug on a hot path, and fixing it
    // collapses the whole list. `findings` arrives in (kind, shape) order, and the
    // sort is stable, so ties keep that order — matching the old explicit
    // `.then((kind, shape))` tie-break.
    let mut rows: Vec<&Finding> = findings.iter().collect();
    rows.sort_by_key(|f| std::cmp::Reverse(f.count()));

    for f in &rows {
        let Detail::Gap(d) = &f.detail;
        println!(
            "  {:>7}×  {:<14} {}{}",
            d.count, d.kind_label, f.site, f.verdict_string
        );
        println!(
            "            {} file(s) · payloads: {}{}",
            d.files,
            d.payloads.join(", "),
            match d.bystander_hits {
                0 => String::new(),
                n if n == d.count => "  (ALL hits knock out a BYSTANDER comment)".to_string(),
                n => format!("  ({n} of {} hits knock out a bystander)", d.count),
            }
        );
        let ex = &f.example;
        if ex.injected {
            // Injection site and victim site coincide — one offset reproduces and locates it.
            println!(
                "            e.g. inject {} at {}:{}  {}",
                ex.payload, ex.path, ex.injection_offset, ex.snippet
            );
        } else {
            // A bystander: injecting at one site drops a DIFFERENT comment. Show both — the
            // injection offset reproduces the drop, the attribution offset (and the snippet)
            // is where the victim comment lived, which is what the shape keys on.
            println!(
                "            e.g. inject {} at {}:{} → drops the comment at :{}  {}",
                ex.payload, ex.path, ex.injection_offset, ex.attribution_offset, ex.snippet
            );
        }
        println!("            comment: {:?}", ex.text);
        println!();
    }

    let unconfirmed = count_confidence(findings, Confidence::Unconfirmed);
    let partial = count_confidence(findings, Confidence::Partial);
    if unconfirmed > 0 || partial > 0 {
        println!(
            "⚠ {unconfirmed} shape(s) UNCONFIRMED (no kept example reproduced) and {partial} \
             PARTIAL (some did): the ledger says a comment was\n  never emitted, yet the output \
             reparses to just as many comments as its input. Something\n  printed it without \
             recording the emit — or printed a MANGLED rebuild (`/* a⏎b */` →\n  `/* ab */`, one \
             comment either way). Real either way, but not the plain drop it is filed as.\n"
        );
    }
}

pub(crate) fn print_json(s: &RunSummary, findings: &[Finding]) {
    let shapes: Vec<Value> = findings
        .iter()
        .map(|f| {
            let Detail::Gap(d) = &f.detail;
            let ex = &f.example;
            serde_json::json!({
                // The producing audit — redundant with the run's `audit` for a
                // single-audit run, load-bearing once findings from several audits
                // share one list (see the module-doc sketches).
                "audit": f.audit,
                // The envelope severity, surfaced for scriptable triage: `gate-failing`
                // (gap's PANIC) vs `informational` (a drop/double-print the ratchet grades).
                "severity": f.severity.label(),
                "kind": d.kind_label,
                "shape": f.site,
                "count": d.count,
                "files": d.files,
                "payloads": d.payloads,
                "bystander_hits": d.bystander_hits,
                "verdict": verdict_json(f.confidence),
                "verify_confirmed": d.verify_confirmed,
                "verify_total": d.verify_total,
                "example_payload": ex.payload,
                "example_path": ex.path,
                // Two offsets: the injection site (reproduces the drop) and the attribution
                // site (the victim's own location for a bystander; == injection when injected).
                // The shape/snippet key on the attribution offset.
                "example_injection_offset": ex.injection_offset,
                "example_attribution_offset": ex.attribution_offset,
                "example_snippet": ex.snippet,
                "example_text": ex.text,
                "example_injected": ex.injected,
            })
        })
        .collect();
    let out = serde_json::json!({
        "files": s.files_done,
        "sites": s.sites,
        "injections": s.injections,
        "accepted": s.accepted,
        "parse_skipped": s.parse_skipped,
        "dirty_files": s.dirty_files,
        "payloads": s.payload_labels,
        "findings": findings.iter().map(Finding::count).sum::<usize>(),
        "shapes": shapes,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

/// How many pinnable findings ([`Severity::Informational`]) carry the given
/// [`Confidence`] — `Unconfirmed` or `Partial`. A `GateFailing` finding (gap's
/// PANIC) matches neither.
fn count_confidence(findings: &[Finding], want: Confidence) -> usize {
    findings
        .iter()
        .filter(|f| f.severity == Severity::Informational && f.confidence == Some(want))
        .count()
}

/// The `--json` verdict string for a confidence axis — `None` reads `unverified`.
fn verdict_json(confidence: Option<Confidence>) -> &'static str {
    match confidence {
        None => "unverified",
        Some(Confidence::Confirmed) => "confirmed",
        Some(Confidence::Partial) => "partial",
        Some(Confidence::Unconfirmed) => "unconfirmed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The confidence axis's `--json` strings — the mapping the JSON view pins.
    #[test]
    fn verdict_json_maps_every_confidence() {
        assert_eq!(verdict_json(None), "unverified");
        assert_eq!(verdict_json(Some(Confidence::Confirmed)), "confirmed");
        assert_eq!(verdict_json(Some(Confidence::Partial)), "partial");
        assert_eq!(verdict_json(Some(Confidence::Unconfirmed)), "unconfirmed");
    }
}
