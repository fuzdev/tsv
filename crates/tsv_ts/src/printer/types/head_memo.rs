//! The per-call memo of the leading-edge descent ([`Printer::head_stripped_paren_shell`]).
//!
//! The descent asks a required-pair link's question twice of the same head: once as the
//! link's `downstream_owns_run` gate (whether the pair the position requires opens over a
//! `//` the head's own leading edge supplies, which descends on the [`EdgeRun::Breaking`]
//! axis) and once as the descent proper, on the axis the caller asked. Each ask descends
//! the whole head, so a head that is itself such a link — `((A extends B ? C : D) extends
//! B ? C : D)`, an array element, an indexed-access object or an intersection's first
//! member holding the next level — doubles the work per level. The memo makes each
//! `(node, axis)` answer once per outermost call.
//!
//! Sound because the answer at a node is a function of the node, the axis and printer
//! state only a build writes — the claim ([`Printer::claimed_shell_leading_run`], read by
//! the required-pair gate) and the array builder's negative-literal marks
//! ([`Printer::negative_literal_pair_targets`], read by the indexed-access object's paren
//! rule) — and none of it changes within one call: the AST is immutable, the descent is a
//! pure predicate (nothing below it builds a doc, and only a build sets a claim or a mark),
//! and the key carries the axis. The claim is in the key anyway, so a future descent that
//! does read a claim it changed cannot be answered stale; the marks are not, which is why
//! the memo cannot outlive the call.
//!
//! Scoped to the outermost call and dropped when it returns, so nothing outlives the
//! question; and unallocated until the call has taken [`HEAD_MEMO_AFTER`] visits, so the
//! short descents real code makes pay a counter test, never a map.

use std::cell::{Cell, RefCell};

use tsv_lang::FxHashMap;

use super::{EdgeRun, HeadRegion, Printer};
use crate::ast::TSType;

/// The visits an outermost descent takes before it memoizes: the descent real code makes
/// is a handful of links, and one that outlives this many memoizes the rest of the call,
/// which bounds it by the (node, axis) pairs it can reach. Unit tests memoize from the
/// first visit unless they ask otherwise (`test_memo_after`).
const HEAD_MEMO_AFTER: u32 = 256;

#[cfg(not(test))]
#[inline]
const fn head_memo_after() -> u32 {
    HEAD_MEMO_AFTER
}

#[cfg(test)]
fn head_memo_after() -> u32 {
    test_memo_after::get()
}

/// The threshold unit tests run at: per thread, zero (memoize from the first visit) unless
/// a test names another.
#[cfg(test)]
pub(super) mod test_memo_after {
    use std::cell::Cell;

    thread_local! {
        static AFTER: Cell<u32> = const { Cell::new(0) };
    }

    pub(super) fn get() -> u32 {
        AFTER.with(Cell::get)
    }

    /// Run `f` with every head memo starting after `visits`, then restore the threshold.
    pub(in crate::printer) fn with<R>(visits: u32, f: impl FnOnce() -> R) -> R {
        let prev = AFTER.with(|a| a.replace(visits));
        let out = f();
        AFTER.with(|a| a.set(prev));
        out
    }
}

/// One memoized answer's key: the node (by address — two nodes never share one, and a
/// span can be shared by a node and its sole member), the axis, and the claim the answer
/// was read under.
type HeadKey = (usize, EdgeRun, Option<(u32, u32)>);

/// One descent's answer, as the memo stores it.
#[derive(Clone, Copy)]
struct HeadAnswer(Option<HeadRegion>);

/// The descent's memo, held by the [`Printer`] and live only within an outermost call.
#[derive(Default)]
pub(in crate::printer) struct HeadMemo {
    /// Calls of the descent in progress — zero between outermost calls.
    depth: Cell<u32>,
    /// Visits left before the memo starts, for the call in progress; zero once it has.
    unmemoized: Cell<u32>,
    map: RefCell<Option<FxHashMap<HeadKey, HeadAnswer>>>,
}

impl HeadMemo {
    /// Enter one call for `key`: its memoized answer, if the memo holds one. Every
    /// `enter` is paired with one [`Self::leave`].
    #[inline]
    fn enter(&self, key: HeadKey) -> Option<HeadAnswer> {
        let depth = self.depth.get();
        self.depth.set(depth + 1);
        if depth == 0 {
            self.unmemoized.set(head_memo_after());
        }
        let unmemoized = self.unmemoized.get();
        if unmemoized != 0 {
            self.unmemoized.set(unmemoized - 1);
            return None;
        }
        self.memoized_get(key)
    }

    /// Leave the call [`Self::enter`] opened, storing a computed answer (`Some`) as
    /// `key`'s once the memo has started, and dropping the memo when the outermost call
    /// returns.
    #[inline]
    fn leave(&self, key: HeadKey, computed: Option<HeadAnswer>) {
        if let Some(value) = computed
            && self.unmemoized.get() == 0
        {
            self.memoized_insert(key, value);
        }
        let depth = self.depth.get() - 1;
        self.depth.set(depth);
        if depth == 0 && self.unmemoized.get() == 0 {
            self.release();
        }
    }

    #[cold]
    #[inline(never)]
    fn memoized_get(&self, key: HeadKey) -> Option<HeadAnswer> {
        self.map
            .borrow_mut()
            .get_or_insert_with(FxHashMap::default)
            .get(&key)
            .copied()
    }

    #[cold]
    #[inline(never)]
    fn memoized_insert(&self, key: HeadKey, value: HeadAnswer) {
        self.map
            .borrow_mut()
            .get_or_insert_with(FxHashMap::default)
            .insert(key, value);
    }

    #[cold]
    #[inline(never)]
    fn release(&self) {
        self.map.borrow_mut().take();
    }
}

impl Printer<'_> {
    /// [`Self::head_stripped_paren_shell_uncached`], answered once per `(node, axis)`
    /// within an outermost call ([`HeadMemo`]).
    pub(super) fn head_stripped_paren_shell<'t>(
        &self,
        ty: &'t TSType<'t>,
        run: EdgeRun,
    ) -> Option<HeadRegion> {
        let key = (
            std::ptr::from_ref(ty) as usize,
            run,
            self.claimed_shell_leading_run
                .get()
                .map(|claim| (claim.start, claim.end)),
        );
        if let Some(HeadAnswer(found)) = self.head_memo.enter(key) {
            // Under test every hit is graded against the descent it stands in for: a key
            // that dropped a context the answer reads can hand one question another's
            // answer where the output happens not to move.
            #[cfg(test)]
            assert_eq!(
                found.map(HeadRegion::bounds),
                self.head_stripped_paren_shell_uncached(ty, run)
                    .map(HeadRegion::bounds),
                "a memo hit disagrees with the descent it stands in for"
            );
            self.head_memo.leave(key, None);
            return found;
        }
        let found = self.head_stripped_paren_shell_uncached(ty, run);
        self.head_memo.leave(key, Some(HeadAnswer(found)));
        found
    }
}

#[cfg(test)]
mod tests {
    //! The memo against the uncached descent: every input formats to the same bytes with
    //! the memo answering from the first visit (`test_memo_after` 0) and with it never
    //! starting. The inputs put each kind of comment at every token boundary of nested
    //! leading-edge links, in the positions that ask the descent on both axes — a
    //! keyword→value hang, a union member, an intersection member, a type argument — so a
    //! key that dropped the axis would hand one question another's answer. The output can
    //! survive a wrong answer, so under test every hit is also graded against the uncached
    //! descent where it is taken ([`super::Printer::head_stripped_paren_shell`]).

    use super::test_memo_after;

    /// Nested leading-edge links, each a required pair whose head is the next link.
    const BASES: &[&str] = &[
        "((A extends B ? C : D) extends E ? F : G)",
        "((A extends B ? C : D)[] extends E ? F : G)[]",
        "((A extends B ? C : D)[K] & E extends F ? G : H)[K]",
        "(((A) extends B ? C : D) & E) & F",
        "((A & B extends C ? D : E) extends F ? G : H)",
        "(((A) extends B ? C : D) extends E ? F : G)",
        "(((A)[] extends B ? C : D)[] extends E ? F : G)",
    ];

    /// The positions a base is asked about in.
    const CONTEXTS: &[(&str, &str)] = &[
        ("type T = ", ";\n"),
        ("type T = | Z | ", " | Y;\n"),
        ("type T = | ", " | Y;\n"),
        ("type T = Z & ", ";\n"),
        ("let x: ", ";\n"),
        ("type T = F<", ">;\n"),
        ("type T = keyof ", ";\n"),
    ];

    const COMMENTS: &[&str] = &["/*c*/ ", "\n/*c*/\n", "// c\n", "/*a\nb*/ "];

    fn format_with_memo_after(visits: u32, source: &str) -> Option<String> {
        test_memo_after::with(visits, || crate::format_str(source).ok())
    }

    #[test]
    fn the_memo_answers_as_the_uncached_descent_does() {
        let mut compared = 0;
        for base in BASES {
            for (pre, post) in CONTEXTS {
                for comment in COMMENTS {
                    // Every boundary just after an opening paren or before a token.
                    for (at, _) in base.match_indices(['(', ' ', '[', 'A']) {
                        let at = if base.as_bytes()[at] == b'(' {
                            at + 1
                        } else {
                            at
                        };
                        let source = format!("{pre}{}{comment}{}{post}", &base[..at], &base[at..]);
                        let uncached = format_with_memo_after(u32::MAX, &source);
                        let memoized = format_with_memo_after(0, &source);
                        assert_eq!(
                            memoized, uncached,
                            "the memo moved the output of {source:?}"
                        );
                        compared += usize::from(uncached.is_some());
                    }
                }
            }
        }
        assert!(compared > 500, "only {compared} inputs formatted");
    }

    /// The production threshold too: a descent that crosses it mid-call answers the rest
    /// of the call from the memo, and the switch-over itself moves nothing.
    #[test]
    fn crossing_the_threshold_mid_call_moves_nothing() {
        let deep = format!(
            "type T = {}A{};\n",
            "(".repeat(14),
            " extends B ? C : D /* c */)".repeat(14)
        );
        let uncached = format_with_memo_after(u32::MAX, &deep);
        assert!(uncached.is_some());
        for visits in [1, 7, 64, super::HEAD_MEMO_AFTER] {
            assert_eq!(
                format_with_memo_after(visits, &deep),
                uncached,
                "after {visits}"
            );
        }
    }
}
