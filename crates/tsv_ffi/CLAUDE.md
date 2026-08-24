# tsv_ffi

> C ABI bindings for `tsv`. Builds to `libtsv_ffi.{so,dylib,dll}` (cdylib) for use from any FFI-capable language.

## Architecture Position

Depends on `tsv_ts`, `tsv_css`, `tsv_svelte`. Sibling binding crates: [`tsv_wasm`](../tsv_wasm/) (WebAssembly) and [`tsv_napi`](../tsv_napi/) (N-API, the Node/Bun native path). This crate is the C-ABI path — consumers include Deno FFI, Python `ctypes`, and any other C-FFI host. Node/Bun use `tsv_napi` instead (no C-FFI glue); the per-thread arena reuse below is shared across all three bindings via the [`tsv_arena`](../tsv_arena/) crate.

The bindings reuse a **per-thread AST `Bump`** (`with_ast_arena`) that is `reset()` between calls rather than allocated fresh per call: the bindings are invoked once per file in tight loops, and per-call arena malloc/free churns the system allocator's heap high-water in a way that is measurable through a host FFI layer. `reset()` retains the largest chunk and rewinds, so a warm thread does no per-call malloc/free. The per-file AST is fully consumed before the next call's `reset()`, so the reuse is sound (incl. after a `catch_unwind`-caught panic). The `format` path additionally reuses a **per-thread doc arena** (`with_doc_arena`, the same shape over `DocArena` and calling each language's `format_in`). Both helpers live in the shared [`tsv_arena`](../tsv_arena/) crate (one copy for all three bindings — `tsv_ffi`, `tsv_napi`, `tsv_wasm`); this crate's `format` feature maps to `tsv_arena/format`, which pulls `tsv_lang` for the `DocArena` type, so the parse-only build stays lean.

Build/usage commands live in [../../CLAUDE.md §JS Bindings](../../CLAUDE.md#js-bindings).

## Features

Mirrors `tsv_wasm`'s split so the bench can size scope-matched native artifacts:

- `format` (default) — `tsv_format_<lang>` exports
- `parse` (default) — `tsv_parse_<lang>` + `tsv_parse_internal_<lang>` exports, and the `convert` layer on each language crate

The default both-features build is the full `libtsv_ffi` the bench perf rows load and any FFI host links. The size table also reports two subset builds, each into its own target dir so they don't clobber the full lib: `--no-default-features --features format` (the native mirror of `@fuzdev/tsv_format_wasm`, no convert layer, scope-matched to oxfmt) and `--no-default-features --features parse` (the mirror of `@fuzdev/tsv_parse_wasm`, printers dropped, scope-matched to oxc-parser). See `deno task build:ffi:format` / `build:ffi:parse` — built only by `build:bench`, which the gate never runs, so `deno task typecheck:features` (in `check`) `cargo check`s each half on its own.

## Public API

The `lang_bindings!` macro generates four `extern "C"` functions per language (svelte, typescript, css) — the full default build; the `format`/`parse` features gate which are emitted (see [Features](#features) above):

- `tsv_parse_<lang>` — JSON AST (public, converted)
- `tsv_parse_<lang>_no_locations` — the span-only variant (drops per-node `loc`; Svelte also drops `name_loc`). CSS is byte-identical to `tsv_parse_css` (`parseCss` emits no `loc`). See [../tsv_ts/CLAUDE.md](../tsv_ts/CLAUDE.md) §Public API.
- `tsv_parse_internal_<lang>` — Empty string (benchmark-only; AST is built but not converted/serialized — `std::hint::black_box` prevents elision)
- `tsv_format_<lang>` — Formatted source

Plus `tsv_free(ptr, len)` for deallocation.

### The uniform signature

Every return-pointer function has the same shape:

```c
uint8_t *tsv_<op>_<lang>(const uint8_t *source_ptr, size_t source_len,
                         uint32_t goal, size_t *out_len, uint32_t *out_status);
```

One export per (language, operation): there is no goalless twin of a goal-aware
export, and no arity that varies by language. A host writes one call shape and
one symbol table.

`goal` is the parse goal — `0` = Module, `1` = Script; any other code is an
error, never a silent default. At Script goal `await` is an ordinary identifier
and `import`/`export`/`import.meta` are syntax errors. **Svelte and CSS REJECT a
non-zero code** rather than ignoring it: Svelte hard-wires `Module` and CSS has
no goal axis, so a caller passing `1` there asked for something that cannot be
honored and is told — the same stance `tsv_wasm`'s `read_options` takes when it
rejects the `goal` key outright (see [../tsv_wasm/CLAUDE.md](../tsv_wasm/CLAUDE.md)
§Format Options). `tsv_napi` spells the axis as a trailing optional goal string;
each binding has its own `lang_bindings!`, but all three read the **same**
`parse_ast!` / `goal_allowed!` pair out of [`tsv_arena`](../tsv_arena/), so which
languages have a goal axis is one fact in one place and coverage is identical by
construction.

## Memory & Safety Contract

- **Allocation**: tsv allocates returned buffers as `Box<[u8]>` and leaks them via `Box::into_raw`. Length is written to `*out_len`.
- **Free**: Caller MUST call `tsv_free(ptr, *out_len)` exactly once per returned pointer. `tsv_free` no-ops on null or zero length.
- **UTF-8 input**: `source_ptr`/`source_len` must point to valid UTF-8. Invalid UTF-8 is reported as an error (`{"error": "Invalid UTF-8: ..."}`), not a crash. A null `source_ptr` with `source_len == 0` is accepted as the empty source (FFI hosts commonly pass (null, 0) for an empty buffer); null with a non-zero length is an error.
- **Errors: the status word, never the payload.** `*out_status` receives `TSV_STATUS_OK` (0) or `TSV_STATUS_ERROR` (1), written exactly once per call alongside `*out_len` — one site writes both (`bytes_to_ptr`), so they cannot disagree about which call they describe. That word is the whole verdict. A failed call's payload IS a `{"error": "..."}` JSON object with a valid pointer the caller still must free, but a caller must not sniff for it: formatted output is arbitrary source text, so no prefix test is sound in general. `tsv_parse_internal_*` is the sharpest case — its success payload is empty, carrying no shape to read a verdict off at all.
- **Panic safety**: Every entry point wraps the work in `std::panic::catch_unwind`. Panics are caught (when built with `panic = "unwind"`) and reported as `TSV_STATUS_ERROR` with a `{"error": "panic: ..."}` payload. Under `panic = "abort"` profiles, panics still abort — the catch is profile-dependent.

## Files

- `src/lib.rs` — All bindings: the `lang_bindings!` macro (over the shared `parse_ast!` / `goal_allowed!` goal axis, with `ffi_goal` decoding the `u32` code), the three `lang_bindings!` invocations, the `TSV_STATUS_*` constants, source-extraction helpers, `tsv_free`, and a `#[cfg(test)]` module. The reusable arenas and the goal macros are imported from `tsv_arena` (`with_ast_arena`, plus `with_doc_arena` under the `format` feature)
- `Cargo.toml` — `crate-type = ["cdylib"]`; `unsafe_code = "allow"` (FFI requires it); deps include `tsv_arena` (`format` → `tsv_arena/format`)

The in-crate test module drives every entry point in-process (real
alloc → write `out_len`/`out_status` → `tsv_free` round-trip), covering the happy
path per language, the error status on invalid syntax, the goal axis and its two
refusals (an unknown code; a goal on a goalless language), the invalid-UTF-8 path,
empty input, and `tsv_free` null/zero no-ops. Its `call_raw` helper pins the one
direction of status↔payload agreement that is a contract — an error status must
carry an `{"error": …}` payload — and deliberately leaves the converse unasserted,
since asserting a success payload *isn't* an error object would rebuild the content
sniff the status channel exists to retire. It runs under `cargo test` (so CI's
`check` job exercises the native binding — the Deno/WASM smoke paths don't).
