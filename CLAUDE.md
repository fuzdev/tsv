# tsv

> precise language tools for TypeScript/JS, CSS, and Svelte in Rust

High-performance Rust parser, a drop-in replacement for Svelte's modern parser (acorn + acorn-typescript), paired with a formatter that took Prettier as its initial guide and still tracks it for the common case — with deliberate, cataloged divergences where tsv's own judgment is more defensible.

**Non-configurable by design** (like `gofmt` and Black) — no config files, CLI flags, or runtime options, ever. The fixed settings and the two carve-outs (file *scope*, the parse goal) are in [Configuration](#configuration).

## Releases

Version bumps, publishing, and **`CHANGELOG.md`** are user-owned. Agents make the source/doc/fixture edits and never touch `CHANGELOG.md` (including `## Unreleased` and its `<!-- bump: … -->` marker); the user stamps it at release time.

## Priorities

1. **Correctness**: Match Svelte's parser exactly — a drop-in replacement on the default span-only wire (every field but `loc` and `name_loc`); `loc` is opt-in with one definition (acorn-exact for TypeScript, Svelte's `loc` quirks not reproduced — ./docs/architecture.md#loc-lines-one-rule-per-document). The formatter tracks Prettier for the common case, diverges deliberately and catalogs it where more defensible (spec, print width, comment position, its own taste), and fixes numerous Prettier bugs. Fixtures are the source of truth — when tests fail, fix the code; when tsv diverges on purpose, the fixture records it.
2. **Performance**: Pure Rust. Dev tools reach the JS oracles through a long-running Node sidecar to minimize process overhead.

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
3. **See it fail** — `npm run fixtures:validate -- <pattern>` shows the failing diff.
4. **⚠️ APPROVAL GATE — STOP HERE.** Show the failing diff to the user and wait for explicit
   confirmation ("lgtm", "proceed", or feedback) before writing any implementation code.
   **If feedback reworks the fixture (naming, structure, cases), redo steps 1-3 and return here —
   the gate resets on every rework.** The gate is for interactive sessions: an arc, or a run the
   user explicitly made autonomous, proceeds past it and shows the failing diff in its report instead.
5. **Implement the fix.**
6. **Validate** — `npm run fixtures:validate -- <pattern>` passes.

**`long` fixtures**: include BOTH a 100-char case (stays inline) and a 101-char case (breaks), at the exact boundary with the minimum content that triggers it. Iterate `fixture_init --force` and read the widths from its output — never estimate manually.

**Never write code before creating the fixture** — it defines what "correct" means. **Failing fixtures are expected** — never delete one to make tests pass; it is a known bug waiting to be fixed.

## Values

- **Spec-first**: Read specs and canonical implementations before implementing. Experiment to verify, not to design.
- **Refactor early**: Fix outdated patterns immediately. Leave no legacy.
- **One sprint at a time**: Implement incrementally, keep tests passing.
- **No backwards compatibility**: Pre-stable — delete old code, migrate fully, don't shim. No new deps without explicit approval.

## Commands

Tasks live in `package.json` (`npm run <name> -- <flags>`) and, for the few whose body runs Deno (the bench's Deno row, `typecheck:js:deno`), `deno.json` (`deno task <name> <flags>`). Each task's scope and rationale: ./docs/tasks.md, keyed by name (a `deno.json` task's sits beside it as its `//<name>` key) — read a task's section before changing it.

**Prerequisites**: Rust, Node (≥ 24.14) and npm cover `check`, the release, and conformance. `npm ci` installs the root `node_modules` (`.npmrc` turns install scripts off); `npm ci --prefix crates/tsv_debug/sidecar` installs the fixture oracle `tsv_debug`'s sidecar runs (its own `package.json`, lock, and `.npmrc` copy) — `cargo test` needs it. Deno is the bench's alone: `check` runs no Deno task and passes without it, and no CI job installs it. The bench also needs `cargo install wasm-pack` + `npm run bench:install` once, and Bun for its Bun leg. `npm run doctor` checks the whole setup in one pass.

### Build & Development

```bash
npm run build              # workspace dev build
npm run dev                # watch: check + test on change (requires `cargo install cargo-watch`)
npm run build:release      # optimized build minus the binding crates (each builds alone so tsv_cli/tsv_debug features don't unify into them)
npm run build:all          # release + ffi + build:packages + build:napi:packages (everything)
npm run build:packages     # the 3 publishable npm packages (format, parse, all); single source of truth for CI + publish.ts
npm run build:bench        # everything `bench`/`smoke` measure and every bench leg builds (build:bench:pgo + the plain size builds + the wasm variants)
npm run build:bench:pgo    # the PROFILE-GUIDED FFI library + N-API addon every native bench row loads, stamped in .pgo/bench/
#                            (a stamp-hit takes seconds, a miss minutes)
npm run install-cli        # build the release CLI and install it to ~/.local/bin/tsv (the local daily driver)
npm run clean              # clean build artifacts
cargo check --workspace    # fast check (no codegen)
cargo build --workspace [--release]   # or -p tsv_cli / -p tsv_debug
```

Binding builds (`build:ffi*`, `build:wasm:*`, `build:napi*`, `build:npm:*`): [JS Bindings](#js-bindings).

### CLI Usage - Parse, Format & Strip

Full reference: ./docs/cli.md. Parser auto-detected from the extension (the JS/TS family `.ts`/`.mts`/`.cts`/`.js`/`.mjs`/`.cjs` → TypeScript, JSX/TSX out of scope; `.svelte`; `.css`); `--content`/`--stdin` require `--parser svelte|typescript|css`. For TypeScript, `--source-type script|module` names the parse goal (for `format`, `--content`/`--stdin` only; `parse` is module-only by default); a `format` **path** parses as a **module, retried as a script** on failure, except `.mjs`/`.mts` ([Strictness](#strictness-module-strict-script-by-directive)).

- `format` writes paths **in place** (only when output differs) and prints changed paths; `--content`/`--stdin` print to stdout. Directories recurse with gitignore-aware, reproducible discovery ([Configuration](#configuration)); a named file must have a supported extension (else an argument error — for `parse <file>` too, unless `--parser` names the grammar). `--list` prints the in-scope files without formatting (path mode only); `--jobs N` overrides the parallel worker count. Exit: 0 clean, 1 would-change (`--check`, also with `--content`/`--stdin`), 2 errors — missing path args fail upfront, per-file and traversal errors report and continue.
- `parse` exits 0 parsed, 1 when the input does not parse (a syntax error, a depth refusal, the size cap — the failures `format` and `strip` report as 2), 2 a usage or read error.
- `strip` is the type strip (./docs/strip.md) on ONE TypeScript input — a `.ts`/`.mts`/`.cts` path (a JavaScript path is refused, not read as TypeScript), `--content` or `--stdin`, no `--parser`, the goal as `format` reads it — printed to stdout byte for byte (`--check` writes nothing to stdout). Exit: 0 stripped, 1 refused (TypeScript that type blanking cannot erase), 2 a parse or usage error.
- A malformed command line (an unknown flag, a missing value, an extra positional) exits 2 for every command.

```bash
cargo run -p tsv_cli parse file.ts                                       # compact JSON (--pretty: formatted)
cargo run -p tsv_cli parse file.ts --locations                           # add per-node loc (the default wire is span-only)
cargo run -p tsv_cli parse --content '<div>x</div>' --parser svelte      # parse string (preferred for agents over --stdin)
cargo run -p tsv_cli format file.svelte src/lib                          # format files/dirs in place
cargo run -p tsv_cli format --check src/lib                              # list would-change files, exit 1 (CI)
cargo run -p tsv_cli format --list src/lib                               # list in-scope files (no formatting)
cargo run -p tsv_cli format --content '<div>x</div>' --parser svelte     # format string to stdout
cargo run -p tsv_cli strip file.ts                                       # TypeScript to JS on stdout, positions kept
cargo run -p tsv_cli strip --check --content 'enum E {}'                 # exit 1 on a refusal, 2 on a parse error
```

### Testing & Code Quality

"In `check`" = gates in `npm run check`, the full committed-tree gate (its composition: ./benches/js/CLAUDE.md §Gate map). A task that needs `npm run bench:install` (the bench's node_modules — CI installs none) can't gate there. What each task covers and why: ./docs/tasks.md §`<name>`.

```bash
npm run check            # full committed-tree gate: fmt, audits, typecheck, tests, clippy; runs no Deno task
npm run doctor           # one-pass setup check (runtimes, pins + checkout alignment, sidecar install, node_modules freshness,
#                          oracle checkouts, corpus, build artifacts). Exit 1 only on MISLEADING state (pin drift, skew,
#                          stale deps) or a broken core; absences warn (--strict promotes them)
npm run typecheck        # cargo check
npm run typecheck:features # each binding crate under each single feature + the language crates without `locations` —
#                          the per-package builds `cargo check --workspace` and clippy's `--all-features` both miss; in `check`
npm run typecheck:scripts # tsc over scripts/'s Node roots, the PGO pipeline, the package suites, the sidecar; in `check`
npm run typecheck:bench-core # tsc over the package-free bench modules nothing else in `check` typechecks; in `check`
npm run typecheck:js     # tsc + deno check over the bench harness + scripts/; NOT in `check` (bench node_modules)
npm run typecheck:packages # the STAGED npm packages' .d.ts + README blocks as a consumer compiles them; NOT in `check`
#                          (publish Step 6; docs/audits.md)
npm run test             # cargo test
npm run test:node        # vitest under Node over the `_test.ts` suites beside their modules in scripts/ and the bench core
#                          (the list is tsconfig.vitest.json; a new suite is added by name); in `check`
npm run test:node:canonical # the same over the suites that load the bench install; NOT in `check`
npm run test:depth       # tests/depth_limit.rs optimized: every shape the depth limits count, at its limit, survives parse +
#                          both wires + format + imports + strip on the stack contract's thread, and one deeper is refused; in `check`
npm run test:depth:napi  # the same in the shipped binaries' optimization over the FULL sweep; NOT in `check` (publish Step 3, CI `artifacts`)
npm run test:depth:wasm  # the same shapes through the staged `all` WASM package; NOT in `check` (publish Step 6, CI `artifacts`)
npm run test:audits      # cargo test -p tsv_lang --features _internal_audits — the audit seams' own tests; in `check`
npm run lint             # cargo clippy
npm run entities:update  # regenerate tsv_html's committed entity table from its entities.json (a unit test fails on a stale table)
cargo fmt                # Rust
tsv format .             # the repo's own TS/JS — tsv formats itself (`--check` in the gate)

cargo test --workspace strict_reserved_words_are_binding_names  # one test by name
cargo test --workspace --test fixtures_tests           # fixture validation tests
cargo test --workspace --test cli_tests                # CLI integration tests
cargo test --workspace --test strip_fixtures_tests     # type-strip fixtures (tests/fixtures_strip)
```

`cargo fmt` (Rust) and `tsv format` (TS/JS) are the repo's ONLY autoformatters and partition it; markdown and JSON are hand-maintained. **Never run `deno fmt` or `prettier` on the repo** — tsv ships no config for them, so they'd churn every file (the prettier oracles pass options inline). `tsv format` on a directory is safe: the root `.formatignore` prunes tests/fixtures/, tests/fixtures_compile/, tests/fixtures_strip/ and tests/fixtures_imports/ (deliberately not format fixed points), and a file named under them is skipped too.

### Fixtures (Rust + Node-based)

All `fixtures:*` tasks accept positional patterns (multiple = OR); `fixtures:validate` and `fixtures:update:parsed` also take `--list`, and `fixtures:validate` `--prettier-only` (skips our parser/formatter — use it during fixture design).

```bash
npm run fixtures:list                # list all fixtures
npm run fixtures:init -- <dir>       # create/reinit a fixture (= `tsv_debug fixture_init`, under Debug Tooling)
npm run fixtures:validate            # validate
npm run fixtures:update              # regenerate expected.json + output_prettier.svelte from the sources of truth — ONLY after
#                                      creating a fixture or when upstream (Svelte/prettier) changes, never to "fix" a failing test
npm run fixtures:update:parsed       # expected.json only (when the parser changes)
npm run fixtures:update:formatted    # output_prettier.svelte only
npm run fixtures:audit               # audit _prettier_divergence fixtures (diagnostic; --all for every fixture)
npm run fixtures:ts-audit            # which input.ts fixtures need .ts vs could be .svelte (= `ts_fixture_audit`)
npm run compile:fixtures:init        # create/reinit a compile fixture (oracle-compiles + canonicalizes; tests/fixtures_compile)
npm run compile:fixtures:validate    # compile fixtures: oracle freshness + expected idempotence + ours parity. Preflights
#                                      `npm run conformance` — the ONLY place oracle freshness is graded
npm run strip:fixtures:validate      # tsv_ts::strip vs tests/fixtures_strip (= `cargo test --test strip_fixtures_tests`); known
#                                      failures pinned in a ratchet (`:update` re-pins). tests/fixtures_strip/README.md
npm run strip:fixtures:update        # regenerate expectations from ts-blank-space, cross-checked against Node's
#                                      module.stripTypeScriptTypes (`strip:fixtures:oracle` re-checks). bench:install; NOT in check
npm run imports:fixtures:validate    # import records vs tests/fixtures_imports, oracle freshness included. Sidecar, NOT in check —
#                                      `cargo test --test imports_fixtures_tests` grades tsv against the committed files
npm run imports:fixtures:update      # regenerate every expected.json from the canonical parsers' trees. Sidecar; NOT in check
```

**Standing audit gates** — ./docs/audits.md is the full reference: its §Overview table maps every audit to its task, what it catches and where it gates, and each has a section on what it proves, its blind spots and its flags. Read that section before running or modifying an audit. RATCHET audits (® below) grade against a committed known-bug snapshot (`*_known.txt`); each has an `:update` task that re-pins after a fix and refuses a narrowed run.

In `npm run check`, each its own `npm run <name>` (pure Rust over tests/fixtures unless noted):

- **comments**: `swallow:audit` (a `//` swallowing following content), `comments:audit` (print-once ledger: DROPPED / DOUBLE-PRINTED), `census:audit` ® (raw trivia multisets — the parse-time drops the ledger can't see), `binding:audit` (comment↔token re-binding), `gaps:audit` ® (gap injection; ./docs/gap_audit.md), `ignore:audit` ® (`prettier-ignore` honoring)
- **blank lines**: `blanks:audit` ® (injection; ./docs/blank_audit.md), `fabrication:audit` ® (a blank the formatter invents)
- **layout**: `width:audit` ® (a new KIND of over-width line, the only gate measuring a column — ⚠️ NOT a debt list, sanctioned overruns are real), `razor:audit` (the only instrument varying WIDTH), `fanout:audit` ® (doc-build fanout, counted, no wall-clock), `authoring:audit` + `paren:audit` (authoring independence: boundary whitespace, redundant parens)
- **reparse / robustness**: `roundtrip:audit` (output reparses with its node population conserved), `roundtrip:audit:prettier` (the same over the prettier suites — `check`'s ONLY non-format-stable corpus; warn-skips without ../prettier), `fuzz:audit` + `fuzz:audit:debug` (seeded mutational fuzzer; also the loc wire, strip, and import records), `canonicalize:audit`
- **docs / repo integrity**: `conformance:audit` (divergences cataloged, every Markdown link resolves, READMEs back-link), `conformance:audit:compiler`, `variants:audit` (`_compact`/`_spaces` direction), `format:audit` (`tsv format --check .`), `pins:audit` (canonical-oracle pin agreement), `docs:audit` + `docs:audit:docsrs` (rustdoc links), `deps:audit` (no `serde_json` in a published graph), `package:audit` (crate tarballs), `scan:audit` (no new raw substring scans), `discovery:audit` (`--list` vs the ../corpora snapshot; warn-skips without it), `check:loc` (the loc wire vs `locations.js`), `check:ast-types` ([Publishing](#publishing))

NOT in `check` (sidecar, built packages, or real-code corpora): `audit:corpus` (the content-loss/robustness bundle over REAL code; publish Step 3c), `idempotency:sweep` (F1 over the real-code corpus), `render:audit -- <paths>` (render-equivalence over real Svelte), `strip:corpus` + `imports:corpus` (vs ts-blank-space / oxc-parser's module record; the last three are `conformance` legs), `engines:audit` (wasm32 vs native bytes; CI's `artifacts` job), `pins:audit:checkouts` (a present oracle checkout matches its pin; a `conformance` preflight), `pins:lock` (regenerate the sidecar lock after a pin bump; network, not a gate), `wire:audit` (wire injection vs the canonical parser; `wire:audit:terminators` is ⚠️ RED BY DESIGN), and the compiler harnesses `compile:corpus:compare`, `compile:validation` ®, `compile:fuzz` (RED by design) — ./docs/compile_tooling.md, ./docs/compile_validation_ratchet.md.

### JS Bindings

Three binding crates:

- `tsv_ffi` (C ABI) — any FFI (Deno, Python, etc.); `libtsv_ffi.so` / `.dylib` / `.dll`
- `tsv_wasm` (wasm-bindgen) — browser, Deno, Node; format / parse / all variants via cargo features (the three npm packages under [Publishing](#publishing))
- `tsv_napi` (napi-rs) — Node.js / Bun native addon (`libtsv_napi.*`, loaded via `process.dlopen`), built with the `napi` profile (`release` + `panic = "unwind"` → `target/napi/`; every export catches a panic, so it throws a JS `Error` whose message is `panic: <payload>`, the C FFI's text, instead of aborting the host). Its npm surface (`@fuzdev/tsv`, wasm-API-parity by contract) is in [Publishing](#publishing); tested per OS by `test:napi:npm`. See ./crates/tsv_napi/CLAUDE.md.

`tsv_wasm` and `tsv_napi` carry the `strip` and `imports` features in their default sets; `tsv_ffi` has neither.

```bash
npm run build:ffi                    # C FFI, full → target/release/libtsv_ffi.so
npm run build:ffi:format             # C FFI, format-only (size only) → target/ffi-format/release/ (also :parse; :all builds all three)
npm run build:napi                   # N-API addon (napi profile) → target/napi/
npm run build:napi:corpus            # N-API addon (corpus profile: unwind, no LTO) → target/corpus/, for tools that grade under Node
npm run build:napi:packages          # + staged npm packages (loader + host platform pkg) → crates/tsv_napi/pkg/
npm run build:pgo                    # the PROFILE-GUIDED pair the platform packages ship (CLI + addon), trained, gated and
#                                      staged where build:napi:packages stages the plain pair (scripts/pgo/pgo.ts; .pgo/)
npm run build:wasm:deno              # deno WASM (requires wasm-pack), format-only → pkg/format/deno/
npm run build:wasm:parse:deno        # deno WASM, parse-only → pkg/parse/deno/
npm run build:wasm:all:deno          # deno WASM, full (benches) → pkg/all/deno/
npm run build:npm:format             # publishable npm package → pkg/format/npm/ (also :parse; :all adds the tsv bin)
```

The cargo-built `build:*` tasks also STAMP their artifact for the `:run` freshness guards (docs/tasks.md §`build:stamped`); a bare `cargo build -p tsv_ffi --release` or `wasm-pack build crates/tsv_wasm --target deno --release --out-dir pkg/all/deno` does not.

### Publishing

Version bumps and every publish are the user's ([Releases](#releases)). npm is the JS package surface (each tag's GitHub Release carries only notes + the native CLI binaries npm already ships), and crates.io the Rust one ([below](#cratesio)). All published packages and crates move together at `Cargo.toml` `[workspace.package] version` (read directly by `wasm-pack`); the root `package.json` is private and carries no version; no changesets.

**The WASM packages** — three from `tsv_wasm` via its `format` + `parse` + `strip` + `imports` cargo features (default = all four), each with its own output directory, released by `npm run release` (`scripts/publish.ts`, dry-run by default — steps, gates, CHANGELOG stamping and resume: ./docs/tasks.md §`release`):

- `@fuzdev/tsv-format-wasm` — format only (`--no-default-features --features format`)
- `@fuzdev/tsv-parse-wasm` — parse only; bundles the hand-maintained `tsv_ast.d.ts` (`crates/tsv_wasm/types/`) + the pure-JS line/column helper behind `{locations: true}` (`crates/tsv_wasm/npm/locations.js`; also exported alone as the `./locations` subpath)
- `@fuzdev/tsv-wasm` — the default features: both of the above plus `strip` (`strip_typescript` — ./docs/strip.md) and `imports` (`parse_imports_typescript` / `parse_imports_svelte` — ./docs/imports.md), neither of which a subset carries; ships the `tsv` bin (`crates/tsv_wasm/npm/cli.js`, mirroring `tsv_cli`'s flags and exit codes — ./docs/cli.md §Binary Structure)

Every entry publishes through one hand-written facade shared with the native `@fuzdev/tsv` (`crates/tsv_wasm/npm/api*.js`; package shape, option bags and the `{ok, …}` result: ./crates/tsv_wasm/CLAUDE.md §The npm Facade). The export list is extracted from the generated JS, so new `lang_bindings!` languages flow through automatically.

**The N-API set** — tsv's native distribution under the one bare name: the `@fuzdev/tsv` loader + six `@fuzdev/tsv-<triple>` platform packages, each shipping **two profile-guided binaries**: the addon (`tsv_napi.node`, `napi` profile) and the real `tsv_cli` binary (`release` profile), which the loader's `tsv` bin execs — `npx tsv` IS the native CLI — falling back to the shared `cli.js` mirror. Staged plain by `npm run build:napi:packages`, profile-guided by `npm run build:pgo`; publishes **only** through the tag-triggered `.github/workflows/release.yml`, **never** `scripts/publish.ts`. Detail: ./crates/tsv_napi/CLAUDE.md §The npm packages and §Release.

**Naming.** Every npm name tsv publishes is kebab-case (`@fuzdev/tsv`, `@fuzdev/tsv-wasm`, `@fuzdev/tsv-format-wasm`, `@fuzdev/tsv-parse-wasm`, `@fuzdev/tsv-<triple>`) — one spelling with the `tsv-<triple>` release assets and the `fuzdev.tsv-format` VS Code extension (vsce forbids `_`), and the `-wasm` convention of tsv's Rust-tooling peers; the rule for every `@fuzdev` WASM or native-delivery package (`@fuzdev/blake3-wasm` too). Rust crates (`tsv_wasm`, `tsv_napi`) and the rest of the `@fuzdev` libraries stay snake_case; crate-derived file names inside a package (`tsv_wasm.js`, `tsv_wasm_bg.wasm`, `tsv_napi.node`) are internal and follow the crate.

**Size bounds**: `scripts/validate_artifacts.ts` holds deliberately tight (~±8%) wasm size bounds — a legitimate binary size change fails the publish until the constants are updated, keeping size moves visible and intentional.

**TS type maintenance**: `crates/tsv_wasm/types/tsv_ast.d.ts` is hand-maintained — any change to the wire JSON a writer emits (`crates/tsv_*/src/ast/convert/write*`) must update it. `npm run check:ast-types` (in `check`) catches drift against the live writer's samples, every `type` discriminant the fixture corpus produces, and every gradable field slot in the committed `expected*.json`. Per-field checklist, the Svelte-built-node rule (a node Svelte constructs is not the acorn node of the same `type` — the tell is a missing `loc` in Svelte's own wire), and the comment-attachment rule: ./crates/tsv_wasm/CLAUDE.md §TS Type Maintenance.

```bash
npm run release                          # dry-run: validate everything, no mutation (`--wetrun` is the user's)
npm run publish:crates                   # dry run: registry read, semver-checks, cargo publish --dry-run (`--wetrun` is the user's)
npm run test:npm[:parse|:all]            # build the npm package + Node tests against it (:all adds CLI tests; `:run` skips the rebuild — freshness-guarded)
npm run test:napi:npm                    # stage the napi loader + host platform package + Node tests on the packaged shape (`:run` as above)
npm run test:bun -- [--require-bun] [format|parse|all|napi] # the parse-failure table over the staged packages under Bun; warn-skips without bun
npm run validate:artifacts               # wasm size bounds + each package's third-party NOTICE (fails if nothing is built or the build is STALE)
```

#### crates.io

The members with a `[package.metadata.docs.rs]` table — `tsv_lang`, `tsv_html`, `tsv_ignore`, `tsv_discover`, `tsv_ts`, `tsv_css`, `tsv_svelte`, `tsv_cli` (`scripts/docsrs_crates.ts`, the set every crate gate shares; each other member is `publish = false`) — publish to crates.io at the workspace version, siblings pinned `=x.y.z`. `npm run publish:crates` is **dry-run by default**; `--wetrun` is a maintainer's, by token, from a clean checkout of the `v<version>` tag — never CI's. The flow, the public-API gate (`cargo semver-checks` against each crate's published baseline), `cargo binstall tsv_cli`, and recovery (a broken release is yanked as a whole set and fixed forward): ./docs/tasks.md §`publish:crates`.

### Corpus Comparison

Compare formatting against Prettier, and parse output against the canonical parsers, on real code: the pinned `../corpora` snapshot (`fuzdev/corpora` — the author's repos, the sveltejs repos, third-party Svelte libraries and CSS frameworks, neutral TypeScript; one collection per upstream, placed by `benches/js/lib/corpus.ts` from the snapshot's manifest). Full runs enforce **pinned expected counts** over the `gates` view, exact and per corpus source and language, in generated files each gate's `--all --update` rewrites (`benches/js/lib/corpus_{format,parse}_pins.txt`), with SAFETY gates over every file; a snapshot refresh is a deliberate re-pin (./docs/gate_counts.md). Operator detail for everything below: ./benches/js/CLAUDE.md.

```bash
npm run corpus:compare:format -- ../some-project # one project, or --all for the gates corpus (../corpora + prettier suites)
# Options: --explain (patterns matched), --summary, --json (stats + safety/partial/unknown/error lists; logs → stderr)
npm run corpus:compare:parse -- --all  # deep-diff parse ASTs vs acorn-typescript/svelte/parseCss
# Options: --multibyte-only, --filter <lang>, --limit <n>, --json

# The three parser gates accept -v, --json, <subtree>; a missing checkout or an empty scan FAILS
npm run conformance:svelte-fixtures    # tsv's Svelte parser vs Svelte's own test suite (../svelte): verdict parity gates (an
#                                        over-rejection must be SANCTIONED or a tracked KNOWN_GAP); AST-shape diff is report-only
npm run conformance:ts-fixtures        # tsv's TS parser vs acorn-typescript's test suite (the adversarial TS edge cases)
npm run conformance:ts-repo            # tsv's TS parser vs ALL of ../typescript/tests/cases; oracle = tsc's .errors.txt baselines
#                                        + tsc's LIVE parser. Freshness-checked ledgers (sanctioned = kept, known = to fix); only an
#                                        UNTRACKED over-rejection fails. ./docs/conformance_tsc.md
npm run conformance                    # pre-release aggregate: pin- and oracle-freshness preflights (fixtures:validate +
#                                        compile:fixtures:validate — `check` grades tsv only against the COMMITTED files), the
#                                        svelte-styles harvest, strip:corpus + imports:corpus, then in one Node process the three
#                                        gates above + corpus:compare:parse/format --all, then render:audit (./docs/tasks.md §`conformance`)
npm run conformance:test262            # tsv's JS parser vs test262 POSITIVES (pure Rust, `test262 --gate`; exact POSITIVE_PASSED_PIN);
#                                        negatives (the deferred early-error frontier) are reported, not gated
npm run conformance:all                # the full drop-in gate = `conformance` + `conformance:test262` — what publish Step 3b
#                                        runs. CSS-WPT harvest stays manual

npm run divergence:audit           # audit divergence pattern coverage (--json)
npm run corpus:stats               # corpus/candidate-dir sizes + language + degenerate-case stats (diagnostic)
```

A `corpus:compare:format` SAFETY hit is self-verified in-run, so treat it as real; a caught **panic** hard-fails either corpus tool on every run. Corpus comparison builds with `--profile corpus` (optimized + `panic = "unwind"`, no LTO — panics in our code are caught and reported; also the single build world every `npm run check` audit shares). Benchmarks measure what ships: every native tsv row loads a **profile-guided** build (`npm run build:bench:pgo`), while `perf:ab` keeps plain builds — a regression instrument must not draw a training per side.

Divergence detection identifies the known differences documented in the `conformance_prettier*.md` family (safety checks, pattern detection, traceability). See ./docs/divergence_detector.md.

### Benchmarks

Harness operation — corpus views, the process model, env vars, harvests, known issues: ./benches/js/CLAUDE.md. Fairness caveats and the implementation catalog (every compared tool and how its row is built): ./docs/benchmarks.md. The shape:

- **Cross-runtime.** One harness runs under **Deno, Node, and Bun**, each emitting its own runtime-labeled report (`report.{deno,node,bun}.{json,md}`, never merged); `npm run bench:compose` folds them into the combined `report.{json,md}` (what tsv.fuz.dev consumes). The native row is **FFI** under Deno, **N-API** under Node/Bun, both profile-guided.
- **A process per row.** Every row is timed in a fresh process of its own, `BENCH_PASSES` times over, each pass taking a group's rows in a different order — rows timed back to back in one process moved each other's numbers, so nothing measured shares a process.
- **Perf vs conformance surfaces.** `bench:perf` measures throughput over a **real-world, third-party-only** corpus from the pinned `../corpora` snapshot (the author's own collections stay gated and swept but off the bench, held out as the training input for profile-guided native builds); every in-scope tool must fully process every file or the run fails. `bench:conformance` measures per-tool **parse coverage** over a **disjoint, fixtures-only** corpus — coverage-only and node-only by design. Coverage counts accepts and so rewards permissiveness; the inverted profiles are `npm run ts-repo:over-acceptance` and `npm run css:over-acceptance`.
- Results are committed under `benches/js/results/`; publish to tsv.fuz.dev with `npm run update-benchmarks` in ../tsv.fuz.dev.

```bash
npm run bench:install   # one-time: the harness's npm deps (re-run after a dep bump or a plain `npm install`)
deno task smoke         # fast sanity check that every formatter+parser produces output (also smoke:node / smoke:bun)
npm run bench           # full refresh = bench:perf + bench:conformance + a closing bench:compose (needs node AND bun)
npm run bench:perf      # perf surface: build the whole artifact set ONCE, then the three :run legs + compose
npm run bench:deno      # Deno only (also bench:node / bench:bun); each standalone leg builds the WHOLE artifact set
npm run bench:compose   # fold per-runtime reports → combined report.{json,md}
deno task bench:deno:run   # run without rebuilding (also :node:run / :bun:run; aborts on stale artifacts)
npm run bench:conformance  # bench:pins:suites + build:bench + the coverage run (`:run` skips harvest + rebuild)
npm run bench:harvest      # regenerate every cache = bench:pins:suites + bench:harvest:svelte-styles
npm run bench:pins:suites  # the conformance-view suite caches + the CSS reject pin (freshness-stamped; --force after harvest-logic changes)
npm run bench:harvest:svelte-styles # the PERF-view CSS cache (REQUIRES every snapshot collection)

# Env vars (any runtime): BENCH_LIMIT, BENCH_FILTER, BENCH_PASSES, BENCH_DURATION, BENCH_WARMUP,
# BENCH_MODE, BENCH_CORPUS, BENCH_STALE_OK, BENCH_FORCED_ASYNC — semantics + defaults in ./benches/js/CLAUDE.md
BENCH_FILTER=bits-ui BENCH_LIMIT=10 deno task bench:deno:run
```

### Performance Profiling

Methodology, results tracking and the `perf` recipe (the `profiling` cargo profile): ./docs/performance.md. All pure Rust, no sidecar:

```bash
cargo run --release -p tsv_debug -- profile ../corpora/collections/zzz/src/lib        # parse vs format timing (--iterations, --json, --flow-stats)
cargo run --release -p tsv_debug -- profile --bind ../corpora/collections/zzz/src     # parse vs lower+bind timing (TS-only) + peak RSS
cargo run --release -p tsv_debug -- profile --strip ../corpora/collections            # parse vs the type strip's walk + blank (.ts/.mts/.cts, no .d.ts)
cargo run --release -p tsv_debug -- json_profile ../corpora/collections/zzz/src/lib   # the bindings' parse path: parse vs span-only wire write (--locations: the loc emitter)
cargo run --release -p tsv_debug -- compile_profile tests/fixtures_compile            # Svelte compile vs the format wall
cargo run -p tsv_debug buffer_sizes ../corpora/collections/zzz/src                    # printer SmallVec sizing histograms
cargo run -p tsv_debug arena_stats ../corpora/collections/zzz/src/lib                 # DocArena node-population + memory audit (--reuse, --list-errors)
npm run ast-census -- ../corpora/collections/zzz/src   # per-node-kind population (--bytes joins the size board, --slots, --arity)
npm run type-sizes                 # the `size_of` board: every public AST type's width
npm run build-visit-census -- ../corpora/collections   # doc-build dispatches per language and dispatch family (a dispatch is not an edge read)
npm run ast-demand-census -- ../corpora/collections    # the parser's allocation demand per store-backed type, per source byte
npm run metrics                    # line counts by crate and phase
npm run perf:ab -- <base-ref> [<cand-ref>]  # parse/format A/B per language + the type strip: wall + instructions:u, exit 1 on a regression;
#                                             cand defaults to the working tree. NOT in check (two release builds)
npm run branch:report -- <base-ref> <cand-ref>  # one document comparing two refs: sizes, type widths, work counters; timing legs behind --timing;
#                                             same ref twice is the A/A null
```

The **density pair** (`ast-census` + `type-sizes`): a lever that narrows a type is worth `count x bytes saved`, and neither factor is guessable — `ast_census --bytes` multiplies them in one table.

## Configuration

**Non-configurable by design.** Formatting options are fixed — no config files, CLI flags, or runtime options, ever. Everything matches Prettier's defaults (e.g. tabWidth=2) except:

- `printWidth` (100) — wider than Prettier's 80
- `useTabs` (true), `singleQuote` (true)
- `trailingComma` ('none') — no trailing comma even when a list breaks; with useTabs + singleQuote this matches the Svelte project's own `.prettierrc`

**`--source-type` is a grammar input, not a style setting** — it names which grammar symbol the parse starts from (ecma262's `ParseScript` / `ParseModule` are two entry points, not two styles). Leaving it unset is not a *default style* either: an unnamed goal parses at Module and retries at Script only on failure, so every valid script formats unconfigured and no module-valid file's output moves (./docs/cli.md §`--source-type` is a grammar input, not a style setting).

**The one carve-out is file *scope*, not style.** Authoritative rules + edge cases (parent-directory rule, re-include idiom, unreadable ignore files, named symlinks, warnings): ./docs/cli.md §Multi-File Formatting. Core:

- Discovery is gitignore-aware with two regimes keyed on `.git`. **Inside a git repo** the **format root** (the scope boundary — derived from the argument, never the cwd) is the repo root, a hard stop for the upward walk; discovery honors `.gitignore`, then `.formatignore` (tsv's native file; its `!` can re-include a gitignore'd path), then `.prettierignore` (drop-in compat; the fallback in any directory with no sibling `.formatignore`), all hierarchical, plus the always-skipped safety nets (`.git`, `node_modules`, `.sl`, `.hg`, `.svn`, `.jj`). **Outside a repo** only `.formatignore` is read, from the filesystem root down (so `~/.formatignore` is global config for loose files).
- A `.gitignore` in scope turns the built-in heuristic (hidden dirs + `dist`/`build`/`target`) **off**.
- A **named** file or directory is bounded by the ignore files alone: one a rule excludes is skipped (with a warning, except a file a `.formatignore`/`.prettierignore` rule excludes); the safety nets and heuristic prune only what a walk discovers.
- The matcher is the `tsv_ignore` crate (`IgnoreStack`); the per-directory prune decision and the named-path gate are `tsv_discover` — shared with the JS CLI and the VS Code extension via WASM, and with native `@fuzdev/tsv` via N-API, so every surface agrees by construction.

**Measuring line widths**: `cargo run -p tsv_debug line_width <file>` — never `wc -c` (bytes, not visual chars; a tab is 1 byte, 2 visual chars). `compare` also shows widths on changed lines.

### Internal Configuration (Rust Library Only)

No runtime configuration. Print width / tab width / indent are compile-time `pub const`s in `tsv_lang::config` (`PRINT_WIDTH`, `TAB_WIDTH`, `INDENT`), read directly by the renderer, never threaded through signatures. Quote preference is hardcoded (single quotes) in `tsv_lang::printing` — the `optimal_string_quote` tie-break `format_string_literal` applies. Doc-builder unit tests exercise smaller widths via the internal `RenderConfig` seam (`doc::render_config`, `pub(crate)`), never at runtime.

One type carries per-input *state* (not configuration), threaded only where it varies: `tsv_lang::EmbedContext` — embedding state for nested formatting (CSS in `<style>`, Svelte template expressions). Its `mode: LayoutMode { Standalone, Embedded }` controls the expression-ROOT binary indent style (nested expressions format context-free), and `root_sequence_indents` is its sequence counterpart, set by the Svelte **block head** alone. A `build_*_doc` call reads only the build-time fields, and the renderer reads no width from the context at all: it measures from the column it is handed, and text a host writes after an embedded doc rides a trailing reserve node (`DocContext::reserving`). Every field and its one reader: ./crates/tsv_lang/CLAUDE.md.

TypeScript formatting is identical for standalone `.ts` and Svelte-embedded TS: one printer — standalone (`tsv_ts::format`) and embedded (`tsv_ts::__private::build_program_doc`) TS build the same doc.

## Project Structure

```
tsv/
├── crates/
│   ├── tsv_lang/    # Foundation (span, location, error, doc builder, printing utils)
│   ├── tsv_arena/   # Shared binding substrate (tsv_ffi, tsv_napi, tsv_wasm): per-thread reusable AST/doc arenas + the goal axis
│   ├── tsv_html/    # HTML spec data: named character references
│   ├── tsv_ignore/  # gitignore-aware matcher: hierarchical .gitignore + .formatignore/.prettierignore
│   ├── tsv_discover/# file-discovery policy (build-output heuristic + safety nets) over tsv_ignore
│   ├── tsv_ts/      # TypeScript: parse_to_json(), format(), strip(), parse_imports(), strip_with_imports()
│   ├── tsv_css/     # CSS: parse_to_json(), format()
│   ├── tsv_svelte/  # Svelte: parse_to_json(), format(), parse_imports()
│   ├── tsv_svelte_compile/ # Svelte→JS compiler (Svelte's compile() oracle) + JS canonicalizer; consumed by tsv_debug — no shipped artifact links it
│   ├── tsv_check/   # EXPERIMENTAL TypeScript binder + checker — may never ship (consumed only by tsv_debug)
│   ├── tsv_cli/     # Production CLI (binary: tsv) - pure Rust
│   ├── tsv_debug/   # Dev utilities (binary: tsv_debug) - uses a Node sidecar
│   ├── tsv_ffi/     # C FFI bindings (Deno's native path)
│   ├── tsv_wasm/    # WASM bindings (the 3 published npm packages; bundles types/tsv_ast.d.ts + npm/locations.js; npm/cli.js is the tsv bin)
│   └── tsv_napi/    # N-API bindings (Node/Bun native path; npm/ is the @fuzdev/tsv loader source)
├── scripts/         # Publish orchestrator + changelog grammar, GitHub Release notes/assets, npm package patcher, Node artifact + N-API tests, AST type drift check, the script-run gates (pins, discovery, `loc`, engine parity) + doctor
├── benches/js/      # Cross-runtime benchmark + conformance harness (Deno/Node/Bun)
├── tests/           # Integration tests (parser, formatter, CLI)
│   ├── fixtures/    # Test fixtures organized by language/feature
│   ├── fixtures_compile/ # Compiler fixtures (input.svelte + canonicalized oracle expected_server.js + expected.css) — separate tree so parser/formatter fixture counts stay unperturbed
│   ├── fixtures_strip/ # Type-strip fixtures for `tsv_ts::strip` (input.ts + oracle-generated expected.js or refusal.txt) — tests/fixtures_strip/README.md
│   └── fixtures_imports/ # Import-record fixtures for `parse_imports` (input.ts / input.svelte + oracle-generated expected.json) — tests/fixtures_imports/README.md
└── docs/            # Documentation (fixtures, cli, architecture, etc.)
```

**Crate pattern** (tsv_ts, tsv_css, tsv_svelte): `lib.rs` (public API: `parse_to_json()` / `parse_to_json_with(JsonOptions)`, `format()`, `Formatter` (`format()` over warm arenas, for many sources on one thread), and the re-exported `run_with_stack` / `STACK_SIZE`; the AST-taking `__private::parse()` / `__private::convert_ast_json_bytes()` beneath them), `ast/` (internal AST + the wire-JSON writer), `lexer/`, `parser/`, `printer/` (uses the tsv_lang doc builder), and escapes (tsv_ts's `lexer/escapes.rs`, tsv_css's `escapes.rs`; Svelte delegates to TS/CSS). `tsv_ts` and `tsv_css` also export embedding APIs for `tsv_svelte` (`parse_embedded` + embedded-formatting variants); `tsv_ts` additionally exports the `build_*_doc` functions. That plumbing, and each language crate's `parse_folded` / `format_folded_in` (the format path `tsv_arena`'s macros call), lives in a `#[doc(hidden)] pub mod __private` — reachable by the sibling crates, not public API.

### Conformance

**Comment position is preserved by default — but the rule is principled, not absolute.** A core tsv stance and the largest category of deliberate Prettier divergence: a comment's placement usually communicates what it refers to, so tsv keeps comments where the author wrote them. Prettier routinely relocates comments across syntactic boundaries, often **losing information** — two comments merging onto one line (the second `//` becoming text), or reordering. tsv treats such a boundary as semantic and holds the comment in place.

The line: **preserve when the position carries authorship signal, or when relocating would lose information** (the common case). But tsv **deliberately trails** a same-line line comment past a *pure separator* when that is **lossless and the position carries no signal** — e.g. between a list element and its comma (`A // c⏎, B` → `A, // c`): the comma is structure, the comment trails the element either way, and per-element line breaks keep multiple comments distinct, so tsv matches Prettier. That carve-out is a deliberate choice, **not** a gap to close. (Contrast the name→`=`/`:`/`?` binding cases, where two comments *would* collide on one trailing line — there tsv preserves + continuation-indents to stay lossless, diverging from Prettier's merge.)

The **opening-delimiter** rule is uniform across the printer: a `//` the author glued to an opening delimiter keeps that line at every one — call and container delimiters, paren shells and operand pairs, `return (` / `throw (`, and every statement header — and the author blank *below* the pulled comment **survives at all of them** (a blank *above* the comment sits against the delimiter and stays erased). **Two** emitters state it — `Printer::split_open_delimiter_glued_run` and `Printer::delimiter_line_comment_prefix` — with one value for the blank, `Printer::push_delimiter_glued_blank`; the full delimiter list and which emitter owns each: ./docs/comments.md §The delimiter-line question. Prettier is **not** uniform: it un-glues at most, **glues** at for-in / for-of, and relocates the comment out of the parens at do-while — each a cataloged divergence. Don't confuse any of them with the *sanctioned* union/intersection carve-out in [docs/comments.md §Trailing and dangling runs](docs/comments.md#trailing-and-dangling-runs-the-separator-goes-before-each-comment-never-after) (a non-last member's redundant paren shell strips and its deferred `//` flushes at the per-member break, `Printer::type_member_separator_follows`) — a deliberate lossless choice the catalog owns. When a fix changes comment handling, default to preserving position; match Prettier only when trailing is lossless and the position carries no signal — otherwise add a `_prettier_divergence` fixture. Principles: ./docs/conformance_prettier.md §Comment Position Philosophy; catalog: ./docs/conformance_prettier_ts_comments.md §Comment relocation.

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
3. **Fixture doesn't match** → it may be outdated: `npm run fixtures:update -- <pattern>`.

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

**Unformatted variant rules:** same content structure as input, usually only whitespace differing; the **bare** `_compact` / `_spaces` names claim exactly that, in one direction (gated by `npm run variants:audit`; ./docs/fixture_naming.md#standard-variant-names). A variant that also flips a *token* prettier normalizes away (a trailing comma, quote style, `<br>` for `<br />`) takes a name that says so (`unformatted_no_self_closing`, `unformatted_with_closing_tag`); older bare-named variants predate the rule and the audit reports them as ungraded. Both formatters must normalize to exactly match input. For `.svelte` this is **enforced** by the render-equivalence check (R rules): the variant and `input` must produce the same browser-visible render via `svelte compile`, so a formatter bug that changed the render *and* landed on `input` can't pass green.

**Fixture files and pattern selection:** `input.*` + `expected.json` at minimum; every optional sibling makes a precise, validated claim. Per-file semantics, each pattern's exclusions and the F/S/R/D/P/N rules: ./docs/fixture_overview.md §Pattern Selection.

- **Parser matches Svelte**: `input.svelte` + `expected.json`
- **Parser differs intentionally** (`_svelte_divergence`): `expected_ours.json` + `expected_svelte.json` — including a tsv over-acceptance (canonical **rejects**, only tsv parses), where `expected_svelte.json` is `{"error": "failed to parse"}`
- **tsv over-rejects but canonical accepts** (`_svelte_divergence`): `tsv_rejects.txt` (trimmed content = expected tsv-error substring) + `expected_svelte.json` + README
- **Formatter matches prettier**: `unformatted_*.*` variants
- **Formatter differs intentionally** (`_prettier_divergence`): `output_prettier.*`
- **Normalization divergences**: `unformatted_ours_*.*` normalizes to input with our formatter only; `unformatted_prettier_*.*` normalizes to `output_prettier.*` with prettier
- **Prettier's stable forms** (`_prettier_divergence`): `prettier_variant_*.*` (ours normalizes), `variant_*.*` (both keep stable), `divergent_variant_*.*` (prettier keeps stable, ours → a third form); multi-pass pins `prettier_intermediate_*` / `prettier_intermediate_to_variant_*` / `prettier_intermediate_to_divergent_variant_*`
- **Prettier's chain fits no single-form marker**: auto-generated `audit_signature.txt` / `audit_signature_<suffix>.txt` (N12) — the marker of last resort; `fixtures:update:formatted` decides, never hand-written
- **No prettier oracle** (`_prettier_divergence` + README): `prettier_nonconvergent.txt` (never converges) or `prettier_rejects.txt` (trimmed content = expected-error substring)
- **The parse fact lives only in a form `input.*` can't hold** (a leading BOM the format side strips, a newline in a region the canonical parser overwrites, a token the format side drops): pin a variant — an empty `expected_<stem>.json` beside variant `<stem>.*`, filled by `fixtures:update:parsed` (P4/S24)
- **Parse goal**: a `goal` marker — the `.ts` input parses at `Goal::Script` on both sides (`typescript/script_goal/*`)
- **Invalid syntax**: `input_invalid_*` — must fail BOTH parsers, one syntax error per file
- **Both differ**: `_svelte_prettier_divergence` suffix

> **Troubleshooting:** ./docs/fixture_overview.md#quick-decision-tree

## Debug Tooling

**tsv_debug** reaches the JS tools (prettier, Svelte parser and compiler, acorn) through a sidecar: Node processes running `crates/tsv_debug/sidecar/sidecar.ts` in place under `node --permission`, spawned on first use and reused (orders of magnitude faster than per-call spawns). A `tsv_debug` binary runs the sidecar from the source tree it was built from, so it needs that tree's `npm ci --prefix crates/tsv_debug/sidecar` and a real Node first on `PATH`. Verify with `cargo run -p tsv_debug check`.

### Commands

**Input methods** (content-processing commands): a file path (parser from its extension, which must be one tsv handles unless `--parser` names the grammar), `--content <string> --parser <type>`, or `--stdin --parser <type>` (`svelte|typescript|css`) — except the single-language commands (`canonical_compile`, `compile_compare`, `compile_fixture_init`, `render_compare`, `line_width`), which take `--content`/`--stdin` with no `--parser` (`render_compare`: two inputs, any mix of repeatable `--content` and file paths, no `--stdin`).

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
#   Svelte 5 first, so render-equivalent forms match; real content / <pre> / presence-of-space changes still differ

cargo run -p tsv_debug canonical_parse file.svelte    # parse with the canonical parsers (Svelte, acorn+typescript, parseCss)
cargo run -p tsv_debug format_prettier file.svelte    # format using prettier (line widths by default; --no-line-widths)
cargo run -p tsv_debug line_width file.svelte         # visual line widths (pure Rust; --line N for one line with preview, --json)
cargo run -p tsv_debug strip file.ts                  # tsv_ts::strip to stdout (a refusal or parse error to stderr, exit 1)
cargo run -p tsv_debug imports file.ts                # parse_imports as a binding returns the records
# strip / imports --stdin answer NDJSON requests (`{source}` / `{path, source}`) — the tsv side of strip:corpus / imports:corpus

# fixture_init - create/reinit a fixture (formats through prettier + generates expected.json)
cargo run -p tsv_debug fixture_init <dir> --content '<code>'   # or --stdin; bare = reformat the existing input file
# Also: --parser <svelte|typescript|css|svelte-ts> (aliases ts, svelte.ts), --force, --goal <script|module>
# (.ts/.svelte.ts only: writes the `goal` marker + generates expected.json at that acorn sourceType; omitted keeps
# the directory's existing marker)
cargo run -p tsv_debug fixtures_validate [pattern...]   # --prettier-only, --list. Cross-fixture duplicate detection is skipped
#   when filters are active; a parser mismatch with expected.json is a hard error (no ratchet)
cargo run -p tsv_debug fixtures_update_parsed     # expected.json only (Svelte / acorn / parseCss; `loc`/`name_loc` stripped — span-only wire)
cargo run -p tsv_debug fixtures_update_formatted  # output_prettier.svelte (auto-deletes if identical to input; skips the no-oracle markers)
cargo run -p tsv_debug fixtures_audit [pattern...]      # normalization graphs (diagnostic; --all, --verbose, --json)
cargo run -p tsv_debug ts_fixture_audit [pattern...]    # Necessary / Convertible (formatting-safe only, not a mandate) / Intentional
cargo run -p tsv_debug loc_wires [root]   # both parse wires of every fixture document as NDJSON — the Rust half of `npm run check:loc`;
#   --stdin answers `{language, goal?, source}` with the loc wire, which no binding ships

cargo run -p tsv_debug test262 [path-pattern]     # ECMAScript conformance (pure Rust; ../test262). --gate is the release gate; a bare
#   run exits non-zero by design. Flags + the tsv-vs-oxc differential: ./docs/conformance_test262.md
```

**Compiler tools** (Svelte→JS vs the canonical compiler): `canonical_compile`, `render_compare a.svelte b.svelte` (do two sources render the same page?), `compile_compare`, `compile_fixture_init`, `compile_fixtures_validate`, `compile_corpus_compare` (`--ratchet` is ⚠️ always a SEPARATE invocation), `compile_fuzz` (⚠️ RED BY DESIGN), `erase_comment_census` — flags, exit codes and what each grades: ./docs/compile_tooling.md and ./docs/compile_validation_ratchet.md.

The pure-Rust integrity audits are `tsv_debug` commands too (`conformance_audit`, `compile_conformance_audit`, `variant_audit`, `canonicalize_audit`, each `--json`), run through their `npm run` gates under [Fixtures](#fixtures-rust--node-based); profiling commands are under [Performance Profiling](#performance-profiling).

**Typechecker conformance (`tsc_conformance`) — EXPERIMENTAL, may never ship.** `tsv_check` is a from-scratch TypeScript binder + checker; no shipped artifact links it, and the parser and formatter are never modified in service of it. `tsv_debug tsc_conformance` grades it against tsgo's committed `.errors.txt` baselines (`../typescript-go`) via **on-demand** tasks — `npm run conformance:tsc-roundtrip`, `npm run conformance:tsc-check` (`:update` re-pins after deliberate drift) — none in `check`, `conformance`, or release gating. ./docs/typechecker.md.

## Architectural Notes

### Closed Scope, Open Convention

tsv ships a closed language set (TypeScript, CSS, Svelte) but is open by convention **at the Rust source/crate level**: each language crate (`tsv_ts`, `tsv_css`, `tsv_svelte`) is self-contained — owns its internal AST, parser, formatter, and convert layer — and exposes the same free-function API (`parse_to_json()`, `parse_to_json_with()`, `format()`; hidden beneath them `__private::parse()` and the `__private::convert_ast_json_*` writers). **No central `Language` trait, no registry, no enum dispatch.** So:

- **Optimal artifacts**: concrete types end-to-end, no dyn dispatch; WASM tree-shakes by feature at link level — `@fuzdev/tsv-format-wasm` excludes the wire-JSON layer (`json` feature), `@fuzdev/tsv-parse-wasm` the printers.
- **Source-level openness**: anyone can publish a same-shaped `my_org/tsv_html_parse` crate and any downstream _Rust_ consumer can `use` it without central buy-in. Published CLI/WASM binaries still hardcode the language list (`lang_bindings!` macro), by design.

Cross-language coupling exists only where languages integrate — `tsv_svelte` depends on `tsv_ts` (for `Expression`) and `tsv_css` (for `StyleSheet`). Don't invert this: no central public-AST crate, no dyn `Language` trait, no workspace-level language registry. ./docs/architecture.md#closed-scope-open-convention.

### Strictness: Module Strict, Script by Directive

**Strictness is a property of the source text, read as the spec defines it.** The rules in brief; the full statement with spec references is [docs/conformance_test262.md §Design Decision: Module Strict, Script by Directive, Annex B Out](docs/conformance_test262.md#design-decision-module-strict-script-by-directive-annex-b-out):

- **Module code is always strict; Script code is strict iff its directive prologue holds a `"use strict"`**, as is any function body, class, or nested scope that inherits or declares it. Every **parse** entry point but the explicit `Goal::Script` parses a module, so the everyday answer is *strict* (a Svelte `<script>`, a `.ts` file).
- **Three rules move with strictness**, each rejected by the parser rather than deferred (tsc reports all three wherever it reads the file): the **leading-zero numeric literal** (`010`, `08`), the **legacy string escape** (`"\7"`, `"\8"` — a bare `"\0"` is legal everywhere, and a later `"use strict"` re-grades the prologue literals ahead of it), and the **`with` statement** (only the statement moves; `with` stays reserved in every mode). The untagged template's `NotEscapeSequence` rule is mode-independent and deferred.
- **Annex B is out** at both goals — tsv is a formatter and parser, not a browser host.
- **The parser is deliberately permissive and defers static-semantic early-errors** (the TypeScript ambient-context rules included) to the diagnostics layer, so the formatter keeps formatting everything well-formed. The **correctness oracle for what's actually an error is tsc**, not acorn-typescript (matched only for AST *shape*), but where ECMAScript defines the construct the **spec's own layering outranks tsc's in both directions** — a production rejects, a Static Semantics early error defers. Prettier is the formatting reference only, never by itself an accept/reject signal. Full rule: [docs/conformance_tsc.md §The reject-vs-defer line](docs/conformance_tsc.md#the-reject-vs-defer-line); see also [crates/tsv_ts/CLAUDE.md §Architecture Position](crates/tsv_ts/CLAUDE.md#architecture-position).
- **Strictness and the *goal* axis are orthogonal, with one coupling: Module ⟹ strict.** A parse runs against `tsv_ts::Goal::{Module, Script}` (`parse_with_goal`, CLI `--source-type script|module`), **defaulting to `Module`** (correct for Svelte `<script>` and ~all real TS; Svelte hard-wires it). At `Script` goal `await` is an ordinary identifier, and top-level `import`/`export` declarations, `import.meta` and a top-level `for await` are syntax errors; dynamic `import(...)` and a TypeScript import-equals stay valid. `sourceType` follows the goal.
- **`format` and the type strip read an *unset* goal as a fallback** (`tsv_ts::__private::parse_with_goal_or_fallback`): Module first, Script only on failure — never after a depth refusal, and never for a path whose extension settles the goal (`.mjs`/`.mts`, `tsv_ts::Goal::from_extension`). When both fail, a Script retry that died on a **goal gate** proves the file a module and the Module error is reported; otherwise the error that reached **further** into the source wins, the Module one on a tie. `parse` keeps the Module default at every surface.

### Line Terminators: parse preserves, format folds

Rationale and the full rules: ./docs/architecture.md#line-terminators-parse-takes-the-authors-bytes-format-folds-first and ./docs/architecture.md#loc-lines-one-rule-per-document.

**`parse` never rewrites its input** — its byte offsets are a drop-in contract with acorn / Svelte / `parseCss` over the author's bytes. **Every parse-then-format entry point folds `<CR>` / `<CR><LF>` to `<LF>` before parsing** (`tsv_lang::printing::normalize_carriage_returns` — each language crate's `format` and `Formatter`, each binding's format export, `canonicalize_js`), so output is LF-only even in regions copied verbatim. Fold ahead of the parse, never on the finished string: several printer sites split lines on `'\n'` alone, and a late fold makes the same document format two ways on two passes. `<LS>` / `<PS>` are deliberately NOT folded. The one `<CR>` whose fold would change meaning — a lone one with comment text after it inside a Svelte in-tag `//` comment, which Svelte ends at `\n` alone — is **refused**.

**A leading byte-order mark is the one input `parse` reads two ways, because the oracles do.** Every lexer skips a BOM at byte 0 and spans stay file-true, but the *emitted* position follows each wire's canonical parser (`tsv_lang::LeadingBom`): Svelte's `parse` and `parseCss` strip the BOM first, so the Svelte and CSS writers build their map `Elided` (every offset indexes the BOM-less string); acorn counts it as whitespace, so the TypeScript writer builds `Counted`. `locations.js` makes the same split. Pinned by each `*/whitespace/bom_prettier_divergence` fixture's `expected_prettier_variant_bom.json` (./docs/conformance_prettier.md#whitespace-bom-handling).

**`loc` has one definition.** Every object on the `loc` wire with numeric `start`/`end` — all three languages, `type`-less objects included — gets `loc` right after `end`: the line (1-based) and column (0-based, UTF-16 code units) of its own emitted offsets, under one line rule per DOCUMENT — ECMAScript's terminators for TypeScript (acorn's), `\n` alone for a Svelte document (everything embedded in it included) and for CSS. A superset of Svelte's wire reproducing none of its `loc` quirks (each a named tolerance in the corpus comparison); the line table is built at write time only when `loc` is requested (`tsv_lang::WirePositions`); fixtures pin the span-only wire, and `tests/loc_definition.rs` grades the definition.

### Language-Level concerns (classification)

HTML knowledge lives in two places, split by whose answer it is. `tsv_html` (published) holds HTML spec data and the spec's answers — today the named character references table (`match_named_character_reference()`) — and its long-term target is a spec-faithful HTML library. The Svelte- and prettier-plugin-svelte-shaped answers (`is_block_element()`, `is_void_element()`, `preserves_whitespace()`, the other whitespace rules, Svelte's auto-close table and tag-name class, Svelte's character-reference decoder) are private to `tsv_svelte` (`tsv_svelte/src/html/`, pure functions over tag names; `tsv_svelte_compile` and `tsv_debug` reach the ones they read through `tsv_svelte::__private`), with thin printer adapters (`tsv_svelte/src/printer/classification/`) that resolve symbols, call them, and traverse the AST, for reuse across all planned tools (formatter, linter, compiler, LSP).

### AST Architecture: Internal AST vs Wire JSON

Drop-in replacement for the canonical parsers' **public JSON AST** (acorn / acorn-typescript / Svelte / `parseCss`), NOT their internal implementation.

- **Internal AST**: clean, semantic representation (decoded strings, normalized values) — what every tool (formatter, linter, …) builds on.
- **Wire JSON**: the parse product. The per-language writers (`ast/convert/write*`) emit it **directly from the internal AST in a single walk**, applying each acorn/`parseCss`/Svelte quirk at emission time — never materializing a typed public-AST Rust layer. The wire shape *is* the contract, documented by the hand-maintained `crates/tsv_wasm/types/tsv_ast.d.ts`. No shipped crate reads the wire back (the CLI's `--pretty` re-indents the bytes); `tsv_debug::json` (the fixture gate, the audits) is the one reader, unbounded in depth.

Worked example + full design: ./docs/architecture.md §Two-AST Design.

**Key Rules**:

- Raw strings NEVER duplicated in the internal AST (extract via `source[span.range()]`)
- The internal AST is NEVER the wire output — the writer hand-emits the wire JSON; `serde_json` is the writer substrate's test oracle (`tsv_lang`'s hand string escaper and its `zmij`-backed `f64` emitter are graded byte-for-byte against it) and, in `tsv_debug` alone, reads bytes back into a `Value` (fixture gate, audits, tests); no crate meant for publishing depends on it

### Position Types: u32 vs usize

- **Span**: `u32` start/end (8 bytes total, 50% savings vs usize)
- **`Token`**: `u32` start/end — a 16-byte POD `{kind, start, end}`, size pinned by a `const` assert in tsv_ts and tsv_css (tsv_svelte's leaner `Token` pins at 12 B). On the parsers' hot path the lexers write it straight into the parser's token slot (`next_token_into`), since a by-value `Result<Token, Error>` returns through a stack slot the parser would reload and re-scatter; other TS and CSS callers take it by value (`next_token`); the Svelte lexer has no by-value form. The decoded value (escapes only) lives out-of-band on the lexer (the reused `Lexer::decode_scratch` buffer, borrowed via `decoded_str`)
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

The shipped language/foundation crates' external deps (`tsv_cli` adds only `argh`; dev tooling adds `tokio`, `futures-util`, `serde` with `derive`, `similar`, `tempfile` and `thiserror`; `tsv_wasm` adds `wasm-bindgen`/`js-sys`):

- `zmij` — the wire writer's non-integral `f64` digits (`tsv_lang`'s `json` feature): the shortest round-trip formatter `serde_json` itself calls, called directly so the bytes stay `serde_json`'s (graded by `f64_matches_serde_json`) with no `serde_json` in the published graph
- `serde_json` — **never a normal or build dependency of a crate meant for publishing** (every crate with a `[package.metadata.docs.rs]` table; `npm run deps:audit` gates it), because the workspace entry's `preserve_order` / `float_roundtrip` / `arbitrary_precision` features would unify into a consumer's own `serde_json` and change its deserialization. It is the dev-dependency oracle of `tsv_lang`'s writer and `tsv_html`'s entity-table generator, and a normal dependency of the unpublished `tsv_ffi`, `tsv_debug` and the root test package; the one reader, `tsv_debug::json`, enables `unbounded_depth`. `serde` itself is a dev-tooling dep, out of every published library crate's normal graph (`tsv_cli` reaches it through `argh` → `argh_shared`, which carries no `serde_json`)
- `smallvec` — stack-allocated vectors (printers + `tsv_check`)
- `unicode-ident` / `unicode-segmentation` / `unicode-width` — XID identifiers, grapheme clustering, display width (CJK, zero-width)
- `bumpalo` — bump arena for the internal AST (and, via `tsv_arena`, the bindings' per-thread `reset()` reuse; `tsv_check`'s caller-owned arenas follow the same contract)
- `talc` — WASM global allocator (`tsv_wasm`, wasm32-only target dep): pure-Rust `no_std`, replacing dlmalloc; the `WasmGrowAndExtend` source keeps the warm instance's linear-memory high-water at dlmalloc parity. Pulls `lock_api` + `allocator-api2` (+ `scopeguard`) into the wasm32 graph only
- `napi` / `napi-derive` / `napi-build` — N-API bindings for `tsv_napi` (tsv-scoped carve-out)

## Canonical References

**Implementations** (versions pinned in `crates/tsv_debug/sidecar/package.json`, the whole tree in its `package-lock.json`):

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
- ./docs/strip.md - the type strip: surfaces, the position invariant, verdicts and refusals, oracles
- ./docs/imports.md - import records: surfaces, the record model, what is and isn't a record, coordinates, oracles
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
