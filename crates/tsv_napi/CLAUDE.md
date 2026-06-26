# tsv_napi

> N-API bindings for `tsv`. Builds to a `cdylib` (`libtsv_napi.{so,dylib,dll}`) loaded by Node.js / Bun as a native addon, via [napi-rs](https://napi.rs).

## Architecture Position

Depends on `tsv_ts`, `tsv_css`, `tsv_svelte`. The **Node/Bun** sibling of the binding trio: [`tsv_ffi`](../tsv_ffi/) is Deno's C-FFI path (`Deno.dlopen`), [`tsv_wasm`](../tsv_wasm/) is the universal WASM path, and `tsv_napi` is the native path for the N-API runtimes. Same engine, same `lang_bindings!` shape, different binding boundary.

This is a **tsv-scoped carve-out** from the ecosystem N-API deferral (decided 2026-06-13; the `napi` crate approved 2026-06-26). It is **not** an ecosystem-wide flip.

Like `tsv_ffi`, the bindings hold a **per-thread reusable AST `Bump`** (`with_ast_arena`) that is `reset()` between calls rather than allocated fresh per call — the bindings are invoked once per file in tight loops, and per-call arena malloc/free churns the system allocator's heap high-water in a way that is measurable through a binding layer. The helper is currently **duplicated** from `tsv_ffi::with_ast_arena` (kept in lockstep by hand); factoring both onto one shared helper is a planned follow-up.

Build/usage commands live in [../../CLAUDE.md §JS Bindings](../../CLAUDE.md#js-bindings).

## Two-stage rollout

- **(3a) measurement binding — done.** A single-platform local build (`deno task build:napi` → `cargo build -p tsv_napi --release`) drives the **Node** benchmark runner (`benches/js/lib/napi.ts` loads the built cdylib directly via `process.dlopen` — no `.node` rename). **No CI, no cross-platform matrix, no npm publish.**
- **(3b) publish matrix — not yet.** The cross-platform prebuilt `.node` artifacts (per-platform `optionalDependencies` under a thin `@fuzdev/tsv_napi` loader) + release CI are deferred, targeted for the 0.2 release. Do **not** bolt N-API onto the single-machine `deno task publish`.

## Features

Mirrors `tsv_ffi` / `tsv_wasm`:

- `format` (default) — `format_<lang>` exports
- `parse` (default) — `parse_<lang>` + `parse_internal_<lang>` exports, and the `convert` layer on each language crate

## Public API

The `lang_bindings!` macro generates three `#[napi]` functions per language (svelte, typescript, css); the `format`/`parse` features gate which are emitted:

- `parse_<lang>(source) -> string` — JSON AST string (host `JSON.parse`s it — parity with FFI/WASM)
- `parse_internal_<lang>(source) -> void` — parses without converting (benchmark-only; `black_box` prevents elision)
- `format_<lang>(source) -> string` — formatted source

JS export names are kept **snake_case** via `#[napi(js_name = "…")]` (napi-rs would otherwise camelCase them) so the addon's surface matches `tsv_wasm`'s.

## Marshalling & errors

napi-rs marshals the JS string into a Rust `String` and the returned `String` back out — **no raw pointers, no manual free** (unlike `tsv_ffi`). Engine errors are returned as `napi::Result::Err(napi::Error)`, which napi-rs converts to a **thrown JS error** — there is no `{"error": …}` envelope to inspect (the FFI shape); a throw just propagates.

**Panic profile:** the published `[profile.release]` is `panic = "abort"`, so a Rust panic aborts the process (napi-rs can only catch panics under `panic = "unwind"`). The measurement binding ships release for native-vs-native parity with `tsv_ffi`; the bench corpus is curated so tsv doesn't panic. If a panic-tolerant build is needed, the `[profile.corpus]` (`panic = "unwind"`) precedent exists.

## Files

- `src/lib.rs` — All bindings: `with_ast_arena`, the `lang_bindings!` macro, the three `lang_bindings!` invocations, and a `#[cfg(test)]` module
- `build.rs` — `napi_build::setup()` (linker config for the addon)
- `Cargo.toml` — `crate-type = ["cdylib"]`; `unsafe_code = "allow"` (N-API generates unsafe code); deps `napi` + `napi-derive` (3.x), build-dep `napi-build` (2.x)

The in-crate test module drives every entry point in-process (`cargo test` exercises the native binding without a Node host — the Deno/WASM smoke paths don't cover napi). Node-side marshalling is covered by the bench's Node runner (and, at 3b, a `scripts/test_napi.ts`).
