# tsv_arena

> The substrate tsv's three bindings share: per-thread reusable arenas for the hot loop, and the goal axis their exports are generated over.

## Why this crate exists

The bindings (`tsv_ffi`, `tsv_napi`, `tsv_wasm`) are invoked once per file in tight loops (formatters, editor save hooks, benchmarks). A fresh arena allocated and freed per call churns the allocator's heap high-water on *every* call — measurable through a host FFI / N-API / WASM layer even when the engine work is unchanged. `tsv_arena` keeps **one arena per thread** and `reset()`s it between calls (rewind the bump pointer, retain the largest chunk), so a warm thread does no per-call malloc/free.

It's a crate, not duplicated inline, because the bindings would otherwise hand-sync it. The helpers are tiny but encode a subtle soundness contract (nothing borrowed may outlive the next call's `reset()`); a single home keeps that contract from drifting.

**The same argument, a second time, is why the goal macros are here too.** Each binding spells the parse goal in its host's idiom — a `u32` code, a trailing optional string — but *which languages have a goal axis at all* is one fact, and three copies of it agree only until one is edited. So the crate's scope is the bindings' shared substrate, not arenas specifically; the name is older than the second half.

**Not in `tsv_lang`:** the foundation crate deliberately doesn't depend on `bumpalo` (the AST `Bump` is passed *into* the language crates), and a thread-local hot-loop reuse policy is a binding concern, not a language primitive — putting it there would invert the layering.

## API

- `with_ast_arena(f)` — runs `f` with a per-thread `bumpalo::Bump`. **Always available** (parse and format both need it).
- `with_doc_arena(f)` — runs `f` with a per-thread `DocArena` (the format-time doc IR). Behind the **`format`** feature, which pulls `tsv_lang` for the type.

Both `reset()` at the *start* of each call; `f` must return an owned value (a formatted `String`, a JSON `String`, or `()`) so nothing borrowed escapes. Full rationale + soundness in the `src/lib.rs` module docs.

Plus the goal-axis macros and the export bodies built on them, `#[macro_export]`ed; `parse_ast!`, `parse_ast_for_format!`, `goal_allowed!`, `parse_convert!` and `parse_internal!` are feature-independent (the first three generate no code of their own; the two bodies need only `with_ast_arena`), `parse_format!` needs the `format` feature:

- `parse_ast!($goalness, $lang, $source, $goal, $arena)` — the per-language parse call. `goal` (TypeScript) threads the decoded goal into `$lang::parse_with_goal`; `nogoal` (Svelte, CSS) drops it and calls `$lang::parse`. `$lang` resolves in the *caller's* scope, so this crate depends on no language crate.
- `parse_ast_for_format!($goalness, $lang, $folded, $goal, $arena)` — the format path's twin, over the CR-folded document (`&FoldedSource`) rather than a source string: it calls the language's `parse_folded`, which maps a parse error back onto the caller's source and, for Svelte, refuses the one lone `<CR>` the fold would change the meaning of (`tsv_svelte::parse_folded` — a positionless `ParseError::refusal`, so each binding reports it as it reports a source over the size cap). Its `$goal` is an `Option`: `goal` calls `tsv_ts::parse_folded`, over `parse_with_goal_or_fallback`, so a named source type is exact and an unnamed one takes the module-then-script fallback (see [../../docs/cli.md §Multi-File Formatting](../../docs/cli.md#multi-file-formatting)); `nogoal` drops it as above. The parse exports keep `parse_ast!`, whose goal is settled: their product is a wire carrying `Program.sourceType`, a claim no retry may make depend on the input.
- `parse_convert!($goalness, $lang, $convert, $source, $goal, $map_err)` — a binding's whole parse export body: parse through `parse_ast!` into the per-thread AST arena and convert with the language's `$convert` writer (`convert_ast_json_bytes` for the C FFI, `convert_ast_json_string` for the two JS bindings), a parse error through `$map_err`. Its `$goal` is the binding's decoded `Option`, an unset one read as `Goal::default()` (`Module`): a parse runs no fallback. No fold either — the parse wire's offsets are a contract over the caller's own bytes.
- `parse_internal!($goalness, $lang, $source, $goal, $map_err)` — the benchmark-only `parse_internal_*` body: `parse_convert!` without the convert, `Result<(), _>`, the AST held live by `black_box` inside the arena closure (the one scope where it exists — outside it the parse is dead code the optimizer may delete).
- `parse_format!($goalness, $lang, $source, $goal, $map_err)` — a binding's whole format export body, behind the **`format`** feature: fold the source's carriage returns, parse the folded document through `parse_ast_for_format!` into the per-thread AST arena, and print it into the per-thread doc arena, with a parse error mapped back onto the caller's own source (`FoldedSource::parse_with`) and then through `$map_err`, the binding's own error type.
- `goal_allowed!($goalness)` — `true` / `false`, read by each binding's own goal decoder (`ffi_source_type`, `napi_source_type`, `wasm_source_type`).

Plus one type and one function:

- `Family { Parse, Format }` — which export family a call belongs to: what an unset source type means (a parse reads it as `Module`, a format as none named), and the noun a refusal names (`Family::noun`). `tsv_ffi`'s `ffi_source_type` reads it to accept the unspecified code from the format exports alone; the two string-axis bindings pass it to `decode_source_type`.
- `decode_source_type(source_type, allowed, family, from_source_type)` — the two string-axis bindings' shared decoder (`napi_source_type`, and `wasm_source_type` past its own not-a-string arm): unset stays unset, a source type on a goalless language and a value naming neither goal are refused, worded once for both (the npm facade restates the words in JS). A plain function generic over the goal type, the language crate's spelling table passed in, so this crate still depends on no language crate. `tsv_ffi` spells the axis as a code and keeps its own `ffi_source_type`.

The load-bearing property is that **one `$goalness` tag drives every macro**: a language with no axis *rejects* a set goal rather than ignoring it, and the macro that picks the parse call and the macro that licenses the refusal can't come to disagree about which languages those are. Each binding still owns its own `lang_bindings!` (three different export signatures); the C FFI words its own refusals, and the two string-axis bindings share `decode_source_type`'s.

## Abort safety: take and park

Each helper **takes** its arena out of the thread-local for the call and **parks** it back after — it never holds a `RefCell` borrow guard across `f`. This is the load-bearing decision in the crate; the argument (a WASM trap runs no `Drop` but leaves the instance callable, so a held guard bricks every later call) is in the `src/lib.rs` module docs, along with the two consequences — a panicking call loses its warm arena, and re-entrancy became a fresh-fallback rather than a panic.

What the module docs don't carry, because it is evidence rather than rationale:

- **Measured end-to-end on the built `format` bundle** with a temporary panicking export: with a held guard, a trap made every subsequent `format_typescript` throw; with take/park, calls after the trap return correct output.
- The change was **byte-identical over 211 corpus files and ~4% faster** on the WASM format path (`wasm_format_probe` net 0.95/0.96 across two runs, floor ~1.01). Two effects are inseparable by construction: dropping the borrow guard, and the `const { Cell::new(None) }` thread-local init that only becomes possible once the parked state is `None` (`Bump::new()` is not `const`, so the old form necessarily used std's lazy thread-local storage — a state check per access; the new one is eager, on wasm a plain static).

## Features

- `format` (default) — adds `with_doc_arena`, `parse_format!` + the optional `tsv_lang` dep, re-exported `#[doc(hidden)]` as `__tsv_lang` so `parse_format!`'s expansion reaches `tsv_lang::printing::normalize_carriage_returns` as `$crate::__tsv_lang::…` — a macro path resolves in the *caller's* crate, and this keeps the bindings free of a `tsv_lang` edge of their own.

The **workspace dependency entry is `default-features = false`**, so a binding gets only `with_ast_arena` by default and re-enables `format` from its own `format` feature — that's what keeps the parse-only binding build from pulling `tsv_lang`. A standalone `cargo test -p tsv_arena` uses the crate's own `default = ["format"]`, so both helpers are exercised.

## Consumers

`tsv_ffi`, `tsv_napi`, and `tsv_wasm`. Each maps its `format` feature to `tsv_arena/format`, and builds every export body inside its `lang_bindings!` macro from the same `parse_convert!` / `parse_internal!` / `parse_format!` set over one `goal_allowed!` tag — the parse exports running `with_ast_arena` through the first two, the format export through `parse_format!`, which runs both arena helpers. What stays the binding's own is its host's idiom: decoding the source type and building the error it reports.

For the two **native** bindings the win is heap-churn through the host FFI/N-API layer. For **`tsv_wasm`** it's the per-call `Bump`/`DocArena` allocation in the sandbox (the documented WASM-format allocation-count lever) — measured at a **byte-identical ~2% warm format speedup** (svelte ~3%) on the zzz corpus via `benches/js/diagnostics/wasm_format_probe.ts`, with a negligible cold single-shot cost (one un-pre-sized first allocation; even `npm/cli.js` is warm after its first file) and +0.08% bundle size.
