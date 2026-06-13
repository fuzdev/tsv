# tsv_ffi

> C ABI bindings for `tsv`. Builds to `libtsv_ffi.{so,dylib,dll}` (cdylib) for use from any FFI-capable language.

## Architecture Position

Depends on `tsv_ts`, `tsv_css`, `tsv_svelte` (each with `convert` feature) and `tsv_lang` transitively. Sibling binding crate: [`tsv_wasm`](../tsv_wasm/) (WebAssembly). Consumers include Deno FFI, Python `ctypes`, and any other C-FFI host. This crate does not use N-API; a separate `tsv_napi` crate (Node/Bun native addon) is planned as a third sibling binding alongside this one and `tsv_wasm`.

Build/usage commands live in [../../CLAUDE.md §JS Bindings](../../CLAUDE.md#js-bindings).

## Public API

The `lang_bindings!` macro generates three `extern "C"` functions per language (svelte, typescript, css):

| Function                    | Returns                                                                                                            |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `tsv_parse_<lang>`          | JSON AST (public, converted)                                                                                       |
| `tsv_parse_internal_<lang>` | Empty string (benchmark-only; AST is built but not converted/serialized — `std::hint::black_box` prevents elision) |
| `tsv_format_<lang>`         | Formatted source                                                                                                   |

Plus `tsv_free(ptr, len)` for deallocation.

All return-pointer functions share the signature `(source_ptr: *const u8, source_len: usize, out_len: *mut usize) -> *mut u8`.

## Memory & Safety Contract

- **Allocation**: tsv allocates returned buffers as `Box<[u8]>` and leaks them via `Box::into_raw`. Length is written to `*out_len`.
- **Free**: Caller MUST call `tsv_free(ptr, *out_len)` exactly once per returned pointer. `tsv_free` no-ops on null or zero length.
- **UTF-8 input**: `source_ptr`/`source_len` must point to valid UTF-8. Invalid UTF-8 returns an error JSON (`{"error": "Invalid UTF-8: ..."}`), not a crash.
- **Errors**: All errors surface as JSON-shaped output (`{"error": "..."}`) with a valid pointer the caller still must free. There is no separate error channel.
- **Panic safety**: Every entry point wraps the work in `std::panic::catch_unwind`. Panics are caught (when built with `panic = "unwind"`) and converted to `{"error": "panic: ..."}`. Under `panic = "abort"` profiles, panics still abort — the catch is profile-dependent.

## Files

| File         | Purpose                                                                                                            |
| ------------ | ----------------------------------------------------------------------------------------------------------------- |
| `src/lib.rs` | All bindings: `lang_bindings!` macro, source-extraction helpers, `tsv_free`, and a `#[cfg(test)]` module           |
| `Cargo.toml` | `crate-type = ["cdylib"]`; `unsafe_code = "allow"` (FFI requires it)                                               |

The in-crate test module drives every entry point in-process (real
alloc → write `out_len` → `tsv_free` round-trip), covering the happy path per
language, JSON-error returns on invalid syntax, the invalid-UTF-8 path, empty
input, and `tsv_free` null/zero no-ops. It runs under `cargo test` (so CI's
`check` job exercises the native binding — the Deno/WASM smoke paths don't).
