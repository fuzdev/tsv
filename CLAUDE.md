# tsv

> precise language tools for TypeScript/JS, CSS, and Svelte in Rust

High-performance Rust parser as a drop-in replacement for Svelte's modern parser (acorn + acorn-typescript), paired with a formatter that took Prettier as its initial guide and still tracks it for the common case — while making deliberate, cataloged choices to diverge where tsv's own judgment is more defensible.

**Non-configurable by design**: formatting options are fixed at Prettier's defaults except printWidth=100, useTabs=true, singleQuote=true, and trailingComma='none' — no config files, CLI flags, or runtime options, ever (opinionated like `gofmt` and Black). The one carve-out is file *scope*, not style: `tsv format` honors `.gitignore` (hierarchically, in a git tree) plus hierarchical `.formatignore` / `.prettierignore`. See [Configuration](#configuration).

## Committing

**Branch exception (`svelte-compiler` branch / the `~/dev/tsv-svelte-compiler`
worktree only — remove this block before merging to main):** agent commits
ARE permitted here. Commit at sensible stopping points with short 1-liner
messages (`fix:` / `feat:` / `docs:` / `test:` / `refactor:` / `chore:`
prefixes; no body, no trailers, no `Co-Authored-By`). Merging to main,
version bumps, and publishing remain user-owned.

On main and every other branch: `git add` and `git commit` are denied by
`.claude/settings.local.json` in this repo — make the edits and stop, the
user commits.

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
deno task fixtures:validate          # validate (use during fixture work; --prettier-only skips our parser/formatter)
deno task fixtures:update            # regenerate expected.json + output_prettier.svelte (source of truth)
deno task fixtures:update:parsed     # regenerate expected.json only (run when parser changes)
deno task fixtures:update:formatted  # regenerate output_prettier.svelte only
deno task fixtures:audit             # audit _prettier_divergence fixtures (diagnostic; --all for every fixture)
deno task conformance:audit          # doc/fixture integrity: divergence fixtures cataloged + every doc/README link resolves + each divergence README back-links its sanctioning doc + no stray READMEs on matching fixtures (gated in `deno task check`)
deno task conformance:audit:compiler # compile-fixture divergence integrity: any _compiled_divergence fixture must be cataloged in docs/conformance_svelte_compiler.md + carry a back-linking README — the catalog is expected to stay EMPTY (gated in `deno task check`); see Debug Tooling
deno task canonicalize:audit         # canonicalize_js idempotence + output validity + comment preservation (the COMMENT-LOSS bucket) over tests/fixtures + tests/fixtures_compile (pure Rust; gated in `deno task check`) — point the command at real corpora on demand; see Debug Tooling
deno task compile:fixtures:init      # create/reinit a compile fixture (oracle-compiles + canonicalizes; tests/fixtures_compile)
deno task compile:fixtures:validate  # validate compile fixtures: oracle freshness + expected idempotence + ours parity (all gating; the sidecar-free slice also gates in cargo test)
deno task compile:corpus:compare     # compile-parity wide net over real repos + the Svelte suites: per-file parity/refused/oracle-rejected buckets; MISMATCH = always a bug by the refusal contract (sidecar-dependent, on demand — kept out of `deno task check`); see Debug Tooling
deno task pins:audit                 # canonical-oracle version sync (gated in `deno task check`): (1) pin agreement — sidecar.ts VERSIONS + npm: imports, benches/js/package.json, actor.rs acorn import-map must be identical; (2) checkout alignment — a PRESENT ../svelte or ../acorn-typescript checkout must match its pin (absent → skipped, so clean machines pass)
deno task scan:audit                 # guard against new raw find/rfind/match_indices substring scans over source (gated in `deno task check`); see Debug Tooling
deno task fanout:audit               # guard against super-linear doc-node fanout (the per-layout-candidate rebuild blowup); gated in `deno task check`; see Debug Tooling
deno task roundtrip:audit            # cheap tripwire that format(tests/fixtures) reparses (pure-Rust phase 1, no *_unreparseable output; gated in `deno task check`) — real yield is external corpora; see Debug Tooling
deno task binding:audit              # comment↔token binding audit: does format re-bind a forward-binding glued block comment (a plain comment, a bundler annotation, or a JSDoc cast — all owned) to a different subtree — the class invisible to ast_diff/roundtrip/SAFETY because the characters only MOVE (pure Rust, no sidecar; gated in `deno task check`) — HARD (a parser-owned glued comment) fails the gate, SOFT (an unowned glued block comment, now rare) is informational; TS-family files only; real yield on external corpora; see Debug Tooling
deno task authoring:audit            # authoring-independence over Svelte boundary whitespace: every render-equivalent authoring of one document (hug ↔ space ↔ newline at a tag's content boundary; space ↔ newline between siblings) must reach ONE tsv fixed point (pure Rust, no sidecar; gated in `deno task check`) — exits 1 on any non-idempotency, site-level or a base-non-idempotent FILE; see Debug Tooling
deno task fuzz:audit                 # seeded mutational fuzzer over tests/fixtures (fixed --seed 0 --iterations 5000; pure Rust, no sidecar; gated in `deno task check`) — asserts no-panic + idempotency + structural-reparse, on every seed file AS AUTHORED and then on mutated input. Corpus-add-stable: mutants come from per-file path-keyed PRNG streams, so a fixture add/rename changes only that file's mutants; see Debug Tooling
deno task comments:audit             # print-once comment ledger: every comment a document PARSES must be EMITTED exactly once (pure Rust, no sidecar; gated in `deno task check`) — reports DROPPED (silent content loss) and DOUBLE-PRINTED; the structural guard on the detached comment model, tsv's `ensureAllCommentsPrinted`; see Debug Tooling
deno task gaps:audit                 # gap-injection audit: inject a comment into EVERY gap (five payloads, one per ownership path) and re-run the ledger — the discovery arm `comments:audit` can't be, since it only formats each file AS AUTHORED and no fixture covers most positions (eight such drops were found BY HAND, all green on every gate). Pure Rust, no sidecar; gated in `deno task check` as a RATCHET over a generated shape snapshot (`gap_audit_known.txt`): every line is a known bug and the file shrinking is the goal, so a shape not on the list, one on it that no longer fires, or any PANIC, FAILS. ~17 s. Full reference: ./docs/gap_audit.md
deno task gaps:audit:update          # regenerate that snapshot after fixing a shape (or when a new fixture merely REACHES a pre-existing one); refuses a narrowed run
deno task blanks:audit               # blank-line injection audit: inject a blank line into EVERY code gap and grade six policy-free invariants — no panic, F1 idempotency, structural reparse, leaf conservation, ledger-clean, and blank-run ≤ 1. Mechanizes the blank-line handling class (the specifier-list / array-pattern bugs). Pure Rust, no sidecar; gated in `deno task check` as a RATCHET over `blank_audit_known.txt` (like gaps:audit): a graded shape not on the list, one that no longer fires, or any PANIC, FAILS. STRUCTURAL-DIVERGENCE is held REPORT-ONLY (fuzz-soft parity: reported but never gated, filtered out of the ratchet); every other policy kind IS pinned. A fast path (a blank the formatter ABSORBS reproduces the proven-clean pristine output, so nothing is checked) keeps it near gaps:audit's cost. ~24 s. Full reference: ./docs/blank_audit.md
deno task blanks:audit:update        # regenerate that snapshot after fixing a shape; refuses a narrowed run
deno task idempotency:sweep          # F1 (idempotency) sweep over the real-code corpus (the `perf` view — sibling dev repos + upstream framework source). NOT in `deno task check`: machine-dependent corpus, minutes not seconds. Run at conformance cadence or after a printer change; see Debug Tooling
deno task audit:corpus               # the standing content-loss / robustness gate over REAL code (the extension-robustness bar `deno task check`'s fixture-only scope is structurally blind to): roundtrip_audit --gate + comment_audit + binding_audit --gate (real gating; prettier suites report-only) + authoring_audit + fuzz --iterations 0, over the `perf` view + the pinned prettier suites. Pure Rust; absent dev repos warn-skip (floor = ../svelte src). NOT in `deno task check` (machine-dependent corpus, minutes); wired into publish Step 3c alongside conformance:all's SAFETY. Run at conformance/release cadence or after a printer change; see benches/js/CLAUDE.md §Gate map
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
- `tsv_napi` (napi-rs) — target: Node.js / Bun native addon; output: `libtsv_napi.{so,dylib,dll}` (loaded via `process.dlopen`). Currently a **measurement-only** binding for the Node benchmark runner (single-platform local build: `deno task build:napi`; boundary tests: `deno task test:napi`); the cross-platform publish matrix as `@fuzdev/tsv_napi` is targeted for 0.2. See ./crates/tsv_napi/CLAUDE.md.

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
# once, fail-fast, corpus FFI built once). The external-oracle correctness gates that can't live in
# `deno task check`. The format leg's prettier calls ride a content-addressed cache
# (benches/js/lib/prettier_cache.ts; TSV_PRETTIER_CACHE=0 disables).

deno task conformance:test262          # tsv's JS parser vs test262 POSITIVES (pure Rust, `test262 --gate`);
# negatives (the deferred early-error frontier) are reported, not gated. Exact POSITIVE_PASSED_PIN in the command.
deno task conformance:all              # the full drop-in conformance gate = `conformance` (5 FFI legs) +
# `conformance:test262` (pure Rust). What publish Step 3b runs. CSS-WPT harvest stays manual.

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

**Perf vs conformance surfaces.** The perf surface (`deno task bench:perf`) measures a
**real-world-only** corpus (app + framework source; fixture suites excluded) — the
throughput headline. Hard invariant: every in-scope tool must fully process every file or
the run fails (see `benches/js/lib/perf_omit.ts`), so coverage is 100% by construction.
The conformance surface (`deno task bench:conformance`) measures per-tool **parse
coverage** over a **disjoint, fixtures-only** corpus (prettier suites + svelte compiler
tests + the wpt-css/test262 harvests), writing `report.conformance.node.{json,md}`. Its
Svelte set excludes the files `svelte/compiler` rejects (the
`bench:harvest:svelte-rejects` cache) so coverage measures fidelity on *valid* Svelte,
not permissiveness over deliberately-invalid error fixtures — svelte-only, since
svelte/compiler is the one canonical parser tsv is a strict drop-in for. It's
**coverage-only and node-only by design**: coverage is a pre-flight product (no timed
phase) and runtime-invariant, so one node run is the whole surface. (A timed run over
this corpus is ad-hoc only — `BENCH_CORPUS=conformance node benches/js/bench.ts` — and
overwrites the report; re-run `bench:conformance:run` after.) `deno task bench` is the
full publish-cadence refresh: perf across all three runtimes + compose, then the node
conformance coverage run. The correctness gates (`deno task conformance`, corpus:compare)
keep their own unchanged corpus scope. See ./benches/js/CLAUDE.md §Corpus for the three views.

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

# Environment variables (apply to any runtime)
BENCH_LIMIT=10 deno task bench:deno:run        # Limit files per language (default: all)
BENCH_FILTER=zzz deno task bench:deno:run      # Filter by path pattern (default: none)
BENCH_DURATION=10000 deno task bench:deno:run  # Duration per benchmark in ms (default: 5000; conformance mode: 15000)
BENCH_WARMUP=10 deno task bench:deno:run       # Set warmup iterations (default: 3; slow >5s-per-sweep tasks tier to 1 unless set explicitly)
BENCH_MODE=union deno task bench:deno:run      # Per-impl iteration (default: intersection)
BENCH_CORPUS=conformance deno task bench:deno:run  # Corpus/surface selector (default: perf)
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

cargo run --release -p tsv_debug -- json_profile ~/dev/zzz/src/lib   # parse vs wire-JSON write timing

cargo run --release -p tsv_debug -- compile_profile tests/fixtures_compile  # Svelte compile vs the format wall
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

**The one carve-out is file *scope*, not style.** `tsv format`'s directory discovery is gitignore-aware, with two regimes keyed on `.git`. The **format root** (the scope boundary, derived from the argument — the cwd never participates) is, inside a git repo, the repo root (a hard stop for the upward walk, so `tsv format --check` is reproducible across machines when the ignore files are committed); outside one, the filesystem root. Inside a repo, discovery honors — relative to the repo root — **`.gitignore`** (hierarchically, exactly like git), then **`.formatignore`** (tsv's native file; hierarchical, deeper wins; applied after `.gitignore`, so its `!` can re-include a gitignore'd path subject to git's parent-directory rule), then **`.prettierignore`** (drop-in compat; hierarchical; the tsv-layer fallback in any directory with no sibling `.formatignore` — a sibling shadows it with a non-fatal warning), plus the always-skipped **safety nets** (`.git`, `node_modules`, `.sl`, `.hg`, `.svn`, `.jj`). Outside a repo, only `.formatignore` is read, hierarchically from the filesystem root down (so `~/.formatignore` acts as global config for loose files). Because the boundary is found by walking up, formatting a subdirectory gives the same result as formatting it via an ancestor.

When a `.gitignore` is in scope it is authoritative and the built-in **heuristic is off**; with none, the heuristic — hidden directories plus `dist`/`build`/`target` — is the fallback "not source" guess (an explicit tsv-layer `!` re-include overrides it). This is *only* about which files are reformatted, never how; an explicitly named file argument is always formatted. The matcher lives in the `tsv_ignore` crate (`IgnoreStack`); the per-directory prune *decision* (heuristic, safety nets, shadow warning) lives in `tsv_discover`. Both are shared with the JS CLI and the VS Code extension via the `IgnoreStack` WASM export (`classify_dir`/`should_format_file`/`heuristic_shadow_warning`, plus per-file `is_path_pruned` for the extension) — so all three surfaces agree by construction. Full rules and edge cases (unreadable ignore files, re-include idiom, warnings): ./docs/cli.md §Multi-File Formatting.

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
│   ├── tsv_svelte_compile/ # Svelte→JS compiler (Svelte's compile() oracle) + JS canonicalizer (intent-erased reprint); consumed by tsv_debug — no shipped artifact links it
│   ├── tsv_cli/     # Production CLI (binary: tsv) - pure Rust
│   ├── tsv_debug/   # Dev utilities (binary: tsv_debug) - uses Deno
│   ├── tsv_ffi/     # C FFI bindings (Deno's native path)
│   ├── tsv_wasm/    # WebAssembly bindings (published as @fuzdev/tsv_format_wasm + @fuzdev/tsv_parse_wasm + @fuzdev/tsv_wasm; bundles hand-maintained types/tsv_ast.d.ts + npm/locations.js no-loc reconstruction helper; npm/cli.js is the tsv bin)
│   └── tsv_napi/    # N-API bindings (Node/Bun native path; measurement-only for the Node bench, 0.2 publish target)
├── scripts/         # Publish orchestrator, npm package patcher, Node artifact + N-API addon tests, AST type drift check
├── tests/           # Integration tests (parser, formatter, CLI)
│   ├── fixtures/    # Test fixtures organized by language/feature
│   └── fixtures_compile/ # Compiler fixtures (input.svelte + canonicalized oracle expected_server.js + expected.css) — a separate tree so parser/formatter fixture counts stay unperturbed
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
├── divergent_variant_*.svelte      # OPTIONAL: Divergent variant (prettier keeps stable, ours rewrites to a distinct third stable form)
├── prettier_intermediate_*.svelte  # OPTIONAL: Prettier's unstable first-pass output (converges to input)
├── prettier_intermediate_to_variant_*.svelte  # OPTIONAL: Prettier's unstable first-pass output (converges to a variant_*/prettier_variant_*)
├── audit_signature.txt             # OPTIONAL: Auto-generated; pins prettier's multi-pass chain from output_prettier.* (F4)
├── prettier_nonconvergent.txt      # OPTIONAL: Prettier never reaches a fixed point on input — no oracle; claim live-verified (F5)
├── prettier_rejects.txt            # OPTIONAL: Prettier throws on input (parse rejection / printer crash) — no oracle; trimmed content is the expected-error substring, claim live-verified (F6)
├── tsv_rejects.txt                 # OPTIONAL: tsv over-rejects an input the canonical parser accepts — trimmed content is the expected tsv-error substring; pairs with expected_svelte.json (the canonical AST), no tsv-side expected/format files; claim live-verified (F7/S20)
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
- **Divergent variant (prettier keeps stable, ours → third form)**: Add `divergent_variant_*.*` files (requires `_prettier_divergence` suffix)
- **Normalization to input divergence**: `unformatted_ours_*.*` normalizes to input with our formatter only
- **Normalization to output_prettier**: `unformatted_prettier_*.*` normalizes to `output_prettier.*` with prettier
- **Prettier never converges (no oracle)**: Add `prettier_nonconvergent.txt` + README (requires `_prettier_divergence` suffix; excludes all prettier-claim files)
- **Prettier rejects/throws on input (no oracle)**: Add `prettier_rejects.txt` (trimmed content = expected-error substring) + README (requires `_prettier_divergence` suffix; excludes all prettier-claim files; mutually exclusive with `prettier_nonconvergent.txt`)
- **tsv over-rejects but canonical accepts**: Add `tsv_rejects.txt` (trimmed content = expected tsv-error substring) + `expected_svelte.json` + README (requires `_svelte_divergence` suffix; no `expected.json`/`expected_ours.json`; excludes all format-claim files, `input_invalid_*`, and the prettier no-oracle markers)
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
# forms match even though the parser keeps boundary whitespace verbatim. Sound: real content /
# <pre> / presence-of-space changes still differ. Confirms block-style render-equivalence at corpus scale.

# canonical_parse - parse using external parsers (Svelte, acorn+typescript, or our CSS)
cargo run -p tsv_debug canonical_parse file.svelte

# canonical_compile - compile Svelte with the canonical compiler (runes-only, deterministic oracle)
cargo run -p tsv_debug canonical_compile file.svelte                      # normalized server JS to stdout
cargo run -p tsv_debug canonical_compile file.svelte --target client --css  # client JS + delimited CSS
cargo run -p tsv_debug canonical_compile file.svelte --json              # { js, css, warnings } as JSON
# Fixed cssHash ('svelte-tsvhash') + constant filename make output byte-identical across runs.
# Also: --target server|client (default server), --dev, --content <str>, --stdin. Compile errors exit non-zero.

# compile_compare - diff tsv's Svelte compile against the canonical compiler, comparing
# the CANONICALIZED JS of both sides (intent-erased reprint via tsv_svelte_compile::canonicalize_js).
# The parity bar tolerates a comment-POSITION difference (compare_canonical: same code + same comment
# sequence, no bundler annotation — tsv's comment placement vs the oracle's esrap), so a remaining diff
# is a real code difference, not incidental whitespace. Self-checks canonicalizer idempotence on the
# oracle side. Exit codes: 0 parity (byte-exact OR comment-position-tolerated), 1 real diff, 2 error
# (including a component shape tsv's compiler doesn't cover yet — that prints the oracle canonical form
# as the target). --json emits { target, parity, comment_position_tolerated, ours_status, hunks }.
cargo run -p tsv_debug compile_compare file.svelte                       # human diff / oracle canonical form
cargo run -p tsv_debug compile_compare --content '<h1>hi</h1>' --json    # machine-readable report
# Also: --target server|client (default server), --content <str>, --stdin. The ad-hoc one-file view;
# durable expectations live in the compile fixtures (tests/fixtures_compile, below).

# compile_fixture_init - create or reinitialize a compile fixture (tests/fixtures_compile/<feature>/<case>/):
# prettier-formats the runes component, compiles it with the deterministic oracle (server, non-dev),
# writes input.svelte + expected_server.js (the CANONICALIZED oracle JS) + expected.css (raw oracle
# CSS, styled components only). Expected files are ALWAYS oracle-generated, never hand-written.
cargo run -p tsv_debug compile_fixture_init tests/fixtures_compile/feature/case --content '<p>text</p>'
echo '<p>text</p>' | cargo run -p tsv_debug compile_fixture_init tests/fixtures_compile/feature/case --stdin
cargo run -p tsv_debug compile_fixture_init tests/fixtures_compile/feature/case  # regenerate from existing input
# Also: --force (overwrite existing input).

# compile_fixtures_validate - validate compile fixtures. Per fixture, all gating: (a) oracle
# freshness — canonicalize(oracle(input)) must equal the committed expected_server.js byte-exact (+ css
# match); (b) ours — tsv_svelte_compile::compile must succeed and its canonicalized JS must be PARITY
# with expected_server.js (byte-exact or comment-position-tolerated, compare_canonical) + CSS match;
# (c) the committed expected_server.js must be a canonicalize fixed point.
# The pure-Rust slice of the contract (input parses, expected idempotent, OURS-VS-EXPECTED parity —
# compile() needs no Deno) also runs sidecar-free in
# `cargo test --workspace --test compile_fixtures_tests`, the offline parity gate.
cargo run -p tsv_debug compile_fixtures_validate [pattern...]
# Also: --list, --json.

# compile_corpus_compare - the compile-parity wide net: compile every .svelte under the given roots
# with the canonical compiler (oracle) AND tsv, comparing the canonical reprints of both sides.
# Buckets per file: parity (byte-exact OR comment-POSITION-tolerated — tsv's comment placement vs the
# oracle's; not a bug, surfaced in a separate comment_position sub-count) / refused (sub-bucketed by
# refusal reason — a clean "not yet", never a bug) / oracle-rejected (legacy mode, invalid syntax; out
# of scope) / MISMATCH (both compiled, canonical CODE differs — always a bug by the refusal contract) /
# error (harness failure).
# Every oracle-rejected file is also probed with tsv's compile(): a success is reported in a loud
# OVER-ACCEPTANCE section (nothing invalid in runes mode may compile — a refusal-contract gap),
# without affecting the exit code.
# Exit codes: 0 clean, 1 mismatch, 2 harness error. Sidecar-dependent — kept out of `deno task
# check`; the `compile:corpus:compare` deno task points it at the real-repo corpus + Svelte suites.
cargo run -p tsv_debug compile_corpus_compare <paths...>
# Also: --list, --json.

# erase_comment_census - size the type-eraser's comment-refusal haircut over a corpus (pure
# Rust, no Deno). Per lang="ts" component: collects the spans type erasure drops (TS-only
# statements, `: T` annotations, type params/args, as/satisfies/! tails, type-only
# imports/exports, declare items) and counts comments intersecting an erased span's refusal
# window — the span extended to the next surviving token, so `let x: Foo /* c */ = v` counts
# while a leading JSDoc on an erased interface (which survives erasure) does not. The census
# measures the FORWARD half of that window only, while the compiler's real refusal window is
# bidirectional (it also reaches BACKWARD over a detached erased region — a return type, an
# `implements` clause, a `<T>` list — where a comment can sit between the region and the token
# before it). So the exposure rate this reports is a LOWER BOUND on the true refusal rate.
# Also flags cheaply-detectable non-TS blockers (directives/spread, special elements, module
# scripts, option/select, instance exports) to approximate "type stripping is this file's only
# blocker"; runes/derived/evaluator refusals are NOT detected, so that bucket is an approximation.
cargo run --release -p tsv_debug -- erase_comment_census ../fuz_ui ../zzz
# Also: --verbose (per exposed file), --json.

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

# conformance_audit - doc/fixture integrity in one fixture walk. Four checks:
#  (1) Orphans - every divergence-suffixed fixture must be linked in its conformance doc
#      (_prettier_divergence → docs/conformance_prettier.md, _svelte_divergence →
#      docs/conformance_svelte.md, _svelte_prettier_divergence in both).
#  (2) Dead links - every Markdown link (relative path + #anchor) in both conformance docs and
#      every fixture README must resolve on disk (catches renamed/deleted fixtures, wrong ../
#      depth, stale anchors).
#  (3) Missing back-links - every divergence fixture's README must contain a link resolving to
#      its sanctioning doc. (A missing README entirely is the validator's D1 rule.)
#  (4) Stray READMEs - a non-divergence fixture shouldn't carry a README; exceptions live in
#      the in-code ALLOWED_NONDIVERGENCE_READMES allowlist.
# Pure Rust (no Deno). Exits non-zero on any finding. Gated in `deno task check`.
cargo run -p tsv_debug conformance_audit
# Also: --json (machine-readable: {orphans, dead_links, missing_backlinks, stray_readmes})

# compile_conformance_audit - the compiler analog of conformance_audit, deliberately minimal:
# any _compiled_divergence-suffixed compile fixture must be cataloged in
# docs/conformance_svelte_compiler.md AND carry a README back-linking it. The catalog is expected
# to stay EMPTY (a safety valve for declining to reproduce a genuine oracle output bug — never a
# tolerance budget), so this mostly asserts emptiness. Pure Rust (no Deno). Exits non-zero on any
# finding. Gated in `deno task check`.
cargo run -p tsv_debug compile_conformance_audit
# Also: --json

# canonicalize_audit - canonicalize_js (the compile-parity reprint) at corpus scale: run the
# canonicalizer twice per TS/JS file (.ts/.js/.mts/.cts/.mjs/.cjs, .svelte.ts included) and bucket —
# input-rejected (informational: invalid fixtures, script-goal files), NON-IDEMPOTENT (failure),
# CORRUPT-OUTPUT / unreparseable reprint (failure; the canonicalizer self-validates by reparse),
# COMMENT-LOSS (failure; whitespace-normalized comment text/order before-vs-after — the bucket the
# other two are structurally blind to: a swallowed comment leaves valid, idempotent JS).
# Pure Rust (no Deno). Exits 1 on any failure. Gated in `deno task check` over tests/fixtures +
# tests/fixtures_compile (fast); point it at real corpora for the full sweep.
cargo run -p tsv_debug canonicalize_audit                              # default scope (tests/fixtures only)
cargo run -p tsv_debug canonicalize_audit tests/fixtures tests/fixtures_compile  # the check-gate scope
cargo run -p tsv_debug canonicalize_audit ~/dev/zzz/src ~/dev/gro/src  # real-corpus sweep
# Also: --json
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
#           A bare run exits non-zero because negatives fail by design, so it's a diagnostic, not a gate.)

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

# json_profile - time the FFI parse path per file: parse vs the wire-JSON write.
# Pure Rust, no Deno; run with --release. Full detail: ./docs/performance.md §2.
cargo run --release -p tsv_debug -- json_profile ~/dev/zzz/src/lib
# Options: --iterations <n> (default: 5), --json (adds per-file data)

# compile_profile - profile Svelte compile timing against the format wall (pure Rust, no Deno).
# Per compiling .svelte file, three medians: compile (the whole cold per-call shape —
# tsv_svelte_compile::compile has no warm/arena-reuse entry point, so this IS its production
# shape) vs parse + format of the same file (warm reset-reuse arenas, the tsv_cli shape).
# Headline = the ratio column, compile / (parse + format): the compile-multiple over the format
# wall (design frame ~2-3x for the all-linear pipeline) — the cheap tripwire for super-linear or
# rebuilt work. Refusals/parse failures are counted, not timed; a CorruptOutput or a
# TypeErasureLeak is a compiler bug and fails the run. Compare ratios only against ratios from this same command (the two
# rows keep their own production shapes on purpose). Run with --release for anchors.
cargo run --release -p tsv_debug -- compile_profile tests/fixtures_compile
cargo run --release -p tsv_debug -- compile_profile ../svelte/packages/svelte/tests/runtime-runes
# Options: --iterations <n> (default: 10), --json

# buffer_sizes - AST histograms for tuning the TS printer's SmallVec inline
# capacities (named_specs, CommentLines) + the line-count distribution behind the
# `MultilineText` doc node: named-import-specifier count per import, and line count
# per multi-line block comment. Covers .ts/.svelte.ts AND .svelte (the <script>/{expr}
# feed the same TS-printer buffers). Prints percentiles + spill rate at candidate
# inline N. For sizing, exclude the prettier/svelte test suites (edge-case skew).
# Pure Rust, no Deno.
cargo run -p tsv_debug buffer_sizes ~/dev/zzz/src ~/dev/gro/src
# Options: --json

# arena_stats - DocArena node-population + memory audit over a corpus (the data
# behind the doc-IR memory/node-count levers): nodes/byte density, capacity fill %,
# output-String/AST-bump pre-size audits, DocNode variant + DocText sub-histograms,
# container degeneracy. Covers .ts/.svelte.ts/.svelte/.css. Pure Rust, no Deno.
# Full detail: ./docs/performance.md §7.
cargo run -p tsv_debug arena_stats ~/dev/zzz/src/lib ~/dev/fuz_css/src/lib
# Options: --json, --reuse (reset()-reuse high-water, as the CLI/FFI/WASM batch
#   drivers use), --list-errors (path + parse error per skipped file — the fast
#   first pass for finding tsv parse over-rejections; canonical-accepted ones are real gaps)

# lex_diff - differential lexer harness: snapshot the raw token stream over a
# corpus and diff against a golden to prove token-stream identity (kind, start, end,
# decoded per token) after a lexer change — stronger than format byte-identity.
# Covers the context-free next_token dispatch for .ts/.mts/.cts/.svelte.ts/.css.
# Pure Rust, no Deno.
cargo run -p tsv_debug lex_diff ~/dev/zzz/src --golden /tmp/lex.golden --write  # capture golden
cargo run -p tsv_debug lex_diff ~/dev/zzz/src --golden /tmp/lex.golden          # check against it
# Options: --write (capture instead of check), --verbose (first divergent line per file)
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
cargo run -p tsv_debug --features audits swallow_audit                 # audit all fixtures
cargo run -p tsv_debug --features audits swallow_audit ~/dev/zzz/src   # audit a real codebase
# Also: --json. The check lives in tsv_lang::doc::swallow behind the `swallow_check`
# cargo feature — off by default, so it's compiled out of prod wasm/cli/ffi AND
# default tsv_debug builds (profile/perf sessions measure production-shaped render
# code). The `swallow:audit` deno task builds it via the `audits` umbrella feature
# (swallow_check + comment_check), so it shares one binary with comments:audit,
# gaps:audit, and blanks:audit; `--features swallow_check` alone still works for a targeted run. Gated
# in `deno task check` (via `swallow:audit`) over tests/fixtures.
#
# Coverage is every render that appends to the output buffer — the main loop AND
# its sub-renders (fill segments, the line-suffix flush), all driving one
# per-thread state machine. A `line_suffix` comment is NOT exempt: two of them
# flushed at the same line break land back-to-back on one line (`x; // c2 // c1`)
# and the first `//` swallows the second. Comments written straight to the output
# buffer (the Svelte template buffer path) bypass the doc renderer and stay out
# of scope.
```

**Comment Ledger Audit (print-once: dropped / double-printed comments):**

```bash
# comment_audit - format files with the print-once comment ledger on and report every
# comment the format DROPPED (parsed, never emitted — silent content loss) or
# DOUBLE-PRINTED. tsv's answer to prettier's `ensureAllCommentsPrinted`, and the
# structural guard on the detached comment model: nothing else forces a comment that the
# parser produced to actually reach the output. Pure Rust, no Deno. Defaults to
# tests/fixtures; pass dirs/files to audit real code. Exits 1 on any finding.
cargo run -p tsv_debug --features audits comment_audit                 # audit all fixtures
cargo run -p tsv_debug --features audits comment_audit ~/dev/zzz/src   # audit a real codebase
# Also: --json. The ledger lives in tsv_lang::comment_ledger behind the `comment_check`
# cargo feature — off by default, so it's compiled out of prod wasm/cli/ffi AND default
# tsv_debug builds (profile/perf sessions measure production-shaped code). The
# `comments:audit` deno task builds it via the `audits` umbrella feature (swallow_check +
# comment_check), sharing one binary with swallow:audit, gaps:audit, and blanks:audit; `--features
# comment_check` alone still works for a targeted run. Gated in `deno task check` (via
# `comments:audit`) over tests/fixtures.
#
# Model. A format entry point (`tsv_ts::format_in`, `tsv_css`'s `format_css*`,
# `tsv_svelte`'s `format_svelte*`) REGISTERS the comment list it is about to print — that
# is the expectation. A doc-based printer (tsv_ts, tsv_svelte) TAGS each comment's doc
# node (`DocArena::tag_comment_doc`) and the RENDERER records the emit when it reaches
# the node; tsv_css, which writes comments straight to its buffer, records at the write.
# The render-time seam is load-bearing: a builder may assemble the same subtree into two
# `conditional_group` candidates of which one renders, so counting at build time reads as
# a double-print (and a comment built only into a LOSING candidate would read as printed
# while being lost). A `format-ignore` region — and any other raw source slice that
# carries comments out verbatim (a raw at-rule prelude, a glued CSS compound selector) —
# records a VERBATIM RANGE that counts as one emit per comment it covers; keep those
# ranges tight, a too-wide carve-out silently re-opens the hole.
#
# Scope. Both comment carriers are registered and guarded: the DETACHED comments (the flat
# `Vec<Comment>` on the language root) and the AST-NODE comments — a Svelte `<!-- … -->`
# (`FragmentNode::Comment`) and a CSS in-block `CssBlockChild::Comment`. The latter are
# carried by the tree rather than by the positional model, but a printer can still drop or
# double-print one, so each format entry walks its tree and registers their spans; with that,
# `unregistered emits` is a pure registration-gap signal (0 over clean fixtures) — a nonzero
# count means the walk missed a container. CSS declaration-VALUE comments remain outside the
# model by construction — never lexed as `Comment`s at all (re-derived from source), so there
# is nothing to register.
```

**Gap-Injection Audit (dropped-comment discovery):**

```bash
# gap_audit - inject a comment into EVERY gap and re-run the print-once ledger. The
# DISCOVERY arm `comments:audit` can't be: the ledger only ever sees a document AS
# AUTHORED, so a gap no fixture puts a comment in is one it never checks (eight such
# drops were found BY HAND, all green on every gate). Pure Rust, no sidecar.
cargo run --profile corpus -p tsv_debug --features audits gap_audit   # tests/fixtures
cargo run --profile corpus -p tsv_debug --features audits gap_audit ~/dev/zzz/src
# Also: --json, --jobs N, --limit N, --payload <one>, --all-bytes, --update.
# Build with `--profile corpus` (release + panic=unwind): plain `--release` is
# panic=abort, so a formatter panic kills the process instead of being caught + reported.
#
# GATED as a RATCHET, not a green gate: `gap_audit_known.txt` is a machine-generated
# snapshot of the ~717 shapes tests/fixtures produces, every line a KNOWN BUG, the file
# shrinking is the goal. A shape not on the list, one on it that no longer fires, or any
# PANIC, FAILS. `--limit`/`--payload`/`--all-bytes`/a path narrow a run, so they skip the
# ratchet and refuse `--update`. ~17 s.
```

Full reference — flags, the ratchet, reading a finding, triage + re-pin workflow,
scope: ./docs/gap_audit.md. Design rationale (why byte offsets and not tokens, why the
ledger is the oracle, why five payloads) lives in the `gap_audit` module docs.

**Blank-Line Injection Audit (blank-line handling discovery):**

```bash
# blank_audit - inject a blank line into EVERY code gap and grade six policy-free
# invariants on the result: (1) no panic, (2) F1 idempotency (pass 2 is a fixed
# point), (3) structural reparse, (4) leaf conservation, (5) ledger-clean (no
# dropped/double-printed comment), (6) blank-run ≤ 1 (no 2+ blank run outside a
# template quasi / <pre> / <textarea> / format-ignore region). Mechanizes the
# blank-line handling class — the specifier-list / array-pattern bugs. Invariants
# 1-4 are the shared `f1_check` (also driving `fuzz`); 5 is the print-once ledger;
# 6 is a region-scoped output scan. Pure Rust, no sidecar.
cargo run --profile corpus -p tsv_debug --features audits blank_audit   # tests/fixtures
cargo run --profile corpus -p tsv_debug --features audits blank_audit ~/dev/zzz/src
# Also: --json, --report, --jobs N, --limit N, --update. Build with `--profile
# corpus` (release + panic=unwind) so a formatter panic is caught + reported.
#
# GATED as a RATCHET (like gap_audit): `blank_audit_known.txt` is a machine-generated
# snapshot of the known-bug shapes, every line a bug, the file shrinking is the goal.
# A graded shape not on the list, one that no longer fires, or any PANIC, FAILS. Unlike
# fuzz/roundtrip, NON-IDEMPOTENT and every policy kind ARE pinned (born red over a live
# bug family); PANIC stays absolute; and STRUCTURAL-DIVERGENCE is held REPORT-ONLY
# (fuzz-soft parity — reported but never gated, filtered out of the ratchet, `"gated":
# false` in --json). A FAST PATH — a blank the formatter ABSORBS reproduces the file's
# proven-clean pristine output byte-for-byte, so nothing is checked — keeps it near
# gap_audit's one-format-per-site cost; only a KEPT blank pays the full battery (~19%
# of injections over tests/fixtures). ~24 s.
# Scope: TS + Svelte body; CSS deferred; string/template interiors excluded (a raw
# newline there is lexed as content, not a gap); only format fixed points injected.
```

Full reference — flags, the ratchet, reading a finding, the six invariants, scope:
./docs/blank_audit.md. Design rationale (the fast path, why a blank is graded against
the injected input not the pristine, the string-interior exclusion) lives in the
`blank_audit` module docs.

**Build-Fanout Audit (exponential-rebuild regression guard):**

```bash
# build_fanout_audit - guard the O(1)-doc-builds-per-source-node invariant. A
# builder that assembles `conditional_group` candidates by RE-INVOKING the recursive
# builder on the same nodes — instead of building the subtree once and reusing the
# DocId — grows the doc-node count exponentially in nesting depth (hang/OOM on a
# deeply-nested but ordinary file). Builds synthetic nested inputs across six axes
# (svelte elements / {#if} / {#each} / {#await} / sibling-`>` dangle, ts member
# chains) at increasing depth and fails if the doc-node count grows faster than
# ~depth^3. Deterministic, pure Rust, no Deno. Exits 1 on any super-linear case.
cargo run -p tsv_debug build_fanout_audit
# Also: --json. Gated in `deno task check` via the `fanout:audit` task.
```

**Raw-Find Scan Audit (delimiter-scan regression guard):**

```bash
# scan_audit - guard against new raw position-anchoring substring scans over
# source. A raw `self.source[..].find(delim)` can match the glyph inside an
# enclosed comment/string and drop content (the "Comment-Aware Delimiter Scans"
# bug class); the fix is the trivia-aware cursor (`tsv_lang::source_scan`).
# Flags every `find`/`rfind`/`match_indices`/`rmatch_indices` (non-closure pattern)
# in the four language crates and fails on any not in the reviewed, categorized
# in-code allow-list (ALLOW). A new scan must move onto the cursor or be consciously
# allow-listed; a migrated/reformatted scan must drop its now-stale entry (the list
# mirrors the live sites exactly). Pure Rust, no Deno.
cargo run -p tsv_debug scan_audit            # audit (exit 1 on any violation/stale)
cargo run -p tsv_debug scan_audit --list     # enumerate every scan site
# Also: --json. Gated in `deno task check` via the `scan:audit` task. Out of scope:
# closure `.find(|…|)` (iterator/predicate), counting/existence checks, and hand
# byte-loops (the cursor is their sanctioned home).
```

**Authoring-Independence Audit (Svelte boundary whitespace):**

```bash
# authoring_audit - probe whether the SAME logical document, authored with
# different boundary whitespace, formats to ONE tsv fixed point. Stronger than the
# corpus idempotency sweep: a formatter can be idempotent yet authoring-DEPENDENT
# (two authorings settling on two different stable outputs). Two mutation families,
# never a blank line (Tier-1 significant) and never inside <pre>/<textarea>:
#   - BETWEEN siblings — space↔single-newline only. Inter-node whitespace is
#     render-SIGNIFICANT (it collapses to one space, it doesn't vanish), so the run is
#     reshaped, never created or destroyed. Both forms collapse identically ⇒ safe.
#   - At a tag's CONTENT BOUNDARY — hug↔space↔newline, i.e. the run IS created and
#     destroyed. Svelte 5 removes start/end-of-content whitespace at compile, so all
#     three authorings render identically. This is the family that catches a formatter
#     letting a render-free character pick the layout (the delimiter-dangle class); it
#     probes only elements whose content already spans lines, where layout is at stake.
#     Note what that skips: for content that FITS on one line tsv preserves an authored
#     boundary space (`<span> text </span>` and `<span>text</span>` are BOTH stable), so a
#     clean run means no render-free character picks a LAYOUT — not that none survives in
#     the output. That preservation is deliberate and prettier-matching (fixture
#     `inline_boundary_whitespace`); see conformance_prettier.md §Svelte: Inline content
#     block-style.
# The element expansion a mutation may trigger is the property under test. Svelte only.
# Gated in `deno task check` via the `authoring:audit` task — which scans tests/fixtures
# ONLY, so point it at a real codebase too: findings live there (a non-idempotent fill
# 2-cycle was green on fixtures while failing on ~/dev/zzz).
cargo run -p tsv_debug authoring_audit                  # audit tests/fixtures (pure Rust)
cargo run -p tsv_debug authoring_audit ~/dev/zzz/src    # audit a real codebase
# Pure-Rust verdict per site: converge / diverge (dual-stable) / diverge
# (NON-IDEMPOTENT); exits 1 on any non-idempotency — site-level, and also a
# base-non-idempotent FILE (one whose own format isn't a fixed point). Such a file
# is excluded from the authoring analysis (its fixed point is undefined, so the
# converge/diverge verdict would be meaningless), but the exclusion is not a reason
# to pass the run — that is how a whole-file reflow could sit here reported-but-green.
#
# --prettier adds sidecar triage:
# (a) tsv diverges where prettier converges (bug); (b) tsv converges where prettier
# diverges (a _prettier_divergence to pin, the space_after_block class); (c) both
# diverge (sanctioned, e.g. Tier-2 element expansion). --dump-dir writes byte-exact
# repro artifacts per hard finding — the basis for a fixtures-first fix.
# Also: --json, --verbose, --limit N (sites/file), --examples N.
cargo run -p tsv_debug authoring_audit ~/dev/zzz/src --prettier --dump-dir /tmp/audit
```

**Format→Reparse Round-Trip Audit (delimiter/structure-corruption gate):**

```bash
# roundtrip_audit - corpus-scale "does format(src) reparse to the SAME document?".
# Catches the class the other gates can't see: output that mis-delimits but loses no
# characters (attr='a"b' → attr="a"b", `+(+x)` → `++x`) — corpus:compare:format's
# SAFETY is char-frequency, BLIND to delimiter/structure corruption. Two phases
# (tsv-self pre-filter → canonical confirm via sidecar): parse input and formatted
# output, reduce each to a STRUCTURAL SKELETON (node-tree shape + `type`, erasing
# reformattable leaf scalars + acorn `extra`), compare — so legit reformatting
# doesn't read as corruption. Buckets: {tsv,canonical}_unreparseable (the prize —
# output the parser rejects) and {tsv,canonical}_divergent (structural change).
# Zero false positives on real formatted code; point it at the delimiter-dense
# prettier suites for the work-list.
cargo run -p tsv_debug roundtrip_audit                              # audit tests/fixtures
cargo run -p tsv_debug roundtrip_audit ../prettier/tests/format/js ../zzz/src
# --gate fails ONLY on the *_unreparseable buckets (the reliable half — divergent is
# render-model noise over tests/fixtures). Bare --gate runs phase 1 only via a
# reparse-only fast path (pure Rust, no sidecar) — the `deno task roundtrip:audit`
# check gate; a cheap tripwire over tests/fixtures, real yield on external corpora.
# --canonical-all confirms every file (also guards canonical_unreparseable: tsv's
# parser accepting output the real parser rejects).
cargo run -p tsv_debug roundtrip_audit --gate                       # the check gate (pure Rust, tests/fixtures)
cargo run -p tsv_debug roundtrip_audit --gate --canonical-all ../prettier/tests/format  # thorough
# Also: --no-render, --verbose (AST diff per finding), --limit N, --json. The full
# (non-gate) run is a diagnostic — the divergent bucket over tests/fixtures is
# Svelte-reflow-noisy vs render_normalize's simpler whitespace model.
cargo run -p tsv_debug roundtrip_audit --canonical-all --verbose ../prettier/tests/format/typescript
```

**Comment↔Token Binding Audit (re-binding gate):**

```bash
# binding_audit - does format re-bind a FORWARD-binding comment to a different
# subtree? Two comment kinds bind to the token AFTER them: a JSDoc type cast
# (`/** @type {T} */ (x)` — the parens + comment ARE the cast) and a bundler
# annotation (`/* @__PURE__ */ f()` — marks the call side-effect-free). A paren
# migrating across such a comment under formatting silently re-binds it (a cast
# annotating a wider node, an annotation gone inert). This class is INVISIBLE to
# every other gate — neither a cast, a grouping paren, nor an annotation is a
# public-AST node, so both forms serialize to byte-identical wire JSON: ast_diff
# says "equivalent", roundtrip_audit's skeleton can't see it, corpus SAFETY is
# char-frequency (the characters only MOVE). Pure Rust, no sidecar.
#
# Signal: reparse input + tsv-formatted output with `preserve_parens` (grouping
# parens become ParenthesizedExpression nodes), and per glued comment compare the
# bound subtree. A cast stays invisible even so (its JsdocCast node emits its bare
# inner), so the audit anchors INSIDE the cast's `(`. And since the only structural
# delta formatting can add under preserve_parens is a clarity-paren (roundtrip_audit
# gates the rest), the skeleton is compared with ParenthesizedExpression STRIPPED —
# the binding-paren signal rides a separate `anchor_is_paren` flag. So a clarity
# paren deep inside is not a finding; a paren at the anchor is.
#
# HARD (a parser-owned glued comment re-binds) fails --gate — every glued block
# comment is owned, so a cast, an annotation, and a plain glued comment alike; SOFT
# (an unowned glued block comment, now rare) is informational. TS-family files
# only (.ts/.js/.mts/.cts/…); casts/annotations concentrate in JSDoc-typed JS.
cargo run -p tsv_debug binding_audit                                  # audit tests/fixtures
cargo run -p tsv_debug binding_audit ../svelte/packages/svelte/src ../prettier/tests/format/js
cargo run -p tsv_debug binding_audit --gate                          # the check gate (HARD only)
# Also: --verbose (in→out bound-subtree per finding), --limit N, --json. A bare
# --gate over tests/fixtures is a cheap tripwire (fixtures are format-stable); the
# real yield is external corpora, where JSDoc casts + annotations are dense.
cargo run -p tsv_debug binding_audit --verbose ../svelte/packages/svelte/src
```

**Layout-Neutrality Audit (ownership-blind-gate probe):**

```bash
# neutrality_audit - does a comment's OWNERSHIP ever change tsv's layout? An owned
# comment must occupy exactly the page space a same-width ordinary comment does — a
# layout gate that instead SKIPS owned comments (asks the to-emit question where it
# should ask on-page) goes blind, and the comment silently changes the layout it
# should have forced. At each glued block-comment position, format the file with the
# comment made OWNED (annotation-shaped) and made ORDINARY (plain filler) at the SAME
# width — only ownership varies, so any layout difference is a gate reading ownership.
# Pure Rust, no sidecar. A development / characterization tool, NOT a `deno task
# check` gate: it needs an owned/ordinary CONTRAST to detect anything, and under the
# "every glued block comment is owned" rule a run passes vacuously — its moment is
# BEFORE any future ownership-rule change (run it then, over external corpora).
# TS-family files only; defaults to tests/fixtures.
cargo run -p tsv_debug neutrality_audit ../svelte/packages/svelte/src
# Also: --gate (exit 1 on findings; dev-loop convenience), --verbose (the
# owned-vs-ordinary output diff per finding), --limit N, --json.
```

**Seeded Mutational Fuzzer (panic / idempotency / structural-reparse safety):**

```bash
# fuzz - dep-free seeded mutational fuzzer (the coverage-trifecta fuzzing leg). A
# SplitMix64 PRNG + byte-level mutation operators (plus multi-byte inserts: a
# unicode span/width stress set — NBSP/zero-width/BOM/combining/CJK/emoji/CRLF —
# and a structure-bearing token dictionary aimed at the parser's ACCEPT paths)
# over a seed corpus (default tests/fixtures); every valid-UTF-8 mutant is driven
# through parse+format+reparse under catch_unwind. Asserts three properties
# nothing else guards on ARBITRARY input: (1) no panic — the parser must never
# crash (prod WASM is panic=abort → a panic is a DoS; the corpus profile only
# catches panics on real code); (2) format idempotency (the F1 fixed point);
# (3) structural reparse (reusing roundtrip_audit's skeleton compare).
# Deterministic per --seed + corpus — and CORPUS-ADD-STABLE: each seed file draws
# mutants from its own path-keyed PRNG stream, scheduled round-robin, so a
# fixture add/remove/rename changes only that file's mutants (every other stream
# is byte-identical; a shrunken per-file budget trims a stream's tail, never
# rewrites it). Pure Rust, no sidecar. Not the differential (tsv-vs-canonical) leg.
# The `fuzz:audit` deno task (fixed --seed 0 --iterations 5000 over tests/fixtures) is
# gated in `deno task check` — a cheap standing tripwire for the three invariants.
#
# Hangs can't be caught in-process (the exponential-rebuild class), so two
# tripwires: every attempt's input is written to a last-input repro file BEFORE
# the attempt (path printed at startup; removed on a clean exit — a killed hung
# run leaves its exact input on disk), and attempts over --slow-budget-ms
# (default 2000) are reported, never fatally.
#
# TWO passes. Pass 1 drives every seed file AS AUTHORED (unmutated), pass 2 the
# mutants. The pristine pass matters because the corpus is the richest source of
# real, formatter-reachable inputs — and over tests/fixtures it is the ONLY gate
# that drives the non-`input.*` fixture files: the validator claims F1 on `input.*`
# alone, so `output_prettier.*` / `variant_*` / `unformatted_*` (all real code)
# were never themselves formatted twice. A pristine seed's *soft* verdict does not
# FAIL the run (the corpus deliberately holds mis-formatted `unformatted_*` files whose
# reflow is the point) but IS reported, with paths — over a real-code corpus there are
# no such files, so each wants triage, and the seed path is itself the repro (an
# unmutated file on disk), so it is listed rather than dumped. HARD verdicts fail.
cargo run -p tsv_debug fuzz                                    # 2000 iters over tests/fixtures
cargo run -p tsv_debug fuzz --seed 7 --iterations 20000 --evolve --minimize --dump-dir /tmp/fz  # discovery
cargo run -p tsv_debug fuzz --iterations 0 ~/dev/zzz/src       # pristine pass only = an F1 sweep
# HARD findings (exit 1): panic / unreparseable / non_idempotent / format_error —
# always real bugs. SOFT findings (reported, non-fatal): structural_divergence — the
# render-model-noisy bucket that needs canonical confirmation (roundtrip_audit
# --canonical-all), like roundtrip_audit --gate. --strict fails on soft too.
#
# Discovery aids (both opt-in, off in the gate): --evolve feeds every mutant that
# passes all invariants back into the seed pool (bounded at 2× the initial corpus)
# so later mutants walk deeper into the ACCEPTED-input space — the formatter's
# coverage, since a mutant must parse before F1/reparse grade anything; --minimize
# ddmin-shrinks each stored HARD finding (greedy chunk removal while the same
# outcome reproduces, bounded probes) into a consumable repro before report/dump.
# Also: --parser not applicable (per-file extension), --max-mutations N, --limit N,
# --max-findings N (HARD only), --slow-budget-ms N, --json.
```

**F1 Idempotency Sweep (real-code corpus):**

```bash
# The pristine pass above, pointed at the `perf` corpus view (the sibling dev repos
# + upstream framework source) — `format(format(x)) == format(x)` on every real file.
# NOT in `deno task check`: the corpus is machine-dependent checkouts and the sweep
# is minutes. It is a different risk surface from the fixtures — a formatter can be
# idempotent on every curated fixture and still reflow a real component on pass 2.
# Run at conformance cadence, or after any printer change.
deno task idempotency:sweep
# Absent corpus checkouts are skipped with a warning (not a failure); builds with
# `--profile corpus` (release + panic=unwind) because the fuzzer needs catch_unwind.
```

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

**Example**:

```rust
// Internal - clean and semantic
struct Literal {
    value: LiteralValue,  // Fully decoded: "test\n" → "test<newline>"
    span: Span,
}

// The writer emits the wire JSON straight from the internal node, applying the
// quirks (here, `raw` reconstructed from source) — no intermediate typed tree:
fn write_literal(w: &mut JsonWriter, lit: &Literal, ctx: &Ctx) {
    node_header(w, "Literal", lit.span, ctx);   // "type"/"start"/"end"/"loc"
    // … "value" emitted from lit.value …
    w.raw(",\"raw\":");
    w.string(lit.span.extract(ctx.source));      // reconstruct from source
    w.raw("}");
}
```

**Key Rules**:

- Raw strings NEVER duplicated in the internal AST (extract via `source[span.range()]`)
- The internal AST is NEVER the wire output — the wire JSON is hand-emitted by the
  writer; `serde_json` is used only for exact string-escape / `f64` parity and to
  parse the bytes back into a `Value` (CLI `--pretty`, tests)

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
    pub bump_pattern_columns: bool, // Serializer hint: +1 loc columns (Svelte block-pattern parse)
    pub owned_by_node: bool,        // Printed by the node it's bound to, not by the enclosing gap
}
```

**Owned comments — the one crack in the detached model.** A comment that is *bound to
the token after it* can't be located positionally at print time, because a paren the
printer synthesizes around an enclosing expression lands between the two and re-binds it.
**Every glued block comment is owned** (`owned_by_node`, set by the parser): the position
is an authoring choice that binds the comment to the operand it leads, so the operand's own
doc prints it and no synthesized paren can land between them. There is no content sniff — a
plain `/* c */` and a bundler annotation `/* @__PURE__ */` bind their token identically. Two
shapes are worth naming:

- the **glued block comment** (the general case) — printed by the innermost node its token
  begins (`printer/comments/owned.rs`, via `build_expression_doc`'s
  `prepend_owned_leading_comment`). Covers an ordinary leading comment and a **bundler
  annotation** alike (`/* @__PURE__ */` and friends mark the call *after* them as
  side-effect-free; a paren between the two would leave the annotation leading a paren, so the
  marked call is no longer droppable). No AST node is involved.
- the **JSDoc cast** — `/** @type {T} */` plus the `(` it glues to **are** the cast, so the
  comment is handed to the `JsdocCast` node, which prints it. `is_jsdoc_type_cast_comment` is
  the **only** remaining content sniff, and it governs the cast's **paren-retention** (building
  the `JsdocCast`), *not* ownership — ownership flows to a cast the same as to any other glued
  comment.

An owned comment is always a **block** comment (`owned ⇒ is_block`), and always **glued** to
its token — a comment the author left on its own line leads the *line*, not the token. The one
exception is the JSDoc cast, whose comment may sit a newline above its `(` and still be the
cast; that difference is load-bearing and named at the shared scan
(`source_scan::CommentGlue`).

**Ownership is a fact about who PRINTS a comment, never about whether it EXISTS.** That one
sentence is the whole rule, and every bug in this class has been a violation of it. A comment
can be asked about along exactly **three** axes, and the lookup API (`tsv_lang::comment`) makes
the caller name which:

| axis | question | owned comments | who asks |
| --- | --- | --- | --- |
| **to emit** | "which comments must *I* print here?" | **skipped** | gap emitters (~200 sites) |
| **on page** | "does any comment OCCUPY THE PAGE here?" | **counted** | layout gates — break / expand / hug / paren / fast-path |
| **in source** | "what comment BYTES are physically here?" | **counted** | cursors — blank-line scans, offsets, `prev_end` |

`comments_to_emit_in_range` / `has_comments_to_emit_in_range` / `comments_to_emit_after` ·
`comments_on_page_in_range` / `has_comments_on_page_in_range` /
`has_multiline_block_comments_on_page_in_range` · `comments_in_source_range` /
`comments_in_source_after`. Every name states its axis, so a miswire reads as a category error
at the call site rather than as plausible code. Two facts about the shape:

- **Three questions, but only two membership sets.** *On page* and *in source* both count an
  owned comment (it is in the output, and its bytes are in the file); only *to emit* skips it.
  The two names exist because the *question* differs — and naming the wrong one is the bug.
- `has_line_comments_in_range` carries **no** axis, provably: `owned ⇒ is_block`, so no line
  comment is ever owned and skip ≡ count. If that ever changes, it must grow an axis.

Two corollaries worth naming, because each was a whole family of bugs:

- A **zero-comment fast gate** (`let has_comments = …` guarding a whole builder) is an
  **on-page** question. It short-circuits the layout gates it guards, so an emit-keyed one makes
  every one of them blind.
- A **blank-line scan** is an **in-source** question. `has_blank_line_between*` is a raw newline
  count — it cannot tell a comment's own newlines from an author's blank line, so the scan must
  step over every comment in the gap (`blank_scan_start` / `blank_scan_end`), not just the ones
  this caller emits.

⚠️ **Three hazards, all of which have bitten.** Ownership is a sharp tool: it takes a comment out
of the positional model, so every consumer of that model has to be re-examined.

1. **An owned comment nothing prints is a DROPPED comment.** Ownership assumes the owning node
   prints it, so any builder that **reassembles** a node instead of routing it through
   `build_expression_doc` must claim it on its own seam (`prepend_owned_leading_comment_at`).
   Two do: `build_arrow_sig_doc` (every call-argument state) and `build_arrow_chain_doc`'s inner
   arrows. Adding a third reassembly path means adding a third claim.
2. **An owned comment travels *inside* its node's doc, so the gap around it can't see it.** The
   assignment layout inspects the operator→value gap (`rhs_comments`), which is empty for an
   owned comment — yet the comment still hangs the value. The node has to be asked instead:
   `owned_leading_comment_hangs_value`. It is the single seam for that question (the declarator,
   the class property, the object value, and the `is_poorly_breakable_chain` invariant all route
   through it).
3. **A region the parser LIFTS OUT of its container is still inside the container's gap** — so two
   emitters print it, where hazard 1 has none. `<svelte:element this={…}>` keeps its `this` out of
   `Element::attributes` (it rides in the kind), yet the braces still sit in the name→`>` gap the
   attribute scan probes: the tag's own doc prints the comment, then the scan prints it again.
   `AttrGaps::claimed` is that seam — the region whose comments the scan must skip. What makes this
   one nasty is that **ownership masks it**: a glued *block* comment is owned, so the gap skips it
   and the double-print never appears; only a **line** comment (never owned — `owned ⇒ is_block`)
   exposes it. A lifted region needs a claim on *both* axes, and testing with block comments alone
   will tell you it is fine.

All three hazards are what the **print-once comment ledger** exists to catch — the structural
guard on this model: every comment a document parses must be emitted exactly once, or the
audit reports it as DROPPED or DOUBLE-PRINTED (`deno task comments:audit`, gated in
`deno task check`; see [Comment Ledger Audit](#debug-tooling)). Nothing else in the
detached model forces a parsed comment to reach the output. Hazard 3 was found by it, not by
review — the block-comment repro looked clean.

Prettier, oxfmt and biome all get the paren binding wrong — see
[conformance_prettier.md §Comment relocation](docs/conformance_prettier.md#comment-relocation)
and [§JSDoc / paren semantics](docs/conformance_prettier.md).

The content is **not stored owned** — comment text is a pure delimiter-stripped
sub-slice of source, so `Comment` holds a `content_span` and recovers the text on
demand via `Comment::content(source) -> &str` (`source` must be the host document the
spans were recorded against); every field is `Copy`, no `String` per comment.
`multiline` is precomputed so the multi-line-block expansion checks
(`has_multiline_block_comments_on_page_in_range` and the printers) read an O(1), source-free
flag instead of re-scanning content. The full comment span includes its delimiters
(`//` / `/* */` / a `#!` hashbang, whose content includes the `#!`); the lexer is the
single owner of those widths.

**Printer strategy**: Range-based lookup via `comments_to_emit_in_range(prev_end, node_start)` (and its on-page / in-source siblings above). Source string for context (same-line detection, blank line preservation). Tradeoff: simple/efficient AST matching Prettier's model, but printer must manually track `prev_end` positions; edge cases (e.g., arrow function comments) require careful span math.

**Leading comments have one rule and one emitter.** A comment run *before* an item (a value, member, list element, or comma-separated item) is emitted by `Printer::push_leading_comment_run` (`printer/comments/mod.rs`), which implements prettier's `printLeadingComment` and picks the separator after each comment from the source around **that comment only**, never from where the item starts: **space** when nothing but spaces follow its `*/` (so a run the author glued stays glued — `/* a */ /* b */⏎X`), a soft **`line`** when a newline follows but none precedes, and a blank-preserving **`hardline`** for an own-line block or any line comment. The glue test alone is `Printer::comment_hugs_next` — the single statement of the rule, called even by the few sites whose surrounding loop must differ. The three hand-rolled leading-run sites whose loop can't route through `push_leading_comment_run` — `build_eq_comment_break_rhs`, `append_keyword_value_line_comments`, `emit_leading_comments_inline_aware` (all always-broken line-comment contexts, so a two-outcome space-or-hardline separator) — share `Printer::push_leading_run_separator`, which pairs the **physical-next** anchor (`blank_scan_end`, so an owned comment glued to the value doesn't desync the decision) with the `comment_hugs_next` hug-or-blank-hardline choice. ⚠️ Do not hand-roll `is_block && is_same_line(c.span.end, X)` at a new site, and don't re-derive the anchor+separator inline — keying the hug on the *item* rather than on *what follows the comment*, or anchoring on the emit-next *past* an owned comment, splits an author-glued run or inserts a phantom blank, and was a whole bug family (unglue / block-run merge / owned-comment blank scan). A site that also owns a comma emits the gap via `push_inter_item_line_comment_gap`, which owns the break too — that is what lets the last comment hug the next item.

**Whether that soft `line` collapses is decided by one fact, and it predicts every list site.** The **array family** — array literals, array patterns, and tuple types — wraps each element's run *plus* the element in a per-element `group` (`Printer::build_list_element_group`), so the `line` is measured against that element alone and collapses (`/* c1 */ /* c2 */ a`) even while the list itself is broken. The **params family** — function / type-parameter / type-argument / call-argument lists — gives an element **no group of its own** (the width path is a bare `join([",", line])`; the comment-forced-multiline path is a hardline-joined list), so the identical `line` has nothing to be measured against but the enclosing broken group, and breaks (`/* c1 */ /* c2 */⏎a`). This mirrors prettier exactly: `printArrayElements` pushes `group(print())` per element and `print()` carries the leading comments (`print/array.js`), while `printFunctionParameters` / `printTypeParameters` / `print/call-arguments.js` do not. Don't re-derive it per site — and don't "fix" a params-family break by adding a group, or an array-family collapse by removing one.

Higher-fidelity models (attached comments, trivia tokens) may be needed for IDE/linter use cases.

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
