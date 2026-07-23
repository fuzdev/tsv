//! Run-level tally primitives — the bounded bookkeeping a per-worker tally accumulates
//! and merges beside its shape map (the per-shape side lives in
//! [`examples`](crate::audit::examples)).

/// An exact count plus a bounded path sample — the "skipped, wants triage" bucket
/// (`blank_audit`'s and `ignore_audit`'s not-a-clean-fixed-point files).
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The count stays exact past the cap, the sample stops growing at it, and a merge
    /// respects both.
    #[test]
    fn count_exact_and_sample_capped_through_merge() {
        let mut a = CappedPaths::default();
        for i in 0..CappedPaths::CAP + 5 {
            a.push(format!("a{i}"));
        }
        assert_eq!(a.count(), CappedPaths::CAP + 5);
        assert_eq!(a.sample().len(), CappedPaths::CAP);
        assert_eq!(a.sample()[0], "a0", "sample keeps arrival order");

        let mut b = CappedPaths::default();
        b.push("b0".to_string());
        b.merge(a);
        assert_eq!(b.count(), CappedPaths::CAP + 6, "counts add exactly");
        assert_eq!(b.sample().len(), CappedPaths::CAP, "sample stays capped");
        assert_eq!(b.sample()[0], "b0", "the absorbing side keeps its head");

        assert!(!b.is_empty());
        assert!(CappedPaths::default().is_empty());
    }
}
