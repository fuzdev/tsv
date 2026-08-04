//! Run-level tally primitives — the bounded bookkeeping a per-worker tally accumulates
//! and merges beside its shape map (the per-shape side lives in `audit::examples`,
//! feature-gated where this module is not, hence no intra-doc link).
//!
//! Two consumer shapes, which is why `CappedPaths::merge` is feature-gated
//! while the rest is not: the injection audits fold one bucket per worker (a
//! `comment_check` build), and the single-threaded pristine
//! [`sweep`](crate::audit::sweep) keeps its panicking inputs in one (every
//! build).

/// An exact count plus a bounded path sample — the "recorded, wants triage" bucket
/// (`blank_audit`'s and `ignore_audit`'s not-a-clean-fixed-point files, and the
/// pristine sweep's panicking inputs).
///
/// The COUNT is exact and always reported (a file the audit couldn't grade is a coverage
/// fact a graded gate must never silently drop); the PATH sample is bounded at
/// [`Self::CAP`] — enough to triage, bounded on a noisy corpus. The sample keeps arrival
/// order (first-seen per worker), so unlike an [`ExampleSet`](crate::audit::examples), which
/// paths it holds can vary with `--jobs`; it is a sample, never a key or a graded set.
#[derive(Default)]
pub(crate) struct CappedPaths {
    count: usize,
    sample: Vec<String>,
}

impl CappedPaths {
    /// Bound on the stored sample (the count stays exact).
    pub(crate) const CAP: usize = 20;

    /// Record one path: count it exactly, keep it only while the sample has room.
    pub(crate) fn push(&mut self, path: String) {
        self.count += 1;
        if self.sample.len() < Self::CAP {
            self.sample.push(path);
        }
    }

    /// Fold another tally's bucket in — counts add exactly, the sample stays capped.
    ///
    /// Gated with its only consumers (the per-worker injection audits): a
    /// single-threaded walk never merges, so in a default build this would be
    /// dead code rather than an unused convenience.
    #[cfg(feature = "comment_check")]
    pub(crate) fn merge(&mut self, other: Self) {
        self.count += other.count;
        for p in other.sample {
            if self.sample.len() < Self::CAP {
                self.sample.push(p);
            }
        }
    }

    /// The exact number of recorded paths (≥ the sample's length).
    pub(crate) fn count(&self) -> usize {
        self.count
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The bounded sample (≤ [`Self::CAP`] paths).
    pub(crate) fn sample(&self) -> &[String] {
        &self.sample
    }

    /// The sample as `indent`-prefixed lines, with a `… and N more` tail
    /// reconciling the exact count against what the sample holds.
    ///
    /// Every consumer that shows the sample owes the reader that tail — the
    /// count is exact and the list is not, so printing the list alone reads as
    /// the whole set. All three had hand-rolled the same two steps; the tail's
    /// subtraction is the part that goes quietly wrong (forget the cap and it
    /// prints "and 0 more", or underflows), so it lives here once, beside the
    /// cap that makes it necessary.
    pub(crate) fn sample_lines(&self, indent: &str) -> Vec<String> {
        let mut lines: Vec<String> = self
            .sample
            .iter()
            .map(|path| format!("{indent}{path}"))
            .collect();
        let omitted = self.count.saturating_sub(self.sample.len());
        if omitted > 0 {
            lines.push(format!("{indent}… and {omitted} more"));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The count stays exact past the cap and the sample stops growing at it.
    #[test]
    fn count_stays_exact_past_the_cap() {
        let mut a = CappedPaths::default();
        for i in 0..CappedPaths::CAP + 5 {
            a.push(format!("a{i}"));
        }
        assert_eq!(a.count(), CappedPaths::CAP + 5);
        assert_eq!(a.sample().len(), CappedPaths::CAP);
        assert_eq!(a.sample()[0], "a0", "sample keeps arrival order");

        assert!(!a.is_empty());
        assert!(CappedPaths::default().is_empty());
    }

    /// The rendering all three consumers share: indented sample lines, and the
    /// tail ONLY when the exact count outruns them (an unconditional tail would
    /// print "… and 0 more" on every uncapped bucket).
    #[test]
    fn sample_lines_indent_and_reconcile_the_exact_count() {
        assert!(CappedPaths::default().sample_lines("  ").is_empty());

        let mut under = CappedPaths::default();
        under.push("a".to_string());
        under.push("b".to_string());
        assert_eq!(under.sample_lines("    "), ["    a", "    b"]);

        let mut over = CappedPaths::default();
        for i in 0..CappedPaths::CAP + 3 {
            over.push(format!("p{i}"));
        }
        let lines = over.sample_lines("  ");
        assert_eq!(lines.len(), CappedPaths::CAP + 1, "sample + one tail line");
        assert_eq!(lines[0], "  p0");
        assert_eq!(lines[CappedPaths::CAP], "  … and 3 more");
    }

    /// A merge respects both halves — gated with [`CappedPaths::merge`] itself.
    #[cfg(feature = "comment_check")]
    #[test]
    fn merge_adds_counts_exactly_and_keeps_the_sample_capped() {
        let mut a = CappedPaths::default();
        for i in 0..CappedPaths::CAP + 5 {
            a.push(format!("a{i}"));
        }
        let mut b = CappedPaths::default();
        b.push("b0".to_string());
        b.merge(a);
        assert_eq!(b.count(), CappedPaths::CAP + 6, "counts add exactly");
        assert_eq!(b.sample().len(), CappedPaths::CAP, "sample stays capped");
        assert_eq!(b.sample()[0], "b0", "the absorbing side keeps its head");
    }
}
