# tsv

> precise language tools for TypeScript/JS, CSS, and Svelte in Rust

High-performance Rust parser, a drop-in replacement for Svelte's modern parser (acorn + acorn-typescript), paired with a formatter that took Prettier as its initial guide and still tracks it for the common case — with deliberate, cataloged divergences where tsv's own judgment is more defensible.

**Non-configurable by design** (like `gofmt` and Black): Prettier defaults except printWidth=100, useTabs=true, singleQuote=true, trailingComma='none' — no config files, CLI flags, or runtime options, ever. The only carve-outs are file *scope* (`tsv format` honors `.gitignore` plus hierarchical `.formatignore` / `.prettierignore`) and the parse goal (`--source-type`, a grammar input rather than a setting). See [Configuration](#configuration).

## Releases

Version bumps, publishing, and **`CHANGELOG.md`** are user-owned. Agents make the source/doc/fixture edits and never touch `CHANGELOG.md` (including `## Unreleased` and its `<!-- bump: … -->` marker); the user stamps it at release time.

## Priorities

1. **Correctness**: Match Svelte's parser exactly — a drop-in replacement on the default span-only wire (every field but `loc` and `name_loc`); `loc` is opt-in with one definition (acorn-exact for TypeScript, Svelte's `loc` quirks not reproduced — ./docs/architecture.md#loc-lines-one-rule-per-document). The formatter tracks Prettier for the common case, diverges deliberately and catalogs it where more defensible (spec, print width, comment position, its own taste), and fixes numerous Prettier bugs. Fixtures are the source of truth — when tests fail, fix the code; when tsv diverges on purpose, the fixture records it.
2. **Performance**: Pure Rust. Dev tools use an embedded Deno sidecar to minimize process overhead.

## Development Philosophy: Test-Driven Development with Fixtures

**ALWAYS use TDD when implementing features or fixing bugs:**

0. **Load context FIRST** — read ./docs/fixture_workflow.md AND ./docs/fixture_naming.md. For ANY
   `_prettier_divergence` fixture, ALSO read ./docs/conformance_prettier.md (the shared frame —
   terminology, `◆reason` tags, decision framework) **plus the catalog for the language you're
   touching**, per its §Catalogs table: ./docs/conformance_prettier_css.md,
   ./docs/conformance_prettier_svelte.md, ./docs/conformance_prettier_ts.md,
   ./docs/conformance_prettier_ts_comments.md, ./docs/conformance_prettier_ignore.md. Every
   divergence must be sanctioned and **cataloged in the relevant section** (comment divergences:
   §Comment Position Philosophy in the frame + the §Comment relocation catalog; others: the
   matching feature section), AND the fixture's `README.md` MUST link back to it
   (`See [conformance_prettier_<lang>.md §…](…)`). `conformance:audit` gates that README ↔ catalog
   agreement; linking only the shared frame does **not** satisfy an entry that lives in a language
   catalog (link both). Study 2-3 existing fixtures in the target category (match their README shape).
1. **Create the fixture FIRST** — `fixture_init` creates `input.svelte` (prettier-formatted) and `expected.json` in one step.
   Use `.svelte` unless the feature is file-level (byte 0: hashbang, BOM). See ./docs/fixture_workflow.md#11-create-directory-and-draft.
2. **Review the input** — read the generated `input.svelte` to verify structure (formatting is guaranteed correct).
3. **See it fail** — `deno task fixtures:validate <pattern>` shows the failing diff.
4. **⚠️ APPROVAL GATE — STOP HERE.** Show the failing diff to the user and wait for explicit
   confirmation ("lgtm", "proceed", or feedback) before writing any implementation code.
   **If feedback reworks the fixture (naming, structure, cases), redo steps 1-3 and return here —
   the gate resets on every rework.**
5. **Implement the fix.**
6. **Validate** — `deno task fixtures:validate <pattern>` passes.

**`long` fixtures**: include BOTH a 100-char case (stays inline) and a 101-char case (breaks), at the exact boundary with the minimum content that triggers it. Iterate `fixture_init --force` and read the widths from its output — never estimate manually.

**Never write code before creating the fixture** — it defines what "correct" means. **Failing fixtures are expected** — never delete one to make tests pass; it is a known bug waiting to be fixed.

## Values

- **Spec-first**: Read specs and canonical implementations before implementing. Experiment to verify, not to design.
- **Refactor early**: Fix outdated patterns immediately. Leave no legacy.
- **One sprint at a time**: Implement incrementally, keep tests passing.
- **No backwards compatibility**: Pre-stable — delete old code, migrate fully, don't shim. No new deps without explicit approval.

## Quick Start - Common Workflows

```bash
cargo check --workspace                # fast check (no codegen)
deno task fixtures:validate <pattern>  # targeted fixture validation (preferred for fixture work)
deno task dev                          # watch: check + test on change (requires `cargo install cargo-watch`)
cargo test --workspace                 # ALL tests (includes all fixtures)
deno task check                        # full committed-tree gate: fmt, audits, typecheck, tests, clippy (benches/js/CLAUDE.md §Gate map)
cargo run -p tsv_debug compare tests/fixtures/path/input.svelte  # diff with prettier
cargo run -p tsv_debug ast_diff tests/fixtures/path/input.svelte # verify AST equivalence
```

`fixtures:update` is only for after creating a fixture or when upstream sources change (Svelte/prettier versions) — never to "fix" failing tests (fix the code). See [Debug Tooling](#debug-tooling).

## Commands

### Build & Development

```bash
deno task build            # workspace dev build
deno task build:release    # optimized build minus the binding crates (each builds alone so tsv_cli/tsv_debug features don't unify into them)
deno task build:all        # release + ffi + build:packages + build:napi:packages (everything)
deno task build:packages   # the 6 WASM bundles: 3 publishable npm packages + their 3 deno bundles (benches); single source of truth for CI + publish.ts
deno task build:bench      # what `bench`/`smoke` measure and EVERY bench leg builds (ffi×3 + 3 wasm:deno variants + napi + wasm:all:nodejs)
deno task install-cli      # build the release CLI and install it to ~/.local/bin/tsv (the local daily driver)
deno task clean            # clean build artifacts
cargo build --workspace [--release]   # or -p tsv_cli / -p tsv_debug
```

Binding builds (`build:ffi*`, `build:wasm:*`, `build:napi*`, `build:npm:*`): [JS Bindings](#js-bindings).

### CLI Usage - Parse & Format

Parser auto-detected from extension (the JS/TS family → TypeScript, `.svelte`, `.css`); `--content`/`--stdin` require `--parser svelte|typescript|css`. For TypeScript, `--source-type script|module` selects the parse goal (for `format`, `--content`/`--stdin` only; `parse` is module-only by default). A `format` **path** parses as a **module, retried as a script** if that fails — except `.mjs`/`.mts`, modules by name, which take no retry; which error is reported when both fail is stated in [Strictness](#strictness-module-strict-script-by-directive).

`format` writes paths **in place** (only when output differs) and prints changed paths; `--content`/`--stdin` print to stdout. Directories recurse over the JS/TS family (`.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs`, all parsed as TypeScript — JSX/TSX out of scope), `.svelte`, and `.css` with gitignore-aware, reproducible discovery; scope rules in [Configuration](#configuration) (full: ./docs/cli.md §Multi-File Formatting). A named file must have a supported extension (else an argument error — for `parse <file>` too, unless `--parser` names the grammar). `--list` prints the in-scope files without formatting (path mode only; an empty scope exits 0). Files format in parallel; `--jobs N` overrides the worker count (both bounded per ./docs/cli.md §Multi-File Formatting). Exit codes: 0 clean, 1 would-change (`--check`, also with `--content`/`--stdin`), 2 errors; missing path args fail upfront, per-file and traversal errors report and continue.

```bash
cargo run -p tsv_cli parse file.ts                                       # compact JSON
cargo run -p tsv_cli parse file.ts --pretty                              # formatted JSON
cargo run -p tsv_cli parse file.ts --locations                           # add per-node loc (the default wire is span-only)
cargo run -p tsv_cli parse --content '<div>x</div>' --parser svelte      # parse string (preferred for agents)
cargo run -p tsv_cli parse --stdin --parser svelte                       # parse stdin (not preferred for agents)
cargo run -p tsv_cli format file.svelte src/lib                          # format files/dirs in place
cargo run -p tsv_cli format --check src/lib                              # list would-change files, exit 1 (CI)
cargo run -p tsv_cli format --list src/lib                               # list in-scope files (no formatting)
cargo run -p tsv_cli format --content '<div>x</div>' --parser svelte     # format string to stdout
```

### Testing & Code Quality

"In `check`" = gates in `deno task check`. A task that needs `deno task bench:install` (node_modules — CI installs none) can't gate there.

```bash
deno task check          # full committed-tree gate (benches/js/CLAUDE.md §Gate map)
deno task doctor         # one-pass setup check: runtimes, pins + checkout alignment, node_modules freshness, oracle
#                          checkouts, corpus, build artifacts. Exit 1 only on MISLEADING state (pin drift, skew, stale
#                          deps); absences warn (--strict promotes them) — except the optional experimental-typechecker
#                          tier, informational at any strictness (a BROKEN checkout there still warns)
deno task typecheck      # cargo check
deno task typecheck:features # cargo check of each binding crate (tsv_wasm/tsv_ffi/tsv_napi) under each single feature,
#                          then the three language crates without the `locations` feature tsv_cli/tsv_debug unify in —
#                          per-package builds `cargo check --workspace` and clippy's `--all-features` both miss; in `check`
deno task typecheck:js   # deno check over the bench harness, scripts/ + the tsv_debug sidecar. NOT in `check` (node_modules)
deno task typecheck:packages # the STAGED npm packages' .d.ts as a consumer compiles them: lean consumer modules + every
#                          ts/js README block (checkJs) through the `exports` map (strict, nodenext + bundler; wasm
#                          packages with the DOM lib, the napi loader and every `./locations` without). Args pick
#                          packages (format/parse/all/napi; none = every staged one). NOT in `check` (node_modules);
#                          publish.ts Step 6 stages the napi loader and runs it over all four (docs/audits.md)
deno task typecheck:scripts # deno check over scripts/ alone — node-modules-free, so in `check` (nothing else typechecks
#                          the release scripts); excludes `scripts/doctor.ts`, whose corpus probe reaches node_modules
deno task typecheck:bench-core # the node-modules-free bench modules scripts/'s import graph misses
#                          (`lib/{wasm,harvest_stamp,fixture_documents,error_text}.ts` + `compose_reports.ts`); the rest
#                          of that core rides `typecheck:scripts` or `test:deno`. Scope rationale: deno.json's `//` note; in `check`
deno task test           # cargo test
deno task test:deno      # deno tests over the node-modules-free bench core (divergence detectors, format-config probe,
#                          span-only wire probe, gate_counts, perf-omit summary, Prettier-suite filter, loc tolerance
#                          rows, parse diff engine + divergence matchers, NDJSON reader) + scripts/ (`changelog_test.ts`
#                          — the changelog grammar publish.ts writes and release_notes.ts reads; `npm_api_test.ts` — the
#                          npm options facade over a fake engine; `typecheck_packages_test.ts` — the README fence +
#                          marker grammar); in `check`
deno task test:audits    # cargo test -p tsv_lang --features audits — the `swallow_check` + `comment_check` seams' own tests; in `check`
deno task lint           # cargo clippy
cargo fmt                # Rust
tsv format .             # the repo's own TS/JS — tsv formats itself (`--check` in the gate)

cargo test --workspace strict_reserved_words_are_binding_names  # one test by name
cargo test --workspace --test fixtures_tests           # fixture validation tests
cargo test --workspace --test cli_tests                # CLI integration tests
```

`cargo fmt` (Rust) and `tsv format` (TS/JS) are the repo's ONLY autoformatters and partition it; markdown and JSON are hand-maintained. **Never run `deno fmt` or `prettier` on the repo** — tsv ships no config for them, so they'd churn every file (the prettier oracles pass options inline). `tsv format` on a directory is safe: the root `.formatignore` prunes tests/fixtures/ and tests/fixtures_compile/ (deliberately not format fixed points), and a file named under them is skipped too.

### Fixtures (Rust + Deno-based)

All `fixtures:*` tasks accept positional patterns (multiple = OR); `fixtures:validate` and `fixtures:update:parsed` also take `--list`, and `fixtures:validate` `--prettier-only`.

```bash
deno task fixtures:list              # list all fixtures
deno task fixtures:init <dir>        # create/reinit a fixture (= `tsv_debug fixture_init`; --content/--stdin/--force/--goal)
deno task fixtures:validate          # validate (--prettier-only skips our parser/formatter)
deno task fixtures:update            # regenerate expected.json + output_prettier.svelte (source of truth)
deno task fixtures:update:parsed     # expected.json only (when the parser changes)
deno task fixtures:update:formatted  # output_prettier.svelte only
deno task fixtures:audit             # audit _prettier_divergence fixtures (diagnostic; --all for every fixture)
deno task fixtures:ts-audit          # which input.ts fixtures need .ts vs could be .svelte (= `ts_fixture_audit`)
deno task compile:fixtures:init      # create/reinit a compile fixture (oracle-compiles + canonicalizes; tests/fixtures_compile)
deno task compile:fixtures:validate  # compile fixtures: oracle freshness + expected idempotence + ours parity, all gating.
#                                      Preflights `deno task conformance` — the ONLY place oracle freshness is graded (the
#                                      sidecar-free slice in cargo test compares to the COMMITTED file, so it stays green
#                                      when tsv and the file drift from the oracle together)
```

**Standing audit gates** — full reference ./docs/audits.md (what each proves, blind spots, flags, where it gates; its overview table maps every task). Read the relevant section before running or modifying an audit. RATCHET audits grade against a committed known-bug snapshot (`*_known.txt`); each has an `:update` task that re-pins after a fix and refuses a narrowed run. Everything below gates in `deno task check` unless noted.

```bash
deno task conformance:audit          # doc/fixture integrity: divergences cataloged, every Markdown link resolves, divergence READMEs back-link, no catalog-family drift
deno task conformance:audit:compiler # compile-fixture divergence integrity + checklist ↔ `Refusal` drift
deno task variants:audit             # `_compact`/`_spaces` variant DIRECTION: a `_compact` may never widen a whitespace gap,
#                                      a `_spaces` never empty one (bare suffixes only — a qualified name like
#                                      `unformatted_ours_hug_spaced` is the escape hatch for a deliberately bidirectional variant)
deno task canonicalize:audit         # canonicalize_js idempotence + output validity + comment preservation
deno task pins:audit                 # canonical-oracle PIN AGREEMENT (a repo fact): sidecar.ts VERSIONS + npm: imports,
#                                      benches/js/package.json, actor.rs acorn import-map and the sidecar deno.lock must be
#                                      identical, as must the prettier OPTIONS (`benches/js/lib/canonical.ts` `PRETTIER_OPTIONS`
#                                      + the sidecar's inline call). The lock also pins the oracle's transitive deps no literal
#                                      names (`LOCKED_TRANSITIVE`: esrap, which PRINTS svelte's compiled JS)
deno task pins:lock                  # REGENERATE the sidecar lockfile (crates/tsv_debug/src/deno/deno.lock) after a canonical
#                                      pin bump — frozen at runtime, so this is the only way it moves; `--check` reports drift.
#                                      Not a gate. `--allow-fresh` passes deno's 24h `minimumDependencyAge`, needed ONLY for a
#                                      version published in the last day (the lock reproduces flag-free once it ages out)
deno task pins:audit:checkouts       # checkout ALIGNMENT (an environment fact): a PRESENT ../svelte or ../acorn-typescript
#                                      must match its pin (absent → skipped; commit drift warns). Gates in `deno task
#                                      conformance`, reported by doctor — NOT in check (nothing there reads the checkouts)
deno task format:audit               # `tsv format --check .`; fails on a would-change file (exit 1) OR a parse error (exit 2)
deno task docs:audit                 # rustdoc `[link]`s resolve (doc lints DENIED, private items, `--all-features`); a dead link is a STALE DOC
deno task scan:audit                 # no new raw find/rfind/match_indices substring scans over source
deno task fanout:audit               # no exponential doc-node rebuild fanout (per-layout-candidate blowup)
deno task roundtrip:audit            # format(tests/fixtures) must reparse with its NODE POPULATION conserved (every node by
#                                      type, minus the shells/separators the formatter rewrites by design, plus every word of
#                                      template text) — zero-tolerance; names a dropped element where the skeleton compare only
#                                      reported it as divergent. Pure Rust; real yield on external corpora
deno task roundtrip:audit:prettier   # the same over the pinned prettier suites — `check`'s ONLY non-format-stable corpus, so
#                                      the only leg there reaching a valid→unreparseable regression. Warn-skips without ../prettier
deno task discovery:audit            # `tsv format --list ../corpora/collections` must name EXACTLY the snapshot's committed files
#                                      in tsv's extensions (the corpus is the snapshot's tree, so a discovery prune can't silently
#                                      shrink every consumer's corpus). Refuses a dirty checkout; warn-skips without ../corpora
deno task check:loc                  # `loc` CROSS-GRADE: tsv's loc wire must deep-equal the shipped `locations.js` reconstruction
#                                      of its span-only wire over every fixture input (one `tsv_debug loc_wires` process) — the
#                                      only `check` leg grading `locations.js`; the Rust loc wire is graded by `tests/loc_definition.rs`
deno task binding:audit              # comment↔token re-binding (HARD fails the gate, SOFT informational)
deno task authoring:audit            # authoring-independence over Svelte boundary whitespace: one fixed point per document
deno task paren:audit                # authoring-independence over redundant PARENS: `a ?? b ?? c` must format like the twin
#                                      prettier rebalances it from (`a ?? (b ?? c)`), and `a < X > c` like `(a < X) > c` /
#                                      `a < (X) > c`. Zero-tolerance, no ratchet (a redundant paren carries no authoring signal);
#                                      invisible on any formatted corpus. `--require-relational` (here, not in `audit:corpus`)
#                                      floors relational sites, which only tests/fixtures holds — a narrowed run drops it
deno task fuzz:audit                 # seeded mutational fuzzer: no-panic + idempotency + structural reparse (node loss HARD,
#                                      skeleton divergence soft); its dictionary carries `// prettier-ignore`, the only standing
#                                      instrument composing a freeze with a comment or a paren shell
deno task swallow:audit              # `//` line comment swallowing following output-line content (also over real code via audit:corpus)
deno task comments:audit             # print-once comment ledger: DROPPED / DOUBLE-PRINTED comments
deno task gaps:audit                 # gap-injection RATCHET (~37 s): ledger DROPPED/DOUBLE-PRINTED + SWALLOW + a bare reparse
#                                      of every output (UNREPARSEABLE — a comment moved into a slot the grammar forbids, or a
#                                      valid output the parser over-rejects; no as-authored gate reaches it) (./docs/gap_audit.md; :update, :rank)
deno task blanks:audit               # blank-line injection RATCHET (node loss among its pinned kinds) + the blank-DROP absorb
#                                      pin (a new kind of silently-eaten blank fails), ~30 s (./docs/blank_audit.md; :update)
deno task fabrication:audit          # blank-FABRICATION on pristine seeds — the F1-blind counterpart to blanks (ratchet born EMPTY; :update)
deno task census:audit               # comment CENSUS: raw input-vs-output trivia multisets per language (own scanners, never
#                                      parse().comments) — parse-time drops/merges/rewrites the ledger can't see; the Svelte scanner
#                                      also counts open-tag names and `{#…}`/`{@…}` heads (roundtrip's parse-time complement) (:update)
deno task width:audit                # print-width RATCHET: a new KIND of over-width line — the ONLY gate measuring a column.
#                                      ⚠️ NOT a debt list (sanctioned overruns are real); :update
deno task ignore:audit               # `prettier-ignore` RATCHET: honoring, second-pass stability, freeze scope, trailing
#                                      inertness, output reparse (a freeze emitting a dead document) (:update)
deno task razor:audit                # print-width RAZOR SWEEP: pads a text word to walk each Svelte seed across column 100,
#                                      grading F1 + the stray line-head boundary space at every width — the ONLY instrument
#                                      varying WIDTH, and the only one seeing a mangled form that is its own fixed point
deno task engines:audit              # ENGINE PARITY: wasm32 (under Node, the host `cli.js` ships for) and native builds must
#                                      produce the SAME BYTES — exit code, changed paths, diagnostics, every file — over both
#                                      fixture trees + ../corpora when present. Needs both packages built: NOT in check; gates
#                                      in CI's `artifacts` job
deno task render:audit <paths>       # render-equivalence over REAL Svelte (sidecar; NOT in check; release-gated via `deno task conformance`)
deno task idempotency:sweep          # F1 sweep over the real-code corpus (minutes; NOT in check; conformance cadence)
deno task audit:corpus               # the content-loss/robustness bundle over REAL code (publish Step 3c; NOT in check)
deno task wire:audit                 # WIRE-INJECTION: whitespace injected into every Svelte tag/block head, the wire graded
#                                      against the canonical parser on both arms (loc definition check + `loc` tolerance rows,
#                                      and span), each variant against its OWN base (a divergence fixture contributes nothing).
#                                      A CENSUS (`--inject-limit 0`): a sample redraws on every fixture edit, so only a census is
#                                      gradeable. Green; needs the canonical parser, so NOT in check
deno task wire:audit:terminators     # same harness, injecting a lone CR / U+2028 / U+2029 anywhere (where ECMAScript's
#                                      terminators and `\n` disagree; tsv's `loc` counts `\n` alone, so the added oracle
#                                      differences fall in the `two_line_classes` row) — grades the SPAN arm (+ the loc
#                                      definition check) on inputs no fixture or real repo carries. Sites are document-wide,
#                                      so it stays a strided SAMPLE and CANNOT be ratcheted. ⚠️ RED BY DESIGN
deno task compile:corpus:compare     # compile-parity wide net over ../corpora, the private live roots and Svelte's suites (sidecar, on demand; ./docs/compile_tooling.md)
deno task compile:validation         # validation-suite RATCHET over Svelte's compiler-errors + validator suites (sidecar, on demand; :update re-pins, never a MISMATCH; ./docs/compile_validation_ratchet.md)
deno task compile:fuzz               # differential compile fuzzer over feature cross-products — a discovery tool, RED by design (sidecar, on demand; ./docs/compile_tooling.md)
```

**Creating new fixtures** (`fixture_init` formats through prettier + generates `expected.json`; see ./docs/fixture_workflow.md, and use `--prettier-only` with `fixtures:validate` during fixture design):

```bash
cargo run -p tsv_debug fixture_init tests/fixtures/path --content '<script>your code</script>'
echo '<script>code</script>' | cargo run -p tsv_debug fixture_init tests/fixtures/path --stdin
cargo run -p tsv_debug fixture_init tests/fixtures/path  # reformat existing input file
```

### JS Bindings

Three binding crates:

- `tsv_ffi` (C ABI) — any FFI (Deno, Python, etc.); `libtsv_ffi.so` / `.dylib` / `.dll`
- `tsv_wasm` (wasm-bindgen) — browser, Deno, Node; format / parse / all variants via cargo features
- `tsv_napi` (napi-rs) — Node.js / Bun native addon (`libtsv_napi.*`, loaded via `process.dlopen`), built with the `napi` profile (`release` + `panic = "unwind"` → `target/napi/`; every export is `catch_unwind`, so a panic throws a JS error instead of aborting the host). Its npm surface (`@fuzdev/tsv`, wasm-API-parity by contract) is in [Publishing](#publishing); tested per OS by `test:napi:npm`; expected to eventually subsume the WASM native path. See ./crates/tsv_napi/CLAUDE.md.

`tsv_wasm` produces three npm packages from one crate via the `format` + `parse` cargo features (default = both): `@fuzdev/tsv-format-wasm`, `@fuzdev/tsv-parse-wasm`, and `@fuzdev/tsv-wasm` (everything + the `tsv` CLI), each with its own output directory.

```bash
deno task build:ffi                  # C FFI, full → target/release/libtsv_ffi.so
deno task build:ffi:format           # C FFI, format-only (size only) → target/ffi-format/release/ (also :parse; :all builds all three)
deno task build:napi                 # N-API addon (napi profile) → target/napi/
deno task build:napi:packages        # + staged npm packages (loader + host platform pkg) → crates/tsv_napi/pkg/
deno task build:wasm:deno            # deno WASM (requires wasm-pack), format-only → pkg/format/deno/
deno task build:wasm:parse:deno      # deno WASM, parse-only → pkg/parse/deno/
deno task build:wasm:all:deno        # deno WASM, full (benches) → pkg/all/deno/
deno task build:npm:format           # publishable npm package → pkg/format/npm/ (also :parse; :all adds the tsv bin)

# Or directly (the cargo-built `build:*` tasks above also STAMP their artifact for the `:run` freshness guards —
# deno.json `//build:stamped`; a bare `cargo build` does not)
cargo build -p tsv_ffi --release
wasm-pack build crates/tsv_wasm --target deno --release --out-dir pkg/all/deno
wasm-pack build crates/tsv_wasm --target deno --release --out-dir pkg/parse/deno -- --no-default-features --features parse
```

### Publishing

npm is the package surface (each tag's GitHub Release carries only notes + the native CLI binaries npm already ships). Three packages from the WASM crate, plus the N-API set:

- `@fuzdev/tsv-format-wasm` — format only (`--no-default-features --features format`)
- `@fuzdev/tsv-parse-wasm` — parse only; bundles the hand-maintained `tsv_ast.d.ts` (`crates/tsv_wasm/types/`) + the pure-JS line/column helper behind `{locations: true}` (`crates/tsv_wasm/npm/locations.js` + `.d.ts`; also exported alone, engine-free, as the `./locations` subpath)
- `@fuzdev/tsv-wasm` — both features; bundles the above and ships the `tsv` bin (`crates/tsv_wasm/npm/cli.js` — `format` + `parse` mirroring `tsv_cli`'s flags/exit codes; argv parsed by a zero-dep transcription of argh's grammar). Path mode fans onto `node:worker_threads` (workers get the main thread's compiled module via the `./worker` entry); `--jobs N` keeps the native ceiling, while the default pool is sized **per engine**, below the native CLI's — ./docs/cli.md §Binary Structure

**Naming.** Every npm name tsv publishes is kebab-case (`@fuzdev/tsv`, `@fuzdev/tsv-wasm`, `@fuzdev/tsv-format-wasm`, `@fuzdev/tsv-parse-wasm`, `@fuzdev/tsv-<triple>`) — one spelling with the `tsv-<triple>` release assets and the `fuzdev.tsv-format` VS Code extension (vsce forbids `_`), and the `-wasm` convention of tsv's Rust-tooling peers; the rule for every `@fuzdev` WASM or native-delivery package (`@fuzdev/blake3-wasm` too). Rust crates (`tsv_wasm`, `tsv_napi`) and the rest of the `@fuzdev` libraries stay snake_case; crate-derived file names inside a package (`tsv_wasm.js`, `tsv_wasm_bg.wasm`, `tsv_napi.node`) are internal and follow the crate.

**The N-API set** — tsv's native distribution under the one bare name: the `@fuzdev/tsv` loader + six `@fuzdev/tsv-<triple>` platform packages, each shipping **two binaries**: the addon (`tsv_napi.node`, `napi` profile) and the real `tsv_cli` binary (`tsv`/`tsv.exe`, plain un-PGO'd `release`, matching the benched artifact). The loader's `tsv` bin (`bin.js`) execs that binary — `npx tsv` IS the native CLI — falling back to the shared `cli.js` mirror. Staged by `deno task build:napi:packages`; publishes **only** through the tag-triggered `.github/workflows/release_napi.yml` (6-target container-pinned matrix → idempotent platforms-then-loader publish → the tag's **GitHub Release**, whose body is `CHANGELOG.md`'s stamped section and whose assets are the native binaries pulled back from the registry + `SHA256SUMS`, each attested), **never** `scripts/publish.ts`. `workflow_dispatch` is a dry-run rehearsal by default (`dry_run=false` is the failed-tag recovery path, dispatched **on the tag**); a weekly cron force-dry-runs it. Packages, triggers, gates, auth, and the Release job: ./crates/tsv_napi/CLAUDE.md §The npm packages and §Release.

A types-only `@fuzdev/tsv-ast` package is deferred — `import type` from `@fuzdev/tsv-parse-wasm` is zero-runtime-cost; reconsider when a real consumer appears.

Version source of truth: `Cargo.toml` `[workspace.package] version` (read directly by `wasm-pack`). No root package.json, no changesets; all published packages move together.

Package shape: wasm-pack `web` target, then `scripts/patch_npm_package.ts` adds a Node/Bun entry (sync auto-init), a browser entry (guarded `await init()`), `index.d.ts`, conditional `exports`, npm metadata, and the variant README. Every entry publishes through one hand-written facade shared with the native `@fuzdev/tsv` (`crates/tsv_wasm/npm/api.js` + `api_parse.js`: the `(source, options?)` bags, their errors, and `{locations: true}` over the span-only wire — ./crates/tsv_wasm/CLAUDE.md §The npm Facade). The export list is extracted from the generated JS, so new `lang_bindings!` languages flow through automatically.

`scripts/publish.ts` orchestrates the release: preflight → bump → check → conformance:all → audit:corpus (Step 3c) → build npm packages + deno bundles → verify → artifact validation (size bounds + Deno smoke + Node tests + `test:bun` over the three wasm packages + `typecheck:packages` over all four npm packages, the napi loader staged) → idempotent npm publish → git commit + tag + push, printing a wasm size summary. It stamps CHANGELOG.md's `## Unreleased` into the released version (which the tag's GitHub Release reads back); that section must be non-empty with a `<!-- bump: <level> -->` marker matching `--bump` (both required; a fresh empty `## Unreleased` is seeded on stamp; shared grammar: `scripts/changelog.ts`; agents never edit it — [Releases](#releases)). A failed wetrun resumes at **every** step, git finalize included — the retry sentinel is removed only once the push lands, so re-run `--wetrun` with no `--bump`: nothing publishes twice and the tag finishes.

**Conformance gates (Step 3b)** — `deno task conformance:all` (see [Corpus Comparison](#corpus-comparison)); skipped by `--no-check`. Preflights the oracles (`../svelte`, `../acorn-typescript`, `../typescript`, `../test262`, `../prettier`, `../prettier-plugin-svelte`, the `../corpora` snapshot + the `benches/js` `node_modules` sidecar): a **`--wetrun` FAILS** when any is missing (releasing without gates requires the explicit `--no-check`); a dry-run warn-skips, re-warned in the final summary. `deno task doctor` checks the same ahead of time. Only the CSS-WPT harvest stays manual. A `corpus:compare:format` SAFETY hit is self-verified in-run (the native format re-runs and must reproduce byte-identically), so treat it as real; FFI nondeterminism surfaces as a loud `native format nondeterminism` per-file error instead (./benches/js/CLAUDE.md §Known Issues). A caught **panic** hard-fails either corpus tool on every run — a shipped artifact would abort the host, so it must never grade as one more per-file error.

**Bun (Step 6)** — `deno task test:bun format parse all` (the only leg grading the engines' deletes of Bun's own `Error` `line`/`column`) over the three wasm packages. Bun is preflighted in Step 1 like Step 3b's oracles: a **`--wetrun` FAILS** without bun, before the bump (`--no-check` waives); with bun the leg runs `--require-bun`; without, a dry-run (or waived wetrun) warn-skips, re-warned in the summary. `deno task doctor` reports bun. The napi engine under Bun is graded only locally (`test:napi:npm[:run]` on a machine with bun), never by a publish.

```bash
deno task publish                        # dry-run: validate everything, no mutation
deno task publish --wetrun --bump patch  # release (--bump required, must match the CHANGELOG marker)
deno task publish --wetrun               # resume a failed wetrun (sentinel retry only)
# Flags: --bump patch|minor|major, --no-check, --no-git
deno task test:npm[:parse|:all]          # build the npm package + Node tests against it (:all adds CLI tests; `:run` skips the rebuild — freshness-guarded)
deno task test:napi:npm                  # stage the napi loader + host platform package + Node tests on the packaged shape (`:run` as above)
deno task test:bun [--require-bun] [format|parse|all|napi] # the parse-failure table over the staged packages under Bun, both engines; no build,
#                                        freshness-guarded, warn-skips without bun (`--require-bun`: fails). Each package task's `:run` chains it
deno task validate:artifacts             # tight wasm size bounds + Deno smoke of all built bundles, lazy entries included (fails if nothing
#                                        is built or what is built is STALE — `pkg/` is gitignored)
```

`scripts/validate_artifacts.ts` holds deliberately tight (~±8%) size bounds — a legitimate binary size change fails the publish until the constants are updated, keeping size moves visible and intentional.

**TS type maintenance**: `crates/tsv_wasm/types/tsv_ast.d.ts` is hand-maintained — any change to the wire JSON a writer emits (`crates/tsv_*/src/ast/convert/write*`) must update it. `deno task check:ast-types` (in `check`) catches drift three ways: the curated `tsv parse --locations` samples still type (the live writer; the only typing of the `loc` fields, since committed files are span-only); every `type` discriminant the fixture corpus produces is declared or explicitly opaque; and a computed minimal cover of every gradable field slot (`ParentType.key -> ChildType`) in the committed `expected*.json` types against the `.d.ts` — grading it against the **canonical** wire, which composed with `fixtures_tests` (`tsv == expected.json`) covers every position tsv's span-only wire emits. Per-field checklist, the Svelte-built-node rule (a node Svelte constructs is not the acorn node of the same `type` — the tell is a missing `loc` in Svelte's own wire), and the comment-attachment rule: ./crates/tsv_wasm/CLAUDE.md §TS Type Maintenance.

### Corpus Comparison

Compare formatting against Prettier, and parse output against the canonical parsers, on real code: the `../corpora` snapshot (`fuzdev/corpora` — the author's repos, the sveltejs repos (kit, svelte, svelte.dev, language-tools) and third-party Svelte libraries, one collection per upstream, tiered by `benches/js/lib/corpus.ts` from the snapshot's manifest; the whole `collections/` tree is pinned by git tree id in `GATE_CHECKOUT_IDS`, so a tooling commit in the snapshot repo moves nothing). Full runs enforce **pinned expected counts** over the `gates` view: exact format `unknown`/`partial` and parse `compared`/tsv-failure counts, a `match` minimum, SAFETY over every file. A snapshot refresh is a deliberate re-pin (`benches/js/lib/gate_counts.ts`, ./docs/gate_counts.md).

```bash
deno task corpus:compare:format ../some-project  # one project, or --all for the gates corpus (../corpora + prettier suites)
# Options: --explain (patterns matched), --summary, --json (stats + safety/partial/unknown/error lists; logs → stderr)
deno task corpus:compare:parse --all   # deep-diff parse ASTs vs acorn-typescript/svelte/parseCss
# Options: --multibyte-only, --filter <lang>, --limit <n>, --json

# These three gates accept -v, --json, <subtree>; the two fixtures gates freshness-check their ledgers on full runs
# (a stale sanction/known-gap entry fails) and warn on checkout↔npm version skew.
deno task conformance:svelte-fixtures  # tsv's Svelte parser vs Svelte's own test suite (../svelte; oracle = the live modern
#                                        parser). Verdict parity gates (an over-rejection must be SANCTIONED or a tracked
#                                        KNOWN_GAP, else exit 1); AST-shape diff is report-only triage
deno task conformance:ts-fixtures      # tsv's TS parser vs acorn-typescript's test suite (the adversarial TS edge cases).
#                                        A missing ../acorn-typescript (0 scanned) FAILS — publish Step 3b's preflight is
#                                        the tolerance point
deno task conformance:ts-repo          # tsv's TS parser vs ALL of ../typescript/tests/cases (every single-file .ts + every
#                                        TS unit of the @filename tests); oracle = tsc's .errors.txt baselines (TS1xxx = a
#                                        grammar reject), with tsc's LIVE parser splitting over-acceptance parser-level vs
#                                        checker-side and attributing goal / encoding artifacts before an over-rejection can
#                                        gate. Four freshness-checked ledgers (sanctioned = kept, known = to fix; acorn-shared
#                                        or not); only an UNTRACKED over-rejection fails; a missing/PARTIAL checkout or an
#                                        empty scan FAILS. ./docs/conformance_tsc.md; operator card: ./benches/js/CLAUDE.md

deno task conformance                  # pre-release aggregate, in order:
#   1. pins:audit:checkouts + bench:pins:suites — PIN-FRESHNESS: re-derives the conformance-view count pins no other
#      cadence grades (seconds when nothing moved; each stamped leg warn-skips an absent checkout; benches/js/CLAUDE.md §Harvests)
#   2. fixtures:validate + compile:fixtures:validate — ORACLE-FRESHNESS: `check` runs only each tree's sidecar-free slice,
#      which grades tsv against the committed file and can't see the oracle moving; only a prettier re-format can (~17 s)
#   3. bench:harvest:svelte-styles — re-extracts the CSS the `gates` view grades
#   4. in ONE process (benches/js/conformance.ts; oracles load once, fail-fast, FFI built once): the three gates above +
#      corpus:compare:parse --all + corpus:compare:parse tests/fixtures --fixtures (each fixture's parse-pinned documents,
#      grading the `loc` tolerance rows real code rarely reaches) + corpus:compare:format --all
#   5. render:audit over the version-pinned checkouts (a subprocess with its own sidecar)
#   The format leg's prettier calls ride a content-addressed cache (benches/js/lib/prettier_cache.ts; TSV_PRETTIER_CACHE=0 disables)
deno task conformance:test262          # tsv's JS parser vs test262 POSITIVES (pure Rust, `test262 --gate`); negatives (the
#                                        deferred early-error frontier) are reported, not gated. Exact POSITIVE_PASSED_PIN in the command
deno task conformance:all              # the full drop-in gate = `conformance` + `conformance:test262` — what publish Step 3b
#                                        runs. CSS-WPT harvest stays manual

deno task divergence:audit         # audit divergence pattern coverage (--json)
deno task corpus:stats             # corpus/candidate-dir sizes + language + degenerate-case stats (diagnostic; ./benches/js/CLAUDE.md)
```

Corpus comparison builds with `--profile corpus` (optimized + `panic = "unwind"`, no LTO — panics in our code are caught and reported; also the single build world every `deno task check` audit shares, trading LTO for build time, measurably free at runtime per the profile's comment in `Cargo.toml`). Benchmarks use `--release` (panic=abort, LTO) — except the N-API artifact, built with the shipped `napi` profile (`release` + `panic = "unwind"`) so its bench rows measure what ships.

Divergence detection identifies the known differences documented in the `conformance_prettier*.md` family (safety checks, pattern detection, traceability). See ./benches/js/CLAUDE.md and ./docs/divergence_detector.md.

### Benchmarks

**Cross-runtime.** One harness runs under **Deno, Node, and Bun**, each emitting its own runtime-labeled report (`report.{deno,node,bun}.{json,md}`, never merged); `deno task bench:compose` folds them into the combined `report.{json,md}` (what tsv.fuz.dev consumes). The native row is **FFI** under Deno, **N-API** under Node/Bun; all else is shared runtime-neutral code. ./benches/js/CLAUDE.md §Cross-Runtime.

**Perf vs conformance surfaces.** `bench:perf` measures a **real-world-only** corpus (app + framework source from the pinned `../corpora` snapshot, named by the report's `corpus_snapshot`) — the throughput headline; every in-scope tool must fully process every file or the run fails (`benches/js/lib/perf_omit.ts`), so coverage is 100% by construction. `bench:conformance` measures per-tool **parse coverage** over a **disjoint, fixtures-only** corpus (prettier suites + svelte compiler tests + the wpt-css/test262/tsc-corpus harvests) — **coverage-only and node-only by design** (no timed phase; runtime-invariant). Each set excludes what its own oracle calls invalid (the Svelte and tsc sets: what their canonical parser rejects; the prettier suites: what Prettier's specs and markers call invalid, plus harness files and the `.js` fixtures Prettier's parser reads as JSX; JS/TS suites are read module-then-script, approximating Prettier's own module-then-CommonJS retry), so each scores its oracle at 100% by construction, and the report splits coverage **per corpus source** (a group aggregate blends corpora — `parse/typescript` is mostly test262, i.e. ECMAScript). Coverage counts accepts and so rewards permissiveness; the opposite axis has inverted profiles, not gates: `deno task ts-repo:over-acceptance` (per-tool accepts over files tsc's parser rejects) and `deno task css:over-acceptance` (over files `parseCss` rejects; its reject-count pin makes movement in the deliberately unfiltered CSS reference row's grammar visible — the pin alone is `css:over-acceptance:pin`, a stamped `bench:pins:suites` leg, also graded by the coverage run). `deno task bench` = perf across all three runtimes + compose + the node coverage run. Correctness gates keep their own corpus scope. ./benches/js/CLAUDE.md §Corpus.

```bash
# One-time: install the harness's npm deps (package.json is the source of truth; runtimes share node_modules).
# Re-run after a dep bump or a plain `npm install` (which prunes the oxc-parser-wasm binding — benches/js/CLAUDE.md).
deno task bench:install

deno task smoke         # fast sanity check that every formatter+parser produces output (also smoke:node / smoke:bun)

# Benchmarks build the runtime's artifacts automatically.
deno task bench         # full refresh = bench:perf + bench:conformance + a closing bench:compose (needs node AND bun;
#                         the closing compose sees the just-rebuilt conformance report's vintage)
deno task bench:perf    # perf surface: build the whole artifact set ONCE, then the three :run legs + compose
deno task bench:deno    # Deno only (also bench:node / bench:bun). Each standalone leg builds the WHOLE artifact set:
#                         every report carries the same tsv size rows, so a half-built leg would publish the other
#                         binding's size at an older commit (deno.json `//bench:deno`; a `:run` leg warns). Warm ≈ a second
deno task bench:compose # fold per-runtime reports → combined report.{json,md}; warns when the conformance report's commit
#                         is behind the perf siblings' (the site publishes both)
deno task bench:deno:run   # run without rebuilding (also :node:run / :bun:run; aborts on stale artifacts)

# Conformance surface: per-tool parse COVERAGE → report.conformance.node.{json,md} (entries carry null timing)
deno task bench:conformance        # bench:pins:suites + build:bench + coverage run (also grades CSS_REJECTS_PIN)
deno task bench:conformance:run    # skip harvest + rebuild (freshness-guarded)
deno task bench:harvest            # regenerate every cache = bench:pins:suites + bench:harvest:svelte-styles (manual
#                                    refresh-everything; each caller takes only the group its corpus VIEW holds)
deno task bench:pins:suites        # the conformance-view group: the five SUITE caches (wpt-css, test262, tsc-corpus,
#                                    prettier-jsx, svelte-rejects) + the CSS reject pin. All freshness-stamped (--force after
#                                    harvest-logic changes) and `--if-present`. The PIN-FRESHNESS preflight of `conformance`;
#                                    `doctor` reports a stamp behind its checkout
deno task bench:harvest:svelte-styles # the PERF-view CSS cache: stamped on the ../corpora `collections/` tree id + its EXACT
#                                    block pin + the perf view's entry list (skips when none moved); REQUIRES every snapshot
#                                    collection, so it fails rather than warn-skips. Chained by `bench:perf`, and late by
#                                    `conformance` beside the `gates`-view legs that read it (benches/js/CLAUDE.md §Harvests)

deno task bench:deno:run -- --verbose   # per-file skip detail (counts always shown; paths/errors opt-in)

# Env vars (any runtime): BENCH_LIMIT, BENCH_FILTER, BENCH_DURATION, BENCH_WARMUP, BENCH_MODE,
# BENCH_CORPUS, BENCH_STALE_OK, BENCH_FORCED_ASYNC — semantics + defaults in ./benches/js/CLAUDE.md
BENCH_FILTER=zzz BENCH_LIMIT=10 deno task bench:deno:run
```

**Prerequisites**: `cargo install wasm-pack` + `deno task bench:install` once (needs npm/Node). Beyond that **Deno is the only hard dependency**; Node ≥ 22.18 (native TS type-stripping) for `bench:node`, Bun for `bench:bun`. The aggregate `bench` needs both and fails fast if either is missing (`bench:runtimes` preflights `bench:perf`; without it the miss would surface only after two of three siblings regenerated and `bench:compose` skipped). Its node arm probes `globalThis.Deno` rather than resolving the name — `deno task` puts a `node`→deno compat shim on PATH that would pass the check and run the harness AS Deno.

**Compares** canonical (prettier + svelte/compiler), native (FFI under Deno / N-API under Node+Bun), WASM, and alternatives:

- oxc-parser (an N-API row + a separate wasm32-wasi row), oxfmt, biome-wasm, dprint-wasm (the engine `deno fmt` runs; TS/JS only), malva-wasm (dprint's CSS plugin over the same formatter host; CSS only, enforced by the plugin)
- `tsc` — parse-only, conformance surface only; the language's definition, not a peer (its parser is error-recovering, so an accept means zero `parseDiagnostics`)
- yuku-parser — a Zig TS/JS parser, N-API + WASM bindings, parse-only, payload-matched to oxc; its lazy `parse()` and error-tolerant parser are corrected for in `benches/js/lib/yuku.ts`
- swc — parse-only TS/JS on both surfaces (its own AST dialect, so oxc-class payload disclosure; `decorators` must be enabled explicitly; goal axis `isModule`)
- postcss — parse-only CSS (the parser behind prettier's CSS printer; the only kind available, since no Rust CSS parser exposes an AST to JS)
- rsvelte's Svelte **parser** via its N-API addon — two `parse/svelte` rows: plain (mechanism-matched to tsv's default parse row; payload Svelte's own sparse-`loc` wire) and `skipExpressionLoc` (named for its option, since that reduction is not tsv's); the first third-party engine on that surface
- `rsvelte-fmt` (Svelte only) — a **coverage-only** row (accept rate, no timing: no in-process API, and a per-file subprocess row would rank process spawn, not format work); its end-to-end CLI numbers live in the separate hyperfine comparison on tsv.fuz.dev. See ./docs/benchmarks.md §Coverage-only rows.

Results: `benches/js/results/report.<runtime>.{json,md}` (committed; every row carries a `runtime` field) + the combined `report.{json,md}`. Publish to tsv.fuz.dev: `npm run update-benchmarks` in ../tsv.fuz.dev. See ./benches/js/CLAUDE.md.

### Performance Profiling

```bash
cargo run --release -p tsv_debug -- profile ../corpora/collections/zzz/src/lib        # profile a directory
cargo run --release -p tsv_debug -- profile file.ts --iterations 20  # more iterations; --json for machine-readable
cargo run --release -p tsv_debug -- json_profile ../corpora/collections/zzz/src/lib   # parse vs wire-JSON write (--locations: the loc emitter)
cargo run --release -p tsv_debug -- compile_profile tests/fixtures_compile  # Svelte compile vs the format wall
```

Function-level hotspots: `perf` with the `profiling` cargo profile:

```bash
cargo build --profile profiling -p tsv_debug
perf record --call-graph=dwarf -- target/profiling/tsv_debug profile ../corpora/collections/zzz/src/lib
perf report --stdio     # function-level hotspots
# line-level — -s takes the EXACT demangled name from perf report (a substring silently annotates nothing)
perf annotate --stdio -s 'tsv_lang::doc::arena_fits::arena_fits_with_lookahead'
```

See ./docs/performance.md.

## Configuration

**Non-configurable by design.** Formatting options are fixed at Prettier's defaults except the list below — no config files, CLI flags, or runtime options, and none planned (a narrower option set may be revisited far down the road; the 0.x contract is no configuration at all).

**`--source-type` is a grammar input, not a style setting** — it names which grammar symbol the parse starts from (ecma262's `ParseScript` / `ParseModule` are two entry points, not two styles); formatting has no knob (./docs/cli.md §`--source-type` is a grammar input, not a style setting). Leaving it unset is not a *default style* either: an unnamed goal parses at Module and retries at Script only on failure (./docs/cli.md §Multi-File Formatting), so every valid script formats unconfigured and no module-valid file's output moves.

**The one carve-out is file *scope*, not style.** Authoritative rules + edge cases (parent-directory rule, re-include idiom, unreadable ignore files, warnings): ./docs/cli.md §Multi-File Formatting. Core:

- Discovery is gitignore-aware with two regimes keyed on `.git`. **Inside a git repo** the **format root** (the scope boundary — derived from the argument, never the cwd) is the repo root, a hard stop for the upward walk; discovery honors `.gitignore`, then `.formatignore` (tsv's native file; its `!` can re-include a gitignore'd path), then `.prettierignore` (drop-in compat; the fallback in any directory with no sibling `.formatignore`), all hierarchical, plus the always-skipped safety nets (`.git`, `node_modules`, `.sl`, `.hg`, `.svn`, `.jj`). **Outside a repo** only `.formatignore` is read, from the filesystem root down (so `~/.formatignore` is global config for loose files). Because the boundary is found by walking up, a subdirectory named directly is bounded like one reached via an ancestor.
- A `.gitignore` in scope turns the built-in heuristic (hidden dirs + `dist`/`build`/`target`) **off**.
- A **named** file or directory is bounded by the ignore files alone: one a rule excludes is skipped (with a warning, except a file a `.formatignore`/`.prettierignore` rule excludes); the safety nets and heuristic prune only what a walk discovers. A named file must have an extension tsv formats (else an argument error, not a TypeScript parse; extension read ASCII-case-insensitively). A named symlink is graded as git grades it — a link, whatever it points at, so a directory-only rule doesn't match a symlinked directory argument.
- The matcher is the `tsv_ignore` crate (`IgnoreStack`); the per-directory prune decision (heuristic, safety nets, shadow warning) and the named-path gate are `tsv_discover` — shared with the JS CLI and the VS Code extension via WASM, and with native `@fuzdev/tsv` via N-API (`classify_dir` / `should_format_file` / `shadow_warning` / `is_path_pruned` / `path_shadow_warning` / `excluded_argument_warning`), so every surface agrees by construction.

Settings that diverge from Prettier's defaults (everything else, e.g. tabWidth=2, matches):

- `printWidth` (100) — wider than Prettier's 80
- `useTabs` (true), `singleQuote` (true)
- `trailingComma` ('none') — no trailing comma even when a list breaks; with useTabs + singleQuote this matches the Svelte project's own `.prettierrc`

**Measuring line widths**: `cargo run -p tsv_debug line_width <file>` — never `wc -c` (bytes, not visual chars; a tab is 1 byte, 2 visual chars). `compare` also shows widths on changed lines.

### Internal Configuration (Rust Library Only)

No runtime configuration. Print width / tab width / indent are compile-time `pub const`s in `tsv_lang::config` (`PRINT_WIDTH`, `TAB_WIDTH`, `INDENT`), read directly by the renderer, never threaded through signatures. Quote preference is hardcoded (single quotes) in `tsv_lang::printing` — the `optimal_string_quote` tie-break `format_string_literal` applies. Doc-builder unit tests exercise smaller widths via the internal `RenderConfig` seam (`doc::render_config`, `pub(crate)`), never at runtime.

One type carries per-input *state* (not configuration), threaded only where it varies: `tsv_lang::EmbedContext { base_indent_offset, first_line_offset, suffix_width, mode: LayoutMode, jsdoc_cast_cannot_hang, root_sequence_indents, printer_owns_line, value_end_takes_no_comment }` — embedding state for nested formatting (CSS in `<style>`, Svelte template expressions). `LayoutMode { Standalone, Embedded }` controls the expression-ROOT binary indent style (nested expressions format context-free); `root_sequence_indents` is its sequence counterpart, set by the Svelte **block head** alone (the one braced head prettier never width-wraps). The three width fields act only at **render** (on the context passed to an `arena_print_doc_*` call); a `build_*_doc` call reads only the five build-time fields (`mode`, `jsdoc_cast_cannot_hang`, `root_sequence_indents`, `printer_owns_line`, `value_end_takes_no_comment`), so a width set there is inert.

TypeScript formatting is identical for standalone `.ts` and Svelte-embedded TS: one entry point, `tsv_ts::format(&ast, source)`.

## Project Structure

```
tsv/
├── crates/
│   ├── tsv_lang/    # Foundation (span, location, error, doc builder, printing utils)
│   ├── tsv_arena/   # Shared binding substrate (tsv_ffi, tsv_napi, tsv_wasm): per-thread reusable AST/doc arenas + the goal axis
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
├── scripts/         # Publish orchestrator + changelog grammar, GitHub Release notes/assets, npm package patcher, Node artifact + N-API tests, AST type drift check, the script-run gates (pins, discovery, `loc`, engine parity) + doctor
├── benches/js/      # Cross-runtime benchmark + conformance harness (Deno/Node/Bun)
├── tests/           # Integration tests (parser, formatter, CLI)
│   ├── fixtures/    # Test fixtures organized by language/feature
│   └── fixtures_compile/ # Compiler fixtures (input.svelte + canonicalized oracle expected_server.js + expected.css) — separate tree so parser/formatter fixture counts stay unperturbed
└── docs/            # Documentation (fixtures, cli, architecture, etc.)
```

**Crate pattern** (tsv_ts, tsv_css, tsv_svelte): `lib.rs` (public API: `parse()`, `format()`, `convert_ast_json_bytes()`), `ast/` (internal AST + the wire-JSON writer), `lexer/`, `parser/`, `printer/` (uses the tsv_lang doc builder), and escapes (tsv_ts's `lexer/escapes.rs`, tsv_css's `escapes.rs`; Svelte delegates to TS/CSS). `tsv_ts` and `tsv_css` also export embedding APIs for `tsv_svelte` (`parse_embedded` + embedded-formatting variants); `tsv_ts` additionally exports the `build_*_doc` functions.

### Conformance

**Comment position is preserved by default — but the rule is principled, not absolute.** A core tsv stance and the largest category of deliberate Prettier divergence: a comment's placement usually communicates what it refers to, so tsv keeps comments where the author wrote them. Prettier routinely relocates comments across syntactic boundaries, often **losing information** — two comments merging onto one line (the second `//` becoming text), or reordering. tsv treats such a boundary as semantic and holds the comment in place.

The line: **preserve when the position carries authorship signal, or when relocating would lose information** (the common case). But tsv **deliberately trails** a same-line line comment past a *pure separator* when that is **lossless and the position carries no signal** — e.g. between a list element and its comma (`A // c⏎, B` → `A, // c`): the comma is structure, the comment trails the element either way, and per-element line breaks keep multiple comments distinct, so tsv matches Prettier. That carve-out is a deliberate choice, **not** a gap to close. (Contrast the name→`=`/`:`/`?` binding cases, where two comments *would* collide on one trailing line — there tsv preserves + continuation-indents to stay lossless, diverging from Prettier's merge.)

The **opening-delimiter** rule is uniform across the printer: a `//` the author glued to an opening delimiter keeps that line at every one — `fn(`, `new(`, `import(`, `function f(`, `[`, `{`, `Array<`, a retained type paren shell's `(`, a required operand pair's `(` (an assignment target, an instantiation head, a non-null / sealed-chain shell, a chain's sealed or IIFE base, an IIFE callee, a function tag), a unary comment-holder's `(`, `return (` / `throw (`, the aligned union-member object's `{`, and every statement header (`if` / `while` / do-while / `with` / `switch` / `catch`, all three `for` spellings). **Two** emitters state it — `Printer::split_open_delimiter_glued_run` (the paren shells, operand pairs, unary holder, restricted-production hang, every statement header) and `Printer::delimiter_line_comment_prefix` (the container and call families, and the aligned object's `{`) — and the author blank *below* the pulled comment **survives at all of them**, via one value for the family, `Printer::push_delimiter_glued_blank` (the pull moves the comment's line, not its membership; prettier keeps the blank everywhere too). A blank *above* the comment sits against the delimiter and stays erased. See ./docs/comments.md §The delimiter-line question. Prettier is **not** uniform: it un-glues at most, **glues** at for-in / for-of, and relocates the comment out of the parens at do-while — each a cataloged divergence. Don't confuse any of them with the *sanctioned* union/intersection carve-out in [docs/comments.md §Trailing and dangling runs](docs/comments.md#trailing-and-dangling-runs-the-separator-goes-before-each-comment-never-after) (a non-last member's redundant paren shell strips and its deferred `//` flushes at the per-member break, `Printer::type_member_separator_follows`) — a deliberate lossless choice the catalog owns. When a fix changes comment handling, default to preserving position; match Prettier only when trailing is lossless and the position carries no signal — otherwise add a `_prettier_divergence` fixture. Principles: ./docs/conformance_prettier.md §Comment Position Philosophy; catalog: ./docs/conformance_prettier_ts_comments.md §Comment relocation.

- ./docs/conformance_prettier.md - Where we differ from Prettier (and why) — the shared frame;
  the per-language catalogs are ./docs/conformance_prettier_css.md,
  ./docs/conformance_prettier_svelte.md, ./docs/conformance_prettier_ts.md,
  ./docs/conformance_prettier_ts_comments.md, and ./docs/conformance_prettier_ignore.md
- ./docs/conformance_svelte.md - Where we differ from Svelte (and why)
- ./docs/conformance_svelte_compiler.md - Where we differ from Svelte's compiler (expected to stay empty — a safety valve, not a budget)

## Fixtures

TDD workflow: [Development Philosophy](#development-philosophy-test-driven-development-with-fixtures). References: ./docs/fixture_workflow.md (creation), ./docs/fixture_overview.md (validation, troubleshooting), ./docs/fixture_naming.md (naming).

### Fixture Protection Rules

**Sources of truth**: Prettier and Svelte's parser. Fixtures record what these tools produce.

**When a fixture test fails:**

1. **Verify the fixture** against the sources of truth:

   ```bash
   cargo run -p tsv_debug compare <fixture>/input.svelte          # vs prettier
   cargo run -p tsv_debug canonical_parse <fixture>/input.svelte  # vs Svelte's AST (raw: carries the oracle's `loc`, which expected.json strips)
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

**Core Invariant**: Input file **always formats to itself** (idempotent) — the one deliberate opt-out is a `tsv_rejects.txt` fixture, whose input tsv *rejects* (the canonical parser accepts), so F1 doesn't apply (see F7/S20).

**Directory Hierarchy**: Each fixture directory has either an input file (fixture) or subdirectories (container), not both, not neither — and a container holds no files but an optional README.md.

**Organization**: by feature. Comment fixtures belong with the feature they test (e.g., `calls/chained/*_comment`); `syntax/comments/` only for basic comment syntax, universal formatting rules, and cross-cutting edge cases.

**Input File Types:**

- `input.svelte` (preferred) — tests code embedded in Svelte context. ⚠️ For CSS it's the only path with an external canonical source (./docs/fixture_overview.md#why-svelte-is-the-default-canonical-source)
- `input.ts` (rare) — only for byte-0 file-level features (hashbang, BOM) or constructs prettier formats differently between contexts (arrow type parameters: `<T>` in `.ts`, `<T,>` in Svelte). TS-only _syntax_ (`import =`, `export =`, types, decorators, `declare`) still uses `.svelte` with `lang="ts"`
- `input.css` (rare) — only for file-level CSS features (e.g., BOM at byte 0)
- `input.svelte.ts` (runes) — Svelte rune modules (`$state`, `$derived`, etc.)

`.ts`/`.svelte.ts` parse with acorn-typescript, `.css` with Svelte's `parseCss`; all format with prettier.

**Fixture File Structure:** `input.*` + `expected.json` at minimum. Every optional sibling makes a precise, validated claim (per-file semantics and F/S/R/D rules: ./docs/fixture_overview.md):

- parser divergence: `expected_ours.json` / `expected_svelte.json`; a sibling variant's parse pin: `expected_<stem>.json`
- formatter divergence + prettier multi-pass pins: `output_prettier.*` / `prettier_variant_*` / `variant_*` / `divergent_variant_*` / `prettier_intermediate_*` / `prettier_intermediate_to_variant_*` / `prettier_intermediate_to_divergent_variant_*` / `audit_signature.txt` / `audit_signature_<suffix>.txt` (the two chain pins, anchored at `output_prettier.*` and at `unformatted_ours_<suffix>.*`)
- no-oracle markers: `prettier_nonconvergent.txt` / `prettier_rejects.txt` / `tsv_rejects.txt`
- `goal` — parse-goal marker: the `.ts` input parses at `Goal::Script` on both sides (see `typescript/script_goal/*`)
- normalization variants: `unformatted_*` / `unformatted_ours_*` / `unformatted_prettier_*`
- `input_invalid_*` — must fail BOTH parsers, one syntax error per file

**Unformatted variant rules:** same content structure as input, usually only whitespace differing; the **bare** `_compact` / `_spaces` names claim exactly that, in one direction (gated by `deno task variants:audit`; ./docs/fixture_naming.md#standard-variant-names). A variant that also flips a *token* prettier normalizes away (a trailing comma, quote style, `<br>` for `<br />`) takes a name that says so (`unformatted_no_self_closing`, `unformatted_with_closing_tag`); older bare-named variants predate the rule and the audit reports them as ungraded. Both formatters must normalize to exactly match input. For `.svelte` this is **enforced** by the render-equivalence check (R rules, ./docs/fixture_overview.md): the variant and `input` must produce the same browser-visible render via `svelte compile`, so a formatter bug that changed the render *and* landed on `input` can't pass green.

**Quick Pattern Selection:**

- **Parser matches Svelte**: `input.svelte` + `expected.json`
- **Parser differs intentionally**: `expected_ours.json` + `expected_svelte.json` (`_svelte_divergence` suffix) — including a tsv over-acceptance (canonical **rejects**, only tsv parses), where `expected_svelte.json` is `{"error": "failed to parse"}`
- **Formatter matches prettier**: `unformatted_*.*` variants
- **Formatter differs intentionally**: `output_prettier.*` (`_prettier_divergence` suffix)
- **Prettier has stable variants (ours normalizes)**: `prettier_variant_*.*` (`_prettier_divergence`)
- **Dual-stable forms (both keep stable)**: `variant_*.*` (`_prettier_divergence`)
- **Divergent variant (prettier keeps stable, ours → third form)**: `divergent_variant_*.*` (`_prettier_divergence`)
- **Normalization to input divergence**: `unformatted_ours_*.*` normalizes to input with our formatter only
- **Normalization to output_prettier**: `unformatted_prettier_*.*` normalizes to `output_prettier.*` with prettier
- **Prettier's output from an `unformatted_ours_*` fits no single-form marker**: auto-generated `audit_signature_<suffix>.txt` pins the whole chain (N12) — the marker of last resort, for a chain with 2+ distinct intermediates or a stable form tsv can't format; `fixtures:update:formatted` decides, never hand-written
- **Prettier never converges (no oracle)**: `prettier_nonconvergent.txt` + README (`_prettier_divergence`; excludes all prettier-claim files)
- **Prettier rejects/throws on input (no oracle)**: `prettier_rejects.txt` (trimmed content = expected-error substring) + README (`_prettier_divergence`; excludes all prettier-claim files; mutually exclusive with `prettier_nonconvergent.txt`)
- **tsv over-rejects but canonical accepts**: `tsv_rejects.txt` (trimmed content = expected tsv-error substring) + `expected_svelte.json` + README (`_svelte_divergence`; no `expected.json`/`expected_ours.json`; excludes all format-claim files, `input_invalid_*`, and the prettier no-oracle markers)
- **The parse fact lives only in a form `input.*` can't hold** — a leading BOM with nothing load-bearing behind it (the format side strips it, so F1 forbids it), or a newline inside a region the canonical parser overwrites (Svelte's `_ as ` annotation window, which tsv's formatter always removes; prettier keeps some, e.g. `{@const a1⏎: T}`): pin a variant — an empty `expected_<stem>.json` beside variant `<stem>.*`, filled by `fixtures:update:parsed` (P4/S24; in-tree: each `bom_prettier_divergence`'s `expected_prettier_variant_bom.json`, `svelte/blocks/binding_annotation_multibyte`'s two `expected_unformatted_*.json`, and `svelte/blocks/each/context_annotation_comment_prettier_divergence`'s `expected_unformatted_ours_colon_newline.json`)
- **Both differ**: `_svelte_prettier_divergence` suffix

> **Troubleshooting:** ./docs/fixture_overview.md#quick-decision-tree

## Debug Tooling

**tsv_debug** uses an embedded Deno sidecar for JS tools (prettier, Svelte parser, acorn), spawned on first use and reused (orders of magnitude faster than per-call spawns). Verify with `cargo run -p tsv_debug check`.

### Commands

**Input methods** (content-processing commands): a file path (parser from its extension, which must be one tsv handles unless `--parser` names the grammar), `--content <string> --parser <type>`, or `--stdin --parser <type>` (`svelte|typescript|css`) — except the single-language commands (`canonical_compile`, `compile_compare`, `compile_fixture_init`, `render_compare`, `line_width`), which take `--content`/`--stdin` with no `--parser` (`render_compare`: two inputs, any mix of repeatable `--content` and file paths, no `--stdin`).

**Content-Processing Commands:**

```bash
# compare - diff our formatter vs prettier (line widths right-aligned on changed lines)
cargo run -p tsv_debug compare file.svelte
# Options: --verbose/-v (full input/ours/prettier), --quiet, --color <auto|always|never>, --json
# "Outputs match" = ours(input) == prettier(input), NOT input stability; a match on a non-format-stable
# input adds a note + input-vs-formatted diff (F1 fails on such an input)

# ast_diff - verify semantic equivalence
cargo run -p tsv_debug ast_diff input.svelte                         # round-trip: parse → format → parse → compare
cargo run -p tsv_debug ast_diff input.svelte output_prettier.svelte  # compare two files' ASTs
cargo run -p tsv_debug ast_diff --render input.svelte                # render-aware: collapse/trim template whitespace per
#   Svelte 5 first, so render-equivalent forms match; real content / <pre> / presence-of-space changes still differ.
#   Sound at corpus scale.

# canonical_parse - parse with the canonical parsers (Svelte, acorn+typescript, or Svelte's parseCss)
cargo run -p tsv_debug canonical_parse file.svelte

# compiler tools (Svelte→JS vs the canonical compiler; detail, flags and exit codes: ./docs/compile_tooling.md)
cargo run -p tsv_debug canonical_compile file.svelte      # canonical compile, deterministic oracle (--target, --css, --dev, --json)
cargo run -p tsv_debug render_compare a.svelte b.svelte   # do two sources render the same page? identical / cosmetic / visible
cargo run -p tsv_debug compile_compare file.svelte        # tsv's compile vs the oracle, canonicalized-JS diff (--json)
cargo run -p tsv_debug compile_fixture_init tests/fixtures_compile/feature/case --content '<p>text</p>'  # oracle-generated expected files
cargo run -p tsv_debug compile_fixtures_validate [pattern...]  # oracle freshness + ours parity + idempotence (./docs/audits.md)
cargo run -p tsv_debug compile_corpus_compare <paths...>  # compile-parity wide net; MISMATCH / OVER-ACCEPTANCE gate. Sidecar, NOT in check
cargo run -p tsv_debug compile_corpus_compare --ratchet [--update]  # validation-suite gate — ⚠️ always a SEPARATE invocation (./docs/compile_validation_ratchet.md)
cargo run --profile corpus -p tsv_debug compile_fuzz      # differential compile fuzzer — ⚠️ RED BY DESIGN, a discovery tool (--seed, --iterations, …)
cargo run --release -p tsv_debug -- erase_comment_census ../corpora/collections/fuz_ui  # type-eraser comment-refusal LOWER BOUND (pure Rust)

# format_prettier - format using prettier (line widths by default; --no-line-widths to hide)
cargo run -p tsv_debug format_prettier file.svelte

# line_width - measure visual line widths (pure Rust; --line N for one line with preview, --json)
cargo run -p tsv_debug line_width file.svelte
```

**Fixture Management Commands** (`fixture*` commands take positional patterns, multiple = OR; `fixtures_validate` and `fixtures_update_parsed` also `--list`; the audits take no patterns — `canonicalize_audit` takes paths):

```bash
# fixture_init - create/reinit a fixture (formats through prettier + generates expected.json)
cargo run -p tsv_debug fixture_init <dir> --content '<code>'   # or --stdin; bare = reformat existing input
# Also: --parser <svelte|typescript|css|svelte-ts> (aliases ts, svelte.ts), --force, --goal <script|module>
# (.ts/.svelte.ts only: writes the `goal` marker + generates expected.json at that acorn sourceType; omitted keeps
# the directory's existing marker)

# fixtures_validate - verify fixtures (CI). --prettier-only skips our parser/formatter. Cross-fixture duplicate
# detection is skipped when filters are active; a parser mismatch with expected.json is a hard error (no ratchet).
cargo run -p tsv_debug fixtures_validate [pattern...]

# fixtures_update - regenerate from canonical sources
cargo run -p tsv_debug fixtures_update            # both parsed + formatted
cargo run -p tsv_debug fixtures_update_parsed     # expected.json only (Svelte / acorn / parseCss; `loc`/`name_loc` stripped — span-only wire)
cargo run -p tsv_debug fixtures_update_formatted  # output_prettier.svelte (auto-deletes if identical to input;
#   skips the no-oracle markers prettier_nonconvergent / prettier_rejects / tsv_rejects)

# fixtures_audit - investigate normalization graphs (diagnostic; --all for every fixture, --verbose, --json)
cargo run -p tsv_debug fixtures_audit [pattern...]

# ts_fixture_audit - which input.ts fixtures need .ts vs could be .svelte (embeds each in <script lang="ts">, checks
# both formatters). Necessary = byte-0 feature, Svelte-parse-fail, or formats-differently; Convertible = formatting-safe
# only, not a mandate (.ts may be deliberate, to cover the standalone path); Intentional = the INTENTIONAL_TS allowlist.
# --verbose shows the TS-vs-Svelte diff on 'formats differently' fixtures.
cargo run -p tsv_debug ts_fixture_audit [pattern...]

# loc_wires - both parse wires (loc + span-only) of every fixture input (at its `goal`) and every
# `expected_<stem>.json` variant, as NDJSON `{path, language, source, loc, span}` — the Rust half of
# `deno task check:loc` (scripts/check_loc.ts reconstructs and compares). Pure Rust. --stdin answers NDJSON
# requests `{language, goal?, source}` with the loc wire (or a parse error, or a caught panic) — how
# `corpus:compare:parse`'s loc arm reaches the loc wire, which no binding ships (benches/js/lib/loc_wire_client.ts)
cargo run -p tsv_debug loc_wires [root]

# the pure-Rust integrity audits behind their `deno task` gates (detail: ./docs/audits.md)
cargo run -p tsv_debug conformance_audit          # = conformance:audit (--json)
cargo run -p tsv_debug compile_conformance_audit  # = conformance:audit:compiler (--json)
cargo run -p tsv_debug variant_audit              # = variants:audit (--list, --json)
cargo run -p tsv_debug canonicalize_audit tests/fixtures tests/fixtures_compile  # = canonicalize:audit (--json; also takes real-corpus dirs)
```

**test262 ECMAScript Conformance Tests** (./docs/conformance_test262.md; §Differential for tsv-vs-oxc):

```bash
# test262 - ECMAScript conformance tests against our parser (pure Rust; expects ../test262)
cargo run -p tsv_debug test262 [path-pattern]
# Options: --path <dir>, --list, --verbose, --negative-only, --positive-only,
#          --gate (the release gate: fails ONLY on a positive-parse regression or a shift in the pinned positive
#           count; negatives — the deferred early-error frontier — are reported, not gated. A bare run exits
#           non-zero by design: a diagnostic, not a gate),
#          --emit-manifest <path> (JSON manifest of the graded subset — feeds the tsv-vs-oxc differential,
#           benches/js/diagnostics/test262_compare.ts)
```

**Typechecker conformance (`tsc_conformance`) — EXPERIMENTAL, may never ship.** `tsv_check` is a from-scratch TypeScript binder + checker; no shipped artifact links it (`cargo tree -i tsv_check` → `tsv_debug`, plus the workspace root's dev-dependency), and the parser and formatter are never modified in service of it. `tsv_debug tsc_conformance` grades it against tsgo's committed `.errors.txt` baselines (`../typescript-go`, pinned in ./docs/typechecker.md) via **on-demand** tasks — none in `check`, `conformance`, or release gating, and `../typescript-go` is not a release-required oracle. ./docs/typechecker.md.

```bash
deno task conformance:tsc-roundtrip     # baseline parse → re-render → byte-compare (zero checker code)
deno task conformance:tsc-check         # the tsv_check conformance sweep + committed report
deno task conformance:tsc-check:update  # re-pin the run's snapshot counts after deliberate drift
```

**Performance Profiling Commands** (pure Rust, no Deno — ./docs/performance.md):

```bash
cargo run -p tsv_debug profile ../corpora/collections/zzz/src/lib                    # parse vs format timing (--iterations, --json, --flow-stats)
cargo run -p tsv_debug profile --bind ../corpora/collections/zzz/src                 # parse vs lower+bind timing (TS-only) + peak RSS (§1)
cargo run --release -p tsv_debug -- json_profile ../corpora/collections/zzz/src/lib  # the bindings' parse path: parse vs span-only wire write (§2)
cargo run -p tsv_debug buffer_sizes ../corpora/collections/zzz/src ../corpora/collections/gro/src     # printer SmallVec sizing histograms (§8)
cargo run -p tsv_debug arena_stats ../corpora/collections/zzz/src/lib                # DocArena node-population + memory audit (§7; --reuse, --list-errors)
cargo run --release -p tsv_debug -- compile_profile tests/fixtures_compile  # Svelte compile against the format wall (§9)
cargo run --release -p tsv_debug -- ast_census ../corpora/collections/zzz/src        # per-node-kind population (--bytes joins the size board, --slots) (§10)
cargo run -p tsv_debug type_sizes                                # the `size_of` board: every public AST type's width (§10)
cargo run -p tsv_debug metrics [--json]                          # line counts by crate and phase; also `deno task metrics`
```

The **density pair** (`ast_census` + `type_sizes`, also `deno task ast-census` / `deno task type-sizes`): a lever that narrows a type is worth `count x bytes saved`, and neither factor is guessable — `ast_census --bytes` multiplies them in one table.

**Audits** — every standing correctness gate and discovery harness (swallow, ledger, census, gap/blank injection, fabrication, ignore-honoring, fuzz, F1 sweep, render-equivalence, the corpus bundle, `lex_diff`, the compiler audits, and the rest) is cataloged in ./docs/audits.md; the `deno task` entry points are indexed in [Fixtures](#fixtures-rust--deno-based). Read the relevant section before running or modifying an audit.

## Architectural Notes

### Closed Scope, Open Convention

tsv ships a closed language set (TypeScript, CSS, Svelte) but is open by convention **at the Rust source/crate level**: each language crate (`tsv_ts`, `tsv_css`, `tsv_svelte`) is self-contained — owns its internal AST, parser, formatter, and convert layer — and exposes the same free-function API (`parse()`, `format()`, `convert_ast_json_bytes()`, `convert_ast_json_string()`). **No central `Language` trait, no registry, no enum dispatch.** So:

- **Optimal artifacts**: concrete types end-to-end, no dyn dispatch; WASM tree-shakes by feature at link level — `@fuzdev/tsv-format-wasm` excludes the convert layer, `@fuzdev/tsv-parse-wasm` the printers.
- **Source-level openness**: anyone can publish a same-shaped `my_org/tsv_html_parse` crate and any downstream _Rust_ consumer can `use` it without central buy-in. Published CLI/WASM binaries still hardcode the language list (`lang_bindings!` macro), by design.

Cross-language coupling exists only where languages integrate — `tsv_svelte` depends on `tsv_ts` (for `Expression`) and `tsv_css` (for `StyleSheet`). Don't invert this: no central public-AST crate, no dyn `Language` trait, no workspace-level language registry. ./docs/architecture.md#closed-scope-open-convention.

### Strictness: Module Strict, Script by Directive

**Strictness is a property of the source text, read as the spec defines it.** Module code is always strict (ecma262 sec-strict-mode-code); Script code is strict **iff** its directive prologue holds a `"use strict"`, as is any function body, class, or nested scope that inherits or declares it. Every **parse** entry point but the explicit `Goal::Script` parses a module, so the everyday answer is *strict* (a Svelte `<script>`, a `.ts` file). The format entry points that name no goal (`format_str`, `--content` with no `--source-type`) read an unset goal as a *fallback* — Module first, Script only on failure — so they reach a sloppy script too (below).

**Three rules move with strictness** — all disallowances strict code states over a production the sloppy grammar admits:

- the **leading-zero numeric literal** — `LegacyOctalIntegerLiteral` (`010`, base 8) and `NonOctalDecimalIntegerLiteral` (`08`) — lexed in every mode, rejected where the token becomes a node, once the enclosing code's strictness is settled;
- the **legacy string escape** — `LegacyOctalEscapeSequence` (`"\7"`, `"\101"`, `"\0"` followed by a decimal digit) and `NonOctalDecimalEscapeSequence` (`"\8"`, `"\9"`) — decoded in every mode, rejected at the same seam (where a string-literal token becomes a node). A bare `"\0"` is the NUL escape, legal everywhere. The rule reaches backwards too: a `"use strict"` directive re-grades the prologue literals ahead of it, so `function f() { "\7"; "use strict"; }` is a syntax error (ecma262 sec-literals-string-literals);
- the **`with` statement**, rejected at its keyword. `with` stays a `ReservedWord` in every mode (as an identifier, `with (a);` would read as a call), so only the STATEMENT moves, never the name channel.

The **untagged template**'s `NotEscapeSequence` rule is mode-independent and not one of these; tsv defers it.

**Annex B is out.** The web-compatibility grammar (HTML-like comments, labelled function declarations, `if (a) function f(){}` hoisting, `for (var x = 1 in o)`) is "normative but optional if the ECMAScript host is not a web browser" (ecma262 sec-web-compat); tsv, a formatter and parser rather than a browser host, takes that carve-out at both goals. One deferral: `if (a) function f(){}` parses because single-statement body positions don't enforce "a declaration is not a `Statement`" at all (`if (a) const x = 1;` parses too; a **labelled** item is the one position that enforces it) — the deferred-early-error stance below, not an Annex B relaxation; listed in [docs/checklist_typescript.md](docs/checklist_typescript.md) §Strictness (its "Early errors that still parse" list).

This is one instance of a broader stance: **the parser is deliberately permissive and defers static-semantic early-errors** (the above, plus the TypeScript ambient-context rules — a `declare` member body, initializer, decorator, etc.) to the diagnostics layer, so the formatter keeps formatting everything well-formed. The **correctness oracle for what's actually an error is tsc**, not acorn-typescript (matched only for AST *shape*), but where ECMAScript defines the construct the **spec's own layering outranks tsc's in both directions** — a production (grammar parameters included) rejects, a Static Semantics early error defers — so `for ((q = a in b) => 1; ;)` parses (a parameter default is an `Initializer[+In]`) though tsc's parser reports `',' expected`. Prettier is the formatting reference only, never by itself an accept/reject signal. A few rejects stand on a floor (representability, faithful reprint) or on a line the sibling positions already draw (`<out out T>`); rows on the wrong side are known gaps, not exceptions (`get`/`set constructor` still rejects inline; a top-level `return` still parses). Full rule: [docs/conformance_tsc.md §The reject-vs-defer line](docs/conformance_tsc.md#the-reject-vs-defer-line); see also [crates/tsv_ts/CLAUDE.md §Architecture Position ("Sources of truth")](crates/tsv_ts/CLAUDE.md#architecture-position) and [docs/conformance_svelte.md §TypeScript Corrections](docs/conformance_svelte.md#typescript-corrections).

**Strictness and the *goal* axis are orthogonal, with one coupling: Module ⟹ strict.** The goal is a parse-time input (which grammar symbol the parse starts from) gating four constructs of its own; strictness is what the source text says. A parse runs against `tsv_ts::Goal::{Module, Script}` (`parse_with_goal`, CLI `--source-type script|module`), **defaulting to `Module`** (correct for Svelte `<script>` and ~all real TS; Svelte hard-wires it). At `Script` goal: `await` is an ordinary identifier (`[~Await]`, tracked via the parser's `in_await` flag, save/restored at every function-like scope), and top-level `import`/`export` declarations, `import.meta` and a top-level `for await` are syntax errors. A TypeScript namespace or module body keeps its `import`/`export` at either goal (tsc decides module-ness from top-level statements alone); dynamic `import(...)` stays valid, as does a TypeScript **import-equals** (`import x = A.B` / `import x = require('y')` is neither an `ImportDeclaration` nor a `ModuleItem` — why the gate fires on the shape, not the `import` keyword). `sourceType` follows the goal.

**`format` alone reads an *unset* goal as a fallback**: `tsv_ts::parse_with_goal_or_fallback` parses at `Module` and retries at `Script` only on failure — which is what reaches a legacy sloppy script through `tsv format <path>` and an editor's bare `format_typescript(source)`. When both fail: a Script retry that died on a **goal gate** (a top-level `import`/`export`, `import.meta` or top-level `for await`, marked at their sites by `ParseError::goal_gate`, or the operand a module reads after a top-level `await`, marked at the parse's exit) proves the file a module, and the Module error is reported wherever it sits; otherwise the error that reached **further** into the source wins, the Module one on a tie — a broken sloppy script's own typo rather than the `with` the retry admits (`tests/format_fallback_error_attribution.rs`). A path whose **extension** settles the goal names it instead: `.mjs`/`.mts` are ES modules whatever any config says, so there's no legacy script to retry for (`tsv_ts::Goal::from_extension`, applied by both `tsv` bins; it can only reject a module-invalid file, never change an output). `parse` keeps the Module default at every surface — the wire's `Program.sourceType` is a claim one settled grammar must produce. See [docs/conformance_test262.md §Module Strict, Script by Directive](docs/conformance_test262.md) and [docs/cli.md §Multi-File Formatting](docs/cli.md#multi-file-formatting).

### Line Terminators: parse preserves, format folds

**`parse` never rewrites its input** — its byte offsets are a drop-in contract with acorn / Svelte / `parseCss` over the author's bytes. **Every parse-then-format entry point folds `<CR>` / `<CR><LF>` to `<LF>` before parsing** (`tsv_lang::printing::normalize_carriage_returns` — each language crate's `format_str`, the CLI's `format_source`, each binding's format export, `canonicalize_js`), so output is LF-only even in regions copied verbatim. The fold's single pass also takes the folded document's line verdict (`FoldedSource`), handed by each crate's `format_folded_in` to its printer, so a document that folds is walked once. Folding ahead of the parse is the only place that answers it once: several printer sites split lines on `'\n'` alone, and folding the finished string leaves them disagreeing with the output — the same document then formats two ways on two passes. `<LS>` / `<PS>` are deliberately NOT folded. The one `<CR>` whose fold would change meaning — a lone one with comment text after it inside a Svelte in-tag `//` comment, which Svelte ends at `\n` alone — is **refused** (`tsv_svelte::parse_folded`, a positionless `ParseError::refusal`), after the author's bytes are parsed first so an unparseable document reports `parse`'s own error.

**A leading byte-order mark is the one input `parse` reads two ways, because the oracles do.** No parser rewrites the source — every lexer skips a BOM at byte 0 and spans stay file-true — but the *emitted* position follows each wire's canonical parser (`tsv_lang::LeadingBom`, named at every map constructor): Svelte's `parse` and `parseCss` strip the BOM first (`remove_bom`), so the Svelte and CSS writers build their map `Elided` and every offset indexes the BOM-less string (one UTF-16 unit below the file's, a line-1 column one lower, acorn islands included — Svelte hands acorn the stripped string); acorn counts it as whitespace, so the TypeScript writer builds `Counted` and keeps file coordinates. `locations.js` makes the same split. Pinned by the three `bom_prettier_divergence` fixtures' `expected_prettier_variant_bom.json` variant pins (no `input.*` can carry a BOM with nothing load-bearing behind it: the format side strips it, so it is never its own fixed point; a BOM ahead of a content U+FEFF is written back, so the `leading_zwnbsp_prettier_divergence` inputs carry one — ./docs/conformance_prettier.md#whitespace-bom-handling).

**Counting lines is separate, answered once: `loc` has one definition.** Every object on the `loc` wire with numeric `start`/`end` — all three languages, `type`-less objects included — gets `loc` right after `end`: the line (1-based) and column (0-based, UTF-16 code units) of its own emitted offsets, under one line rule per DOCUMENT — ECMAScript's terminators for TypeScript (acorn's), `\n` alone for a Svelte document (everything embedded in it included) and for CSS. A superset of Svelte's wire reproducing none of its `loc` quirks (each a named tolerance in the corpus comparison); the line table is built at write time only when `loc` is requested (`tsv_lang::WirePositions`); fixtures pin the span-only wire, and `tests/loc_definition.rs` grades the definition. Rationale: ./docs/architecture.md#line-terminators-parse-takes-the-authors-bytes-format-folds-first and ./docs/architecture.md#loc-lines-one-rule-per-document.

### Language-Level concerns (classification)

HTML element classification is split between `tsv_html` — pure functions over tag names (`is_block_element()`, `is_void_element()`, `preserves_whitespace()`, and the other whitespace rules) — and thin printer adapters (`tsv_svelte/src/printer/classification/`) that resolve symbols, call tsv_html, and traverse the AST, for reuse across all planned tools (formatter, linter, compiler, LSP).

### AST Architecture: Internal AST vs Wire JSON

Drop-in replacement for the canonical parsers' **public JSON AST** (acorn / acorn-typescript / Svelte / `parseCss`), NOT their internal implementation.

- **Internal AST**: clean, semantic representation (decoded strings, normalized values) — what every tool (formatter, linter, …) builds on.
- **Wire JSON**: the parse product. The per-language writers (`ast/convert/write*`) emit it **directly from the internal AST in a single walk**, applying each acorn/`parseCss`/Svelte quirk at emission time — never materializing a typed public-AST Rust layer. The wire shape *is* the contract, documented by the hand-maintained `crates/tsv_wasm/types/tsv_ast.d.ts`. No shipped crate reads the wire back (the CLI's `--pretty` re-indents the bytes); `tsv_debug::json` (the fixture gate, the audits) is the one reader, unbounded in depth.

Worked example + full design: ./docs/architecture.md §Two-AST Design.

**Key Rules**:

- Raw strings NEVER duplicated in the internal AST (extract via `source[span.range()]`)
- The internal AST is NEVER the wire output — the writer hand-emits the wire JSON; `serde_json` is used only for exact `f64` parity (the writer substrate in `tsv_lang`, whose hand string escaper is graded byte-for-byte against it) and, in `tsv_debug` alone, to read bytes back into a `Value` (fixture gate, audits, tests)

### Position Types: u32 vs usize

- **Span**: `u32` start/end (8 bytes total, 50% savings vs usize)
- **`Token`**: `u32` start/end — a 16-byte POD `{kind, start, end}`, size pinned by a `const` assert in tsv_ts and tsv_css (tsv_svelte's leaner `Token` pins at 12 B). On the parsers' hot path the lexers write it straight into the parser's token slot (`next_token_into`), since a by-value `Result<Token, ParseError>` returns through a stack slot the parser would reload and re-scatter; other TS and CSS callers take it by value (`next_token`); the Svelte lexer has no by-value form. The decoded value (escapes only) lives out-of-band on the lexer (the reused `Lexer::decode_scratch` buffer, borrowed via `decoded_str`)
- **Lexer/Parser positions**: `usize` (natural for `source[pos]` indexing); the lexer dispatches on raw bytes (`cur_byte`) and decodes a `char` only at non-ASCII branches
- **Convert at boundaries only**: `as u32` when creating Spans/`Token` fields, `as usize` when extracting; prefer `span.extract(source)` / `span.range()` over manual casts

### Comment Handling: Detached Model

Comments live **separately from AST nodes**, in a flat `Comment` array at the root (`Program.comments`, `CssStyleSheet.comments`, `Root.comments`) — a `Copy` POD of spans + flags whose text is recovered via `Comment::content(source)` — and the printer finds them by span position. Glued block comments are **owned** (`owned_by_node`): printed by the node they're bound to, not the enclosing gap. **Ownership is a fact about who PRINTS a comment, never about whether it EXISTS.** A lookup names one of three axes (`tsv_lang::comment`):

| axis | question | owned comments | who asks |
| --- | --- | --- | --- |
| **to emit** | "which comments must *I* print here?" | **skipped** | gap emitters (most sites) |
| **on page** | "does any comment OCCUPY THE PAGE here?" | **counted** | layout gates — break / expand / hug / paren / fast-path |
| **in source** | "what comment BYTES are physically here?" | **counted** | cursors — blank-line scans, offsets, `prev_end` |

`comments_to_emit_*` · `comments_on_page_*` / `has_*_on_page_*` · `comments_in_source_*` — every name states its axis; a printer body asks its `Printer::comments_*_between` / `has_*_between` wrappers. A **zero-comment fast gate** guarding a builder is an **on-page** question; a **blank-line scan** is an **in-source** one (`blank_scan_start` / `blank_scan_end`).

⚠️ **The five hazards** (all have bitten): (1) an owned comment nothing prints is DROPPED — a builder that reassembles a node or swaps in a frozen slice must claim on its own seam (`prepend_owned_leading_comment_at`, `build_frozen_node_doc`); (2) a gap's **to emit** reading can't see an owned comment — ask the gap **on page**, never the node, ahead of every shape-keyed layout arm; (3) a region the parser lifts out of its container is printed twice (`AttrGaps::claimed`; only a line comment exposes it); (4) an alternate-layout container builder that emits only children's docs DROPS every gap comment — route a commented container to its comment-aware twin, gated before the empty arm; (5) a blank scan crossing an owned comment **fabricates** a blank line — no gate sees it, only a prettier `compare`.

**⚠️ Read ./docs/comments.md before touching comment handling in any printer** — the ownership doctrine and its suppressions, the hazards in full with their guards, the merged view of byte-adjacent block comments, and the one emitter per question (leading / trailing / dangling runs, deferred runs, the element-comma and statement-gap seams, the delimiter line, the left-spine shell run) — never hand-roll a copy.

## Dependencies

### Rust Crates (minimal deps)

The shipped language/foundation crates' external deps (`tsv_cli` adds only `argh`; dev tooling adds `tokio`, `futures-util`, `serde` with `derive`, `similar` and `tempfile`; `tsv_wasm` adds `wasm-bindgen`/`js-sys`):

- `serde_json` — wire-JSON emission (exact `f64` formatting; the oracle the hand string escaper is tested against), reached through `tsv_lang`'s `json` feature (and directly by `tsv_ffi`, which serializes its `{"error": …}` payload with it); no shipped crate deserializes. The one reader, `tsv_debug::json` (fixture gate, audits, tests), enables `unbounded_depth` — the default 128-level recursion limit refused wires the parser emits fine. `serde` itself is a dev-tooling dep (`tsv_debug`'s `derive`); the language crates see it only transitively
- `smallvec` — stack-allocated vectors (printers + `tsv_check`)
- `thiserror` — error type derivation
- `phf` — compile-time perfect hash maps (`tsv_html`'s entity table)
- `unicode-ident` / `unicode-segmentation` / `unicode-width` — XID identifiers, grapheme clustering, display width (CJK, zero-width)
- `bumpalo` — bump arena for the internal AST (and, via `tsv_arena`, the bindings' per-thread `reset()` reuse; `tsv_check`'s caller-owned arenas follow the same contract)
- `talc` — WASM global allocator (`tsv_wasm`, wasm32-only target dep): pure-Rust `no_std`, replacing dlmalloc; the `WasmGrowAndExtend` source keeps the warm instance's linear-memory high-water at dlmalloc parity. Pulls `lock_api` + `allocator-api2` (+ `scopeguard`) into the wasm32 graph only
- `napi` / `napi-derive` / `napi-build` — N-API bindings for `tsv_napi` (tsv-scoped carve-out)

## Canonical References

**Implementations** (versions pinned in `crates/tsv_debug/src/deno/sidecar.ts`):

- Prettier (`../prettier/`) — formatting reference; read source for layout logic
- Svelte compiler (`../svelte/`) — parsing reference

**IMPORTANT**: Read `../prettier/` source instead of searching the web for formatting behavior. Key files: `src/language-js/print/assignment.js` (assignment layout), `src/language-js/print/call-arguments.js` (call arg expansion), `src/language-js/print/member-chain.js` (chain formatting), `src/language-js/print/binaryish.js` (binary operators).

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

- **Leave `// TODO:` comments** for known future work or code smells
- **Suppress a lint with `#[expect]`, never `#[allow]`** — `expect` warns (so, under the gate's `-D warnings`, FAILS) once the lint stops firing, so a suppression can't outlive its cause; a stale `#[allow]` keeps suppressing the lint for code added under it later (how a stale `struct_excessive_bools` silently swallows the next bool). Exception: a lint `expect` can't see fulfilled — emitted from inside a function body, or behind a proc-macro expansion (`#[napi]`) — where the expectation reads as dead however placed. Those keep `#[allow]` **and** a comment saying why `allow`, not `expect`.

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
