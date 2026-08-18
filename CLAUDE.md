# tsv

> precise language tools for TypeScript/JS, CSS, and Svelte in Rust

High-performance Rust parser as a drop-in replacement for Svelte's modern parser (acorn + acorn-typescript), paired with a formatter that took Prettier as its initial guide and still tracks it for the common case — while making deliberate, cataloged divergences where tsv's own judgment is more defensible.

**Non-configurable by design**: Prettier defaults except printWidth=100, useTabs=true, singleQuote=true, trailingComma='none' — no config files, CLI flags, or runtime options, ever (opinionated like `gofmt` and Black). The one carve-out is file *scope*, not style: `tsv format` honors `.gitignore` plus hierarchical `.formatignore` / `.prettierignore`. See [Configuration](#configuration).

## Committing

`git add` and `git commit` are denied by `.claude/settings.local.json` in this
repo — make the edits and stop, the user commits. Version bumps and publishing
are user-owned too.

**Do not edit `CHANGELOG.md`.** Like release version bumps, the changelog is the
user's responsibility — agents make the source/doc/fixture edits and leave
`CHANGELOG.md` alone (including `## Unreleased` and its `<!-- bump: … -->`
marker). The user stamps it at release time.

## Priorities

1. **Correctness**: Match Svelte's parser exactly — it's a drop-in replacement. The formatter began with Prettier as its guide and tracks it for the common case, but makes deliberate, cataloged divergences where more defensible (spec, print width, comment position, its own taste) and fixes numerous Prettier bugs. Fixtures are the source of truth — when tests fail, fix the code; when tsv diverges on purpose, the fixture records it.
2. **Performance**: Pure Rust for speed. Dev tools use an embedded Deno sidecar that minimizes process overhead.

## Development Philosophy: Test-Driven Development with Fixtures

**ALWAYS use TDD when implementing features or fixing bugs:**

0. **Load context FIRST** - Read BOTH ./docs/fixture_workflow.md AND ./docs/fixture_naming.md into context.
   For ANY `_prettier_divergence` fixture, ALSO read ./docs/conformance_prettier.md (the shared
   frame — terminology, `◆reason` tags, decision framework) **plus the catalog for the language
   you're touching**, listed in its §Catalogs table: ./docs/conformance_prettier_css.md,
   ./docs/conformance_prettier_svelte.md, ./docs/conformance_prettier_ts.md,
   ./docs/conformance_prettier_ts_comments.md, ./docs/conformance_prettier_ignore.md. Every
   divergence (not just comment ones) must be sanctioned and **cataloged in the relevant section**
   (comment divergences: §Comment Position Philosophy in the frame + the §Comment relocation
   catalog; others: the matching feature section), AND the fixture's `README.md` MUST link back to
   that section (`See [conformance_prettier_<lang>.md §…](…)`) — the README and catalog entry must
   agree, and `conformance:audit` gates that agreement: linking the shared frame does **not**
   satisfy a divergence whose entry lives in a language catalog (link both — the frame for the
   principle, the catalog for the entry). Study 2-3 existing fixtures in the target category
   (match their README shape).
1. **Create the fixture FIRST** - `fixture_init` creates `input.svelte` (prettier-formatted) and `expected.json` in one step.
   Use `.svelte` unless the feature is file-level (byte 0: hashbang, BOM). See ./docs/fixture_workflow.md#11-create-directory-and-draft.
2. **Review the input** - Read the generated `input.svelte` to verify structure (formatting is guaranteed correct).
3. **See it fail** - Run `deno task fixtures:validate <pattern>` to show the failing diff
4. **⚠️ APPROVAL GATE — STOP HERE.** Show the failing diff to the user and wait for explicit
   confirmation ("lgtm", "proceed", or feedback) before writing any implementation code.
   **If feedback requires reworking the fixture (naming, structure, cases), redo steps 1-3 and
   return here — the gate resets on every rework.**
5. **Implement the fix** - Write code to make the test pass
6. **Validate** - Run `deno task fixtures:validate <pattern>` to confirm it passes

**For `long` fixtures**: include BOTH a 100-char case (stays inline) and a 101-char case (breaks); test the exact 100/101 boundary with the minimum content that triggers it. Iterate `fixture_init --force` and read the widths from its output — never estimate manually.

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
deno task fixtures:validate <pattern>    # Fast, targeted fixture validation (preferred for fixture work)
cargo test --workspace                   # Run ALL tests (~5-10s, includes all fixtures)
deno task check                          # Full committed-tree gate: fmt, audits, typecheck, tests, clippy (benches/js/CLAUDE.md §Gate map)
```

**When to use `fixtures:update` commands:** after creating a new fixture, or when upstream sources change (Svelte/prettier versions) — never to "fix" failing tests (fix the code instead).

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
deno task build:all        # release + ffi + build:packages + build:napi:packages (everything)
deno task build:packages   # the 6 publishable WASM bundles (npm + deno) — single source of truth shared by CI + publish.ts
deno task build:bench      # the artifact set `bench`/`smoke` measure (ffi×3 + the 3 wasm:deno variants + the node half: napi + wasm:all:nodejs)
deno task build:ffi        # C FFI library (:format / :parse size-only variants; :all builds all three)
deno task build:wasm:deno  # deno-target WASM bundle (requires wasm-pack; :parse:deno / :all:deno for the other variants)
deno task install-cli      # build the release CLI and install it to ~/.local/bin/tsv (the local daily driver)
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

Parser auto-detected from extension (the JS/TS family → TypeScript, `.svelte`, `.css`); `--content` and `--stdin` require `--parser svelte|typescript|css`. For TypeScript, `--goal script|module` selects the parse goal (for `format`, `--content`/`--stdin` only).

`format` writes paths **in place** (only when output differs) and prints changed paths to stdout; `--content`/`--stdin` print to stdout. Directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`, all parsed as TypeScript — JSX/TSX out of scope), `.svelte`, and `.css` with gitignore-aware, reproducible discovery (see [Configuration](#configuration); full rules in ./docs/cli.md §Multi-File Formatting); an explicitly named file argument bypasses the ignore files (but not the extension check — an unsupported extension is an argument error). `--list` prints the discovered in-scope files without formatting (path mode only; an empty scope exits 0). Files format in parallel; `--jobs N` overrides the default worker count of `min(logical CPUs, ceil(1.5 × physical cores))` (rationale in docs/cli.md — the work doesn't scale onto SMT siblings). Exit codes: 0 clean, 1 would-change (`--check`, which also works with `--content`/`--stdin`), 2 errors; missing path args fail the run upfront, per-file and traversal errors report and continue.

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
deno task doctor         # one-pass setup check: runtimes, pins + checkout alignment, node_modules freshness, oracle checkouts, corpus, build artifacts. Exit 1 only on MISLEADING state (pin drift, skew, stale deps); absences are warnings (--strict promotes them) — except the explicitly optional experimental-typechecker tier, informational at any strictness (a BROKEN checkout there still warns)
deno task typecheck      # cargo check
deno task typecheck:js   # deno check over the bench harness, scripts/ + the tsv_debug sidecar (the JS/TS cargo can't see).
#                          NOT in `check` — needs `deno task bench:install`, and CI installs no node_modules
deno task typecheck:scripts # deno check over scripts/ alone — node-modules-free, so this one DOES gate in `check`
#                          (nothing else typechecks the release scripts, and `deno run` doesn't).
#                          `scripts/doctor.ts` is the one exclusion: its corpus probe reaches the bench node_modules
deno task test           # cargo test
deno task lint           # cargo clippy
cargo fmt                # format Rust code
tsv format .             # format the repo's own TS/JS — tsv formats itself (`--check` in the gate)
# cargo fmt (Rust) and tsv format (TS/JS) are the repo's ONLY autoformatters, and they partition
# it; markdown and JSON stay hand-maintained. Never run `deno fmt` or `prettier` on the repo:
# tsv ships NO config for them, so they'd reformat to their own defaults and churn every file
# (the fixture/corpus prettier oracles pass options inline, so they're unaffected).
# `tsv format` on a DIRECTORY is safe — the root `.formatignore` prunes tests/fixtures/ and
# tests/fixtures_compile/, deliberately not format fixed points. Never name a file under tests/
# explicitly: an explicit file argument bypasses the ignore files and would destroy the fixture's claim.

cargo test --workspace strict_reserved_words_are_binding_names  # run specific test by name
cargo test --workspace --test fixtures_tests           # fixture validation tests
cargo test --workspace --test cli_tests                # CLI integration tests
```

### Fixtures (Rust + Deno-based)

All `fixtures:*` tasks accept positional patterns (multiple = OR); `fixtures:validate` and `fixtures:update:parsed` also accept `--list`, and `fixtures:validate` `--prettier-only`.

```bash
deno task fixtures:list              # list all fixtures (read-only)
deno task fixtures:init <dir>        # create/reinit a fixture (alias of `tsv_debug fixture_init`; --content/--stdin/--force)
deno task fixtures:validate          # validate (use during fixture work; --prettier-only skips our parser/formatter)
deno task fixtures:update            # regenerate expected.json + output_prettier.svelte (source of truth)
deno task fixtures:update:parsed     # regenerate expected.json only (run when parser changes)
deno task fixtures:update:formatted  # regenerate output_prettier.svelte only
deno task fixtures:audit             # audit _prettier_divergence fixtures (diagnostic; --all for every fixture)
deno task fixtures:ts-audit          # which input.ts fixtures genuinely need .ts vs could be .svelte (alias of `ts_fixture_audit`)
deno task compile:fixtures:init      # create/reinit a compile fixture (oracle-compiles + canonicalizes; tests/fixtures_compile)
deno task compile:fixtures:validate  # compile fixtures: oracle freshness + expected idempotence + ours parity, all gating. Preflights `deno task conformance` — the ONLY place oracle freshness is graded, since the sidecar-free slice that also gates in cargo test compares tsv to the COMMITTED file and stays green when both drift away from the oracle together
```

**Standing audit gates** — full reference ./docs/audits.md: what each proves, blind spots, flags, and where it gates (its overview table maps every task). Read the relevant section before running or modifying an audit. RATCHET audits grade against a committed known-bug snapshot (`*_known.txt`); each has an `:update` task that re-pins after a fix and refuses a narrowed run. Everything below gates in `deno task check` unless noted.

```bash
deno task conformance:audit          # doc/fixture integrity: divergences cataloged, every Markdown link in the repo resolves, divergence READMEs back-link, no catalog-family drift
deno task conformance:audit:compiler # compile-fixture divergence integrity + checklist ↔ `Refusal` drift
deno task variants:audit             # `_compact`/`_spaces` variant DIRECTION: the pair is two opposite claims about one input, so a `_compact` may never widen a whitespace gap and a `_spaces` may never empty one (bare suffixes only — a qualified name like `unformatted_ours_hug_spaced` is the escape hatch for a deliberately bidirectional variant)
deno task canonicalize:audit         # canonicalize_js idempotence + output validity + comment preservation
deno task pins:audit                 # canonical-oracle PIN AGREEMENT, a repo fact: sidecar.ts VERSIONS + npm: imports, benches/js/package.json, actor.rs acorn import-map, and the sidecar deno.lock must be identical. The lock also pins what no literal names — the oracle's own transitive deps (`LOCKED_TRANSITIVE`: esrap, which PRINTS svelte's compiled JS)
deno task pins:lock                  # REGENERATE the sidecar lockfile (crates/tsv_debug/src/deno/deno.lock) after a canonical pin bump — the lock is frozen at runtime, so this is the only way it moves; `--check` reports drift without writing. Not a gate (resolution depends on what the registry currently offers). `--allow-fresh` opts past deno's 24h `minimumDependencyAge` supply-chain window, needed ONLY to take a version published in the last day — a lock made with it reproduces flag-free once that version ages out
deno task pins:audit:checkouts       # checkout ALIGNMENT, an environment fact: a PRESENT ../svelte or ../acorn-typescript checkout must match its pin (absent → skipped); warn-only commit drift. Gates in `deno task conformance`, reported by doctor — deliberately NOT in check (nothing there reads the checkouts)
deno task format:audit               # tsv formats its own TS/JS (`tsv format --check .`); fails on a would-change file (exit 1) OR a parse error (exit 2)
deno task docs:audit                 # doc-comment `[link]`s resolve — rustdoc, doc lints DENIED, private items, `--all-features`; a dead link is a STALE DOC
deno task scan:audit                 # no new raw find/rfind/match_indices substring scans over source
deno task fanout:audit               # no super-linear doc-node rebuild fanout (per-layout-candidate blowup)
deno task roundtrip:audit            # format(tests/fixtures) must reparse — pure-Rust tripwire, real yield on external corpora
deno task roundtrip:audit:prettier   # the same audit over the pinned prettier suites — `check`'s ONLY non-format-stable corpus, so the only leg there that can reach a valid→unreparseable regression. ~0.1 s; warn-skips when `../prettier` is absent (sibling checkout, so `check` still runs on a bare clone)
deno task binding:audit              # comment↔token re-binding (HARD fails the gate, SOFT informational)
deno task authoring:audit            # authoring-independence over Svelte boundary whitespace: one fixed point per document
deno task fuzz:audit                 # seeded mutational fuzzer (fixed seed/iterations): no-panic + idempotency + structural reparse
deno task swallow:audit              # `//` line comment swallowing following output-line content (also over real code via audit:corpus)
deno task comments:audit             # print-once comment ledger: DROPPED / DOUBLE-PRINTED comments
deno task gaps:audit                 # gap-injection RATCHET, ~17 s (./docs/gap_audit.md; also :update and :rank for triage)
deno task blanks:audit               # blank-line injection RATCHET + the blank-DROP absorb pin (a new kind of silently-eaten blank fails), ~30 s (./docs/blank_audit.md; also :update)
deno task fabrication:audit          # blank-FABRICATION on pristine seeds — the F1-blind counterpart to blanks (ratchet born EMPTY; also :update)
deno task census:audit               # comment CENSUS: raw input-vs-output trivia multisets per language bucket (own scanners, never parse().comments) — catches parse-time drops/merges/rewrites the ledger can't see (also :update)
deno task width:audit                # print-width RATCHET: a new KIND of over-width output line — the ONLY gate that measures a column. ⚠️ NOT a debt list (sanctioned overruns are real); also :update
deno task ignore:audit               # `prettier-ignore` honoring RATCHET: honoring, second-pass stability, freeze scope, trailing inertness (also :update)
deno task razor:audit                # print-width RAZOR SWEEP: pads a text word to walk each Svelte seed across column 100, grading F1 + the stray line-head boundary space at every width — the ONLY instrument that varies WIDTH, and the only one that can see a mangled form that is its own fixed point (pure Rust; ./docs/audits.md)
deno task render:audit <paths>       # render-equivalence over REAL Svelte (sidecar — NOT in check; release-gated leg of `deno task conformance`)
deno task idempotency:sweep          # F1 idempotency sweep over the real-code corpus (minutes — NOT in check; conformance cadence)
deno task audit:corpus               # the standing content-loss/robustness bundle over REAL code (publish Step 3c; NOT in check)
deno task compile:corpus:compare     # compile-parity wide net over real repos + Svelte suites (sidecar, on demand; ./docs/compile_tooling.md)
deno task compile:validation         # validation-suite RATCHET over Svelte's compiler-errors + validator suites (sidecar, on demand; :update re-pins, never a MISMATCH; ./docs/compile_validation_ratchet.md)
deno task compile:fuzz               # differential compile fuzzer over feature cross-products — a discovery tool, currently RED by design (sidecar, on demand; ./docs/compile_tooling.md)
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

- `tsv_ffi` (C ABI) — any FFI (Deno, Python, etc.); output: `libtsv_ffi.so` / `.dylib` / `.dll`
- `tsv_wasm` (wasm-bindgen) — browser, Deno, Node; output: `.wasm` module (format / parse / all variants via cargo features)
- `tsv_napi` (napi-rs) — Node.js / Bun native addon (`libtsv_napi.*`, loaded via `process.dlopen`). Builds with the `napi` profile (`release` + `panic = "unwind"` → `target/napi/`; every export is `catch_unwind`, so a panic throws a JS error instead of aborting the host). The npm surface — the bare `@fuzdev/tsv` loader over per-platform `@fuzdev/tsv-<triple>` packages, wasm-API-parity by contract; each platform package also ships the `tsv_cli` binary, which the loader's `tsv` bin execs (`npx tsv` = the native CLI) — is staged by `deno task build:napi:packages` (which also builds `tsv_cli --release`) and tested per OS by `test:napi:npm`; the cross-platform **publish** runs through the tag-triggered `.github/workflows/release_napi.yml` (see [Publishing](#publishing)), and the native set is expected to eventually subsume the WASM native path. See ./crates/tsv_napi/CLAUDE.md §The npm packages.

`tsv_wasm` produces three npm packages from one crate via the `format` + `parse` cargo features (default = both): `@fuzdev/tsv_format_wasm`, `@fuzdev/tsv_parse_wasm`, and `@fuzdev/tsv_wasm` (everything + the `tsv` CLI). Each variant has its own output directory.

```bash
# Build bindings
deno task build:ffi                  # C FFI, full build → target/release/libtsv_ffi.so
deno task build:ffi:format           # C FFI, format-only (size only) → target/ffi-format/release/
deno task build:ffi:parse            # C FFI, parse-only (size only) → target/ffi-parse/release/
deno task build:napi                 # N-API addon (napi profile: release + unwind) → target/napi/
deno task build:napi:packages        # + staged npm packages (loader + host platform pkg) → crates/tsv_napi/pkg/
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
- `@fuzdev/tsv_parse_wasm` — parse only; bundles hand-maintained `tsv_ast.d.ts` (`crates/tsv_wasm/types/`) + the pure-JS `no-locations` line/column reconstruction helper (`crates/tsv_wasm/npm/locations.js` + `.d.ts`)
- `@fuzdev/tsv_wasm` — full tool (both features); bundles the above and ships the `tsv` bin (`crates/tsv_wasm/npm/cli.js` — `format` + `parse` mirroring `tsv_cli`'s flags/exit codes; `node:util` `parseArgs`, zero deps, single-threaded)

The **N-API set** — the bare `@fuzdev/tsv` loader + five `@fuzdev/tsv-<triple>` platform packages (see ./crates/tsv_napi/CLAUDE.md §The npm packages). Each platform package ships **two binaries**: the addon (`tsv_napi.node`, `napi` profile) and the real `tsv_cli` binary (`tsv`/`tsv.exe`, plain `release` profile), and the loader's `tsv` bin is a dispatcher (`bin.js`) that execs that binary — `npx tsv` on the native set IS the native CLI (real `--jobs`, parallel discovery), the esbuild/biome shape — falling back to the shared `cli.js` JS mirror (one source with `@fuzdev/tsv_wasm`, bound to the native engine via its own `./index.js` import). The set is staged by `deno task build:napi:packages` and publishes through the tag-triggered `.github/workflows/release_napi.yml`, **never** through the single-machine `scripts/publish.ts`. The v\* tag `publish.ts` pushes triggers the 5-target matrix: container-pinned builds of both binaries (glibc **2.28 floor** on the gnu rows via almalinux:8 — Node's own binary floor; musl in rust:alpine with `-crt-static` off), each gated by the measured glibc-floor / musl-purity checks over both artifacts, per-artifact size bounds (`deno task validate:napi`, tight like `validate:artifacts`), and the npm-shape test over the real artifacts (in node:alpine for musl); then an idempotent platforms-then-loader publish (`scripts/publish_napi.ts` — refuses partial sets, re-arms the CLI binaries' executable bit that artifact transport drops; `deno task publish:napi --dry-run` runs it locally — no `--` separator, which deno task forwards literally and the script's `parseArgs` rejects — where a local run proves staging + the refusal logic and then **stops at the partial-set refusal by design**, since local staging holds only the host platform; the full-set rehearsal is the `workflow_dispatch` dry run). `workflow_dispatch` runs the whole matrix as a dry-run rehearsal by default (`dry_run=false` is the recovery path for a failed tag run — dispatch it **on the tag**, since the tag↔version assertion keys on the ref, and a branch dispatch publishes without it and says so), and a weekly cron force-dry-runs it as a rot watch. Auth is a granular `NPM_TOKEN` secret from the bootstrap releases (trusted publishing is only configurable on packages that already exist on the registry); switching the workflow to npm trusted publishing (OIDC) is the pending follow-up.

A types-only `@fuzdev/tsv_ast` package is deferred — `import type` from `tsv_parse_wasm` is zero-runtime-cost; reconsider when a real consumer appears. The bare `@fuzdev/tsv` is **taken by the N-API set** above — tsv's native distribution ships under one name: the platform packages carry the `tsv_cli` binary beside the addon (un-PGO'd `release`, matching the benched artifact; PGO stays a deliberate future re-baseline).

Version source of truth: `Cargo.toml` `[workspace.package] version` (read directly by `wasm-pack`). No root package.json, no changesets; all published packages move together.

Package shape: wasm-pack `web` target, then `scripts/patch_npm_package.ts` adds a Node/Bun entry (sync auto-init), a browser entry (guarded `await init()`), `index.d.ts`, conditional `exports`, npm metadata, and the variant README. The export list is extracted from the generated JS, so new `lang_bindings!` languages flow through automatically.

`scripts/publish.ts` orchestrates the release end to end (preflight → bump → check → conformance:all → audit:corpus (Step 3c) → build npm packages + deno bundles → verify → artifact validation: size bounds + Deno smoke + Node tests → idempotent npm publish → git commit + tag + push), printing a wasm size summary. It stamps CHANGELOG.md's `## Unreleased` section into the released version — that section must be non-empty and carry a `<!-- bump: <level> -->` marker matching `--bump` (required in both places; a fresh empty `## Unreleased` is seeded on stamp). Agents don't touch `CHANGELOG.md` (see [Committing](#committing)). A failed wetrun is resumable at **every** step, the git finalize included — the retry sentinel is removed only once the push lands, so a rejected push (or a failed commit/tag) re-runs with `--wetrun` and no `--bump`, publishes nothing twice, and finishes the tag.

**Conformance gates (Step 3b).** The external-oracle correctness gates (see [Corpus Comparison](#corpus-comparison)) run here via `deno task conformance:all`; skipped by `--no-check`. The step preflights the oracles (`../svelte`, `../acorn-typescript`, `../typescript`, `../test262` checkouts + the `benches/js` `node_modules` sidecar): a **`--wetrun` FAILS** when any is missing (releasing without gates requires the explicit `--no-check`); a dry-run warn-and-skips, re-warned in the final summary. `deno task doctor` checks the same setup ahead of time. Only the CSS-WPT harvest stays manual. A `corpus:compare:format` SAFETY hit is self-verified in-run (the native format re-runs and must reproduce byte-identically), so treat it as real; FFI nondeterminism surfaces as a loud `native format nondeterminism` per-file error instead (./benches/js/CLAUDE.md §Known Issues). A caught **panic** hard-fails either corpus tool on every run — the corpus profile catches it where a shipped artifact would abort the host, so it must never grade as one more per-file error.

```bash
deno task publish                        # dry-run: validate everything, no mutation
deno task publish --wetrun --bump patch  # release: bump + publish + git finalize (--bump required, must match CHANGELOG marker)
deno task publish --wetrun               # resume a failed wetrun (sentinel retry only)
# Flags: --bump patch|minor|major, --no-check, --no-git
deno task test:npm[:parse|:all]          # builds the npm package, then runs Node tests against it (:all includes CLI tests; `:run` suffix skips the rebuild)
deno task test:napi:npm                  # stages the napi loader + host platform package, then runs Node tests against the packaged shape (`:run` skips the rebuild)
deno task validate:artifacts             # tight wasm size bounds + Deno smoke of all built bundles (fails if nothing is built)
```

`scripts/validate_artifacts.ts` holds deliberately tight (~±8%) size bounds — a legitimate binary size change fails the publish until the constants are updated, keeping size moves visible and intentional.

**TS type maintenance**: `crates/tsv_wasm/types/tsv_ast.d.ts` is hand-maintained. Any PR changing the wire JSON a writer emits (`crates/tsv_*/src/ast/convert/write*`) must also update the `.d.ts`. Drift is caught by `deno task check:ast-types` (part of `deno task check`). Per-field checklist: ./crates/tsv_wasm/CLAUDE.md §TS type maintenance.

### Corpus Comparison

Compare formatting against Prettier, and parse output against the canonical parsers, on real codebases. Full runs enforce **pinned expected counts**: the format `--all` counts hold over the reproducible subset (version-pinned framework + prettier checkouts; live dev repos are a non-gating WARN, SAFETY still gates every file); parse `compared` counts + committed fixtures are live-growth minimums. See `benches/js/lib/gate_counts.ts` and ./docs/gate_counts.md.

```bash
deno task corpus:compare:format ../some-project  # single project, or --all for the gates corpus (real repos + prettier suites)
# Options: --explain (patterns matched), --summary (compact), --json (stats + safety/partial/unknown/error lists; logs → stderr)

deno task corpus:compare:parse --all   # deep-diff parse ASTs vs acorn-typescript/svelte/parseCss
# Options: --multibyte-only, --filter <lang>, --limit <n>, --json

deno task conformance:svelte-fixtures  # tsv's Svelte parser vs Svelte's own test suite (../svelte); oracle = the live modern parser.
# Verdict parity gates (over-rejections must be SANCTIONED or a tracked KNOWN_GAP, else exit 1); AST-shape diff is report-only triage.
deno task conformance:ts-fixtures      # tsv's TS parser vs acorn-typescript's test suite (the adversarial TS edge-case corpus).
# Strict: a missing ../acorn-typescript (0 scanned) FAILS — publish Step 3b's preflight is the tolerance point. Both fixtures
# gates freshness-check their ledgers on full runs (a stale sanction/known-gap entry fails) and warn on checkout↔npm version skew.
deno task conformance:ts-repo          # tsv's TS parser vs the tsc corpus (ALL of ../typescript/tests/cases, ~13.7k
# single-file .ts — not the conformance/parser subtree); oracle = tsc's
# .errors.txt baselines (a TS1xxx code = tsc's parser rejects). Buckets accept/reject parity, over-acceptances, tracked gaps.
# A missing/PARTIAL ../typescript checkout, or an empty scan, FAILS. See ./benches/js/CLAUDE.md.
# The three gates above accept: -v, --json, <subtree>.

deno task conformance                  # pre-release aggregate: preflights pins:audit:checkouts +
# compile:fixtures:validate (the ORACLE-FRESHNESS leg — `check` runs only the sidecar-free slice of the compile
# fixtures, which grades tsv against the committed file and so cannot see the oracle itself moving), then the three
# gates above + corpus:compare:parse --all +
# corpus:compare:format --all in ONE process (benches/js/conformance.ts; oracles load once, fail-fast, FFI built once),
# then render:audit over the version-pinned checkouts (a subprocess — drives its own sidecar). The format leg's prettier
# calls ride a content-addressed cache (benches/js/lib/prettier_cache.ts; TSV_PRETTIER_CACHE=0 disables).
deno task conformance:test262          # tsv's JS parser vs test262 POSITIVES (pure Rust, `test262 --gate`); negatives
# (the deferred early-error frontier) are reported, not gated. Exact POSITIVE_PASSED_PIN in the command.
deno task conformance:all              # the full drop-in gate = `conformance` (5 FFI legs) + `conformance:test262`.
# What publish Step 3b runs. CSS-WPT harvest stays manual.

deno task divergence:audit         # audit divergence pattern coverage (--json)
deno task corpus:stats             # corpus/candidate-dir sizes + language + degenerate-case stats (diagnostic; ./benches/js/CLAUDE.md)
```

The corpus comparison builds with `--profile corpus` (optimized + `panic = "unwind"`, no LTO — panics in our code are caught and reported; also the single build world every `deno task check` audit shares, trading LTO for build time, measurably free at runtime per the profile's comment in `Cargo.toml`). Benchmarks use `--release` (panic=abort, LTO) for maximum performance — except the N-API artifact, which builds with the `napi` profile (`release` + `panic = "unwind"`, the shipped panic contract), so its bench rows measure the artifact that actually ships.

Divergence detection identifies known differences documented in the `conformance_prettier*.md` family (safety checks, pattern detection, traceability). See ./benches/js/CLAUDE.md and ./docs/divergence_detector.md.

### Benchmarks

**Cross-runtime.** One harness runs under **Deno, Node, and Bun** — each emits its own runtime-labeled report (`report.{deno,node,bun}.{json,md}`), never merged; `deno task bench:compose` folds them into the combined `report.{json,md}` (what tsv.fuz.dev consumes). The native row is **FFI** under Deno, **N-API** under Node/Bun; everything else is shared runtime-neutral code. Full detail: ./benches/js/CLAUDE.md §Cross-Runtime.

**Perf vs conformance surfaces.** `bench:perf` measures a **real-world-only** corpus (app + framework source) — the throughput headline; every in-scope tool must fully process every file or the run fails (`benches/js/lib/perf_omit.ts`), so coverage is 100% by construction. `bench:conformance` measures per-tool **parse coverage** over a **disjoint, fixtures-only** corpus (prettier suites + svelte compiler tests + the wpt-css/test262/tsc-corpus harvests; the Svelte and tsc sets exclude what their own canonical parser rejects) — **coverage-only and node-only by design** (no timed phase; runtime-invariant). Its report splits coverage **per corpus source**, since a group's aggregate blends corpora that answer different questions (`parse/typescript` is mostly test262, i.e. ECMAScript) and each filtered set scores its own oracle at 100% by construction. Coverage counts accepts and so can only reward permissiveness; the opposite axis has a task per surface, both read inverted as profiles rather than gates — `deno task ts-repo:over-acceptance` (per-tool accepts over the files tsc's parser rejects) and `deno task css:over-acceptance` (over the files `parseCss` rejects; its reject-count pin is what makes the CSS reference row's grammar moving visible, since that surface is deliberately unfiltered). `deno task bench` = perf across all three runtimes + compose + the node coverage run. The correctness gates keep their own unchanged corpus scope. Full detail: ./benches/js/CLAUDE.md §Corpus.

```bash
# One-time: install the harness's npm deps (package.json is the source of truth; both runtimes
# share node_modules). Re-run after a dep bump or a plain `npm install` (which prunes the
# oxc-parser-wasm binding — see benches/js/CLAUDE.md).
deno task bench:install

deno task smoke         # fast sanity check that every formatter+parser produces output (also smoke:node / smoke:bun)

# Benchmarks build the runtime's artifacts automatically. `bench` runs ALL three runtimes and
# fails if node or bun is missing — Deno is the only hard dep; otherwise run the per-runtime tasks.
deno task bench         # full refresh: perf ×3 + compose + node conformance COVERAGE (needs node AND bun)
deno task bench:perf    # perf surface only: all three runtimes + compose
deno task bench:deno    # Deno only (no node/bun needed)
deno task bench:node    # Node only
deno task bench:bun     # Bun only (reuses the Node artifacts)
deno task bench:compose # fold existing per-runtime reports → combined report.{json,md}
deno task bench:deno:run   # run without rebuilding (also :node:run / :bun:run; aborts on stale artifacts)

# Conformance surface: per-tool parse COVERAGE → report.conformance.node.{json,md} (entries carry null timing)
deno task bench:conformance        # harvest + build:bench:node + coverage run
deno task bench:conformance:run    # skip harvest + rebuild (freshness-guarded)
deno task bench:harvest            # regenerate the wpt-css + test262 + tsc-corpus + svelte-reject + svelte-styles caches
                                   # (first four freshness-stamped, --force after harvest-logic changes; svelte-styles always re-harvests, ~2 s)

deno task bench:deno:run -- --verbose   # per-file skip detail (counts always shown; paths/errors opt-in)

# Env vars (any runtime): BENCH_LIMIT, BENCH_FILTER, BENCH_DURATION, BENCH_WARMUP, BENCH_MODE,
# BENCH_CORPUS, BENCH_STALE_OK, BENCH_FORCED_ASYNC — semantics + defaults in ./benches/js/CLAUDE.md
BENCH_FILTER=zzz BENCH_LIMIT=10 deno task bench:deno:run
```

**Prerequisites**: `cargo install wasm-pack` + `deno task bench:install` once (the install needs npm/Node). Beyond that **Deno is the only hard dependency**; Node ≥ 22.18 (native TS type-stripping) for `bench:node`, Bun for `bench:bun` — the aggregate `bench` needs both and fails fast if either is missing.

Compares: canonical (prettier + svelte/compiler), native (FFI under Deno / N-API under Node+Bun), WASM, and alternatives (oxc-parser — an N-API row plus a separate wasm32-wasi row — oxfmt, biome-wasm, dprint-wasm — the engine `deno fmt` runs, TS/JS only — malva-wasm, dprint's CSS plugin over the same formatter host (CSS only, enforced by the plugin), `tsc` itself, parse-only and conformance-surface-only (the language's definition, not a peer: its parser is error-recovering, so an accept there means zero `parseDiagnostics`), yuku-parser, a Zig TS/JS parser shipped as both an N-API and a WASM binding, parse-only and payload-matched to oxc; its lazy `parse()` and error-tolerant parser are corrected for in `benches/js/lib/yuku.ts`, swc, parse-only over TS/JS on both surfaces (its own AST dialect, so oxc-class payload disclosure; `decorators` must be enabled explicitly and its goal axis is `isModule`), postcss, parse-only over CSS — the parser behind prettier's CSS printer, and the only kind available there since no Rust CSS parser exposes an AST to JS — and rsvelte's Svelte **parser** via its N-API addon, two rows on `parse/svelte` (plain, mechanism- *and* payload-matched to `tsv-json`, plus a `skipExpressionLoc` variant named for its option because that reduction is not tsv's), the first third-party engine on a surface that otherwise holds only the oracle and tsv itself). `rsvelte-fmt` (Svelte only) is a **coverage-only** row — an accept rate with no timing, since it ships no in-process API and a per-file subprocess row would rank process spawn rather than format work; its end-to-end CLI numbers live in the separate hyperfine comparison published on tsv.fuz.dev. See ./docs/benchmarks.md §Coverage-only rows. Results: `benches/js/results/report.<runtime>.{json,md}` (committed; every row carries a `runtime` field) + the combined `report.{json,md}`. To publish to tsv.fuz.dev: `npm run update-benchmarks` in ../tsv.fuz.dev. See ./benches/js/CLAUDE.md.

### Performance Profiling

```bash
cargo run --release -p tsv_debug -- profile ../zzz/src/lib        # profile a directory
cargo run --release -p tsv_debug -- profile file.ts --iterations 20  # more iterations
# Also: --json (machine-readable)

cargo run --release -p tsv_debug -- json_profile ../zzz/src/lib   # parse vs wire-JSON write timing

cargo run --release -p tsv_debug -- compile_profile tests/fixtures_compile  # Svelte compile vs the format wall
```

For function-level hotspots, use `perf` with the `profiling` cargo profile:

```bash
cargo build --profile profiling -p tsv_debug
perf record --call-graph=dwarf -- target/profiling/tsv_debug profile ../zzz/src/lib
perf report --stdio                              # function-level hotspots
perf annotate --stdio -s fits_with_lookahead     # line-level within a function
```

See ./docs/performance.md.

## Configuration

**Non-configurable by design.** Formatting options are fixed at Prettier's defaults except the list below, and cannot be changed — no config files, CLI flags, or runtime options, and none are planned (a narrower option set may be revisited far down the road, but the 0.x contract is no configuration at all).

**The one carve-out is file *scope*, not style.** Authoritative rules + edge cases (parent-directory rule, re-include idiom, unreadable ignore files, warnings): ./docs/cli.md §Multi-File Formatting. Core: `tsv format`'s discovery is gitignore-aware with two regimes keyed on `.git`. Inside a git repo the **format root** (the scope boundary — derived from the argument, never the cwd) is the repo root, a hard stop for the upward walk; discovery honors `.gitignore`, then `.formatignore` (tsv's native file; its `!` can re-include a gitignore'd path), then `.prettierignore` (drop-in compat; the fallback in any directory with no sibling `.formatignore`), all hierarchical, plus the always-skipped safety nets (`.git`, `node_modules`, `.sl`, `.hg`, `.svn`, `.jj`). Outside a repo only `.formatignore` is read, from the filesystem root down (so `~/.formatignore` is global config for loose files). Because the boundary is found by walking up, formatting a subdirectory equals formatting it via an ancestor. A `.gitignore` in scope turns the built-in heuristic (hidden dirs + `dist`/`build`/`target`) **off**. Scope only, never style; an explicitly named file argument is always formatted, provided its extension is one tsv formats (naming an unsupported one is an argument error, not a TypeScript parse). The matcher is the `tsv_ignore` crate (`IgnoreStack`); the per-directory prune decision (heuristic, safety nets, shadow warning) is `tsv_discover` — both shared with the JS CLI and the VS Code extension via WASM, and with the native `@fuzdev/tsv` via N-API (`classify_dir` / `should_format_file` / `heuristic_shadow_warning` / `is_path_pruned`), so every surface agrees by construction.

Settings that diverge from Prettier's defaults (everything else, e.g. tabWidth=2, matches):

- `printWidth` (100) — wider than Prettier's 80
- `useTabs` (true), `singleQuote` (true)
- `trailingComma` ('none') — no trailing comma even when a list breaks across lines; with useTabs + singleQuote this matches the Svelte project's own `.prettierrc`

**Measuring line widths**: use `cargo run -p tsv_debug line_width <file>` — never `wc -c`, which counts bytes, not visual chars (a tab is 1 byte, 2 visual chars). `compare` also shows line widths on changed lines.

### Internal Configuration (Rust Library Only)

There is no runtime configuration. Print width / tab width / indent are compile-time `pub const`s in `tsv_lang::config` (`PRINT_WIDTH`, `TAB_WIDTH`, `INDENT`), read directly by the renderer — not threaded through any signature. Quote preference is likewise hardcoded (single quotes) in `tsv_lang::printing` — the `optimal_string_quote` tie-break that `format_string_literal` applies. The doc-builder unit tests exercise smaller widths via the internal `RenderConfig` seam (`doc::render_config`, `pub(crate)`), never at runtime.

One type carries genuine per-input *state* (not configuration), threaded only where it varies: `tsv_lang::EmbedContext { base_indent_offset, first_line_offset, suffix_width, mode: LayoutMode, jsdoc_cast_cannot_hang, root_sequence_indents }` — embedding state for nested formatting (CSS in `<style>`, Svelte template expressions). `LayoutMode { Standalone, Embedded }` controls the expression-ROOT binary indent style (nested expressions format context-free); `root_sequence_indents` is its sequence counterpart, set by the Svelte **block head** alone (the one braced head prettier never width-wraps). The three width fields are read at **render** (they act only on the context passed to an `arena_print_doc_*` call); on a `build_*_doc` call only the three build-time fields — `mode`, `jsdoc_cast_cannot_hang` and `root_sequence_indents` — survive, so a width set there is inert.

TypeScript formatting is identical for standalone `.ts` and Svelte-embedded TS, so there is a single entry point: `tsv_ts::format(&ast, source)`.

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
│   ├── tsv_svelte_compile/ # Svelte→JS compiler (Svelte's compile() oracle) + JS canonicalizer; consumed by tsv_debug — no shipped artifact links it
│   ├── tsv_check/   # EXPERIMENTAL TypeScript binder + checker — may never ship (consumed only by tsv_debug)
│   ├── tsv_cli/     # Production CLI (binary: tsv) - pure Rust
│   ├── tsv_debug/   # Dev utilities (binary: tsv_debug) - uses Deno
│   ├── tsv_ffi/     # C FFI bindings (Deno's native path)
│   ├── tsv_wasm/    # WASM bindings (the 3 published npm packages; bundles types/tsv_ast.d.ts + npm/locations.js; npm/cli.js is the tsv bin)
│   └── tsv_napi/    # N-API bindings (Node/Bun native path; npm/ is the @fuzdev/tsv loader source)
├── scripts/         # Publish orchestrator, npm package patcher, Node artifact + N-API tests, AST type drift check
├── benches/js/      # Cross-runtime benchmark + conformance harness (Deno/Node/Bun)
├── tests/           # Integration tests (parser, formatter, CLI)
│   ├── fixtures/    # Test fixtures organized by language/feature
│   └── fixtures_compile/ # Compiler fixtures (input.svelte + canonicalized oracle expected_server.js + expected.css) — separate tree so parser/formatter fixture counts stay unperturbed
└── docs/            # Documentation (fixtures, cli, architecture, etc.)
```

**Crate pattern** (tsv_ts, tsv_css, tsv_svelte):

- `lib.rs` - Public API: `parse()`, `format()`, `convert_ast_json_bytes()`
- `ast/` - Internal AST + the conversion layer (the wire-JSON writer)
- `lexer/` - Tokenization
- `parser/` - AST construction
- `printer/` - Code formatting (uses doc builder from tsv_lang)
- `escapes` - Language-specific escape handling (tsv_ts's at `lexer/escapes.rs`, tsv_css's at `escapes.rs`; Svelte delegates to TS/CSS)

`tsv_ts` and `tsv_css` also export embedding APIs for `tsv_svelte`: `parse_embedded` and embedded-formatting variants; `tsv_ts` additionally exports the `build_*_doc` functions.

### Conformance

**Comment position is preserved by default — but the rule is principled, not absolute.** A core tsv stance and the single largest category of deliberate Prettier divergence: a comment's placement usually communicates what it refers to, so tsv keeps comments where the author wrote them. Prettier routinely relocates comments across syntactic boundaries and in doing so often **loses information** — two comments merging onto one line (the second `//` becoming text), or reordering. tsv treats such a boundary as semantic and holds the comment in place.

The line tsv draws: **preserve when the position carries authorship signal, or when relocating would lose information** (the common case). But tsv will **deliberately trail** a same-line line comment past a *pure separator* when doing so is **lossless and the position carries no signal** — e.g. a comment between a list element and its comma (`A // c⏎, B` → `A, // c`): the comma is structure, the comment trails the element either way, and per-element line breaks keep even multiple comments distinct, so tsv matches Prettier. That carve-out is a deliberate choice, **not** a gap to close. (Contrast the name→`=`/`:`/`?` binding cases, where two comments *would* collide on one trailing line — there tsv preserves + continuation-indents to stay lossless, diverging from Prettier's merge.)

The **opening-delimiter** rule is now uniform across the printer: a `//` the author glued to an opening delimiter keeps that line at every one of them — `fn(`, `new(`, `import(`, `function f(`, `[`, `{`, `Array<`, a retained type paren shell's `(`, a required operand pair's `(` (an assignment target, an instantiation head, a non-null / sealed-chain shell), a unary comment-holder's `(`, `return (` / `throw (`, the aligned union-member object's `{`, and every statement header (`if` / `while` / do-while / `switch` / `catch`, all three `for` spellings). **Two** emitters state it — `Printer::split_open_delimiter_glued_run` (the paren shells, the operand pairs, the unary holder, the restricted-production hang, every statement header) and `Printer::delimiter_line_comment_prefix` (the container and call families, and the aligned object's `{`) — and the author blank *below* the pulled comment **survives at all of them**, one value for the whole family via `Printer::push_delimiter_glued_blank` (the pull moves the comment's line, not its membership; prettier keeps the blank everywhere too). A blank *above* the comment sits against the delimiter and stays erased. See ./docs/comments.md §The delimiter-line question. Prettier's own answer is **not** uniform: it un-glues at most of them, **glues** at for-in / for-of, and relocates the comment out of the parens entirely at do-while — so each is a cataloged divergence. Do not confuse any of them with the *sanctioned* union/intersection carve-out in §Comment Handling below (a non-last member's redundant paren shell strips and its deferred `//` flushes at the per-member break, `Printer::type_member_separator_follows`) — that one is a deliberate lossless choice the catalog owns. When a fix changes comment handling, default to preserving position; matching Prettier is fine only when trailing is lossless and the position carries no signal — otherwise add a `_prettier_divergence` fixture. Full principles: ./docs/conformance_prettier.md §Comment Position Philosophy; the divergence catalog: ./docs/conformance_prettier_ts_comments.md §Comment relocation.

- ./docs/conformance_prettier.md - Where we differ from Prettier (and why) — the shared frame;
  the per-language catalogs are ./docs/conformance_prettier_css.md,
  ./docs/conformance_prettier_svelte.md, ./docs/conformance_prettier_ts.md,
  ./docs/conformance_prettier_ts_comments.md, and ./docs/conformance_prettier_ignore.md
- ./docs/conformance_svelte.md - Where we differ from Svelte (and why)
- ./docs/conformance_svelte_compiler.md - Where we differ from Svelte's compiler (expected to stay empty — a safety valve, not a budget)

## Fixtures

See [Development Philosophy](#development-philosophy-test-driven-development-with-fixtures) for the TDD workflow.

### Fixture Protection Rules

**Sources of truth**: Prettier and Svelte's parser. Fixtures record what these tools produce.

**When a fixture test fails:**

1. **Verify the fixture** against the sources of truth:

   ```bash
   cargo run -p tsv_debug compare <fixture>/input.svelte          # vs prettier
   cargo run -p tsv_debug canonical_parse <fixture>/input.svelte  # vs Svelte's AST
   ```

2. **Fixture matches prettier/Svelte** → the fixture is correct; fix our code to match.
3. **Fixture doesn't match** → it may be outdated: `deno task fixtures:update <pattern>`.

**CRITICAL: Never modify fixtures to work around our bugs.** Fix the code, not the fixture. Prohibited without verifying against the sources of truth: modifying `input.svelte` to avoid edge cases, removing `unformatted_*` cases, changing `expected.json` to match incorrect output, any fixture change that hides a bug.

**When our formatter differs from prettier:**

- Default: for cosmetic or ambiguous differences, match prettier — but a mismatch is a question, not automatically a bug. Diverge when there's a defensible reason, recorded in a `_prettier_divergence`
- Spec precedence: when the spec defines a canonical form prettier doesn't emit, follow the spec — document with spec refs
- Comment position: when prettier moves comments, preserve the user's placement. See ./docs/conformance_prettier.md#comment-position-philosophy
- Other defensible tsv-native choices (print width as a hard limit, a clearly better layout) are legitimate too — sanction them deliberately, never to hide a bug
- `_prettier_divergence` suffix: deliberate, documented differences only. Requires a README that **links back to its `conformance_prettier*.md` section** and a matching catalog entry there

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
`prettier_intermediate_to_divergent_variant_*` /
`audit_signature.txt` / `audit_signature_<suffix>.txt` (formatter divergence + prettier
multi-pass pins — the two chain pins, anchored at `output_prettier.*` and at
`unformatted_ours_<suffix>.*` respectively),
`prettier_nonconvergent.txt` / `prettier_rejects.txt` / `tsv_rejects.txt` (no-oracle
markers), `goal` (parse-goal marker: the `.ts` input parses at `Goal::Script` on both
sides — see `typescript/script_goal/*`), `unformatted_*` / `unformatted_ours_*` /
`unformatted_prettier_*`
(normalization variants), `input_invalid_*` (must fail both parsers). Per-file semantics
and validation rules (F/S/R/D): ./docs/fixture_overview.md.

**Other file types** (same structure): `.ts`/`.svelte.ts` use acorn-typescript for parsing; `.css` uses Svelte's `parseCss`. All use prettier for formatting.

**Unformatted variant rules:** Same content structure as input — usually only whitespace differs, and the **bare** `_compact` / `_spaces` names claim exactly that, in one direction (gated by `deno task variants:audit`; see ./docs/fixture_naming.md#standard-variant-names). A variant that also flips a *token* prettier normalizes away — a trailing comma, quote style, `<br>` for `<br />` — takes a name that says so (`unformatted_no_self_closing`, `unformatted_with_closing_tag`); ~300 bare-named variants predate that rule and the audit reports them as ungraded rather than pretending to cover them. Both formatters must normalize to exactly match input. For `.svelte` fixtures this is **enforced**: the render-equivalence check (R rules, ./docs/fixture_overview.md) asserts the variant and `input` produce the same browser-visible render via `svelte compile` — so a formatter bug that changed the render *and* happened to land on `input` can't pass green.

**Invalid syntax rules (`input_invalid_*`):** Must fail BOTH parsers. One syntax error per file.

**Quick Pattern Selection:**

- **Parser matches Svelte**: `input.svelte` + `expected.json`
- **Parser differs intentionally**: Add `expected_ours.json` + `expected_svelte.json` (requires `_svelte_divergence` suffix) — including when the canonical parser **rejects** and only tsv parses (a tsv over-acceptance), where `expected_svelte.json` is `{"error": "failed to parse"}`
- **Formatter matches prettier**: Add `unformatted_*.*` variants
- **Formatter differs intentionally**: Add `output_prettier.*` (requires `_prettier_divergence` suffix)
- **Prettier has stable variants (ours normalizes)**: Add `prettier_variant_*.*` files (requires `_prettier_divergence` suffix)
- **Dual-stable forms (both keep stable)**: Add `variant_*.*` files (requires `_prettier_divergence` suffix)
- **Divergent variant (prettier keeps stable, ours → third form)**: Add `divergent_variant_*.*` files (requires `_prettier_divergence` suffix)
- **Normalization to input divergence**: `unformatted_ours_*.*` normalizes to input with our formatter only
- **Normalization to output_prettier**: `unformatted_prettier_*.*` normalizes to `output_prettier.*` with prettier
- **Prettier's output from an `unformatted_ours_*` fits no single-form marker**: auto-generated `audit_signature_<suffix>.txt` pins the whole chain (N12) — the marker of last resort, for a chain with 2+ distinct intermediates or a stable form tsv can't format; `fixtures:update:formatted` decides, never hand-written
- **Prettier never converges (no oracle)**: Add `prettier_nonconvergent.txt` + README (requires `_prettier_divergence` suffix; excludes all prettier-claim files)
- **Prettier rejects/throws on input (no oracle)**: Add `prettier_rejects.txt` (trimmed content = expected-error substring) + README (requires `_prettier_divergence` suffix; excludes all prettier-claim files; mutually exclusive with `prettier_nonconvergent.txt`)
- **tsv over-rejects but canonical accepts**: Add `tsv_rejects.txt` (trimmed content = expected tsv-error substring) + `expected_svelte.json` + README (requires `_svelte_divergence` suffix; no `expected.json`/`expected_ours.json`; excludes all format-claim files, `input_invalid_*`, and the prettier no-oracle markers)
- **Both differ**: Use `_svelte_prettier_divergence` suffix

## Debug Tooling

**tsv_debug** uses an embedded Deno sidecar for JS tools (prettier, Svelte parser, acorn). Requires Deno; the sidecar spawns on first use and is reused (orders of magnitude faster than spawning per call). Verify with `cargo run -p tsv_debug check`.

### Commands

**Input methods** (consistent across content-processing commands): a file path (parser auto-detected from extension), `--content <string> --parser <type>`, or `--stdin --parser <type>` (`svelte|typescript|css`).

**Content-Processing Commands:**

```bash
# compare - diff our formatter vs prettier (line widths shown right-aligned on changed lines)
cargo run -p tsv_debug compare file.svelte
# Options: --verbose/-v (full input/ours/prettier), --quiet, --color <auto|always|never>, --json
# "Outputs match" = ours(input) == prettier(input), NOT input stability; a match on a
# non-format-stable input adds a note + input-vs-formatted diff (F1 fails on such an input)

# ast_diff - verify semantic equivalence
cargo run -p tsv_debug ast_diff input.svelte                         # round-trip: parse → format → parse → compare
cargo run -p tsv_debug ast_diff input.svelte output_prettier.svelte  # compare two files' ASTs
cargo run -p tsv_debug ast_diff --render input.svelte                # render-aware: normalize both ASTs per Svelte 5
# --render collapses/trims template whitespace per Svelte 5 before comparing, so render-equivalent
# forms match; real content / <pre> / presence-of-space changes still differ. Sound at corpus scale.

# canonical_parse - parse using the canonical parsers (Svelte, acorn+typescript, or Svelte's parseCss)
cargo run -p tsv_debug canonical_parse file.svelte

# canonical_compile - compile Svelte with the canonical compiler (runes-only, deterministic oracle:
# fixed cssHash 'svelte-tsvhash' + constant filename → byte-identical output). Errors exit non-zero.
cargo run -p tsv_debug canonical_compile file.svelte [--target server|client] [--css] [--dev] [--json] [--content|--stdin]

# render_compare - do TWO Svelte sources render the same page? The pairwise triage arm of the
# render-equivalence oracles. Tiers: identical (compiled server JS byte-equal) / cosmetic (bytes
# differ, render key equal — same page) / visible (render keys differ). The key is static (no SSR
# execution), so unresolvable imports still grade. Exit codes: 0 same render, 1 visible, 2 error.
# Two inputs total, via --content (repeatable) and/or file paths; --json.
cargo run -p tsv_debug render_compare a.svelte b.svelte

# compile_compare - diff tsv's Svelte compile vs the canonical compiler, comparing the CANONICALIZED
# JS of both sides (intent-erased reprint via tsv_svelte_compile::canonicalize_js). The parity bar
# tolerates a comment-POSITION difference (compare_canonical), so a remaining diff is a real code
# difference. Exit codes: 0 parity, 1 real diff, 2 error (incl. a component shape tsv doesn't cover
# yet — prints the oracle canonical form as the target). --json emits { target, parity,
# comment_position_tolerated, ours_status, hunks }. The ad-hoc one-file view; durable expectations
# live in the compile fixtures (tests/fixtures_compile).
cargo run -p tsv_debug compile_compare file.svelte [--target server|client] [--content|--stdin] [--json]

# compile_fixture_init - create/reinit a compile fixture (tests/fixtures_compile/<feature>/<case>/):
# prettier-formats the runes component, oracle-compiles it (server, non-dev), writes input.svelte +
# expected_server.js (CANONICALIZED oracle JS) + expected.css (styled components only). Expected
# files are ALWAYS oracle-generated, never hand-written. Also: --content/--stdin/--force.
cargo run -p tsv_debug compile_fixture_init tests/fixtures_compile/feature/case --content '<p>text</p>'

# compile_fixtures_validate - validate compile fixtures; per fixture, all gating: (a) oracle
# freshness — canonicalize(oracle(input)) equals the committed expected_server.js byte-exact + css
# match; (b) ours — tsv compile succeeds and its canonicalized JS is PARITY with expected + CSS
# match; (c) expected_server.js is a canonicalize fixed point. The pure-Rust slice (input parses,
# expected idempotent, ours-vs-expected parity) also runs sidecar-free in
# `cargo test --workspace --test compile_fixtures_tests`, the offline parity gate.
cargo run -p tsv_debug compile_fixtures_validate [pattern...]   # --list, --json

# compile_corpus_compare - the compile-parity wide net: compile every .svelte under the roots with
# the canonical compiler AND tsv, comparing canonical reprints; buckets per file (parity / refused /
# fenced / oracle-rejected / MISMATCH / error). A MISMATCH or an OVER-ACCEPTANCE (oracle rejected,
# tsv compiled) is a refusal-contract bug and gates. Exit codes: 0/1/2. Sidecar-dependent, NOT in
# `deno task check`. --list, --json, --census. Full detail: ./docs/compile_tooling.md
cargo run -p tsv_debug compile_corpus_compare <paths...>
# --ratchet: the VALIDATION-SUITE GATE — same pipeline over Svelte's own compiler-errors + validator
# suites (~2/3 deliberately INVALID), graded against compile_validation_known.txt (--update re-pins).
# ⚠️ Always a SEPARATE invocation — never extra roots on it. Reference: ./docs/compile_validation_ratchet.md
cargo run -p tsv_debug compile_corpus_compare --ratchet [--update]

# compile_fuzz - the DIFFERENTIAL compile fuzzer: feature CROSS-PRODUCTS from the compile fixtures
# (eleven AST/feature-level operators), each mutant graded against the oracle — the adversarial leg
# the real-component corpus can't be. ⚠️ CURRENTLY RED BY DESIGN: a discovery tool with an open work
# list, not a regression gate. Deterministic per --seed, independent of --jobs. Build with
# `--profile corpus` so a panic in tsv's compile is caught and REPORTED rather than killing the run.
# Options: --seed, --iterations, --max-mutations N, --limit N, --jobs N, --max-findings N,
# --dump-dir, --list, --json. Findings cataloged in ./docs/checklist_svelte_compiler.md;
# full detail: ./docs/compile_tooling.md
cargo run --profile corpus -p tsv_debug compile_fuzz

# erase_comment_census - size the type-eraser's comment-refusal haircut over a corpus (pure Rust):
# per lang="ts" component, comments intersecting an erased span's refusal window. The rate is a
# LOWER BOUND. Also: --verbose, --json. Full detail: ./docs/compile_tooling.md
cargo run --release -p tsv_debug -- erase_comment_census ../fuz_ui ../zzz

# format_prettier - format using prettier (line widths by default; --no-line-widths to hide)
cargo run -p tsv_debug format_prettier file.svelte

# line_width - measure visual line widths (pure Rust; --line N for one line with preview, --json)
cargo run -p tsv_debug line_width file.svelte
```

**Fixture Management Commands** (all accept positional patterns, multiple = OR; `fixtures_validate` and `fixtures_update_parsed` also take `--list`):

```bash
# fixture_init - create/reinit a fixture (formats through prettier + generates expected.json)
cargo run -p tsv_debug fixture_init <dir> --content '<code>'   # or --stdin; bare = reformat existing input
# Also: --parser <svelte|typescript|css|svelte-ts> (aliases ts, svelte.ts), --force (overwrite)

# fixtures_validate - verify fixtures are correct (CI). --prettier-only skips our parser/formatter.
# Cross-fixture duplicate detection is skipped when filters are active; a parser mismatch with
# expected.json is a hard error (no ratchet — all fixtures must match).
cargo run -p tsv_debug fixtures_validate [pattern...]

# fixtures_update - regenerate from canonical sources
cargo run -p tsv_debug fixtures_update            # both parsed + formatted
cargo run -p tsv_debug fixtures_update_parsed     # expected.json only (Svelte for .svelte, acorn for .ts, parseCss for .css)
cargo run -p tsv_debug fixtures_update_formatted  # output_prettier.svelte (auto-deletes if identical to input;
#   skips the no-oracle markers prettier_nonconvergent / prettier_rejects / tsv_rejects)

# fixtures_audit - investigate normalization graphs (diagnostic; --all for every fixture, --verbose, --json)
cargo run -p tsv_debug fixtures_audit [pattern...]

# ts_fixture_audit - which input.ts fixtures genuinely need .ts vs could be .svelte (embeds each in
# <script lang="ts"> and checks both formatters). Necessary = byte-0 feature, Svelte-parse-fail, or
# formats-differently; Convertible = formatting-safe only, not a mandate (a fixture may be .ts on
# purpose to cover the standalone path); Intentional = the INTENTIONAL_TS allowlist. --verbose shows
# the TS-vs-Svelte diff on 'formats differently' fixtures.
cargo run -p tsv_debug ts_fixture_audit [pattern...]

# conformance_audit - doc/fixture integrity in one fixture walk: divergence fixtures cataloged in
# their conformance doc, every Markdown link in the repo resolves (docs/*.md, fixture READMEs, and
# the CLAUDE.md / README.md set — a walk sharing tsv_discover's safety nets + tsv_ignore's
# .gitignore matcher, skipping symlinks),
# divergence READMEs back-link their sanctioning doc, no stray READMEs (exceptions: the in-code
# ALLOWED_NONDIVERGENCE_READMES allowlist), no catalog-family drift (the
# docs/conformance_prettier*.md on disk are exactly CONFORMANCE_PRETTIER, each indexed by the
# frame's §Catalogs table). Pure Rust; gated in `deno task check`. --json.
cargo run -p tsv_debug conformance_audit

# compile_conformance_audit - the compiler analog, deliberately minimal: _compiled_divergence
# fixtures must be cataloged in docs/conformance_svelte_compiler.md (expected to stay EMPTY — a
# tripwire) + checklist ↔ `Refusal` drift (a bucket key the catalog can't produce GATES; the
# reverse is report-only). Pure Rust; gated in `deno task check`. --json.
cargo run -p tsv_debug compile_conformance_audit

# variant_audit - the `_compact` / `_spaces` variant DIRECTION: the pair is two opposite claims
# about one reference form (input.*, or output_prettier.* for `unformatted_prettier_*`), so a
# `_compact` may never widen a newline-free whitespace gap and a `_spaces` may never empty one.
# Bare suffixes only — a qualified name (`unformatted_ours_hug_spaced`) is the escape hatch for a
# deliberately bidirectional variant. A variant that is not whitespace-only against its reference
# cannot be aligned and is reported UNGRADED, never silently skipped. Pure Rust; gated in
# `deno task check`. --list, --json. See ./docs/audits.md
cargo run -p tsv_debug variant_audit

# canonicalize_audit - canonicalize_js at corpus scale: run twice per TS/JS file and bucket —
# input-rejected (informational), NON-IDEMPOTENT / CORRUPT-OUTPUT / COMMENT-LOSS (all failures).
# Pure Rust; gated in `deno task check` over tests/fixtures + tests/fixtures_compile. --json.
cargo run -p tsv_debug canonicalize_audit tests/fixtures tests/fixtures_compile  # or real-corpus dirs
```

> **Troubleshooting:** See ./docs/fixture_overview.md#quick-decision-tree

**test262 ECMAScript Conformance Tests:**

```bash
# test262 - run ECMAScript conformance tests against our parser (pure Rust; expects ../test262)
cargo run -p tsv_debug test262 [path-pattern]
# Options: --path <dir>, --list, --verbose, --negative-only, --positive-only,
#          --gate (the release gate: fails ONLY on a positive-parse regression or a shift in the
#           pinned positive count; negatives — the deferred early-error frontier — are reported,
#           not gated. A bare run exits non-zero by design, so it's a diagnostic, not a gate.),
#          --emit-manifest <path> (JSON manifest of the graded strict subset — feeds the tsv-vs-oxc
#           differential consumer, benches/js/diagnostics/test262_compare.ts)
```

See ./docs/conformance_test262.md (command interface; §Differential for the tsv-vs-oxc comparison).

**Typechecker conformance (`tsc_conformance`) — EXPERIMENTAL, may never ship.**
`tsv_check` is a from-scratch TypeScript binder + checker; no shipped artifact links it
(`cargo tree -i tsv_check` → only `tsv_debug`), and the parser and formatter are never modified in
service of it. `tsv_debug tsc_conformance` grades it against tsgo's committed `.errors.txt`
baselines (`../typescript-go`, pin `168e7015`), surfaced as **on-demand** tasks — none in
`deno task check`, `deno task conformance`, or release gating, and `../typescript-go` is not a
release-required oracle. Full reference: ./docs/typechecker.md.

```bash
deno task conformance:tsc-roundtrip     # baseline parse → re-render → byte-compare (zero checker code)
deno task conformance:tsc-check         # the tsv_check conformance sweep + committed report
deno task conformance:tsc-check:update  # re-pin the run's snapshot counts after deliberate drift
```

**Performance Profiling Commands** (all pure Rust, no Deno — full reference: ./docs/performance.md):

```bash
cargo run -p tsv_debug profile ../zzz/src/lib                    # parse vs format phase timing (--iterations, --json, --flow-stats)
cargo run -p tsv_debug profile --bind ../zzz/src                 # parse vs lower+bind timing (TS-only) + peak RSS (§1)
cargo run --release -p tsv_debug -- json_profile ../zzz/src/lib  # FFI parse path: parse vs the wire-JSON write (§2)
cargo run -p tsv_debug buffer_sizes ../zzz/src ../gro/src     # printer SmallVec sizing histograms (§8)
cargo run -p tsv_debug arena_stats ../zzz/src/lib                # DocArena node-population + memory audit (§7; --reuse, --list-errors)
cargo run --release -p tsv_debug -- compile_profile tests/fixtures_compile  # Svelte compile against the format wall (§9)
```

**Codebase Metrics:**

```bash
cargo run -p tsv_debug metrics [--json]    # line counts by crate and phase (pure Rust); also `deno task metrics`
```

**Audits** — every standing correctness gate and discovery harness (swallow, ledger, census, gap/blank injection, fabrication, ignore-honoring, fuzz, F1 sweep, render-equivalence, the corpus bundle, `lex_diff`, the compiler audits, and the rest) is cataloged in ./docs/audits.md — what it proves, what it is blind to, flags, and where it gates; the `deno task` entry points are indexed in [Fixtures](#fixtures-rust--deno-based). Read the relevant section there before running or modifying an audit.

## Architectural Notes

### Closed Scope, Open Convention

tsv ships a closed language set (TypeScript, CSS, Svelte) but is open by convention **at the Rust source/crate level**: each language crate (`tsv_ts`, `tsv_css`, `tsv_svelte`) is self-contained — owns its internal AST, parser, formatter, and convert layer — and exposes the same free-function API (`parse()`, `format()`, `convert_ast_json_bytes()`, `convert_ast_json_string()`, `convert_ast_json()`). **No central `Language` trait, no registry, no enum dispatch.** Two properties follow:

- **Optimal artifacts**: concrete types end-to-end, no dyn dispatch; WASM tree-shakes by feature at the link level — `@fuzdev/tsv_format_wasm` excludes the convert layer, `@fuzdev/tsv_parse_wasm` the printers.
- **Source-level openness**: anyone can publish a same-shaped `my_org/tsv_html_parse` crate and any downstream _Rust_ consumer can `use` it without central buy-in. Published CLI/WASM binaries still hardcode the language list (`lang_bindings!` macro), by design.

Cross-language coupling exists only where languages integrate — `tsv_svelte` depends on `tsv_ts` (for `Expression`) and `tsv_css` (for `StyleSheet`). Avoid inverting this: no central public-AST crate, no dyn `Language` trait, no workspace-level language registry. Full discussion: ./docs/architecture.md#closed-scope-open-convention.

### Strict Mode Only

**`tsv` parses TypeScript/JS as strict mode only.** Intentional: TypeScript, ES modules, and Svelte `<script>` are all inherently strict. tsv parses the syntactic grammar and rejects only the *lexically* sloppy-mode constructs — the `with` statement and legacy octal literals (`010`). Strict-mode **early errors** (duplicate parameter names, reserved words as identifiers, octal string escapes, `delete` of a plain name) still parse; enforcement is deferred to a future diagnostics layer. These leaks only matter for standalone JS — Svelte/TS module context is strict.

This is one instance of a broader stance: **the parser is deliberately permissive and defers static-semantic early-errors** (the above, plus the TypeScript ambient-context rules — a `declare` member body, initializer, decorator, etc.) to the diagnostics layer, so the formatter keeps formatting everything well-formed. The **correctness oracle for what's actually an error is tsc**, not acorn-typescript (matched only for AST *shape*); the accept-vs-reject test starts with prettier — a construct prettier can't parse, tsv rejects — but among those prettier formats, tsv defers only the **mode/context-dependent** early-errors and still rejects the **unconditional-local** ones (e.g. `get`/`set constructor`). See [crates/tsv_ts/CLAUDE.md §Architecture Position ("Sources of truth")](crates/tsv_ts/CLAUDE.md#architecture-position) and [docs/conformance_svelte.md §TypeScript Corrections](docs/conformance_svelte.md#typescript-corrections).

**Strict ≠ module-only — there is an orthogonal *goal* axis.** Both goals are strict (no sloppy mode, no `"use strict"` detection). A parse runs against `tsv_ts::Goal::{Module, Script}` (`parse_with_goal`, CLI `--goal script|module`), **defaulting to `Module`** (correct for Svelte `<script>` and ~all real TS; Svelte hard-wires it). The goal toggles only the four goal-specific constructs: at `Script` goal `await` is an ordinary identifier (`[~Await]` tracked via the parser's `in_await` flag, save/restored at every function-like scope), and `import`/`export` declarations + `import.meta` are syntax errors (dynamic `import(...)` stays valid, and so does a TypeScript **import-equals** — `import x = A.B` / `import x = require('y')` is not an `ImportDeclaration` and not a `ModuleItem`, which is why the gate fires on the shape rather than on the `import` keyword). `sourceType` follows the goal. See [docs/conformance_test262.md §Strict Mode Only, Explicit Goal Axis](docs/conformance_test262.md).

### Language-Level concerns (classification)

HTML element classification is split between the `tsv_html` crate — pure functions over tag names (`is_block_element()`, `is_void_element()`, `preserves_whitespace()`, and the other whitespace rules) — and thin printer adapters (`tsv_svelte/src/printer/classification/`) that resolve symbols, call tsv_html, and traverse the AST. Enables reuse across all planned tools (formatter, linter, compiler, LSP).

### AST Architecture: Internal AST vs Wire JSON

Drop-in replacement for the canonical parsers' **public JSON AST** (acorn / acorn-typescript / Svelte / `parseCss`), NOT their internal implementation.

- **Internal AST**: Clean, semantic representation (decoded strings, normalized values) — what every tool (formatter, linter, …) builds on.
- **Wire JSON**: the parse product. The per-language writers (`ast/convert/write*`) emit it **directly from the internal AST in a single walk** — applying each acorn/`parseCss`/Svelte quirk at emission time — never materializing a typed public-AST Rust layer. The wire shape *is* the contract, documented by the hand-maintained `crates/tsv_wasm/types/tsv_ast.d.ts`; `convert_ast_json` is a thin `serde_json::from_slice` over the writer's bytes.

Worked example + full design: ./docs/architecture.md §Two-AST Design.

**Key Rules**:

- Raw strings NEVER duplicated in the internal AST (extract via `source[span.range()]`)
- The internal AST is NEVER the wire output — the wire JSON is hand-emitted by the writer; `serde_json` is used only for exact string-escape / `f64` parity and to parse the bytes back into a `Value` (CLI `--pretty`, tests)

### Position Types: u32 vs usize

- **Span**: `u32` for start/end (8 bytes total, 50% memory savings vs usize)
- **`Token`**: `u32` start/end — a 16-byte POD `{kind, start, end}` returned from `next_token` in registers (size pinned by a `const` assert in tsv_ts and tsv_css; tsv_svelte's leaner `Token` pins at 12 B); the decoded value (escapes only) lives out-of-band on the lexer (the reused `Lexer::decode_scratch` buffer, borrowed via `decoded_str`)
- **Lexer/Parser positions**: `usize` (natural for `source[pos]` indexing); the lexer dispatches on raw bytes (`cur_byte`) and decodes a `char` only at non-ASCII branches
- **Conversions at boundaries only**: `as u32` when creating Spans/`Token` fields, `as usize` when extracting; prefer `span.extract(source)` / `span.range()` over manual casts

### Comment Handling: Detached Model

Comments are stored **separately from AST nodes** in a flat `Comment` array at the root
level (`Program.comments`, `CssStyleSheet.comments`, `Root.comments`); the printer finds
them via O(log n) binary search on span positions. `Comment` (`tsv_lang/src/comment.rs`)
is a `Copy` POD of spans + flags — text is recovered on demand via
`Comment::content(source)`, never stored owned. The full model — fields, ownership
doctrine, hazards, and the leading-comment emitter rules — lives in ./docs/comments.md.
**Read it before touching comment handling in any printer.** The always-loaded core:

**Owned comments** (`owned_by_node`, set by the parser): **every glued block comment is
owned** — bound to the token after it and printed by that node's doc rather than by the
enclosing gap, so a paren synthesized around an ENCLOSING expression can never land
between them. A bundler annotation
(`/* @__PURE__ */`), a JSDoc cast (`/** @type {T} */ (x)` — handed to the `JsdocCast`
node), and a plain glued comment bind identically; `owned ⇒ is_block`, so no line comment
is ever owned. **Ownership is a fact about who PRINTS a comment, never about whether it
EXISTS** — every bug in this class has been a violation of that sentence.

⚠️ **"The innermost node its token begins" holds only because that node prints FIRST**, and
a **paren-less arrow** breaks it: its span starts at its own sole parameter, so both nodes
answer the position-keyed lookup while the arrow still prints a synthesized `(` ahead of the
parameter. The arrow keeps the claim and the parameter is suppressed
(`Printer::with_owned_comment_claimed_above`) — pushing the claim down would move the comment
inside a paren the author never wrote, and declining the bind would give up
`OwnedCommentEffect` for the shape. A suppression is only a de-duplication where some
enclosing node provably claims; otherwise it is hazard 1 spelled backwards.

A comment can be asked about along exactly **three** axes, and the lookup API
(`tsv_lang::comment`) makes the caller name which:

| axis | question | owned comments | who asks |
| --- | --- | --- | --- |
| **to emit** | "which comments must *I* print here?" | **skipped** | gap emitters (~400 sites) |
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

⚠️ **Five hazards, all of which have bitten** (full text + war stories in
./docs/comments.md): (1) an owned comment nothing prints is a DROPPED comment — a builder
that *reassembles* a node instead of routing through `build_expression_doc` must claim it
on its own seam (`prepend_owned_leading_comment_at`); (2) an owned comment travels
*inside* its node's doc, so the gap around it can't see it — ask the node instead
(`owned_leading_comment_effect`, the single seam for that question); (3) a region the
parser *lifts out* of its container is still inside the container's gap, so two emitters
print it (`AttrGaps::claimed` is that seam) — and ownership masks it: only a line comment
(never owned) exposes the double-print; (4) an **alternate-layout container builder** that
emits only its children's docs runs no gap lookup, so every leading / inter-item /
trailing / empty-container comment is DROPPED — hand a commented container to its
comment-aware twin (gate BEFORE the empty arm) or share the per-item emission seam;
ownership masks this one in mirror image (the glued *leading* comment is owned and
survives, so a leading-comment repro reports the builder healthy); (5) an owned comment a
blank scan CROSSES **fabricates a blank line** — it is skipped by the emit axis but still
occupies its bytes, so a scan measuring a distance reads its interior newlines as an author
blank. The gaps that can hold one are exactly those ending at an EXPRESSION START (a
brace-less header→body, a binary operator→operand, a ternary `?`→branch), and the fix is
the in-source ceiling (`blank_scan_end`) at the scan's far end. Guards: the **print-once
ledger** (`comments:audit`) is the structural guard on the first four but only sees a
document AS AUTHORED — a wholly comment-blind builder stays green until some file puts a
comment there; the **injection audits** (`gaps:audit`) are the discovery arm for hazard 4;
the **census** (`census:audit`) lexes trivia off raw input AND output, so a comment a
parse path consumed without registering (invisible to the ledger by construction), a
merge, or an interior rewrite still counts. **Hazard 5 no gate reaches at all** — nothing is
dropped or rewritten and the fabricated line is its own fixed point, so only a prettier
`compare` finds it.

⚠️ **Leading comments have one rule and one emitter** — `Printer::push_leading_comment_run`
(prettier's `printLeadingComment`), with `Printer::comment_hugs_next` as the single glue
test and `Printer::push_leading_run_separator` for the two hand-rolled always-broken
sites. Don't hand-roll `is_block && is_same_line(...)` at a new site or re-derive the
anchor+separator inline — keying the hug on the *item* rather than on *what follows the
comment* was a whole bug family, and asking the whole `(`→value RANGE instead of the
comment's own neighbours is the same error one step out (it splits an author-glued run and
forces the shell open). A caller that owns a shell asks the emitter, which **returns
whether it pushed a hardline** — tsv has no `propagateBreaks`, so a break inside the run is
invisible to the group that must open for it. Whether the soft `line` after a leading run collapses is
per-element grouping (the array family groups each element → collapses; the params family
doesn't → breaks), mirrored from prettier; full rule in ./docs/comments.md.

⚠️ **A run at the END of a container takes its separator BEFORE each comment**, never after
— `Printer::build_trailing_body_comments_doc` where a last item precedes it (`prev_end ==
0` being the program's `}`-less form) and `Printer::push_dangling_comment_run` where the
run is the container's only content. The "separator after each non-last comment"
formulation must ask the comment's **kind**, and its answer — "a block needs no break" —
is false as soon as another comment follows, welding that comment onto the block's line
(`/* c1 *//* c2 */`). The weld is **lossless** and idempotent, so the ledger, census, F1,
fuzzer, and round-trip are all blind to it; only a prettier `compare` finds it — which is
why the rule lives in one emitter per question rather than at each container.

⚠️ **A trailing GAP inside a construct is the third emitter of that same rule** —
`Printer::push_trailing_comments_in_range` (a paren shell's `)`, an indexed access's `]`, a
mapped-type member's value). It asks the separator question of the **source** ("did the
author give this comment its own line?"), since these comments may be legitimately glued,
and the break rides **inside** the `line_suffix` — a real break between two deferred
comments splits the very construct they sit in. Only a **line** comment defers by
construction; a block defers solely to stay behind one. Back-to-back emission welds the run,
and an inline block mixed into a deferred one **reorders** it — so open-coding this loop is
the recurring bug rather than a shortcut. Its float-out sibling
(`Printer::append_trailing_paren_comments`, a stripped grouping paren's orphaned region)
defers an **own-line block** too, which makes that reorder reachable there — and a run whose
enclosing layout is already vertical takes real breaks instead
(`Printer::push_anchored_trailing_run`).

⚠️ **A run's ANCHOR advances over each comment it emits.** Own-line-ness is a question about
a comment's own neighbours, so re-asking a *fixed* anchor per comment
(`has_newline_between(node_end, c.span.start)`) reads the second half of an author-glued
pair across the first, calls it own-line, and splits it — the same defect as the kind-keyed
separator above, reached without ever asking the glue question. The anchor is legitimate
only for the run's **first** comment, where the thing behind it really is the node.

⚠️ **A deferred run must not leave the construct it was written in.** Deferring is *end of
line*, not *escape*: a construct that closes without breaking carries the comment past its
own closer, re-binding it and landing it on a line that may already hold a deferred comment,
where the two weld irreversibly. So a **line** comment in one of these gaps forces its
construct **open** — the closer drops to its own line and the comment flushes inside. Every
bracketed type region does this (`{}`, `<>`, tuple `[]`, function-type `()`, indexed-access
`[]`), and a paren shell is **retained** rather than stripped for the same reason. The one
sanctioned exception is a union / intersection member a `|`/`&` separator still **follows**,
whose per-member break ends the line where the shell ends
(`Printer::type_member_separator_follows`); the last member has no separator and retains.
The break that sanctioned strip forces is **flush-scoped** (`DocArena::flush_break`, not
`break_parent`): only the group the deferred run actually flushes in breaks — an
intermediate composite with no line after the suffix stays flat, since forcing it was a
break the reparse could not reproduce. Unscoped `break_parent` after a deferred suffix
stays correct only where the comment's construct is *retained* (its doc regenerates
identically each pass); a strip changes the reparse geometry and needs the scoped node.

⚠️ **A deferred run's FLUSH must end the line, so a `lineSuffixBoundary` belongs only where
nothing else does.** The renderer drains the buffer at a break-mode `line` or at a boundary,
and both then emit the newline (prettier pushes `hardlineWithoutBreakParent` there) — a flush
that ends inline lets the deferred `//` swallow the code after it, output that doesn't
reparse. Because the boundary breaks on its own, planting one in front of a doc's own forced
break renders a blank line instead. `arena_fits` answers the same question at measure time:
a boundary reached with a suffix pending doesn't fit.

⚠️ **A comma-separated list asks about one gap TWICE** — the previous element's trailing
run and the next element's leading run — and the two must **partition** it: unclaimed is a
DROP, doubly-claimed a DOUBLE-PRINT. `collect_trailing_comments` / `push_element_comma_trailing`
state that split once (object literal, both patterns, specifier list, enum members); the array
literal states its own as a POSITION rather than two filters (`element_gap_split`, each
emitter taking a range), which is what keeps its three readers — trailing, leading, end-of-array
scan — from drifting apart; the **call-argument family** (plain call, `new`, member chain,
`import()`) states it as `PartitionedComments::for_routed_arg_gap` — the partition and its
hugging **route** as ONE step, since partitioning without the route silently reclassifies every
hugging comment as stranded, moving it with nothing dropped and no gate firing. Three standing
rules: the leading scan resumes at the trailing run's
**end**, never past the separator — a comment before a comma the author pushed onto its own line
(`a: 1⏎// c⏎, b`) belongs to neither side otherwise, which was a live drop at four sites; the
claimed run must stay a **prefix** of the gap, so a same-line block ahead of a deferred line
comment is claimed too — skipping it drops it, and leaving it to lead the next element
**reorders** it past the `line_suffix`; and the gate is the **source** reading
(`comment_follows_content_on_its_line`), since a stripped `)` and the comma itself sit in the gap
outside every item span, where a `is_same_line(prev_end, …)` gate is blind to both. **Both arms
ask it** — a `//` glued to that `)` trails its element too, and `is_own_line_comment` cannot
answer, its `!is_block` short-circuit calling every line comment own-line — so the run must then
**end at the first line comment**, which the anchored gate used to guarantee for free: a second
deferred `//` welds onto the first's line and swallows the code behind it. The seam's **anchor**
is likewise the element's PRINTED end: where the element node's span swallows a stripped `)`
(a destructuring `Property`, an `AssignmentPattern`) the scan starts past the shell's interior and
nothing else emits it. And an **elision opens no
gap of its own**: the anchor stays the last REAL element's split however many holes intervene,
so a comment slides *forward* past their (structural) commas to the element it leads — or, past
the last one, to the list's own trailing position. Counting commas instead handed the region to
seams that print nothing, which was a DROP every time; no blank line is measured across a hole
either. **The expansion gate over the seam asks the same source question**, in four spellings —
one per family shape (bracketed lists, object/type literal, call arguments, parameter lists) — and
an item-boundary reading there expands a list that fits, a third fixed point neither the bare
authoring nor prettier produces. The **width** emitters ask the seam too, as a range
(`inline_trailing_run_end`): claiming `[item_end, comma)` wholesale trails an own-line comment onto
the *previous* item, sliding it BACKWARD across that item's comma. A **deferred** run leads the next
item, so its last comment takes the leading separator (`comment_hugs_next`, keyed on the source
right after the `*/` — never on where the item starts, which the list's own comma sits between), and
an author blank belongs ahead of it and **forces the break**, since a soft `line` cannot carry one —
but WHICH blank is the **family's** rule (`BlankRule`), not the arm's, and answering the hugging arm
with the params rule gave the tuple a blank prettier collapses. And a scan measuring a **distance**
rather than claiming a comment takes the other end of the shell (`element_shell_end`): anchored
inside, it reads the erased `)`'s line breaks as an author blank and fabricates one — in every
trailing dangling run, so a container that HAND-ROLLS that run (the object literal did) gets to be
wrong about it separately. Full text: [docs/comments.md](docs/comments.md) §The element-comma seam.

Higher-fidelity models (attached comments, trivia tokens) may be needed for IDE/linter use
cases; prettier, oxfmt and biome all get the JSDoc-cast paren binding wrong — see
[conformance_prettier_ts_comments.md §Comment relocation](docs/conformance_prettier_ts_comments.md#comment-relocation).

## Dependencies

### Rust Crates (minimal deps)

The shipped language/foundation crates' external deps (dev tooling adds `argh`, `tokio`, `futures-util`, and serde derive; `tsv_wasm` adds `wasm-bindgen`/`js-sys`):

- `serde_json` — wire-JSON emission (exact string-escape / `f64` formatting) + reparsing bytes to a `Value` (CLI `--pretty`, tests). Language crates depend on `serde` only transitively, without its `derive` (derive is dev-tooling only: `tsv_debug` / `tsv_cli`)
- `smallvec` — stack-allocated vectors (printers + `tsv_check`)
- `thiserror` — error type derivation
- `phf` — compile-time perfect hash maps (keywords, entities)
- `unicode-ident` / `unicode-segmentation` / `unicode-width` — XID identifiers, grapheme clustering, display width (CJK, zero-width)
- `bumpalo` — bump arena for the internal AST (and, via `tsv_arena`, the bindings' per-thread `reset()` reuse; `tsv_check`'s caller-owned arenas follow the same contract)
- `talc` — WASM global allocator (`tsv_wasm`, wasm32-only target dep): pure-Rust `no_std` allocator replacing dlmalloc; the `WasmGrowAndExtend` source keeps the warm instance's linear-memory high-water at dlmalloc parity. Pulls `lock_api` + `allocator-api2` (+ `scopeguard`) into the wasm32 graph only
- `napi` / `napi-derive` / `napi-build` — N-API bindings for `tsv_napi` (tsv-scoped carve-out)

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

- ./docs/cli.md - CLI architecture, command patterns, multi-file formatting rules
- ./docs/audits.md - the standing audit gates: what each proves, blind spots, flags, gating
- ./docs/benchmarks.md - benchmark fairness caveats, the implementation catalog, binary sizes, the canonical-oracle-pin ritual
- ./docs/gate_counts.md - the pinned counts every graded gate and harvest enforces
- ./docs/comments.md - the detached comment model: ownership, the three axes, hazards, emitters
- ./docs/directives.md - the format-ignore directive family: placement, freeze scope, per-language behavior
- ./docs/compile_tooling.md - the sidecar-dependent compiler harnesses: corpus compare, compile fuzz, erase census
- ./docs/compile_validation_ratchet.md - the validation-suite ratchet: snapshot, kinds, verdict, triage
- ./docs/typechecker.md - the experimental `tsv_check` typechecker (may never ship) + its on-demand tsgo-conformance harness
- ./docs/performance.md - profiling methodology, tooling, and results tracking
- ./docs/workflow_corpus.md - corpus-driven formatting conformance workflow
- ./docs/workflow_test262.md - test262 conformance workflow
- ./docs/fixture_workflow.md - **step-by-step script for creating fixtures**
- ./docs/fixture_overview.md - Validation rules, troubleshooting, divergence patterns
- ./docs/fixture_naming.md - content naming conventions

### Language Checklists

- ./docs/checklist_css.md
- ./docs/checklist_svelte.md
- ./docs/checklist_svelte_compiler.md
- ./docs/checklist_typescript.md

## Bash Tool Notes

Use heredocs for multiline strings (`cat <<'EOF'`), `$(...)` for command substitution (not backticks), double quotes for strings with spaces.
