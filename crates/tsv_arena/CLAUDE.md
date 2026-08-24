# tsv_arena

> The substrate tsv's three bindings share: per-thread reusable arenas for the hot loop, and the goal axis their exports are generated over.

## Why this crate exists

The bindings (`tsv_ffi`, `tsv_napi`, `tsv_wasm`) are invoked once per file in tight loops (formatters, editor save hooks, benchmarks). A fresh arena allocated and freed per call churns the allocator's heap high-water on *every* call — measurable through a host FFI / N-API / WASM layer even when the engine work is unchanged. `tsv_arena` keeps **one arena per thread** and `reset()`s it between calls (rewind the bump pointer, retain the largest chunk), so a warm thread does no per-call malloc/free.

It's a crate, not duplicated inline, because the bindings would otherwise hand-sync it. The helpers are tiny but encode a subtle soundness contract (nothing borrowed may outlive the next call's `reset()`); a single home keeps that contract from drifting.

**The same argument, a second time, is why the goal macros are here too.** Each binding spells the parse goal in its host's idiom — a `u32` code, a trailing optional string, one key of an options bag — but *which languages have a goal axis at all* is one fact, and three copies of it agree only until one is edited. So the crate's scope is the bindings' shared substrate, not arenas specifically; the name is older than the second half.

**Not in `tsv_lang`:** the foundation crate deliberately doesn't depend on `bumpalo` (the AST `Bump` is passed *into* the language crates), and a thread-local hot-loop reuse policy is a binding concern, not a language primitive — putting it there would invert the layering.

## API

- `with_ast_arena(f)` — runs `f` with a per-thread `bumpalo::Bump`. **Always available** (parse and format both need it).
- `with_doc_arena(f)` — runs `f` with a per-thread `DocArena` (the format-time doc IR). Behind the **`format`** feature, which pulls `tsv_lang` for the type.

Both `reset()` at the *start* of each call; `f` must return an owned value (a formatted `String`, a JSON `String`, or `()`) so nothing borrowed escapes. Full rationale + soundness in the `src/lib.rs` module docs.

Plus the goal-axis pair, `#[macro_export]`ed and feature-independent (they generate no code of their own):

- `parse_ast!($goalness, $lang, $source, $goal, $arena)` — the per-language parse call. `goal` (TypeScript) threads the decoded goal into `$lang::parse_with_goal`; `nogoal` (Svelte, CSS) drops it and calls `$lang::parse`. `$lang` resolves in the *caller's* scope, so this crate depends on no language crate.
- `goal_allowed!($goalness)` — `true` / `false`, read by each binding's own goal decoder (`ffi_goal`, `napi_goal`, `read_options`).

The load-bearing property is that **one `$goalness` tag drives both**: a language with no axis *rejects* a set goal rather than ignoring it, and the macro that picks the parse call and the macro that licenses the refusal can't come to disagree about which languages those are. Each binding still owns its own `lang_bindings!` (three different export signatures) and its own refusal wording.

## Abort safety: take and park

Each helper **takes** its arena out of the thread-local for the call and **parks** it back after — it never holds a `RefCell` borrow guard across `f`. This is the load-bearing decision in the crate; the argument (a WASM trap runs no `Drop` but leaves the instance callable, so a held guard bricks every later call) is in the `src/lib.rs` module docs, along with the two consequences — a panicking call loses its warm arena, and re-entrancy became a fresh-fallback rather than a panic.

What the module docs don't carry, because it is evidence rather than rationale:

- **Measured end-to-end on the built `format` bundle** with a temporary panicking export: with a held guard, a trap made every subsequent `format_typescript` throw; with take/park, calls after the trap return correct output.
- The change was **byte-identical over 211 corpus files and ~4% faster** on the WASM format path (`wasm_format_probe` net 0.95/0.96 across two runs, floor ~1.01). Two effects are inseparable by construction: dropping the borrow guard, and the `const { Cell::new(None) }` thread-local init that only becomes possible once the parked state is `None` (`Bump::new()` is not `const`, so the old form necessarily used std's lazy thread-local storage — a state check per access; the new one is eager, on wasm a plain static).

## Features

- `format` (default) — adds `with_doc_arena` + the optional `tsv_lang` dep.

The **workspace dependency entry is `default-features = false`**, so a binding gets only `with_ast_arena` by default and re-enables `format` from its own `format` feature — that's what keeps the parse-only binding build from pulling `tsv_lang`. A standalone `cargo test -p tsv_arena` uses the crate's own `default = ["format"]`, so both helpers are exercised.

## Consumers

`tsv_ffi`, `tsv_napi`, and `tsv_wasm`. Each maps its `format` feature to `tsv_arena/format`, calls the two arena helpers from its `lang_bindings!` macro, and expands the same `parse_ast!` / `goal_allowed!` pair inside it.

For the two **native** bindings the win is heap-churn through the host FFI/N-API layer. For **`tsv_wasm`** it's the per-call `Bump`/`DocArena` allocation in the sandbox (the documented WASM-format allocation-count lever) — measured at a **byte-identical ~2% warm format speedup** (svelte ~3%) on the zzz corpus via `benches/js/diagnostics/wasm_format_probe.ts`, with a negligible cold single-shot cost (one un-pre-sized first allocation; even `npm/cli.js` is warm after its first file) and +0.08% bundle size. Before this, `tsv_wasm` was the lone binding still allocating fresh arenas per call.
