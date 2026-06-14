# tsv

> a formatter, parser, and future linter + more for Svelte, TypeScript, and CSS

High-performance Rust parser as a drop-in replacement for Svelte's modern parser (acorn + acorn-typescript) with a near-Prettier formatter that tracks Prettier closely.

**Non-configurable by design**: formatting options are fixed at Prettier's defaults except print_width=100, use_tabs=true, and bracketSpacing=false — no config files, CLI flags, or runtime options, ever (opinionated like `gofmt` and Black). See [Configuration](#configuration).

## Committing

`git add` and `git commit` are denied by `.claude/settings.local.json` in
this repo — make the edits and stop, the user commits.

## Priorities

1. **Correctness**: Match Svelte's parser and Prettier's formatter exactly. Fixtures are the source of truth for correct behavior - when tests fail, fix the code.
2. **Performance**: Pure Rust for speed. Embedded Deno sidecar (dev tooling) is orders of magnitude faster than spawning a process per call.

## Development Philosophy: Test-Driven Development with Fixtures

**ALWAYS use TDD when implementing features or fixing bugs:**

0. **Load context FIRST** - Read BOTH ./docs/fixture_workflow.md AND ./docs/fixture_naming.md into context.
   For a `_prettier_divergence` fixture, ALSO read ./docs/conformance_prettier.md first (§Comment Position
   Philosophy + §Comment relocation catalog) — the divergence must be sanctioned and cataloged there.
   Study 2-3 existing fixtures in the target category. No code changes without a failing fixture.
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
deno task build:ffi        # C FFI library
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

`format` writes paths **in place** (only when output differs) and prints changed paths to stdout; `--content`/`--stdin` print formatted source to stdout. Directories recurse over `.ts`/`.svelte`/`.css`, skipping hidden directories (`.git`, `.svelte-kit`, …) and `node_modules`/`dist`/`build`/`target`. Files format in parallel (`--jobs N` overrides the thread count; path mode only). Exit codes: 0 clean, 1 would-change (`--check`, which also works with `--content`/`--stdin`), 2 errors; missing path args fail the run upfront, while per-file and traversal errors report and continue.

```bash
cargo run -p tsv_cli parse file.ts                                       # compact JSON
cargo run -p tsv_cli parse file.ts --pretty                              # formatted JSON
cargo run -p tsv_cli parse --content '<div>x</div>' --parser svelte      # parse string (preferred for agents)
cargo run -p tsv_cli parse --stdin --parser svelte                       # parse stdin (not preferred for agents)
cargo run -p tsv_cli format file.svelte src/lib                          # format files/dirs in place
cargo run -p tsv_cli format --check src/lib                              # list would-change files, exit 1 (CI)
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
deno task conformance:audit          # verify every divergence fixture is linked in its conformance doc (prettier + svelte; gated in `deno task check`)
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

Two binding crates for different use cases:

| Crate      | Technology   | Target                       | Output                                                            |
| ---------- | ------------ | ---------------------------- | ----------------------------------------------------------------- |
| `tsv_ffi`  | C ABI        | Any FFI (Deno, Python, etc.) | `libtsv_ffi.so` / `.dylib` / `.dll`                               |
| `tsv_wasm` | wasm-bindgen | Browser, Deno, Node          | `.wasm` module (format / parse / all variants via cargo features) |

N-API is a maybe — the decision is deferred; there is currently no `tsv_napi` crate.

`tsv_wasm` produces three npm packages from one crate via the `format` + `parse` cargo features (default = both): `@fuzdev/tsv_format_wasm` (format only), `@fuzdev/tsv_parse_wasm` (parse only), and `@fuzdev/tsv_wasm` (everything + the `tsv` CLI). Each variant has its own output directory.

```bash
# Build bindings
deno task build:ffi                  # C FFI → target/release/libtsv_ffi.so
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

`scripts/publish.ts` orchestrates the release end to end (preflight → bump → check → build (npm packages + deno bundles, so artifact validation never sees stale bundles) → verify → artifact validation: size bounds + Deno smoke + Node tests → idempotent npm publish → git commit + tag + push), printing a wasm size summary (raw + gzipped) at the end. It stamps CHANGELOG.md's `## Unreleased` section into the released version's section — that section must be non-empty and carry a `<!-- bump: <level> -->` marker that matches `--bump` (the bump is required in **both** places and they must agree; on stamp the marker is dropped and a fresh empty `## Unreleased` reset to `bump: patch` is seeded for the next cycle). Keep it updated as work lands. A failed wetrun is resumable: re-run `--wetrun` without `--bump`.

```bash
deno task publish                        # dry-run: validate everything, no mutation
deno task publish --wetrun --bump patch  # release: bump + publish + git finalize (--bump required, must match CHANGELOG marker)
deno task publish --wetrun               # resume a failed wetrun (sentinel retry only)
# Flags: --bump patch|minor|major, --no-check, --no-git
deno task test:npm[:parse|:all]          # Node tests against a built pkg/{format,parse,all}/npm/ (:all includes CLI tests)
deno task validate:artifacts             # tight wasm size bounds + Deno smoke of all built bundles
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

Divergence detection identifies known differences documented in `conformance_prettier.md` (safety checks, pattern detection, traceability). See ./benches/deno/CLAUDE.md and ./docs/divergence_detector.md.

### Benchmarks

```bash
# Smoke test (fast sanity check that every formatter+parser produces output)
deno task smoke

# Run benchmarks (builds ffi + wasm:deno automatically)
deno task bench

# Run without rebuilding (if already built). Aborts if the FFI/WASM artifacts
# are older than crate source — rebuild, or set BENCH_STALE_OK=1 to override
deno task bench:run

# Per-file skip detail (off by default — counts always shown, paths/errors opt-in)
deno task bench:run -- --verbose          # Include per-file skip detail in report

# Environment variables
BENCH_LIMIT=10 deno task bench:run        # Limit files per language (default: all)
BENCH_FILTER=zzz deno task bench:run      # Filter by path pattern (default: none)
BENCH_DURATION=10000 deno task bench:run  # Duration per benchmark in ms (default: 5000)
BENCH_WARMUP=10 deno task bench:run       # Set warmup iterations (default: 3)
BENCH_MODE=union deno task bench:run      # Per-impl iteration (default: intersection)
BENCH_STALE_OK=1 deno task bench:run      # Run despite stale artifacts (default: off)
```

**Prerequisites**: `cargo install wasm-pack`

Compares three implementations: canonical (prettier + svelte/compiler), native (FFI), WASM. Results are saved to `benches/deno/results/report.{json,md}` (committed). To publish to tsv.fuz.dev: `npm run update-benchmarks` in ~/dev/tsv.fuz.dev. See ./benches/deno/CLAUDE.md.

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

The table lists the settings that diverge from Prettier's defaults; everything else (e.g. tab_width=2) matches Prettier.

| Setting          | Value | Notes                                       |
| ---------------- | ----- | ------------------------------------------- |
| `print_width`    | 100   | Wider than Prettier's default of 80         |
| `use_tabs`       | true  | Tabs, not spaces (Prettier defaults to off) |
| `bracketSpacing` | false | No spaces inside `{ }` — discussion welcome |

**Measuring line widths**: Use `cargo run -p tsv_debug line_width <file>` to measure visual width of lines (accounts for tab_width=2). Never use `wc -c` — it counts bytes, not visual characters (tabs are 1 byte but 2 visual chars). The `compare` command also shows line widths on changed lines.

### Internal Configuration (Rust Library Only)

There is no runtime configuration. Print width / tab width / indent are compile-time `pub const`s in `tsv_lang::config` (`PRINT_WIDTH`, `TAB_WIDTH`, `INDENT`), read directly by the renderer — not threaded through any signature. Quote preference is likewise hardcoded (single quotes) inside `tsv_lang::printing::format_string_literal`. The doc-builder unit tests exercise the layout at smaller widths via the internal `RenderConfig` seam (`doc::render_config`, `pub(crate)`), never at runtime.

Two types carry genuine per-input *state* (not configuration), threaded only where they vary:

- `tsv_lang::EmbedContext { base_indent_offset, first_line_offset, suffix_width, mode: LayoutMode }` — embedding state for nested formatting (CSS in `<style>`, Svelte template expressions). `LayoutMode { Standalone, Embedded }` controls binary-expression indent style.
- `tsv_ts::TsContext { Standalone, Svelte }` — whether the TypeScript is standalone or embedded in a Svelte file. Derived from the file kind, not a user option; `TsContext::Svelte` enables `<T,>` arrow-type-param disambiguation. `tsv_svelte` passes it when embedding TS.

```rust
use tsv_ts::{TsContext, format, format_with_context};

let formatted = format(&ast, source);                                   // pure TS (Standalone)
let formatted = format_with_context(&ast, source, TsContext::Svelte);   // Svelte-embedded TS
```

## Project Structure

```
tsv/
├── crates/
│   ├── tsv_lang/    # Foundation (span, location, error, doc builder, printing utils)
│   ├── tsv_html/    # HTML element classification and whitespace rules
│   ├── tsv_ts/      # TypeScript: parse(), format(), convert_ast()
│   ├── tsv_css/     # CSS: parse(), format(), convert_ast()
│   ├── tsv_svelte/  # Svelte: parse(), format(), convert_ast()
│   ├── tsv_cli/     # Production CLI (binary: tsv) - pure Rust
│   ├── tsv_debug/   # Dev utilities (binary: tsv_debug) - uses Deno
│   ├── tsv_ffi/     # C FFI bindings
│   └── tsv_wasm/    # WebAssembly bindings (published as @fuzdev/tsv_format_wasm + @fuzdev/tsv_parse_wasm + @fuzdev/tsv_wasm; bundles hand-maintained types/tsv_ast.d.ts; npm/cli.js is the tsv bin)
├── scripts/         # Publish orchestrator, npm package patcher, Node artifact tests, AST type drift check
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

- Default: fix our formatter to match prettier
- Exception (spec precedence): when the spec defines a canonical form prettier doesn't emit, follow the spec — even if prettier's output is itself valid. Document with spec refs in a `_prettier_divergence`
- Exception: when prettier moves comments to different syntactic positions, preserve the user's placement. See ./docs/conformance_prettier.md#comment-position-philosophy
- `_prettier_divergence` suffix: rare, documented intentional differences only. Requires README. Never use to hide bugs.

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
# Line widths appear as right-aligned numbers on diff lines (helps spot print_width issues)
# "Outputs match" = ours(input) == prettier(input), NOT input stability; a match on a
# non-format-stable input adds a note + input-vs-formatted diff (F1 fails on such an input)

# ast_diff - verify semantic equivalence
cargo run -p tsv_debug ast_diff input.svelte                         # round-trip: parse → format → parse → compare
cargo run -p tsv_debug ast_diff input.svelte output_prettier.svelte  # compare two files' ASTs

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
cargo run -p tsv_debug fixtures_update_formatted  # output_prettier.svelte (auto-deletes if identical to input; skips prettier_nonconvergent.txt fixtures — no fixed point to record)

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

# conformance_audit - verify every divergence-suffixed fixture is linked in its conformance
# doc: _prettier_divergence → docs/conformance_prettier.md, _svelte_divergence →
# docs/conformance_svelte.md (_svelte_prettier_divergence must appear in both). The suffix
# asserts a deliberate difference from the canonical tool; that claim must be cataloged so
# the divergence is sanctioned and discoverable.
# Pure Rust (no Deno). Exits non-zero on any unlinked fixture. Gated in `deno task check`.
cargo run -p tsv_debug conformance_audit
# Also: --json (machine-readable: per-doc array of {doc, suffix, total, unlinked_count, unlinked})
```

> **Troubleshooting:** See ./docs/fixture_overview.md#quick-decision-tree

**test262 ECMAScript Conformance Tests:**

```bash
# test262 - run ECMAScript conformance tests against our parser (pure Rust, no Deno)
cargo run -p tsv_debug test262                       # run all tests (expects ../test262)
cargo run -p tsv_debug test262 language/expressions  # filter by path pattern
# Options: --path <dir>, --list, --verbose (show all failures), --negative-only, --positive-only
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
```

See ./docs/performance.md.

**Codebase Metrics Commands:**

```bash
# metrics - codebase structure analysis (pure Rust, no Deno needed)
cargo run -p tsv_debug metrics             # line counts by crate and phase (lexer/parser/ast/printer)
cargo run -p tsv_debug metrics --json      # JSON output for scripting
deno task metrics                          # shorthand
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

Unsupported sloppy-mode features: `with` statement, legacy octal literals (`010`), duplicate parameter names, reserved words as identifiers.

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
- **Lexer/Parser/Token**: `usize` (natural for `source[pos]` indexing)
- **Conversions**: At boundaries only - `as u32` when creating Spans, `as usize` when extracting
- **Helpers**: Use `span.extract(source)` or `span.range()` instead of manual casts

### Comment Handling: Detached Model

Comments are stored **separately from AST nodes** in a flat `Vec<Comment>` at the root level (`Program.comments`, `CssStyleSheet.comments`, `Root.comments`). The printer finds comments via O(log n) binary search on span positions.

**Core type** (`tsv_lang/src/comment.rs`):

```rust
pub struct Comment {
    pub content: String,           // WITHOUT delimiters (/* */ or // stripped)
    pub is_block: bool,
    pub span: Span,
    pub emit_character_field: bool, // Serializer hint: include `character` in JSON loc
}
```

**Printer strategy**: Range-based lookup via `comments_in_range(prev_end, node_start)`. Source string for context (same-line detection, blank line preservation). Tradeoff: simple/efficient AST matching Prettier's model, but printer must manually track `prev_end` positions; edge cases (e.g., arrow function comments) require careful span math.

Higher-fidelity models (attached comments, trivia tokens) may be needed for IDE/linter use cases.

## Dependencies

### Rust Crates (minimal deps)

| Crate                  | Purpose                                             |
| ---------------------- | --------------------------------------------------- |
| `serde`, `serde_json`  | Public AST serialization                            |
| `smallvec`             | Stack-allocated vectors                             |
| `string-interner`      | String interning for AST symbols                    |
| `thiserror`            | Error type derivation                               |
| `phf`                  | Compile-time perfect hash maps (keywords, entities) |
| `unicode-ident`        | Unicode XID_Start/XID_Continue for identifiers      |
| `unicode-segmentation` | Grapheme clustering for visual width measurement    |
| `unicode-width`        | Character display width (CJK, zero-width)           |

## Canonical References

**Implementations** (versions pinned in `crates/tsv_debug/src/deno/sidecar.ts`):

| Implementation  | Local          | Purpose                                             |
| --------------- | -------------- | --------------------------------------------------- |
| Prettier        | `../prettier/` | Formatting reference — read source for layout logic |
| Svelte compiler | `../svelte/`   | Parsing reference                                   |

**IMPORTANT**: Read `../prettier/` source code instead of searching the web when investigating
formatting behavior. Key files: `src/language-js/print/assignment.js` (assignment layout),
`src/language-js/print/call-arguments.js` (call arg expansion), `src/language-js/print/member-chain.js`
(chain formatting), `src/language-js/print/binaryish.js` (binary operators).

**Specs** — consult BEFORE implementing CSS/HTML/JS features (don't search the web):

| Spec       | Local              |
| ---------- | ------------------ |
| CSS        | `../csswg-drafts/` |
| HTML       | `../html/`         |
| DOM        | `../dom/`          |
| ECMAScript | `../ecma262/`      |
| test262    | `../test262/`      |
| Web data   | `../webref/`       |

**Workflow**: Read local spec → `canonical_parse` to test behavior → `compare` to check formatting.

## Development conventions

- **Leave `// TODO:` comments** - when there's known future work or the code smells
- **Do not use the `cd` Bash command** - instead of cd use relative paths from the cwd

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
