# tsv

> precise language tools for TypeScript/JS, CSS, and Svelte in Rust

High-performance Rust parser as a drop-in replacement for Svelte's modern parser (acorn + acorn-typescript), paired with a formatter that took Prettier as its initial guide and still tracks it for the common case — while making deliberate, cataloged choices to diverge where tsv's own judgment is more defensible.

**Non-configurable by design**: formatting options are fixed at Prettier's defaults except printWidth=100, useTabs=true, singleQuote=true, and trailingComma='none' — no config files, CLI flags, or runtime options, ever (opinionated like `gofmt` and Black). The one carve-out is file *scope*, not style: `tsv format` honors `.gitignore` (hierarchically, in a git tree) plus hierarchical `.formatignore` / `.prettierignore`. See [Configuration](#configuration).

## Committing

`git add` and `git commit` are denied by `.claude/settings.local.json` in
this repo — make the edits and stop, the user commits.

**Do not edit `CHANGELOG.md`.** Like release version bumps, the changelog is
the user's responsibility — agents make the source/doc/fixture edits and leave
`CHANGELOG.md` alone (including the `## Unreleased` section and its
`<!-- bump: … -->` marker). The user stamps it at release time.

## Priorities

1. **Correctness**: Match Svelte's parser exactly — it's a drop-in replacement. The formatter began with Prettier as its guide and tracks it for the common case, but tsv has its own identity and makes deliberate, cataloged choices to diverge where they're more defensible (spec, print width, comment position, its own taste). tsv also fixes numerous Prettier bugs. Fixtures are the source of truth for correct behavior — when tests fail, fix the code; when tsv diverges on purpose, the fixture records it.
2. **Performance**: Pure Rust for speed. Dev tools use an embedded Deno sidecar that minimizes process overhead.

## Development Philosophy: Test-Driven Development with Fixtures

**ALWAYS use TDD when implementing features or fixing bugs:**

0. **Load context FIRST** - Read BOTH ./docs/fixture_workflow.md AND ./docs/fixture_naming.md into context.
   For ANY `_prettier_divergence` fixture, ALSO read ./docs/conformance_prettier.md first — the divergence
   must be sanctioned and **cataloged in the relevant section** there (for a comment divergence that's
   §Comment Position Philosophy + §Comment relocation catalog; for others it's the matching feature
   section, e.g. §Svelte: Blocks), AND the fixture's `README.md` MUST link back to that section
   (`See [conformance_prettier.md §…](…)`) — the README and the catalog entry must agree. This applies to
   every divergence, not just comment ones. Study 2-3 existing fixtures in the target category (match their
   README shape). No code changes without a failing fixture.
1. **Create the fixture FIRST** - Use `fixture_init` to create `input.svelte` (prettier-formatted) and `expected.json` in one step.
   Use `.svelte` unless the feature is file-level (byte 0: hashbang, BOM). See ./docs/fixture_workflow.md#11-create-directory-and-draft.
2. **Review the input** - Read the generated `input.svelte` to verify structure (formatting is guaranteed correct).
3. **See it fail** - Run `deno task fixtures:validate <pattern>` to show the failing diff
4. **⚠️ APPROVAL GATE — STOP HERE.** Show the failing diff to the user and wait for explicit
   confirmation ("lgtm", "proceed", or feedback) before writing any implementation code.
   Do not proceed to step 5 without this confirmation. **If the user gives feedback that
   requires reworking the fixture (naming, structure, cases), redo steps 1-3 and return
   here — the gate resets on every rework.**
5. **Implement the fix** - Write code to make the test pass
6. **Validate** - Run `deno task fixtures:validate <pattern>` to confirm it passes

**For `long` fixtures**: include BOTH a 100-char case (stays inline) and a 101-char case (breaks); test the exact 100/101 boundary and simplify content to the minimum that triggers it. Iterate `fixture_init --force` until widths are exact — read the widths from its output, never estimate manually.

**Never write code before creating the fixture.** The fixture defines what "correct" means.

**Failing fixtures are expected.** Never delete a fixture to make tests pass — a failing fixture is a known bug waiting to be fixed.

## Values

- **Spec-first**: Read specs and canonical implementations before implementing. Experiment to verify, not to design.
- **Refactor early**: Fix outdated patterns immediately. Leave no legacy.
- **One sprint at a time**: Implement incrementally, keep tests passing.
- **No backwards compatibility**: Pre-stable — delete old code, migrate fully, don't shim. No new deps without explicit approval.

## Quick Start - Common Workflows

**Fast iteration during development:**

```bash
cargo check --workspace                # Fast syntax check (no codegen, ~instant on incremental)
deno task fixtures:validate <pattern>  # Validate specific fixtures (preferred for fixture work)
deno task dev                          # Watch mode - auto check + test on file changes (requires cargo-watch)
```

**After making changes:**

```bash
# For fixture work - validate specific fixtures first
deno task fixtures:validate <pattern>    # Fast, targeted fixture validation

# For full validation before committing
cargo test --workspace                   # Run ALL tests (~5-10s, includes all fixtures)
deno task check                          # Full committed-tree gate: fmt, audits, typecheck, tests, clippy (see benches/js/CLAUDE.md §Gate map)
```

**Prefer `fixtures:validate` over `cargo test --workspace`** when working on fixtures - it's faster, shows detailed diffs, and filters by pattern. Use `cargo test --workspace` for full test suite before committing.

**When to use `fixtures:update` commands:**

- After creating a new fixture (generates initial `expected.json`)
- When upstream sources change (Svelte parser version, prettier version)
- Not to "fix" failing tests - fix the code instead

```bash
# Only when appropriate (see above)
deno task fixtures:update:parsed     # Regenerate expected.json from Svelte/acorn parsers
deno task fixtures:update:formatted  # Regenerate output_prettier.svelte from prettier
deno task fixtures:update            # Both of the above
```

**Debugging a specific issue:**

```bash
cargo run -p tsv_debug compare tests/fixtures/path/input.svelte  # diff with prettier
cargo run -p tsv_debug ast_diff tests/fixtures/path/input.svelte # verify AST equivalence
```

See [Debug Tooling](#debug-tooling).

## Commands

### Build & Development

```bash
# Deno tasks (recommended)
deno task build            # workspace dev build
deno task build:release    # workspace optimized build
deno task build:all        # release + ffi + build:packages (everything)
deno task build:packages   # the 6 publishable WASM bundles (npm + deno) — single source of truth shared by CI + publish.ts
deno task build:bench      # the artifact set `bench`/`smoke` measure (ffi×3 + the 3 wasm:deno variants)
deno task build:ffi        # C FFI library (:format / :parse size-only variants; :all builds all three)
deno task build:wasm:deno  # deno-target WASM bundle (requires wasm-pack; :parse:deno / :all:deno for the other variants)
deno task clean            # clean build artifacts
deno task dev              # watch mode: check + test on changes (requires cargo-watch)

# Cargo directly
cargo build --workspace [--release]  # workspace build
cargo check --workspace              # fast syntax check (no codegen)
cargo build -p tsv_cli               # CLI only
cargo build -p tsv_debug             # debug tools only

cargo install cargo-watch  # optional, for `deno task dev`
```

### CLI Usage - Parse & Format

Parser auto-detected from extension (`.ts`/`.svelte`/`.css`). `--content` and `--stdin` modes require `--parser svelte|typescript|css`.

`format` writes paths **in place** (only when output differs) and prints changed paths to stdout; `--content`/`--stdin` print formatted source to stdout. Directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`, all parsed as TypeScript — JSX/TSX is out of scope), `.svelte`, and `.css` with gitignore-aware, reproducible discovery (see [Configuration](#configuration); full rules in ./docs/cli.md §Multi-File Formatting); an explicitly named file argument bypasses the ignore files. `--list` prints the discovered in-scope files without formatting (path mode only; an empty scope still exits 0). Files format in parallel (`--jobs N` overrides the thread count; path mode only). Exit codes: 0 clean, 1 would-change (`--check`, which also works with `--content`/`--stdin`), 2 errors; missing path args fail the run upfront, while per-file and traversal errors report and continue.

```bash
cargo run -p tsv_cli parse file.ts                                       # compact JSON
cargo run -p tsv_cli parse file.ts --pretty                              # formatted JSON
cargo run -p tsv_cli parse file.ts --no-locations                        # span-only wire (no per-node loc; ~46% smaller)
cargo run -p tsv_cli parse --content '<div>x</div>' --parser svelte      # parse string (preferred for agents)
cargo run -p tsv_cli parse --stdin --parser svelte                       # parse stdin (not preferred for agents)
cargo run -p tsv_cli format file.svelte src/lib                          # format files/dirs in place
cargo run -p tsv_cli format --check src/lib                              # list would-change files, exit 1 (CI)
cargo run -p tsv_cli format --list src/lib                               # list in-scope files (no formatting)
cargo run -p tsv_cli format --content '<div>x</div>' --parser svelte     # format string to stdout
```

### Testing & Code Quality

```bash
deno task check          # full committed-tree gate: fmt, audits, typecheck, tests, clippy (benches/js/CLAUDE.md §Gate map)
deno task doctor         # one-pass setup check: runtimes, canonical pins + checkout alignment, node_modules freshness, oracle checkouts, corpus entries, build artifacts. Exit 1 only on MISLEADING state (pin drift, skew, stale deps); absences are warnings (--strict promotes warnings to failures)
deno task typecheck      # cargo check
deno task test           # cargo test
deno task lint           # cargo clippy
cargo fmt                # format Rust code — the only autoformatter in this repo
# Non-Rust files (TS/MD/JSON) are hand-maintained: tsv ships NO prettier or deno
# fmt config. Never run `deno fmt` or `prettier` on the repo — with no config they
# reformat to their own defaults (spaces, double quotes) and churn every file. The
# fixture/corpus prettier oracles pass options inline, so they're unaffected.

cargo test --workspace test_typescript_parser_literal  # run specific test by name
cargo test --workspace --test fixtures_tests           # fixture validation tests
cargo test --workspace --test cli_tests                # CLI integration tests
```

### Fixtures (Rust + Deno-based)

All `fixtures:*` tasks accept positional patterns (multiple = OR), `--list`, and (where applicable) `--prettier-only`.

```bash
deno task fixtures:list              # list all fixtures (read-only)
deno task fixtures:init <dir>        # create/reinit a fixture (alias of `tsv_debug fixture_init`; --content/--stdin/--force)
deno task fixtures:validate          # validate (use during fixture work; --prettier-only skips our parser/formatter)
deno task fixtures:update            # regenerate expected.json + output_prettier.svelte (source of truth)
deno task fixtures:update:parsed     # regenerate expected.json only (run when parser changes)
deno task fixtures:update:formatted  # regenerate output_prettier.svelte only
deno task fixtures:audit             # audit _prettier_divergence fixtures (diagnostic; --all for every fixture)
deno task fixtures:ts-audit          # which input.ts fixtures genuinely need .ts vs could be .svelte (alias of `ts_fixture_audit`)
deno task conformance:audit          # doc/fixture integrity: divergences cataloged + every docs/*.md + README link resolves + divergence READMEs back-link their sanctioning doc + no stray READMEs (gated in `deno task check`; ./docs/audits.md)
deno task pins:audit                 # canonical-oracle version sync (gated in `deno task check`): (1) pin agreement — sidecar.ts VERSIONS + npm: imports, benches/js/package.json, actor.rs acorn import-map must be identical; (2) checkout alignment — a PRESENT ../svelte or ../acorn-typescript checkout must match its pin (absent → skipped, so clean machines pass)
deno task scan:audit                 # no new raw find/rfind/match_indices substring scans over source (gated in `deno task check`; ./docs/audits.md)
deno task fanout:audit               # no super-linear doc-node fanout — the per-layout-candidate rebuild blowup (gated in `deno task check`; ./docs/audits.md)
deno task roundtrip:audit            # format(tests/fixtures) must reparse — pure-Rust tripwire, real yield on external corpora (gated in `deno task check`; ./docs/audits.md)
deno task binding:audit              # comment↔token re-binding audit — HARD fails the gate, SOFT informational (gated in `deno task check`; ./docs/audits.md)
deno task authoring:audit            # authoring-independence over Svelte boundary whitespace — one fixed point per document, exit 1 on any non-idempotency (gated in `deno task check`; ./docs/audits.md)
deno task fuzz:audit                 # seeded mutational fuzzer (fixed --seed 0 --iterations 5000): no-panic + idempotency + structural-reparse (gated in `deno task check`; ./docs/audits.md)
deno task swallow:audit              # `//` line-comment swallow check — a line comment swallowing following output-line content (gated in `deno task check`; ./docs/audits.md)
deno task comments:audit             # print-once comment ledger: DROPPED / DOUBLE-PRINTED comments (gated in `deno task check`; ./docs/audits.md)
deno task gaps:audit                 # gap-injection audit — RATCHET over `gap_audit_known.txt` (every line a known bug), ~17 s (gated in `deno task check`; ./docs/gap_audit.md)
deno task gaps:audit:update          # regenerate that snapshot after fixing a shape (or when a new fixture merely REACHES a pre-existing one); refuses a narrowed run
deno task gaps:audit:rank            # rank the pinned shapes for triage (also --since; see ./docs/gap_audit.md)
deno task blanks:audit               # blank-line injection audit — RATCHET over `blank_audit_known.txt`, ~24 s (gated in `deno task check`; ./docs/blank_audit.md)
deno task blanks:audit:update        # regenerate that snapshot after fixing a shape; refuses a narrowed run
deno task render:audit <paths>       # render-equivalence over REAL Svelte code (needs the sidecar — NOT in `deno task check`; release-gated as a leg of `deno task conformance`; ./docs/audits.md)
deno task idempotency:sweep          # F1 (idempotency) sweep over the real-code corpus (minutes, machine-dependent — NOT in `deno task check`; conformance cadence; ./docs/audits.md)
deno task audit:corpus               # the standing content-loss/robustness bundle over REAL code (publish Step 3c; NOT in `deno task check`; ./docs/audits.md)
```

For direct `cargo run -p tsv_debug` usage, see [Debug Tooling](#debug-tooling).

**Creating new fixtures** (`fixture_init` formats through prettier + generates `expected.json`):

```bash
cargo run -p tsv_debug fixture_init tests/fixtures/path --content '<script>your code</script>'
echo '<script>code</script>' | cargo run -p tsv_debug fixture_init tests/fixtures/path --stdin
cargo run -p tsv_debug fixture_init tests/fixtures/path  # reformat existing input file
```

See ./docs/fixture_workflow.md. Use `--prettier-only` with `fixtures:validate` during fixture design.

### JS Bindings

Three binding crates for different use cases:

- `tsv_ffi` (C ABI) — target: Any FFI (Deno, Python, etc.); output: `libtsv_ffi.so` / `.dylib` / `.dll`
- `tsv_wasm` (wasm-bindgen) — target: Browser, Deno, Node; output: `.wasm` module (format / parse / all variants via cargo features)
- `tsv_napi` (napi-rs) — target: Node.js / Bun native addon; output: `libtsv_napi.{so,dylib,dll}` (loaded via `process.dlopen`). Currently a **measurement-only** binding for the Node benchmark runner (single-platform local build: `deno task build:napi`; boundary tests: `deno task test:napi`); the cross-platform publish matrix as `@fuzdev/tsv_napi` is a fast-follow after 0.2 — it needs GitHub release infrastructure, so it doesn't block WASM/VS Code publishing (may land as 0.3), and is expected to eventually subsume the WASM native path. See ./crates/tsv_napi/CLAUDE.md.

`tsv_wasm` produces three npm packages from one crate via the `format` + `parse` cargo features (default = both): `@fuzdev/tsv_format_wasm` (format only), `@fuzdev/tsv_parse_wasm` (parse only), and `@fuzdev/tsv_wasm` (everything + the `tsv` CLI). Each variant has its own output directory.

```bash
# Build bindings
deno task build:ffi                  # C FFI, full build → target/release/libtsv_ffi.so
deno task build:ffi:format           # C FFI, format-only (size only) → target/ffi-format/release/
deno task build:ffi:parse            # C FFI, parse-only (size only) → target/ffi-parse/release/
deno task build:wasm:deno            # deno WASM, format-only → pkg/format/deno/
deno task build:wasm:parse:deno      # deno WASM, parse-only → pkg/parse/deno/
deno task build:wasm:all:deno        # deno WASM, full build (benches/sidecar) → pkg/all/deno/
deno task build:npm:format           # publishable npm package → pkg/format/npm/
deno task build:npm:parse            # publishable npm package → pkg/parse/npm/
deno task build:npm:all              # publishable npm package + tsv bin → pkg/all/npm/

# Or via cargo/wasm-pack directly
cargo build -p tsv_ffi --release
wasm-pack build crates/tsv_wasm --target deno --release --out-dir pkg/all/deno
wasm-pack build crates/tsv_wasm --target deno --release --out-dir pkg/parse/deno -- --no-default-features --features parse
```

### Publishing

npm-only, three packages from one WASM crate:

- `@fuzdev/tsv_format_wasm` — format only (`--no-default-features --features format`)
- `@fuzdev/tsv_parse_wasm` — parse only (`--no-default-features --features parse`); bundles hand-maintained `tsv_ast.d.ts` from `crates/tsv_wasm/types/` and the pure-JS `no-locations` line/column reconstruction helper (`crates/tsv_wasm/npm/locations.js` + `.d.ts`)
- `@fuzdev/tsv_wasm` — full tool (default build, both features); bundles `tsv_ast.d.ts` + the `locations.js` reconstruction helper and ships the `tsv` bin (`crates/tsv_wasm/npm/cli.js` — `format` + `parse` subcommands mirroring `tsv_cli`'s flags/exit codes, `node:util` `parseArgs`, zero deps, single-threaded)

A separate types-only `@fuzdev/tsv_ast` package is deferred — `import type` from `tsv_parse_wasm` is zero-runtime-cost, and no 0.1 consumer profile needs the standalone split. Reconsider if/when a real consumer appears. `@fuzdev/tsv` (bare) stays reserved for a future native-binary flagship.

Version source of truth: `Cargo.toml` `[workspace.package] version` (read directly by `wasm-pack`). No root package.json, no changesets. All published packages move together at this version.

Package shape: built from the wasm-pack `web` target, then `scripts/patch_npm_package.ts` adds a Node/Bun entry (`index.js`, sync auto-init), a browser entry (`browser.js`, guarded `await init()`), `index.d.ts`, conditional `exports`, npm metadata, and the variant README. The export list is extracted from the generated JS, so new `lang_bindings!` languages flow through automatically.

`scripts/publish.ts` orchestrates the release end to end (preflight → bump → check → conformance:all → build npm packages + deno bundles → verify → artifact validation: size bounds + Deno smoke + Node tests → idempotent npm publish → git commit + tag + push), printing a wasm size summary (raw + gzipped) at the end. It stamps CHANGELOG.md's `## Unreleased` section into the released version's section — that section must be non-empty and carry a `<!-- bump: <level> -->` marker that matches `--bump` (required in **both** places, must agree; on stamp a fresh empty `## Unreleased` at `bump: patch` is seeded). The user keeps it updated as work lands — agents don't touch `CHANGELOG.md` (see [Committing](#committing)). A failed wetrun is resumable: re-run `--wetrun` without `--bump`.

**Conformance gates (Step 3b).** The external-oracle correctness gates (see [Corpus Comparison](#corpus-comparison)) run here via `deno task conformance:all`; skipped by `--no-check`. The step preflights the oracles (`../svelte`, `../acorn-typescript`, `../typescript`, `../test262` checkouts + the `benches/js` `node_modules` sidecar, `deno task bench:install`): a **`--wetrun` FAILS** when any is missing (releasing without gates requires the explicit `--no-check`), a dry-run warn-and-skips, and any skip is re-warned in the run's final summary. `deno task doctor` checks the same setup (and more) ahead of time. Only the CSS-WPT harvest stays manual. A `corpus:compare:format` SAFETY hit is self-verified in-run (the native format is re-run and must reproduce byte-identically), so treat it as real; FFI nondeterminism surfaces as a loud `native format nondeterminism` per-file error instead (see ./benches/js/CLAUDE.md §Known Issues).

```bash
deno task publish                        # dry-run: validate everything, no mutation
deno task publish --wetrun --bump patch  # release: bump + publish + git finalize (--bump required, must match CHANGELOG marker)
deno task publish --wetrun               # resume a failed wetrun (sentinel retry only)
# Flags: --bump patch|minor|major, --no-check, --no-git
deno task test:npm[:parse|:all]          # builds the npm package, then runs Node tests against it (:all includes CLI tests; `:run` suffix skips the rebuild)
deno task validate:artifacts             # tight wasm size bounds + Deno smoke of all built bundles (fails if nothing is built)
```

`scripts/validate_artifacts.ts` holds deliberately tight (~±8%) size bounds — a legitimate binary size change fails the publish until the constants are updated, keeping size moves visible and intentional.

**TS type maintenance**: `crates/tsv_wasm/types/tsv_ast.d.ts` is hand-maintained. Any PR that changes the wire JSON a writer emits (`crates/tsv_*/src/ast/convert/write*`) must also update the `.d.ts`. Drift is caught by `deno task check:ast-types` (part of `deno task check`) and reviewed at PR time.

See ./crates/tsv_wasm/CLAUDE.md §TS type maintenance for the per-field checklist.

### Corpus Comparison

Compare formatting against Prettier, and parse output against the canonical
parsers, on real codebases. The gates, corpus tools, and harvests enforce
**pinned expected counts** on full runs. The format `corpus:compare:format --all`
counts are enforced over the **reproducible subset** (the version-pinned
framework + prettier checkouts), so they hold on any `pins:audit`-aligned machine;
the live dev repos are a non-gating WARN (SAFETY still gates every file). Parse
`compared` counts + committed fixtures stay live-growth minimums; the Rust-side
test262/fixtures/swallow gates carry their own — see
`benches/js/lib/gate_counts.ts` and ./benches/js/CLAUDE.md §Pinned gate counts:

```bash
deno task corpus:compare:format ~/dev/some-project  # single project (or --all for the gates corpus view: real repos + prettier suites)
# Options: --explain (show patterns matched), --summary (compact, no diffs),
#          --json (single JSON report to stdout: stats + safety/partial/unknown/error lists; logs → stderr)

deno task corpus:compare:parse --all   # deep-diff parse ASTs vs acorn-typescript/svelte/parseCss
# Options: --multibyte-only (offset-translation slice), --filter <lang>, --limit <n>, --json

deno task conformance:svelte-fixtures  # tsv's Svelte parser vs Svelte's own test suite (../svelte/packages/svelte/tests)
# Drop-in-parser analog of test262 (JS) / wpt (CSS); oracle = the live modern Svelte parser. Verdict
# parity gates (over-rejections must be SANCTIONED or a tracked KNOWN_GAP, else exit 1); AST-shape
# diff is a report-only triage surface.

deno task conformance:ts-fixtures      # tsv's TS parser vs acorn-typescript's own test suite (../acorn-typescript/test)
# Same shape; oracle = the live @sveltejs/acorn-typescript parser (the adversarial TS edge-case corpus).
# Strict setup: a missing ../acorn-typescript checkout (0 scanned) FAILS — publish Step 3b's preflight
# probe is the tolerance point. Both fixtures gates freshness-check their ledgers on full-suite runs
# (a stale sanction/known-gap entry fails) and warn on checkout↔npm-oracle version skew.

deno task conformance:ts-repo          # tsv's TS parser vs the tsc corpus (../typescript conformance/parser tests)
# Oracle = tsc's own error baselines (tests/baselines/reference/*.errors.txt; a TS1xxx code = tsc's
# parser rejects). Buckets accept/reject parity, over-acceptances (the deferred-early-error surface),
# and tracked gaps. A missing/PARTIAL ../typescript checkout, or an empty scan, FAILS. See ./benches/js/CLAUDE.md.

# The three gates above accept: -v, --json, <subtree>.

deno task conformance                  # the pre-release aggregate: the three gates above + corpus:compare:parse
# --all + corpus:compare:format --all, in ONE process (benches/js/conformance.ts; oracle modules load
# once, fail-fast, corpus FFI built once), then render:audit over the version-pinned checkouts (a
# subprocess — it drives its own sidecar). The external-oracle correctness gates that can't live in
# `deno task check`. The format leg's prettier calls ride a content-addressed cache
# (benches/js/lib/prettier_cache.ts; TSV_PRETTIER_CACHE=0 disables).

deno task conformance:test262          # tsv's JS parser vs test262 POSITIVES (pure Rust, `test262 --gate`);
# negatives (the deferred early-error frontier) are reported, not gated. Exact POSITIVE_PASSED_PIN in the command.
deno task conformance:all              # the full drop-in conformance gate = `conformance` (5 FFI legs) +
# `conformance:test262` (pure Rust). What publish Step 3b runs. CSS-WPT harvest stays manual.

deno task divergence:audit         # audit divergence pattern coverage (--json for machine-readable)
deno task corpus:stats             # corpus/candidate-dir sizes + language + degenerate-case stats (diagnostic; ./benches/js/CLAUDE.md)
```

The corpus comparison builds with `--profile corpus` (optimized + `panic = "unwind"`, no LTO) so panics in our code are caught and reported as errors instead of crashing the process. `corpus` is also the single build world every `deno task check` audit runs under, so it trades LTO for build time — measurably free at runtime, see the profile's own comment in `Cargo.toml`. Benchmarks use `--release` (with `panic = "abort"`, LTO on) for maximum performance.

Divergence detection identifies known differences documented in `conformance_prettier.md` (safety checks, pattern detection, traceability). See ./benches/js/CLAUDE.md and ./docs/divergence_detector.md.

### Benchmarks

**Cross-runtime.** One harness runs under **Deno, Node, and Bun** — each emits its own
runtime-labeled report (`report.{deno,node,bun}.{json,md}`), never merged; `deno task
bench:compose` folds them into the combined `report.{json,md}` (the cross-runtime view
tsv.fuz.dev consumes). The native row differs by runtime — Deno loads the **FFI** library,
Node/Bun the **N-API** addon (`tsv_napi`); everything else is runtime-neutral shared code.
Full detail: ./benches/js/CLAUDE.md §Cross-Runtime.

**Perf vs conformance surfaces.** `deno task bench:perf` measures a **real-world-only**
corpus (app + framework source) — the throughput headline; every in-scope tool must fully
process every file or the run fails (`benches/js/lib/perf_omit.ts`), so coverage is 100%
by construction. `deno task bench:conformance` measures per-tool **parse coverage** over a
**disjoint, fixtures-only** corpus (prettier suites + svelte compiler tests + the
wpt-css/test262 harvests; the Svelte set excludes files `svelte/compiler` rejects, so
coverage measures fidelity on *valid* Svelte) — **coverage-only and node-only by design**
(coverage is a pre-flight product with no timed phase, and runtime-invariant). `deno task
bench` is the full publish-cadence refresh: perf across all three runtimes + compose, then
the node coverage run. The correctness gates (`deno task conformance`, corpus:compare)
keep their own unchanged corpus scope. Full detail: ./benches/js/CLAUDE.md §Corpus.

```bash
# One-time: install the harness's npm deps (package.json is the source of truth;
# both runtimes consume the same node_modules). Re-run after a dep bump or a plain
# `npm install` (which prunes the oxc-parser-wasm binding — see benches/js/CLAUDE.md).
deno task bench:install

# Smoke test (fast sanity check that every formatter+parser produces output; also smoke:node / smoke:bun)
deno task smoke

# Run benchmarks (builds the runtime's bench artifacts automatically).
# `bench` runs ALL three and FAILS FAST if node or bun is missing — Deno is the
# only hard dep, so without node/bun run the per-runtime tasks you have instead.
deno task bench         # full refresh: perf ×3 + compose + node conformance COVERAGE (needs node AND bun)
deno task bench:perf    # perf surface only: all three runtimes + compose
deno task bench:deno    # Deno only (no node/bun needed)
deno task bench:node    # Node only (needs node)
deno task bench:bun     # Bun only (needs bun; reuses the Node artifacts)
deno task bench:compose # Fold existing per-runtime reports → combined report.{json,md}

# Run without rebuilding (if already built; aborts on stale artifacts)
deno task bench:deno:run
deno task bench:node:run
deno task bench:bun:run

# Conformance surface: per-tool parse COVERAGE, fixtures-only corpus disjoint from perf (Svelte set
# minus canonical-rejects) (parse groups only) → report.conformance.node.{json,md}. Coverage-only + node-only
# by design (see "Perf vs conformance surfaces" above): entries carry null timing.
deno task bench:conformance        # harvest + build:bench:node + coverage run
deno task bench:conformance:run    # skip harvest + rebuild (freshness-guarded)
deno task bench:harvest            # regenerate the wpt-css + test262 + svelte-reject + svelte-styles caches (first three freshness-stamped: skip when the source commits + pins match, --force after harvest-logic changes; svelte-styles always re-harvests — live-repo source, ~2 s)

# Per-file skip detail (off by default — counts always shown, paths/errors opt-in)
deno task bench:deno:run -- --verbose

# Environment variables (any runtime): BENCH_LIMIT, BENCH_FILTER, BENCH_DURATION,
# BENCH_WARMUP, BENCH_MODE, BENCH_CORPUS, BENCH_STALE_OK, BENCH_FORCED_ASYNC —
# semantics + defaults in ./benches/js/CLAUDE.md
BENCH_FILTER=zzz BENCH_LIMIT=10 deno task bench:deno:run
```

**Prerequisites**: `cargo install wasm-pack` and `deno task bench:install` once
(the install needs `npm`, i.e. Node, to populate `node_modules`). Beyond that,
**Deno is the only hard dependency** — `bench:deno` / `smoke` run with just Deno.
Node ≥ 22.18 (native TS type-stripping) is needed for `bench:node` (and the
install); Bun for `bench:bun`. The aggregate `deno task bench` needs both and
fails fast if either is missing; run the per-runtime tasks for the ones you have.

Compares: canonical (prettier + svelte/compiler), native (FFI under Deno / N-API under Node
and Bun), WASM, and alternatives (oxc-parser, oxfmt, biome-wasm, and dprint-wasm —
`dprint-plugin-typescript`, the engine `deno fmt` runs, TS/JS only). Each runtime's
results save to `benches/js/results/report.<runtime>.{json,md}` (committed; every row
carries a `runtime` field), plus the combined `report.{json,md}`. To publish to tsv.fuz.dev: `npm run update-benchmarks` in ~/dev/tsv.fuz.dev. See
./benches/js/CLAUDE.md.

### Performance Profiling

```bash
cargo run --release -p tsv_debug -- profile ~/dev/zzz/src/lib        # profile a directory
cargo run --release -p tsv_debug -- profile file.ts --iterations 20  # more iterations
# Also: --json (machine-readable)

cargo run --release -p tsv_debug -- json_profile ~/dev/zzz/src/lib   # parse vs wire-JSON write timing
```

For function-level hotspots, use `perf` with the `profiling` cargo profile:

```bash
cargo build --profile profiling -p tsv_debug
perf record --call-graph=dwarf -- target/profiling/tsv_debug profile ~/dev/zzz/src/lib
perf report --stdio                              # function-level hotspots
perf annotate --stdio -s fits_with_lookahead     # line-level within a function
```

See ./docs/performance.md.

## Configuration

**Non-configurable by design.** Formatting options are fixed at Prettier's defaults, except where noted below, and cannot be changed — there are no config files, CLI flags, or runtime options, and none are planned. tsv is opinionated like `gofmt` and Black: one canonical style, always. A narrower user-facing option set may be revisited far down the road, but the 0.x contract is no configuration at all.

**The one carve-out is file *scope*, not style.** `tsv format`'s directory discovery is
gitignore-aware, with two regimes keyed on `.git`. Inside a git repo the **format root**
(the scope boundary — derived from the argument, never the cwd) is the repo root, a hard
stop for the upward walk, and discovery honors `.gitignore`, then `.formatignore` (tsv's
native file; its `!` can re-include a gitignore'd path), then `.prettierignore` (drop-in
compat; the fallback in any directory with no sibling `.formatignore`), all hierarchical,
plus the always-skipped safety nets (`.git`, `node_modules`, `.sl`, `.hg`, `.svn`, `.jj`).
Outside a repo only `.formatignore` is read, from the filesystem root down (so
`~/.formatignore` acts as global config for loose files). Because the boundary is found by
walking up, formatting a subdirectory gives the same result as formatting it via an
ancestor. A `.gitignore` in scope turns the built-in heuristic (hidden dirs +
`dist`/`build`/`target`) **off**. This is *only* about which files are reformatted, never
how; an explicitly named file argument is always formatted. The matcher is the
`tsv_ignore` crate (`IgnoreStack`); the per-directory prune *decision* (heuristic, safety
nets, shadow warning) is `tsv_discover` — both shared with the JS CLI and the VS Code
extension via WASM (`classify_dir` / `should_format_file` / `heuristic_shadow_warning` /
`is_path_pruned`), so all three surfaces agree by construction. Authoritative rules +
edge cases (parent-directory rule, re-include idiom, unreadable ignore files, warnings):
./docs/cli.md §Multi-File Formatting.

The list below covers the settings that diverge from Prettier's defaults; everything else (e.g. tabWidth=2) matches Prettier.

- `printWidth` (100) — Wider than Prettier's default of 80
- `useTabs` (true) — Tabs, not spaces (Prettier defaults to off)
- `singleQuote` (true) — Single quotes, not double (Prettier off)
- `trailingComma` ('none') — No trailing comma on multiline lists; differs from Prettier's `'all'`

`trailingComma: 'none'`: no trailing comma is emitted even when a list breaks across lines. With `useTabs` and `singleQuote`, this matches the Svelte project's own Prettier config (`.prettierrc`).

**Measuring line widths**: Use `cargo run -p tsv_debug line_width <file>` to measure visual width of lines (accounts for tabWidth=2). Never use `wc -c` — it counts bytes, not visual characters (tabs are 1 byte but 2 visual chars). The `compare` command also shows line widths on changed lines.

### Internal Configuration (Rust Library Only)

There is no runtime configuration. Print width / tab width / indent are compile-time `pub const`s in `tsv_lang::config` (`PRINT_WIDTH`, `TAB_WIDTH`, `INDENT`), read directly by the renderer — not threaded through any signature. Quote preference is likewise hardcoded (single quotes) in `tsv_lang::printing` — the `optimal_string_quote` tie-break that `format_string_literal` applies. The doc-builder unit tests exercise the layout at smaller widths via the internal `RenderConfig` seam (`doc::render_config`, `pub(crate)`), never at runtime.

One type carries genuine per-input *state* (not configuration), threaded only where it varies:

- `tsv_lang::EmbedContext { base_indent_offset, first_line_offset, suffix_width, mode: LayoutMode }` — embedding state for nested formatting (CSS in `<style>`, Svelte template expressions). `LayoutMode { Standalone, Embedded }` controls binary-expression indent style.

TypeScript formatting is identical for standalone `.ts` and Svelte-embedded TS, so there is a single entry point:

```rust
use tsv_ts::format;

let formatted = format(&ast, source); // same output standalone or Svelte-embedded
```

## Project Structure

```
tsv/
├── crates/
│   ├── tsv_lang/    # Foundation (span, location, error, doc builder, printing utils)
│   ├── tsv_arena/   # Per-thread reusable AST/doc arenas for the bindings' hot loop (tsv_ffi, tsv_napi, tsv_wasm)
│   ├── tsv_html/    # HTML element classification and whitespace rules
│   ├── tsv_ignore/  # gitignore-aware matcher: hierarchical .gitignore + .formatignore/.prettierignore
│   ├── tsv_discover/# file-discovery policy (build-output heuristic + safety nets) over tsv_ignore
│   ├── tsv_ts/      # TypeScript: parse(), format(), convert_ast_json_bytes()
│   ├── tsv_css/     # CSS: parse(), format(), convert_ast_json_bytes()
│   ├── tsv_svelte/  # Svelte: parse(), format(), convert_ast_json_bytes()
│   ├── tsv_cli/     # Production CLI (binary: tsv) - pure Rust
│   ├── tsv_debug/   # Dev utilities (binary: tsv_debug) - uses Deno
│   ├── tsv_ffi/     # C FFI bindings (Deno's native path)
│   ├── tsv_wasm/    # WebAssembly bindings (published as @fuzdev/tsv_format_wasm + @fuzdev/tsv_parse_wasm + @fuzdev/tsv_wasm; bundles hand-maintained types/tsv_ast.d.ts + npm/locations.js no-loc reconstruction helper; npm/cli.js is the tsv bin)
│   └── tsv_napi/    # N-API bindings (Node/Bun native path; measurement-only for the Node bench, publish is a fast-follow after 0.2)
├── scripts/         # Publish orchestrator, npm package patcher, Node artifact + N-API addon tests, AST type drift check
├── tests/           # Integration tests (parser, formatter, CLI)
│   └── fixtures/    # Test fixtures organized by language/feature
└── docs/            # Documentation (fixtures, cli, architecture, etc.)
```

**Crate pattern** (tsv_ts, tsv_css, tsv_svelte):

- `lib.rs` - Public API: `parse()`, `format()`, `convert_ast_json_bytes()`
- `ast/` - Internal AST + the conversion layer (the wire-JSON writer)
- `lexer/` - Tokenization
- `parser/` - AST construction
- `printer/` - Code formatting (uses doc builder from tsv_lang)
- `escapes/` - Language-specific escape handling (tsv_ts, tsv_css only; Svelte delegates to TS/CSS)

`tsv_ts` and `tsv_css` also export embedding APIs for `tsv_svelte`: `parse_with_interner`, `parse_embedded`, expression formatting variants, `build_*_doc` functions.

### Conformance

**Comment position is preserved by default — but the rule is principled, not
absolute.** A core tsv stance and the single largest category of deliberate
divergence from Prettier: a comment's placement is usually an authoring choice
that communicates what it refers to, so tsv keeps comments where the author wrote
them rather than moving them to a "canonical" position. Prettier routinely
relocates comments across syntactic boundaries (into adjacent blocks, parens, onto
their own line, past `;`), and in doing so often **loses information** — two
comments merging onto one line (the second `//` becoming text), or reordering
them. tsv treats such a boundary as semantic and holds the comment in place.

The line tsv draws: **preserve when the comment's position carries authorship
signal, or when relocating would lose information** (the common case, and why most
relocations are divergences). But tsv will **deliberately trail** a same-line line
comment past a *pure separator* when doing so is **lossless and the position
carries no signal** — e.g. a line comment between a list element and its comma
(`A // c⏎, B` → `A, // c`): the comma is structure, the comment trails the element
either way, and the list's per-element line breaks keep even multiple comments
distinct, so there is nothing to preserve and tsv matches Prettier. That carve-out
is a deliberate choice, **not** a gap to close. (Contrast the name→`=`/`:`/`?`
binding cases, where two comments *would* collide on one trailing line — there tsv
preserves + continuation-indents to stay lossless, diverging from Prettier's merge.)

Separately, the union-member / parenthesized-intersection alignment rendering
(`type T = | { // c } | B`) is the one remaining spot where tsv still matches a
Prettier relocation that crosses a semantic boundary — an un-converted
implementation gap whose fix is coupled to the intersection-printer convergence.
(The value-level function-definition parameter `(` and the `with {…}`
import-attribute brace, formerly in this list, now preserve.) When a fix changes
comment handling, default to preserving position; matching Prettier is fine only
when trailing is lossless and the position carries no signal — otherwise add a
`_prettier_divergence` fixture. Full principles + the divergence catalog:
./docs/conformance_prettier.md §Comment Position Philosophy.

- ./docs/conformance_prettier.md - Where we differ from Prettier (and why)
- ./docs/conformance_svelte.md - Where we differ from Svelte (and why)

## Fixtures

See [Development Philosophy](#development-philosophy-test-driven-development-with-fixtures) for the TDD workflow.

### Fixture Protection Rules

**Sources of truth**: Prettier and Svelte's parser. Fixtures record what these tools produce.

**When a fixture test fails:**

1. **First, verify the fixture is correct** by checking against prettier/Svelte:

   ```bash
   cargo run -p tsv_debug compare <fixture>/input.svelte  # Compare with prettier
   cargo run -p tsv_debug canonical_parse <fixture>/input.svelte  # Check Svelte's AST
   ```

2. **If the fixture matches prettier/Svelte**: The fixture is correct. Fix our code to match.

3. **If the fixture doesn't match prettier/Svelte**: The fixture may be outdated or incorrect. Update it:
   ```bash
   deno task fixtures:update <pattern>  # Regenerate from prettier/Svelte
   ```

**CRITICAL: Never modify fixtures to work around our bugs.** Fix the code, not the fixture.

**Prohibited** (without verifying against sources of truth): modifying `input.svelte` to avoid edge cases, removing `unformatted_*` test cases, changing `expected.json` to match incorrect output, any fixture change that hides a bug.

**When our formatter differs from prettier:**

- Default: for cosmetic or ambiguous differences, match prettier — but a mismatch is a question, not automatically a bug. Diverge when there's a defensible reason, recorded in a `_prettier_divergence`
- Spec precedence: when the spec defines a canonical form prettier doesn't emit, follow the spec — even if prettier's output is itself valid. Document with spec refs in a `_prettier_divergence`
- Comment position: when prettier moves comments to different syntactic positions, preserve the user's placement. See ./docs/conformance_prettier.md#comment-position-philosophy
- Other defensible tsv-native choices (print width as a hard limit, a clearly better layout) are legitimate too — just sanction them deliberately, never to hide a bug
- `_prettier_divergence` suffix: deliberate, documented intentional differences only. Requires a README that **links back to its `conformance_prettier.md` section** (`See [conformance_prettier.md §…](…)`) and a matching catalog entry there. Never use to hide bugs.

---

**References:** ./docs/fixture_workflow.md (creation), ./docs/fixture_overview.md (validation, troubleshooting), ./docs/fixture_naming.md (naming conventions)

---

**Core Invariant**: Input file **always formats to itself** (idempotent) - no exceptions, save one deliberate opt-out: a `tsv_rejects.txt` fixture, whose input tsv *rejects* (the canonical parser accepts), so F1 doesn't apply (see F7/S20)

**Directory Hierarchy**: Each fixture directory has either an input file (fixture) or subdirectories (container), not both, not neither.

**Fixture Organization Policy**: Organize by feature. Comment fixtures belong with the feature they test (e.g., `calls/chained/*_comment`), not centralized. Use `syntax/comments/` only for basic comment syntax, universal formatting rules, and cross-cutting edge cases.

**Input File Types:**

- `input.svelte` (preferred) - Tests code embedded in Svelte context
- `input.ts` (rare) - Only for byte-0 file-level features (hashbang, BOM) or constructs that format differently between contexts (JSDoc cast paren stripping). TS-only _syntax_ (`import =`, `export =`, types, decorators, `declare`) still uses `.svelte` with `lang="ts"`
- `input.css` (rare) - Only for file-level CSS features (e.g., BOM at byte 0)
- `input.svelte.ts` (runes) - Svelte rune modules (`$state`, `$derived`, etc.)

⚠️ **Prefer `.svelte`**: For CSS, it's the only path with an external canonical source. See ./docs/fixture_overview.md#why-svelte-is-the-default-canonical-source.

**Fixture File Structure:** `input.*` + `expected.json` at minimum. Every optional
sibling makes a precise, validated claim — `expected_ours.json` / `expected_svelte.json`
(parser divergence), `output_prettier.*` / `prettier_variant_*` / `variant_*` /
`divergent_variant_*` / `prettier_intermediate_*` / `prettier_intermediate_to_variant_*` /
`audit_signature.txt` (formatter divergence + prettier multi-pass pins),
`prettier_nonconvergent.txt` / `prettier_rejects.txt` / `tsv_rejects.txt` (no-oracle
markers), `unformatted_*` / `unformatted_ours_*` / `unformatted_prettier_*`
(normalization variants), `input_invalid_*` (must fail both parsers). Per-file semantics
and validation rules (F/S/R/D): ./docs/fixture_overview.md.

**Other file types** (same structure): `.ts`/`.svelte.ts` use acorn-typescript for parsing; `.css` uses Svelte's `parseCss`. All use prettier for formatting.

**Unformatted variant rules:** Same content structure as input, only whitespace differs. Both formatters must normalize to exactly match input. For `.svelte` fixtures this is **enforced**, not just convention: the render-equivalence check (R rules, see ./docs/fixture_overview.md) asserts the variant and `input` produce the same browser-visible render via `svelte compile` — so a formatter bug that changed the render *and* happened to land on `input` can no longer pass green.

**Invalid syntax rules (`input_invalid_*`):** Must fail BOTH parsers. One syntax error per file.

**Quick Pattern Selection:**

- **Parser matches Svelte**: `input.svelte` + `expected.json`
- **Parser differs intentionally**: Add `expected_ours.json` + `expected_svelte.json` (requires `_svelte_divergence` suffix)
- **Formatter matches prettier**: Add `unformatted_*.*` variants
- **Formatter differs intentionally**: Add `output_prettier.*` (requires `_prettier_divergence` suffix)
- **Prettier has stable variants (ours normalizes)**: Add `prettier_variant_*.*` files (requires `_prettier_divergence` suffix)
- **Dual-stable forms (both keep stable)**: Add `variant_*.*` files (requires `_prettier_divergence` suffix)
- **Divergent variant (prettier keeps stable, ours → third form)**: Add `divergent_variant_*.*` files (requires `_prettier_divergence` suffix)
- **Normalization to input divergence**: `unformatted_ours_*.*` normalizes to input with our formatter only
- **Normalization to output_prettier**: `unformatted_prettier_*.*` normalizes to `output_prettier.*` with prettier
- **Prettier never converges (no oracle)**: Add `prettier_nonconvergent.txt` + README (requires `_prettier_divergence` suffix; excludes all prettier-claim files)
- **Prettier rejects/throws on input (no oracle)**: Add `prettier_rejects.txt` (trimmed content = expected-error substring) + README (requires `_prettier_divergence` suffix; excludes all prettier-claim files; mutually exclusive with `prettier_nonconvergent.txt`)
- **tsv over-rejects but canonical accepts**: Add `tsv_rejects.txt` (trimmed content = expected tsv-error substring) + `expected_svelte.json` + README (requires `_svelte_divergence` suffix; no `expected.json`/`expected_ours.json`; excludes all format-claim files, `input_invalid_*`, and the prettier no-oracle markers)
- **Both differ**: Use `_svelte_prettier_divergence` suffix

## Debug Tooling

**tsv_debug** uses an embedded Deno sidecar for JS tools (prettier, Svelte parser, acorn). Requires Deno. Sidecar spawns on first use and is reused (orders of magnitude faster than spawning per call).

```bash
curl -fsSL https://deno.land/install.sh | sh  # Install Deno if needed
cargo run -p tsv_debug check                  # Verify sidecar works
```

### Commands

**Input methods** (consistent across content-processing commands):

- **File path**: `command <file>` - Auto-detects parser from extension
- **Content**: `command --content <string> --parser <type>` - Requires `--parser svelte|typescript|css`
- **Stdin**: `echo '...' | command --stdin --parser <type>` - Requires `--parser svelte|typescript|css`

**Content-Processing Commands:**

```bash
# compare - diff our formatter vs prettier (shows line widths on changed lines)
cargo run -p tsv_debug compare file.svelte
# Options: --verbose/-v (full input/ours/prettier), --quiet, --color <auto|always|never>, --json
# Line widths appear as right-aligned numbers on diff lines (helps spot printWidth issues)
# "Outputs match" = ours(input) == prettier(input), NOT input stability; a match on a
# non-format-stable input adds a note + input-vs-formatted diff (F1 fails on such an input)

# ast_diff - verify semantic equivalence
cargo run -p tsv_debug ast_diff input.svelte                         # round-trip: parse → format → parse → compare
cargo run -p tsv_debug ast_diff input.svelte output_prettier.svelte  # compare two files' ASTs
cargo run -p tsv_debug ast_diff --render input.svelte               # render-aware: normalize both ASTs per Svelte 5
# --render normalizes template whitespace per Svelte 5 (collapse inter-node runs to one space, trim
# start/end-of-content whitespace, honor <pre>/<textarea>) BEFORE comparing — so render-equivalent
# forms match even though the parser keeps boundary whitespace verbatim. Sound: real content /
# <pre> / presence-of-space changes still differ. Confirms block-style render-equivalence at corpus scale.

# canonical_parse - parse using external parsers (Svelte, acorn+typescript, or our CSS)
cargo run -p tsv_debug canonical_parse file.svelte

# format_prettier - format using prettier (shows line widths by default; --no-line-widths to hide)
cargo run -p tsv_debug format_prettier file.svelte

# line_width - measure line widths (pure Rust, no Deno needed)
cargo run -p tsv_debug line_width file.svelte           # all lines
cargo run -p tsv_debug line_width file.svelte --line 5  # specific line with preview
# Also: --json
```

**Fixture Management Commands:**

All `fixtures_*` commands accept positional patterns (multiple = OR) and `--list`.

```bash
# fixture_init - create or reinitialize a fixture (formats through prettier + generates expected.json)
cargo run -p tsv_debug fixture_init <dir> --content '<code>'   # create from content string
cargo run -p tsv_debug fixture_init <dir> --stdin              # create from stdin (heredoc)
cargo run -p tsv_debug fixture_init <dir>                      # reformat existing input + regenerate expected.json
# Also: --parser <typescript|css> (non-svelte), --force (overwrite)

# fixtures_validate - verify fixtures are correct (CI). --prettier-only skips our parser/formatter.
cargo run -p tsv_debug fixtures_validate [pattern...]
# Note: cross-fixture duplicate detection skipped when filters are active
# Note: parser mismatch with expected.json is a hard error (no ratchet — all fixtures must match)

# fixtures_update - regenerate from canonical sources
cargo run -p tsv_debug fixtures_update            # both parsed + formatted
cargo run -p tsv_debug fixtures_update_parsed     # expected.json only (Svelte for .svelte, acorn for .ts, parseCss for .css)
cargo run -p tsv_debug fixtures_update_formatted  # output_prettier.svelte (auto-deletes if identical to input; skips prettier_nonconvergent.txt / prettier_rejects.txt / tsv_rejects.txt fixtures — prettier can't format the first two, and a tsv_rejects fixture makes no formatting claim)

# fixtures_audit - investigate normalization graphs (diagnostic; --all for every fixture, not just divergence)
cargo run -p tsv_debug fixtures_audit [pattern...]
# Also: --verbose (full graph), --json

# ts_fixture_audit - verify which input.ts fixtures genuinely need .ts vs. could be .svelte.
# Embeds every .ts file (input + variants) in <script lang="ts"> and checks (tsv AND prettier)
# whether it formats identically. Necessary = byte-0 feature, Svelte-parse-fail, or
# formats-differently; Convertible = formatting-safe only, not a mandate (a fixture may be .ts
# on purpose to cover the standalone tsv_ts/acorn path); Intentional = the INTENTIONAL_TS
# allowlist, reported separately.
cargo run -p tsv_debug ts_fixture_audit [pattern...]
# Also: --verbose (show the TS-vs-Svelte diff on 'formats differently' fixtures)

# conformance_audit - doc/fixture integrity in one fixture walk: divergence fixtures
# cataloged in their conformance doc, every docs/*.md + fixture-README link resolves,
# divergence READMEs back-link their sanctioning doc, no stray READMEs (exceptions live
# in the in-code ALLOWED_NONDIVERGENCE_READMES allowlist). Pure Rust (no Deno); exits
# non-zero on any finding. Gated in `deno task check`. Full detail: ./docs/audits.md
cargo run -p tsv_debug conformance_audit
# Also: --json (machine-readable: {orphans, dead_links, missing_backlinks, stray_readmes})
```

> **Troubleshooting:** See ./docs/fixture_overview.md#quick-decision-tree

**test262 ECMAScript Conformance Tests:**

```bash
# test262 - run ECMAScript conformance tests against our parser (pure Rust, no Deno)
cargo run -p tsv_debug test262                       # run all tests (expects ../test262)
cargo run -p tsv_debug test262 language/expressions  # filter by path pattern
# Options: --path <dir>, --list, --verbose (show all failures), --negative-only, --positive-only,
#          --gate (the release gate: fail ONLY on a positive-parse regression or a shift in the pinned
#           positive count; negatives — the deferred early-error frontier — are reported, not gated.
#           A bare run exits non-zero because negatives fail by design, so it's a diagnostic, not a gate.),
#          --emit-manifest <path> (JSON manifest of the graded strict subset — feeds the tsv-vs-oxc
#           differential consumer, benches/js/diagnostics/test262_compare.ts)
```

See ./docs/conformance_test262.md (command interface; §Differential for the tsv-vs-oxc comparison).

**Performance Profiling Commands** (all pure Rust, no Deno — full reference: ./docs/performance.md):

```bash
cargo run -p tsv_debug profile ~/dev/zzz/src/lib                    # parse vs format phase timing (--iterations, --json)
cargo run --release -p tsv_debug -- json_profile ~/dev/zzz/src/lib  # FFI parse path: parse vs the wire-JSON write (§2)
cargo run -p tsv_debug buffer_sizes ~/dev/zzz/src ~/dev/gro/src     # printer SmallVec sizing histograms (§8)
cargo run -p tsv_debug arena_stats ~/dev/zzz/src/lib                # DocArena node-population + memory audit (§7; --reuse, --list-errors)
```

**Codebase Metrics Commands:**

```bash
# metrics - codebase structure analysis (pure Rust, no Deno needed)
cargo run -p tsv_debug metrics             # line counts by crate and phase (lexer/parser/ast/printer)
cargo run -p tsv_debug metrics --json      # JSON output for scripting
deno task metrics                          # shorthand
```

**Audits** — the standing correctness gates and discovery harnesses: line-comment swallow,
the print-once comment ledger, gap/blank injection, build-fanout, raw-find scan,
authoring-independence, format→reparse round-trip, comment↔token binding,
render-equivalence, layout-neutrality, the seeded mutational fuzzer, the F1 idempotency
sweep, the `audit:corpus` bundle, and the `lex_diff` differential lexer harness. Each is
cataloged in ./docs/audits.md — what it proves, what it is blind to, flags, and where it
gates; the `deno task` entry points are indexed in [Fixtures](#fixtures-rust--deno-based).
Read the relevant section there before running or modifying an audit.

## Architectural Notes

### Closed Scope, Open Convention

tsv ships a closed language set (TypeScript, CSS, Svelte) but is open by
convention **at the Rust source/crate level**: each language crate
(`tsv_ts`, `tsv_css`, `tsv_svelte`) is self-contained — owns its internal
AST, parser, formatter, and convert layer (the wire-JSON writer) — and
exposes the same free-function API (`parse()`, `format()`,
`convert_ast_json_bytes()`, `convert_ast_json_string()`,
`convert_ast_json()`). **No central `Language`
trait, no registry, no enum dispatch.** Two properties follow:

- **Optimal artifacts**: concrete types end-to-end, no dyn dispatch, WASM
  tree-shakes by feature at the link level — `@fuzdev/tsv_format_wasm` excludes
  the convert/serialization layer, `@fuzdev/tsv_parse_wasm` excludes the printers.
- **Source-level openness**: once the `tsv_*` crates hit crates.io, anyone can
  write a `my_org/tsv_html_parse` crate of the same shape and any downstream
  _Rust_ consumer can `use` it without central buy-in. Published CLI/WASM
  binaries still hardcode the language list (`lang_bindings!` macro), by design.

Cross-language coupling exists only where languages integrate — `tsv_svelte`
depends on `tsv_ts` (for `Expression`) and `tsv_css` (for `StyleSheet`).

Avoid inverting this: no central public-AST crate, no `Language` trait with dyn
dispatch, no workspace-level language registry. Full discussion:
./docs/architecture.md#closed-scope-open-convention.

### Strict Mode Only

**`tsv` parses TypeScript/JS as strict mode only.** This is intentional:

- **TypeScript**: Always strict (implicitly `"use strict"`)
- **ES Modules**: Always strict (`import`/`export` implies strict)
- **Svelte `<script>`**: ES modules, always strict

tsv parses the syntactic grammar and rejects only the constructs that are *lexically* sloppy-mode — the `with` statement and legacy octal literals (`010`). Strict-mode **early errors** (duplicate parameter names, reserved words as identifiers, octal string escapes, `delete` of a plain name) still parse for now; enforcement is deferred to a future diagnostics layer. These leaks only matter for standalone JS — Svelte/TS module context is strict, so the real compiler would still flag them.

This is one instance of a broader stance: **the parser is deliberately permissive and defers static-semantic early-errors** (the above, plus the TypeScript ambient-context rules — a `declare` member body, initializer, or decorator, etc.) to the diagnostics layer, so the formatter keeps formatting everything well-formed. The **correctness oracle for what's actually an error is tsc**, not acorn-typescript (which tsv matches only for AST *shape*); the accept-vs-reject test starts with prettier — a construct prettier can't parse, tsv rejects — but among those prettier formats, tsv defers only the **mode/context-dependent** early-errors and still rejects the **unconditional-local** ones (e.g. `get`/`set constructor`). See [crates/tsv_ts/CLAUDE.md §Architecture Position ("Sources of truth")](crates/tsv_ts/CLAUDE.md#architecture-position) and [docs/conformance_svelte.md §TypeScript Corrections](docs/conformance_svelte.md#typescript-corrections).

**Strict ≠ module-only — there is an orthogonal *goal* axis.** Both goals are strict (no sloppy mode, no `"use strict"` detection). A parse runs against `tsv_ts::Goal::{Module, Script}`, exposed as `parse_with_goal` and `tsv parse|format --goal script|module`, **defaulting to `Module`** (correct for Svelte `<script>` and ~all real TS; Svelte hard-wires it). The goal toggles only the four goal-specific constructs: at `Script` goal `await` is an ordinary identifier (`[~Await]` context tracked via the parser's `in_await` flag, save/restored at every function-like scope), and `import`/`export` declarations + `import.meta` are syntax errors (dynamic `import(...)` stays valid). `sourceType` in the public AST follows the goal. See [docs/conformance_test262.md §Strict Mode Only, Explicit Goal Axis](docs/conformance_test262.md).

### Language-Level concerns (classification)

HTML element classification is separated into the `tsv_html` crate (pure functions)
and tool-specific adapters (`tsv_svelte/src/printer/classification/`):

- **tsv_html crate**: Pure classification functions
  (`is_inline_element()`, `is_block_element()`, `is_void_element()`)
  and whitespace rules that operate on tag names (`&str`)
- **Printer adapters**: Thin methods that resolve symbols and call tsv_html functions,
  plus AST traversal utilities

Enables reuse across all planned tools (formatter, linter, compiler, LSP).

### AST Architecture: Internal AST vs Wire JSON

Drop-in replacement for the canonical parsers' **public JSON AST** (acorn /
acorn-typescript / Svelte / `parseCss`), NOT their internal implementation.

- **Internal AST**: Clean, semantic representation (decoded strings, normalized
  values) — what every tool (formatter, linter, …) builds on.
- **Wire JSON**: the parse product. The per-language writers (`ast/convert/write/`)
  emit it **directly from the internal AST** in a single walk — applying each
  acorn/`parseCss`/Svelte quirk at emission time — never materializing a typed
  public-AST Rust layer. The wire shape *is* the contract, documented by the
  hand-maintained `crates/tsv_wasm/types/tsv_ast.d.ts`; consumers that want a tree
  parse the bytes (`convert_ast_json` is a thin `serde_json::from_slice(&bytes)`
  wrapper over the writer).

Worked example + full design: ./docs/architecture.md §Two-AST Design.

**Key Rules**:

- Raw strings NEVER duplicated in the internal AST (extract via `source[span.range()]`)
- The internal AST is NEVER the wire output — the wire JSON is hand-emitted by the
  writer; `serde_json` is used only for exact string-escape / `f64` parity and to
  parse the bytes back into a `Value` (CLI `--pretty`, tests)

### Position Types: u32 vs usize

- **Span**: `u32` for start/end (8 bytes total, 50% memory savings vs usize)
- **`Token`**: `u32` for start/end — `Token` is a 16-byte `Copy`-free POD (`{kind, start: u32, end: u32}`) returned from `next_token` in registers; the decoded value (escapes only) lives out-of-band on the lexer (a reused `Lexer::decode_scratch` buffer, borrowed via `decoded_str`). A `const _: () = assert!(size_of::<Token>() == 16)` guards the size. See [docs/performance.md] and the lexer's byte cursor (`bytes: &[u8]` + `position: usize`).
- **Lexer/Parser positions**: `usize` (natural for `source[pos]` indexing); the lexer dispatches on raw bytes (`cur_byte`) and decodes a `char` only at non-ASCII branches.
- **Conversions**: At boundaries only - `as u32` when creating Spans / `Token` fields, `as usize` when extracting
- **Helpers**: Use `span.extract(source)` or `span.range()` instead of manual casts

### Comment Handling: Detached Model

Comments are stored **separately from AST nodes** in a flat `Vec<Comment>` at the root
level (`Program.comments`, `CssStyleSheet.comments`, `Root.comments`); the printer finds
them via O(log n) binary search on span positions. `Comment` (`tsv_lang/src/comment.rs`)
is a `Copy` POD of spans + flags — text is recovered on demand via
`Comment::content(source)`, never stored owned. The full model — the `Comment` fields,
ownership doctrine, the hazards, and the leading-comment emitter rules — lives in
./docs/comments.md. **Read it before touching comment handling in any printer.** The
always-loaded core:

**Owned comments** (`owned_by_node`, set by the parser): **every glued block comment is
owned** — bound to the token after it and printed by that node's doc rather than by the
enclosing gap, so a synthesized paren can never land between them. A bundler annotation
(`/* @__PURE__ */`), a JSDoc cast (`/** @type {T} */ (x)` — handed to the `JsdocCast`
node), and a plain glued comment bind identically; `owned ⇒ is_block`, so no line comment
is ever owned. **Ownership is a fact about who PRINTS a comment, never about whether it
EXISTS** — every bug in this class has been a violation of that sentence.

A comment can be asked about along exactly **three** axes, and the lookup API
(`tsv_lang::comment`) makes the caller name which:

| axis | question | owned comments | who asks |
| --- | --- | --- | --- |
| **to emit** | "which comments must *I* print here?" | **skipped** | gap emitters (~200 sites) |
| **on page** | "does any comment OCCUPY THE PAGE here?" | **counted** | layout gates — break / expand / hug / paren / fast-path |
| **in source** | "what comment BYTES are physically here?" | **counted** | cursors — blank-line scans, offsets, `prev_end` |

`comments_to_emit_in_range` / `has_comments_to_emit_in_range` / `comments_to_emit_after` ·
`comments_on_page_in_range` / `has_comments_on_page_in_range` /
`has_multiline_block_comments_on_page_in_range` · `comments_in_source_range` /
`comments_in_source_after`. Every name states its axis, so a miswire reads as a category
error at the call site. Two standing corollaries: a **zero-comment fast gate** guarding a
whole builder is an **on-page** question (an emit-keyed one blinds every layout gate it
guards); a **blank-line scan** is an **in-source** question (step over every comment in
the gap via `blank_scan_start` / `blank_scan_end`, not just the ones this caller emits).

⚠️ **Three hazards, all of which have bitten** (full text + war stories in
./docs/comments.md): (1) an owned comment nothing prints is a DROPPED comment — a builder
that *reassembles* a node instead of routing through `build_expression_doc` must claim it
on its own seam (`prepend_owned_leading_comment_at`); (2) an owned comment travels
*inside* its node's doc, so the gap around it can't see it — ask the node instead
(`owned_leading_comment_hangs_value`, the single seam for that question); (3) a region the
parser *lifts out* of its container is still inside the container's gap, so two emitters
print it (`AttrGaps::claimed` is that seam) — and ownership masks it: only a line comment
(never owned) exposes the double-print. The **print-once comment ledger**
(`deno task comments:audit`, gated in `deno task check`; see ./docs/audits.md) is the
structural guard on all three.

⚠️ **Leading comments have one rule and one emitter** — `Printer::push_leading_comment_run`
(prettier's `printLeadingComment`), with `Printer::comment_hugs_next` as the single glue
test and `Printer::push_leading_run_separator` for the three hand-rolled always-broken
sites. Do not hand-roll `is_block && is_same_line(c.span.end, X)` at a new site, and don't
re-derive the anchor+separator inline — keying the hug on the *item* rather than on *what
follows the comment* was a whole bug family. Whether the soft `line` after a leading run
collapses is decided by per-element grouping — the array family groups each element (run
collapses), the params family doesn't (run breaks) — mirrored from prettier; the full rule
is in ./docs/comments.md.

Higher-fidelity models (attached comments, trivia tokens) may be needed for IDE/linter use
cases; prettier, oxfmt and biome all get the JSDoc-cast paren binding wrong — see
[conformance_prettier.md §Comment relocation](docs/conformance_prettier.md#comment-relocation).

## Dependencies

### Rust Crates (minimal deps)

- `serde_json` — wire-JSON emission: the writer's exact string-escape / `f64` formatting, and reparsing bytes to a `Value` (CLI `--pretty`, tests). The language crates no longer depend on `serde` directly (only transitively, without its `derive`); `serde`'s derive is dev-tooling only (`tsv_debug` / `tsv_cli`)
- `smallvec` — Stack-allocated vectors
- `string-interner` — String interning for the residual symbol tenants (Svelte element/attribute names, escaped identifiers); identifier names are span-identity (`IdentName`)
- `thiserror` — Error type derivation
- `phf` — Compile-time perfect hash maps (keywords, entities)
- `unicode-ident` — Unicode XID_Start/XID_Continue for identifiers
- `unicode-segmentation` — Grapheme clustering for visual width measurement
- `unicode-width` — Character display width (CJK, zero-width)
- `bumpalo` — Bump arena for the internal AST (and, via the `tsv_arena` crate, the bindings' per-thread `reset()` reuse — `tsv_ffi`/`tsv_napi`/`tsv_wasm`)
- `talc` — WASM global allocator (`tsv_wasm` only, wasm32-only target dep): pure-Rust `no_std` allocator replacing std's default dlmalloc; the `WasmGrowAndExtend` source keeps the warm instance's linear-memory high-water at dlmalloc parity. Pulls `lock_api` + `allocator-api2` (+ `scopeguard`) into the wasm32 graph only; native builds unaffected
- `napi` / `napi-derive` / `napi-build` — N-API bindings for `tsv_napi` (Node/Bun native addon; tsv-scoped carve-out)

## Canonical References

**Implementations** (versions pinned in `crates/tsv_debug/src/deno/sidecar.ts`):

- Prettier (`../prettier/`) — Formatting reference — read source for layout logic
- Svelte compiler (`../svelte/`) — Parsing reference

**IMPORTANT**: Read `../prettier/` source code instead of searching the web when investigating
formatting behavior. Key files: `src/language-js/print/assignment.js` (assignment layout),
`src/language-js/print/call-arguments.js` (call arg expansion), `src/language-js/print/member-chain.js`
(chain formatting), `src/language-js/print/binaryish.js` (binary operators).

**Specs** — consult BEFORE implementing CSS/HTML/JS features (don't search the web):

- CSS — `../csswg-drafts/`
- CSS Houdini — `../css-houdini-drafts/` (the Houdini Task Force's own repo, not part of `csswg-drafts`; home of `css-properties-values-api`, the `@property` spec)
- HTML — `../html/`
- DOM — `../dom/`
- ECMAScript — `../ecma262/`
- test262 — `../test262/`
- Web data — `../webref/`

**Workflow**: Read local spec → `canonical_parse` to test behavior → `compare` to check formatting.

## Development conventions

- **Leave `// TODO:` comments** - when there's known future work or the code smells

## Documentation

### Priority & Planning

- ./docs/architecture.md - design decisions
- ./README.md - project overview and current status

### Implementation Guides

- ./docs/cli.md - CLI architecture and command patterns
- ./docs/audits.md - the standing audit gates: what each proves, blind spots, flags, gating
- ./docs/comments.md - the detached comment model: ownership, the three axes, hazards, emitters
- ./docs/performance.md - profiling methodology, tooling, and results tracking
- ./docs/workflow_corpus.md - corpus-driven formatting conformance workflow
- ./docs/workflow_test262.md - test262 conformance workflow
- ./docs/fixture_workflow.md - **step-by-step script for creating fixtures**
- ./docs/fixture_overview.md - Validation rules, troubleshooting, divergence patterns
- ./docs/fixture_naming.md - content naming conventions

### Language Checklists

- ./docs/checklist_css.md
- ./docs/checklist_svelte.md
- ./docs/checklist_typescript.md

## Bash Tool Notes

Use heredocs for multiline strings (`cat <<'EOF'`), `$(...)` for command substitution (not backticks), double quotes for strings with spaces.
