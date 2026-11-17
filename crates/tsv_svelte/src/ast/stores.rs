//! A Svelte document's stores — its own, and the two language crates' it is composed of — and
//! their parking.
//!
//! [`SvelteStores`] holds three halves. The TypeScript half holds the rows every `<script>`
//! and template expression of the document parses into, and the CSS half the stores of its
//! top-level `<style>`; each is the box its own crate's slot hands over, so a document's
//! stores are taken from and parked in the slots a TypeScript or a CSS document on the same
//! thread uses, and a thread holds one TypeScript store set and one CSS store set whatever mix
//! of languages it parses. The third, [`SvelteOwn`], holds the template's own lists and
//! strings: one store per element type of a list — a fragment's nodes, an element's
//! attributes, an attribute's value parts, a directive's modifier spans, the wire's
//! `{#snippet}` parameter lists — each with the scratch stack that builds its lists, which
//! `tsv_lang`'s `value_runs!` declares from one entry per element type; and a text pool, for
//! the one string the template keeps as text of its own. It has a slot of its own in this
//! crate.
//!
//! Every list element type is `Copy`, so a finished list is copied into its store as a
//! block.
//!
//! Between parses every scratch stack is empty: the parse driver leaves them so, whether the
//! parse succeeded or failed, which is what lets the next parse begin without marking them.
//!
//! Handles die with the parse that minted them: nothing id-, run- or pool-text-shaped leaves
//! [`with_stores`]'s closure or a `ParsedRoot`, whose stores it keeps beside the tree they
//! belong to.

use std::cell::Cell;
use std::fmt;

use tsv_css::ast::CssStores;
use tsv_lang::Span;
use tsv_lang::store::TextPool;
#[cfg(test)]
use tsv_lang::value_runs::FieldShape;
use tsv_ts::ast::AstStores;

use super::{AttributeNode, AttributeValue, FragmentNode, SnippetWireParameters, SvelteView};

/// The family tag of a document's text pool: the strings the template keeps as text of its
/// own rather than as a span of the source.
pub enum SvelteText {}

// A handle keeps its niche, so an absent string costs nothing beside a present one.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::{PoolText, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<PoolText<SvelteText>>() == 8);
    assert!(STORE_CHECK || size_of::<Option<PoolText<SvelteText>>>() == 8);
};

tsv_lang::value_runs! {
    /// The stores of a document's template lists, one per element type, and the scratch
    /// stacks that build them.
    pub(crate) struct SvelteRuns;
    /// The tops of every scratch stack, taken together.
    pub(crate) struct SvelteScratchMarks;
    pub view SvelteView::runs;

    /// A fragment's nodes: [`FragmentNode`] rows.
    nodes, node_scratch: FragmentNode => nodes;
    /// An element's, a special element's, `<svelte:options>`'s, a `<script>`'s or a
    /// `<style>`'s attributes and directives: [`AttributeNode`] rows.
    attributes, attribute_scratch: AttributeNode => attributes;
    /// An attribute's or a style directive's value parts: [`AttributeValue`] rows.
    attribute_values, attribute_value_scratch: AttributeValue => attribute_values;
    /// The spans of a directive's modifiers: [`Span`] rows.
    modifiers, modifier_scratch: Span as Row => modifiers;
    /// The wire's parameter lists of the `{#snippet}` heads that hold a grouping pair:
    /// [`SnippetWireParameters`] rows.
    wire_params, wire_param_scratch: SnippetWireParameters => wire_params;
}

// Every list is a `Run<F>`, which lays out alike for every family `F`, and an absent list folds
// into the run's start.
#[cfg(test)]
const _: () = {
    use tsv_lang::store::{Run, STORE_CHECK};
    assert!(STORE_CHECK || size_of::<Run<AttributeValue>>() == 8);
    assert!(STORE_CHECK || size_of::<Option<Run<AttributeValue>>>() == 8);
};

/// A document's own lists and strings: the template's list stores, the scratch stacks that
/// build their lists, and its text pool.
///
/// A fresh one allocates nothing: a store's buffer is allocated by its first write, a scratch
/// stack's by its first push, and the pool's by its first string.
pub(crate) struct SvelteOwn {
    pub(crate) runs: SvelteRuns,
    pub(crate) text: TextPool<SvelteText>,
}

impl SvelteOwn {
    /// Empty stores for one document's template.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            runs: SvelteRuns::new(),
            text: TextPool::new(),
        }
    }

    /// Rewinds the stores in place between documents. Every store once written keeps its
    /// slot 0, so the next document's first write to it mints straight behind it; every
    /// store, scratch stack and the pool keep their capacity. Every run and pool text minted
    /// so far is invalid from here on.
    ///
    /// Nothing is ever trimmed. The capacity a holder's largest document grew stays parked
    /// for as long as the holder keeps the stores — what lets a warm loop parse without
    /// allocating — and dropping the stores is what releases it: a `Formatter` dropped, a
    /// binding's thread retired, a WASM instance replaced. The TypeScript and CSS halves keep
    /// the same rule.
    #[inline]
    pub(crate) fn reset(&mut self) {
        // destructured whole, so a field added to the stores without a reset here is a build
        // error rather than growth across documents
        let Self { runs, text } = self;
        runs.reset();
        text.reset();
    }

    /// Empties every scratch stack, discarding the frames a failed parse left open. The
    /// stores keep their rows.
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
    /// as [`SvelteOwn::reset`] is, so no field is missed.
    #[cfg(test)]
    pub(crate) fn shapes(&self) -> Vec<FieldShape> {
        let Self { runs, text } = self;
        let mut out = vec![FieldShape::pool("text", text)];
        runs.shapes(&mut out);
        out
    }
}

impl Default for SvelteOwn {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SvelteOwn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SvelteOwn").finish_non_exhaustive()
    }
}

thread_local! {
    /// This thread's parked own half. Boxed, so a take and a park each move a pointer, and a
    /// holder that keeps it in a [`SvelteStores`] between the two keeps the allocation this
    /// slot hands over.
    static PARKED: Cell<Option<Box<SvelteOwn>>> = const { Cell::new(None) };
}

/// One document's stores: a TypeScript half and a CSS half, each a box its own crate's slot
/// hands over, and the template's own half, a box this crate's slot hands over.
///
/// Fresh ones allocate the three boxes and nothing beside them: each store's buffer is
/// allocated by its first write ([`SvelteStores::for_source`] also reserves the TypeScript
/// half's busiest stores).
#[derive(Debug, Default)]
pub struct SvelteStores {
    /// The rows of the document's embedded TypeScript, one instance across its islands.
    pub(crate) ts: Box<AstStores>,
    /// The stores of the document's top-level `<style>`.
    pub(crate) css: Box<CssStores>,
    /// The template's own lists and strings.
    pub(crate) own: Box<SvelteOwn>,
}

impl SvelteStores {
    /// Empty stores for one document.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty stores for one document of `source_len` bytes, the TypeScript half's busiest
    /// stores reserved for that length (`tsv_ts::ast::AstStores::for_source`): the stores of a
    /// one-shot call. The document's length bounds the bytes of its scripts and template
    /// expressions, so it sizes the half from above. The CSS half and the template's own
    /// stores are each first written by a flush, which sizes that first allocation itself.
    #[must_use]
    pub(crate) fn for_source(source_len: usize) -> Self {
        Self {
            ts: Box::new(AstStores::for_source(source_len)),
            css: Box::default(),
            own: Box::default(),
        }
    }

    /// Rewinds the three halves in place between documents, each keeping its capacity. Every
    /// id, run and pool text minted so far is invalid from here on.
    #[inline]
    pub fn reset(&mut self) {
        // destructured whole, so a half added without a reset here is a build error
        let Self { ts, css, own } = self;
        ts.reset();
        css.reset();
        own.reset();
    }
}

/// Takes this thread's parked stores out of the three slots — `tsv_ts`'s, `tsv_css`'s and this
/// crate's, in that order — or a fresh half where a slot holds none. The slots are empty until
/// [`park_stores`] hands the halves back, so a second take meanwhile, and a thread that never
/// parks them (a panic unwinding past the holder), get fresh stores.
#[must_use]
pub fn take_stores() -> SvelteStores {
    SvelteStores {
        ts: tsv_ts::__private::take_stores(),
        css: tsv_css::__private::take_stores(),
        own: PARKED.with(Cell::take).unwrap_or_default(),
    }
}

/// Parks each half in its own slot, rewound in place and keeping its capacity, so every id,
/// run and pool text minted from them is invalid from here on.
pub fn park_stores(stores: SvelteStores) {
    let SvelteStores { ts, css, mut own } = stores;
    tsv_ts::__private::park_stores(ts);
    tsv_css::__private::park_stores(css);
    own.reset();
    PARKED.with(|cell| cell.set(Some(own)));
}

/// Run `f` over this thread's parked stores, or over fresh ones where a slot holds none:
/// `tsv_arena`'s take-and-park protocol over the slots `tsv_ts`, `tsv_css` and this crate own.
///
/// The three halves are out of their slots while `f` runs ([`take_stores`]), so a nested call
/// takes fresh ones and a panic out of `f` leaves the slots empty, as a new thread finds them.
/// Parked rewound ([`park_stores`]), so every id, run and pool text `f` minted is invalid once
/// it returns.
pub fn with_stores<R>(f: impl FnOnce(&mut SvelteStores) -> R) -> R {
    let mut stores = take_stores();
    let result = f(&mut stores);
    park_stores(stores);
    result
}

/// Stores a caller keeps warm across calls in a slot it owns, rewound between them: the
/// take-and-park protocol of [`with_stores`] over the caller's slot instead of the thread's, as
/// the `Formatter` keeps its stores. The slot holds the three boxes, so a take and a park each
/// move three pointers.
#[derive(Debug, Default)]
pub struct WarmStores {
    parked: Option<SvelteStores>,
}

impl WarmStores {
    /// Takes the parked stores out of the slot, or fresh ones when there are none. The slot is
    /// empty until [`WarmStores::park`] hands them back.
    #[must_use]
    pub fn take(&mut self) -> SvelteStores {
        self.parked.take().unwrap_or_default()
    }

    /// Parks `stores` in the slot after a [`SvelteStores::reset`], keeping capacity, so every
    /// id, run and pool text minted from them is invalid from here on.
    pub fn park(&mut self, mut stores: SvelteStores) {
        stores.reset();
        self.parked = Some(stores);
    }

    /// Run `f` over the parked stores, or over fresh ones when there are none, and park them
    /// after a [`SvelteStores::reset`], keeping capacity, so every id, run and pool text `f`
    /// minted is invalid once it returns. A panic out of `f` leaves the slot empty, and the
    /// next call starts from fresh stores.
    pub fn with<R>(&mut self, f: impl FnOnce(&mut SvelteStores) -> R) -> R {
        let mut stores = self.take();
        let result = f(&mut stores);
        self.park(stores);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Attribute, FragmentNode, SvelteView};
    use tsv_lang::value_runs::FieldKind;

    /// Where each half lives: a take that hands back the allocations a park left reads the
    /// same three addresses.
    fn addresses(stores: &SvelteStores) -> (usize, usize, usize) {
        (
            ts_address(&stores.ts),
            css_address(&stores.css),
            own_address(&stores.own),
        )
    }

    fn own_address(stores: &SvelteOwn) -> usize {
        std::ptr::from_ref(stores) as usize
    }

    fn ts_address(stores: &AstStores) -> usize {
        std::ptr::from_ref(stores) as usize
    }

    fn css_address(stores: &CssStores) -> usize {
        std::ptr::from_ref(stores) as usize
    }

    /// A document with both islands, whose parse writes the TypeScript half.
    const DOCUMENT: &str =
        "<script>let x = [1, 2];</script>\n<p>{x}</p>\n<style>p { color: red; }</style>\n";

    /// The wire and the formatted output of `source`, or its error, through `stores`.
    fn outputs(source: &str, stores: &mut SvelteStores) -> Result<(Vec<u8>, String), String> {
        let root = crate::__private::parse_into(source, stores).map_err(|e| format!("{e:#}"))?;
        let parsed = crate::__private::parsed_ref(&root, stores);
        let wire = crate::__private::convert_ast_json_bytes(parsed, source);
        let formatted = crate::__private::format_ast(parsed, source);
        Ok((wire, formatted))
    }

    /// Where the TypeScript half mints next, as a parse's tree names it: the same text for
    /// fresh stores and rewound ones, another for stores still holding a document's rows.
    fn typescript_mint_mark(stores: &mut AstStores) -> String {
        format!("{:?}", tsv_ts::__private::parse_into("a;", stores))
    }

    #[test]
    fn the_mint_mark_tells_used_stores_from_fresh_ones() {
        let fresh = typescript_mint_mark(&mut AstStores::new());
        let mut used = SvelteStores::new();
        outputs(DOCUMENT, &mut used).expect("parses");
        assert_ne!(typescript_mint_mark(&mut used.ts), fresh);
    }

    /// Where the CSS half mints next, as a parse's tree names it: its runs and its property's
    /// pool text, the same for fresh stores and rewound ones, another for stores still holding
    /// a stylesheet's lists and strings.
    fn css_mint_mark(stores: &mut CssStores) -> String {
        format!(
            "{:?}",
            tsv_css::__private::parse_into("a { b: c(d, e); }", stores)
        )
    }

    #[test]
    fn the_css_mint_mark_tells_used_stores_from_fresh_ones() {
        let fresh = css_mint_mark(&mut CssStores::new());
        let mut used = SvelteStores::new();
        outputs(DOCUMENT, &mut used).expect("parses");
        assert_ne!(css_mint_mark(&mut used.css), fresh);
    }

    /// One warm handle over a run of documents gives what fresh stores give for each: across
    /// a document that fails to parse in its markup, in its `<script>` and in its `<style>`,
    /// across documents that fail with list frames open at every level (a fragment inside an
    /// element inside a block, an attribute list, a value, a directive's modifiers, a snippet
    /// head's wire parameters), across documents with and without a `<style>`, and with one
    /// nested in markup. The same run through the thread's slots.
    #[test]
    fn a_warm_handle_parses_as_fresh_stores_do() {
        let large = format!(
            "{}<style>a {{ margin: 0 auto; }}</style>\n",
            "<p>{a + b}</p>\n".repeat(200)
        );
        let sources = [
            DOCUMENT,
            "<p>{</p>",
            "<div><span class={a}><p>{#if x}",
            "<div on:click|once={",
            "{#snippet s(a = (1))}<p>",
            "<ul><li>a<li>b</ul>{#each xs as x}<p class=\"a {b}\" style:c=\"{d}\">",
            "<svelte:element class=\"a\" this={tag} id=\"b\"><p>{#key k}<span",
            "<style>a { color: red; } @media (min-width: 1px) { b { top: 0; } }</style>",
            "<style>a {</style>",
            "<script>let = ;</script><style>a { color: red; }</style>",
            "<div><style>b { color: blue; }</style></div>\n<style>c { top: 0; }</style>",
            large.as_str(),
            "<p>text only</p>",
            "<style>/* c */ d { content: '\\41'; }</style>",
            "",
        ];
        let mut warm = WarmStores::default();
        for source in sources {
            let reused = warm.with(|stores| outputs(source, stores));
            let fresh = outputs(source, &mut SvelteStores::new());
            assert_eq!(reused, fresh, "{source:?}");
        }
        for source in sources {
            let reused = with_stores(|stores| outputs(source, stores));
            let fresh = outputs(source, &mut SvelteStores::new());
            assert_eq!(reused, fresh, "{source:?}");
        }
    }

    /// Fresh allocations of the three halves' sizes, held across the next take: a box dropped
    /// where it should have been parked frees its address, which these take, so a fresh box
    /// cannot land on it and read as the parked one.
    fn decoys() -> (Box<AstStores>, Box<CssStores>, Box<SvelteOwn>) {
        (Box::default(), Box::default(), Box::default())
    }

    /// A TypeScript document, then a Svelte document, then a CSS document on one thread run
    /// over the same two language allocations, and a second Svelte document over the own
    /// half the first parked: the thread holds one TypeScript store set, one CSS store set
    /// and one own half.
    #[test]
    fn a_thread_parks_one_typescript_and_one_css_store_set() {
        let ts = tsv_ts::__private::with_stores(|stores| {
            tsv_ts::__private::parse_into("const a = [b, c => d];", stores).expect("parses");
            ts_address(stores)
        });
        let _held = decoys();
        let svelte = with_stores(|stores| {
            outputs(DOCUMENT, stores).expect("parses");
            // the three slots are empty while the document holds their boxes
            let meanwhile = take_stores();
            assert_ne!(addresses(&meanwhile).0, addresses(stores).0);
            assert_ne!(addresses(&meanwhile).1, addresses(stores).1);
            assert_ne!(addresses(&meanwhile).2, addresses(stores).2);
            addresses(stores)
        });
        assert_eq!(svelte.0, ts, "the box the TypeScript document parked");
        let _held = decoys();
        let css = tsv_css::__private::with_stores(|stores| {
            tsv_css::__private::parse_into("a { color: red; }", stores).expect("parses");
            css_address(stores)
        });
        assert_eq!(css, svelte.1, "the box the Svelte document parked");
        let _held = decoys();
        // and back: a second Svelte document takes the two boxes the others parked, and the
        // own half the first Svelte document parked
        assert_eq!(with_stores(|stores| addresses(stores)), (ts, css, svelte.2));
        let _held = decoys();
        assert_eq!(
            tsv_ts::__private::with_stores(|stores| ts_address(stores)),
            ts
        );
        let _held = decoys();
        assert_eq!(
            tsv_css::__private::with_stores(|stores| css_address(stores)),
            css
        );
    }

    #[test]
    fn a_warm_handle_takes_and_parks_the_same_three_allocations() {
        let mut warm = WarmStores::default();
        let stores = warm.take();
        let at = addresses(&stores);
        warm.park(stores);
        let _held = decoys();
        let stores = warm.take();
        assert_eq!(
            addresses(&stores),
            at,
            "the park and the take move the boxes"
        );
        let fresh = warm.take();
        assert_ne!(addresses(&fresh).0, at.0, "the slot is empty meanwhile");
        assert_ne!(addresses(&fresh).1, at.1, "the slot is empty meanwhile");
        assert_ne!(addresses(&fresh).2, at.2, "the slot is empty meanwhile");
    }

    /// The park rewinds what a document wrote: the next parse mints where fresh stores do,
    /// through an owned slot and through the thread's.
    #[test]
    fn a_park_rewinds_the_typescript_half() {
        let fresh = typescript_mint_mark(&mut AstStores::new());
        let mut warm = WarmStores::default();
        warm.with(|stores| outputs(DOCUMENT, stores))
            .expect("parses");
        assert_eq!(
            warm.with(|stores| typescript_mint_mark(&mut stores.ts)),
            fresh
        );
        with_stores(|stores| outputs(DOCUMENT, stores)).expect("parses");
        assert_eq!(
            with_stores(|stores| typescript_mint_mark(&mut stores.ts)),
            fresh
        );
    }

    /// The park rewinds what a document's `<style>` wrote: the next stylesheet mints where
    /// fresh stores do, through an owned slot and through the thread's.
    #[test]
    fn a_park_rewinds_the_css_half() {
        let fresh = css_mint_mark(&mut CssStores::new());
        let mut warm = WarmStores::default();
        warm.with(|stores| outputs(DOCUMENT, stores))
            .expect("parses");
        assert_eq!(warm.with(|stores| css_mint_mark(&mut stores.css)), fresh);
        with_stores(|stores| outputs(DOCUMENT, stores)).expect("parses");
        assert_eq!(with_stores(|stores| css_mint_mark(&mut stores.css)), fresh);
    }

    /// A `<style>` nested in markup is parsed as the document formats, into stores of its own,
    /// and read through their view: its escaped property names are its own pool's text, beside
    /// the top-level `<style>`'s in the document's. Read through the document's view instead,
    /// a nested property would resolve against the other pool — a stamp mismatch under the
    /// store-check build, another stylesheet's bytes in bounds outside it.
    #[test]
    fn a_nested_style_reads_its_own_stores() {
        let source = "<style>p { c\\6flor: red; }</style>\n\
                      <div><style>a { content: '\\41'; c\\6flor: blue; }</style></div>\n";
        let expected = "<div>\n\t<style>\n\t\ta {\n\t\t\tcontent: '\\41';\n\t\t\tc\\6flor: blue;\n\t\t}\n\t</style>\n</div>\n\n<style>\n\tp {\n\t\tc\\6flor: red;\n\t}\n</style>\n";
        let (_, formatted) = outputs(source, &mut SvelteStores::new()).expect("parses");
        assert_eq!(formatted, expected);
        let mut warm = WarmStores::default();
        for _ in 0..2 {
            let (_, formatted) = warm.with(|stores| outputs(source, stores)).expect("parses");
            assert_eq!(formatted, expected);
        }
    }

    /// A panic out of the closure parks nothing: the stores it held are dropped with its
    /// frame, the three slots stay empty, and the next call in each of the three languages
    /// starts from stores holding none of what the panicking call wrote.
    #[test]
    fn with_stores_recovers_after_a_caught_panic() {
        let fresh = typescript_mint_mark(&mut AstStores::new());
        // warm the three slots, so the panic has parked stores to lose
        let at = with_stores(|stores| {
            outputs(DOCUMENT, stores).expect("parses");
            addresses(stores)
        });
        let caught = std::panic::catch_unwind(|| {
            with_stores(|stores| {
                assert_eq!(addresses(stores), at, "the warm boxes are in use");
                outputs(DOCUMENT, stores).expect("parses");
                panic!("boom");
            });
        });
        assert!(caught.is_err());
        assert_eq!(
            tsv_ts::__private::with_stores(typescript_mint_mark),
            fresh,
            "the TypeScript slot hands out clean stores"
        );
        let css = tsv_css::__private::with_stores(|stores| {
            tsv_css::__private::parse_into("a { color: red; }", stores)
                .map(|sheet| format!("{sheet:?}"))
                .map_err(|e| format!("{e:#}"))
        });
        let fresh_css = tsv_css::__private::parse("a { color: red; }")
            .map(|parsed| format!("{:?}", parsed.stylesheet))
            .map_err(|e| format!("{e:#}"));
        assert_eq!(css, fresh_css, "the CSS slot hands out clean stores");
        assert!(
            take_stores()
                .own
                .shapes()
                .iter()
                .all(|shape| shape.capacity == 0),
            "the own slot hands out fresh stores"
        );
        let reused = with_stores(|stores| outputs(DOCUMENT, stores));
        assert_eq!(reused, outputs(DOCUMENT, &mut SvelteStores::new()));
    }

    #[test]
    fn a_warm_handle_recovers_after_a_caught_panic() {
        let mut warm = WarmStores::default();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            warm.with(|stores| {
                outputs(DOCUMENT, stores).expect("parses");
                panic!("boom");
            });
        }));
        assert!(caught.is_err());
        assert!(warm.parked.is_none(), "the slot is empty after the panic");
        let reused = warm.with(|stores| outputs(DOCUMENT, stores));
        assert_eq!(reused, outputs(DOCUMENT, &mut SvelteStores::new()));
    }

    #[test]
    fn a_nested_with_stores_takes_fresh_stores() {
        with_stores(|outer| {
            let at = addresses(outer);
            with_stores(|inner| {
                assert_ne!(addresses(inner).0, at.0, "not the stores already in use");
                assert_ne!(addresses(inner).1, at.1, "not the stores already in use");
                assert_ne!(addresses(inner).2, at.2, "not the stores already in use");
            });
            assert_eq!(addresses(outer), at);
        });
    }

    /// A document whose parse writes every store of the own half and its pool: an element's
    /// attributes with a value and a directive's modifiers, fragments nested in an element
    /// and a block, a `{#snippet}` head holding a grouping pair (a wire parameter list) and
    /// generics (the pool).
    const OWN_DOCUMENT: &str = "<div class=\"a {b}\" on:click|once={c}>{#if d}<p>e</p>{/if}</div>\n\
         {#snippet s<T>(f = (1))}<span>{f}</span>{/snippet}\n";

    /// Writes every store, scratch stack and the pool of the own half: a sizeable parse, then
    /// one element left on every scratch stack, as a parse cut short leaves it.
    fn fill_own(stores: &mut SvelteStores) {
        let document = OWN_DOCUMENT.repeat(16);
        outputs(&document, stores).expect("the document parses");
        let parsed = crate::__private::parse(OWN_DOCUMENT).expect("the document parses");
        let view = parsed.view();
        let node = view.nodes(parsed.fragment.nodes)[0];
        let FragmentNode::Element(element) = &node else {
            panic!("an unexpected node: {node:?}");
        };
        let attribute = view.attributes(element.attributes)[0];
        let AttributeNode::Attribute(Attribute {
            value: Some(value), ..
        }) = &attribute
        else {
            panic!("an unexpected attribute: {attribute:?}");
        };
        let value = view.attribute_values(*value)[0];
        let wire = view.wire_params(parsed.snippet_wire_parameters)[0];
        let runs = &mut stores.own.runs;
        runs.node_scratch.push(node);
        runs.attribute_scratch.push(attribute);
        runs.attribute_value_scratch.push(value);
        runs.modifier_scratch.push(Span::new(0, 0));
        runs.wire_param_scratch.push(wire);
    }

    fn assert_every_own_field_written(filled: &[FieldShape]) {
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
        for field in SvelteRuns::FIELDS {
            assert!(
                filled.iter().any(|shape| shape.name == *field),
                "{field} has no shape"
            );
        }
    }

    fn assert_own_scratch_empty(stores: &SvelteStores) {
        let open: Vec<_> = stores
            .own
            .shapes()
            .into_iter()
            .filter(|shape| shape.kind == FieldKind::Scratch && shape.len != 0)
            .map(|shape| (shape.name, shape.len))
            .collect();
        assert!(open.is_empty(), "open scratch frames: {open:?}");
        #[cfg(debug_assertions)]
        assert!(stores.own.scratch_is_empty());
    }

    #[test]
    fn a_fresh_own_half_allocates_nothing() {
        let stores = SvelteOwn::new();
        assert!(
            stores
                .shapes()
                .iter()
                .all(|shape| (shape.len, shape.capacity) == (0, 0))
        );
    }

    /// The reset rewinds every field of the own half to its rewound length and keeps its
    /// capacity, through the stores' reset and through each slot's park.
    #[test]
    fn a_reset_rewinds_every_field_of_the_own_half() {
        let mut stores = SvelteStores::new();
        fill_own(&mut stores);
        let filled = stores.own.shapes();
        assert_every_own_field_written(&filled);
        stores.reset();
        let rewound = stores.own.shapes();
        for (before, after) in filled.iter().zip(&rewound) {
            assert_eq!(before.name, after.name);
            assert_eq!(after.len, before.rewound_len(), "{}", before.name);
            assert_eq!(
                after.capacity, before.capacity,
                "{} keeps its capacity",
                before.name
            );
        }
        let mut warm = WarmStores::default();
        let parked = warm.with(|stores| {
            fill_own(stores);
            stores.own.shapes()
        });
        let taken = warm.with(|stores| stores.own.shapes());
        for (before, after) in parked.iter().zip(&taken) {
            assert_eq!(after.len, before.rewound_len(), "{}", before.name);
            assert_eq!(after.capacity, before.capacity, "{}", before.name);
        }
        let parked = with_stores(|stores| {
            fill_own(stores);
            stores.own.shapes()
        });
        let taken = with_stores(|stores| stores.own.shapes());
        for (before, after) in parked.iter().zip(&taken) {
            assert_eq!(after.len, before.rewound_len(), "{}", before.name);
            assert_eq!(after.capacity, before.capacity, "{}", before.name);
        }
    }

    /// `clear_scratch` empties every scratch stack of the own half and touches nothing else.
    #[test]
    fn clear_scratch_empties_every_own_scratch_stack_and_nothing_else() {
        let mut stores = SvelteStores::new();
        fill_own(&mut stores);
        let filled = stores.own.shapes();
        assert_every_own_field_written(&filled);
        stores.own.clear_scratch();
        assert_own_scratch_empty(&stores);
        let expected: Vec<FieldShape> = filled
            .iter()
            .map(|&shape| match shape.kind {
                FieldKind::Scratch => FieldShape { len: 0, ..shape },
                FieldKind::Store | FieldKind::Pool => shape,
            })
            .collect();
        assert_eq!(stores.own.shapes(), expected);
    }

    /// A parse that fails with frames open — a fragment in an element in a block in an
    /// element, an attribute list, a value, a directive's modifiers, a snippet head's wire
    /// parameters, an implicitly closed element's siblings — leaves no frame behind, so the
    /// next parse into the same stores starts over empty scratch stacks.
    #[test]
    fn a_failed_parse_leaves_every_own_scratch_stack_empty() {
        let mut stores = SvelteStores::new();
        for source in [
            "<div><span class={a}><p>{#if x}",
            "<div on:click|once={",
            "<div on:click|once class=\"a {b",
            "{#snippet s(a = (1))}<p>",
            "<ul><li>a<li>b",
            "<svelte:element this={a} class=\"b\"><p>",
            "<textarea>a{b}",
            "{#await p}<p>{:then v}<span>{v}",
        ] {
            crate::__private::parse_into(source, &mut stores).expect_err(source);
            assert_own_scratch_empty(&stores);
        }
        crate::__private::parse_into(OWN_DOCUMENT, &mut stores).expect("parses");
        assert_own_scratch_empty(&stores);
    }

    /// The parse driver's entry check, in a build that carries it: a parse handed stores
    /// with a frame open is a caller's bug.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "a parse began over an open list frame")]
    fn a_parse_over_an_open_frame_is_refused() {
        let mut stores = SvelteStores::new();
        stores.own.runs.modifier_scratch.push(Span::new(0, 0));
        let _ = crate::__private::parse_into("<p></p>", &mut stores);
    }

    /// The tree's outline: each node's kind and, for an element, its name, attributes and
    /// children, read through the parsed root's view — what a frame left unflushed on a way
    /// out would change (a child's nodes becoming its parent's siblings).
    fn outline(parsed: &crate::ast::ParsedRoot, source: &str) -> String {
        fn nodes(view: SvelteView<'_>, list: &[FragmentNode], source: &str, out: &mut String) {
            for node in list {
                match node {
                    FragmentNode::Element(el) => {
                        out.push_str(el.name(source));
                        out.push('(');
                        for attribute in view.attributes(el.attributes) {
                            out.push_str(&source[attribute.span().range()]);
                            out.push(';');
                        }
                        out.push_str(")[");
                        nodes(view, view.nodes(el.fragment.nodes), source, out);
                        out.push(']');
                    }
                    FragmentNode::SpecialElement(el) => {
                        out.push_str(&source[el.name_span.range()]);
                        out.push('(');
                        for attribute in view.attributes(el.attributes) {
                            out.push_str(&source[attribute.span().range()]);
                            out.push(';');
                        }
                        out.push_str(")[");
                        nodes(view, view.nodes(el.fragment.nodes), source, out);
                        out.push(']');
                    }
                    FragmentNode::IfBlock(block) => {
                        out.push_str("if[");
                        nodes(view, view.nodes(block.consequent.nodes), source, out);
                        out.push_str("]else[");
                        if let Some(alternate) = &block.alternate {
                            nodes(view, view.nodes(alternate.nodes), source, out);
                        }
                        out.push(']');
                    }
                    FragmentNode::Text(text) => {
                        out.push('"');
                        out.push_str(text.raw(source));
                        out.push('"');
                    }
                    other => {
                        out.push_str(&source[other.span().range()]);
                    }
                }
                out.push(' ');
            }
        }
        let mut out = String::new();
        let view = parsed.view();
        nodes(view, view.nodes(parsed.fragment.nodes), source, &mut out);
        out
    }

    /// Every way a fragment frame closes with `Ok` flushes it as its own run: an element's
    /// children closed by the next opening tag, by an ancestor's closing tag and by its own,
    /// a `{:else if}`'s one-node alternate, and the attribute frames the special element's
    /// `this` splice edits. The same outline through a warm handle that just saw each
    /// document fail.
    #[test]
    fn every_ok_exit_flushes_its_own_frame() {
        let cases = [
            ("<ul><li>a<li>b</ul>", "ul()[li()[\"a\" ] li()[\"b\" ] ] "),
            ("<div><p>a</div>", "div()[p()[\"a\" ] ] "),
            ("<p>a<div>b</div>", "p()[\"a\" ] div()[\"b\" ] "),
            (
                "<table><tr><td>a<td>b</table>",
                "table()[tr()[td()[\"a\" ] td()[\"b\" ] ] ] ",
            ),
            (
                "{#if a}x{:else if b}y{:else}z{/if}",
                "if[\"x\" ]else[if[\"y\" ]else[\"z\" ] ] ",
            ),
            (
                "<div style:color=\"{c}\" style:top=\"1{a}px\" on:click|once|capture={f}></div>",
                "div(style:color=\"{c}\";style:top=\"1{a}px\";on:click|once|capture={f};)[] ",
            ),
            (
                "<svelte:element class=\"a\" this={tag} id=\"b\">x</svelte:element>",
                "svelte:element(class=\"a\";id=\"b\";)[\"x\" ] ",
            ),
            (
                "{#snippet s(a = (1))}{#snippet t(b = (2))}x{/snippet}{/snippet}",
                "{#snippet s(a = (1))}{#snippet t(b = (2))}x{/snippet}{/snippet} ",
            ),
        ];
        let mut warm = WarmStores::default();
        for (source, expected) in cases {
            let parsed = crate::__private::parse(source).expect(source);
            assert_eq!(outline(&parsed, source), expected, "{source:?}");
            let cut = &source[..source.len() - 2];
            let _ = warm.with(|stores| crate::__private::parse_into(cut, stores).map(|_| ()));
            let reused = warm.with(|stores| {
                let root = crate::__private::parse_into(source, stores).expect(source);
                let parsed = crate::ast::ParsedRoot {
                    root,
                    stores: std::mem::take(stores),
                };
                let outline = outline(&parsed, source);
                *stores = parsed.stores;
                outline
            });
            assert_eq!(reused, expected, "{source:?} through a warm handle");
        }
        let parsed = crate::__private::parse(
            "{#snippet s(a = (1))}{#snippet t(b = (2))}x{/snippet}{/snippet}",
        )
        .expect("parses");
        assert_eq!(
            parsed
                .view()
                .wire_params(parsed.snippet_wire_parameters)
                .len(),
            2
        );
    }

    /// The format paths that parse a whole document again — the lifted-section rewrite, the
    /// respelled template tail, the lone-`<CR>` refusal's parse of the author's bytes — each
    /// parse into stores of their own and read the result through those stores' view, never
    /// the host's: through a warm handle whose stores hold the host document's rows, each
    /// formats as fresh stores do. Under the store-check build a run, list or pool text read
    /// against the other instance fails its stamp.
    #[test]
    fn a_nested_document_parse_reads_its_own_stores() {
        let sources = [
            "<p class=\"a\">x</p>\n<script>let a = 1;</script>\n<p class=\"b\">y</p>\n",
            "{#snippet s<T>(a = (1))}<b>{a}</b>{/snippet}\nx\u{a0}<script>let a;</script>",
            "<div // c\rclass=\"x\">y</div>",
            "<div // c\rd\nclass=\"x\">y</div><script>let a;</script><p>z</p>",
            "<div // c\r\nclass=\"x\">y</div><script>let a;</script><span>z</span>",
        ];
        let mut warm = WarmStores::default();
        for source in sources {
            let fresh = crate::format(source).map_err(|e| format!("{e:#}"));
            let reused = warm.with(|stores| {
                crate::format_in(source, stores, tsv_lang::doc::arena::DocArena::for_source)
                    .map_err(|e| format!("{e:#}"))
            });
            assert_eq!(reused, fresh, "{source:?}");
        }
    }
}
