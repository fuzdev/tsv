# tsv_ffi

> C ABI bindings for `tsv`. Builds to `libtsv_ffi.{so,dylib,dll}` (cdylib) for use from any FFI-capable language.

## Architecture Position

Depends on `tsv_ts`, `tsv_css`, `tsv_svelte`. Sibling binding crate: [`tsv_wasm`](../tsv_wasm/) (WebAssembly). Consumers include Deno FFI, Python `ctypes`, and any other C-FFI host. N-API is not used.

The bindings hold a **per-thread reusable AST `Bump`** (`with_ast_arena`) that is `reset()` between calls rather than allocated fresh per call: the bindings are invoked once per file in tight loops, and per-call arena malloc/free churns the system allocator's heap high-water in a way that is measurable through a host FFI layer. `reset()` retains the largest chunk and rewinds, so a warm thread does no per-call malloc/free; this replaces the old per-call `with_capacity` pre-size (hence no `tsv_lang` dependency). The per-file AST is fully consumed before the next call's `reset()`, so the reuse is sound (incl. after a `catch_unwind`-caught panic).

Build/usage commands live in [../../CLAUDE.md §JS Bindings](../../CLAUDE.md#js-bindings).

## Features

Mirrors `tsv_wasm`'s split so the bench can size scope-matched native artifacts:

- `format` (default) — `tsv_format_<lang>` exports
- `parse` (default) — `tsv_parse_<lang>` + `tsv_parse_internal_<lang>` exports, and the `convert` layer on each language crate

The default both-features build is the full `libtsv_ffi` the bench perf rows load and any FFI host links. The size table also reports two subset builds, each into its own target dir so they don't clobber the full lib: `--no-default-features --features format` (the native mirror of `@fuzdev/tsv_format_wasm`, no convert layer, scope-matched to oxfmt) and `--no-default-features --features parse` (the mirror of `@fuzdev/tsv_parse_wasm`, printers dropped, scope-matched to oxc-parser). See `deno task build:ffi:format` / `build:ffi:parse`.

## Public API

The `lang_bindings!` macro generates three `extern "C"` functions per language (svelte, typescript, css) — the full default build; the `format`/`parse` features gate which are emitted (see [Features](#features) above):

- `tsv_parse_<lang>` — JSON AST (public, converted)
- `tsv_parse_internal_<lang>` — Empty string (benchmark-only; AST is built but not converted/serialized — `std::hint::black_box` prevents elision)
- `tsv_format_<lang>` — Formatted source

Plus `tsv_free(ptr, len)` for deallocation.

All return-pointer functions share the signature `(source_ptr: *const u8, source_len: usize, out_len: *mut usize) -> *mut u8`.

## Memory & Safety Contract

- **Allocation**: tsv allocates returned buffers as `Box<[u8]>` and leaks them via `Box::into_raw`. Length is written to `*out_len`.
- **Free**: Caller MUST call `tsv_free(ptr, *out_len)` exactly once per returned pointer. `tsv_free` no-ops on null or zero length.
- **UTF-8 input**: `source_ptr`/`source_len` must point to valid UTF-8. Invalid UTF-8 returns an error JSON (`{"error": "Invalid UTF-8: ..."}`), not a crash.
- **Errors**: All errors surface as JSON-shaped output (`{"error": "..."}`) with a valid pointer the caller still must free. There is no separate error channel.
- **Panic safety**: Every entry point wraps the work in `std::panic::catch_unwind`. Panics are caught (when built with `panic = "unwind"`) and converted to `{"error": "panic: ..."}`. Under `panic = "abort"` profiles, panics still abort — the catch is profile-dependent.

## Files

- `src/lib.rs` — All bindings: `lang_bindings!` macro, source-extraction helpers, `tsv_free`, and a `#[cfg(test)]` module
- `Cargo.toml` — `crate-type = ["cdylib"]`; `unsafe_code = "allow"` (FFI requires it)

The in-crate test module drives every entry point in-process (real
alloc → write `out_len` → `tsv_free` round-trip), covering the happy path per
language, JSON-error returns on invalid syntax, the invalid-UTF-8 path, empty
input, and `tsv_free` null/zero no-ops. It runs under `cargo test` (so CI's
`check` job exercises the native binding — the Deno/WASM smoke paths don't).
