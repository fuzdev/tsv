# tsv_napi

> N-API bindings for `tsv`. Builds to a `cdylib` (`libtsv_napi.{so,dylib,dll}`) loaded by Node.js / Bun as a native addon, via [napi-rs](https://napi.rs).

## Architecture Position

Depends on `tsv_ts`, `tsv_css`, `tsv_svelte`. The **Node/Bun** sibling of the binding trio: [`tsv_ffi`](../tsv_ffi/) is Deno's C-FFI path (`Deno.dlopen`), [`tsv_wasm`](../tsv_wasm/) is the universal WASM path, and `tsv_napi` is the native path for the N-API runtimes. Same engine, same `lang_bindings!` shape, different binding boundary.

This is a **tsv-scoped carve-out** from the ecosystem N-API deferral — **not** an ecosystem-wide flip.

Like `tsv_ffi`, the bindings reuse a **per-thread AST `Bump`** (`with_ast_arena`) that is `reset()` between calls rather than allocated fresh per call — the bindings are invoked once per file in tight loops, and per-call arena malloc/free churns the system allocator's heap high-water in a way that is measurable through a binding layer. The `format` path likewise reuses a **per-thread doc arena** (`with_doc_arena`, the same shape over `DocArena`, calling each language's `format_in`). Both helpers live in the shared [`tsv_arena`](../tsv_arena/) crate (used by all three bindings — `tsv_ffi`, `tsv_napi`, and `tsv_wasm` — so there's one copy of the subtle reuse/soundness contract, not three hand-synced ones). This crate's `format` feature maps to `tsv_arena/format`, which pulls `tsv_lang` for the `DocArena` type; the parse-only build leaves it off and stays lean.

Build/usage commands live in [../../CLAUDE.md §JS Bindings](../../CLAUDE.md#js-bindings).

## Two-stage rollout

- **(3a) measurement binding — done.** A single-platform local build (`deno task build:napi` → `cargo build -p tsv_napi --profile napi`) drives the **Node** benchmark runner (`benches/js/lib/napi.ts` loads the built cdylib from `target/napi/` directly via `process.dlopen` — no `.node` rename). CI builds and boundary-tests the addon per OS (the `platforms` job runs `deno task test:napi` on macOS + Windows); **no cross-platform publish yet.**
- **(3b) publish matrix — targets 0.3 (napi-only), decoupled from the WASM releases.** The cross-platform prebuilt `.node` artifacts (per-platform `optionalDependencies` under a thin `@fuzdev/tsv_napi` loader) + release CI. They need GitHub release infrastructure (a tag-triggered matrix workflow) that the WASM/npm path doesn't — so N-API publish **must not block** the WASM package publish or the VS Code extension. It is expected to eventually **subsume** the WASM path as tsv's primary native distribution. Do **not** bolt N-API onto the single-machine `deno task publish`.

## Features

Mirrors `tsv_ffi` / `tsv_wasm`:

- `format` (default) — `format_<lang>` exports
- `parse` (default) — `parse_<lang>` + `parse_internal_<lang>` exports, and the `convert` layer on each language crate
- `panic_probe` (test-only, never default) — the `__panic_probe` export driving the panic-contract test; see §Marshalling & errors

## Public API

The `lang_bindings!` macro generates three `#[napi]` functions per language (svelte, typescript, css); the `format`/`parse` features gate which are emitted:

- `parse_<lang>(source) -> string` — JSON AST string (host `JSON.parse`s it — parity with FFI/WASM)
- `parse_<lang>_no_locations(source) -> string` — the span-only variant (drops per-node `loc`; Svelte also `name_loc`; CSS identical to `parse_css`). See [../tsv_ts/CLAUDE.md](../tsv_ts/CLAUDE.md) §Public API.
- `parse_internal_<lang>(source) -> void` — parses without converting (benchmark-only; `black_box` prevents elision)
- `format_<lang>(source) -> string` — formatted source

Plus, outside the macro, three goal-aware TypeScript parse exports taking a
trailing goal string (`"script"` / `"module"`): `parse_typescript_with_goal`,
`parse_typescript_no_locations_with_goal`, `parse_internal_typescript_with_goal`.

JS export names are kept **snake_case** via `#[napi(js_name = "…")]` (napi-rs would otherwise camelCase them) so the addon's names match `tsv_wasm`'s. The per-call axes are where this surface diverges from `tsv_wasm`, in two ways:

- **Shape**, on parse: here they are flat exports (`parse_<lang>_no_locations`, the TS `*_with_goal` variants), matching `tsv_ffi`'s C-style surface, where `tsv_wasm` takes an acorn-style `{locations?, goal?}` options object (see [../tsv_wasm/CLAUDE.md](../tsv_wasm/CLAUDE.md) §Parse Options & Typed Returns).
- **Coverage**, on format: `format_<lang>(source)` has no goal axis at all, so a `Script`-goal format is unreachable here — `tsv_wasm`'s `format_typescript(source, {goal})` (see that doc's §Format Options) has no counterpart. Only `parse` grew the flat `*_with_goal` variants.

When the publish matrix lands (3b), the `@fuzdev/tsv_napi` loader is the natural place to present that same options bag over these flat exports — and to close the format-goal gap, whether by a `format_typescript_with_goal` export beneath it or by threading the goal in the loader.

## Marshalling & errors

napi-rs marshals the JS string into a Rust `String` and the returned `String` back out — **no raw pointers, no manual free** (unlike `tsv_ffi`). Engine errors are returned as `napi::Result::Err(napi::Error)`, which napi-rs converts to a **thrown JS error** — there is no `{"error": …}` envelope to inspect (the FFI shape); a throw just propagates.

**Panic contract:** a Rust panic — always a tsv bug — surfaces as a **thrown JS error**, never a host abort. Two halves, both required (unwind alone enables the catch, it does not perform it): every export carries `#[napi(catch_unwind)]`, and the addon builds with the workspace **`napi` profile** (`release` + `panic = "unwind"`; `panic` can't be set per-package, and flipping `[profile.release]` would perturb the size-bounded WASM bundles and the abort-profile FFI/CLI artifacts). Bench, `test:napi`, and publish all use the same profile, so the measured artifact is the shipped one (`target/napi/`). After a caught panic the per-thread arenas stay usable — `tsv_arena`'s take/park protocol leaves the thread-local slot empty while a call runs, so unwind and abort converge on the same state. **Stack overflow is not catchable** and still aborts the host. The contract is proven end to end by `scripts/test_napi.ts` via the test-only `panic_probe` feature's `__panic_probe` export (panics inside `with_ast_arena`; the test asserts a thrown error twice, then correct parse + format output — the arena recovery at the real boundary). Published builds never enable the feature; the test skips when the export is absent.

## Files

- `src/lib.rs` — All bindings: the `lang_bindings!` macro, the three `lang_bindings!` invocations, and a `#[cfg(test)]` module. The reusable arenas are imported from `tsv_arena` (`with_ast_arena`, plus `with_doc_arena` under the `format` feature)
- `build.rs` — `napi_build::setup()` (linker config for the addon)
- `Cargo.toml` — `crate-type = ["cdylib"]`; `unsafe_code = "allow"` (N-API generates unsafe code); deps `napi` + `napi-derive` (3.x) + `tsv_arena`, build-dep `napi-build` (2.x). `format` → `tsv_arena/format`

The in-crate test module drives **every entry point** in-process — all three languages × `parse` / `parse_internal` / `format` — so `cargo test` exercises the native binding without a Node host (the Deno/WASM smoke paths don't cover napi). The per-language `parse` assertions check the language's own JSON root type (`Program` / `StyleSheetFile` / `Root`), which also guards the `lang_bindings!` wiring against a transposed invocation; the error tests cover the thrown-`napi::Error` arm for both parse and format; one test exercises this crate's distinctive risk — that the per-thread `with_ast_arena` / `with_doc_arena` `reset()` cleanly between back-to-back calls; and a multibyte round-trip guards the char-offset boundary. What `cargo test` can **not** reach is the napi-rs **marshalling** layer (the `#[napi]` JS-string ↔ Rust `String` conversion and the `napi::Error` → *thrown* JS error path) — that's covered by the bench's Node runner and by `scripts/test_napi.ts` (`deno task test:napi`), which `process.dlopen`s the built addon and asserts a format, a JSON-AST parse, a thrown error, a multibyte round-trip, and the panic contract (via the `panic_probe` feature; skipped against a probe-less artifact) across the real JS boundary.
