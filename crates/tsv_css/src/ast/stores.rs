//! The stores a stylesheet's parse writes beside its arena, and the per-thread parking of
//! their capacity.
//!
//! [`CssStores`] is one stylesheet's: a text pool, for the strings a span of the source
//! cannot express. No parse mints from it — the tree's nodes, lists and strings live in the
//! caller's `Bump` — so a parse leaves it as it found it; the parser, the value parser, the
//! printer and the wire writer carry it, or its view ([`super::CssView`]), to where such a
//! string is written and read.
//!
//! What outlives a parse is the rewound form: [`CssStores::reset`] rewinds the pool in place,
//! keeping its capacity, and never trims. Handles die with the parse that minted them:
//! nothing pool-text-shaped leaves [`with_stores`]'s closure or a [`ParsedCss`], whose stores
//! it keeps beside the tree they belong to.

use std::cell::Cell;
use std::fmt;
use std::ops::Deref;

use tsv_lang::store::TextPool;
#[cfg(test)]
use tsv_lang::value_runs::FieldShape;

use super::CssStyleSheet;

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

/// One stylesheet's text pool.
///
/// A fresh one allocates nothing: the pool's buffer is allocated by its first string.
pub struct CssStores {
    pub(crate) text: TextPool<CssText>,
}

impl CssStores {
    /// Empty stores for one stylesheet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: TextPool::new(),
        }
    }

    /// Rewinds the stores in place between stylesheets, keeping the pool's capacity. Every
    /// pool text minted so far is invalid from here on.
    #[inline]
    pub fn reset(&mut self) {
        // destructured whole, so a field added to the stores without a reset here is a build
        // error rather than growth across stylesheets
        let Self { text } = self;
        text.reset();
    }

    /// Every field's shape, for the tests that read what a reset leaves. Destructured whole,
    /// as [`CssStores::reset`] is, so no field is missed.
    #[cfg(test)]
    fn shapes(&self) -> Vec<FieldShape> {
        let Self { text } = self;
        vec![FieldShape::pool("text", text)]
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
pub struct ParsedCss<'arena> {
    /// The tree.
    pub stylesheet: CssStyleSheet<'arena>,
    /// The stores the tree's handles index.
    pub stores: CssStores,
}

impl<'arena> Deref for ParsedCss<'arena> {
    type Target = CssStyleSheet<'arena>;

    fn deref(&self) -> &CssStyleSheet<'arena> {
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

/// Parks `stores` in this thread's slot after a [`CssStores::reset`], keeping capacity, so
/// every pool text minted from them is invalid from here on. Stores already in the slot are
/// dropped.
pub fn park_stores(mut stores: Box<CssStores>) {
    stores.reset();
    PARKED.with(|cell| cell.set(Some(stores)));
}

/// Run `f` over this thread's parked stores, or over fresh ones when there are none:
/// `tsv_arena`'s take-and-park protocol, here because that crate cannot name these types.
///
/// The stores are out of the slot while `f` runs ([`take_stores`]), so a nested call takes
/// fresh ones and a panic out of `f` leaves the slot empty, as a new thread finds it. Parked
/// after a [`CssStores::reset`] ([`park_stores`]), keeping capacity, so every pool text `f`
/// minted is invalid once it returns.
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

    /// Parks `stores` in the slot after a [`CssStores::reset`], keeping capacity, so every
    /// pool text minted from them is invalid from here on.
    pub fn park(&mut self, mut stores: Box<CssStores>) {
        stores.reset();
        self.parked = Some(stores);
    }

    /// Run `f` over the parked stores, or over fresh ones when there are none, and park them
    /// after a [`CssStores::reset`], keeping capacity, so every pool text `f` minted is
    /// invalid once it returns. A panic out of `f` leaves the slot empty, and the next call
    /// starts from fresh stores.
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
    use tsv_lang::store::PoolText;

    /// Writes one string to every field, so each holds something a reset has to rewind.
    fn fill_every_field(stores: &mut CssStores) -> PoolText<CssText> {
        stores.text.push("a pooled string")
    }

    fn assert_every_field_written(filled: &[FieldShape]) {
        for shape in filled {
            assert!(shape.len > 1, "{} was not written", shape.name);
            assert!(shape.capacity > 0, "{} allocated nothing", shape.name);
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
        let caught = std::panic::catch_unwind(|| {
            with_stores(|stores| {
                assert_eq!(address(stores), at);
                panic!("boom");
            });
        });
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
            let arena = bumpalo::Bump::new();
            let reused = warm.with(|stores| {
                crate::__private::parse_into(source, &arena, stores)
                    .map(|sheet| format!("{sheet:?}"))
                    .map_err(|e| format!("{e:#}"))
            });
            let fresh_arena = bumpalo::Bump::new();
            let fresh = crate::__private::parse(source, &fresh_arena)
                .map(|parsed| format!("{:?}", parsed.stylesheet))
                .map_err(|e| format!("{e:#}"));
            assert_eq!(reused, fresh, "{source:?}");
        }
    }
}
