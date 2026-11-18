//! The stores a stylesheet's lists and strings live in, and the per-thread parking of their
//! capacity.
//!
//! [`CssStores`] is one stylesheet's: one store per element type of a list — the nodes, the
//! block children, the three selector levels, a `::part()`'s name spans, the values and the
//! condition parts and segments — each with the scratch stack that builds its lists, which
//! `tsv_lang`'s `value_runs!` declares from one entry per element type; and a text pool, for
//! the strings a span of the source cannot express. Every element type is `Copy`, so a
//! finished list is copied into its store as a block.
//!
//! Between parses every scratch stack is empty: the parse driver leaves them so, whether the
//! parse succeeded or failed, which is what lets the next parse begin without marking them.
//!
//! What outlives a parse is the rewound form: [`CssStores::reset`] rewinds the stores in
//! place, keeping each written store's slot 0 and every allocation's capacity, and never
//! trims. Handles die with the parse that minted them: nothing run- or pool-text-shaped leaves
//! [`with_stores`]'s closure or a [`ParsedCss`], whose stores it keeps beside the tree they
//! belong to.

use std::cell::Cell;
use std::fmt;
use std::ops::Deref;

use tsv_lang::Span;
use tsv_lang::store::{MarkGroup, TextPool};
#[cfg(test)]
use tsv_lang::value_runs::FieldShape;

use super::{
    ComplexSelector, ConditionPart, ConditionSegment, CssBlockChild, CssNode, CssStyleSheet,
    CssValue, CssView, RelativeSelector, SimpleSelector,
};

/// The family tag of a stylesheet's text pool: the decoded and normalized strings a span of
/// the source cannot express.
pub enum CssText {}

// A handle keeps its niche, so an absent string costs nothing beside a present one.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::{PoolText, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<PoolText<CssText>>() == 8);
    assert!(STORE_CHECK || size_of::<Option<PoolText<CssText>>>() == 8);
};

tsv_lang::value_runs! {
    /// The stores of a stylesheet's lists, one per element type, and the scratch stacks that
    /// build them.
    pub(crate) struct RunStores;
    /// The tops of every scratch stack, taken together: the restore point of a sub-parse
    /// whose failure the parse recovers from.
    pub(crate) struct CssScratchMarks;
    pub view CssView::runs;

    /// A stylesheet's top-level rules and at-rules: [`CssNode`] rows.
    nodes, node_scratch: CssNode => nodes;
    /// The children of a block — a rule's declarations, an at-rule block's children:
    /// [`CssBlockChild`] rows.
    block_children, block_child_scratch: CssBlockChild => block_children;
    /// A selector list's complex selectors: [`ComplexSelector`] rows.
    complex_selectors, complex_selector_scratch: ComplexSelector => complex_selectors;
    /// A complex selector's relative selectors: [`RelativeSelector`] rows.
    relative_selectors, relative_selector_scratch: RelativeSelector => relative_selectors;
    /// A relative selector's compound, its simple selectors: [`SimpleSelector`] rows.
    simple_selectors, simple_selector_scratch: SimpleSelector => simple_selectors;
    /// The spans of a `::part()` argument's names: [`Span`] rows.
    part_spans, part_span_scratch: Span as Row => part_spans;
    /// A condition query's parts: [`ConditionPart`] rows.
    condition_parts, condition_part_scratch: ConditionPart => condition_parts;
    /// A condition part's segments: [`ConditionSegment`] rows.
    condition_segments, condition_segment_scratch: ConditionSegment => condition_segments;
    /// The members of a value list — a function's arguments, a space- or comma-separated
    /// list's members, an `@import` prelude's values: [`CssValue`] rows.
    values, value_scratch: CssValue => values;
}

// Every list is a `Run<F>`, which lays out alike for every family `F`, and an absent list folds
// into the run's start.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::{Run, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<Run<CssValue>>() == 8);
    assert!(STORE_CHECK || size_of::<Option<Run<CssValue>>>() == 8);
};

/// One stylesheet's list stores, the scratch stacks that build their lists, and its text pool.
///
/// A fresh one allocates nothing: a store's buffer is allocated by its first write, a scratch
/// stack's by its first push, and the pool's by its first string. There is no `for_source`:
/// every store here is first written by a flush, which sizes its own first allocation
/// (`tsv_lang::store`'s `FIRST_ALLOC_BYTES`).
pub struct CssStores {
    pub(crate) runs: RunStores,
    pub(crate) text: TextPool<CssText>,
}

impl CssStores {
    /// Empty stores for one stylesheet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            runs: RunStores::new(),
            text: TextPool::new(),
        }
    }

    /// Rewinds the stores in place between stylesheets. Every store once written keeps its
    /// slot 0, so the next stylesheet's first write to it mints straight behind it; every
    /// store, scratch stack and the pool keep their capacity. Every run and pool text minted
    /// so far is invalid from here on.
    ///
    /// Nothing is ever trimmed. The capacity a holder's largest stylesheet grew stays parked
    /// for as long as the holder keeps the stores — what lets a warm loop parse without
    /// allocating — and dropping the stores is what releases it: a `Formatter` dropped, a
    /// binding's thread retired, a WASM instance replaced.
    #[inline]
    pub fn reset(&mut self) {
        // destructured whole, so a field added to the stores without a reset here is a build
        // error rather than growth across stylesheets
        let Self { runs, text } = self;
        runs.reset();
        text.reset();
    }

    /// The current tops of the scratch stacks, every height checked against the `u32` a mark
    /// holds by one [`MarkGroup`].
    #[inline]
    pub(crate) fn scratch_marks(&self) -> CssScratchMarks {
        let mut group = MarkGroup::new();
        let marks = self.runs.marks(&mut group);
        group.check();
        marks
    }

    /// Truncates the scratch stacks to `marks`, discarding the frames opened since. The
    /// stores keep the rows flushed since: an abandoned run is an orphan.
    #[inline]
    pub(crate) fn truncate_scratch(&mut self, marks: &CssScratchMarks) {
        self.runs.truncate(marks);
    }

    /// Empties every scratch stack, discarding the frames a failed parse left open. The
    /// stores keep their rows, as [`CssStores::truncate_scratch`] leaves them.
    ///
    /// Out of line and cold: it runs only behind a failed parse, and inlined it is a store
    /// per scratch stack at the parse driver's failure exit.
    #[cold]
    #[inline(never)]
    pub(crate) fn clear_scratch(&mut self) {
        // destructured whole, as `reset` is, so a scratch stack added to the stores beside the
        // generated ones is a build error here rather than a frame a failed parse leaves open
        let Self { runs, text: _ } = self;
        runs.clear_scratch();
    }

    /// Whether every scratch stack is empty, as the parse driver finds and leaves them.
    #[cfg(debug_assertions)]
    pub(crate) fn scratch_is_empty(&self) -> bool {
        let Self { runs, text: _ } = self;
        runs.scratch_is_empty()
    }

    /// Every field's shape, for the tests that read what a reset leaves. Destructured whole,
    /// as [`CssStores::reset`] is, so no field is missed.
    #[cfg(test)]
    fn shapes(&self) -> Vec<FieldShape> {
        let Self { runs, text } = self;
        let mut out = vec![FieldShape::pool("text", text)];
        runs.shapes(&mut out);
        out
    }
}

impl Default for CssStores {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CssStores {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CssStores").finish_non_exhaustive()
    }
}

/// A parsed stylesheet and the stores its handles index.
///
/// Derefs to the [`CssStyleSheet`] for the tree's own fields. A reading entry point takes
/// `&parsed`, which is also where it gets the view the tree's handles resolve through.
#[derive(Debug)]
pub struct ParsedCss {
    /// The tree.
    pub stylesheet: CssStyleSheet,
    /// The stores the tree's handles index.
    pub stores: CssStores,
}

impl Deref for ParsedCss {
    type Target = CssStyleSheet;

    fn deref(&self) -> &CssStyleSheet {
        &self.stylesheet
    }
}

thread_local! {
    /// This thread's parked stores. Boxed, so a take and a park each move a pointer, and a
    /// holder that keeps them in a struct of its own between the two (a Svelte document's
    /// stores) keeps the allocation this slot hands over.
    static PARKED: Cell<Option<Box<CssStores>>> = const { Cell::new(None) };
}

/// Takes this thread's parked stores out of their slot, or fresh ones when there are none.
/// The slot is empty until [`park_stores`] hands them back, so a second take meanwhile, and a
/// thread that never parks them (a panic unwinding past the holder), get fresh stores.
///
/// The half of [`with_stores`] a holder that keeps the stores in a struct of its own calls:
/// it owns the box for as long as it likes, and the thread still parks one store set.
#[must_use]
pub fn take_stores() -> Box<CssStores> {
    PARKED.with(Cell::take).unwrap_or_default()
}

/// Parks `stores` in this thread's slot after a [`CssStores::reset`], keeping slot 0 and
/// capacity, so every run and pool text minted from them is invalid from here on. Stores
/// already in the slot are dropped.
pub fn park_stores(mut stores: Box<CssStores>) {
    stores.reset();
    PARKED.with(|cell| cell.set(Some(stores)));
}

/// Run `f` over this thread's parked stores, or over fresh ones when there are none:
/// `tsv_arena`'s take-and-park protocol, here because that crate cannot name these types.
///
/// The stores are out of the slot while `f` runs ([`take_stores`]), so a nested call takes
/// fresh ones and a panic out of `f` leaves the slot empty, as a new thread finds it. Parked
/// after a [`CssStores::reset`] ([`park_stores`]), keeping slot 0 and capacity, so every run
/// and pool text `f` minted is invalid once it returns.
pub fn with_stores<R>(f: impl FnOnce(&mut CssStores) -> R) -> R {
    let mut stores = take_stores();
    let result = f(&mut stores);
    park_stores(stores);
    result
}

/// Stores a caller keeps warm across calls in a slot it owns, rewound between them: the
/// take-and-park protocol of [`with_stores`] over the caller's slot instead of the thread's, as
/// the `Formatter` keeps its stores. The slot holds a box, so a take and a park each move a
/// pointer.
///
/// A holder names no store type, so its code reads the same whatever the stores hold.
#[derive(Debug, Default)]
pub struct WarmStores {
    parked: Option<Box<CssStores>>,
}

impl WarmStores {
    /// Takes the parked stores out of the slot, or fresh ones when there are none. The slot is
    /// empty until [`WarmStores::park`] hands them back.
    #[must_use]
    pub fn take(&mut self) -> Box<CssStores> {
        self.parked.take().unwrap_or_default()
    }

    /// Parks `stores` in the slot after a [`CssStores::reset`], keeping slot 0 and capacity,
    /// so every run and pool text minted from them is invalid from here on.
    pub fn park(&mut self, mut stores: Box<CssStores>) {
        stores.reset();
        self.parked = Some(stores);
    }

    /// Run `f` over the parked stores, or over fresh ones when there are none, and park them
    /// after a [`CssStores::reset`], keeping slot 0 and capacity, so every run and pool text
    /// `f` minted is invalid once it returns. A panic out of `f` leaves the slot empty, and the
    /// next call starts from fresh stores.
    pub fn with<R>(&mut self, f: impl FnOnce(&mut CssStores) -> R) -> R {
        let mut stores = self.take();
        let result = f(&mut stores);
        self.park(stores);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{CssAtrule, CssBlockChild, CssNode, PreludeValue, PseudoClassArgs};
    use tsv_lang::store::PoolText;
    use tsv_lang::value_runs::FieldKind;

    /// A stylesheet whose parse writes every store: a top-level at-rule over a rule (the
    /// nodes and the block children), its `@supports` condition with a `selector()` argument
    /// (the condition parts and segments, and the three selector stores), a `::part()` (the
    /// part spans), a declaration whose value is a list holding a function (the values), and
    /// an escaped property and string (the pool).
    const DOCUMENT: &str =
        "@supports (a: b) and selector(c > d) { e::part(f) { c\\6flor: g(1, 2) 'h\\41'; } }\n";

    /// One element of every list type, the first of each list [`DOCUMENT`] holds: what a
    /// test leaves on a scratch stack, as a parse cut short leaves its open frames.
    struct OneOfEach {
        node: CssNode,
        block_child: CssBlockChild,
        complex_selector: ComplexSelector,
        relative_selector: RelativeSelector,
        simple_selector: SimpleSelector,
        part_span: Span,
        value: CssValue,
        condition_part: ConditionPart,
        condition_segment: ConditionSegment,
    }

    /// The first element of each list of [`DOCUMENT`]'s parse.
    fn one_of_each() -> OneOfEach {
        let parsed = crate::__private::parse(DOCUMENT).expect("the document parses");
        let view = parsed.view();
        let node = view.nodes(parsed.nodes)[0];
        let CssNode::Atrule(CssAtrule {
            prelude: PreludeValue::Supports { condition, .. },
            block: Some(block),
            ..
        }) = &node
        else {
            panic!("an unexpected node: {node:?}");
        };
        let condition_part = view.condition_parts(condition.parts)[0];
        let condition_segment = view.condition_segments(condition_part.segments)[0];
        let block_child = view.block_children(block.children)[0];
        let CssBlockChild::Rule(rule) = &block_child else {
            panic!("an unexpected child: {block_child:?}");
        };
        let complex_selector = view.complex_selectors(rule.selector.selectors)[0];
        let relative_selector = view.relative_selectors(complex_selector.children)[0];
        let simple_selectors = view.simple_selectors(relative_selector.selectors);
        let simple_selector = simple_selectors[0];
        let Some(&SimpleSelector::PseudoElement {
            args: Some(PseudoClassArgs::Part { ident_spans, .. }),
            ..
        }) = simple_selectors.last()
        else {
            panic!("an unexpected compound: {simple_selectors:?}");
        };
        let part_span = view.part_spans(ident_spans)[0];
        let CssBlockChild::Declaration(declaration) = &view.block_children(rule.declarations)[0]
        else {
            panic!("an unexpected declaration list");
        };
        let CssValue::List { values, .. } = declaration.value else {
            panic!("an unexpected value: {:?}", declaration.value);
        };
        let value = view.values(values)[0];
        OneOfEach {
            node,
            block_child,
            complex_selector,
            relative_selector,
            simple_selector,
            part_span,
            value,
            condition_part,
            condition_segment,
        }
    }

    /// Pushes one element onto every scratch stack, each left open.
    fn push_one_of_each(stores: &mut CssStores, rows: &OneOfEach) {
        let runs = &mut stores.runs;
        runs.node_scratch.push(rows.node);
        runs.block_child_scratch.push(rows.block_child);
        runs.complex_selector_scratch.push(rows.complex_selector);
        runs.relative_selector_scratch.push(rows.relative_selector);
        runs.simple_selector_scratch.push(rows.simple_selector);
        runs.part_span_scratch.push(rows.part_span);
        runs.value_scratch.push(rows.value);
        runs.condition_part_scratch.push(rows.condition_part);
        runs.condition_segment_scratch.push(rows.condition_segment);
    }

    /// Writes every store, scratch stack and the pool of `stores`: a sizeable parse, then one
    /// element left on every scratch stack, as a parse cut short leaves it, and one string
    /// pushed last, whose handle is returned.
    fn fill_every_field(stores: &mut CssStores) -> PoolText<CssText> {
        let rows = one_of_each();
        let document = DOCUMENT.repeat(16);
        crate::__private::parse_into(&document, stores).expect("the document parses");
        push_one_of_each(stores, &rows);
        stores.text.push("a pooled string")
    }

    /// Asserts `filled` wrote every field, so a field nothing fills cannot pass a rewind test
    /// by standing empty: a store past slot 0, a scratch stack holding an element, the pool
    /// past its sentinel — every store the value-run macro declares among them.
    fn assert_every_field_written(filled: &[FieldShape]) {
        let unwritten: Vec<_> = filled
            .iter()
            .filter(|shape| match shape.kind {
                FieldKind::Store | FieldKind::Pool => shape.len <= 1,
                FieldKind::Scratch => shape.len == 0,
            })
            .map(|shape| shape.name)
            .collect();
        assert!(
            unwritten.is_empty(),
            "the fill leaves these unwritten: {unwritten:?}"
        );
        for field in RunStores::FIELDS {
            assert!(
                filled.iter().any(|shape| shape.name == *field),
                "{field} has no shape"
            );
        }
    }

    fn assert_rewound(filled: &[FieldShape], rewound: &[FieldShape]) {
        assert_eq!(filled.len(), rewound.len());
        for (before, after) in filled.iter().zip(rewound) {
            assert_eq!(before.name, after.name);
            assert_eq!(after.len, before.rewound_len(), "{}", before.name);
            assert_eq!(
                after.capacity, before.capacity,
                "{} keeps its capacity",
                before.name
            );
        }
    }

    /// The scratch stacks' names and heights.
    fn scratch_heights(stores: &CssStores) -> Vec<(&'static str, usize)> {
        stores
            .shapes()
            .into_iter()
            .filter(|shape| shape.kind == FieldKind::Scratch)
            .map(|shape| (shape.name, shape.len))
            .collect()
    }

    fn assert_scratch_empty(stores: &CssStores) {
        let open: Vec<_> = scratch_heights(stores)
            .into_iter()
            .filter(|&(_, len)| len != 0)
            .collect();
        assert!(open.is_empty(), "open scratch frames: {open:?}");
        #[cfg(debug_assertions)]
        assert!(stores.scratch_is_empty());
    }

    /// Where `stores` live: a take that hands back the allocation a park left reads the same
    /// address.
    fn address(stores: &CssStores) -> *const CssStores {
        std::ptr::from_ref(stores)
    }

    #[test]
    fn fresh_stores_allocate_nothing() {
        let stores = CssStores::new();
        assert!(stores.shapes().iter().all(|shape| shape.capacity == 0));
        assert!(stores.shapes().iter().all(|shape| shape.len == 0));
    }

    #[test]
    fn a_reset_rewinds_every_field() {
        let mut stores = CssStores::new();
        let text = fill_every_field(&mut stores);
        assert_eq!(stores.view().text(text), "a pooled string");
        let filled = stores.shapes();
        assert_every_field_written(&filled);
        stores.reset();
        assert_rewound(&filled, &stores.shapes());
        // the next stylesheet's first string lands where the last one's did
        let again = fill_every_field(&mut stores);
        assert_eq!(stores.view().text(again), "a pooled string");
        assert_eq!(stores.shapes(), filled);
    }

    /// `clear_scratch` empties every scratch stack the stores hold, one left open on each,
    /// and touches nothing else: every store and the pool keep their rows, every buffer its
    /// capacity.
    #[test]
    fn clear_scratch_empties_every_scratch_stack_and_nothing_else() {
        let mut stores = CssStores::new();
        fill_every_field(&mut stores);
        let filled = stores.shapes();
        assert_every_field_written(&filled);
        #[cfg(debug_assertions)]
        assert!(!stores.scratch_is_empty());
        stores.clear_scratch();
        assert_scratch_empty(&stores);
        let expected: Vec<FieldShape> = filled
            .iter()
            .map(|&shape| match shape.kind {
                FieldKind::Scratch => FieldShape { len: 0, ..shape },
                FieldKind::Store | FieldKind::Pool => shape,
            })
            .collect();
        assert_eq!(stores.shapes(), expected);
    }

    /// `truncate_scratch` rewinds every scratch stack to its mark, more than once, and leaves
    /// the element below each mark.
    #[test]
    fn truncate_scratch_rewinds_every_scratch_stack_to_its_mark() {
        let mut stores = CssStores::new();
        fill_every_field(&mut stores);
        let marked = scratch_heights(&stores);
        assert!(marked.iter().all(|&(_, len)| len == 1), "{marked:?}");
        let marks = stores.scratch_marks();
        let rows = one_of_each();
        for _ in 0..2 {
            push_one_of_each(&mut stores, &rows);
            let grown = scratch_heights(&stores);
            assert!(grown.iter().all(|&(_, len)| len == 2), "{grown:?}");
            stores.truncate_scratch(&marks);
            assert_eq!(scratch_heights(&stores), marked);
        }
    }

    /// A parse that fails with lists open — a block's children, a selector list in a
    /// pseudo-class argument, a `::part()`'s names, a value list, a condition's parts and
    /// segments, an `@import` function's arguments — leaves no frame behind, so the next
    /// parse into the same stores starts over empty scratch stacks.
    #[test]
    fn a_failed_parse_leaves_every_scratch_stack_empty() {
        let mut stores = CssStores::new();
        for source in [
            "a { b: c; d { e: f(g, h); i",
            "@media (x) { a, b:is(.c, d > e",
            "a::part(b c",
            "@supports (a: b) and selector(c > d) { e { f: g h",
            "@import url(a) supports((b: c)",
            "a, b > c d:not(e f",
        ] {
            crate::__private::parse_into(source, &mut stores).expect_err(source);
            assert_scratch_empty(&stores);
        }
        crate::__private::parse_into(DOCUMENT, &mut stores).expect("parses");
        assert_scratch_empty(&stores);
    }

    /// A sub-parse the parse recovers from — an item of a forgiving list, an unknown
    /// pseudo-class's argument, a `selector()` argument, a `@custom-selector` list — fails
    /// with a compound already pushed (the `.b` of `.b .`), and the recovery discards it: the
    /// complex selector around the pseudo-class keeps its one compound, and the parse ends
    /// with every scratch stack empty.
    #[test]
    fn a_recovered_failure_leaves_no_element_behind() {
        for source in [
            "a:is(.b, .c ., .d) {}",
            "a:foo(.c .) {}",
            "@supports selector(.c .) { a {} }",
            "@custom-selector :--x .c .; a {}",
        ] {
            let mut stores = CssStores::new();
            let sheet = crate::__private::parse_into(source, &mut stores).expect(source);
            assert_scratch_empty(&stores);
            let view = stores.view();
            let rule = match view.nodes(sheet.nodes).last() {
                Some(CssNode::Rule(rule)) => rule,
                Some(CssNode::Atrule(CssAtrule {
                    block: Some(block), ..
                })) => match view.block_children(block.children) {
                    [CssBlockChild::Rule(rule)] => rule,
                    children => panic!("{source:?}: an unexpected block: {children:?}"),
                },
                node => panic!("{source:?}: an unexpected node: {node:?}"),
            };
            let [complex] = view.complex_selectors(rule.selector.selectors) else {
                panic!("{source:?}: not one complex selector");
            };
            assert_eq!(
                view.relative_selectors(complex.children).len(),
                1,
                "{source:?}: the rule's selector holds one compound"
            );
        }
    }

    /// The parse driver's entry check, in a build that carries it: a parse handed stores
    /// with a frame open is a caller's bug.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "a parse began over an open list frame")]
    fn a_parse_over_an_open_frame_is_refused() {
        let mut stores = CssStores::new();
        stores.runs.part_span_scratch.push(Span::new(0, 0));
        let _ = crate::__private::parse_into("a {}", &mut stores);
    }

    #[test]
    fn a_warm_handle_parks_its_stores_with_their_capacity() {
        let mut warm = WarmStores::default();
        let filled = warm.with(|stores| {
            fill_every_field(stores);
            stores.shapes()
        });
        assert_every_field_written(&filled);
        let rewound = warm.with(|stores| stores.shapes());
        assert_rewound(&filled, &rewound);
    }

    #[test]
    fn with_stores_parks_its_stores_with_their_capacity() {
        let filled = with_stores(|stores| {
            fill_every_field(stores);
            stores.shapes()
        });
        assert_every_field_written(&filled);
        let rewound = with_stores(|stores| stores.shapes());
        assert_rewound(&filled, &rewound);
    }

    #[test]
    fn a_warm_handle_takes_and_parks_one_allocation() {
        let mut warm = WarmStores::default();
        let mut stores = warm.take();
        fill_every_field(&mut stores);
        let filled = stores.shapes();
        let at = address(&stores);
        warm.park(stores);
        let stores = warm.take();
        assert_eq!(
            address(&stores),
            at,
            "the park and the take move the box, never the stores"
        );
        assert_rewound(&filled, &stores.shapes());
        let fresh = warm.take();
        assert_ne!(
            address(&fresh),
            at,
            "the slot is empty while its stores are out"
        );
        assert!(fresh.shapes().iter().all(|shape| shape.capacity == 0));
    }

    #[test]
    fn the_thread_slot_takes_and_parks_one_allocation() {
        let mut stores = take_stores();
        fill_every_field(&mut stores);
        let filled = stores.shapes();
        let at = address(&stores);
        park_stores(stores);
        with_stores(|stores| {
            assert_eq!(
                address(stores),
                at,
                "`with_stores` runs over the box the park left"
            );
            assert_rewound(&filled, &stores.shapes());
            let fresh = take_stores();
            assert_ne!(
                address(&fresh),
                at,
                "the slot is empty while its stores are out"
            );
        });
        assert_eq!(
            address(&take_stores()),
            at,
            "the stores come back through the slot"
        );
    }

    /// Whether `read` is refused as the store-check build refuses a handle from another
    /// generation; an unstamped build reads it in bounds instead.
    fn assert_rejected_under_the_store_check(read: impl FnOnce()) {
        let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(read));
        if tsv_lang::store::STORE_CHECK {
            let message = read.expect_err("a retained handle is rejected");
            let message = message
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| message.downcast_ref::<&str>().copied())
                .expect("a panic message");
            assert!(message.contains("store stamp mismatch"), "{message}");
        } else {
            assert!(read.is_ok(), "unstamped, a retained handle reads in bounds");
        }
    }

    /// A pool text kept past the park reads the next stylesheet's text in bounds; the
    /// store-check build rejects it, because the reset advances the pool's generation.
    #[test]
    fn a_handle_retained_across_a_park_is_rejected_under_the_store_check() {
        let mut warm = WarmStores::default();
        let text = warm.with(|stores| {
            stores.text.push("a");
            stores.text.push("b")
        });
        assert_rejected_under_the_store_check(|| {
            warm.with(|stores| {
                stores.text.push("cd");
                stores.view().text(text);
            });
        });
    }

    #[test]
    fn with_stores_hands_out_rewound_stores() {
        let filled = with_stores(|stores| {
            fill_every_field(stores);
            stores.shapes()
        });
        with_stores(|stores| {
            assert_rewound(&filled, &stores.shapes());
        });
    }

    #[test]
    fn with_stores_recovers_after_a_caught_panic() {
        // warm the slot, so the panic has parked stores to lose
        let at = with_stores(|stores| {
            fill_every_field(stores);
            address(stores)
        });
        // asserted: a scratch stack's live marks are a `Cell` in the store-check build
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_stores(|stores| {
                assert_eq!(address(stores), at);
                panic!("boom");
            });
        }));
        assert!(caught.is_err());
        // the slot is empty, as a new thread finds it: the next call builds fresh stores
        with_stores(|stores| {
            assert!(stores.shapes().iter().all(|shape| shape.capacity == 0));
            fill_every_field(stores);
        });
    }

    #[test]
    fn a_warm_handle_recovers_after_a_caught_panic() {
        let mut warm = WarmStores::default();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            warm.with(|stores| {
                fill_every_field(stores);
                panic!("boom");
            });
        }));
        assert!(caught.is_err());
        warm.with(|stores| {
            assert!(stores.shapes().iter().all(|shape| shape.capacity == 0));
        });
    }

    #[test]
    fn a_nested_with_stores_takes_fresh_stores() {
        with_stores(|outer| {
            fill_every_field(outer);
            let filled = outer.shapes();
            with_stores(|inner| {
                assert_ne!(address(inner), address(outer));
                assert!(
                    inner.shapes().iter().all(|shape| shape.len == 0),
                    "not the stores already in use"
                );
            });
            assert_eq!(outer.shapes(), filled, "the outer stores are untouched");
        });
    }

    /// A warm handle's parse gives what fresh stores give, across a failed parse.
    #[test]
    fn a_warm_handle_parses_as_fresh_stores_do() {
        let sources = [
            "a { color: red; }",
            "a {",
            "@media (min-width: 1px) { b { margin: 0 auto; } }",
            "c { content: '\\41'; }",
            "",
        ];
        let mut warm = WarmStores::default();
        for source in sources {
            let reused = warm.with(|stores| {
                crate::__private::parse_into(source, stores)
                    .map(|sheet| format!("{sheet:?}"))
                    .map_err(|e| format!("{e:#}"))
            });
            let fresh = crate::__private::parse(source)
                .map(|parsed| format!("{:?}", parsed.stylesheet))
                .map_err(|e| format!("{e:#}"));
            assert_eq!(reused, fresh, "{source:?}");
        }
    }
}
