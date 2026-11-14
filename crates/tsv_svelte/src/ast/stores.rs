//! A Svelte document's stores, composed of the two language crates', and their parking.
//!
//! [`SvelteStores`] holds a TypeScript half, the rows every `<script>` and template
//! expression of the document parses into, and a CSS half, the stores of its top-level
//! `<style>`. Each half is the box its own crate's slot hands over, so a document's stores are
//! taken from and parked in the slots a TypeScript or a CSS document on the same thread
//! uses, and a thread holds one TypeScript store set and one CSS store set whatever mix of
//! languages it parses.
//!
//! Handles die with the parse that minted them: nothing id-shaped or pool-text-shaped leaves
//! [`with_stores`]'s closure or a `ParsedRoot`, whose stores it keeps beside the tree they
//! belong to.

use tsv_css::ast::{CssStores, CssView};
use tsv_ts::ast::{AstStores, AstView};

/// One document's stores: a TypeScript half and a CSS half, each a box its own crate's slot
/// hands over.
///
/// Fresh ones allocate the two boxes and nothing beside them: each store's buffer is
/// allocated by its first write.
#[derive(Debug, Default)]
pub struct SvelteStores {
    /// The rows of the document's embedded TypeScript, one instance across its islands.
    pub(crate) ts: Box<AstStores>,
    /// The stores of the document's top-level `<style>`.
    pub(crate) css: Box<CssStores>,
}

impl SvelteStores {
    /// Empty stores for one document.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Rewinds both halves in place between documents, each keeping its capacity. Every id,
    /// run and pool text minted so far is invalid from here on.
    #[inline]
    pub fn reset(&mut self) {
        // destructured whole, so a half added without a reset here is a build error
        let Self { ts, css } = self;
        ts.reset();
        css.reset();
    }

    /// The view reading the TypeScript half: the document's expressions and programs.
    #[inline]
    pub fn view(&self) -> AstView<'_> {
        self.ts.view()
    }

    /// The view reading the CSS half: the top-level `<style>`'s stylesheet.
    #[inline]
    pub fn css_view(&self) -> CssView<'_> {
        self.css.view()
    }
}

/// Takes this thread's parked stores out of both crates' slots, the TypeScript half first,
/// or a fresh half where a slot holds none. Both slots are empty until [`park_stores`] hands
/// the halves back, so a second take meanwhile, and a thread that never parks them (a panic
/// unwinding past the holder), get fresh stores.
#[must_use]
pub fn take_stores() -> SvelteStores {
    SvelteStores {
        ts: tsv_ts::__private::take_stores(),
        css: tsv_css::__private::take_stores(),
    }
}

/// Parks each half in its own crate's slot, rewound in place and keeping its capacity, so
/// every id, run and pool text minted from them is invalid from here on.
pub fn park_stores(stores: SvelteStores) {
    let SvelteStores { ts, css } = stores;
    tsv_ts::__private::park_stores(ts);
    tsv_css::__private::park_stores(css);
}

/// Run `f` over this thread's parked stores, or over fresh ones where a slot holds none:
/// `tsv_arena`'s take-and-park protocol over the slots `tsv_ts` and `tsv_css` own.
///
/// Both halves are out of their slots while `f` runs ([`take_stores`]), so a nested call
/// takes fresh ones and a panic out of `f` leaves both slots empty, as a new thread finds
/// them. Parked rewound ([`park_stores`]), so every id, run and pool text `f` minted is
/// invalid once it returns.
pub fn with_stores<R>(f: impl FnOnce(&mut SvelteStores) -> R) -> R {
    let mut stores = take_stores();
    let result = f(&mut stores);
    park_stores(stores);
    result
}

/// Stores a caller keeps warm across calls in a slot it owns, rewound between them: the
/// take-and-park protocol of [`with_stores`] over the caller's slot instead of the thread's, as
/// the `Formatter` keeps its stores. The slot holds the two boxes, so a take and a park each
/// move two pointers.
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

    /// Where each half lives: a take that hands back the allocations a park left reads the
    /// same pair of addresses.
    fn addresses(stores: &SvelteStores) -> (usize, usize) {
        (ts_address(&stores.ts), css_address(&stores.css))
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
        let arena = bumpalo::Bump::new();
        let root =
            crate::__private::parse_into(source, &arena, stores).map_err(|e| format!("{e:#}"))?;
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

    /// One warm handle over a run of documents gives what fresh stores give for each: across
    /// a document that fails to parse in its markup, in its `<script>` and in its `<style>`,
    /// across documents with and without a `<style>`, and with one nested in markup. The
    /// same run through the thread's slots.
    #[test]
    fn a_warm_handle_parses_as_fresh_stores_do() {
        let large = format!(
            "{}<style>a {{ margin: 0 auto; }}</style>\n",
            "<p>{a + b}</p>\n".repeat(200)
        );
        let sources = [
            DOCUMENT,
            "<p>{</p>",
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

    /// Fresh allocations of both halves' sizes, held across the next take: a box dropped
    /// where it should have been parked frees its address, which these take, so a fresh box
    /// cannot land on it and read as the parked one.
    fn decoys() -> (Box<AstStores>, Box<CssStores>) {
        (Box::default(), Box::default())
    }

    /// A TypeScript document, then a Svelte document, then a CSS document on one thread run
    /// over the same two allocations: the thread holds one TypeScript store set and one CSS
    /// store set.
    #[test]
    fn a_thread_parks_one_typescript_and_one_css_store_set() {
        let ts = tsv_ts::__private::with_stores(|stores| {
            tsv_ts::__private::parse_into("const a = [b, c => d];", stores).expect("parses");
            ts_address(stores)
        });
        let _held = decoys();
        let svelte = with_stores(|stores| {
            outputs(DOCUMENT, stores).expect("parses");
            // both slots are empty while the document holds their boxes
            let meanwhile = take_stores();
            assert_ne!(addresses(&meanwhile).0, addresses(stores).0);
            assert_ne!(addresses(&meanwhile).1, addresses(stores).1);
            addresses(stores)
        });
        assert_eq!(svelte.0, ts, "the box the TypeScript document parked");
        let _held = decoys();
        let css = tsv_css::__private::with_stores(|stores| {
            let arena = bumpalo::Bump::new();
            tsv_css::__private::parse_into("a { color: red; }", &arena, stores).expect("parses");
            css_address(stores)
        });
        assert_eq!(css, svelte.1, "the box the Svelte document parked");
        let _held = decoys();
        // and back: a second Svelte document takes the two boxes the others parked
        assert_eq!(with_stores(|stores| addresses(stores)), (ts, css));
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
    fn a_warm_handle_takes_and_parks_the_same_two_allocations() {
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

    /// A panic out of the closure parks nothing: the stores it held are dropped with its
    /// frame, both crates' slots stay empty, and the next call in each of the three languages
    /// starts from stores holding none of what the panicking call wrote.
    #[test]
    fn with_stores_recovers_after_a_caught_panic() {
        let fresh = typescript_mint_mark(&mut AstStores::new());
        // warm both slots, so the panic has parked stores to lose
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
            let arena = bumpalo::Bump::new();
            tsv_css::__private::parse_into("a { color: red; }", &arena, stores)
                .map(|sheet| format!("{sheet:?}"))
                .map_err(|e| format!("{e:#}"))
        });
        let arena = bumpalo::Bump::new();
        let fresh_css = tsv_css::__private::parse("a { color: red; }", &arena)
            .map(|parsed| format!("{:?}", parsed.stylesheet))
            .map_err(|e| format!("{e:#}"));
        assert_eq!(css, fresh_css, "the CSS slot hands out clean stores");
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
            });
            assert_eq!(addresses(outer), at);
        });
    }
}
