//! The bounded, deterministic **example set** a shape aggregate keeps — the
//! smallest-by-`(path, offset)` reproducers every injection audit reports.
//!
//! Each audit's per-shape aggregate (its `ShapeAgg`) owns its own counts and vocabulary —
//! gap's payload set and bystander split, blank's and ignore's leaner tallies — but the
//! example-keeping underneath them is one invariant, and a subtle one: the kept set must
//! be the `N` **globally** smallest by [`ExampleOrd::sort_key`], independent of worker
//! count and merge order, with a tie keeping the first-seen. Threads take files by stride,
//! so which worker first sees a shape depends on `--jobs`; keeping the smallest instead of
//! "whoever merged first" is what keeps a report (and any diff of one) stable across
//! `--jobs 1` and `--jobs 12`. It was copied three times (gap at `N = 5` for its verify
//! pass, blank and ignore at `N = 1`) before it lived here.
//!
//! The aggregates themselves stay per-audit by design — a generic whole-`ShapeAgg` would
//! force gap's extra fields (`payloads` / `bystander_hits` / `verify`) through a wrapper or
//! an extras trait, complicating the most complex consumer to deduplicate the two simple
//! ones. Only the shared core is shared.

/// Orders an audit's examples for canonical selection: `(path, offset)`, where the offset
/// is the audit's **attribution** locus — gap keys on the attribution offset (the victim
/// comment's own site for a bystander, so the canonical example is the finding's smallest
/// victim site, not wherever a payload went in), blank and ignore on the injection offset
/// (their findings have no bystander axis).
pub(crate) trait ExampleOrd {
    fn sort_key(&self) -> (&str, usize);
}

/// The `N` smallest examples by [`ExampleOrd::sort_key`], kept sorted ascending — so
/// [`Self::canonical`] (`examples[0]`) is the smallest, and a corpus that fires a bug a
/// million times still reports in bounded memory.
#[derive(Clone)]
pub(crate) struct ExampleSet<E, const N: usize> {
    examples: Vec<E>,
}

// Manual, not derived: a derive would bound `E: Default` for an empty-vec impl that
// needs no such thing.
impl<E, const N: usize> Default for ExampleSet<E, N> {
    fn default() -> Self {
        Self {
            examples: Vec::new(),
        }
    }
}

impl<E: ExampleOrd, const N: usize> ExampleSet<E, N> {
    /// Offer `candidate` to the bounded min-`N` set.
    ///
    /// A later candidate that **ties** an existing one on `sort_key` sorts *after* it
    /// (`<=` insertion point), so the first-seen among equal keys stays canonical —
    /// `examples[0]` never regresses to a later arrival. Ties only ever arise within one
    /// worker's tally (workers take disjoint files, and a path is visited by exactly one
    /// worker), so the merged set is deterministic regardless of `--jobs`.
    pub(crate) fn offer(&mut self, candidate: E) {
        let pos = self
            .examples
            .partition_point(|e| e.sort_key() <= candidate.sort_key());
        if pos >= N && self.examples.len() >= N {
            return; // larger than every kept example, and the set is already full
        }
        self.examples.insert(pos, candidate);
        self.examples.truncate(N);
    }

    /// The canonical example — the smallest by sort key, shown in every report.
    ///
    /// A recorded shape is always created *with* the hit that recorded it (every audit's
    /// `record` offers an example on the same entry it creates), so this never sees an
    /// empty set — an empty one is a construction bug.
    #[allow(clippy::expect_used)] // invariant: a recorded shape carries an example
    pub(crate) fn canonical(&self) -> &E {
        self.examples
            .first()
            .expect("a recorded shape always carries an example")
    }

    /// Fold `other`'s kept examples in, keeping the `N` smallest across both — the
    /// tally-merge arm.
    pub(crate) fn merge(&mut self, other: Self) {
        for ex in other.examples {
            self.offer(ex);
        }
    }

    /// The kept examples, ascending by sort key (gap's verify pass re-checks each).
    pub(crate) fn iter(&self) -> std::slice::Iter<'_, E> {
        self.examples.iter()
    }

    /// How many examples are kept (≤ `N`) — gap's verify denominator, never zero for a
    /// recorded shape.
    pub(crate) fn len(&self) -> usize {
        self.examples.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A toy example exercising the set in isolation from any real audit.
    #[derive(Clone, PartialEq, Eq, Debug)]
    struct Toy {
        path: &'static str,
        offset: usize,
        tag: &'static str,
    }

    impl ExampleOrd for Toy {
        fn sort_key(&self) -> (&str, usize) {
            (self.path, self.offset)
        }
    }

    fn toy(path: &'static str, offset: usize) -> Toy {
        Toy {
            path,
            offset,
            tag: "",
        }
    }

    /// The bounded set keeps the `N` smallest by `sort_key`, whatever the arrival order —
    /// the property that makes the kept set (and any diff of it) independent of `--jobs`.
    #[test]
    fn offer_keeps_the_n_smallest_by_sort_key() {
        let mut set: ExampleSet<Toy, 5> = ExampleSet::default();
        for off in [9, 3, 7, 1, 5, 8, 2, 6, 0, 4] {
            set.offer(toy("a.svelte", off));
        }
        let offsets: Vec<usize> = set.iter().map(|e| e.offset).collect();
        assert_eq!(offsets, vec![0, 1, 2, 3, 4]);
        assert_eq!(set.canonical().offset, 0, "canonical is the smallest");
        assert_eq!(set.len(), 5);
    }

    /// A later candidate that TIES an existing one on `sort_key` sorts AFTER it, so the
    /// first-seen stays canonical — `examples[0]` never regresses to a later arrival.
    #[test]
    fn ties_keep_the_first_seen_canonical() {
        let mut set: ExampleSet<Toy, 5> = ExampleSet::default();
        set.offer(Toy {
            tag: "first",
            ..toy("a.svelte", 0)
        });
        set.offer(Toy {
            tag: "second",
            ..toy("a.svelte", 0)
        });
        assert_eq!(set.len(), 2, "both ties are distinct examples, both kept");
        assert_eq!(set.canonical().tag, "first", "first-seen stays canonical");
    }

    /// At `N = 1` (blank / ignore) the set degenerates to "replace iff strictly smaller" —
    /// the exact semantics of the single-`Option` version it replaced: a tie keeps the
    /// incumbent, a smaller candidate replaces it.
    #[test]
    fn n1_replaces_only_on_strictly_smaller() {
        let mut set: ExampleSet<Toy, 1> = ExampleSet::default();
        set.offer(Toy {
            tag: "b5",
            ..toy("b.ts", 5)
        });
        // A larger candidate is dropped.
        set.offer(toy("b.ts", 9));
        assert_eq!(set.canonical().offset, 5);
        // A tie keeps the incumbent.
        set.offer(Toy {
            tag: "tie",
            ..toy("b.ts", 5)
        });
        assert_eq!(set.canonical().tag, "b5");
        // A strictly smaller candidate replaces it, and the set stays at one.
        set.offer(toy("a.ts", 7));
        assert_eq!(set.canonical().path, "a.ts");
        assert_eq!(set.len(), 1);
    }

    /// Merging keeps the `N` smallest across both sets — the property `Tally::merge`
    /// rides: the result is determined purely by the global key set, not merge order.
    #[test]
    fn merge_keeps_the_n_smallest_across_sets() {
        let mut a: ExampleSet<Toy, 3> = ExampleSet::default();
        let mut b: ExampleSet<Toy, 3> = ExampleSet::default();
        for off in [0, 2, 4] {
            a.offer(toy("a.svelte", off));
        }
        for off in [1, 3, 5] {
            b.offer(toy("b.svelte", off));
        }
        // `a.svelte` sorts before `b.svelte`, so the three smallest are all of a's,
        // whichever way the merge runs.
        let mut merged_ab = a.clone();
        merged_ab.merge(b.clone());
        let mut merged_ba = b;
        merged_ba.merge(a);
        for merged in [merged_ab, merged_ba] {
            let got: Vec<(&str, usize)> = merged.iter().map(|e| (e.path, e.offset)).collect();
            assert_eq!(got, vec![("a.svelte", 0), ("a.svelte", 2), ("a.svelte", 4)]);
        }
    }
}
