# tsv_napi

> N-API bindings for `tsv`. Builds to a `cdylib` (`libtsv_napi.{so,dylib,dll}`) loaded by Node.js / Bun as a native addon, via [napi-rs](https://napi.rs).

## Architecture Position

Depends on `tsv_ts`, `tsv_css`, `tsv_svelte`. The **Node/Bun** sibling of the binding trio: [`tsv_ffi`](../tsv_ffi/) is Deno's C-FFI path (`Deno.dlopen`), [`tsv_wasm`](../tsv_wasm/) is the universal WASM path, and `tsv_napi` is the native path for the N-API runtimes. Same engine, same `lang_bindings!` shape, different binding boundary.

This is a **tsv-scoped carve-out** from the ecosystem N-API deferral — **not** an ecosystem-wide flip.

Like `tsv_ffi`, the bindings reuse a **per-thread AST `Bump`** (`with_ast_arena`) that is `reset()` between calls rather than allocated fresh per call — the bindings are invoked once per file in tight loops, and per-call arena malloc/free churns the system allocator's heap high-water in a way that is measurable through a binding layer. The `format` path likewise reuses a **per-thread doc arena** (`with_doc_arena`, the same shape over `DocArena`, calling each language's `format_in`). Both helpers live in the shared [`tsv_arena`](../tsv_arena/) crate (used by all three bindings — `tsv_ffi`, `tsv_napi`, and `tsv_wasm` — so there's one copy of the subtle reuse/soundness contract, not three hand-synced ones). This crate's `format` feature maps to `tsv_arena/format`, which pulls `tsv_lang` for the `DocArena` type; the parse-only build leaves it off and stays lean.

Build/usage commands live in [../../CLAUDE.md §JS Bindings](../../CLAUDE.md#js-bindings).

## Local build & CI

A single-platform local build (`deno task build:napi` → `cargo build -p tsv_napi --profile napi`) drives the **Node** benchmark runner (`benches/js/lib/napi.ts` loads the built cdylib from `target/napi/` directly via `process.dlopen` — no `.node` rename). CI builds and boundary-tests the addon per OS (the `platforms` job runs `deno task test:napi` on macOS + Windows). The cross-platform publish is the tag-triggered matrix workflow (see §The npm packages, **Release**), decoupled from the WASM releases — never bolted onto the single-machine `deno task publish`. The native set is expected to eventually **subsume** the WASM path as tsv's primary native distribution.

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

Plus, outside the macro, four goal-aware TypeScript exports taking a trailing
goal string (`"script"` / `"module"`): `parse_typescript_with_goal`,
`parse_typescript_no_locations_with_goal`, `parse_internal_typescript_with_goal`,
and `format_typescript_with_goal` (the flat counterpart of `tsv_wasm`'s
`format_typescript(source, {goal})` — the goal shapes only the parse the
formatter runs).

JS export names are kept **snake_case** via `#[napi(js_name = "…")]` (napi-rs would otherwise camelCase them) so the addon's names match `tsv_wasm`'s. The per-call SHAPE is where the raw addon diverges from `tsv_wasm`: here the axes are flat exports (`parse_<lang>_no_locations`, the `*_with_goal` variants), matching `tsv_ffi`'s C-style surface, where `tsv_wasm` takes an acorn-style `{locations?, goal?}` options object (see [../tsv_wasm/CLAUDE.md](../tsv_wasm/CLAUDE.md) §Parse Options & Typed Returns). Coverage matches. The published `@fuzdev/tsv` loader erases the shape difference too — see §The npm packages.

Outside the macro, the `format` feature also exports **`IgnoreStack`** — a `#[napi]` class over `tsv_ignore::IgnoreStack` plus the `tsv_discover` verdicts, method for method with `tsv_wasm`'s `#[wasm_bindgen]` twin (see [../tsv_wasm/CLAUDE.md](../tsv_wasm/CLAUDE.md) §Discovery Matcher + Policy). ⚠️ Its three maybe-a-warning methods return **`Either<String, Undefined>`, not `Option<String>`**: napi-rs maps `None` to JS `null` where wasm-bindgen maps it to `undefined`, and a package that exists to be swapped for the other must not change which one a caller sees. `Undefined` is napi-rs's `()`, so the none arm allocates nothing. Two tests pin it — the in-crate one on the `Either::B` variant, `scripts/test_napi_npm.ts` on the JS value, since only the latter can observe it.

## The npm packages: `@fuzdev/tsv` + platform packages

Staged by `deno task build:napi:packages` (`scripts/build_napi_packages.ts`)
into `crates/tsv_napi/pkg/` (gitignored):

- **`pkg/napi/` — `@fuzdev/tsv`**, the loader: `npm/index.js` +
  `npm/index.d.ts` + `npm/platform.js` (triple detection, shared by the next
  two) + `npm/bin.js` (the `tsv` bin — a dispatcher, see below) +
  `npm/README.md` + a copy of `tsv_wasm`'s `tsv_ast.d.ts`,
  `locations.js`/`.d.ts`, and `cli.js` (the JS CLI mirror, `bin.js`'s
  fallback) + a generated package.json pinning the platform packages as
  **exact-version `optionalDependencies`**. The staging directory is named
  for the binding (`napi`), not for the package — the published name is the
  bare `@fuzdev/tsv`, tsv's native distribution.
- **`pkg/<triple>/` — `@fuzdev/tsv-<triple>`**, one platform package per
  invocation: the built cdylib copied to `tsv_napi.node` (byte-identical
  rename, named for the crate it came from), the real `tsv_cli` binary copied
  to `tsv`/`tsv.exe` beside it (plain `release` profile — what `bin.js`
  execs), + a generated package.json whose `os`/`cpu`/`libc` fields drive
  install-time selection. Naming is the ecosystem-universal
  `<loader>-<dash triple>` shape (swc's). The set: `linux-x64-gnu`,
  `linux-arm64-gnu`, `linux-x64-musl`, `darwin-arm64`, `win32-x64`. One per
  invocation by design — a machine can only have built its own triple; the
  release workflow runs the script once per matrix target (`--triple` +
  `--artifact`/`--cli-artifact` name cross-built binaries).

**The loader is ESM**, the same module system as the wasm packages — one
dialect across tsv's whole npm surface, which is what lets shared sources
(`locations.js`, `cli.js`) load unchanged in either package. `.node` binaries
have no ESM loader, so the platform addon rides a
`createRequire(import.meta.url)` shim (oxc-parser's shape); that is the only
CommonJS left. A CommonJS host reaches the package by dynamic `import()` — the
path every supported Node allows, and the one `test_napi_npm.ts` gates. It
detects the platform triple (musl via a `/lib/ld-musl-*` probe first, then
`process.report`'s `glibcVersionRuntime` — trusted only positively — to rule
out a glibc system that merely has musl installed), requires
`@fuzdev/tsv-<triple>`, and on failure throws an error
naming the triple, the prebuilt set, and `@fuzdev/tsv_wasm` as the universal
fallback.

**`@fuzdev/tsv` is the full native distribution; `@fuzdev/tsv_wasm` is the
fallback** — so parity runs the whole way, not just the engine calls. Same
export names, same `(source, options?)` bags, same error strings: the loader's
`read_options` mirrors the wasm crate's key for key, and
`scripts/test_napi_npm.ts` asserts the strings. `parse_<lang>` returns the
JSON-parsed object, `parse_<lang>_json` the wire string. `init()` is the one
export that is deliberately absent — there is nothing to initialize. Neither
package exports the bench-only `parse_internal_*` family
(`scripts/patch_npm_package.ts` filters it out of the wasm wrappers too).

The locations helpers (`reconstruct_locations` / `create_locator` / `loc_of`)
ship here too: `tsv_wasm/npm/locations.js` is pure JS over the span-only wire,
so the staging script copies that same file in and appends the re-export to the
staged entry — the export names are extracted from the helper, never listed a
second time. The `IgnoreStack` discovery class ships too — a `#[napi]` twin of
`tsv_wasm`'s wrapper over the same `tsv_ignore` / `tsv_discover` pair, re-exported
straight off the addon since it takes no options bag.

**The `tsv` bin here IS the native CLI** (the esbuild/biome npm shape): the
loader's bin is `npm/bin.js`, a dispatcher that resolves the platform
package's `tsv`/`tsv.exe` — the real `tsv_cli` binary shipped beside the
addon — and execs it, forwarding argv, stdio, exit codes, and signals
verbatim. So `npx tsv format src` on this package gets the native CLI's exact
contract: real `--jobs` parallelism, parallel discovery, native error paths.
The dispatch never loads the addon (it resolves the package and probes the
file, ~1 ms on top of Node's ~20 ms startup) and never reads PATH — only this
package's own optionalDependency. When no binary is reachable, `bin.js`
defers to `cli.js` — `tsv_wasm/npm/cli.js`, the shared JS mirror of the same
contract, copied in at stage time; it imports its engine from `./index.js`,
so the copy binds to the native loader with no adapter (and remains
single-threaded, `--jobs` accepted-and-ignored — in `@fuzdev/tsv_wasm`, where
it is the bin itself, that caveat still holds). The dispatcher lives in a
napi-only file rather than as a branch inside the shared `cli.js` so the wasm
copy stays byte-identical with no dead dispatch code — and so the wasm CLI
can never resolve a sibling-installed native platform package by accident.

Tests: `deno task test:napi:npm` stages a temp `node_modules` and drives the
packaged shape under Node — loader resolution, ESM and CommonJS hosts, the options
surface with exact error strings, package.json coherence (pins, selection
fields, `files` on both packages, the executable bit on the CLI binary, the
loader-`SUPPORTED`-vs-optionalDependencies agreement), the
unsupported-platform error, and the `tsv` bin: that `bin.js` really
dispatches to the binary (argh's help output — a discriminator the JS mirror
never prints), that it forwards exit codes, stdout/stderr, and stdin, that
`--version` through the bin matches the staged package version (a
binary↔package lockstep gate — a stale binary staged into a fresh package
fails it), and that `npm pack` would ship the binary in the tarball — the
package.json check covers the `files` declaration and the file on disk, this
covers what npm actually packs — executable where a mode exists (npm packs
the on-disk mode). Then the degraded paths: binary removed → `cli.js`,
present-but-unrunnable → warn + `cli.js`, child killed by a signal →
re-raised. The last two are posix-only by nature, not by omission: the
fallback branch keys on any spawn error, so a Windows staging would re-enter
an already-proven branch, and signal death has no Windows analogue.

Both CLIs exist here and only here, so this is also where their **flag sets**
are held together: each side is handed every flag the other advertises and
must not answer with its unknown-flag error. Recognition, not behavior —
semantics stay with `scripts/test_npm.ts` and `tests/cli_tests.rs` — which is
what catches the hand-written mirror going stale against argh. The shared
`tests/discovery/scenarios.json` parity table runs through **both** bin
entries: `bin.js` (native discovery via the shim — the real `npx tsv` path)
and `cli.js` directly (the fallback JS loop over the native `IgnoreStack`,
the table's standing third consumer beside `scripts/test_npm.ts`'s wasm CLI
run and the native `tests/discovery_parity.rs`). Runs per OS in CI (the
`platforms` job).

**Release**: `.github/workflows/release_napi.yml`, triggered by the v\* tag
`scripts/publish.ts` pushes (or `workflow_dispatch` as a dry-run rehearsal).
Per target: container-pinned builds of **both** shipped binaries — the addon
(`napi` profile) and the `tsv_cli` binary (plain `release`: abort + LTO, the
same artifact the hyperfine benches measure; a standalone process owns its
own crash, so the addon's unwind rationale doesn't apply) — gnu rows in
almalinux:8 → glibc 2.28 floor, measured by the workflow's floor gate over
both artifacts; musl in rust:alpine with `-crt-static` off, both gated
GLIBC-free. Then per-artifact size bounds
(`scripts/validate_napi_artifact.ts`, one anchored band per binary) and the
npm-shape test over the real artifacts (node:alpine for musl). The publish
job gathers all five, stages the loader (`--loader-only`), and runs
`scripts/publish_napi.ts` — completeness (addon + CLI binary per platform)
and version-lockstep checks, re-arming the CLI binaries' executable bit
(artifact transport drops file modes — without it every posix `npx tsv`
would EACCES), platforms-then-loader order, idempotent skip-if-published.
See the root [CLAUDE.md §Publishing](../../CLAUDE.md#publishing).

## Marshalling & errors

napi-rs marshals the JS string into a Rust `String` and the returned `String` back out — **no raw pointers, no manual free** (unlike `tsv_ffi`). Engine errors are returned as `napi::Result::Err(napi::Error)`, which napi-rs converts to a **thrown JS error** — there is no `{"error": …}` envelope to inspect (the FFI shape); a throw just propagates.

**Panic contract:** a Rust panic — always a tsv bug — surfaces as a **thrown JS error**, never a host abort. Two halves, both required (unwind alone enables the catch, it does not perform it): every export carries `#[napi(catch_unwind)]`, and the addon builds with the workspace **`napi` profile** (`release` + `panic = "unwind"`; `panic` can't be set per-package, and flipping `[profile.release]` would perturb the size-bounded WASM bundles and the abort-profile FFI/CLI artifacts). Bench, `test:napi`, and publish all use the same profile, so the measured artifact is the shipped one (`target/napi/`). After a caught panic the per-thread arenas stay usable — `tsv_arena`'s take/park protocol leaves the thread-local slot empty while a call runs, so unwind and abort converge on the same state. **Stack overflow is not catchable** and still aborts the host. The contract is proven end to end by `scripts/test_napi.ts` via the test-only `panic_probe` feature's `__panic_probe` export (panics inside `with_ast_arena`; the test asserts a thrown error twice, then correct parse + format output — the arena recovery at the real boundary). Published builds never enable the feature; the test skips when the export is absent.

## Files

- `src/lib.rs` — All bindings: the `lang_bindings!` macro, the three `lang_bindings!` invocations, the flat goal-aware TS exports, the `format`-gated `IgnoreStack` class, the `panic_probe` export, and a `#[cfg(test)]` module. The reusable arenas are imported from `tsv_arena` (`with_ast_arena`, plus `with_doc_arena` under the `format` feature)
- `npm/` — the `@fuzdev/tsv` loader package source (`index.js` + `index.d.ts` + `platform.js` (triple detection) + `bin.js` (the `tsv` bin dispatcher) + `README.md`); staged with generated package.jsons by `scripts/build_napi_packages.ts`, which also copies in the shared `locations.js` helper and `cli.js` fallback (see §The npm packages)
- `build.rs` — `napi_build::setup()` (linker config for the addon)
- `Cargo.toml` — `crate-type = ["cdylib"]`; `unsafe_code = "allow"` (N-API generates unsafe code); deps `napi` + `napi-derive` (3.x) + `tsv_arena`, plus the `format`-optional `tsv_ignore` + `tsv_discover` behind `IgnoreStack`, build-dep `napi-build` (2.x). `format` → `tsv_arena/format` + those two

The in-crate test module drives **every entry point** in-process — all three languages × `parse` / `parse_internal` / `format` — so `cargo test` exercises the native binding without a Node host (the Deno/WASM smoke paths don't cover napi). The per-language `parse` assertions check the language's own JSON root type (`Program` / `StyleSheetFile` / `Root`), which also guards the `lang_bindings!` wiring against a transposed invocation; the error tests cover the thrown-`napi::Error` arm for both parse and format; one test exercises this crate's distinctive risk — that the per-thread `with_ast_arena` / `with_doc_arena` `reset()` cleanly between back-to-back calls; and a multibyte round-trip guards the char-offset boundary. What `cargo test` can **not** reach is the napi-rs **marshalling** layer (the `#[napi]` JS-string ↔ Rust `String` conversion and the `napi::Error` → *thrown* JS error path) — that's covered by the bench's Node runner and by `scripts/test_napi.ts` (`deno task test:napi`), which `process.dlopen`s the built addon and asserts a format, a JSON-AST parse, a thrown error, a multibyte round-trip, and the panic contract (via the `panic_probe` feature; skipped against a probe-less artifact) across the real JS boundary.
