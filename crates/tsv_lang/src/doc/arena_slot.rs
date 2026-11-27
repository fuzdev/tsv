//! The thread's parked doc arena: one slot per thread, shared by the two kinds of caller that
//! format on a thread they do not own — the bindings' format exports ([`with_doc_arena`],
//! through `tsv_arena`'s `parse_format!`) and the language crates' free `format` functions
//! ([`with_one_shot_doc_arena`]). Re-exported from [`crate::doc::arena`].
//!
//! # Why park it
//!
//! The format path is invoked once per file in tight loops — formatters, editor save hooks,
//! benchmarks. Allocating a fresh doc arena per call (and freeing it at call end) churns the
//! allocator's heap high-water on *every* call: [`DocArena::for_source`] reserves about two
//! nodes per source byte. Instead the thread keeps one `DocArena` and [`DocArena::reset`]s it
//! between calls, which rewinds it and keeps its capacity, so once a thread warms to its
//! high-water mark a call allocates nothing for its doc. WASM is single-threaded, so the slot
//! is effectively a module static there; the reuse is sound for the same reason (the per-file
//! work is consumed before the next reset).
//!
//! # Two policies over one slot
//!
//! The slot holds one arena; the policy is the caller's, at its park.
//!
//! - **The bindings** ([`with_doc_arena`]) park whatever the call grew, uncapped: a binding's
//!   thread is a worker the host owns, its valves documented by the bindings (retire the
//!   worker or thread; `reinstantiate()` on wasm).
//! - **The free functions** ([`with_one_shot_doc_arena`]) park the arena back only while its
//!   capacity is at most [`ONE_SHOT_PARK_CAP_BYTES`], and drop it otherwise, so a library
//!   caller's thread retains a bounded amount, released at thread exit. A caller wanting the
//!   capacity under its own ownership holds a language crate's `Formatter` instead.
//!
//! A thread that runs both meets one arena: a free function that takes an over-cap arena a
//! binding parked drops it at its own park, and the binding's next call builds a new one.
//!
//! # Soundness
//!
//! A caller hands `f` a shared `&DocArena` and the arena is reset at the *start* of the next
//! call that takes it. `f` must fully consume the per-file work and return an owned value (a
//! formatted `String`), so nothing borrowed from the arena outlives the next reset.
//!
//! # Abort safety: take and park, never a held guard
//!
//! Both protocols **take** the arena out of the slot for the duration of `f` and **park** it
//! back afterwards — the protocol `DocArena` uses for its own `render_scratch`, at a coarser
//! grain.
//!
//! The tempting form is a `RefCell` whose borrow guard is held across `f`. That is correct
//! only where a panic *unwinds*, because only an unwind drops the guard. The shipped WASM,
//! FFI and CLI artifacts are `panic = "abort"` (`[profile.release]`; the N-API addon alone
//! ships `[profile.napi]`'s unwind, and take/park is correct under both), and a WASM trap is
//! not a process death: the JS host catches it as a `RuntimeError` and the module instance
//! stays alive and callable. So a held guard would be left locked with nothing to release
//! it, and **every later call on that warm instance** would fail on `borrow_mut` — one bad
//! file bricking a whole `tsv format` run. Taking the arena out leaves **no state to
//! restore**: the slot is already empty while `f` runs, so a trap leaves it exactly as a fresh
//! thread finds it, and the next call builds a new arena and proceeds. Unwind (the
//! `catch_unwind` / `[profile.corpus]` world) converges on the same state — the taken arena is
//! a local, dropped as the frame unwinds — which is why the native tests below are a faithful
//! proxy for the abort case. No `Drop` parks it back, for the same reason.
//!
//! The cost is that a panicking call loses its thread's *warm* arena rather than keeping its
//! capacity; the next call re-warms. Correctness on a path that should never run beats
//! capacity retention on it.
//!
//! Re-entrancy follows from the same protocol: a call inside another's `f` finds an empty slot
//! and builds a **fresh** arena (correct, costing one allocation) rather than resetting the
//! outer one under its feet. And a call from inside a thread-local destructor, once the slot
//! itself is gone, builds a fresh arena and parks nothing (`LocalKey::try_with`), never
//! panicking.

use std::cell::Cell;

use super::arena::DocArena;
use crate::store::ONE_SHOT_PARK_CAP_BYTES;

thread_local! {
    /// This thread's parked doc arena. **Boxed**: a `DocArena` is hundreds of bytes by value
    /// (its stores' headers and scratch buffers; the static cache is already boxed), so
    /// parking it directly would copy it in and out on every call. Boxed, take and park move
    /// one pointer and `f` still receives a plain `&DocArena`.
    static PARKED: Cell<Option<Box<DocArena>>> = const { Cell::new(None) };
}

/// The parked arena, leaving the slot empty; `None` when the slot is empty or, inside a
/// thread-local destructor, gone.
#[inline]
fn take_parked() -> Option<Box<DocArena>> {
    PARKED.try_with(Cell::take).ok().flatten()
}

/// Parks `arena` in the slot, dropping whatever it holds; drops `arena` instead when the slot
/// is gone (a call from a thread-local destructor).
#[inline]
fn park(arena: Box<DocArena>) {
    // a slot already destroyed drops the closure, and `arena` with it
    let _ = PARKED.try_with(|slot| slot.set(Some(arena)));
}

/// Run `f` with this thread's parked doc arena, reset, or a new one when the slot is empty —
/// the bindings' protocol (`tsv_arena`'s `parse_format!`): parked back after `f` whatever it
/// grew, never trimmed.
///
/// See the [module docs](self) for the reuse rationale, the take/park protocol and the
/// soundness contract on what `f` may return.
pub fn with_doc_arena<R>(f: impl FnOnce(&DocArena) -> R) -> R {
    let mut arena = take_parked().unwrap_or_else(|| Box::new(DocArena::new()));
    arena.reset();
    let result = f(&arena);
    park(arena);
    result
}

/// A free format function's doc arena for one call: taken from this thread's slot the first
/// time the call asks for it ([`OneShotDocArena::get`], after its parse has succeeded), and
/// parked back capped by [`with_one_shot_doc_arena`] once the call returns.
#[derive(Default)]
pub struct OneShotDocArena {
    taken: Option<Box<DocArena>>,
}

impl std::fmt::Debug for OneShotDocArena {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneShotDocArena")
            .field("taken", &self.taken.is_some())
            .finish()
    }
}

impl OneShotDocArena {
    /// The call's arena: this thread's parked one, reset, or a new one sized for the folded
    /// text `folded` ([`DocArena::for_source`]) when the slot is empty. Taken once; a later
    /// ask returns the same arena.
    ///
    /// By value on the reference, so the closure a format path hands it to moves its
    /// `&mut` in and returns a borrow of the arena for the whole call.
    pub fn get<'a>(this: &'a mut Self, folded: &str) -> &'a DocArena {
        this.taken.get_or_insert_with(|| match take_parked() {
            Some(mut arena) => {
                arena.reset();
                arena
            }
            None => Box::new(DocArena::for_source(folded)),
        })
    }
}

/// Run `f` over a free format function's doc arena (taken lazily, [`OneShotDocArena::get`]),
/// then park the arena back in this thread's slot when its capacity is at most
/// [`ONE_SHOT_PARK_CAP_BYTES`], dropping it otherwise — the free functions' protocol. A call
/// that never asked for the arena (a failed parse) parks nothing and leaves the slot as it
/// found it.
///
/// The arena is out of the slot from its take to the park, so a nested call builds a fresh
/// one and a panic out of `f` leaves the slot empty ([module docs](self)).
pub fn with_one_shot_doc_arena<R>(f: impl FnOnce(&mut OneShotDocArena) -> R) -> R {
    let mut call = OneShotDocArena::default();
    let result = f(&mut call);
    if let Some(mut arena) = call.taken
        && arena.capacity_bytes() <= ONE_SHOT_PARK_CAP_BYTES
    {
        park(arena);
    }
    result
}

/// The capacity of the arena parked in this thread's slot, or `None` when the slot is empty
/// (or gone) — the retention the tests of the free functions read. The arena stays parked.
#[doc(hidden)]
pub fn parked_doc_arena_capacity() -> Option<usize> {
    let mut arena = take_parked()?;
    let bytes = arena.capacity_bytes();
    park(arena);
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::arena_print_doc;

    fn render(arena: &DocArena, word: &str) -> String {
        let id = arena.text_pooled(word);
        arena_print_doc(arena, id)
    }

    // The arena is reset and reused across calls without the prior call's contents leaking:
    // each call allocates into the (reset) arena and returns an OWNED value, so the next call's
    // reset can never observe a live borrow.

    #[test]
    fn doc_arena_is_reusable_across_calls() {
        let first = with_doc_arena(|arena| render(arena, "first"));
        let second = with_doc_arena(|arena| render(arena, "second"));
        assert_eq!(first, "first", "first render");
        assert_eq!(
            second, "second",
            "second render must see a clean, reset arena"
        );
    }

    // The soundness claims the bindings depend on, tested directly.
    //
    // These run native (unwinding), and the abort case they stand in for is the shipped one:
    // under `panic = "abort"` a WASM trap runs no `Drop` at all, so any scheme needing a guard
    // released after the fact is untestable here AND broken there. Take/park has nothing to
    // release — the slot is empty for the whole of `f` — so unwind and abort leave *identical*
    // thread-local state (an empty slot), and a test of one is a test of the other. That
    // equivalence is the property to keep; a refactor that reintroduces held state breaks it
    // silently, since native tests would still pass.

    #[test]
    fn doc_arena_recovers_after_caught_panic() {
        // one bad file must not brick the instance for every file after it (the WASM `tsv`
        // bin's failure mode)
        let caught = std::panic::catch_unwind(|| {
            with_doc_arena(|arena| {
                let _ = arena.text_pooled("doomed");
                panic!("boom");
            })
        });
        assert!(
            caught.is_err(),
            "the panic must propagate out of the helper"
        );
        assert_eq!(
            parked_doc_arena_capacity(),
            None,
            "the panic empties the slot"
        );
        let after = with_doc_arena(|arena| render(arena, "after"));
        assert_eq!(after, "after", "arena must be usable after a caught panic");
    }

    #[test]
    fn doc_arena_slot_is_empty_while_in_use() {
        // The structural claim behind abort safety: while `f` runs, the slot holds nothing.
        // A nested call therefore gets a *distinct* arena instead of resetting the outer one
        // under its feet — and a trap out of `f` leaves the slot as a fresh thread finds it.
        // The `Box` is the one place this could hide a mistake, since `f` receives a
        // `&DocArena` either way.
        with_doc_arena(|outer| {
            let outer_addr = std::ptr::from_ref(outer);
            with_doc_arena(|inner| {
                assert_ne!(
                    std::ptr::from_ref(inner),
                    outer_addr,
                    "a nested call must not receive the arena already in use"
                );
            });
        });
        let after = with_doc_arena(|arena| render(arena, "after"));
        assert_eq!(after, "after", "the slot must be usable after nesting");
    }

    /// A free function's call over `word`, rendering only when `render_it`.
    fn one_shot(word: &str, render_it: bool) -> Option<(String, usize)> {
        with_one_shot_doc_arena(|call| {
            render_it.then(|| {
                let arena = OneShotDocArena::get(call, word);
                (render(arena, word), std::ptr::from_ref(arena) as usize)
            })
        })
    }

    #[test]
    fn a_one_shot_arena_parks_under_the_cap_and_is_reused() {
        let (first, at) = one_shot("first", true).expect("rendered");
        assert_eq!(first, "first");
        let parked = parked_doc_arena_capacity().expect("an under-cap arena parks");
        assert!(parked <= ONE_SHOT_PARK_CAP_BYTES);
        let (second, again) = one_shot("second", true).expect("rendered");
        assert_eq!(second, "second", "the parked arena is reset before reuse");
        assert_eq!(again, at, "the second call reuses the parked box");
    }

    #[test]
    fn a_one_shot_call_that_never_asks_leaves_the_slot_alone() {
        assert_eq!(one_shot("x", false), None);
        assert_eq!(
            parked_doc_arena_capacity(),
            None,
            "nothing taken, nothing parked"
        );
        with_doc_arena(|_| ());
        let parked = parked_doc_arena_capacity();
        assert!(parked.is_some());
        assert_eq!(one_shot("x", false), None);
        assert_eq!(
            parked_doc_arena_capacity(),
            parked,
            "the binding's arena stays parked"
        );
    }

    #[test]
    fn an_over_cap_one_shot_arena_is_dropped() {
        // an arena sized past the cap: `for_source` reserves two 24-byte nodes a byte
        let big = "x".repeat(ONE_SHOT_PARK_CAP_BYTES / 16);
        let rendered = with_one_shot_doc_arena(|call| {
            let arena = OneShotDocArena::get(call, &big);
            render(arena, "big")
        });
        assert_eq!(rendered, "big");
        assert_eq!(
            parked_doc_arena_capacity(),
            None,
            "an over-cap arena is dropped"
        );
        let (after, _) = one_shot("after", true).expect("rendered");
        assert_eq!(after, "after", "the next call starts cold and is correct");
        assert!(
            parked_doc_arena_capacity().is_some(),
            "and parks its small arena"
        );
    }

    #[test]
    fn an_over_cap_binding_arena_stays_parked() {
        // the bindings' park is uncapped
        with_doc_arena(|arena| {
            let _ = arena.text_pooled(&"y".repeat(2 * ONE_SHOT_PARK_CAP_BYTES));
        });
        let parked = parked_doc_arena_capacity().expect("parked");
        assert!(parked > ONE_SHOT_PARK_CAP_BYTES);
        // a free function on the same thread takes it and drops it at its capped park
        let (word, _) = one_shot("w", true).expect("rendered");
        assert_eq!(word, "w");
        assert_eq!(parked_doc_arena_capacity(), None);
    }

    #[test]
    fn a_nested_one_shot_call_gets_a_fresh_arena() {
        let rendered = with_one_shot_doc_arena(|outer_call| {
            let outer = OneShotDocArena::get(outer_call, "outer");
            let outer_id = outer.text_pooled("outer");
            let (inner, inner_at) = one_shot("inner", true).expect("rendered");
            assert_ne!(inner_at, std::ptr::from_ref(outer) as usize);
            assert_eq!(inner, "inner");
            arena_print_doc(outer, outer_id)
        });
        assert_eq!(
            rendered, "outer",
            "the nested call left the outer arena alone"
        );
    }

    #[test]
    fn a_one_shot_panic_leaves_the_slot_empty() {
        let _ = one_shot("warm", true);
        assert!(parked_doc_arena_capacity().is_some());
        let caught = std::panic::catch_unwind(|| {
            with_one_shot_doc_arena(|call| {
                let _ = OneShotDocArena::get(call, "doomed").text_pooled("doomed");
                panic!("boom");
            })
        });
        assert!(caught.is_err());
        assert_eq!(parked_doc_arena_capacity(), None);
        assert_eq!(
            one_shot("after", true).map(|(s, _)| s).as_deref(),
            Some("after")
        );
    }

    #[test]
    fn a_call_from_a_thread_local_destructor_falls_back_to_a_fresh_arena() {
        struct FormatsOnDrop(std::sync::mpsc::Sender<(String, String)>);
        impl Drop for FormatsOnDrop {
            fn drop(&mut self) {
                let binding = with_doc_arena(|arena| render(arena, "binding"));
                let free = one_shot("free", true).map(|(s, _)| s).unwrap_or_default();
                let _ = self.0.send((binding, free));
            }
        }
        thread_local! {
            static ON_EXIT: Cell<Option<FormatsOnDrop>> = const { Cell::new(None) };
        }
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // registered ahead of the slot, so std's reverse-order destructors run the slot's
            // first and this one finds it gone
            ON_EXIT.with(|cell| cell.set(Some(FormatsOnDrop(send))));
            let _ = one_shot("warm", true);
        })
        .join()
        .expect("the thread exits cleanly");
        assert_eq!(
            receive.recv().expect("the destructor ran"),
            ("binding".to_owned(), "free".to_owned())
        );
    }
}
