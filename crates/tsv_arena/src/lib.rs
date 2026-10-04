//! The substrate tsv's three bindings share: per-thread reusable arenas for the
//! hot loop, and the goal axis their exports are generated over.
//!
//! Both halves are here for one reason — `tsv_ffi`, `tsv_napi` and `tsv_wasm`
//! would otherwise hand-sync them, and each encodes a contract too subtle to
//! keep three copies of honest (see the crate's CLAUDE.md §Why this crate
//! exists). The arenas are the bulk of it and are described below; the goal
//! macros ([`parse_ast!`], [`parse_ast_for_format!`], [`goal_allowed!`]), the export
//! bodies built on them ([`parse_convert!`], [`parse_internal!`], and the format path's
//! `parse_format!`), the export [`Family`] and the string-axis decoder
//! ([`decode_source_type`]) sit at the bottom of this file.
//!
//! # The arenas
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
//! guard. The shipped WASM, FFI and CLI artifacts are `panic = "abort"`
//! (`[profile.release]`; the N-API addon alone ships `[profile.napi]`'s unwind,
//! and take/park is correct under both), and a WASM trap is not a process death: the JS host catches it as a
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

/// `tsv_lang`, re-exported for [`parse_format!`]'s expansion alone: a macro's paths
/// resolve in the *caller's* crate, so without this every binding would need its own
/// `tsv_lang` edge just to spell the line-terminator fold. Not API.
#[cfg(feature = "format")]
#[doc(hidden)]
pub use tsv_lang as __tsv_lang;

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
    // Parked **behind a `Box`**, unlike the AST arena: a `DocArena` is ~48.9 KB
    // by value (its inline 2048-slot static cache), so parking it directly would
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

//
// The goal axis
//
// The second thing all three bindings would otherwise hand-sync. Each one
// spells the parse goal in its host's idiom — `tsv_ffi` a `u32` code, `tsv_napi`
// and `tsv_wasm` a trailing optional string — but the axis underneath is one
// question asked three times, so it is answered once here. The macros carry no
// arena; they live beside the helpers because this crate is where the bindings'
// shared substrate goes rather than in any one of them (see the crate's
// CLAUDE.md §Why this crate exists).

/// Which export family a binding call belongs to — the one fact that decides what an
/// **unset** source type means (a parse reads it as `Module`, a format as "none named",
/// the module-then-script fallback), and the noun every source-type refusal names.
///
/// One type for all three bindings' decoders: `tsv_ffi`'s `ffi_source_type` reads it to
/// decide whether its unspecified code is accepted (format only), and the two string-axis
/// bindings pass it through [`decode_source_type`] for the refusal's wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// `parse_*` / `parse_*_json` / `parse_internal_*` — a wire whose
    /// `Program.sourceType` one settled grammar must produce.
    Parse,
    /// `format_*` — whose source type shapes only the parse the formatter runs.
    Format,
}

impl Family {
    /// The family's name as the refusals spell it: `parse` / `format`.
    pub const fn noun(self) -> &'static str {
        match self {
            Family::Parse => "parse",
            Family::Format => "format",
        }
    }
}

/// Decode a string-spelled source type — `tsv_wasm` and `tsv_napi`'s trailing optional
/// argument — into the language's goal; `None` (omitted, `undefined`, `null`) stays
/// **unset**, and what that means is the `family`'s (a parse reads it as `Module`, a
/// format as "none named").
///
/// `allowed` is the language's goal axis ([`goal_allowed!`]): a source type named on a
/// language with none (Svelte hard-wires `Module`; CSS has no goal) is refused rather
/// than ignored, `"module"` included, so a caller cannot believe it selected a goal that
/// was silently dropped; the refusal names the export `family`. A value naming neither
/// goal is refused with the value echoed. `from_source_type` is the language crate's own
/// spelling table (`tsv_ts::Goal::from_source_type`), passed in so this crate depends on
/// no language crate.
///
/// One spelling of both refusals for the two string-axis bindings — the npm facade both
/// package sets publish through (`crates/tsv_wasm/npm/api.js`) restates them by hand in
/// JS, the one copy this crate cannot reach — so a caller reads the same text past the
/// facade or through it, and swapping `@fuzdev/tsv-wasm` for `@fuzdev/tsv` changes
/// nothing. The C FFI spells the axis as a code, not a string, and words its own; the
/// CLI's `--source-type` spells the flag instead of the key and keeps its own copy.
///
/// # Errors
///
/// The refusal message, for a source type on a goalless language or one naming neither
/// goal.
pub fn decode_source_type<G>(
    source_type: Option<&str>,
    allowed: bool,
    family: Family,
    from_source_type: impl FnOnce(&str) -> Option<G>,
) -> Result<Option<G>, String> {
    let Some(source_type) = source_type else {
        return Ok(None);
    };
    if !allowed {
        return Err(format!(
            "{} option 'sourceType' is only supported for TypeScript",
            family.noun()
        ));
    }
    from_source_type(source_type).map(Some).ok_or_else(|| {
        format!("invalid sourceType '{source_type}' (expected 'script' or 'module')")
    })
}

/// The per-language parse call behind each binding's uniform exports.
///
/// The `$goalness` tag is the language's goal axis, fixed at the
/// `lang_bindings!` invocation: `goal` (TypeScript) threads the decoded goal
/// into `parse_with_goal`; `nogoal` (Svelte, CSS) ignores it — a set goal is
/// already rejected by the binding's own goal decoder, which reads the same tag
/// through [`goal_allowed!`].
///
/// `$lang` and `$arena` resolve in the caller's scope, so this crate needs no
/// dependency on the language crates.
///
/// ```ignore
/// let ast = parse_ast!(goal, tsv_ts, source, goal, arena)?;
/// ```
#[macro_export]
macro_rules! parse_ast {
    (goal, $lang:ident, $source:expr, $goal:expr, $arena:expr) => {
        $lang::parse_with_goal($source, $goal, $arena)
    };
    (nogoal, $lang:ident, $source:expr, $goal:expr, $arena:expr) => {{
        // Consume the (always-`Module`) goal so the binding stays used in every
        // expansion.
        let _ = $goal;
        $lang::parse($source, $arena)
    }};
}

/// A binding's whole **parse** export body: parse `$source` through [`parse_ast!`] into the
/// per-thread AST arena and convert it with the language's `$convert` writer
/// (`convert_ast_json_bytes` for the C FFI, `convert_ast_json_string` for the two JS
/// bindings), a parse error mapped through `$map_err`, the binding's own error type.
///
/// `$goal` is the binding's decoded `Option` goal, and an unset one is read here as the
/// grammar's default, `Module` (`Goal::default()`): a parse has no fallback to run, since
/// its wire carries `Program.sourceType`, a claim one settled grammar has to produce.
/// The parse reads the caller's own bytes — no line-terminator fold, unlike
/// `parse_format!` — because the wire's offsets are a drop-in contract over them.
///
/// Expands to `Result<_, _>` inside the caller's context, so the caller's `?`-conversion
/// rules apply; `$lang` resolves in the caller's scope, as in [`parse_ast!`].
///
/// ```ignore
/// parse_convert!(goal, tsv_ts, convert_ast_json_string, source, goal, |e| e.to_string())
/// ```
#[macro_export]
macro_rules! parse_convert {
    ($goalness:ident, $lang:ident, $convert:ident, $source:expr, $goal:expr, $map_err:expr) => {{
        let source = $source;
        let goal = $goal.unwrap_or_default();
        $crate::with_ast_arena(|arena| {
            let ast =
                $crate::parse_ast!($goalness, $lang, source, goal, arena).map_err($map_err)?;
            Ok($lang::$convert(&ast, source))
        })
    }};
}

/// [`parse_convert!`] without the convert: the benchmark-only `parse_internal_*` body,
/// which parses and throws the AST away so the timing isolates the parse from the
/// convert/serialize layers. Expands to `Result<(), _>`; the goal reads as
/// [`parse_convert!`]'s does.
///
/// `black_box(&ast)` sits *inside* the arena closure on purpose — that is the only scope
/// where the AST still exists, so nothing further out can stand in for it. Drop it and
/// the whole parse becomes dead code the optimizer may delete, leaving a benchmark that
/// got faster by measuring nothing.
///
/// ```ignore
/// parse_internal!(goal, tsv_ts, source, goal, |e| e.to_string())
/// ```
#[macro_export]
macro_rules! parse_internal {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr, $map_err:expr) => {{
        let source = $source;
        let goal = $goal.unwrap_or_default();
        $crate::with_ast_arena(|arena| {
            let ast =
                $crate::parse_ast!($goalness, $lang, source, goal, arena).map_err($map_err)?;
            ::std::hint::black_box(&ast);
            Ok(())
        })
    }};
}

/// [`parse_ast!`]'s **format-path** twin, over the folded document
/// (`tsv_lang::printing::FoldedSource`): it calls the language's `parse_folded`, which maps
/// a parse error back onto the caller's own source — and, for Svelte, refuses the one lone
/// `<CR>` the fold cannot rewrite without changing meaning (`tsv_svelte::parse_folded`).
/// The goal arrives as an `Option`, and an unset one means "no source type named" rather
/// than `Module`.
///
/// Every binding's format export reads its source type the same way — a caller
/// that names one gets exactly that grammar, a caller that names none gets
/// `tsv_ts::parse_with_goal_or_fallback`'s module-then-script retry, which is what
/// lets `tsv format <path>` and an editor's bare `format_typescript(source)` format
/// a legacy sloppy script. The parse exports keep [`parse_ast!`], whose goal is a
/// settled `Goal`: their product is a wire carrying `Program.sourceType`, a claim
/// no retry may make depend on the input.
///
/// `nogoal` ignores the option exactly as [`parse_ast!`] ignores the goal — Svelte
/// hard-wires `Module` and CSS has no goal axis, so neither has a fallback to run.
///
/// ```ignore
/// let ast = parse_ast_for_format!(goal, tsv_ts, &folded, source_type, arena)?;
/// ```
#[macro_export]
macro_rules! parse_ast_for_format {
    (goal, $lang:ident, $folded:expr, $goal:expr, $arena:expr) => {
        $lang::parse_folded($folded, $goal, $arena)
    };
    (nogoal, $lang:ident, $folded:expr, $goal:expr, $arena:expr) => {{
        // Consume the (always-unset) goal so the binding stays used in every
        // expansion.
        let _ = $goal;
        $lang::parse_folded($folded, $arena)
    }};
}

/// The whole format export: fold the source's line terminators, parse the folded text
/// through [`parse_ast_for_format!`] in the per-thread AST arena, and format it in the
/// per-thread doc arena — a parse error mapped back onto the caller's source
/// (`tsv_lang::printing::FoldedSource::parse_with`) and then through `$map_err`, each
/// binding's own error type (a message string, a located failure, a JS value). A Svelte
/// refusal (`tsv_lang::ParseError::refusal`) takes the same road, positionless, so each
/// binding reports it as it reports a source over the size cap.
///
/// Expands to `Result<String, _>` inside the caller's `with_*_arena` closures, so the
/// caller's `?`-conversion rules apply; `$lang` resolves in the caller's scope, as in
/// [`parse_ast!`], while `tsv_lang` is reached through this crate (`$crate::__tsv_lang`),
/// so a binding needs no `tsv_lang` edge of its own. The parse exports skip the fold: their wire's offsets are
/// a drop-in contract over the author's own bytes.
///
/// ```ignore
/// parse_format!(goal, tsv_ts, source, source_type, |e| e.to_string())
/// ```
#[cfg(feature = "format")]
#[macro_export]
macro_rules! parse_format {
    ($goalness:ident, $lang:ident, $source:expr, $goal:expr, $map_err:expr) => {{
        let folded = $crate::__tsv_lang::printing::normalize_carriage_returns($source);
        $crate::with_ast_arena(|arena| {
            let ast = $crate::parse_ast_for_format!($goalness, $lang, &folded, $goal, arena)
                .map_err($map_err)?;
            Ok($crate::with_doc_arena(|doc_arena| {
                $lang::format_folded_in(&ast, &folded, doc_arena)
            }))
        })
    }};
}

/// Whether the binding's goal decoder accepts a set goal for this language —
/// the same `$goalness` tag [`parse_ast!`] reads, so the two can never disagree
/// about which languages have a goal axis.
///
/// A language with no axis **rejects** a set goal rather than ignoring it, so a
/// caller cannot believe it selected one that was silently dropped. Each binding
/// spells the refusal in its host's idiom; this macro only says which languages
/// owe one.
#[macro_export]
macro_rules! goal_allowed {
    (goal) => {
        true
    };
    (nogoal) => {
        false
    };
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

    // The goal axis: one `$goalness` tag driving two macros, tested here rather
    // than three times over in the bindings. A stand-in language module gives
    // `parse_ast!` the two entry points every language crate exposes, so the
    // DISPATCH is pinned without this crate depending on any of them.

    mod fake_lang {
        pub fn parse(source: &str, arena: &str) -> String {
            format!("parse({source}, {arena})")
        }
        pub fn parse_with_goal(source: &str, goal: &str, arena: &str) -> String {
            format!("parse_with_goal({source}, {goal}, {arena})")
        }
        /// The goal-axis language's format-path parse (`tsv_ts::parse_folded`).
        pub fn parse_folded(folded: &str, goal: Option<&str>, arena: &str) -> String {
            match goal {
                Some(goal) => format!("exact({folded}, {goal}, {arena})"),
                None => format!("fallback({folded}, {arena})"),
            }
        }
    }

    /// A goalless language (Svelte, CSS): its format-path parse takes no goal at all.
    mod fake_goalless_lang {
        pub fn parse_folded(folded: &str, arena: &str) -> String {
            format!("parse_folded({folded}, {arena})")
        }
    }

    #[test]
    fn parse_ast_dispatches_on_the_goalness_tag() {
        assert_eq!(
            parse_ast!(goal, fake_lang, "src", "script", "arena"),
            "parse_with_goal(src, script, arena)",
            "`goal` must thread the goal into `parse_with_goal`"
        );
        assert_eq!(
            parse_ast!(nogoal, fake_lang, "src", "script", "arena"),
            "parse(src, arena)",
            "`nogoal` must drop the goal and take the goalless entry point"
        );
    }

    #[test]
    fn parse_ast_for_format_dispatches_on_the_goalness_tag() {
        assert_eq!(
            parse_ast_for_format!(goal, fake_lang, "folded", Some("script"), "arena"),
            "exact(folded, script, arena)",
            "a NAMED source type must be exact — the same grammar `parse_ast!` runs"
        );
        assert_eq!(
            parse_ast_for_format!(goal, fake_lang, "folded", None, "arena"),
            "fallback(folded, arena)",
            "an unset source type must take the module-then-script fallback"
        );
        assert_eq!(
            parse_ast_for_format!(nogoal, fake_goalless_lang, "folded", None::<&str>, "arena"),
            "parse_folded(folded, arena)",
            "`nogoal` must drop the option and take the goalless entry point"
        );
    }

    /// A language whose parse can fail, for the export-body macros: its goal type is `&str`,
    /// whose `Default` (`""`) stands in for `Goal::Module`.
    mod fake_fallible_lang {
        pub fn parse(source: &str, arena: &bumpalo::Bump) -> Result<String, String> {
            parse_with_goal(source, "goalless", arena)
        }
        pub fn parse_with_goal(
            source: &str,
            goal: &str,
            _arena: &bumpalo::Bump,
        ) -> Result<String, String> {
            if source == "bad" {
                Err(format!("bad source at {goal:?}"))
            } else {
                Ok(format!("ast({source}, {goal:?})"))
            }
        }
        pub fn convert(ast: &String, source: &str) -> String {
            format!("convert({ast}, {source})")
        }
    }

    #[test]
    fn parse_convert_parses_converts_and_maps_the_error() {
        let run = |source: &str, goal: Option<&str>| -> Result<String, String> {
            parse_convert!(
                goal,
                fake_fallible_lang,
                convert,
                source,
                goal,
                |e| format!("mapped {e}")
            )
        };
        assert_eq!(
            run("src", Some("script")),
            Ok(r#"convert(ast(src, "script"), src)"#.to_owned()),
            "a named goal threads through"
        );
        assert_eq!(
            run("src", None),
            Ok(r#"convert(ast(src, ""), src)"#.to_owned()),
            "an unset goal reads as the goal type's default (`Goal::Module`)"
        );
        assert_eq!(
            run("bad", None),
            Err(r#"mapped bad source at """#.to_owned()),
            "a parse error goes through `$map_err`"
        );
        let goalless: Result<String, String> = parse_convert!(
            nogoal,
            fake_fallible_lang,
            convert,
            "src",
            None::<&str>,
            |e| e
        );
        assert_eq!(
            goalless,
            Ok(r#"convert(ast(src, "goalless"), src)"#.to_owned()),
            "`nogoal` takes the goalless entry point"
        );
    }

    #[test]
    fn parse_internal_parses_and_returns_nothing() {
        let run = |source: &str| -> Result<(), String> {
            parse_internal!(goal, fake_fallible_lang, source, None::<&str>, |e| e)
        };
        assert_eq!(run("src"), Ok(()));
        assert_eq!(run("bad"), Err(r#"bad source at """#.to_owned()));
    }

    #[test]
    fn decode_source_type_decodes_and_words_every_refusal() {
        let from = |s: &str| match s {
            "module" => Some("M"),
            "script" => Some("S"),
            _ => None,
        };
        assert_eq!(
            decode_source_type(None, true, Family::Parse, from),
            Ok(None)
        );
        assert_eq!(
            decode_source_type(None, false, Family::Format, from),
            Ok(None)
        );
        assert_eq!(
            decode_source_type(Some("script"), true, Family::Parse, from),
            Ok(Some("S"))
        );
        assert_eq!(
            decode_source_type(Some("module"), true, Family::Format, from),
            Ok(Some("M"))
        );
        // a goalless language refuses the axis, `"module"` included, ahead of the value
        for (family, noun) in [(Family::Parse, "parse"), (Family::Format, "format")] {
            assert_eq!(family.noun(), noun);
            for value in ["script", "module", "sloppy"] {
                assert_eq!(
                    decode_source_type(Some(value), false, family, from),
                    Err(format!(
                        "{noun} option 'sourceType' is only supported for TypeScript"
                    ))
                );
            }
        }
        assert_eq!(
            decode_source_type(Some("sloppy"), true, Family::Parse, from),
            Err("invalid sourceType 'sloppy' (expected 'script' or 'module')".to_owned())
        );
        // the empty string is a string — refused by value
        assert_eq!(
            decode_source_type(Some(""), true, Family::Parse, from),
            Err("invalid sourceType '' (expected 'script' or 'module')".to_owned())
        );
    }

    #[test]
    fn goal_allowed_matches_the_same_tag() {
        // Bound to locals first: the macro expands to a literal, and an
        // `assert!(true)` is a clippy error under the workspace's `-D warnings`.
        let has_axis: bool = goal_allowed!(goal);
        let no_axis: bool = goal_allowed!(nogoal);
        assert!(has_axis, "TypeScript has a goal axis");
        assert!(!no_axis, "Svelte and CSS have none");
    }
}
