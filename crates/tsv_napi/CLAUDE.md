# tsv_napi

> N-API bindings for `tsv`. Builds to a `cdylib` (`libtsv_napi.{so,dylib,dll}`) loaded by Node.js / Bun as a native addon, via [napi-rs](https://napi.rs).

## Architecture Position

Depends on `tsv_ts`, `tsv_css`, `tsv_svelte`. The **Node/Bun** sibling of the binding trio: [`tsv_ffi`](../tsv_ffi/) is Deno's C-FFI path (`Deno.dlopen`), [`tsv_wasm`](../tsv_wasm/) is the universal WASM path, and `tsv_napi` is the native path for the N-API runtimes. Same engine, same `lang_bindings!` shape, different binding boundary.

This is a **tsv-scoped carve-out** from the ecosystem N-API deferral — **not** an ecosystem-wide flip.

Like `tsv_ffi`, the bindings reuse a **per-thread AST `Bump`** (`with_ast_arena`) that is `reset()` between calls rather than allocated fresh per call — the bindings are invoked once per file in tight loops, and per-call arena malloc/free churns the system allocator's heap high-water in a way that is measurable through a binding layer. The `format` path likewise reuses a **per-thread doc arena** (`with_doc_arena`, the same shape over `DocArena`, calling each language's `format_in`). Both helpers live in the shared [`tsv_arena`](../tsv_arena/) crate (used by all three bindings — `tsv_ffi`, `tsv_napi`, and `tsv_wasm` — so there's one copy of the subtle reuse/soundness contract, not three hand-synced ones). This crate's `format` feature maps to `tsv_arena/format`, which pulls `tsv_lang` for the `DocArena` type; the parse-only build leaves it off and stays lean.

Build/usage commands live in [../../CLAUDE.md §JS Bindings](../../CLAUDE.md#js-bindings).

## Local build & CI

A single-platform local build (`deno task build:napi` → `cargo build -p tsv_napi --profile napi`) drives the **Node and Bun** benchmark runners (`benches/js/lib/napi.ts` loads the built cdylib from `target/napi/` directly via `process.dlopen` — no `.node` rename). CI builds and boundary-tests the addon per OS (the `platforms` job runs `deno task test:napi` + `deno task test:napi:npm` on macOS + Windows; the `artifacts` job runs `deno task test:napi:npm` on Linux — the only PR runner that reaches `platform.js`'s `is_musl()` branch). The cross-platform publish is the tag-triggered matrix workflow (see §The npm packages, **Release**), decoupled from the WASM releases — never bolted onto the single-machine `deno task publish`. The native set is expected to eventually **subsume** the WASM path as tsv's primary native distribution.

## Features

Mirrors `tsv_ffi` / `tsv_wasm`:

- `format` (default) — `format_<lang>` exports
- `parse` (default) — `parse_<lang>` + `parse_internal_<lang>` exports, and the `convert` layer on each language crate
- `panic_probe` (test-only, never default) — the `__panic_probe` export driving the panic-contract test; see §Marshalling & errors

Unlike `tsv_ffi`'s and `tsv_wasm`'s, **no build task produces a single-feature `tsv_napi`** — the addon always ships both halves, and the split exists so the crate mirrors its siblings. `deno task typecheck:features` (in `check`) `cargo check`s each half on its own, so the declared sets can't rot unbuilt; neither `cargo check --workspace` (features unified, defaults ON) nor clippy's `--all-features` union sees a subset.

## Public API

The `lang_bindings!` macro generates four `#[napi]` functions per language (svelte, typescript, css); the `format`/`parse` features gate which are emitted:

- `parse_<lang>(source, goal?) -> string` — JSON AST string (host `JSON.parse`s it — parity with FFI/WASM)
- `parse_<lang>_no_locations(source, goal?) -> string` — the span-only variant (drops per-node `loc`; Svelte also `name_loc`; CSS identical to `parse_css`). See [../tsv_ts/CLAUDE.md](../tsv_ts/CLAUDE.md) §Public API.
- `parse_internal_<lang>(source, goal?) -> void` — parses without converting (benchmark-only; `black_box` prevents elision)
- `format_<lang>(source, goal?) -> string` — formatted source

Every one takes the parse goal as a **trailing optional argument**
(`"script"` / `"module"`; omitted or `undefined` = module) — one export per
(language, operation), with no goalless twin to pick between. **Svelte and CSS
REJECT a set goal** rather than ignoring it: Svelte hard-wires `Module` and CSS
has no goal axis, so a caller passing one asked for something that cannot be
honored and is told — the same stance `tsv_wasm`'s `read_options` takes when it
rejects the `goal` key outright. The goal shapes only the parse the formatter
runs; formatting itself is non-configurable.

JS export names are kept **snake_case** via `#[napi(js_name = "…")]` (napi-rs would otherwise camelCase them) so the addon's names match `tsv_wasm`'s. The per-call SHAPE is where the raw addon diverges from `tsv_wasm`: here the two axes are a flat export (`parse_<lang>_no_locations`) and a positional argument (the goal), matching `tsv_ffi`'s C-style surface, where `tsv_wasm` takes an acorn-style `{locations?, goal?}` options object (see [../tsv_wasm/CLAUDE.md](../tsv_wasm/CLAUDE.md) §Parse Options & Typed Returns). Coverage matches — each binding has its own `lang_bindings!`, but all three read the **same** `parse_ast!` / `goal_allowed!` pair out of [`tsv_arena`](../tsv_arena/), so which languages have a goal axis is one fact in one place. The published `@fuzdev/tsv` loader erases the shape difference too — see §The npm packages.

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
  release workflow runs the script once per matrix target with `--triple` alone —
  every row builds on its own architecture (the gnu/musl rows in pinned containers, the
  arm row on an arm runner, mac and Windows on native runners), so the
  `--artifact`/`--cli-artifact` flags that would name cross-built binaries exist but go
  unused today.

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
JSON-parsed object, `parse_<lang>_json` the wire string. The deliberately
absent exports are the WASM lifecycle trio — `init()`/`init_sync()` (nothing
to initialize), `wasm_module` (no compiled module), and `reinstantiate()` (no
instance to poison; a native overflow is a process-fatal SIGSEGV) — whose
absence is itself the engine signal `cli.js` keys on. Neither
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
so the copy binds to the native loader with no adapter. Its `--jobs` is real
too — path mode fans onto `node:worker_threads` above a file-count threshold
(`NATIVE_WORKER_FILE_THRESHOLD`, its own: the mirror sizes the pool per engine,
and this one has no wasm tier-up competing for cores, so it both crosses over
sooner and scales to the full physical core count where the WASM copy peaks at
half) —
but the workers here load the addon themselves, since this package exports no
compiled `wasm_module` for them to inherit (the WASM package does, and its
workers take it through the `./worker` entry). That path is the only place the
pool runs over the native engine, and `scripts/test_napi_npm.ts` covers it
against a `--jobs 1` run. The dispatcher lives in a
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
`platforms` job on macOS + Windows, the `artifacts` job on Linux).

**Release**: `.github/workflows/release_napi.yml`, triggered by the v\* tag
`scripts/publish.ts` pushes, by `workflow_dispatch` (dry-run by default — the
pre-tag rehearsal; `dry_run=false` is the recovery path for a failed tag run,
dispatched **on the tag** so the tag↔version assertion still applies — it keys
on the ref, and a branch dispatch publishes without it and logs that it did),
and by a weekly cron that builds and gates the whole matrix as a
forced dry-run, so a container/runner breakage surfaces within a week.
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

napi-rs marshals the JS string into a Rust `String` and the returned `String` back out — **no raw pointers, no manual free** (unlike `tsv_ffi`). Engine errors are returned as `napi::Result::Err(napi::Error)`, which napi-rs converts to a **thrown JS error** — there is no status out-param to read (the FFI shape); a throw just propagates.

**Panic contract:** a Rust panic — always a tsv bug — surfaces as a **thrown JS error**, never a host abort. Two halves, both required (unwind alone enables the catch, it does not perform it): every export carries `#[napi(catch_unwind)]`, and the addon builds with the workspace **`napi` profile** (`release` + `panic = "unwind"`; `panic` can't be set per-package, and flipping `[profile.release]` would perturb the size-bounded WASM bundles and the abort-profile FFI/CLI artifacts). Bench, `test:napi`, and publish all use the same profile, so the measured artifact is the shipped one (`target/napi/`). After a caught panic the per-thread arenas stay usable — `tsv_arena`'s take/park protocol leaves the thread-local slot empty while a call runs, so unwind and abort converge on the same state. **Stack overflow is not catchable** and still aborts the host. The contract is proven end to end by `scripts/test_napi.ts` via the test-only `panic_probe` feature's `__panic_probe` export (panics inside `with_ast_arena`; the test asserts a thrown error twice, then correct parse + format output — the arena recovery at the real boundary). Published builds never enable the feature; the test skips when the export is absent.

## Threading & host residency

Three properties a Node/Bun host inherits from this crate, none of them visible from the export signatures:

**Every export is SYNCHRONOUS and runs on the calling thread.** There is no `AsyncTask`, no threadpool hop — a `format_svelte` call on the main thread blocks the event loop for the duration of the format. That is the right default for a formatter (the work is CPU-bound and short, and an async hop costs more than most files take), but it is a contract a host has to plan around: a server formatting large files on its request thread stalls every other connection for that span. The escape hatch is the host's, not the addon's — a `worker_threads` pool, which the next paragraph is about.

**The addon is loaded per JS context, and its arenas are per THREAD.** `with_ast_arena` / `with_doc_arena` (crate [`tsv_arena`](../tsv_arena/)) are thread-locals, so a worker pool gets one arena set per worker and needs no coordination — concurrent calls across workers never share a `Bump`. This is a real contract rather than an implementation note, because a single-threaded test cannot tell "per thread" from "one global": a change that made an arena shared would pass every in-crate test and corrupt output only under concurrency. `scripts/test_napi.ts` pins it directly — four workers `process.dlopen` the addon and loop format/parse/throw, asserting each worker observed exactly ONE distinct result per operation — which also covers addon **context-awareness**, since a non-context-aware addon fails to load in a worker at all.

**⚠️ The recursion depth the addon reaches is the HOST's thread, and the addon cannot raise it.** The parser and printer are recursive descents, and a stack overflow is not a catchable panic — the `catch_unwind` on every export turns a *panic* into a thrown JS error but can do nothing about an overflow, which is a bare `SIGSEGV` that kills the host process with no message at all (Rust's guard-page handler is installed by its runtime startup, which a cdylib loaded into Node never runs). Measured on `const x = ((((…1…))));` at ~1.2 KiB of stack per nesting level (~25% above the CLI's ~0.94, since the `napi` profile carries unwind tables the plain `release` one does not): **~6,960 levels on a host main thread with the common 8 MiB `RLIMIT_STACK`** (it tracks that limit exactly — 16 MiB measures at 13,956 — so it is whatever the machine says), and **~3,460 on a `worker_threads` worker**, which is Node's 4 MiB `stackSizeMb` default and the *lowest* of the native routes — the pool that fixes the event-loop block above lowers this ceiling. For scale, acorn + `@sveltejs/acorn-typescript` give up at 497 levels and prettier at 805, both through V8's checked stack limit, so both routes out-reach them; the difference is that theirs is a catchable `RangeError`. The mitigation is the host's, in the same shape as the arena advice below: raise it where it matters (`new Worker(…, {resourceLimits: {stackSizeMb: 16}})`), or run outsized input on a worker the host then retires. The native CLI in each platform package is unaffected — it states its own reservation (`tsv_cli`'s `cli/stack.rs`) and reaches ~34,900 levels on every route and platform.

**⚠️ The arenas retain their high-water mark for the life of the thread.** `Bump::reset` retains the largest chunk (excess chunks go back to the allocator) and `DocArena::reset` keeps its buffers' capacity outright, so both only rewind — that reuse is the whole point (see §Architecture Position) and it is why the bindings are fast in a per-file loop. The cost is that one pathologically large file permanently sets that thread's floor: a long-lived host that formats a 10 MB file once holds that arena's chunks until the thread exits. There is no trim entry point today, and adding one is deliberate future work rather than an oversight — a `trim`/`shrink` export has to answer what it costs the steady-state loop it would be called from. The available mitigation is host-side and coarse: format outsized inputs on a worker the host then retires.


## Files

- `src/lib.rs` — All bindings: the `lang_bindings!` macro (over the shared `parse_ast!` / `goal_allowed!` goal axis, with `napi_goal` decoding the optional goal string), the three `lang_bindings!` invocations, the `format`-gated `IgnoreStack` class, the `panic_probe` export, and a `#[cfg(test)]` module. The reusable arenas and the goal macros are imported from `tsv_arena` (`with_ast_arena`, plus `with_doc_arena` under the `format` feature)
- `npm/` — the `@fuzdev/tsv` loader package source (`index.js` + `index.d.ts` — hand-written, mirroring the wasm packages' surface minus `init`/`init_sync`/`wasm_module`/`reinstantiate`, and bound by the same `.js`-extension rule on relative specifiers ([../tsv_wasm/CLAUDE.md](../tsv_wasm/CLAUDE.md) §The Span-Only Wire), asserted by `scripts/test_napi_npm.ts` — + `platform.js` (triple detection) + `bin.js` (the `tsv` bin dispatcher) + `README.md`); staged with generated package.jsons by `scripts/build_napi_packages.ts`, which also copies in the shared `locations.js` helper and `cli.js` fallback (see §The npm packages)
- `build.rs` — `napi_build::setup()` (linker config for the addon)
- `Cargo.toml` — `crate-type = ["cdylib"]`; `unsafe_code = "deny"`, not `allow` — `#[napi]`'s generated items carry their own `#[allow(unsafe_code)]` (an inner `allow` overrides `deny`), so the macro output compiles while any hand-written `unsafe` stays a compile error; deps `napi` + `napi-derive` (3.x) + `tsv_arena`, plus the `format`-optional `tsv_ignore` + `tsv_discover` + `tsv_lang` (`normalize_carriage_returns`) behind `IgnoreStack`, build-dep `napi-build` (2.x). `format` → `tsv_arena/format` + those two

The in-crate test module drives the binding in-process — all three languages × `parse` / `parse_internal` / `format`, plus the goal axis and its two refusals (an invalid goal string; a goal handed to a goalless language) — so `cargo test` exercises the native binding without a Node host (the Deno/WASM smoke paths don't cover napi). The per-language `parse` assertions check the language's own JSON root type (`Program` / `StyleSheetFile` / `Root`), which also guards the `lang_bindings!` wiring against a transposed invocation; the error tests cover the thrown-`napi::Error` arm for both parse and format; one test exercises this crate's distinctive risk — that the per-thread `with_ast_arena` / `with_doc_arena` `reset()` cleanly between back-to-back calls; and a multibyte round-trip guards the char-offset boundary. What `cargo test` can **not** reach is the napi-rs **marshalling** layer (the `#[napi]` JS-string ↔ Rust `String` conversion and the `napi::Error` → *thrown* JS error path) — that's covered by the bench's Node runner and by `scripts/test_napi.ts` (`deno task test:napi`), which `process.dlopen`s the built addon and asserts a format, a JSON-AST parse, a thrown error, a multibyte round-trip, the panic contract (via the `panic_probe` feature; skipped against a probe-less artifact), and the worker-thread threading contract (§Threading & host residency) across the real JS boundary.
