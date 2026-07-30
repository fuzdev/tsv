//! Per-thread reusable arenas for tsv's binding hot loop.
//!
//! The bindings (`tsv_ffi`, `tsv_napi`, `tsv_wasm`) are invoked once per file
//! in tight loops — formatters, editor save hooks, benchmarks. Allocating a
//! fresh arena per call (and freeing it at call end) churns the allocator's
//! heap high-water on *every* call, which is measurable through a host FFI /
//! N-API / WASM layer even when the engine work is unchanged. Instead each
//! thread keeps one arena and `reset()`s it between calls: `reset()` rewinds to
//! the start of the backing memory and retains the largest chunk, so once a
//! thread warms to its high-water mark there is no per-call malloc/free (this
//! supersedes per-call `with_capacity` pre-sizing — the first few calls pay the
//! chunk-growth tail once, then it amortizes to zero). WASM is single-threaded,
//! so its thread-local is effectively a module static; the reuse is sound there
//! for the same reason (the per-file work is consumed before the next `reset()`).
//!
//! Two reusables, gated to match the bindings' `format` / `parse` split:
//!
//! - [`with_ast_arena`] — the parse-time `bumpalo::Bump`. Always available;
//!   parse and format both need it.
//! - [`with_doc_arena`] — the format-time doc IR arena (`DocArena`). Behind the
//!   `format` feature.
//!
//! # Soundness
//!
//! Both helpers hand `f` a shared `&Arena` and `reset()` it at the *start* of
//! the next call. The caller must fully consume the per-file work inside `f`
//! and return an owned value (a formatted `String`, a JSON `String`, or `()`),
//! so nothing borrowed from the arena outlives the next call's `reset()`.
//!
//! # Abort safety: take and park, never a held guard
//!
//! Each helper **takes** its arena out of the thread-local for the duration of
//! `f` and **parks** it back afterwards — the same protocol `DocArena` uses for
//! its own `render_scratch`, and the reason is the same shape of hazard read at
//! a coarser grain.
//!
//! The tempting form is a `RefCell` whose borrow guard is held across `f`. That
//! is correct only where a panic *unwinds*, because only an unwind drops the
//! guard. The shipped profiles are `panic = "abort"` (`[profile.release]`), and
//! a WASM trap is not a process death: the JS host catches it as a
//! `RuntimeError` and the module instance stays alive and callable. So a held
//! guard would be left locked with nothing to release it, and **every later call
//! on that warm instance** would fail on `borrow_mut` — one bad file bricking a
//! whole `tsv format` run. Taking the arena out leaves **no state to restore**:
//! the slot is already empty while `f` runs, so a trap leaves it exactly as a
//! fresh thread finds it, and the next call builds a new arena and proceeds.
//! Unwind (the `catch_unwind` / `[profile.corpus]` world) converges on the same
//! state — the taken arena is a local, dropped as the frame unwinds — which is
//! why the native tests below are a faithful proxy for the abort case.
//!
//! The cost is that a panicking call loses its thread's *warm* arena rather than
//! keeping the high-water chunk; the next call re-warms. Correctness on a path
//! that should never run beats capacity retention on it.
//!
//! Re-entrancy follows from the same protocol: re-entering the *same* helper
//! inside its own closure finds an empty slot and builds a **fresh** arena
//! (fresh-fallback — correct, costing one allocation), rather than panicking as
//! a guard-based version did. It is still worth avoiding for the allocation:
//! a nested parse *during* formatting — the Svelte printer reparsing embedded
//! CSS — uses a *local* `bumpalo::Bump` rather than [`with_ast_arena`], and
//! should keep doing so. (Nesting [`with_doc_arena`] inside [`with_ast_arena`]
//! is not re-entrancy at all — distinct thread-locals, and exactly the format
//! path.)

use std::cell::Cell;
use std::thread::LocalKey;

/// Take the parked arena out of `slot` (building one with `make` when the slot
/// is empty — first call, or a prior call that panicked), `reset` it, run `f`
/// over it, and park it back.
///
/// The single implementation of the take/park protocol both helpers rely on;
/// see the [module docs](crate) for why the arena must not stay in the slot
/// while `f` runs.
fn with_parked_arena<T: 'static, R>(
    slot: &'static LocalKey<Cell<Option<T>>>,
    make: impl FnOnce() -> T,
    reset: impl FnOnce(&mut T),
    f: impl FnOnce(&T) -> R,
) -> R {
    let mut arena = slot.with(Cell::take).unwrap_or_else(make);
    reset(&mut arena);
    let result = f(&arena);
    slot.with(|cell| cell.set(Some(arena)));
    result
}

/// Run `f` with a per-thread reusable AST arena (a `bumpalo::Bump`).
///
/// See the [module docs](crate) for the reuse rationale, the take/park
/// protocol, and the soundness contract on what `f` may return.
pub fn with_ast_arena<R>(f: impl FnOnce(&bumpalo::Bump) -> R) -> R {
    // Parked by value: a `Bump` is 24 bytes (a chunk pointer and its limits),
    // so the take/park moves are free — the chunks themselves never move.
    thread_local! {
        static AST_ARENA: Cell<Option<bumpalo::Bump>> = const { Cell::new(None) };
    }
    with_parked_arena(&AST_ARENA, bumpalo::Bump::new, bumpalo::Bump::reset, f)
}

/// Run `f` with a per-thread reusable doc arena (a `DocArena`).
///
/// The `format` path's analogue of [`with_ast_arena`]; see the
/// [module docs](crate). Gated behind the `format` feature (the only consumer
/// of the doc IR), which pulls `tsv_lang` for the `DocArena` type.
#[cfg(feature = "format")]
pub fn with_doc_arena<R>(f: impl FnOnce(&tsv_lang::doc::arena::DocArena) -> R) -> R {
    // Parked **behind a `Box`**, unlike the AST arena: a `DocArena` is ~12.9 KB
    // by value (its inline 512-slot static cache), so parking it directly would
    // memcpy it in and out on every call — a per-call cost paid to avoid a
    // per-call allocation, which is the reuse win inverted. Boxed, take/park
    // moves one pointer and `f` still receives a plain `&DocArena`.
    thread_local! {
        static DOC_ARENA: Cell<Option<Box<tsv_lang::doc::arena::DocArena>>> =
            const { Cell::new(None) };
    }
    with_parked_arena(
        &DOC_ARENA,
        || Box::new(tsv_lang::doc::arena::DocArena::new()),
        |arena| arena.reset(),
        |arena| f(arena),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // The crate's whole reason to exist is that the arena is reset and reused
    // across calls without the prior call's contents leaking. These drive that
    // invariant directly, with no parser/formatter in the loop: each call
    // allocates into the (reset) arena and returns an OWNED value, so the next
    // call's `reset()` can never observe a live borrow.

    #[test]
    fn ast_arena_is_reusable_across_calls() {
        let first = with_ast_arena(|arena| arena.alloc_str("first").to_owned());
        let second = with_ast_arena(|arena| arena.alloc_str("second").to_owned());
        assert_eq!(first, "first", "first call's result");
        assert_eq!(
            second, "second",
            "second call must see a clean, reset arena"
        );
    }

    #[cfg(feature = "format")]
    #[test]
    fn doc_arena_is_reusable_across_calls() {
        use tsv_lang::EmbedContext;
        use tsv_lang::doc::arena_print_doc;

        let render = |word: &str| {
            with_doc_arena(|arena| {
                let id = arena.text_pooled(word);
                arena_print_doc(arena, id, &EmbedContext::default())
            })
        };
        let first = render("first");
        let second = render("second");
        assert_eq!(first, "first", "first render");
        assert_eq!(
            second, "second",
            "second render must see a clean, reset arena"
        );
    }

    // The soundness claims the bindings actually depend on, tested directly.
    //
    // These run native (unwinding), and the abort case they stand in for is the
    // shipped one: under `panic = "abort"` a WASM trap runs no `Drop` at all, so
    // any scheme needing a guard released after the fact is untestable here AND
    // broken there. Take/park has nothing to release — the slot is empty for the
    // whole of `f` — so unwind and abort leave *identical* thread-local state
    // (an empty slot), and a test of one is a test of the other. That
    // equivalence is the property to keep; a refactor that reintroduces held
    // state breaks it silently, since native tests would still pass.

    #[test]
    fn ast_arena_recovers_after_caught_panic() {
        // The panic-then-call-again sequence: one bad file must not brick the
        // instance for every file after it (the WASM `tsv` bin's failure mode).
        let caught = std::panic::catch_unwind(|| {
            with_ast_arena(|arena| {
                let _ = arena.alloc_str("doomed");
                panic!("boom");
            })
        });
        assert!(
            caught.is_err(),
            "the panic must propagate out of the helper"
        );
        let after = with_ast_arena(|arena| arena.alloc_str("after").to_owned());
        assert_eq!(after, "after", "arena must be usable after a caught panic");
    }

    #[cfg(feature = "format")]
    #[test]
    fn doc_arena_recovers_after_caught_panic() {
        // The doc arena's twin of the above — the format path takes both arenas,
        // so both must be abort-safe or the brick just moves.
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
        let after = with_doc_arena(|arena| {
            let id = arena.text_pooled("after");
            tsv_lang::doc::arena_print_doc(arena, id, &tsv_lang::EmbedContext::default())
        });
        assert_eq!(after, "after", "arena must be usable after a caught panic");
    }

    #[test]
    fn ast_arena_slot_is_empty_while_in_use() {
        // The structural claim behind abort safety, asserted directly: while `f`
        // runs, the thread-local holds nothing. A nested call therefore gets a
        // *distinct* arena (fresh-fallback) instead of resetting the outer one
        // under its feet or panicking on a held guard — and a trap out of `f`
        // leaves the slot exactly as a fresh thread finds it.
        with_ast_arena(|outer| {
            let outer_addr = std::ptr::from_ref(outer);
            let outer_str = outer.alloc_str("outer");
            with_ast_arena(|inner| {
                assert_ne!(
                    std::ptr::from_ref(inner),
                    outer_addr,
                    "a nested call must not receive the arena already in use"
                );
                let _ = inner.alloc_str("inner");
            });
            assert_eq!(outer_str, "outer", "the outer arena must be untouched");
        });
        let after = with_ast_arena(|arena| arena.alloc_str("after").to_owned());
        assert_eq!(after, "after", "the slot must be usable after nesting");
    }

    #[cfg(feature = "format")]
    #[test]
    fn doc_arena_slot_is_empty_while_in_use() {
        // The doc arena's twin — and the one place the `Box` indirection could
        // hide a mistake, since `f` receives a `&DocArena` either way.
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
        let after = with_doc_arena(|arena| {
            let id = arena.text_pooled("after");
            tsv_lang::doc::arena_print_doc(arena, id, &tsv_lang::EmbedContext::default())
        });
        assert_eq!(after, "after", "the slot must be usable after nesting");
    }
}
