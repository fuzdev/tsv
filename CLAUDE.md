# tsv

> a formatter, parser, and future linter + more for Svelte, TypeScript, and CSS

High-performance Rust parser as a drop-in replacement for Svelte's modern parser (acorn + acorn-typescript), paired with a formatter that took Prettier as its initial guide and still tracks it for the common case — while making deliberate, cataloged choices to diverge where tsv's own judgment is more defensible.

**Non-configurable by design**: formatting options are fixed at Prettier's defaults except printWidth=100, useTabs=true, singleQuote=true, and trailingComma='none' — no config files, CLI flags, or runtime options, ever (opinionated like `gofmt` and Black). The one carve-out is file *scope*, not style: `tsv format` honors `.gitignore` (hierarchically, in a git tree) plus a repo-root `.formatignore` / `.prettierignore`. See [Configuration](#configuration).

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
deno task check                          # Full validation (check + test + clippy + fmt)
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

`format` writes paths **in place** (only when output differs) and prints changed paths to stdout; `--content`/`--stdin` print formatted source to stdout. Directories recurse over `.ts`/`.svelte`/`.css`, honoring `.gitignore`/`.formatignore`/`.prettierignore` and always skipping the safety nets (`.git`, `node_modules`, `.hg`/`.svn`/`.jj`); an explicitly named file argument bypasses the ignore files. Discovery is gitignore-aware and reproducible, scoped to a cwd-independent **format root** (the repo root in a git tree, else the filesystem root) — see [Configuration](#configuration) for the full two-regime rules. `--list` prints the discovered in-scope files (one per line) without formatting — a read-only view of what `format` would touch (path mode only; an empty scope still exits 0). Files format in parallel (`--jobs N` overrides the thread count; path mode only). Exit codes: 0 clean, 1 would-change (`--check`, which also works with `--content`/`--stdin`), 2 errors; missing path args fail the run upfront, while per-file and traversal errors report and continue.

```bash
cargo run -p tsv_cli parse file.ts                                       # compact JSON
cargo run -p tsv_cli parse file.ts --pretty                              # formatted JSON
cargo run -p tsv_cli parse --content '<div>x</div>' --parser svelte      # parse string (preferred for agents)
cargo run -p tsv_cli parse --stdin --parser svelte                       # parse stdin (not preferred for agents)
cargo run -p tsv_cli format file.svelte src/lib                          # format files/dirs in place
cargo run -p tsv_cli format --check src/lib                              # list would-change files, exit 1 (CI)
cargo run -p tsv_cli format --list src/lib                               # list in-scope files (no formatting)
cargo run -p tsv_cli format --content '<div>x</div>' --parser svelte     # format string to stdout
```

### Testing & Code Quality

```bash
deno task check          # typecheck + test + lint + fmt (full validation)
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
deno task fixtures:validate          # validate (use during fixture work; --prettier-only skips our parser/formatter)
deno task fixtures:update            # regenerate expected.json + output_prettier.svelte (source of truth)
deno task fixtures:update:parsed     # regenerate expected.json only (run when parser changes)
deno task fixtures:update:formatted  # regenerate output_prettier.svelte only
deno task fixtures:audit             # audit _prettier_divergence fixtures (diagnostic; --all for every fixture)
deno task conformance:audit          # doc/fixture integrity: divergence fixtures cataloged + every doc/README link resolves + each divergence README back-links its sanctioning doc + no stray READMEs on matching fixtures (gated in `deno task check`)
deno task scan:audit                 # guard against new raw find/rfind/match_indices substring scans over source (gated in `deno task check`); see Debug Tooling
deno task fanout:audit               # guard against super-linear doc-node fanout (the per-layout-candidate rebuild blowup); gated in `deno task check`; see Debug Tooling
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
- `tsv_napi` (napi-rs) — target: Node.js / Bun native addon; output: `libtsv_napi.{so,dylib,dll}` (loaded via `process.dlopen`). Currently a **measurement-only** binding for the Node benchmark runner (single-platform local build, `deno task build:napi`, with `deno task test:napi` driving the Node-side boundary tests in `scripts/test_napi.ts`); the cross-platform publish matrix as `@fuzdev/tsv_napi` is targeted for 0.2. tsv-scoped carve-out from the ecosystem N-API deferral. See ./crates/tsv_napi/CLAUDE.md.

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
- `@fuzdev/tsv_parse_wasm` — parse only (`--no-default-features --features parse`); bundles hand-maintained `tsv_ast.d.ts` from `crates/tsv_wasm/types/`
- `@fuzdev/tsv_wasm` — full tool (default build, both features); bundles `tsv_ast.d.ts` and ships the `tsv` bin (`crates/tsv_wasm/npm/cli.js` — `format` + `parse` subcommands mirroring `tsv_cli`'s flags/exit codes, `node:util` `parseArgs`, zero deps, single-threaded)

A separate types-only `@fuzdev/tsv_ast` package is deferred — `import type` from `tsv_parse_wasm` is zero-runtime-cost, and no 0.1 consumer profile needs the standalone split. Reconsider if/when a real consumer appears. `@fuzdev/tsv` (bare) stays reserved for a future native-binary flagship.

Version source of truth: `Cargo.toml` `[workspace.package] version` (read directly by `wasm-pack`). No root package.json, no changesets. All published packages move together at this version.

Package shape: built from the wasm-pack `web` target, then `scripts/patch_npm_package.ts` adds a Node/Bun entry (`index.js`, sync auto-init), a browser entry (`browser.js`, guarded `await init()`), `index.d.ts`, conditional `exports`, npm metadata, and the variant README. The export list is extracted from the generated JS, so new `lang_bindings!` languages flow through automatically.

`scripts/publish.ts` orchestrates the release end to end (preflight → bump → check → build (npm packages + deno bundles, so artifact validation never sees stale bundles) → verify → artifact validation: size bounds + Deno smoke + Node tests → idempotent npm publish → git commit + tag + push), printing a wasm size summary (raw + gzipped) at the end. It stamps CHANGELOG.md's `## Unreleased` section into the released version's section — that section must be non-empty and carry a `<!-- bump: <level> -->` marker that matches `--bump` (the bump is required in **both** places and they must agree; on stamp the marker is dropped and a fresh empty `## Unreleased` reset to `bump: patch` is seeded for the next cycle). The user keeps it updated as work lands — agents don't touch `CHANGELOG.md` (see [Committing](#committing)). A failed wetrun is resumable: re-run `--wetrun` without `--bump`.

```bash
deno task publish                        # dry-run: validate everything, no mutation
deno task publish --wetrun --bump patch  # release: bump + publish + git finalize (--bump required, must match CHANGELOG marker)
deno task publish --wetrun               # resume a failed wetrun (sentinel retry only)
# Flags: --bump patch|minor|major, --no-check, --no-git
deno task test:npm[:parse|:all]          # builds the npm package, then runs Node tests against it (:all includes CLI tests; `:run` suffix skips the rebuild)
deno task validate:artifacts             # tight wasm size bounds + Deno smoke of all built bundles (fails if nothing is built)
```

`scripts/validate_artifacts.ts` holds deliberately tight (~±8%) size bounds — a legitimate binary size change fails the publish until the constants are updated, keeping size moves visible and intentional.

**TS type maintenance**: `crates/tsv_wasm/types/tsv_ast.d.ts` is hand-maintained. Any PR that changes a `pub` field in `crates/tsv_*/src/ast/public*` must also update the `.d.ts`. Drift is caught by `deno task check:ast-types` (part of `deno task check`) and reviewed at PR time.

See ./crates/tsv_wasm/CLAUDE.md §TS type maintenance for the per-field checklist.

### Corpus Comparison

Compare formatting against Prettier, and parse output against the canonical
parsers, on real codebases:

```bash
deno task corpus:compare:format ~/dev/some-project  # single project (or --all for default corpus repos in ~/dev)
# Options: --explain (show patterns matched), --summary (compact, no diffs),
#          --json (single JSON report to stdout: stats + safety/partial/unknown/error lists; logs → stderr)

deno task corpus:compare:parse --all   # deep-diff parse ASTs vs acorn-typescript/svelte/parseCss
# Options: --multibyte-only (offset-translation slice), --filter <lang>, --limit <n>, --json

deno task divergence:audit         # audit divergence pattern coverage (--json for machine-readable)
```

The corpus comparison builds with `--profile corpus` (release + `panic = "unwind"`) so panics in our code are caught and reported as errors instead of crashing the process. Benchmarks use `--release` (with `panic = "abort"`) for maximum performance.

Divergence detection identifies known differences documented in `conformance_prettier.md` (safety checks, pattern detection, traceability). See ./benches/js/CLAUDE.md and ./docs/divergence_detector.md.

### Benchmarks

**Cross-runtime.** The same harness runs under **Deno, Node, and Bun** — each emits its
own runtime-labeled sibling report (`report.{deno,node,bun}.{json,md}`), never merged;
`deno task bench:compose` folds them into a compact combined `report.{json,md}` (the
cross-runtime view tsv.fuz.dev consumes). The native row differs by runtime: Deno loads the
**FFI** library via `Deno.dlopen`, Node/Bun load the **N-API** addon (`tsv_napi`) via
`process.dlopen`. Everything else (corpus, registry, timing, report) is runtime-neutral
shared code using `node:` builtins.

```bash
# One-time: install the harness's npm deps (package.json is the source of truth;
# both runtimes consume the same node_modules). Re-run after a dep bump or a plain
# `npm install` (which prunes the oxc-parser-wasm binding — see benches/js/CLAUDE.md).
deno task bench:install

# Smoke test (Deno; fast sanity check that every formatter+parser produces output)
deno task smoke

# Run benchmarks (builds the runtime's bench artifacts automatically).
# `bench` runs ALL three and FAILS FAST if node or bun is missing — Deno is the
# only hard dep, so without node/bun run the per-runtime tasks you have instead.
deno task bench         # ALL three + compose (needs node AND bun installed)
deno task bench:deno    # Deno only (no node/bun needed)
deno task bench:node    # Node only (needs node)
deno task bench:bun     # Bun only (needs bun; reuses the Node artifacts)
deno task bench:compose # Fold existing per-runtime reports → combined report.{json,md}

# Run without rebuilding (if already built; aborts on stale artifacts)
deno task bench:deno:run
deno task bench:node:run
deno task bench:bun:run

# Per-file skip detail (off by default — counts always shown, paths/errors opt-in)
deno task bench:deno:run -- --verbose

# Environment variables (apply to any runtime)
BENCH_LIMIT=10 deno task bench:deno:run        # Limit files per language (default: all)
BENCH_FILTER=zzz deno task bench:deno:run      # Filter by path pattern (default: none)
BENCH_DURATION=10000 deno task bench:deno:run  # Duration per benchmark in ms (default: 5000)
BENCH_WARMUP=10 deno task bench:deno:run       # Set warmup iterations (default: 3)
BENCH_MODE=union deno task bench:deno:run      # Per-impl iteration (default: intersection)
BENCH_STALE_OK=1 deno task bench:deno:run      # Run despite stale artifacts (default: off)
BENCH_FORCED_ASYNC=1 deno task bench:deno:run  # Add tsv-forced-async control row (diagnostic; default: off)
```

**Prerequisites**: `cargo install wasm-pack` and `deno task bench:install` once
(the install needs `npm`, i.e. Node, to populate `node_modules`). Beyond that,
**Deno is the only hard dependency** — `bench:deno` / `smoke` run with just Deno.
Node ≥ 22.18 (native TS type-stripping) is needed for `bench:node` (and the
install); Bun for `bench:bun`. The aggregate `deno task bench` needs both and
fails fast if either is missing; run the per-runtime tasks for the ones you have.

Compares: canonical (prettier + svelte/compiler), native (FFI under Deno / N-API under Node
and Bun), WASM, and alternatives (oxc-parser, oxfmt, biome-wasm). Each runtime's results save
to `benches/js/results/report.<runtime>.{json,md}` (committed; every row carries a `runtime`
field), plus the combined `report.{json,md}`. To publish to tsv.fuz.dev: `npm run update-benchmarks` in ~/dev/tsv.fuz.dev. See
./benches/js/CLAUDE.md.

### Performance Profiling

```bash
cargo run --release -p tsv_debug -- profile ~/dev/zzz/src/lib        # profile a directory
cargo run --release -p tsv_debug -- profile file.ts --iterations 20  # more iterations
# Also: --json (machine-readable)

cargo run --release -p tsv_debug -- json_profile ~/dev/zzz/src/lib   # parse→JSON materialization sub-steps
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

**The one carve-out is file *scope*, not style.** `tsv format`'s directory discovery is gitignore-aware, with two regimes keyed on `.git`. The **format root** (the scope boundary, derived from the argument — the cwd never participates) is, **inside a git repo**, the repo root: a hard stop where the upward walk ends, so nothing above the repo is read and `tsv format --check` is reproducible across machines (when the ignore files are committed — a local/uncommitted `.formatignore`/`.prettierignore`, or git's unread `.git/info/exclude` / `core.excludesFile`, makes a clean CI checkout disagree). **Outside a git repo**, it's the filesystem root.

Inside a repo, discovery honors, relative to the repo root:

- **`.gitignore`**, hierarchically and repo-rooted exactly like git ([gitignore(5) syntax](https://git-scm.com/docs/gitignore#_pattern_format); matched against `git check-ignore` on case-sensitive filesystems);
- **`.formatignore`** (tsv's native file), **hierarchically** — one per directory from the repo root down, deeper wins — applied after `.gitignore`, so its `!` can re-include a gitignore'd path (subject to git's parent-directory rule);
- a single repo-root **`.prettierignore`** (drop-in compat; a repo-root `.formatignore` shadows it), never hierarchical;
- always-skipped **safety nets**: `.git`, `node_modules`, `.hg`, `.svn`, `.jj`.

Outside a repo, `.gitignore` and `.prettierignore` are **not read** (matching git, which honors `.gitignore` only in a worktree); `.formatignore` is honored hierarchically from the filesystem root down, so a `~/.formatignore` acts as global config for loose files. Because the boundary is derived by walking up, the repo-root ignore files apply even from a subdirectory invocation, and formatting a subdirectory directly gives the same result as formatting it via an ancestor.

When a `.gitignore` is in scope it is authoritative and the built-in **heuristic is off**; with no `.gitignore` (outside a repo, or a repo without one) the heuristic — hidden directories plus `dist`/`build`/`target` — is the fallback "not source" guess, except that an explicit tsv-layer `!` re-include overrides it. This is *only* about which files are reformatted, never how any file is formatted; it does not reopen style configuration. An explicitly named file argument is always formatted (the ignore files govern directory discovery). The matcher lives in the `tsv_ignore` crate (`IgnoreStack`); the per-directory prune *decision* layered on it (build-output heuristic, safety nets, the heuristic-shadow warning) lives in the `tsv_discover` crate. Both are shared with the JS CLI and the VS Code extension via the `IgnoreStack` WASM export (the matcher as the class, the policy as its `classify_dir`/`should_format_file`/`heuristic_shadow_warning` methods, plus `is_path_pruned` — a per-file form of the prune verdict for the extension, which has no top-down walk) — so all three surfaces agree by construction, not by hand-mirrored logic.

The list below covers the settings that diverge from Prettier's defaults; everything else (e.g. tabWidth=2) matches Prettier.

- `printWidth` (100) — Wider than Prettier's default of 80
- `useTabs` (true) — Tabs, not spaces (Prettier defaults to off)
- `singleQuote` (true) — Single quotes, not double (Prettier off)
- `trailingComma` ('none') — No trailing comma on multiline lists; differs from Prettier's `'all'`

`trailingComma: 'none'`: no trailing comma is emitted even when a list breaks across lines. With `useTabs` and `singleQuote`, this matches the Svelte project's own Prettier config (`.prettierrc`).

**Measuring line widths**: Use `cargo run -p tsv_debug line_width <file>` to measure visual width of lines (accounts for tabWidth=2). Never use `wc -c` — it counts bytes, not visual characters (tabs are 1 byte but 2 visual chars). The `compare` command also shows line widths on changed lines.

### Internal Configuration (Rust Library Only)

There is no runtime configuration. Print width / tab width / indent are compile-time `pub const`s in `tsv_lang::config` (`PRINT_WIDTH`, `TAB_WIDTH`, `INDENT`), read directly by the renderer — not threaded through any signature. Quote preference is likewise hardcoded (single quotes) inside `tsv_lang::printing::format_string_literal`. The doc-builder unit tests exercise the layout at smaller widths via the internal `RenderConfig` seam (`doc::render_config`, `pub(crate)`), never at runtime.

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
│   ├── tsv_ts/      # TypeScript: parse(), format(), convert_ast()
│   ├── tsv_css/     # CSS: parse(), format(), convert_ast()
│   ├── tsv_svelte/  # Svelte: parse(), format(), convert_ast()
│   ├── tsv_cli/     # Production CLI (binary: tsv) - pure Rust
│   ├── tsv_debug/   # Dev utilities (binary: tsv_debug) - uses Deno
│   ├── tsv_ffi/     # C FFI bindings (Deno's native path)
│   ├── tsv_wasm/    # WebAssembly bindings (published as @fuzdev/tsv_format_wasm + @fuzdev/tsv_parse_wasm + @fuzdev/tsv_wasm; bundles hand-maintained types/tsv_ast.d.ts; npm/cli.js is the tsv bin)
│   └── tsv_napi/    # N-API bindings (Node/Bun native path; measurement-only for the Node bench, 0.2 publish target)
├── scripts/         # Publish orchestrator, npm package patcher, Node artifact + N-API addon tests, AST type drift check
├── tests/           # Integration tests (parser, formatter, CLI)
│   └── fixtures/    # Test fixtures organized by language/feature
└── docs/            # Documentation (fixtures, cli, architecture, etc.)
```

**Crate pattern** (tsv_ts, tsv_css, tsv_svelte):

- `lib.rs` - Public API: `parse()`, `format()`, `convert_ast()`
- `ast/` - Internal AST, public AST types, conversion layer
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

Separately, a few spots where tsv still matches a Prettier relocation that genuinely
moves a comment across a semantic boundary (the method/call/construct-signature `(`,
the union-member alignment rendering, the `with {…}` import-attribute brace) are
un-converted implementation gaps being closed incrementally. When a fix changes
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

**Core Invariant**: Input file **always formats to itself** (idempotent) - no exceptions

**Directory Hierarchy**: Each fixture directory has either an input file (fixture) or subdirectories (container), not both, not neither.

**Fixture Organization Policy**: Organize by feature. Comment fixtures belong with the feature they test (e.g., `calls/chained/*_comment`), not centralized. Use `syntax/comments/` only for basic comment syntax, universal formatting rules, and cross-cutting edge cases.

**Input File Types:**

- `input.svelte` (preferred) - Tests code embedded in Svelte context
- `input.ts` (rare) - Only for byte-0 file-level features (hashbang, BOM) or constructs that format differently between contexts (JSDoc cast paren stripping). TS-only _syntax_ (`import =`, `export =`, types, decorators, `declare`) still uses `.svelte` with `lang="ts"`
- `input.css` (rare) - Only for file-level CSS features (e.g., BOM at byte 0)
- `input.svelte.ts` (runes) - Svelte rune modules (`$state`, `$derived`, etc.)

⚠️ **Prefer `.svelte`**: For CSS, it's the only path with an external canonical source. See ./docs/fixture_overview.md#why-svelte-is-the-default-canonical-source.

**Fixture File Structure:**

```
tests/fixtures/example_fixture/
├── input.svelte                    # Canonical source (ALWAYS formats to itself)
├── expected.json                   # AST from parsing input.svelte
├── expected_ours.json              # OPTIONAL: Our parser's AST (when intentionally different)
├── expected_svelte.json            # OPTIONAL: Svelte's AST (documents the difference)
├── output_prettier.svelte          # OPTIONAL: Prettier's output (when different from input)
├── prettier_variant_*.svelte         # OPTIONAL: Prettier's stable variants our formatter normalizes to input
├── variant_*.svelte        # OPTIONAL: Dual-stable forms (both formatters keep stable, NOT input)
├── prettier_intermediate_*.svelte  # OPTIONAL: Prettier's unstable first-pass output (converges to input)
├── prettier_intermediate_to_variant_*.svelte  # OPTIONAL: Prettier's unstable first-pass output (converges to a variant_*/prettier_variant_*)
├── audit_signature.txt             # OPTIONAL: Auto-generated; pins prettier's multi-pass chain from output_prettier.* (F4)
├── prettier_nonconvergent.txt      # OPTIONAL: Prettier never reaches a fixed point on input — no oracle; claim live-verified (F5)
├── prettier_rejects.txt            # OPTIONAL: Prettier throws on input (parse rejection / printer crash) — no oracle; trimmed content is the expected-error substring, claim live-verified (F6)
├── unformatted_*.svelte            # OPTIONAL: Variants that normalize to input.svelte (both formatters)
├── unformatted_ours_*.svelte       # OPTIONAL: Variants that normalize to input.svelte (our formatter only)
├── unformatted_prettier_*.svelte   # OPTIONAL: Variants that normalize to output_prettier.svelte (prettier only)
└── input_invalid_*.svelte          # OPTIONAL: Invalid syntax that must fail to parse (both parsers)
```

**Other file types** (same structure): `.ts`/`.svelte.ts` use acorn-typescript for parsing; `.css` uses Svelte's `parseCss`. All use prettier for formatting.

**Unformatted variant rules:** Same content structure as input, only whitespace differs. Both formatters must normalize to exactly match input.

**Invalid syntax rules (`input_invalid_*`):** Must fail BOTH parsers. One syntax error per file.

**Quick Pattern Selection:**

- **Parser matches Svelte**: `input.svelte` + `expected.json`
- **Parser differs intentionally**: Add `expected_ours.json` + `expected_svelte.json` (requires `_svelte_divergence` suffix)
- **Formatter matches prettier**: Add `unformatted_*.*` variants
- **Formatter differs intentionally**: Add `output_prettier.*` (requires `_prettier_divergence` suffix)
- **Prettier has stable variants (ours normalizes)**: Add `prettier_variant_*.*` files (requires `_prettier_divergence` suffix)
- **Dual-stable forms (both keep stable)**: Add `variant_*.*` files (requires `_prettier_divergence` suffix)
- **Normalization to input divergence**: `unformatted_ours_*.*` normalizes to input with our formatter only
- **Normalization to output_prettier**: `unformatted_prettier_*.*` normalizes to `output_prettier.*` with prettier
- **Prettier never converges (no oracle)**: Add `prettier_nonconvergent.txt` + README (requires `_prettier_divergence` suffix; excludes all prettier-claim files)
- **Prettier rejects/throws on input (no oracle)**: Add `prettier_rejects.txt` (trimmed content = expected-error substring) + README (requires `_prettier_divergence` suffix; excludes all prettier-claim files; mutually exclusive with `prettier_nonconvergent.txt`)
- **Both differ**: Use `_svelte_prettier_divergence` suffix

**Example Workflow: Handling a Prettier Difference**

```bash
cargo run -p tsv_debug compare <fixture>/input.svelte  # 1. Discover difference
mkdir <fixture>_prettier_divergence && cp input.svelte # 2. Create divergence dir
deno task fixtures:update:formatted <pattern>          # 3. Generate output_prettier.svelte
# 4. Add prettier_variant_*.svelte and unformatted_ours_*.svelte as needed
deno task fixtures:update:parsed <pattern>             # 5. Generate expected.json
deno task fixtures:validate <pattern>                  # 6. Validate
```

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
# forms like block-style inline content (<small>⏎\ttext⏎</small> vs <small>text</small>) match even
# though the parser keeps boundary whitespace verbatim. Sound: real content / <pre> / presence-of-space
# changes still differ. Use to confirm block-style render-equivalence at corpus scale.

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
cargo run -p tsv_debug fixtures_update_formatted  # output_prettier.svelte (auto-deletes if identical to input; skips prettier_nonconvergent.txt / prettier_rejects.txt fixtures — prettier can't format them)

# fixtures_audit - investigate normalization graphs (diagnostic; --all for every fixture, not just divergence)
cargo run -p tsv_debug fixtures_audit [pattern...]
# Also: --verbose (full graph), --json

# ts_fixture_audit - verify which input.ts fixtures genuinely need .ts vs. could be .svelte.
# Embeds EVERY .ts file (input + variants) in <script lang="ts"> and checks (tsv AND prettier)
# whether it formats identically. Necessary = byte-0 feature, Svelte-parse-fail, or
# formats-differently (often in a variant); else convertible. Convertible = formatting-safe only,
# not a mandate (a fixture may be .ts on purpose to cover the standalone tsv_ts/acorn path).
# Intentional = in the INTENTIONAL_TS allowlist (kept .ts on purpose; reported separately so the
# convertible list stays limited to fixtures genuinely free to move).
cargo run -p tsv_debug ts_fixture_audit [pattern...]
# Also: --verbose (show the TS-vs-Svelte diff on 'formats differently' fixtures)

# conformance_audit - doc/fixture integrity in one fixture walk (reads each file once):
#  (1) Orphans - every divergence-suffixed fixture linked in its conformance doc:
#      _prettier_divergence → docs/conformance_prettier.md, _svelte_divergence →
#      docs/conformance_svelte.md (_svelte_prettier_divergence in both). The suffix asserts a
#      deliberate difference; that claim must be cataloged so it's sanctioned and discoverable.
#  (2) Dead links - every Markdown link (relative path + #anchor) in both conformance docs and
#      every fixture README resolves on disk. The reverse direction of (1): a link to a
#      renamed/demoted/deleted fixture, or a back-link with the wrong ../ depth or stale anchor,
#      is otherwise invisible (the orphan check only proves live-fixture → mentioned-in-doc).
#  (3) Missing back-links - every divergence fixture's README must *contain* a link resolving to
#      its sanctioning doc (_prettier_divergence → conformance_prettier.md, _svelte_divergence →
#      conformance_svelte.md, both for the combined suffix). (1)+(2) prove the doc catalogs the
#      fixture and that any link present resolves, but neither requires the back-link to exist — a
#      README omitting it passes both. A missing README entirely is the validator's D1 rule.
#  (4) Stray READMEs - a non-divergence fixture (matches both tools) shouldn't carry a README;
#      deliberate exceptions live in the in-code ALLOWED_NONDIVERGENCE_READMES allowlist.
# Pure Rust (no Deno). Exits non-zero on any finding. Gated in `deno task check`.
cargo run -p tsv_debug conformance_audit
# Also: --json (machine-readable: {orphans, dead_links, missing_backlinks, stray_readmes})
```

> **Troubleshooting:** See ./docs/fixture_overview.md#quick-decision-tree

**test262 ECMAScript Conformance Tests:**

```bash
# test262 - run ECMAScript conformance tests against our parser (pure Rust, no Deno)
cargo run -p tsv_debug test262                       # run all tests (expects ../test262)
cargo run -p tsv_debug test262 language/expressions  # filter by path pattern
# Options: --path <dir>, --list, --verbose (show all failures), --negative-only, --positive-only

# Differential conformance (tsv vs oxc-parser) — emit a JSON manifest of the
# graded strict subset, then run the Deno consumer to bucket the agreement and
# triage tsv's failures (real bug vs shared limitation). See ./docs/conformance_test262.md §Differential.
cargo run -p tsv_debug test262 --emit-manifest /tmp/t262.json   # path/expected/tsv verdict per graded test
deno run --allow-read --allow-env --allow-ffi --allow-net --allow-sys \
  --config benches/js/deno.json \
  benches/js/diagnostics/test262_compare.ts --manifest /tmp/t262.json
```

See ./docs/conformance_test262.md.

**Performance Profiling Commands:**

```bash
# profile - measure parse vs format phase timing (pure Rust, no Deno needed)
cargo run -p tsv_debug profile ~/dev/zzz/src/lib      # profile a directory
cargo run -p tsv_debug profile file1.ts file2.svelte  # profile specific files
# Options: --iterations <n> (default: 10), --json

# json_profile - split the FFI parse path (parse + convert_ast_json_string)
# into materialization sub-steps with per-file byte-identity checks (pure
# Rust, no Deno; run with --release). See ./docs/performance.md.
cargo run --release -p tsv_debug -- json_profile ~/dev/zzz/src/lib
# Options: --iterations <n> (default: 5), --json (adds per-file data)

# buffer_sizes - AST histograms for tuning the TS printer's SmallVec inline
# capacities (named_specs, CommentLines) + sizing the future multiline-text doc
# node. Two metrics: named-import-specifier count per import, and line count per
# multi-line block comment. Covers .ts/.svelte.ts AND .svelte (the <script>/{expr}
# feed the same TS-printer buffers). Pure Rust, no Deno. Prints percentiles +
# spill rate at candidate inline N. For sizing, exclude the prettier/svelte test
# suites (edge-case skew). See ./docs/performance.md.
cargo run -p tsv_debug buffer_sizes ~/dev/zzz/src ~/dev/gro/src
# Options: --json
```

See ./docs/performance.md.

**Codebase Metrics Commands:**

```bash
# metrics - codebase structure analysis (pure Rust, no Deno needed)
cargo run -p tsv_debug metrics             # line counts by crate and phase (lexer/parser/ast/printer)
cargo run -p tsv_debug metrics --json      # JSON output for scripting
deno task metrics                          # shorthand
```

**Line-Comment Swallow Audit:**

```bash
# swallow_audit - format files with the render-time swallow check on and report
# any `//` line comment followed by content on the same output line (silent
# content loss). Pure Rust, no Deno. Defaults to tests/fixtures; pass dirs/files
# to audit real code. Exits 1 on any finding.
cargo run -p tsv_debug swallow_audit                 # audit all fixtures
cargo run -p tsv_debug swallow_audit ~/dev/zzz/src   # audit a real codebase
# Also: --json. The check lives in tsv_lang::doc::swallow, behind the
# `swallow_check` cargo feature (off by default → compiled out of prod
# wasm/cli/ffi; tsv_debug enables it). Gated in `deno task check` (via the
# `swallow:audit` task) over tests/fixtures.
```

**Build-Fanout Audit (exponential-rebuild regression guard):**

```bash
# build_fanout_audit - guard the O(1)-doc-builds-per-source-node invariant. A
# builder that assembles `conditional_group` candidates (flat vs expanded,
# inline vs multiline) by RE-INVOKING the recursive builder on the same nodes —
# instead of building the subtree once and reusing the DocId — makes the doc-node
# count grow exponentially in nesting depth (the formatter can hang/OOM on a
# deeply-nested but ordinary file). This builds synthetic nested inputs (block
# elements, {#if} blocks, member chains) at increasing depth, formats each into a
# fresh DocArena via `format_in`, and fails if the doc-node count (read straight
# from `arena.borrow_nodes().len()` — no prod instrumentation) grows faster than
# ~depth^3. Deterministic, pure Rust, no Deno. Exits 1 on any super-linear case.
cargo run -p tsv_debug build_fanout_audit
# Also: --json. Gated in `deno task check` via the `fanout:audit` task. Green: all
# six axes (svelte elements / {#if} / {#each} / {#await} / sibling-`>` dangle, ts
# member chains) build O(1) docs per node, so the audit holds the line against a
# reintroduced per-candidate rebuild.
```

**Raw-Find Scan Audit (delimiter-scan regression guard):**

```bash
# scan_audit - guard against new raw position-anchoring substring scans over
# source. A raw `self.source[..].find(delim)` can match the glyph inside an
# enclosed comment/string and drop content (the "Comment-Aware Delimiter Scans"
# bug class); the fix is the trivia-aware cursor (`tsv_lang::source_scan`).
# This audit flags every `find`/`rfind`/`match_indices`/`rmatch_indices`
# (non-closure pattern) in the four language crates and fails on any not in the
# reviewed in-code allow-list (ALLOW, each entry categorized: comment-marker /
# newline / css-value / at-rule-range / delimiter-latent / delimiter-deferred-bug
# / …). A new scan must move onto the cursor or be consciously allow-listed; a
# migrated/reformatted scan must drop its now-stale entry (the list mirrors the
# live sites exactly). Pure Rust, no Deno.
cargo run -p tsv_debug scan_audit            # audit (exit 1 on any violation/stale)
cargo run -p tsv_debug scan_audit --list     # enumerate every scan site
# Also: --json. Gated in `deno task check` via the `scan:audit` task. Out of
# scope: closure `.find(|…|)`/`.match_indices(|…|)` (iterator/predicate), counting
# (`.matches(c).count()`) and existence (`contains`/`starts_with`) checks, and
# hand byte-loops (the cursor is their sanctioned home).
```

**Authoring-Independence Audit (Svelte boundary whitespace):**

```bash
# authoring_audit - probe whether the SAME logical document, authored with
# different boundary whitespace, formats to ONE tsv fixed point. Stronger than
# the corpus idempotency sweep (which only checks format(x) is stable): a
# formatter can be idempotent yet authoring-DEPENDENT (two authorings settling
# on two different stable outputs). Mutates only non-significant boundary
# whitespace — an existing run between two siblings (a whitespace-only Text node
# or a content Text node's boundary whitespace), space↔single-newline, never a
# blank line, never inside <pre>/<textarea> (via tsv_html::preserves_whitespace).
# Safe by construction (HTML whitespace collapse); the element expansion a toggle
# may trigger is the property under test. Svelte (.svelte) only for now.
cargo run -p tsv_debug authoring_audit                  # audit tests/fixtures (pure Rust)
cargo run -p tsv_debug authoring_audit ~/dev/zzz/src    # audit a real codebase
# Pure-Rust verdict per site: converge / diverge (dual-stable) / diverge
# (NON-IDEMPOTENT). Exits 1 on any non-idempotency. With --prettier it adds the
# triage via the sidecar: (a) tsv diverges where prettier converges (bug);
# (b) tsv converges where prettier diverges (a _prettier_divergence to pin, the
# space_after_block class); (c) both diverge (sanctioned, e.g. Tier-2 element
# expansion). --dump-dir writes byte-exact repro artifacts (base/variant/ftry/
# ftry2) for each hard finding — the basis for a fixtures-first fix.
# Also: --json, --verbose, --limit N (sites/file), --examples N.
cargo run -p tsv_debug authoring_audit ~/dev/zzz/src --prettier --dump-dir /tmp/audit
```

## Architectural Notes

### Closed Scope, Open Convention

tsv ships a closed language set (TypeScript, CSS, Svelte) but is open by
convention **at the Rust source/crate level**: each language crate
(`tsv_ts`, `tsv_css`, `tsv_svelte`) is self-contained — owns its internal
AST, public AST, parser, formatter, and convert layer — and exposes the same
free-function API (`parse()`, `format()`, `convert_ast()`,
`convert_ast_json()`, `convert_ast_json_string()`). **No central `Language`
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

### AST Architecture: Internal vs Public

Drop-in replacement for Svelte's **public JSON AST**, NOT its internal implementation.

- **Internal AST**: Clean, semantic representation (decoded strings, normalized values) for all tools
- **Public AST**: Conversion layer matching Svelte's exact JSON output, quirks applied at boundary only

**Example**:

```rust
// Internal - clean and semantic
struct Literal {
    value: LiteralValue,  // Fully decoded: "test\n" → "test<newline>"
    span: Span,
}

// Public conversion - applies Svelte quirks
fn to_json(lit: &Literal, source: &str) -> Value {
    json!({
        "value": lit.value,
        "raw": &source[lit.span.range()],  // Reconstruct from source
    })
}
```

**Key Rules**:

- Raw strings NEVER duplicated in AST (extract via `source[span.range()]`)
- Internal ASTs NEVER serialized (no `serde::Serialize`; only public types use serde)

### Position Types: u32 vs usize

- **Span**: `u32` for start/end (8 bytes total, 50% memory savings vs usize)
- **`Token`**: `u32` for start/end — `Token` is a 16-byte `Copy`-free POD (`{kind, start: u32, end: u32}`) returned from `next_token` in registers; the decoded value (escapes only) lives out-of-band on the lexer (`Lexer::take_decoded`). A `const _: () = assert!(size_of::<Token>() == 16)` guards the size. See [docs/performance.md] and the lexer's byte cursor (`bytes: &[u8]` + `position: usize`).
- **Lexer/Parser positions**: `usize` (natural for `source[pos]` indexing); the lexer dispatches on raw bytes (`cur_byte`) and decodes a `char` only at non-ASCII branches.
- **Conversions**: At boundaries only - `as u32` when creating Spans / `Token` fields, `as usize` when extracting
- **Helpers**: Use `span.extract(source)` or `span.range()` instead of manual casts

### Comment Handling: Detached Model

Comments are stored **separately from AST nodes** in a flat `Vec<Comment>` at the root level (`Program.comments`, `CssStyleSheet.comments`, `Root.comments`). The printer finds comments via O(log n) binary search on span positions.

**Core type** (`tsv_lang/src/comment.rs`):

```rust
pub struct Comment {
    pub content_span: Span,        // content WITHOUT delimiters; text via content(source)
    pub is_block: bool,
    pub multiline: bool,           // content contains '\n' (precomputed; block-only in practice)
    pub span: Span,                // full comment span, delimiters included
    pub emit_character_field: bool, // Serializer hint: include `character` in JSON loc
}
```

The content is **not stored owned** — comment text is a pure delimiter-stripped
sub-slice of source (no decoding for JS/TS/CSS comments), so `Comment` holds a
`content_span` and recovers the text on demand via `Comment::content(source) -> &str`
(`source` must be the host document the spans were recorded against). This avoids a
`String` allocation per comment in the lexer and the parser's collect-clone; every
field is `Copy`. `multiline` is precomputed so the multi-line-block expansion checks
(`has_multiline_block_comments_in_range` and the printers) stay O(1) and source-free.
The full comment span includes its delimiters (`//` / `/* */` / a `#!` hashbang,
whose content includes the `#!`); the lexer is the single owner of those widths.

**Printer strategy**: Range-based lookup via `comments_in_range(prev_end, node_start)`. Source string for context (same-line detection, blank line preservation). Tradeoff: simple/efficient AST matching Prettier's model, but printer must manually track `prev_end` positions; edge cases (e.g., arrow function comments) require careful span math.

Higher-fidelity models (attached comments, trivia tokens) may be needed for IDE/linter use cases.

## Dependencies

### Rust Crates (minimal deps)

- `serde`, `serde_json` — Public AST serialization
- `smallvec` — Stack-allocated vectors
- `string-interner` — String interning for AST symbols
- `thiserror` — Error type derivation
- `phf` — Compile-time perfect hash maps (keywords, entities)
- `unicode-ident` — Unicode XID_Start/XID_Continue for identifiers
- `unicode-segmentation` — Grapheme clustering for visual width measurement
- `unicode-width` — Character display width (CJK, zero-width)
- `bumpalo` — Bump arena for the internal AST (and, via the `tsv_arena` crate, the bindings' per-thread `reset()` reuse — `tsv_ffi`/`tsv_napi`/`tsv_wasm`)
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
