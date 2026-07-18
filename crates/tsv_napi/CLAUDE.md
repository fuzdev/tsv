# tsv_napi

> N-API bindings for `tsv`. Builds to a `cdylib` (`libtsv_napi.{so,dylib,dll}`) loaded by Node.js / Bun as a native addon, via [napi-rs](https://napi.rs).

## Architecture Position

Depends on `tsv_ts`, `tsv_css`, `tsv_svelte`. The **Node/Bun** sibling of the binding trio: [`tsv_ffi`](../tsv_ffi/) is Deno's C-FFI path (`Deno.dlopen`), [`tsv_wasm`](../tsv_wasm/) is the universal WASM path, and `tsv_napi` is the native path for the N-API runtimes. Same engine, same `lang_bindings!` shape, different binding boundary.

This is a **tsv-scoped carve-out** from the ecosystem N-API deferral — **not** an ecosystem-wide flip.

Like `tsv_ffi`, the bindings reuse a **per-thread AST `Bump`** (`with_ast_arena`) that is `reset()` between calls rather than allocated fresh per call — the bindings are invoked once per file in tight loops, and per-call arena malloc/free churns the system allocator's heap high-water in a way that is measurable through a binding layer. The `format` path likewise reuses a **per-thread doc arena** (`with_doc_arena`, the same shape over `DocArena`, calling each language's `format_in`). Both helpers live in the shared [`tsv_arena`](../tsv_arena/) crate (used by all three bindings — `tsv_ffi`, `tsv_napi`, and `tsv_wasm` — so there's one copy of the subtle reuse/soundness contract, not three hand-synced ones). This crate's `format` feature maps to `tsv_arena/format`, which pulls `tsv_lang` for the `DocArena` type; the parse-only build leaves it off and stays lean.

Build/usage commands live in [../../CLAUDE.md §JS Bindings](../../CLAUDE.md#js-bindings).

## Two-stage rollout

- **(3a) measurement binding — done.** A single-platform local build (`deno task build:napi` → `cargo build -p tsv_napi --release`) drives the **Node** benchmark runner (`benches/js/lib/napi.ts` loads the built cdylib directly via `process.dlopen` — no `.node` rename). **No CI, no cross-platform matrix, no npm publish.**
- **(3b) publish matrix — a fast-follow, decoupled from 0.2.** The cross-platform prebuilt `.node` artifacts (per-platform `optionalDependencies` under a thin `@fuzdev/tsv_napi` loader) + release CI are deferred. They need GitHub release infrastructure (tagged releases hosting the per-platform binaries) that the WASM/npm path doesn't — so N-API publish **must not block** the WASM package publish or the VS Code extension. It's queued immediately after 0.2 (landing as 0.3 if the GitHub setup slips), and is expected to eventually **subsume** the WASM path as tsv's primary native distribution. Do **not** bolt N-API onto the single-machine `deno task publish`.

## Features

Mirrors `tsv_ffi` / `tsv_wasm`:

- `format` (default) — `format_<lang>` exports
- `parse` (default) — `parse_<lang>` + `parse_internal_<lang>` exports, and the `convert` layer on each language crate

## Public API

The `lang_bindings!` macro generates three `#[napi]` functions per language (svelte, typescript, css); the `format`/`parse` features gate which are emitted:

- `parse_<lang>(source) -> string` — JSON AST string (host `JSON.parse`s it — parity with FFI/WASM)
- `parse_<lang>_no_locations(source) -> string` — the span-only variant (drops per-node `loc`; Svelte also `name_loc`; CSS identical to `parse_css`). See [../tsv_ts/CLAUDE.md](../tsv_ts/CLAUDE.md) §Public API.
- `parse_internal_<lang>(source) -> void` — parses without converting (benchmark-only; `black_box` prevents elision)
- `format_<lang>(source) -> string` — formatted source

JS export names are kept **snake_case** via `#[napi(js_name = "…")]` (napi-rs would otherwise camelCase them) so the addon's surface matches `tsv_wasm`'s.

## Marshalling & errors

napi-rs marshals the JS string into a Rust `String` and the returned `String` back out — **no raw pointers, no manual free** (unlike `tsv_ffi`). Engine errors are returned as `napi::Result::Err(napi::Error)`, which napi-rs converts to a **thrown JS error** — there is no `{"error": …}` envelope to inspect (the FFI shape); a throw just propagates.

**Panic profile:** the published `[profile.release]` is `panic = "abort"`, so a Rust panic aborts the process (napi-rs can only catch panics under `panic = "unwind"`). The measurement binding ships release for native-vs-native parity with `tsv_ffi`; the bench corpus is curated so tsv doesn't panic. If a panic-tolerant build is needed, the `[profile.corpus]` (`panic = "unwind"`) precedent exists.

## Files

- `src/lib.rs` — All bindings: the `lang_bindings!` macro, the three `lang_bindings!` invocations, and a `#[cfg(test)]` module. The reusable arenas are imported from `tsv_arena` (`with_ast_arena`, plus `with_doc_arena` under the `format` feature)
- `build.rs` — `napi_build::setup()` (linker config for the addon)
- `Cargo.toml` — `crate-type = ["cdylib"]`; `unsafe_code = "allow"` (N-API generates unsafe code); deps `napi` + `napi-derive` (3.x) + `tsv_arena`, build-dep `napi-build` (2.x). `format` → `tsv_arena/format`

The in-crate test module drives **every entry point** in-process — all three languages × `parse` / `parse_internal` / `format` — so `cargo test` exercises the native binding without a Node host (the Deno/WASM smoke paths don't cover napi). The per-language `parse` assertions check the language's own JSON root type (`Program` / `StyleSheetFile` / `Root`), which also guards the `lang_bindings!` wiring against a transposed invocation; the error tests cover the thrown-`napi::Error` arm for both parse and format; one test exercises this crate's distinctive risk — that the per-thread `with_ast_arena` / `with_doc_arena` `reset()` cleanly between back-to-back calls; and a multibyte round-trip guards the char-offset boundary. What `cargo test` can **not** reach is the napi-rs **marshalling** layer (the `#[napi]` JS-string ↔ Rust `String` conversion and the `napi::Error` → *thrown* JS error path) — that's covered by the bench's Node runner and by `scripts/test_napi.ts` (`deno task test:napi`), which `process.dlopen`s the built addon and asserts a format, a JSON-AST parse, a thrown error, and a multibyte round-trip across the real JS boundary.
