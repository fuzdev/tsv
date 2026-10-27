//! Opt-in work counters for `tsv_debug build_fanout_audit` (`fanout_check` feature, off by
//! default).
//!
//! The fanout audit grades a format by the work it does, not by its time or memory: how often
//! the printers built each AST node ([`build`]), how many nodes the doc walks visited
//! ([`walk`]), how many positions the source scans asked `skip_trivia` about ([`scan`]), and
//! how many steps the printers' structural predicates took ([`predicate`]: the type printer's
//! leading-edge descents, the statement-list comment-run lookahead, the Svelte root
//! inline-run lookahead) — each placed where a blowup that no other counter sees was found.
//! Counts are deterministic, so a growth curve over them is a gate where a timing would be
//! noise. Beside them, [`node`] counts every doc node the arena allocates, whichever printer
//! asks: it is budgeted, not reported (the audit reads the arena's own length for that).
//!
//! **With the feature off every entry point here is an empty `#[inline]` function**, so the
//! call sites the printers and the doc walks carry compile to nothing in every shipped build.
//! With it on, a site costs one relaxed load of [`arm`]'s flag until the audit arms the
//! counters, so the other audits sharing the `audits` build pay only that.
//!
//! The counters are per thread: the audit arms them, formats one input on the same thread,
//! and reads them back with [`disarm`]. A [`Budget`] bounds an armed run — the call that takes
//! a counter past its budget panics with [`BUDGET_EXCEEDED`], so a shape whose work is
//! exponential stops while it is still small instead of taking the machine's memory.

/// What a [`build`] call counts: the dispatch that built the node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Build {
    Expression,
    Type,
    Statement,
}

/// The panic payload a run that outgrows its [`Budget`] stops with.
pub const BUDGET_EXCEEDED: &str = "fanout budget exceeded";

/// The most work an armed run may do before it is stopped ([`BUDGET_EXCEEDED`]).
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    /// Node builds, every node's together.
    pub builds: u64,
    /// Doc-walk visits.
    pub walks: u64,
    /// Source-scan steps.
    pub scans: u64,
    /// Printer predicate steps.
    pub predicates: u64,
    /// Doc nodes allocated, by any printer. The one count that bounds the doc's memory
    /// whatever builds it, the builders the other counters do not sit on included.
    pub nodes: u64,
}

/// What an armed run counted, read back by [`disarm`].
#[derive(Clone, Debug, Default)]
pub struct Tally {
    /// Builds of each distinct node, keyed by its dispatch, kind and span.
    pub builds: Vec<(BuildKey, u32)>,
    /// Doc-walk visits.
    pub walks: u64,
    /// Source-scan steps.
    pub scans: u64,
    /// Printer predicate steps.
    pub predicates: u64,
}

/// One AST node, as the build counter tells nodes apart: its dispatch, its kind (a hash of
/// the enum discriminant) and its span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BuildKey {
    pub dispatch: Build,
    pub kind: u64,
    pub start: u32,
    pub end: u32,
}

impl Tally {
    /// The number of builds of every node together.
    pub fn total_builds(&self) -> u64 {
        self.builds.iter().map(|&(_, n)| u64::from(n)).sum()
    }

    /// The most builds of any one node, with the node.
    pub fn max_builds(&self) -> Option<(BuildKey, u32)> {
        self.builds.iter().copied().max_by_key(|&(key, n)| (n, key))
    }
}

#[cfg(feature = "fanout_check")]
mod imp {
    use std::cell::RefCell;
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

    use super::{BUDGET_EXCEEDED, Budget, Build, BuildKey, Tally};
    use crate::{FxHashMap, FxHasher};

    /// Whether any thread is armed. Read first at every site, so an unarmed build pays a
    /// relaxed load and nothing else.
    static ANY_ARMED: AtomicBool = AtomicBool::new(false);

    struct State {
        budget: Budget,
        builds: FxHashMap<BuildKey, u32>,
        total_builds: u64,
        walks: u64,
        scans: u64,
        predicates: u64,
        nodes: u64,
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    }

    /// Run `f` on this thread's state when it is armed, and stop the run when `f` reports
    /// it past its budget (the panic comes after the borrow ends). Out of line and cold, so
    /// an unarmed site is the flag's load and a branch, and adds nothing to the frame of the
    /// recursive walk it sits in.
    #[cold]
    #[inline(never)]
    fn armed(f: impl FnOnce(&mut State) -> bool) {
        let over = STATE.with(|state| state.borrow_mut().as_mut().is_some_and(f));
        if over {
            // the stop IS a panic: the audit catches it and reads the payload, the one way to
            // leave a format midway without a return path through every printer
            #[expect(clippy::panic)]
            std::panic::panic_any(BUDGET_EXCEEDED);
        }
    }

    #[inline]
    fn any_armed() -> bool {
        ANY_ARMED.load(Relaxed)
    }

    #[inline]
    pub fn build<K>(dispatch: Build, kind: &K, span: impl FnOnce() -> (u32, u32)) {
        if any_armed() {
            let (start, end) = span();
            build_armed(dispatch, discriminant_hash(kind), start, end);
        }
    }

    #[cold]
    #[inline(never)]
    fn discriminant_hash<K>(kind: &K) -> u64 {
        let mut hasher = FxHasher::default();
        std::mem::discriminant(kind).hash(&mut hasher);
        hasher.finish()
    }

    /// The armed half of [`build`], over scalars alone: the printers' dispatch frames carry
    /// the call and nothing it captures.
    #[cold]
    #[inline(never)]
    fn build_armed(dispatch: Build, kind: u64, start: u32, end: u32) {
        let key = BuildKey {
            dispatch,
            kind,
            start,
            end,
        };
        armed(|state| {
            *state.builds.entry(key).or_insert(0) += 1;
            state.total_builds += 1;
            state.total_builds > state.budget.builds
        });
    }

    #[inline]
    pub fn walk() {
        if any_armed() {
            armed(|state| {
                state.walks += 1;
                state.walks > state.budget.walks
            });
        }
    }

    #[inline]
    pub fn scan(n: u64) {
        if any_armed() {
            armed(|state| {
                state.scans += n;
                state.scans > state.budget.scans
            });
        }
    }

    #[inline]
    pub fn predicate() {
        if any_armed() {
            armed(|state| {
                state.predicates += 1;
                state.predicates > state.budget.predicates
            });
        }
    }

    #[inline]
    pub fn node() {
        if any_armed() {
            node_armed();
        }
    }

    /// The armed half of [`node`], out of line: [`node`] sits in the doc arena's `alloc`,
    /// which every builder inlines, so the builders' frames carry the flag's load and the
    /// call alone.
    #[cold]
    #[inline(never)]
    fn node_armed() {
        armed(|state| {
            state.nodes += 1;
            state.nodes > state.budget.nodes
        });
    }

    pub fn arm(budget: Budget) {
        STATE.with(|state| {
            *state.borrow_mut() = Some(State {
                budget,
                builds: FxHashMap::default(),
                total_builds: 0,
                walks: 0,
                scans: 0,
                predicates: 0,
                nodes: 0,
            });
        });
        ANY_ARMED.store(true, Relaxed);
    }

    pub fn disarm() -> Tally {
        STATE.with(|state| {
            state
                .borrow_mut()
                .take()
                .map_or_else(Tally::default, |state| {
                    let mut builds: Vec<_> = state.builds.into_iter().collect();
                    builds.sort_unstable();
                    Tally {
                        builds,
                        walks: state.walks,
                        scans: state.scans,
                        predicates: state.predicates,
                    }
                })
        })
    }

    pub const ENABLED: bool = true;
}

#[cfg(not(feature = "fanout_check"))]
mod imp {
    use super::{Budget, Build, Tally};

    #[inline]
    pub fn build<K>(_dispatch: Build, _kind: &K, _span: impl FnOnce() -> (u32, u32)) {}

    #[inline]
    pub fn walk() {}

    #[inline]
    pub fn scan(_n: u64) {}

    #[inline]
    pub fn predicate() {}

    #[inline]
    pub fn node() {}

    #[inline]
    pub fn arm(_budget: Budget) {}

    #[inline]
    pub fn disarm() -> Tally {
        Tally::default()
    }

    pub const ENABLED: bool = false;
}

/// Whether the counters are compiled in (the `fanout_check` feature). Without them every
/// [`disarm`] reads an empty [`Tally`], which an audit must refuse rather than grade.
pub const ENABLED: bool = imp::ENABLED;

/// Count one build of the AST node of kind `kind` (an enum whose discriminant names it), by
/// `dispatch`; `span` answers the node's `(start, end)`, asked only when a run is armed. A
/// no-op without the `fanout_check` feature, which never calls `span`.
#[inline]
pub fn build<K>(dispatch: Build, kind: &K, span: impl FnOnce() -> (u32, u32)) {
    imp::build(dispatch, kind, span);
}

/// Count one doc-walk visit. A no-op without the `fanout_check` feature.
#[inline]
pub fn walk() {
    imp::walk();
}

/// Count `n` source-scan steps. A no-op without the `fanout_check` feature.
#[inline]
pub fn scan(n: u64) {
    imp::scan(n);
}

/// Count one step of a printer's structural predicate — an AST descent or a lookahead over
/// sibling nodes or comments. A no-op without the `fanout_check` feature.
#[inline]
pub fn predicate() {
    imp::predicate();
}

/// Count one doc node allocated. A no-op without the `fanout_check` feature.
#[inline]
pub fn node() {
    imp::node();
}

/// Start counting on this thread, from zero, under `budget`. A no-op without the
/// `fanout_check` feature.
#[inline]
pub fn arm(budget: Budget) {
    imp::arm(budget);
}

/// Stop counting on this thread and read what was counted since [`arm`] (empty when it was
/// never armed, or without the `fanout_check` feature).
#[inline]
pub fn disarm() -> Tally {
    imp::disarm()
}
