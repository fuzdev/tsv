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
//! Two arenas, gated to match the bindings' `format` / `parse` split:
//!
//! - [`with_ast_arena`] — the parse-time `bumpalo::Bump`. Always available;
//!   parse and format both need it.
//! - [`with_doc_arena`] — the format-time doc IR arena (`DocArena`). Behind the
//!   `format` feature, which pulls `tsv_lang` for the type, so a parse-only
//!   build doesn't link it.
//!
//! # Soundness
//!
//! Both helpers hand `f` a shared `&Arena` and `reset()` it at the *start* of
//! the next call. The caller must fully consume the per-file work inside `f`
//! and return an owned value (a formatted `String`, a JSON `String`, or `()`),
//! so nothing borrowed from the arena outlives the next call's `reset()`. The
//! `RefCell` borrow is released on unwind, so the reuse also recovers cleanly
//! after a `catch_unwind`-caught panic (the FFI path) — the next call
//! re-borrows and resets a valid arena.
//!
//! These helpers are **non-reentrant**: each holds its thread-local's `RefCell`
//! borrow for the duration of `f`, so re-entering the *same* helper inside its
//! own closure panics on the second `borrow_mut`. (Nesting [`with_doc_arena`]
//! inside [`with_ast_arena`] is fine — they are distinct thread-locals, and that
//! is exactly the format path.) This is why a nested parse *during* formatting —
//! the Svelte printer reparsing embedded CSS — uses a *local* `bumpalo::Bump`
//! rather than [`with_ast_arena`]; keep it that way.

use std::cell::RefCell;

/// Run `f` with a per-thread reusable AST arena (a `bumpalo::Bump`).
///
/// See the [module docs](crate) for the reuse rationale and the soundness
/// contract on what `f` may return.
pub fn with_ast_arena<R>(f: impl FnOnce(&bumpalo::Bump) -> R) -> R {
    thread_local! {
        static AST_ARENA: RefCell<bumpalo::Bump> = RefCell::new(bumpalo::Bump::new());
    }
    AST_ARENA.with(|cell| {
        let mut arena = cell.borrow_mut();
        arena.reset();
        f(&arena)
    })
}

/// Run `f` with a per-thread reusable doc arena (a `DocArena`).
///
/// The `format` path's analogue of [`with_ast_arena`]; see the
/// [module docs](crate). Gated behind the `format` feature (the only consumer
/// of the doc IR), which pulls `tsv_lang` for the `DocArena` type.
#[cfg(feature = "format")]
pub fn with_doc_arena<R>(f: impl FnOnce(&tsv_lang::doc::arena::DocArena) -> R) -> R {
    thread_local! {
        static DOC_ARENA: RefCell<tsv_lang::doc::arena::DocArena> =
            RefCell::new(tsv_lang::doc::arena::DocArena::new());
    }
    DOC_ARENA.with(|cell| {
        let mut arena = cell.borrow_mut();
        arena.reset();
        f(&arena)
    })
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
                let id = arena.text_owned(word.to_owned());
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

    // The two soundness claims the bindings actually depend on, tested directly.

    #[test]
    fn ast_arena_recovers_after_caught_panic() {
        // The FFI path wraps the work in `catch_unwind`. A panic inside `f`
        // unwinds out of `with_ast_arena`, dropping the `RefCell` borrow guard,
        // so a later call must re-borrow and `reset()` a valid arena rather than
        // hit "already borrowed". This is the exact sequence tsv_ffi relies on.
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

    #[test]
    #[should_panic(expected = "already borrowed")]
    fn ast_arena_is_not_reentrant() {
        // The helper holds its thread-local's borrow for the closure's duration,
        // so re-entering the *same* helper inside its own closure panics. Pins the
        // documented non-reentrancy contract — a nested parse/format must use a
        // local `Bump` (as the Svelte embedded-CSS path does) — so a refactor that
        // routes a nested parse through the thread-local fails here, not in prod.
        with_ast_arena(|_outer| {
            with_ast_arena(|_inner| {});
        });
    }
}
