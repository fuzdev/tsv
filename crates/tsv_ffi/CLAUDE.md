# tsv_ffi

> C ABI bindings for `tsv`. Builds to `libtsv_ffi.{so,dylib,dll}` (cdylib) for use from any FFI-capable language.

## Architecture Position

Depends on `tsv_ts`, `tsv_css`, `tsv_svelte`. The C-ABI path, for Deno FFI, Python `ctypes`, and any other C-FFI host. Sibling binding crates: [`tsv_wasm`](../tsv_wasm/) (WebAssembly) and [`tsv_napi`](../tsv_napi/) (N-API — what Node/Bun use instead, no C-FFI glue).

The bindings reuse a **per-thread AST `Bump`** (`with_ast_arena`), `reset()` between calls rather than allocated fresh per call: they are invoked once per file in tight loops, and per-call arena malloc/free churns the system allocator's heap high-water measurably through a host FFI layer. `reset()` retains the largest chunk and rewinds, so a warm thread does no per-call malloc/free; the per-file AST is fully consumed before the next call's `reset()`, so the reuse is sound (incl. after a `catch_unwind`-caught panic). The `format` path also reuses a **per-thread doc arena** (`with_doc_arena`, the same shape over `DocArena`, calling each language's `format_folded_in` — the fold ahead of the parse hands its line verdict to the printer, so a document is walked once). Both helpers live in [`tsv_arena`](../tsv_arena/), one copy for all three bindings. This crate's `format` feature maps to `tsv_arena/format`, which pulls `tsv_lang` for the `DocArena` type and the line-terminator fold `parse_format!` runs — re-exported by `tsv_arena`, so this crate has no `tsv_lang` edge of its own and the parse-only build stays lean.

Build/usage commands live in [../../CLAUDE.md §JS Bindings](../../CLAUDE.md#js-bindings).

## Features

Mirrors `tsv_wasm`'s split so the bench can size scope-matched native artifacts:

- `format` (default) — `tsv_format_<lang>` exports
- `parse` (default) — `tsv_parse_<lang>` + `tsv_parse_internal_<lang>` exports, and the `convert` layer on each language crate

The default (both features) is the full `libtsv_ffi` the bench perf rows load and any FFI host links. The size table also reports two subset builds, each in its own target dir so they don't clobber the full lib: `--no-default-features --features format` (the native mirror of `@fuzdev/tsv-format-wasm`, no convert layer, scope-matched to oxfmt) and `--no-default-features --features parse` (the mirror of `@fuzdev/tsv-parse-wasm`, printers dropped, scope-matched to oxc-parser). See `deno task build:ffi:format` / `build:ffi:parse` — built only by `build:bench`, which the gate never runs, so `deno task typecheck:features` (in `check`) `cargo check`s each half on its own.

## Public API

The `lang_bindings!` macro generates three `extern "C"` functions per language (svelte, typescript, css) — the full default build; the `format`/`parse` features gate which are emitted (see [Features](#features) above):

- `tsv_parse_<lang>` — the span-only JSON AST (`start`/`end` offsets, no per-node `loc`; Svelte also no `name_loc`) — the one parse wire every binding emits. See [../tsv_ts/CLAUDE.md](../tsv_ts/CLAUDE.md) §Public API. The `loc`-bearing wire has no C-ABI export; the corpus tools ask `tsv_debug loc_wires --stdin` for it.
- `tsv_parse_internal_<lang>` — Empty payload (`*out_len == 0` with `TSV_STATUS_OK`; benchmark-only; AST is built but not converted/serialized — `std::hint::black_box` prevents elision)
- `tsv_format_<lang>` — Formatted source

Plus `tsv_free(ptr, len)` for deallocation.

### The uniform signature

Every return-pointer function has the same shape:

```c
uint8_t *tsv_<op>_<lang>(const uint8_t *source_ptr, size_t source_len,
                         uint32_t source_type, size_t *out_len,
                         uint32_t *out_status);
```

One export per (language, operation): there is no goalless twin of a goal-aware
export, and no arity that varies by language. A host writes one call shape and
one symbol table.

`source_type` is the parse goal — `0` = Module, `1` = Script, `2` = unspecified;
any other code is an error, never a silent default. At Script goal `await` is an
ordinary identifier, so a top-level `await` with an operand is a syntax error — as
are top-level `import`/`export` declarations, `import.meta` and a top-level `for await`.

- **Code `2`** says the caller named **no** source type, and is accepted by the
  **format exports only**, on every language: a formatter answers it with the module
  grammar retried as a script (`tsv_ts::parse_with_goal_or_fallback` — see
  [../../docs/cli.md §Multi-File Formatting](../../docs/cli.md#multi-file-formatting)),
  and a goalless one has nothing to answer at all, while a parse export's wire carries
  a `Program.sourceType` that one settled grammar has to produce.
- **Svelte and CSS REJECT code `1`** rather than ignoring it: Svelte hard-wires
  `Module` and CSS has no goal axis, so the caller asked for something that cannot be
  honored and is told — the stance `tsv_wasm`'s flat exports and the npm facade's
  options reader share (see [../tsv_wasm/CLAUDE.md](../tsv_wasm/CLAUDE.md) §Format
  Options).

`tsv_napi` and `tsv_wasm` spell the axis as a trailing optional `sourceType` string.
Each binding has its own `lang_bindings!`, but all three build their bodies from the
**same** [`tsv_arena`](../tsv_arena/) macros (`parse_convert!` / `parse_internal!` /
`parse_format!`, over one `goal_allowed!` tag), so which languages have a goal axis is
one fact in one place and coverage is identical by construction.

## Memory & Safety Contract

- **Allocation**: tsv allocates returned buffers as `Box<[u8]>` and leaks them via `Box::into_raw`. Length is written to `*out_len`.
- **Free**: Caller MUST call `tsv_free(ptr, *out_len)` exactly once per returned pointer. `tsv_free` no-ops on null or zero length.
- **UTF-8 input**: `source_ptr`/`source_len` must point to valid UTF-8. Invalid UTF-8 is reported as an error (`{"error": "Invalid UTF-8: ..."}`), not a crash. A null `source_ptr` with `source_len == 0` is accepted as the empty source (FFI hosts commonly pass (null, 0) for an empty buffer); null with a non-zero length is an error.
- **Errors: the status word, never the payload.** `*out_status` receives `TSV_STATUS_OK` (0) or `TSV_STATUS_ERROR` (1), written exactly once per call alongside `*out_len` — one site writes both (`bytes_to_ptr`), so they cannot disagree about which call they describe. That word is the whole verdict. A failed call's payload IS a `{"error": "..."}` JSON object with a valid pointer the caller still must free, but a caller must not sniff for it: formatted output is arbitrary source text, so no prefix test is sound in general. `tsv_parse_internal_*` is the sharpest case — its success payload is empty, carrying no shape to read a verdict off at all.
- **Panic safety**: Every entry point wraps the work in `std::panic::catch_unwind`. Built with `panic = "unwind"`, a panic is caught and reported as `TSV_STATUS_ERROR` with a `{"error": "panic: ..."}` payload; under `panic = "abort"` profiles it still aborts.

## Files

- `src/lib.rs` — All bindings: the `lang_bindings!` macro (its bodies the shared `tsv_arena` macros — `parse_convert!` / `parse_internal!` for the parse exports, `parse_format!` carrying the format path's `Option<Goal>` — with `ffi_source_type` decoding the `u32` code per `tsv_arena::Family`), the three `lang_bindings!` invocations, the `TSV_STATUS_*` constants, source-extraction helpers, `tsv_free`, and a `#[cfg(test)]` module. `with_ast_arena` is imported only by the tests; every export reaches the arenas through the macros
- `Cargo.toml` — `crate-type = ["cdylib"]`; `unsafe_code = "allow"` (FFI requires it); deps include `tsv_arena` (`format` → `tsv_arena/format`)

The in-crate test module drives every entry point in-process (real
alloc → write `out_len`/`out_status` → `tsv_free` round-trip): the happy
path per language, the error status on invalid syntax, the goal axis and its three
refusals (an unknown code; a script goal on a goalless language; the unspecified
code on a parse export) beside the fallback that code answers, the invalid-UTF-8 path,
empty input, and `tsv_free` null/zero no-ops. Its `call_raw` helper pins the one
direction of status↔payload agreement that is a contract — an error status must
carry an `{"error": …}` payload — and deliberately leaves the converse unasserted,
since asserting a success payload *isn't* an error object would rebuild the content
sniff the status channel exists to retire. It runs under `cargo test` (so CI's
`check` job exercises the native binding — the Deno/WASM smoke paths don't).
