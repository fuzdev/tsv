# Benchmarking & Corpus Comparison Infrastructure

> Compare tsv formatting against Prettier on real codebases

Uses [@fuzdev/fuz_util](https://github.com/fuzdev/fuz_util) benchmarking library for statistical analysis.

> **Directory note:** this is the **runtime-neutral** JS benchmark harness —
> named `js` (not `deno`) because the same code runs under Deno, Node, and Bun
> (see [Cross-Runtime](#cross-runtime-deno--node--bun)). The `corpus_compare_*`
> and `diagnostics/` entries stay Deno-idiomatic; `smoke` is portable across all
> three (`smoke` / `smoke:node` / `smoke:bun`).

## Cross-Runtime (Deno + Node + Bun)

The bench runs under **Deno, Node, and Bun** from one shared codebase.
The motivation: a single-runtime bench can silently fold a runtime-specific effect
into an engine number — the concrete case being the Deno-FFI fast-call memory
sensitivity that mismeasured the native path (see §Known Issues). A per-runtime
delta on the same row is the detector.

**Design (well-factored, not forked):**

- **Runtime-labeled sibling reports.** Each runtime writes its own
  `results/report.<runtime>.{json,md}` (+ a timestamped `…_<commit>.<runtime>.*`
  pair), same schema, never merged. Every row carries a `runtime` field; the JSON
  has a top-level `runtime` and `version: 5`. `deno task bench:compose` (run at
  the end of `deno task bench`) then folds the siblings into a compact combined
  `results/report.{json,md}` — the cross-runtime view tsv.fuz.dev consumes
  (`compose_reports.ts`; a per-runtime delta on a row is the headline).
- **One bench body, runtime-detected.** `bench.ts` runs under both — it detects
  the runtime (`lib/runtime.ts` `current_runtime()`) and selects the
  runtime-specific artifacts. No forked entry; `bench:node:run` is literally
  `node benches/js/bench.ts`.
- **Portable shared modules.** The shared/entry modules use `node:` builtins
  (Deno supports them) + `@fuzdev/fuz_util` helpers (`fs_search`, `fs_exists`,
  `spawn_out`, `to_file_path`) — **no `Deno.*`, no `@std/*`**. The only genuinely
  runtime-specific files are the native loader (`ffi.ts` `Deno.dlopen` vs
  `napi.ts` `process.dlopen`) and the WASM target the loader picks. The
  Deno-only entry points (`corpus_compare_*`, `diagnostics/*`) stay
  Deno-idiomatic (`Deno.*` + `node:` builtins, no `@std/*`). The `deno test`
  suite is the dependency-free divergence detectors (`node:assert` + relative
  imports — see §Divergence Detection).
- **The native row differs by runtime, fairly.** Deno → FFI (`tsv_ffi`, via
  `Deno.dlopen`); Node → N-API (`tsv_napi`, via `process.dlopen`). Same engine,
  same per-thread arena reuse, different binding boundary.
- **The WASM row uses each runtime's own wasm-pack target bundle** (same
  `tsv_wasm_bg.wasm`, different JS glue) — Deno the `deno` target, Node the
  `nodejs` target — both with the full export set incl. `parse_internal_*`. The
  shipped web bundle is deliberately not used (it curates `parse_internal_*` out).

**Dependencies: `package.json` is the source of truth.** Both runtimes consume one
`node_modules` (`benches/js/package.json`). Deno reads it via
`"nodeModulesDir": "manual"` in `deno.json`; Node reads it directly. There are no
jsr or remote deps — the bench and the diagnostics import npm packages by bare
specifier (resolved from `package.json`) and otherwise use `node:` builtins, so
`deno.json` carries only `nodeModulesDir: manual` + `lock: false` (no `deno.lock`;
npm integrity is `package-lock.json`'s job). `@types/node` is a types-only
devDependency so the `node:` builtins type-check under `deno check`.

**Install with `deno task bench:install`** (runs `benches/js/install_deps.ts`).
It is the canonical installer: a plain `npm install` works but **prunes the
`@oxc-parser/binding-wasm32-wasi` binding** (the oxc-parser-wasm row) — that
binding is pure-wasm but its metadata declares `cpu: wasm32`, so npm skips it on
any non-wasm host and reconciles away a `--no-save` add. `install_deps.ts` runs
`npm install` then force-fetches it at the `oxc-parser` version (oxc ships all
bindings in lockstep). After a bump or a stray `npm install`, re-run
`bench:install`.

**oxc-parser-wasm runs under all three.** Its binding ships two entries — a
fetch-based browser entry (`parser.wasi-browser.js`) that Deno needs, and the
default `node:wasi` entry that Node/Bun need — so `oxc_wasm.ts` picks the right
one per runtime (`current_runtime()`). (Node also has oxc-parser **native**, the
more relevant Node number, regardless.)

## Corpus Comparison

Compare formatting output against Prettier on arbitrary codebases:

```bash
# All default corpus repos from ~/dev (~5600 files)
deno task corpus:compare:format --all
deno task corpus:compare:format --all --explain
deno task corpus:compare:format --all --summary
deno task corpus:compare:format --all --limit 20

# Single project (scans <path> recursively — NO srcDir filtering)
# ⚠ For monorepos like svelte/, use --all instead to avoid scanning test fixtures
deno task corpus:compare:format ~/dev/some-project
deno task corpus:compare:format ~/dev/some-project --filter svelte  # Only .svelte files
deno task corpus:compare:format ~/dev/some-project --limit 50       # First 50 per language
deno task corpus:compare:format ~/dev/some-project --explain        # Show divergence patterns
deno task corpus:compare:format ~/dev/some-project --summary        # Compact output (no diffs)
deno task corpus:compare:format ~/dev/some-project --strict         # Fail on any difference
deno task corpus:compare:format ~/dev/some-project --safety-only    # Only check for data loss
deno task corpus:compare:format ~/dev/some-project --json           # JSON report to stdout (logs → stderr)

# Run without rebuilding FFI (if already built) — guarded against a stale FFI
# binary (see "Artifact Freshness Guard" below); BENCH_STALE_OK=1 overrides
deno task corpus:compare:format:run ~/dev/some-project
```

The `corpus:compare:format:run` task sets `PRETTIER_DEBUG=1` so
prettier-plugin-svelte's verbatim-on-error fallback (whole `<script>` block
echoed when the embedded formatter throws) surfaces as a per-file **error**
with a code frame instead of fake-stable prettier output that would land in
`unknown`. Same posture as the tsv_debug sidecar; see
`docs/conformance_prettier.md` §Triage caveat.

### Machine-readable output (`--json`)

`--json` emits a single buffered JSON object to **stdout** and routes all
human/progress output to **stderr**, so `2>/dev/null` leaves a clean
`JSON.parse`-able document. It's a `stats` block plus per-file lists for the
statuses worth inspecting:

```jsonc
{
	"stats": {
		"languages": {
			"typescript": {
				"total": 27,
				"match": 11,
				"known": 3,
				"partial": 1,
				"unknown": 8,
				"safety": 4,
				"errors": 0,
				"expected_errors": 0
			}
		},
		"total": { "total": 27, "match": 11, "...": "..." }
	},
	"safety": [
		{
			"path": "union-parens.ts",
			"language": "typescript",
			"bytes": 0,
			"violations": [
				{
					"type": "content_lost",
					"total": 7,
					"chars": [{ "char": "|", "real": 2, "ours": 28, "prettier": 26 }],
					"missing_lines": [],
					"summary": "..."
				}
			]
		}
	],
	"partial": [{ "path": "...", "patterns": ["..."] }],
	"unknown": [{ "path": "...", "diff_summary": "we break (+1 lines): \"...\"" }],
	"errors": [{ "path": "...", "error": "..." }],
	"expected_errors": [{ "path": "...", "error": "...", "expected_reason": "..." }]
}
```

`match` and `known_divergence` files are excluded (their counts live in
`stats`); full diffs are excluded (`unknown` carries a one-line `diff_summary`
instead) — so the object stays small regardless of corpus size, and the
`results` map is already fully in memory for the end-of-run report so there's
nothing to stream. Works with `--all`, `--safety-only`, and `--filter`.
Automation that just wants the table reads `.stats.total`; triage tooling reads
`.safety` / `.unknown` / `.partial` / `.errors`.

**Usage** (redirect stderr away to get a clean stream; use the rebuild variant
`corpus:compare:format` after a Rust change — the `:run` form now aborts on a stale
FFI binary, see "Artifact Freshness Guard" below):

```bash
# Capture the whole report
deno task corpus:compare:format:run --all --json 2>/dev/null > report.json

# Pipe into jq for specific slices
deno task corpus:compare:format:run --all --json 2>/dev/null | jq '.stats.total'        # table numbers
deno task corpus:compare:format:run --all --json 2>/dev/null | jq -r '.safety[].path'   # files losing content
deno task corpus:compare:format:run --all --json 2>/dev/null | jq '.unknown | length'   # count unknowns
# Per-char loss breakdown for one safety file (ours vs prettier, real = beyond prettier)
deno task corpus:compare:format:run --all --json 2>/dev/null | jq '.safety[0].violations[0].chars'
```

Output shows match rates, known divergences, and any issues:

```
Results:
  svelte       166/179 match (92.7%)    | 13 known
  typescript   172/188 match (91.5%)    | 16 known
  css          1/2 match (50.0%)        | 1 known
  ────────────────────────────────────────────────────────────────────────
  total        339/369 match (91.9%)    | 30 known

Known Divergence Patterns:
  fill_101_boundary: 29 files
  template_literal_width: 13 files
  ...

PASS: No safety violations or unknown differences
```

## Parse Comparison

Deep-diff tsv's shipped parse output against the canonical parsers
(acorn-typescript / `svelte.parse` / `parseCss`) — the parser-side sibling of
the formatting comparison above. Native-FFI-only by design: the WASM artifact
rides the same Rust wire (`convert_ast_json_string`), differing only at the
boundary, and is already exercised per-file by the bench preflight and
`deno task smoke`. This is the external
oracle the internal identity gates can't provide: fixtures cover curated
cases, and P4/P5/`json_profile` only prove tsv's two materialization paths
agree with each other, so a bug shared by both walks (e.g. an untranslated
position field) is invisible to all of them.

```bash
deno task corpus:compare:parse --all                    # full corpus
deno task corpus:compare:parse --all --multibyte-only   # offset-translation slice (riskiest machinery)
deno task corpus:compare:parse ~/dev/zzz --filter typescript --limit 100
deno task corpus:compare:parse --all --json 2>/dev/null > report.json
deno task corpus:compare:parse:run --all                # skip rebuild (freshness-guarded)
```

Method: ASTs are **raw-diffed with no pre-diff normalization**; diffs are
classified against the documented divergences (docs/conformance_svelte.md) at
the reporting layer only, so a bug in our own divergence reasoning surfaces as
an undocumented group instead of being silently absorbed. The canonical AST is
serialized exactly like the fixture sidecar (JSON round-trip, BigInt → string)
so corpus and fixture semantics match. Diffs are grouped by path signature
(array indices erased) across files; undocumented groups are the actionable
output and fail the run (exit 1). Parse failures on either side are counted
and skipped — `skip_triage.ts` is the dedicated tool for those.

The documented-divergence matchers live in `corpus_compare_parse.ts`
(`DOCUMENTED_MATCHERS`) and cover only the AST-content divergences that parse
on both sides (comment-attachment duplication, async-generic-arrow params);
the parser-feature corrections (`using`, v-flag regex, CSS namespaces) make
the canonical parser throw, so they land in the error buckets. When triage
confirms a new group is intentional, add a matcher AND catalog it in
docs/conformance_svelte.md.

## Divergence Detection

Automatically detects known divergence patterns from `conformance_prettier.md`:

- **Safety checks**: Differential character-frequency comparison vs prettier detects data loss — reports only the semantic chars our output drops/adds **beyond** what prettier does (shared normalizations cancel)
- **Pattern detection**: Hunk-aware detection - patterns must explain specific diff hunks, not just match global file properties
- **Classification**: `known` (all hunks explained), `partial` (some hunks unexplained), `unknown` (needs investigation), `SAFETY` (data loss)

### Detected Patterns

See `patterns.ts` for the full list with detection logic, `patterns_test.ts` for tests.
Patterns are ordered specific to broad. Each links to `conformance_sections` and `fixtures`.

### Divergence Audit & Testing

```bash
# Static check: cross-references pattern fixtures arrays vs conformance_prettier.md
deno task divergence:audit        # Human-readable report
deno task divergence:audit --json # Machine-readable JSON

# Deno test suite — the divergence detectors under lib/divergence/, gated by
# `deno task check`. Covers pattern positive/negative overmatch-rejection cases,
# safety differential cases, and a behavioral fixture-coverage audit that drives
# each detector against its own committed fixtures — input == ours, output_prettier
# == prettier — failing if a pattern stops claiming a hunk in a fixture it lists.
# These are dependency-free (`node:assert` + relative imports, no node_modules), so
# CI runs them on a clean checkout with no `bench:install` — that's why they're in
# the core `check` gate.
deno task test:deno

# The canonical-oracle test (NOT gated — it needs prettier/svelte, so run after
# `bench:install`): asserts the prettier baseline formats with a filepath, so `.ts`
# single-type-param arrows stay `<T>` and `.svelte` ones get `<T,>`.
deno task test:deno:canonical

# Per-pattern corpus coverage with sample diffs (spot-check for overmatching)
deno task corpus:compare:format --all --audit-patterns
```

Output shows coverage gaps (numbers illustrative — the live run prints
current counts). "Documented" = every `*_prettier_divergence`-suffixed
fixture linked from `conformance_prettier.md` in any of its three anchor
formats (table rows, list items, prose paragraphs); non-divergence fixture
links (match/contrast anchors) don't count. Coverage is partial by design —
see `docs/divergence_detector.md` §Traceability.

```
Divergence Detection Audit Report
==================================================

Documented divergences: 209
Covered by patterns:    125
Uncovered:              84
Coverage:               60%

Uncovered Fixtures (no pattern detects these):
--------------------------------------------------
  CSS: At-Rules:
    - container_spacing (Spec violation)
      css/at_rules/container_spacing_prettier_divergence
    ...
```

Every pattern in `patterns.ts` includes:

- `conformance_sections` - Which doc sections it covers
- `fixtures` - Which `*_prettier_divergence` fixtures it detects

## Benchmark Commands

Each runtime saves to `benches/js/results/` as timestamped files plus a
committed `report.<runtime>.{json,md}` pair (`report.deno.*` / `report.node.*`).
To publish benchmarks to tsv.fuz.dev, run `npm run update-benchmarks` in
~/dev/tsv.fuz.dev (it reads the per-runtime files — see the cross-repo note in
the lore when renaming from the old single `report.json`).

The committed `report.<runtime>.json` (baseline `version: 5`) carries, beyond
timing stats: a top-level `runtime`, per-language `corpus` totals, `versions`,
and `binary_sizes` (each with `gzip_bytes`). Each `entries[]` row adds a
`runtime` field, `files_processed`/`files_total` (per-impl preflight coverage —
the `Coverage:` line) and `files_iterated` (the timed set — the `Files
(intersection):` count). A top-level `suppressed_noise` map records silenced
third-party stderr crashes as `{pattern: count}`. `report.<runtime>.md` renders
coverage/iterated as prose; the per-entry numbers and `suppressed_noise` are
JSON-only.

```bash
deno task bench:install   # one-time: install harness npm deps (see Cross-Runtime above)

# Run benchmarks (builds the runtime's bench artifacts automatically).
# `bench` runs all three and FAILS FAST if node or bun isn't installed (the `&&`
# chain stops at the missing binary). Deno is the only hard dependency, so if you
# don't have node and/or bun, run the per-runtime tasks you DO have — each writes
# its own report.<runtime>.* sibling, and `bench:compose` folds whatever exists.
deno task bench           # ALL three + compose (needs node AND bun installed)
deno task bench:deno      # Deno only (no node/bun needed)
deno task bench:node      # Node only (needs node)
deno task bench:bun       # Bun only (needs bun; reuses the Node artifacts — N-API + nodejs-target WASM)
deno task bench:compose   # Fold whatever report.{deno,node,bun}.json exist → combined report.{json,md}

# Run without rebuilding (if already built) — guarded against stale artifacts,
# see "Artifact Freshness Guard" below
deno task bench:deno:run
deno task bench:node:run
deno task bench:bun:run

# Output formats / flags (shown for :deno:run; same for :node:run)
deno task bench:deno:run -- --json          # Output as JSON (for CI/tooling)
deno task bench:deno:run -- --markdown      # Output as Markdown tables
deno task bench:deno:run -- --verbose       # Include per-file skip detail (paths + errors)
deno task bench:deno:run -- --save-report   # Force-overwrite the committed report.<runtime>.{json,md}
                                            # on a limited/filtered run (full runs overwrite it anyway;
                                            # the timestamped results/<ts>_<commit>.<runtime>.* pair is always written)

# Baseline regression detection
deno task bench:deno:run -- --save-baseline     # Save current results as baseline
deno task bench:deno:run -- --compare-baseline  # Compare against saved baseline

# Wipe local-only bench state (gitignored): baseline.json + timestamped
# results pairs. Preserves the committed `report.<runtime>.{json,md}` because
# the glob is anchored on a leading digit (timestamped files start with a year).
deno task bench:clean

# Environment variables (apply to any runtime's :run)
BENCH_LIMIT=5 deno task bench:deno:run
BENCH_FILTER=zzz deno task bench:deno:run
BENCH_DURATION=10000 deno task bench:deno:run
BENCH_WARMUP=10 deno task bench:deno:run
BENCH_MODE=union deno task bench:deno:run
BENCH_STALE_OK=1 deno task bench:deno:run
BENCH_FORCED_ASYNC=1 deno task bench:deno:run
```

## Artifact Freshness Guard

The rebuild-skipping tasks (`bench:deno:run` / `bench:node:run`,
`corpus:compare:format:run`, and `smoke`) skip the rebuild so you can iterate on
the harness without paying the wasm-pack cost — at the risk of silently measuring
a binary older than current source (a CSS run once reported `146/183` against a
stale `.so` that should have been `155/183`). `lib/check_artifact_freshness.ts`
guards against this: before a run touches the executed artifacts (the runtime's
native binding + WASM bundle — Deno: FFI + `pkg/all/deno`; Node: N-API +
`pkg/all/nodejs`), it compares their mtimes against the crate sources feeding them
and **aborts (exit 1)** if any is stale, or missing. The build-first tasks
(`bench`, `bench:deno`, `bench:node`, `corpus:compare:format`) rebuild first, so
they pass the guard for free.

**Escape hatch:** `BENCH_STALE_OK=1` downgrades a _stale_ artifact to a `⚠`
warning and proceeds (a _missing_ one stays fatal). See the module doc for the
full rationale (e.g. why stale is a hard error, not a warning).

```bash
# After editing a crate, the fast/correct paths:
deno task bench:deno                              # rebuilds, then runs — always fresh
deno task build:ffi && deno task bench:deno:run  # rebuild just what you changed, then :run
BENCH_STALE_OK=1 deno task bench:deno:run         # deliberately measure the current (stale) binary
```

## Smoke Test

`deno task smoke` runs a fast sanity check on every formatter and parser
(trivial fixed inputs, non-throwing + non-empty + idempotent). Exits non-zero
on any failure. Use it to catch "implementation totally broken" before
running the full bench. `corpus_compare_format` is still the real correctness gate.
It is runtime-neutral like the bench: `smoke` (Deno), `smoke:node`, and
`smoke:bun` each load that runtime's own native + WASM artifacts, so an
impl-load break is caught per runtime (it's how the Bun biome-load issue surfaced).

Like the bench/corpus `:run` tasks, `smoke` skips the rebuild for speed and is
guarded by the freshness check above — it aborts on a stale or missing native/WASM
artifact (rebuild with `deno task build:bench`, or `BENCH_STALE_OK=1` to
override). It is **not** a build-first task.

## Fairness Caveats

Things the published numbers measure that aren't quite what they look like:

- **Single-threaded, per-file (universal)** — the harness times one file at a
  time, sequentially (`await`ed in order, no `Promise.all` over files), so the
  numbers are per-file single-core latency/throughput, not multi-core batch
  throughput. Per-file compute is single-threaded for every impl: tsv (FFI +
  WASM) pulls in no threading crate (`rayon`/`num_cpus`/`threadpool`/`crossbeam`
  absent from every `Cargo.toml`, and the workspace's `tokio` is dev/debug-only —
  not in the shipped `tsv_ffi`/`tsv_wasm` chain); prettier, `svelte/compiler`, and
  `oxc-parser.parseSync` are single-threaded JS. The lone nuance is `oxfmt`,
  whose async API bundles `tinypool` and may run a call on a worker thread —
  still one thread of compute per file, and each call is fully awaited before
  the next, so no fan-out is exploited. This deliberately excludes the
  multi-core batch throughput a CLI gets formatting many files at once (which
  most of these tools, tsv included, could provide) — that's a different
  benchmark.
- **Different tools produce different output — speed is not conditioned on
  correctness.** The timed work is "produce _this tool's own_ formatting," not
  "produce the same bytes," and no two of these tools emit identical output.
  `prettier` is the reference; `oxfmt` also targets prettier conformance, so
  `prettier` vs `oxfmt` is the closest to a same-output, same-work race. `tsv`
  tracks prettier closely but _intentionally diverges_ in documented cases (the
  `_prettier_divergence` fixtures / `conformance_prettier.md`; ~92%
  `corpus:compare:format` match, measured separately — not here). `biome` formats to
  its own style. Because layout decisions differ, a format ratio is partly an
  output-shape difference, not pure engine speed — and nothing here verifies
  output validity, so a formatter emitting subtly wrong output fast would still
  "win."
- **The format headline is cross-tier (native Rust vs JIT JS).** The `format`
  baseline is `prettier` (JS) and the flagship `tsv` row is the native FFI
  binary (AOT Rust). That's a fair "what you get replacing prettier with tsv"
  number, not a language-neutral algorithm comparison. The same-tier reads are
  WASM-vs-WASM (`tsv_wasm` vs `biome-wasm` vs `oxc-parser-wasm`) and
  native-vs-native (`tsv` vs `oxfmt`/`oxc-parser`); compare within a tier before
  attributing a gap to the formatter rather than the runtime.
- **Self-corpus / representativeness.** The corpus is dominated by the author's
  own fuz ecosystem plus the svelte/kit/prettier test suites — the same code
  tsv is developed and fixture-tuned against. Throughput tracks the syntactic
  mix of _this_ corpus, so the ratios are "N× on this corpus," not universal.
  CSS is the weakest sample (149–155 iterated files, 0.2 MB), so its per-file
  ratios are the noisiest in the report.
- **Measurement-shape asymmetries (small, mostly self-cancelling).** (a) Every
  `tsv` FFI format call UTF-8-encodes the input and decodes the output back to a
  JS string (`lib/ffi.ts`); `tsv_wasm` marshals strings across the JS↔WASM
  boundary. prettier pays no such boundary tax — so the published `tsv` /
  `tsv_wasm` format numbers are _conservative_ (the raw engine is faster than
  the FFI/WASM figure; the parse analogue is the `tsv-internal` vs `tsv-json`
  gap). (b) The async impls (`prettier`, `oxfmt`) are `await`ed per file
  (`process_corpus_async`), carrying a per-file microtask cost the sync impls
  skip — swamped by their actual format time, but real. The opt-in
  **`tsv-forced-async`** control row (`BENCH_FORCED_ASYNC=1` — the same native
  engine as `tsv`, routed through the awaited async path) quantifies this tax
  directly: the `tsv` vs `tsv-forced-async` delta is within the run-to-run noise
  floor even on a fast sub-ms-per-file engine, so the per-file await does **not**
  materially inflate the async impls' numbers — their gaps vs `tsv` are engine
  differences, not harness tax. It's **off by default**: a noise-level delta would
  only add a confusing duplicate-`tsv` row to the published report and feed
  spurious flags to the regression baseline, so it's an on-demand re-confirmation
  tool, not a standing row. (Why a control and not a real sync row: `prettier` and
  `oxfmt` are async-only — neither ships a sync format API — so the tax can't be
  removed, only measured.) (c) Task return values
  are discarded uniformly for all impls; the FFI/WASM/async boundaries block
  dead-code elimination, so no impl's work is optimized away.
- **`tsv_wasm` is measured on the full build.** The WASM bench loads
  `pkg/all/deno` (the default both-features artifact, ~2.8 MB — what
  `@fuzdev/tsv_wasm` ships) for _both_ parse and format, while subset
  consumers ship the smaller `@fuzdev/tsv_format_wasm` (~2.2 MB, no convert
  layer) or `@fuzdev/tsv_parse_wasm` (~1.7 MB, no printers). The Binary
  Sizes table lists all three; the throughput rows reflect the full build.
  The native `tsv` row is the same story: the perf row loads the full
  `libtsv_ffi`, while the Binary Sizes table also lists `tsv format (ffi)`
  and `tsv parse (ffi)` subset builds (no perf rows of their own — they
  exist only to size scope-matched against `oxfmt` and `oxc-parser`).
- **Intersection-corpus iteration (default)** — within each group, every
  impl is timed on the same all-N intersection: the set of files every impl
  in the group successfully processed during pre-flight. Ratios within a
  group are then apples-to-apples (`ops_per_sec(A) / ops_per_sec(B)` reads
  as "A is N× faster than B on the files they both handle"). Trade-off: one
  noisy impl shrinks the corpus for the whole group — e.g. if `biome-wasm`
  skips 60% of CSS files, `tsv`/`prettier`/`oxfmt` are timed on only the
  remaining 40%. The Coverage section in `report.<runtime>.md` still discloses each
  impl's preflight skip rate, so the asymmetry is visible to the reader
  even though the timed numbers normalize over it. `Throughput` and the
  `(Mf)` annotation in every table reflect the iterated set, not the full
  corpus.
- **`BENCH_MODE=union`** — opt-in escape hatch that restores per-impl
  iteration (each impl runs its preflight success set, not the
  intersection). Ratios then reflect different file sets per impl, and the
  `(Mf)` annotation describes the self impl's iterated count (not the
  pair's overlap). Useful for auditing what intersection mode hides.
- **Ratio convention (universal)** — every `Nx` in the report is **speedup
  form**: `>1` means self is faster than the named opponent, `<1` means
  slower. Column headers spell this out (`vs prettier (speedup)`, `vs Best
  (speedup)`). The only intentional exception is `JSON overhead` rows,
  which are explicitly labeled as `json_ns / internal_ns` (higher = more
  cost) because overhead is inherently a slowdown ratio.
- **Per-iteration forced GC** — off by default (set `BENCH_GC=1` to enable).
  When on, the bench calls `globalThis.gc()` between every iteration. Not a
  uniform bias: measured on a BENCH_LIMIT=20 / 500ms / WARMUP=2 sample
  (hook=on vs hook=off):
  - Low-allocation paths (internal parsers, native parse) are penalized
    heavily — `tsv-internal` runs 1.4–1.7× slower with the hook on, and
    `svelte/compiler` is 2.8× slower (it allocates JS objects every call).
  - Format paths land in the 1.07–1.24× slower range with the hook on.
  - CSS workloads on large inputs reverse the trend — the hook can be
    1.0–1.6× _faster_ than off, because amortizing GC pauses per-iteration
    avoids long mid-loop major-GC stalls.
  - Default off because the published ratios should reflect what users
    see in real code (opportunistic GC). Enable via `BENCH_GC=1` if you
    want the stability of forced GC for a noisy high-allocation workload.
  - A `report.<runtime>.md` generated with the hook on has a narrower
    internal-vs-JSON-materializing-parser spread than a default (hook-off)
    run, so don't diff numbers across the two configurations line-for-line.
- **`-json` parse rows are apples-to-apples; the `oxc-parser` "lazy" story
  is a myth for the path we benchmark.** In oxc-parser's _default_ mode
  (what we call), the AST is serialized to a JSON string in Rust and
  deserialized in JS — the native `oxc-parser` package's `index.js`
  `wrap()` runs `JSON.parse` on `.program` access (verified: `typeof
  program === 'object'`), exactly the model `tsv-json` uses (Rust →
  JSON string → FFI → `JSON.parse`) and `tsv_wasm-json` uses (Rust →
  JSON string → boundary decode → engine `JSON.parse` via `js_sys`).
  So `tsv-json` vs `oxc-parser` and `tsv_wasm-json` vs
  `oxc-parser-wasm` are like-for-like full-materialization comparisons —
  oxc is just faster at it. Two non-obvious points this turned up:
  - **The WASI binding (`oxc-parser-wasm`) does _not_ wrap**, so `.program`
    is the raw unparsed JSON _string_ — `lib/oxc_wasm.ts` now `JSON.parse`s it
    so the row materializes like the others. Before that fix it skipped the
    parse and looked artificially fast, even beating native oxc (the old
    "NAPI marshalling" note that tried to explain this was wrong).
  - **There is intentionally no `oxc-parser-lazy` row.** oxc's genuine lazy
    mode (`experimentalLazy` raw transfer, native-only — `rawTransferSupported()`
    is `false` on WASI) is _not_ a fast parse-only path: it eagerly copies the
    whole AST transfer buffer, so it's setup-dominated, not parse-bound.
    Measured per-call on a 7.6 KB file: ~1.7 ms Node / ~2.1 ms Deno, vs
    ~0.7 ms eager-materialize and ~0.16 ms parse-only — i.e. lazy is _slower_
    than the eager JSON path. This is **not** a Deno artifact: the eager paths
    are byte-identical across Node and Deno (0.706/0.705 ms materialize,
    0.165 ms parse-only), and only the lazy path is ~20% worse under Deno on
    top of an already-slow Node baseline. So `tsv-internal`/`tsv_wasm-internal`
    (parse-only, no JS materialization) have **no fair oxc counterpart** — oxc's
    JS API always serializes to cross into JS — and that asymmetry is left
    honest rather than papered over with a misleading lazy row.
- **Format groups include parse time.** Every formatter parses internally
  before printing. The numbers measure "how fast can implementation X
  format my file end-to-end," which is what users care about — but format
  ratios are partly parser ratios. Documented in the report footnotes.

## Corpus

~5,500 files (~15 MB) of real `.svelte`, `.ts`/`.js`, and `.css` from three
sources (this framing is the source of truth for the public benchmark page's
"What's measured" prose — keep them in sync):

1. **Application & library source** — the fuz.dev repos (`src/` of real projects).
2. **Upstream framework source** — Svelte, SvelteKit, and the svelte.dev site.
3. **Formatter conformance fixtures** — Prettier's and prettier-plugin-svelte's
   own test suites (deliberately tricky edge cases, not typical code — they skew
   the corpus toward hard cases).

Paths defined in `lib/corpus.ts` `DEFAULT_CORPUS_PATHS`, relative to project root:

- zzz (large apps)
- fuz.dev, fuz_app, fuz_css, fuz_ui, fuz_util, fuz_template, fuz_blog, fuz_mastodon, fuz_code, fuz_docs, fuz_gitops (fuz ecosystem)
- gro, svelte-docinfo, tsv.fuz.dev (build tooling)
- kit/packages/kit, svelte/packages/svelte, svelte.dev subpaths (external monorepos)
- prettier-plugin-svelte/test (.html treated as Svelte, files with companion `options.json` skipped)
- prettier/tests/format/typescript, prettier/tests/format/js, prettier/tests/format/css, prettier/tests/format/html (prettier test suites)

Extensions: `.svelte`, `.ts`, `.js`, `.css`, `.html` (treated as Svelte)

Each entry is a string path (e.g., `'../zzz/src'`) or an object with `path`, `extensions`,
and `skip` overrides. Non-existent paths are silently skipped.

## Architecture

```
benches/js/
├── package.json           # npm dep source of truth (both runtimes); install_deps drives it
├── package-lock.json      # npm lock (committed for reproducibility)
├── deno.json              # nodeModulesDir: manual + lock: false (npm from package.json; no jsr/remote deps)
├── install_deps.ts        # `bench:install`: npm install + force-fetch the oxc wasi binding
├── bench.ts               # Benchmark entry point (runtime-neutral — runs under Deno AND Node)
├── smoke.ts               # Smoke test for formatters and parsers (runtime-neutral: smoke / smoke:node / smoke:bun)
├── compose_reports.ts     # Fold report.{deno,node,bun}.json → combined report.{json,md} (bench:compose)
├── corpus_compare_format.ts  # Formatting comparison vs prettier (Deno-only entry point)
├── corpus_compare_parse.ts   # Parse/AST comparison vs canonical parsers (Deno-only entry point)
├── divergence_audit.ts    # Divergence audit entry point (Deno-only)
├── diagnostics/           # ad-hoc diagnostic scripts (not wired into `deno task` — see §Diagnostic scripts)
│   ├── skip_triage.ts        # parse-gap triage (tsv vs canonical)
│   ├── test262_compare.ts    # test262 differential (tsv vs oxc-parser, from the Rust manifest)
│   ├── wasm_json_probe.ts    # WASM-vs-native JSON parse penalty attribution
│   ├── wasm_format_probe.ts  # WASM format wall-time A/B
│   ├── comment_dup_scan.ts   # comment-dup fixture-corpus completeness guard
│   └── acorn_dup_fuzz.ts     # comment-dup fuzz over acorn-typescript's construct corpus
├── results/baseline.json  # Saved baseline for regression detection (gitignored; written by @fuzdev/fuz_util's benchmark_baseline module)
├── lib/
│   ├── binary_sizes.ts    # Binary/WASM size collection and reporting
│   ├── biome.ts           # Biome WASM wrapper (Svelte, TypeScript, CSS)
│   ├── canonical.ts       # Prettier + Svelte parser wrappers
│   ├── compare_cli.ts     # Shared scaffolding for the corpus_compare_* entry points
│   ├── corpus.ts          # DevReposLoader + DirectoryLoader (load/stream; node: builtins)
│   ├── diff.ts            # Line-based diff utilities (LCS algorithm)
│   ├── ffi.ts             # Deno.dlopen bindings (NativeImplementation — Deno native, runtime-specific)
│   ├── napi.ts            # process.dlopen bindings (NapiImplementation — Node/Bun native, runtime-specific)
│   ├── runtime.ts         # Tiny cross-runtime helpers: current_runtime / os / arch normalizers
│   ├── implementations.ts # Implementation registry (branches native FFI vs N-API by runtime)
│   ├── oxc.ts             # OXC native wrappers (oxc-parser + oxfmt)
│   ├── oxc_wasm.ts        # OXC WASM wrapper (oxc-parser via wasm32-wasi; per-runtime wasi entry)
│   ├── report.ts          # Summary report generation
│   ├── types.ts           # Shared type definitions
│   ├── versions.ts        # Version loading from package.json
│   ├── wasm.ts            # WASM module loader (WasmImplementation — deno/nodejs target by runtime)
│   └── divergence/        # Divergence detection module
│       ├── mod.ts         # Main exports
│       ├── safety.ts      # Safety check (differential char-frequency vs prettier)
│       ├── patterns.ts    # Known divergence pattern detectors (with traceability)
│       ├── expected_errors.ts  # Expected-error fixtures (parse-rejection cases)
│       └── validation.ts  # Audit: cross-ref patterns vs conformance_prettier.md
```

## Implementations

Versions read automatically from `package.json` `dependencies` at runtime
(`lib/versions.ts`).

### Updating dependencies

**How resolution works on any machine.** `benches/js/package.json` pins the npm
dep versions (the single source of truth, consumed by both runtimes) and
`package-lock.json` pins their integrity. **Run `deno task bench:install`** to
populate `node_modules` (one installer — `npm install` — plus the force-fetch of
the oxc wasi binding; see [Cross-Runtime](#cross-runtime-deno--node--bun)). Deno reads
that `node_modules` via `"nodeModulesDir": "manual"`; Node reads it directly (the
config shape — no jsr/remote deps, no `deno.lock` — is covered above).
The Rust artifacts the bench builds (`tsv_ffi`, `tsv_napi`, `tsv_wasm`) are pinned
via `Cargo.lock`. Upgrading is always a deliberate, committed act. A plain
`npm install` prunes the oxc wasi binding — re-run `bench:install`.

**Routine refresh** (alternative impls + infra — no fixture impact):

```bash
cd benches/js && npm outdated   # shows current vs latest
# bump the version in benches/js/package.json, then:
deno task bench:install   # re-install at the new pins (+ re-fetch the oxc wasi binding)
deno task smoke           # confirm every impl still loads + formats (32 checks)
deno task bench           # regenerate report.{deno,node,bun}.* + combined report.{json,md}
# commit package.json + package-lock.json + results/report.*
```

These packages are free to bump independently — they're measured against, not
baked into fixtures.

⚠ **The oxc wasm binding is not a regular dep.** It's pure-wasm but its metadata
declares `cpu: wasm32`, so it lives in neither `dependencies` nor
`optionalDependencies` (both break or get pruned). `install_deps.ts` force-fetches
it at the `oxc-parser` version (oxc ships all bindings in lockstep) — so bumping
`oxc-parser` in `package.json` automatically carries it. `binary_sizes.ts` reads
it from `node_modules` (flat, no version dir).

**Canonical baseline is coupled — do NOT bump it as routine.** The five
canonical packages (`prettier`, `svelte`, `acorn`, `@sveltejs/acorn-typescript`,
`prettier-plugin-svelte`) are also pinned, as literals, in
`crates/tsv_debug/src/deno/sidecar.ts` — the sidecar that generates every
fixture's `expected.json` and `output_prettier.svelte`. The two pin sets **must
stay identical**: the bench has to measure against the same parser/formatter
that defines fixture correctness. Bumping any of the five is therefore not a
benchmark refresh — it re-baselines the entire fixture corpus. Do it
deliberately: edit `package.json` and `sidecar.ts` in lockstep (the
`//canonical-sync` note in package.json restates this), run
`deno task fixtures:update`, and review the resulting fixture churn.

### Canonical (JS baseline)

- svelte — Svelte parser (`svelte/compiler`)
- acorn — JS parser base
- @sveltejs/acorn-typescript — TypeScript extension for acorn
- prettier — Code formatter
- prettier-plugin-svelte — Svelte formatting support

`canonical.ts` formats with a `filepath` hint (`file.ts` / `file.svelte` /
`file.css`) so prettier applies the same extension-specific heuristics a real
on-disk file gets — matching how `tsv_debug`'s sidecar invokes prettier. This is
load-bearing, not cosmetic: without it prettier can't tell `.ts` from `.tsx` and
force-adds the JSX-disambiguating trailing comma to single-type-param arrows
(`<T,>`) that a real `.ts` run never emits — which once manufactured ~39 phantom
corpus divergences against `@ryanatkn` code that tsv was formatting correctly.

### Alternative Implementations

- oxc-parser (NAPI) — Fast TypeScript parser; languages: TypeScript, JS
- oxfmt (NAPI) — Fast formatter; languages: TypeScript, JS, CSS, Svelte (experimental)
- biome (WASM) — Formatter/linter; languages: TypeScript, JS, CSS, and Svelte
  (Svelte via biome's experimental HTML-superset support — `html.experimentalFullSupportEnabled`;
  it formats the template **and** the embedded `<script>`/`<style>`, so it's comparable
  work to prettier-plugin-svelte / tsv, just on an experimental path)

### OXC Package Details

**oxc-parser** (version pinned in `package.json`) ships three package types:

- **Main** (`oxc-parser`): JS wrapper with platform detection. Contains `src-js/wasm.js`
  entry point for direct WASM usage. Supports `NAPI_RS_FORCE_WASI` env var to force WASM.
- **Native bindings** (`@oxc-parser/binding-{platform}`): 20 platform-specific `.node` files
  (e.g., `binding-linux-x64-gnu`). Listed as `optionalDependencies` of main package.
- **WASM binding** (`@oxc-parser/binding-wasm32-wasi`): Official WASI build. Also an
  optional dependency of main package — ships alongside native, not as a separate product.
  Depends on `@napi-rs/wasm-runtime` → `@emnapi/runtime`, `@emnapi/core`, `@tybys/wasm-util`.

**oxfmt** (version pinned in `package.json`) ships native bindings only:

- **Main** (`oxfmt`): JS wrapper bundling Prettier internals. Depends on `tinypool`.
- **Native bindings** (`@oxfmt/{platform}`): 8 platform variants. **No WASM variant exists.**
- **Svelte support** is experimental (added in v0.49 via oxc-project/oxc#21700);
  we enable it and let the per-file try/catch + effective-corpus report quantify coverage.

**Deprecated**: `@oxc-parser/wasm` exists on npm but is deprecated. The correct WASM
package is `@oxc-parser/binding-wasm32-wasi`.

**Deno compatibility note**: The WASM binding's default CJS entry uses `node:wasi` which
Deno doesn't support. We import the browser entry point explicitly
(`@oxc-parser/binding-wasm32-wasi/parser.wasi-browser.js`) which uses
`@napi-rs/wasm-runtime` with `fetch()` + `WebAssembly` — works in Deno.

## Error Tracking

Benchmark failures are recorded during the up-front pre-flight pass
(each task runs once per file untimed). The timed loop then iterates
the pre-filtered intersection (or per-impl success set under
`BENCH_MODE=union`), so throws during measurement would be real bugs —
they're allowed to propagate instead of being silently catalogued.

Two surfaces summarize what was skipped:

- **Effective corpus report**: per-benchmark coverage rate (e.g. `⚠ biome 500/660 files (76%)`).
- **Skipped files report**: total counts + per-benchmark skip counts always
  shown. Per-file detail (paths, error messages, failure sets) is opt-in
  via `--verbose` since most universal-tsv failures are unsupported-syntax
  fixtures (SCSS in `.css`, JSX in `.js`, stage-1 proposals, etc.). When
  verbose, entries are sorted ascending by failure-set size so rare /
  impl-specific failures land at the top, and the `Failed in:` line
  collapses to `all tsv variants` when the failure set matches the
  canonical 6-element pattern (`parse|format / native|wasm |
  native-internal|wasm-internal`). All labels use display names
  (`tsv-json`, `acorn-typescript`) rather than internal trackingKeys.

If an implementation fails on many files (e.g. WASM panics corrupting
internal state), the effective corpus report and per-benchmark skip
counts make this immediately visible without needing `--verbose`.

## Binary Size Reporting

Benchmark output includes binary/WASM size comparison across implementations:

- **`tsv`**: native FFI (`.so`/`.dylib`/`.dll`), N-API addon (`.node`), and WASM
  (`.wasm`) from build output. The FFI side ships three rows from one `tsv_ffi` crate
  via its `format`/`parse` features (matching the three WASM rows): the full
  `libtsv_ffi` (`target/release`, both features — the build the perf rows load),
  `tsv format (ffi)` (`target/ffi-format/release`, `--features format`, no convert
  layer — scope-matched to `oxfmt (napi)`), and `tsv parse (ffi)`
  (`target/ffi-parse/release`, `--features parse`, printers dropped — scope-matched to
  `oxc-parser (napi)`). `tsv (napi)` is the N-API addon (`tsv_napi`, the Node/Bun
  native path). Native-kind labels name the binding (`ffi`/`napi`), not just "native",
  so the row's mechanism is unambiguous. `deno task bench` builds all of them; the
  subset rows are omitted if those builds haven't been run.
- **biome**: WASM (`.wasm`) from node_modules
- **oxc-parser**: N-API binding (`.node`) and WASM (`.wasm` from `binding-wasm32-wasi`) from node_modules
- **oxfmt**: N-API binding (`.node`) from node_modules (no WASM variant)

Each row reports **raw on-disk size** plus **gzipped size** (≈ npm-tarball
wire size). Sizes are grouped by kind (WASM vs native) with ratios
relative to `tsv` shown for both raw and gzipped. Gzipped column shows
`—` when `gzip` isn't on PATH (e.g., bare Windows); raw size still
collects fine. `bench:deno:run` needs `--allow-run=git,gzip` for the
subprocess (Node needs no permission flags). gzip runs via `node:child_process`
`execFile` (portable across both runtimes).

Compression mechanism is `gzip -c` (system default level 6), matching
`scripts/patch_npm_package.ts`. Level 6 corresponds to what
`tar | gzip` and most npm publishers produce; the slightly tighter
numbers cited in some perf-doc histories used `gzip -9` and run
~2-3% smaller — both are recorded in `docs/performance.md` for the
WASM binaries.

JSON output (`results/report.<runtime>.json`) gains a per-entry `gzip_bytes:
number | null` field alongside the existing `bytes`.

Combined `oxc-parser+oxfmt (napi)` row sums both raw and gzipped
sizes from the parts. The gzipped sum slightly overstates wire size
because the streams don't share a dictionary, but it matches npm's
two-tarball reality.

Implementation: `lib/binary_sizes.ts`

## Known Issues

- **Corpus SAFETY counts are flaky under `--all` load (two heisenbugs).** A
  `corpus:compare:format --all` run can disagree with a clean, small-scope run — the
  count is **not** reliable from a single `--all` run. Because the safety check
  is differential vs prettier — it iterates the characters _ours_ deviates on and
  uses prettier only as a subtrahend — the two sides fail in **opposite**
  directions:
  1. **FFI buffer marshalling (false positive — fabricates a violation).** Deno's
     `buffer` fast-call path intermittently hands the native `.so` a stale/wrong
     source pointer under memory pressure, so **ours** reads corrupted input and
     genuinely (but spuriously) drops content — a real-looking `content_lost`.
     Hardened in `lib/ffi.ts` via `Deno.UnsafePointer.of` (see the comment there),
     but timing-sensitive. This is what re-flags
     `prettier/tests/format/css/numbers/numbers.css` (a documented `@include`
     divergence, deterministically `known`) as SAFETY under `--all`. **This is the
     only source of spurious SAFETY violations** — only `ours`-side corruption can
     fake a loss.
  2. **Prettier sidecar miss (false _negative_ — masks a violation).** The prettier
     Deno sidecar intermittently returns empty output for a file under load.
     Prettier-side corruption can **never fabricate** a `content_lost`: an empty
     `prettier` inflates `prettier_excess` to the whole source, which only cancels
     `ours`'s deltas, so the file surfaces as a large **unknown diff**, not SAFETY.
     The real danger is the reverse — if `ours` _also_ drops content in that same
     file, the empty prettier subtracts the loss away and hides it. `corpus_compare_format.ts`
     guards this by erroring out when prettier returns empty for non-empty source,
     so the sidecar miss is surfaced rather than silently suppressing a verdict.

  **Triage — never trust one `--all` run for a SAFETY verdict:** (a) re-run the
  single file/dir cleanly (`corpus:compare:format ~/dev/.../that-dir`) — a real loss
  reproduces, a heisenbug doesn't; (b) confirm with the **native CLI** (`tsv
  format <file>` is deterministic) and diff semantic chars vs prettier; (c) for
  "did my change regress?", diff the sorted `.safety[].path` lists before/after
  (a real regression is a _new path_, not a count bump). A change scoped to one
  printer/crate can't lose content in unrelated languages. See `lib/ffi.ts` for
  the Deno-FFI corpus heisenbug.
- **Parse benchmark overhead**: JSON materialization, not parsing, dominates
  the `-json` rows (see `results/report.<runtime>.md` for the current per-language
  ratios). Use `tsv-internal` for raw parse speed. Both the native and WASM rows go through
  `convert_ast_json_string`, which skips the intermediate `serde_json::Value`
  when eligible (per-language eligibility:
  [docs/architecture.md §Closed Scope, Open Convention](../../docs/architecture.md#closed-scope-open-convention)).
  They differ only at the boundary: native crosses via FFI copy +
  `JSON.parse` in JS; WASM decodes the string across the boundary and runs
  the engine's `JSON.parse` from Rust via `js_sys` (measurably faster than a
  `serde_wasm_bindgen`-built object graph). The Rust-side sub-step split (convert / to_value /
  translate / to_string) plus whole-call value-baseline-vs-shipped timings are
  measured by `cargo run --release -p tsv_debug -- json_profile <paths>` —
  `wasm_json_probe.ts` covers the end-to-end view including the JS boundary.
- **TypeScript canonical parser**: acorn-typescript fails on some modern syntax
  (files skipped) — and the reverse, files tsv fails that acorn accepts, is a
  known parse gap.
- **prettier-plugin-svelte verbatim fallback**: when the embedded formatter
  throws on any construct in a `<script>` block (e.g. `@(a?.b)()` decorators
  crash prettier's typescript parser), the plugin emits the whole block
  verbatim — a corpus diff on such a file is prettier's error fallback, not a
  real style divergence. See ../../docs/conformance_prettier.md §Tooling for
  the triage procedure.

## Diagnostic scripts (ad-hoc, not wired into `deno task`)

These live under `diagnostics/`. The parser-analysis ones (`comment_dup_scan`,
`acorn_dup_fuzz`) import the canonical parser (`acorn`, `svelte/compiler`) by bare
specifier, so pass `--config benches/js/deno.json` to resolve them from
`node_modules` (`nodeModulesDir: manual`); all run from the repo root
(corpus/artifact paths are CWD-relative).

- `diagnostics/skip_triage.ts` — parse every corpus file with tsv + the canonical parser,
  bucket into tsv-fails-canonical-ok / canonical-fails-tsv-ok / both-fail.
  Run:
  `deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys benches/js/diagnostics/skip_triage.ts`
- `diagnostics/test262_compare.ts` — test262 differential conformance, tsv vs oxc-parser. Consumes
  the manifest from `tsv_debug test262 --emit-manifest <file>` (tsv's graded strict subset + each
  test's expected/tsv verdict + `module` flag), runs oxc over the same files at each test's goal
  (mirroring tsv: `module`-flagged → module, else strict script), and buckets the agreement —
  surfacing positive **tsv real-bug candidates** (tsv rejects,
  oxc accepts) and negative **early-error gaps** (oxc rejects, tsv accepts). On-demand triage, not a
  CI gate; numbers move with the pinned oxc version. No biome (its js-api has no parser to grade).
  See `docs/conformance_test262.md` §Differential. Run from the repo root:
  `cargo run -p tsv_debug test262 --emit-manifest /tmp/t262.json && deno run --allow-read --allow-env --allow-ffi --allow-net --allow-sys --config benches/js/deno.json benches/js/diagnostics/test262_compare.ts --manifest /tmp/t262.json`
- `diagnostics/comment_dup_scan.ts` — comment-duplication fixture-corpus completeness guard.
  Walks all fixtures with two oracles (live `svelte/compiler` parse + committed expected
  JSON), flagging any comment span emitted ≥2× within one array (the acorn backtrack-reparse
  signature tsv corrects to single). RED buckets must stay empty. Re-run after touching the
  comment-convert layer or bumping `@sveltejs/acorn-typescript`.
  Run:
  `deno run --allow-read --allow-env --allow-net --allow-sys --config benches/js/deno.json benches/js/diagnostics/comment_dup_scan.ts`
- `diagnostics/acorn_dup_fuzz.ts` — fuzzes a comment into every position of
  acorn-typescript's own ~200 construct test inputs and flags any `onComment` double-fire;
  the broadest net for an un-enumerated duplicating construct, and the upstream-fix
  validation harness (a correct A+B patch drops the count to 0). Default reads
  `~/dev/acorn-typescript-fork/test`; pass a path to override.
  Run:
  `deno run --allow-read --allow-env --allow-net --allow-sys --config benches/js/deno.json benches/js/diagnostics/acorn_dup_fuzz.ts`
- `diagnostics/wasm_json_probe.ts` — split parse cost into pure-parse vs materialization for
  native + WASM, isolating JS-side `JSON.parse`.
- `diagnostics/wasm_format_probe.ts` — measure WASM **format** wall-time at the resolution
  the full bench folds into noise (single-digit-% changes). A/Bs two WASM builds
  (copy `pkg/all/deno` aside before editing, rebuild, pass `--baseline
  …/tsv_wasm.js`) with the ../../docs/performance.md §5 paired discipline:
  interleaved pairs, the A/A noise floor measured in the same run, `net = A/B ÷
  floor`, and a corpus byte-identity gate that aborts if the builds format
  differently. Omit `--baseline` for an A/A-only run (floor + current-build
  baseline number, no comparison). See the module doc for the full workflow.
- **wasm-opt**: runs with explicit feature flags in `crates/tsv_wasm/Cargo.toml` — Rust 2024's bulk-memory and nontrapping-float-to-int ops are passed by name to wasm-opt v117, giving ~−2% gzipped on the WASM bundle.
- **oxfmt × Deno timer interaction (workaround in place)**: once `oxfmt.format` runs once,
  Deno's timer wheel processes exactly one further `setTimeout` callback and then stalls all
  subsequent timers indefinitely. Repro:
  `await import('oxfmt').then((m) => m.format('file.ts', 'x=1', {useTabs:true}))` followed by
  two `new Promise((r) => setTimeout(r, 50))` — first resolves, second never does.
  Independent of oxfmt version (reproduced with 0.28.0 and 0.50.0) so the regression is on
  the Deno / napi-rs side. In `bench.ts` oxfmt is invoked per-iteration during the `format/*` measurement
  loops; the leak shows up at the next inter-task `await wait(cooldown_ms)`, which never
  fires. Workaround: `cooldown_ms: 0` in `run_benchmark_group`'s `Benchmark` config — runs
  tasks back-to-back without the cooldown await. Async measurement loops (`prettier`,
  `oxfmt` itself) are unaffected because their per-iteration awaits resolve via microtasks,
  not timers.
