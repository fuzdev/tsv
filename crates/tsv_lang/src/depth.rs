//! The parsers' depth limits: how deep a document may nest, and how long a left-nested
//! chain may run, before a parse refuses it with a syntax error instead of building a
//! tree whose later walks would exhaust the stack.
//!
//! Every consumer of a parsed tree — the wire-JSON writers, the printers, the type strip —
//! recurses over it, and a stack overflow is not a catchable panic: it aborts the host
//! process (a CLI run, a Node or Deno process holding the N-API addon or the FFI library)
//! or traps a WASM instance. So the parsers bound the tree's depth, and the bound is
//! sized so that every walk of an accepted tree fits the stack contract stated in
//! `docs/cli.md` §Recursion Depth.
//!
//! Two quantities are bounded, because no one counter can bound both cheaply:
//!
//! - **Nesting** ([`NESTING_LIMIT`], [`DepthBudget`]) — the depth of the parsers' own
//!   recursion, counted in **wire levels**: each recursive step adds at least the
//!   wire-JSON nodes it puts on the path from the root (a nested call or paren adds one, a
//!   type reference's argument two, an arrow function's block body three, a Svelte element
//!   two — its node and its fragment), so no path through an accepted tree's wire is
//!   deeper than the limit — what the depth-limit gate checks of every shape it
//!   generates. The count runs **document-wide**: a Svelte template
//!   expression or `<style>` body starts at the depth of the markup around it
//!   ([`DepthBudget::at`]), so the markup and the islands it holds share one budget.
//! - **Spine** ([`SPINE_LIMIT`], [`SpineCount`]) — the links on a **left spine**: the
//!   chains the parsers build *iteratively*, without recursing — a binary or logical
//!   operator chain (`a + b + c` nests to the left), a member / call / non-null /
//!   tagged-template / instantiation / `as` / `satisfies` chain (`a.b().c!`), the type
//!   chains (`T[][]`, `A.B.C`), and the other nodes that wrap an operand parsed before the
//!   parser knew it was one (a conditional's test, a sequence's first expression, a
//!   conditional type's check type, a tuple element's `?`). Their parse costs no stack, but every walk of the tree
//!   recurses down the spine, so a flat chain needs a bound of its own; a nesting counter
//!   never sees one. A link counts the spine units its walks pay for
//!   ([`SpineCount::try_link`]): one for an operator, more for the links whose walks spend
//!   more stack.
//!
//! **The spine count is an upper bound on every root-to-leaf path**, not a measure of one
//! chain, because the walks recurse along paths and a path can pass several chains — a
//! chain whose leaf holds another (`(a + … + a) + a + …`), or one inside a call's
//! argument. The leaf is parsed before the chain over it is known, so the parsers cannot
//! count a path top-down; instead the count runs over **frames**: within a frame every
//! link adds up, chains on one path and beside one another alike, and a frame that closes
//! hands its total up as the greater of it and its siblings' ([`SpineCount::take`],
//! [`SpineCount::fold`]). The parsers open a frame where siblings can run long — the
//! statements of a list, the operands of an operator chain, the elements of the other
//! lists — once the count runs long or the list does ([`SpineCount::runs_long`]), so short
//! code pays nothing for them and the bound is loose by at most a few dozen units.
//!
//! **Nesting and spine share the spine's budget along one path.** Each limit alone fits
//! the stack, but a path can hold both at once — a chain at its limit whose leaf is
//! nested to its limit costs the two walks' stack added up, which neither limit sees. So a
//! link is checked against the spine limit with the frame's deepest nesting charged on
//! top, at [`NESTING_SPINE_WEIGHT`] spine units a level — the stack a level costs,
//! measured in links ([`DepthBudget::charged_nesting`]). The deepest nesting is framed as
//! the spine count is ([`DepthBudget::take_peak`], [`DepthBudget::fold_peak`]): a
//! statement, or an element of a long list, is charged the nesting of its own paths, never
//! a sibling's, so whether a document parses does not depend on the order of its
//! statements. The nesting limit alone
//! (`NESTING_LIMIT × NESTING_SPINE_WEIGHT`) stays under the spine limit, so a path with no
//! chain on it is bounded by the nesting limit only.
//!
//! **The output's nesting is the input's plus the printer's pairs.** A format writes grouping
//! parens the source does not hold (`(a == b) == c`), and its output, parsed again, counts each
//! as a level. So the spine count carries the pairs the printer will add beside its links
//! ([`SpineCount::try_pair`]), framed as the links are, and charges them as nesting in both
//! checks — so a format's output parses wherever its input did. A parser charges at a node the
//! pairs the printer may write there less the pairs the author wrote: what the printer writes
//! is a function of the tree, which a format keeps, so on the output — whose pairs are the
//! ones the printer wrote — the charge is none, and the reparse counts no more than the parse
//! did. No frame decision reads the pairs ([`SpineCount::runs_long`]): the output, whose
//! pairs are grouping parens there, is framed exactly as its input was.

use crate::error::ParseError;

/// The deepest nesting a parse accepts, in wire levels (see the module docs). Above every
/// input the conformance gates parse (the deepest, tsc's
/// `parsingDeepParenthensizedExpression.ts`, reaches 137), and low enough that the deepest
/// accepted tree's walks fit a 2 MiB stack in an optimized build.
pub const NESTING_LIMIT: u32 = 256;

/// The longest left spine a parse accepts, in spine units, the nesting charge included
/// (see the module docs). Above every input the conformance gates parse (the longest,
/// tsc's two `binderBinaryExpressionStress` files, chain 4,953 operators, 5,440 units
/// with the charge), and low enough that the longest accepted chain's walks fit a 2 MiB
/// stack in an optimized build.
pub const SPINE_LIMIT: u32 = 6_000;

/// The spine units one level of nesting on a chain's path costs: the stack the dearest
/// nesting level's walk spends (a nested member call's print) over the stack the cheapest
/// spine unit's (an operator's print), rounded up. With it, a path's spine units bound its
/// stack in every walk, whatever mix of chain and nesting it holds.
pub const NESTING_SPINE_WEIGHT: u32 = 15;

/// The nesting every chain is charged at least ([`DepthBudget::charged_nesting`]): below
/// it a statement's deepest level is not tracked, which keeps the nesting count's hot path
/// a single compare. Real code seldom nests past it.
pub const NESTING_FLOOR: u32 = 32;

// A path with no chain on it must be bounded by the nesting limit alone, and a lone chain
// must keep room for the gate inputs' (above).
const _: () = assert!(NESTING_LIMIT * NESTING_SPINE_WEIGHT < SPINE_LIMIT);
const _: () = assert!(NESTING_FLOOR * NESTING_SPINE_WEIGHT + 5_000 < SPINE_LIMIT);

/// Which depth limit a parse exceeded ([`ParseError::too_deep`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthLimit {
    /// [`NESTING_LIMIT`].
    Nesting,
    /// [`SPINE_LIMIT`].
    Spine,
}

impl DepthLimit {
    /// The limit's value.
    #[must_use]
    pub const fn value(self) -> u32 {
        match self {
            DepthLimit::Nesting => NESTING_LIMIT,
            DepthLimit::Spine => SPINE_LIMIT,
        }
    }
}

/// A parser's nesting count — the wire levels it has entered, top-down — and the deepest
/// level the open frame has reached, which the spine check charges
/// ([`DepthBudget::charged_nesting`]). One per parser, `Copy` so a checkpoint can carry it.
///
/// A parser may keep part of its nesting in a counter of its own and hand it in as
/// `beside` ([`DepthBudget::try_enter_beside`], [`DepthBudget::check_beside`]) — the
/// TypeScript parser's grouping-delimiter depth, which it maintains anyway. Every
/// `try_enter` is paired with a `leave`; a parse that fails is abandoned, or rewound with
/// the budget restored from a checkpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DepthBudget {
    nesting: u32,
    /// The deepest level entered in the open frame, once past [`NESTING_FLOOR`]; at least
    /// the nesting the frame started at ([`DepthBudget::take_peak`]).
    peak: u32,
}

impl DepthBudget {
    /// A budget with `nesting` levels already spent — the depth of the host markup at an
    /// embedded island (a Svelte template expression, a `<script>` or `<style>` body), so
    /// the island's own nesting counts on top of the markup's.
    #[must_use]
    pub const fn at(nesting: u32) -> Self {
        DepthBudget {
            nesting,
            peak: nesting,
        }
    }

    /// The nesting entered so far, in wire levels.
    #[must_use]
    pub const fn nesting(&self) -> u32 {
        self.nesting
    }

    /// Enter `units` wire levels of nesting; `false` (with nothing entered) where that
    /// would pass [`NESTING_LIMIT`]. The caller turns a `false` into
    /// [`ParseError::too_deep`] at its current token.
    #[inline]
    #[must_use]
    pub fn try_enter(&mut self, units: u32) -> bool {
        self.try_enter_beside(units, 0)
    }

    /// [`DepthBudget::try_enter`] for a parser keeping `beside` more levels in a counter of
    /// its own: the limit is tested against the two together.
    #[inline]
    #[must_use]
    pub fn try_enter_beside(&mut self, units: u32, beside: u32) -> bool {
        let nesting = self.nesting + units;
        let total = nesting + beside;
        if total > NESTING_FLOOR && !self.enter_deep(total) {
            return false;
        }
        self.nesting = nesting;
        true
    }

    /// Test the nesting with `beside` levels counted elsewhere — the parser's own counter,
    /// just raised: `false` where the two together pass [`NESTING_LIMIT`].
    #[inline]
    #[must_use]
    pub fn check_beside(&mut self, beside: u32) -> bool {
        let total = self.nesting + beside;
        total <= NESTING_FLOOR || self.enter_deep(total)
    }

    /// A level past [`NESTING_FLOOR`]: the limit's test, and the deepest level so far.
    #[cold]
    #[inline(never)]
    fn enter_deep(&mut self, total: u32) -> bool {
        if total > NESTING_LIMIT {
            return false;
        }
        self.peak = self.peak.max(total);
        true
    }

    /// Leave `units` wire levels entered by a successful [`DepthBudget::try_enter`].
    #[inline]
    pub fn leave(&mut self, units: u32) {
        debug_assert!(self.nesting >= units, "leave without a matching enter");
        self.nesting -= units;
    }

    /// The nesting a link is charged for: the deepest level the frame has reached, or
    /// [`NESTING_FLOOR`] where it never passed that — at least the nesting of every node on
    /// any path through the chain, so with the chain's own spine units it bounds the
    /// path's.
    #[inline]
    #[must_use]
    pub fn charged_nesting(&self) -> u32 {
        self.peak.max(NESTING_FLOOR)
    }

    /// Whether the open frame has reached past [`NESTING_FLOOR`] — below it, opening a
    /// frame of the deepest level ([`DepthBudget::take_peak`]) would change nothing a
    /// charge reads, since every charge is at least the floor.
    #[inline]
    #[must_use]
    pub fn is_deep(&self) -> bool {
        self.peak > NESTING_FLOOR
    }

    /// Open a frame of the deepest level, as [`SpineCount::take`] opens one of the spine
    /// count: the deepest level restarts at the nesting entered so far (with `beside` more
    /// levels kept in the parser's own counter), so a chain in a sibling — the next
    /// statement, the next element of a long list — is charged its own nesting rather than
    /// the deepest a sibling before it reached. Returns the enclosing frame's, which
    /// [`DepthBudget::fold_peak`] folds this frame's into.
    #[inline]
    #[must_use]
    pub fn take_peak(&mut self, beside: u32) -> u32 {
        std::mem::replace(&mut self.peak, self.nesting + beside)
    }

    /// Close the frame [`DepthBudget::take_peak`] opened: the deepest level becomes the
    /// greater of the two, so a chain the frame sits in is charged the nesting below it.
    #[inline]
    pub fn fold_peak(&mut self, outer: u32) {
        self.peak = self.peak.max(outer);
    }
}

/// The spine count of the open frame, in spine units ([`SPINE_LIMIT`]): every link in the
/// frame added up, and every closed frame inside it folded in as the greater of it and its
/// siblings'. One per parser.
///
/// Beside the links it counts the **pairs the printer will add** in the frame — the grouping
/// parens a format writes where the source has none (`a == b == c` prints
/// `(a == b) == c`, `f(a = b)` prints `f((a = b))`), framed exactly as the links are. A
/// format's output must parse again, and in that output every such pair is a level of
/// nesting; so a pair is charged as one ([`SpineCount::try_pair`]), on top of the frame's
/// deepest nesting against [`NESTING_LIMIT`] and on top of its charged nesting in every
/// link's spine check — the reparse of the output then counts no more than the parse of
/// the input did.
///
/// The two counts share one word, the units in its low half and the pairs in its high
/// half, so a link's common case — no pair in the frame and the count far from the limit
/// — is one add and one compare ([`SpineCount::try_link`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpineCount {
    packed: u64,
}

/// The pairs' place in [`SpineCount`]'s word.
const PAIR_SHIFT: u32 = 32;

impl SpineCount {
    /// The frame's links, in spine units.
    #[inline]
    const fn units(self) -> u32 {
        self.packed as u32
    }

    /// The frame's printer pairs.
    #[inline]
    const fn pairs(self) -> u32 {
        (self.packed >> PAIR_SHIFT) as u32
    }

    /// The count of `units` and `pairs`.
    #[inline]
    const fn of(units: u32, pairs: u32) -> Self {
        SpineCount {
            packed: units as u64 | (pairs as u64) << PAIR_SHIFT,
        }
    }

    /// Add a link of `units` spine units; `false` (with nothing added) where the frame's
    /// count, with `budget`'s charged nesting on top, would pass [`SPINE_LIMIT`]. The caller
    /// turns a `false` into [`ParseError::too_deep`].
    #[inline]
    #[must_use]
    pub fn try_link(&mut self, units: u32, budget: &DepthBudget) -> bool {
        // Under `ROOM_AT_ANY_NESTING` units, no nesting can carry the count past the limit,
        // so the charge is asked only above it — or where the frame holds a printer pair,
        // whose level the nesting limit bounds together with the frame's nesting only when
        // it is counted. A pair sets the word's high half, so one compare asks both.
        const ROOM_AT_ANY_NESTING: u32 = SPINE_LIMIT - NESTING_LIMIT * NESTING_SPINE_WEIGHT;
        let packed = self.packed + u64::from(units);
        if packed > u64::from(ROOM_AT_ANY_NESTING) && !Self::link_fits(packed, *budget) {
            return false;
        }
        self.packed = packed;
        true
    }

    /// [`SpineCount::try_link`] past its common case: whether the frame's count `packed`,
    /// the new link added, fits with `budget`'s charged nesting and the pairs on top.
    #[cold]
    #[inline(never)]
    fn link_fits(packed: u64, budget: DepthBudget) -> bool {
        let count = SpineCount { packed };
        count.units() + (budget.charged_nesting() + count.pairs()) * NESTING_SPINE_WEIGHT
            <= SPINE_LIMIT
    }

    /// Add a pair the printer will add (see the type's docs); `false` (with nothing added)
    /// where the frame's pairs with `budget`'s charged nesting would pass [`NESTING_LIMIT`],
    /// or its links with that nesting and the pairs charged would pass [`SPINE_LIMIT`]. The
    /// caller turns a `false` into [`ParseError::too_deep`].
    #[inline]
    #[must_use]
    pub fn try_pair(&mut self, budget: &DepthBudget) -> bool {
        let pairs = self.pairs() + 1;
        let nesting = budget.charged_nesting() + pairs;
        if nesting > NESTING_LIMIT || self.units() + nesting * NESTING_SPINE_WEIGHT > SPINE_LIMIT {
            return false;
        }
        *self = Self::of(self.units(), pairs);
        true
    }

    /// Whether the frame's count has run past a few dozen units — the point past which a
    /// parser frames siblings it would otherwise let add up (an operator chain's operands),
    /// so short chains pay nothing for it and a long one is charged its operands' greatest.
    /// It reads the links alone, never the printer pairs, so a format's output — whose
    /// pairs are grouping parens there — is framed exactly as its input was, and its parse
    /// counts no more than the input's did.
    #[inline]
    #[must_use]
    pub fn runs_long(&self) -> bool {
        self.units() > 64
    }

    /// Whether the frame has counted nothing.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packed == 0
    }

    /// The greater of two frames' counts, each count apart: the frames are siblings, so
    /// no path passes both.
    #[inline]
    #[must_use]
    pub fn max(self, other: SpineCount) -> SpineCount {
        // with no pair in either, the word's order is the units'
        if (self.packed | other.packed) >> PAIR_SHIFT == 0 {
            return SpineCount {
                packed: self.packed.max(other.packed),
            };
        }
        Self::of(
            self.units().max(other.units()),
            self.pairs().max(other.pairs()),
        )
    }

    /// Open a frame: its count restarts at zero. Returns the enclosing frame's count, which
    /// [`SpineCount::fold`] folds this frame's into.
    #[inline]
    #[must_use]
    pub fn take(&mut self) -> SpineCount {
        std::mem::take(self)
    }

    /// Close the frame [`SpineCount::take`] opened: the enclosing frame's count becomes the
    /// greater of its own and this one's, the two being siblings rather than one path.
    #[inline]
    pub fn fold(&mut self, outer: SpineCount) {
        *self = self.max(outer);
    }
}

impl ParseError {
    /// The parse passed `limit` at byte offset `position` (the token where the count
    /// tripped): a syntax error, positioned like every other, and told apart by
    /// [`ParseError::is_too_deep`].
    #[cold]
    #[inline(never)]
    #[must_use]
    pub fn too_deep(limit: DepthLimit, position: usize) -> Self {
        ParseError::depth_limit(limit, position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The depth error is a located syntax error, told apart from the others and naming
    /// its limit.
    #[test]
    fn the_depth_error_is_a_located_syntax_error_naming_its_limit() {
        let coordinates = crate::WireCoordinates {
            lines: crate::LineRule::Ecmascript,
            bom: crate::LeadingBom::Counted,
        };
        let nesting = ParseError::too_deep(DepthLimit::Nesting, 2).with_context("((x", coordinates);
        assert!(nesting.is_too_deep() && nesting.is_syntax_error());
        assert_eq!(nesting.position(), Some(2));
        assert!(
            nesting
                .to_string()
                .starts_with("Nesting exceeds tsv's depth limit of 256 levels\n")
        );
        let spine = ParseError::too_deep(DepthLimit::Spine, 0);
        assert!(spine.is_too_deep());
        assert!(spine.to_string().contains("exceeds tsv's length limit"));
        assert!(!ParseError::invalid_syntax("x".to_string(), 0).is_too_deep());
    }

    #[test]
    fn nesting_enters_up_to_the_limit_and_no_further() {
        let mut budget = DepthBudget::at(NESTING_LIMIT - 3);
        assert!(budget.try_enter(3));
        assert_eq!(budget.nesting(), NESTING_LIMIT);
        assert!(!budget.try_enter(1));
        assert_eq!(
            budget.nesting(),
            NESTING_LIMIT,
            "a refused enter enters nothing"
        );
        budget.leave(3);
        assert_eq!(budget.nesting(), NESTING_LIMIT - 3);
    }

    #[test]
    fn spine_links_up_to_the_limit_and_no_further() {
        let budget = DepthBudget::default();
        let room = SPINE_LIMIT - NESTING_FLOOR * NESTING_SPINE_WEIGHT;
        let mut spine = SpineCount::default();
        for _ in 0..room {
            assert!(spine.try_link(1, &budget));
        }
        assert!(!spine.try_link(1, &budget));
        assert_eq!(spine.take().units(), room);
    }

    /// Siblings take the greater of their counts; within a frame links add up.
    #[test]
    fn frames_fold_as_the_max_of_siblings() {
        let budget = DepthBudget::default();
        let mut spine = SpineCount::default();
        for _ in 0..3 {
            assert!(spine.try_link(1, &budget));
        }
        let outer = spine.take();
        for _ in 0..2 {
            assert!(spine.try_link(1, &budget));
        }
        spine.fold(outer);
        assert_eq!(spine.units(), 3, "a sibling takes the max, not the sum");
        for _ in 0..2 {
            assert!(spine.try_link(1, &budget));
        }
        assert_eq!(spine.units(), 5, "a chain over the frame adds to it");
    }

    /// The deepest level the parse has reached — past the floor — is charged to every link.
    #[test]
    fn the_deepest_nesting_is_charged_to_every_link() {
        let mut budget = DepthBudget::default();
        assert_eq!(budget.charged_nesting(), NESTING_FLOOR);
        assert!(budget.try_enter(100));
        budget.leave(100);
        assert_eq!(
            budget.charged_nesting(),
            100,
            "the deepest level, not the current"
        );
        let room = SPINE_LIMIT - 100 * NESTING_SPINE_WEIGHT;
        let mut spine = SpineCount::default();
        for _ in 0..room {
            assert!(spine.try_link(1, &budget));
        }
        assert!(!spine.try_link(1, &budget));
    }

    /// A frame of the deepest level restarts at the nesting entered so far, and folds back
    /// as the greater of the two.
    #[test]
    fn the_deepest_level_is_framed() {
        let mut budget = DepthBudget::default();
        assert!(budget.try_enter(40));
        assert!(budget.try_enter(100));
        budget.leave(100);
        assert_eq!(budget.charged_nesting(), 140);
        let outer = budget.take_peak(2);
        assert_eq!(outer, 140);
        assert_eq!(
            budget.charged_nesting(),
            42,
            "restarts at the nesting entered, the levels beside included"
        );
        budget.fold_peak(outer);
        assert_eq!(budget.charged_nesting(), 140);
    }

    /// A printer pair is a level of the output's nesting: the pairs and the charged nesting
    /// together stay within the nesting limit, and every link is charged them on top.
    #[test]
    fn printer_pairs_are_charged_as_nesting() {
        let budget = DepthBudget::default();
        let mut spine = SpineCount::default();
        for _ in 0..NESTING_LIMIT - NESTING_FLOOR {
            assert!(spine.try_pair(&budget));
        }
        assert!(!spine.try_pair(&budget), "past the nesting limit");
        let mut spine = SpineCount::default();
        assert!(spine.try_pair(&budget));
        let room = SPINE_LIMIT - (NESTING_FLOOR + 1) * NESTING_SPINE_WEIGHT;
        for _ in 0..room {
            assert!(spine.try_link(1, &budget));
        }
        assert!(
            !spine.try_link(1, &budget),
            "the pair is charged to the link"
        );
        assert!(!spine.try_pair(&budget), "and the link to the pair");
    }

    /// Pairs fold like links: siblings take the greater, and no frame decision reads them.
    #[test]
    fn printer_pairs_fold_as_the_max_of_siblings() {
        let budget = DepthBudget::default();
        let mut spine = SpineCount::default();
        for _ in 0..100 {
            assert!(spine.try_pair(&budget));
        }
        assert!(!spine.runs_long(), "pairs never open a frame");
        let outer = spine.take();
        assert!(spine.try_pair(&budget));
        spine.fold(outer);
        assert_eq!(spine.pairs(), 100);
    }

    /// Levels kept beside the budget count toward the limit and the deepest level alike.
    #[test]
    fn levels_beside_the_budget_count() {
        let mut budget = DepthBudget::at(NESTING_LIMIT - 10);
        assert!(budget.check_beside(10));
        assert!(!budget.check_beside(11));
        assert!(!budget.try_enter_beside(1, 10));
        assert!(budget.try_enter_beside(1, 9));
        assert_eq!(budget.charged_nesting(), NESTING_LIMIT);
    }
}
