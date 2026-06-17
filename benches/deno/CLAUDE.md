# Benchmarking & Corpus Comparison Infrastructure

> Compare tsv formatting against Prettier on real codebases

Uses [@fuzdev/fuz_util](https://github.com/fuzdev/fuz_util) benchmarking library for statistical analysis.

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

# Deno test suite — every *_test.ts under lib/, gated by `deno task check`. Covers
# the divergence detectors (pattern positive/negative overmatch-rejection cases,
# safety differential cases, and a behavioral fixture-coverage audit that drives
# each detector against its own committed fixtures — input == ours, output_prettier
# == prettier — failing if a pattern stops claiming a hunk in a fixture it lists)
# plus canonical_test.ts (asserts the prettier baseline formats with a filepath, so
# `.ts` single-type-param arrows stay `<T>` and `.svelte` ones get `<T,>`).
deno task test:deno

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

Results are saved to `benches/deno/results/` as timestamped files plus
`report.json` and `report.md` (committed to git). To publish benchmarks
to tsv.fuz.dev, run `npm run update-benchmarks` in ~/dev/tsv.fuz.dev.

The committed `report.json` (baseline `version: 4`) carries, beyond timing
stats: per-language `corpus` totals, `versions`, and `binary_sizes` (each with
`gzip_bytes`). Each `entries[]` row adds `files_processed`/`files_total`
(per-impl preflight coverage — the `Coverage:` line) and `files_iterated` (the
timed set — the `Files (intersection):` count). A top-level `suppressed_noise`
map records silenced third-party stderr crashes as `{pattern: count}`.
`report.md` renders coverage/iterated as prose; the per-entry numbers and
`suppressed_noise` are JSON-only.

```bash
# Run benchmarks (builds ffi + wasm:deno automatically)
deno task bench

# Run without rebuilding (if already built) — guarded against stale artifacts,
# see "Artifact Freshness Guard" below
deno task bench:run

# Output formats
deno task bench:run -- --json          # Output as JSON (for CI/tooling)
deno task bench:run -- --markdown      # Output as Markdown tables
deno task bench:run -- --verbose       # Include per-file skip detail (paths + errors)
deno task bench:run -- --save-report   # Force-overwrite the committed results/report.{json,md}
                                       # on a limited/filtered run (full runs overwrite it anyway;
                                       # the timestamped results/<ts>_<commit>.{json,md} pair is always written)

# Baseline regression detection
deno task bench:run -- --save-baseline     # Save current results as baseline
deno task bench:run -- --compare-baseline  # Compare against saved baseline

# Wipe local-only bench state (gitignored): baseline.json + timestamped
# results pairs. Preserves the committed `results/report.{json,md}` because
# the glob is anchored on a leading digit (timestamped files start with a year).
deno task bench:clean

# Environment variables
BENCH_LIMIT=5 deno task bench:run           # Limit files per language (default: all)
BENCH_FILTER=zzz deno task bench:run        # Filter by path pattern (default: none)
BENCH_DURATION=10000 deno task bench:run    # Duration per benchmark in ms (default: 5000)
BENCH_WARMUP=10 deno task bench:run         # Set warmup iterations (default: 3)
BENCH_MODE=union deno task bench:run        # Per-impl iteration (default: intersection)
BENCH_STALE_OK=1 deno task bench:run        # Override the freshness guard (see below; default: off)
```

## Artifact Freshness Guard

The `:run` variants (`bench:run`, `corpus:compare:format:run`) skip the rebuild so you
can iterate on the harness without paying the wasm-pack cost — at the risk of
silently measuring a binary older than current source (a CSS run once reported
`146/183` against a stale `.so` that should have been `155/183`).
`lib/check_artifact_freshness.ts` guards against this: before a `:run` touches
the executed artifacts (the profile-aware FFI library + the `pkg/all/deno` WASM
bundle), it compares their mtimes against the crate sources feeding them and
**aborts (exit 1)** if any is stale, or missing. The build-first tasks (`bench`,
`corpus:compare:format`) rebuild first, so they pass the guard for free.

**Escape hatch:** `BENCH_STALE_OK=1` downgrades a _stale_ artifact to a `⚠`
warning and proceeds (a _missing_ one stays fatal). See the module doc for the
full rationale (e.g. why stale is a hard error, not a warning).

```bash
# After editing a crate, the fast/correct paths:
deno task bench                         # rebuilds, then runs — always fresh
deno task build:ffi && deno task bench:run   # rebuild just what you changed, then :run
BENCH_STALE_OK=1 deno task bench:run    # deliberately measure the current (stale) binary
```

## Smoke Test

`deno task smoke` runs a fast sanity check on every formatter and parser
(trivial fixed inputs, non-throwing + non-empty + idempotent). Exits non-zero
on any failure. Use it to catch "implementation totally broken" before
running the full bench. `corpus_compare_format` is still the real correctness gate.

## Fairness Caveats

Things the published numbers measure that aren't quite what they look like:

- **Single-threaded, per-file (universal)** — the harness times one file at a
  time, sequentially (`await`ed in order, no `Promise.all` over files), so the
  numbers are per-file single-core latency/throughput, not multi-core batch
  throughput. Per-file compute is single-threaded for every impl: tsv (FFI +
  WASM) pulls in no threading crate (`rayon`/`num_cpus`/`threadpool`/`crossbeam`
  absent from every `Cargo.toml`); prettier, `svelte/compiler`, and
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
  skip — swamped by their actual format time, but real. (c) Task return values
  are discarded uniformly for all impls; the FFI/WASM/async boundaries block
  dead-code elimination, so no impl's work is optimized away.
- **`tsv_wasm` is measured on the full build.** The WASM bench loads
  `pkg/all/deno` (the default both-features artifact, ~2.8 MB — what
  `@fuzdev/tsv_wasm` ships) for _both_ parse and format, while subset
  consumers ship the smaller `@fuzdev/tsv_format_wasm` (~2.2 MB, no convert
  layer) or `@fuzdev/tsv_parse_wasm` (~1.7 MB, no printers). The Binary
  Sizes table lists all three; the throughput rows reflect the full build.
  The native `tsv` row is the same story: the perf row loads the full
  `libtsv_ffi`, while the Binary Sizes table also lists `tsv format (native)`
  and `tsv parse (native)` subset builds (no perf rows of their own — they
  exist only to size scope-matched against `oxfmt` and `oxc-parser`).
- **Intersection-corpus iteration (default)** — within each group, every
  impl is timed on the same all-N intersection: the set of files every impl
  in the group successfully processed during pre-flight. Ratios within a
  group are then apples-to-apples (`ops_per_sec(A) / ops_per_sec(B)` reads
  as "A is N× faster than B on the files they both handle"). Trade-off: one
  noisy impl shrinks the corpus for the whole group — e.g. if `biome-wasm`
  skips 60% of CSS files, `tsv`/`prettier`/`oxfmt` are timed on only the
  remaining 40%. The Coverage section in `report.md` still discloses each
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
  - A `report.md` generated with the hook on has a narrower
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
benches/deno/
├── bench.ts               # Benchmark entry point
├── smoke.ts               # Smoke test for formatters and parsers
├── corpus_compare_format.ts  # Formatting comparison vs prettier (entry point)
├── corpus_compare_parse.ts   # Parse/AST comparison vs canonical parsers (entry point)
├── divergence_audit.ts    # Divergence audit entry point
├── diagnostics/           # ad-hoc diagnostic scripts (not wired into `deno task` — see §Diagnostic scripts)
│   ├── skip_triage.ts        # parse-gap triage (tsv vs canonical)
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
│   ├── corpus.ts          # DevReposLoader + DirectoryLoader (load/stream)
│   ├── diff.ts            # Line-based diff utilities (LCS algorithm)
│   ├── ffi.ts             # Deno.dlopen bindings (NativeImplementation)
│   ├── implementations.ts # Implementation registry and task generation
│   ├── oxc.ts             # OXC native wrappers (oxc-parser + oxfmt)
│   ├── oxc_wasm.ts        # OXC WASM wrapper (oxc-parser via wasm32-wasi)
│   ├── report.ts          # Summary report generation
│   ├── types.ts           # Shared type definitions
│   ├── versions.ts        # Version loading from deno.json
│   ├── wasm.ts            # WASM module loader (WasmImplementation)
│   └── divergence/        # Divergence detection module
│       ├── mod.ts         # Main exports
│       ├── safety.ts      # Safety check (differential char-frequency vs prettier)
│       ├── patterns.ts    # Known divergence pattern detectors (with traceability)
│       └── validation.ts  # Audit: cross-ref patterns vs conformance_prettier.md
```

## Implementations

Versions read automatically from `deno.json` import map at runtime.

### Updating dependencies

**How resolution works on any machine.** The `benches/deno/deno.json` import
map pins exact versions and `benches/deno/deno.lock` pins their integrity
hashes. `deno task bench` (and `smoke`, `corpus:compare:format`, `test:deno`) fetch
**exactly** those versions from npm/jsr on first run and cache them — there is
no `npm install`, no `node_modules`, and no auto-upgrade. A fresh checkout on
another machine reproduces the pinned set byte-for-byte; it never pulls newer
releases on its own. The Rust artifacts the bench builds (`tsv_ffi`,
`tsv_wasm`) are pinned the same way via `Cargo.lock`. Upgrading is always a
deliberate, committed act.

**Routine refresh** (alternative impls + infra — no fixture impact):

```bash
deno outdated   # run in benches/deno/ — shows current vs latest
deno update --latest oxc-parser oxfmt @biomejs/js-api @biomejs/wasm-bundler @fuzdev/fuz_util zod @std/fs
deno task smoke # confirm every impl still loads + formats (32 checks)
deno task bench # regenerate results/report.{json,md}
# commit deno.json + deno.lock + results/report.*
```

These packages are free to bump independently — they're measured against, not
baked into fixtures.

⚠ **The oxc wasm binding has a subpath import entry**
(`@oxc-parser/binding-wasm32-wasi/.../parser.wasi-browser.js`) that
`deno update` does **not** bump — set it by hand to match `oxc-parser`'s
version and re-run `deno install`. `binary_sizes.ts` keys the wasm size off the
oxc version, so a mismatch silently drops the WASM size from the report.

**Canonical baseline is coupled — do NOT bump it as routine.** The five
canonical packages (`prettier`, `svelte`, `acorn`, `@sveltejs/acorn-typescript`,
`prettier-plugin-svelte`) are also pinned, as literals, in
`crates/tsv_debug/src/deno/sidecar.ts` — the sidecar that generates every
fixture's `expected.json` and `output_prettier.svelte`. The two pin sets **must
stay identical**: the bench has to measure against the same parser/formatter
that defines fixture correctness. Bumping any of the five is therefore not a
benchmark refresh — it re-baselines the entire fixture corpus. Do it
deliberately: edit `deno.json` and `sidecar.ts` in lockstep, run
`deno task fixtures:update`, and review the resulting fixture churn.

### Canonical (JS baseline)

| Package                    | Purpose                           |
| -------------------------- | --------------------------------- |
| svelte                     | Svelte parser (`svelte/compiler`) |
| acorn                      | JS parser base                    |
| @sveltejs/acorn-typescript | TypeScript extension for acorn    |
| prettier                   | Code formatter                    |
| prettier-plugin-svelte     | Svelte formatting support         |

`canonical.ts` formats with a `filepath` hint (`file.ts` / `file.svelte` /
`file.css`) so prettier applies the same extension-specific heuristics a real
on-disk file gets — matching how `tsv_debug`'s sidecar invokes prettier. This is
load-bearing, not cosmetic: without it prettier can't tell `.ts` from `.tsx` and
force-adds the JSX-disambiguating trailing comma to single-type-param arrows
(`<T,>`) that a real `.ts` run never emits — which once manufactured ~39 phantom
corpus divergences against `@ryanatkn` code that tsv was formatting correctly.

### Alternative Implementations

| Package    | Binding | Purpose                | Languages                                  |
| ---------- | ------- | ---------------------- | ------------------------------------------ |
| oxc-parser | NAPI    | Fast TypeScript parser | TypeScript, JS                             |
| oxfmt      | NAPI    | Fast formatter         | TypeScript, JS, CSS, Svelte (experimental) |
| biome      | WASM    | Formatter/linter       | Svelte, TypeScript, JS, CSS                |

### OXC Package Details

**oxc-parser** (version pinned in `deno.json`) ships three package types:

- **Main** (`oxc-parser`): JS wrapper with platform detection. Contains `src-js/wasm.js`
  entry point for direct WASM usage. Supports `NAPI_RS_FORCE_WASI` env var to force WASM.
- **Native bindings** (`@oxc-parser/binding-{platform}`): 20 platform-specific `.node` files
  (e.g., `binding-linux-x64-gnu`). Listed as `optionalDependencies` of main package.
- **WASM binding** (`@oxc-parser/binding-wasm32-wasi`): Official WASI build. Also an
  optional dependency of main package — ships alongside native, not as a separate product.
  Depends on `@napi-rs/wasm-runtime` → `@emnapi/runtime`, `@emnapi/core`, `@tybys/wasm-util`.

**oxfmt** (version pinned in `deno.json`) ships native bindings only:

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

- **`tsv`**: native FFI (`.so`/`.dylib`/`.dll`) and WASM (`.wasm`) from build output.
  Native ships three rows from one `tsv_ffi` crate via its `format`/`parse` features
  (matching the three WASM rows): the full `libtsv_ffi` (`target/release`, both
  features — the build the perf rows load), `tsv format (native)`
  (`target/ffi-format/release`, `--features format`, no convert layer — scope-matched
  to `oxfmt (native)`), and `tsv parse (native)` (`target/ffi-parse/release`,
  `--features parse`, printers dropped — scope-matched to `oxc-parser (native)`).
  `deno task bench` builds all three; the subset rows are omitted if those builds
  haven't been run.
- **biome**: WASM (`.wasm`) from Deno npm cache
- **oxc-parser**: native binding (`.node`) and WASM (`.wasm` from `binding-wasm32-wasi`) from Deno npm cache
- **oxfmt**: native binding (`.node`) from Deno npm cache (no WASM variant)

Each row reports **raw on-disk size** plus **gzipped size** (≈ npm-tarball
wire size). Sizes are grouped by kind (WASM vs native) with ratios
relative to `tsv` shown for both raw and gzipped. Gzipped column shows
`—` when `gzip` isn't on PATH (e.g., bare Windows); raw size still
collects fine. `bench:run` needs `--allow-run=git,gzip` for the
subprocess.

Compression mechanism is `gzip -c` (system default level 6), matching
`scripts/patch_npm_package.ts`. Level 6 corresponds to what
`tar | gzip` and most npm publishers produce; the slightly tighter
numbers cited in some perf-doc histories used `gzip -9` and run
~2-3% smaller — both are recorded in `docs/performance.md` for the
WASM binaries.

JSON output (`results/report.json`) gains a per-entry `gzip_bytes:
number | null` field alongside the existing `bytes`.

Combined `oxc-parser+oxfmt (native)` row sums both raw and gzipped
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
  the `-json` rows (see `results/report.md` for the current per-language
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
`acorn_dup_fuzz`) need the canonical-parser import map, so pass `--config
benches/deno/deno.json`; all run from the repo root (corpus/artifact paths are
CWD-relative).

- `diagnostics/skip_triage.ts` — parse every corpus file with tsv + the canonical parser,
  bucket into tsv-fails-canonical-ok / canonical-fails-tsv-ok / both-fail.
  Run:
  `deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys benches/deno/diagnostics/skip_triage.ts`
- `diagnostics/comment_dup_scan.ts` — comment-duplication fixture-corpus completeness guard.
  Walks all fixtures with two oracles (live `svelte/compiler` parse + committed expected
  JSON), flagging any comment span emitted ≥2× within one array (the acorn backtrack-reparse
  signature tsv corrects to single). RED buckets must stay empty. Re-run after touching the
  comment-convert layer or bumping `@sveltejs/acorn-typescript`.
  Run:
  `deno run --allow-read --allow-env --allow-net --allow-sys --config benches/deno/deno.json benches/deno/diagnostics/comment_dup_scan.ts`
- `diagnostics/acorn_dup_fuzz.ts` — fuzzes a comment into every position of
  acorn-typescript's own ~200 construct test inputs and flags any `onComment` double-fire;
  the broadest net for an un-enumerated duplicating construct, and the upstream-fix
  validation harness (a correct A+B patch drops the count to 0). Default reads
  `~/dev/acorn-typescript-fork/test`; pass a path to override. See grimoire
  `lore/tsv/TODO_ACORN_COMMENT_DUP.md`.
  Run:
  `deno run --allow-read --allow-env --allow-net --allow-sys --config benches/deno/deno.json benches/deno/diagnostics/acorn_dup_fuzz.ts`
- `diagnostics/wasm_json_probe.ts` — split parse cost into pure-parse vs materialization for
  native + WASM, isolating JS-side `JSON.parse`.
- `diagnostics/wasm_format_probe.ts` — measure WASM **format** wall-time at the resolution
  the full bench folds into noise (single-digit-% changes). A/Bs two WASM builds
  (copy `pkg/all/deno` aside before editing, rebuild, pass `--baseline
  …/tsv_wasm.js`) with the ../../docs/performance.md §5 paired discipline:
  interleaved pairs, the A/A noise floor measured in the same run, `net = A/B ÷
  floor`, and a corpus byte-identity gate that aborts if the builds format
  differently. See the module doc for the full workflow.
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
