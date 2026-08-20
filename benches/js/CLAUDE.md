# Benchmarking & Corpus Comparison Infrastructure

> The JS harness: benchmarks, corpus comparison vs Prettier, and the
> external-oracle conformance gates.

Uses [@fuzdev/fuz_util](https://github.com/fuzdev/fuz_util)'s benchmarking library
for statistical analysis.

**Directory note:** this is the **runtime-neutral** JS harness — named `js` (not
`deno`) because the same code runs under Deno, Node, and Bun (see
[Cross-Runtime](#cross-runtime-deno--node--bun)). The `corpus_compare_*` and
`diagnostics/` entries stay Deno-idiomatic; `smoke` is portable across all three.

**Companion docs** — this file is the operational surface (what to run, what it
grades); the reference halves live in `docs/`:

| Doc | Covers |
| --- | --- |
| [../../docs/benchmarks.md](../../docs/benchmarks.md) | Fairness caveats, the implementation catalog, binary sizes, the dependency + canonical-oracle-pin ritual |
| [../../docs/gate_counts.md](../../docs/gate_counts.md) | The pinned counts every graded gate and harvest enforces |
| [../../docs/audits.md](../../docs/audits.md) | The standing audit gates (what each proves, blind spots, where it gates) |
| [../../docs/divergence_detector.md](../../docs/divergence_detector.md) | Divergence-pattern detection internals |
| [../../docs/workflow_corpus.md](../../docs/workflow_corpus.md) | The corpus-driven conformance workflow (triage → fixture → fix) |

## Gate map

> Which check runs which corpus/oracle, and when. `deno task check` needs only
> this repo — its one sibling-checkout leg (`roundtrip:audit:prettier`) widens
> onto `../prettier` when present and warn-skips when not, so a bare clone still
> passes. Every other gate *requires* sibling checkouts (`../svelte`,
> `../acorn-typescript`, `../typescript`, `../prettier`, `../test262`, `../wpt`),
> so they run at dev/release cadence and CI runs only the committed-tree tier.

| Gate | Composition | Corpus / oracle | Cadence |
| --- | --- | --- | --- |
| **`deno task check`** | `cargo fmt --check` · `format:audit` · `pins:audit` · `docs:audit` · `typecheck` · `typecheck:scripts` · `conformance:audit` · `conformance:audit:compiler` · `variants:audit` · `scan:audit` · `fanout:audit` · `roundtrip:audit` · `roundtrip:audit:prettier` · `canonicalize:audit` · `binding:audit` · `authoring:audit` · `razor:audit` · `fuzz:audit` · `test:deno` · `cargo test` (incl. fixtures) · `test:audits` · `swallow:audit` · `comments:audit` · `gaps:audit` · `blanks:audit` · `fabrication:audit` · `census:audit` · `width:audit` · `ignore:audit` · `check:ast-types` · `clippy` | **committed tree only** — `tests/fixtures` + pure-Rust/Deno audits, no external oracle — save `roundtrip:audit:prettier`, which gates the pinned `../prettier` format suites when that checkout is present (a loud skip when not; ~0.1 s) | every commit; the CI `check` job |
| **`deno task conformance:all`** | `pins:audit:checkouts` + `compile:fixtures:validate` preflights, then `conformance` (one process, five FFI legs: `svelte-fixtures` · `ts-fixtures` · `ts-repo` · `corpus:compare:parse --all` · `corpus:compare:format --all`, plus `render:audit` as its one subprocess leg) **+** `conformance:test262` (pure Rust) | `../svelte`, `../acorn-typescript`, `../typescript` (tsc baselines), `../prettier`, `../test262`; the **`gates`** corpus view (~6,200) | release; `scripts/publish.ts` **Step 3b** |
| **`deno task bench` / `bench:conformance`** | perf throughput ×3 runtimes + compose; parse-coverage report | **`perf`** view (~3,200; 100%-coverage invariant) / **`conformance`** view (fixtures + wpt/test262 harvests; coverage-only + node-only) | dev / release cadence; feeds tsv.fuz.dev |
| **`deno task idempotency:sweep`** | `tsv_debug fuzz --iterations 0` over the corpus dirs — F1 (`format(format(x)) == format(x)`) + no-panic + structural reparse on every file **as authored** | **`perf`** view (real code; absent checkouts skipped with a warning) | after a printer change; conformance cadence |
| **`deno task audit:corpus`** | the pure-Rust content-loss / robustness suite over **real code**: `roundtrip_audit --gate` · `comment_audit` · `swallow_audit` · `binding_audit --gate` (real code gating; prettier suites report-only) · `authoring_audit` · `census_audit` · `fabrication_audit` (both strict-zero off their default corpus) · `fuzz --iterations 0`. `width_audit` is NOT a leg — it has no zero to grade (../../docs/audits.md §The Corpus Bundle) | **`perf`** view + the pinned `../prettier` format suites (absent dev repos skipped with a warning; floor = `../svelte` src) | release; `scripts/publish.ts` **Step 3c**; conformance cadence |
| **`deno task render:audit <paths>`** | `render_audit --gate` — per `.svelte` file, does `tsv format` change what it RENDERS? Compares the browser-visible render key of the source vs of `format(source)`. The corpus-scale arm of the fixture **R** rules. **Needs the Deno sidecar** (`svelte compile`), so it is deliberately not a leg of the pure-Rust `audit:corpus` — it rides `conformance` instead | standalone: any `.svelte` corpus, given explicitly. As a `conformance` leg: the version-pinned `framework` + `suite` checkouts, so a live working tree can't move a release verdict | release (in `conformance`); standalone after a printer change |

**JS parser (test262) IS release-gated** — `conformance:test262` (`tsv_debug
test262 --gate`) gates the exact test262 **positive-parse** count
(`POSITIVE_PASSED_PIN` in the command); the ~2.5k negatives are the deferred
early-error frontier (reported, not gated). **Only CSS-WPT grading (`../wpt`) stays
manual** — its frontier is deferred §5.5 error-recovery, and real-CSS regressions
are already partly caught by `corpus:compare:parse` (CSS AST vs `parseCss`).

**Preflight.** `deno task doctor` checks the whole chain (runtimes, canonical pins,
checkout alignment, oracle presence, corpus entries, build artifacts) ahead of time.
Publish re-probes per step — Step 3b's posture is under [§Pre-release
aggregate](#pre-release-aggregate--conformance--conformanceall); Step 3c
(`audit:corpus`) re-probes `../svelte` src (its reproducible floor) the same way, and
the audit itself warn-skips any absent dev-repo checkout. CI's `check` job runs `deno task check`
alone — it has no sibling checkouts, so of the pinned counts only the committed-tree
ones (`fixtures_validate`, `swallow_audit`) execute there.

**`deno task check` cannot prove real-code robustness.** `tests/fixtures` is
format-stable by construction, so it never exercises a content-loss / panic / reflow
bug on real source. The extension-robustness bar is therefore two release-cadence
gates over real code: **Step 3b**'s `corpus:compare:format --all` SAFETY (content
loss vs prettier — needs the FFI + prettier sidecar) and **Step 3c**'s
`audit:corpus` (the pure-Rust half: reparse-corruption, dropped/double comments,
`//` swallows, comment re-binding, boundary-whitespace + F1 idempotency,
whole-comment conservation, blank fabrication, no-panic). Every content-loss /
non-idempotency bug this release cycle was found by one of these, never by `check`.

`roundtrip:audit:prettier` narrows that gap without closing it: it puts one
non-format-stable corpus inside `check`, which is enough to catch a
valid→unreparseable regression at the branch (it would have caught the `let`
statement-head paren strip), but the prettier suites are hand-written edge cases —
no app code, no real Svelte components, and only the reparse question. The
release-cadence gates above stay the bar for real-code robustness.

Corpus **views** are defined in [§Corpus](#corpus); the pinned counts are
[../../docs/gate_counts.md](../../docs/gate_counts.md).

## Cross-Runtime (Deno + Node + Bun)

The bench runs under all three from one shared codebase. The motivation: a
single-runtime bench can silently fold a runtime-specific effect into an engine
number — the concrete case being the Deno-FFI fast-call memory sensitivity that
mismeasured the native path (see [§Known Issues](#known-issues)). A per-runtime
delta on the same row is the detector.

**Design:**

- **Runtime-labeled sibling reports.** Each runtime writes its own
  `results/report.<runtime>.{json,md}` (+ a timestamped `…_<commit>.<runtime>.*`
  pair), same schema, never merged. `deno task bench:compose` (run at the end of
  `bench:perf`) folds the siblings into the combined `results/report.{json,md}`
  (`compose_reports.ts`; a per-runtime delta on a row is the
  headline); tsv.fuz.dev consumes it plus `report.node.json` and
  `report.conformance.node.json`. The composer records per-source provenance (runtime, commit,
  timestamp, tsv version, machine — in the JSON `sources[]` and the md header) and
  flags loudly (md banner + stderr + a JSON field): **mixed vintages**
  (`mixed_vintage`) when siblings come from different commits/versions — it folds
  whatever exists, so a fresh `report.deno.*` beside a stale `report.node.*` would
  otherwise read as a runtime effect; **mixed machines** (`mixed_machine`) when the
  siblings' hardware identity disagrees, since cross-runtime ratios are only
  meaningful on same-box siblings; **within-noise** deltas (`within_noise`), the
  per-runtime cells whose difference is smaller than the combined cv of the two
  means they divide — this report's whole subject is those deltas, and a ratio
  inherits both means' noise while printing neither, so the cells that are NOT a
  runtime effect are named (a reading aid, not a significance test — that is
  `benchmark_baseline_compare`'s Welch job, on a run the composer never sees); any
  row whose per-runtime intersections
  differ (`⚠ files a/b/c`) — each runtime times the files *its* impls passed
  preflight on, so unequal counts mean a sliver of the ratio is file-set, not
  runtime; and **partially measured** rows (`partial_rows`), which one sibling
  measured and another doesn't carry at all with no recorded load failure to
  explain the gap — a bare `—` cell otherwise reads identically to an impl that
  couldn't load, so a row added since a sibling was last run is named rather than
  left to the vintage banner. A runtime whose sibling predates the `unavailable`
  field is skipped there rather than accused: with nothing recorded, an absent row
  can't be told from an unloadable impl. `within_noise` skips on its own precondition
  — a row needs ten cleaned timings a side, and prints `n` for the ones it does call.
  The bench floors iterations at 5 (7 on the slow tier) and drives the rest from
  `duration_ms`, so sample count spans two orders of magnitude inside one table and
  17 of 44 rows per runtime sit under ten; this test consumes cv in the direction
  where an UNDERestimate is expensive, since it would report a real runtime
  difference as "no difference" — the one verdict a reader cannot check against the
  table. The conformance surface writes its own
  `report.conformance.node.*`, outside the compose glob.
- **One bench body, runtime-detected.** `bench.ts` detects the runtime
  (`lib/runtime.ts` `current_runtime()`) and selects the runtime-specific artifacts.
  No forked entry; `bench:node:run` is literally `node benches/js/bench.ts`.
- **Portable shared modules.** Shared/entry modules use `node:` builtins (Deno
  supports them) + `@fuzdev/fuz_util` helpers (`fs_search`, `fs_exists`,
  `spawn_out`, `to_file_path`) — **no `Deno.*`, no `@std/*`**. The only genuinely
  runtime-specific files are the native loader (`ffi.ts` `Deno.dlopen` vs `napi.ts`
  `process.dlopen`) and the WASM target the loader picks. The Deno-only entry points
  (`corpus_compare_*`, `diagnostics/*`) stay Deno-idiomatic. The `deno test` suite is
  the dependency-free divergence detectors (`node:assert` + relative imports).
- **The native row differs by runtime, fairly.** Deno → FFI (`tsv_ffi`, via
  `Deno.dlopen`); Node/Bun → N-API (`tsv_napi`, via `process.dlopen`). Same engine,
  same per-thread arena reuse, different binding boundary.
- **The WASM row uses each runtime's own wasm-pack target bundle** (same
  `tsv_wasm_bg.wasm`, different JS glue) — Deno the `deno` target, Node the `nodejs`
  target — both with the full export set incl. `parse_internal_*`. The shipped web
  bundle is deliberately not used (it curates `parse_internal_*` out).

**Dependencies: `package.json` is the source of truth.** Both runtimes consume one
`node_modules`. Deno reads it via `"nodeModulesDir": "manual"` in `deno.json`; Node
reads it directly. There are no jsr or remote deps — everything imports npm packages
by bare specifier or uses `node:` builtins, so `deno.json` carries only
`nodeModulesDir: manual` + `lock: false` (npm integrity is `package-lock.json`'s
job). `@types/node` is a types-only devDependency so `node:` builtins type-check
under `deno check`.

**Install with `deno task bench:install`** (`install_deps.ts`), always — a plain
`npm install` **prunes the `@oxc-parser/binding-wasm32-wasi` binding**, which the
installer force-fetches back (why: ../../docs/benchmarks.md §Updating
dependencies). Re-run after a dep bump or a stray `npm install`. Every harness
entry point preflights `node_modules` via `lib/check_node_modules.ts`: missing is
fatal with the installer hint, and **stale** — any exactly-pinned dep whose
installed `version` differs from the pin (or is absent) — is fatal too, listing
each mismatch as `name: pinned X, installed Y` (`BENCH_STALE_OK=1` downgrades stale
to a warning), so a run can't silently measure old installed versions under new
labels. The comparison is against the installed **versions**, not against
`package.json`'s mtime: a mtime proxy tripped on any edit to the file (a comment, a
branch switch restamping it) and missed an install that ran without taking. Range
pins (`^4.4.3`) are skipped — a range constrains rather than fixes, so any
satisfying version is legitimate. One package is graded off-list, because
`dependencies` is not where it lives: the wasi binding above, whose installed
version must equal the `oxc-parser` pin it was force-fetched at (the reports label
its row with that pin, so a version skew there is exactly the mislabeling this check
prevents). Its **absence** is not graded — nothing is measured then, so nothing is
mislabeled, and the report's `unavailable` carries the missing row's cause.

**Per-runtime impl availability.** `oxc-parser-wasm` runs under Deno and Node — its
binding ships a fetch-based browser entry (`parser.wasi-browser.js`) that Deno needs
and a default `node:wasi` entry that Node needs, so `oxc_wasm.ts` picks per runtime.
Under Bun the `node:wasi` entry fails to load, so the Bun report has no
oxc-parser-wasm row (same class as the biome-wasm Bun-load issue); Node also has oxc
native, the more relevant Node number, regardless. `dprint-wasm` runs under all
three: the `@dprint/formatter` host loads its plugin from a plain buffer
(`createFromBuffer` over `node:fs`) with no wasm-bindgen `start` hook and no
`node:wasi` dependency — verified byte-identical output under all three.

## Corpus Comparison

Compare formatting output against Prettier on arbitrary codebases.

```bash
# The gates corpus view (~6,200 files: real repos + prettier suites — see §Corpus)
deno task corpus:compare:format --all

# Single project (scans <path> recursively — NO srcDir filtering)
# ⚠ For monorepos like svelte/, use --all instead to avoid scanning test fixtures
deno task corpus:compare:format ../some-project

# Flags (any invocation): --explain (list known divergences + patterns), --summary
# (compact, no diffs), --limit N (per language), --filter <lang>, --strict (fail on
# any difference), --safety-only (data loss only), --json, --audit-patterns
# (per-pattern corpus coverage with sample diffs — spot-check for overmatching)

# Run without rebuilding FFI — guarded against a stale binary (§Artifact Freshness
# Guard); BENCH_STALE_OK=1 overrides
deno task corpus:compare:format:run ../some-project
```

**`TSV_FFI_PROFILE=corpus` lives on the `:run` task, not the wrapper.** Every
corpus/conformance FFI entry (`corpus:compare:{format,parse}:run`,
`conformance:{svelte-fixtures,ts-fixtures,ts-repo}:run`) selects the profile itself,
so running one directly loads `target/corpus` — the same binary the build-first
wrapper produces — instead of falling back to `target/release`. The profile is not a
detail: `corpus` is `panic = "unwind"`, so a formatter panic is caught and reported
as a per-file error, where `release` (`panic = "abort"`) kills the run. It also aims
the freshness guard (which derives its path from the same env var) at the binary
that will actually be loaded, so its staleness verdict and its rebuild hint both
name `build:ffi:corpus`. The **bench** and **smoke** tasks deliberately stay on
`release` — that is the artifact they measure.

`corpus:compare:format:run` sets `PRETTIER_DEBUG=1` so prettier-plugin-svelte's
verbatim-on-error fallback (whole `<script>` block echoed when the embedded
formatter throws) surfaces as a per-file **error** with a code frame instead of
fake-stable prettier output that would land in `unknown`. Same posture as the
tsv_debug sidecar; see `docs/conformance_prettier.md` §Triage caveat.

**Prettier-output cache.** The format comparison's dominant cost is prettier over
~6k mostly-unchanged files, so its oracle calls go through a content-addressed cache
(`lib/prettier_cache.ts`, `.cache/prettier/`): keyed on the source content +
parser/filepath routing + the full options + the canonical-5 pins (incl. svelte, the
plugin's peer) + `PRETTIER_DEBUG` + a schema constant — a hit is exactly equivalent
to a live run. Success-only: errors and semantically-empty outputs are never cached
(`put` rejects whitespace-only, `get` treats a stored whitespace-only entry as a
miss), so the prettier-miss heisenbug can't poison it and cached hits remove the
prettier-side flake from repeat runs entirely; the tsv/FFI side stays live. Writes
are **atomic** (temp file + `rename`), so an interrupted run can't leave a
TRUNCATED entry — which the empty guards can't catch and which every later run
would then read as the oracle. A write that fails is counted, never thrown (the
caller reads a throw as "prettier failed on this file"). The run
reports `prettier cache: N hits / M misses`, plus `/ K writes FAILED` if any did.
Scope: this tool + the conformance
driver only — never the bench (it times prettier), never the fixture validator (live
by design). `TSV_PRETTIER_CACHE=0` disables; `deno task bench:clean` wipes.

Output shape (counts illustrative — read them live):

```
Results:
  svelte       166/179 match (92.7%)    | 13 known
  typescript   172/188 match (91.5%)    | 16 known
  ────────────────────────────────────────────────────────────────────────
  total        339/369 match (91.9%)    | 30 known

Known Divergence Patterns:
  fill_101_boundary: 29 files

PASS: No safety violations or unknown differences
```

### Machine-readable output (`--json`)

`--json` emits a single buffered JSON object to **stdout** and routes all
human/progress output to **stderr**, so `2>/dev/null` leaves a clean
`JSON.parse`-able document: a `stats` block plus per-file lists for the statuses
worth inspecting.

```jsonc
{
	"stats": {
		"languages": {
			"typescript": { "total": 27, "match": 11, "known": 3, "partial": 1,
				"unknown": 8, "safety": 4, "errors": 0, "expected_errors": 0 }
		},
		"total": { "total": 27, "match": 11, "...": "..." }
	},
	"safety": [{ "path": "union-parens.ts", "language": "typescript", "bytes": 0,
		"violations": [{ "type": "content_lost", "total": 7,
			"chars": [{ "char": "|", "real": 2, "ours": 28, "prettier": 26 }],
			"missing_lines": [], "summary": "..." }] }],
	"partial": [{ "path": "...", "patterns": ["..."] }],
	"unknown": [{ "path": "...", "diff_summary": "we break (+1 lines): \"...\"" }],
	"errors": [{ "path": "...", "error": "..." }],
	"expected_errors": [{ "path": "...", "error": "...", "expected_reason": "..." }]
}
```

`match` and `known_divergence` files are excluded (their counts live in `stats`) and
full diffs are excluded (`unknown` carries a one-line `diff_summary`), so the object
stays small regardless of corpus size. Works with `--all`, `--safety-only`, and
`--filter`. Automation that just wants the table reads `.stats.total`; triage tooling
reads `.safety` / `.unknown` / `.partial` / `.errors`.

```bash
deno task corpus:compare:format:run --all --json 2>/dev/null > report.json
… | jq '.stats.total'        # table numbers
… | jq -r '.safety[].path'   # files losing content
… | jq '.safety[0].violations[0].chars'  # per-char loss (ours vs prettier; real = beyond prettier)
```

## Parse Comparison

Deep-diff tsv's shipped parse output against the canonical parsers
(acorn-typescript / `svelte.parse` / `parseCss`) — the parser-side sibling of the
formatting comparison. Native-FFI-only by design: the WASM artifact rides the same
Rust wire (`convert_ast_json_string`), differing only at the boundary, and is
already exercised per-file by the bench preflight and `deno task smoke`. This is the
external oracle the internal identity gates can't provide: fixtures cover curated
cases and the wire-JSON writer is the sole emission path, so a writer bug (e.g. an
untranslated position field) on an uncurated shape is invisible without a
canonical-parser comparison at corpus scale.

```bash
deno task corpus:compare:parse --all                    # full corpus
deno task corpus:compare:parse --all --multibyte-only   # offset-translation slice (riskiest machinery)
deno task corpus:compare:parse ../zzz --filter typescript --limit 100
deno task corpus:compare:parse --all --json 2>/dev/null > report.json
deno task corpus:compare:parse:run --all                # skip rebuild (freshness-guarded)
```

Method: ASTs are **raw-diffed with no pre-diff normalization**; diffs are classified
against the documented divergences (`docs/conformance_svelte.md`) at the reporting
layer only, so a bug in our own divergence reasoning surfaces as an undocumented
group instead of being silently absorbed. The canonical AST is serialized exactly
like the fixture sidecar (JSON round-trip, BigInt → string) so corpus and fixture
semantics match. Diffs are grouped by path signature (array indices erased) across
files; undocumented groups are the actionable output and fail the run (exit 1).
Parse failures on either side are counted and skipped — `skip_triage.ts` is the
dedicated tool for those.

The documented-divergence matchers live in `corpus_compare_parse.ts`
(`DOCUMENTED_MATCHERS`) and cover only the AST-content divergences that parse on
both sides (comment-attachment duplication, async-generic-arrow params); the
parser-feature corrections (`using`, v-flag regex, CSS namespaces) make the
canonical parser throw, so they land in the error buckets. When triage confirms a
new group is intentional, add a matcher AND catalog it in
`docs/conformance_svelte.md`.

## Parse-Conformance Gates

Three gates run tsv's parsers against an upstream suite. All three share one shape —
**verdict parity** (enforced) plus **AST-shape** deep-diff (report-only, via the
SHARED `corpus_compare_parse.ts` engine: `diff_asts` + `DOCUMENTED_MATCHERS`, which
is `import.meta.main`-guarded so importing it doesn't run the CLI). All accept `-v`,
`--json`, and a subtree path; each has a `:run` variant that skips the FFI rebuild
(freshness-guarded).

```bash
deno task conformance:svelte-fixtures   # builds the corpus FFI, then runs
deno task conformance:ts-fixtures
deno task conformance:ts-repo
deno task conformance:ts-fixtures:run -v     # skip rebuild; + per-file gap / AST-group detail
deno task conformance:ts-fixtures:run --json 2>/dev/null > report.json
deno task conformance:svelte-fixtures:run ../svelte/packages/svelte/tests/parser-modern  # a subtree
```

**Verdict parity buckets over-rejections** (tsv rejects what the oracle accepts)
into `SANCTIONED` (tsv diverges *deliberately*; the shared list is
`lib/parse_sanctions.ts`), `KNOWN_GAPS` (tsv wrong; a tracked drop-in gap that must
only shrink, an in-file allowlist per gate), and `unexpected` (a NEW gap — **exits
1**). `over_acceptance` (tsv accepts, the oracle rejects) is a deferred early-error:
reported, not gated. Green at baseline = every gap is sanctioned or tracked.

**Shared gate hygiene** (`lib/fixtures_gate.ts`, the svelte + ts fixtures gates).
The suite INPUTS come from a sibling checkout while the grading parser is the pinned
npm oracle, so full-suite runs compare the checkout's `package.json` version against
the pin and **warn on skew** (non-fatal — a checkout tracking upstream main is
legitimate, but silent divergence isn't). Full-suite runs also **freshness-check the
ledgers**: a `SANCTIONED`/`KNOWN_GAPS` entry that matched no over-rejection fails
the run (delete it when its gap is fixed; update the pattern on an upstream rename)
— the same mirror-the-live-corpus discipline as `scan_audit`'s ALLOW list. Subtree
runs skip both checks.

**Oracles are LIVE parsers, never committed artifacts.** A committed artifact can
drift from the pinned version that defines fixture correctness, and the live parser
is exactly what `corpus:compare:parse` diffs against, so the two stay consistent by
construction.

### `conformance:svelte-fixtures`

tsv's Svelte parser vs **Svelte's own compiler test suite**
(`../svelte/packages/svelte/tests`) — the drop-in-parser analog of test262 (JS) and
the WPT harness (CSS). Entry: `diagnostics/svelte_fixtures_compare.ts`.

Oracle = the live modern parser (`svelte/compiler` `parse(src, {modern:true})`):
`parser-legacy`'s `output.json` is the *legacy* AST, `compiler-errors/_config.js`
encodes *compiler* (analysis-stage) verdicts, and `css` ships compiled CSS, so none
is a correct oracle for a drop-in *modern-parser* replacement. Using the live modern
parser also makes the two trap partitions resolve for free: `loose-*` inputs throw
under the non-loose oracle (→ parity), and analysis-stage `compiler-errors` parse
fine on both sides (→ never miscounted as a tsv bug).

Scope: the canonical `.svelte` INPUTs (`input.svelte`/`main.svelte`/`index.svelte`),
skipping generated `_`-prefixed artifacts, `output.svelte` dups, and the `migrate/`
tree (Svelte-4 migrator inputs). `.svelte.js`/`.ts`/`.css` are out of scope (test262
/ wpt cover those). A merely *lenient parser* is not grounds for a sanction — tsv is
a drop-in for Svelte's parser — so the SANCTIONED list is currently **empty** here.
The adversarial fixture tree exposes many AST-shape edge divergences; triaging each
into the shared `DOCUMENTED_MATCHERS` (which also shrinks the
`corpus:compare:parse` count) or fixing it as a writer bug is a tracked campaign, so
that half does **not** gate yet.

### `conformance:ts-fixtures`

tsv's TypeScript parser vs **acorn-typescript's own test suite**
(`../acorn-typescript/test`, ~200 adversarial `input.ts` fixtures). tsv is a drop-in
for acorn + acorn-typescript, so that parser's own regression corpus is the natural
TS edge-case oracle: the shape real-world code can't reach. Entry:
`diagnostics/ts_fixtures_compare.ts`.

Scope: every `input.ts` under the suite root (`*.test.ts` / `utils.ts` harness files
excluded by basename). `.tsx`/JSX fixtures parse as ordinary `.ts` here — tsv and
acorn (module mode, no JSX plugin) both reject them, so they land in `parity`.
Sanctions here are deprecated syntax tsv declines (e.g. import assertions `assert
{…}`) or input its own grammar rejects (`TS_FIXTURE_SANCTIONS`). Strict about
setup: a missing checkout (0 scanned) **FAILS** — a run that graded nothing must not
read as a pass; the tolerance point for machines without it is publish Step 3b's
preflight. Unlike the Svelte tree's backlog this corpus is near-clean, so promoting
AST-shape to a gate once the undocumented-group count hits 0 is a natural follow-up.

### `conformance:ts-repo`

tsv's TS parser over `../typescript/tests/cases` — the WHOLE corpus, ~13.7k
single-file `.ts` — using **tsc's OWN baselines as the validity oracle** — a
`tests/baselines/reference/<name>.errors.txt` with a `TS1xxx` code = tsc's parser
rejects (→ tsv correctly stricter), no `TS1xxx` = tsc accepts (→ a tsv reject is a
real gap). Entry: `diagnostics/ts_repo_compare.ts`.

tsc is authoritative because acorn-ts (tsv's *target*) is itself over-lenient; using
tsc's baselines auto-resolves those leniency cases to reject-parity (no sanction
needed), and acorn's verdict sub-labels each gap (`gap` = acorn-confirmed → gates;
`gap_beyond_acorn` = acorn also rejects, a mixed acorn-gap / early-error-timing
surface → reported, not gated). In the blocking `conformance` aggregate (promoted
once its baseline hit 0 untracked gaps), tracked separately from the acorn-suite
gate (own `KNOWN_GAPS`, freshness-checked on full-corpus runs). `.tsx` and
`@filename` multi-file tests are skipped (5,158 of them — a filed coverage hole; the
directive rule mirrors tsc's own harness, which `is_multi_file_test` argues in full);
`.d.ts` cases ARE graded (61 of them — a declaration file is ordinary TS to tsv, and
the bench harvest skips them for a reason of its own, argued at each `DECLARATIONS`).
Baseline: 13,708 scanned, 12,284 accept-parity, a 487-entry over-acceptance pin, 0 untracked gaps. ⚠️ The root is the
whole corpus deliberately: the old `conformance/parser` default was green at 768 files
while 32 over-rejections sat untracked in the checker/emitter trees, whose ordinary TS is
likelier reachable in real code than the parser torture suite. A
missing checkout, a partial one (baselines or corpus subtree missing), or an empty
scan all FAIL rather than green-skipping.

### Pre-release aggregate — `conformance` / `conformance:all`

The three gates above plus `corpus:compare:parse --all` and `corpus:compare:format
--all` are the release-cadence correctness gates that run against external oracles
(and so can't live in `deno task check`). The typechecker's `tsc_conformance` tasks
are deliberately NOT legs — `tsv_check` is experimental and may never ship, so its
gates stay on-demand (../../docs/typechecker.md).

`deno task conformance` builds the corpus FFI once and runs all six legs in **ONE
process** (`conformance.ts`): the canonical oracle modules (prettier, the svelte
plugin, svelte/compiler, acorn, acorn-ts) load once via the module cache instead of
once per leg (`render:audit`, the lone non-JS leg, is a `cargo` subprocess — which
is why the task carries `--allow-run=cargo`), each leg gets a timing line, and
failure semantics match a `&&` chain exactly (every leg exits the process on a
finding — fail-fast). The driver takes no arguments; the per-leg tasks remain the
scoped/triage entries.

`deno task conformance:all` runs the pure-Rust **test262 positive gate** FIRST, THEN
the aggregate — so a positive-parse regression trips the ~1-min gate before the
multi-minute FFI legs run. That superset is what publish **Step 3b** runs (skipped by
`--no-check`), after preflighting the oracles + `node_modules`: a missing one **FAILS
a `--wetrun`**, warn-and-skips a dry-run, and any skip is re-warned in the final
summary. The gates themselves fail closed on a missing checkout (0 scanned = FAIL),
so a manual `deno task conformance` can't green-skip a leg. `corpus:compare:format`
there gates on **SAFETY** (data loss) — the ~8% intentional style divergences are
non-blocking WARNs, and every SAFETY finding is self-verified in-run (the native
format is re-run and must reproduce byte-identically; nondeterminism surfaces as a
loud per-file error instead — see [§Known Issues](#known-issues)). Both corpus tools
also fail (exit 1) on a run that compared nothing: an empty scope (`No files found`)
or an every-file-errored / every-file-parse-fail-skipped run is a systemic failure —
sidecar/FFI down or a wrong corpus — never a pass.

**A caught panic hard-fails both corpus tools, on every run** (not just `--all`, and
never inside a bucket). These tools build tsv with `--profile corpus` (`panic =
"unwind"`) precisely so a crash is caught and reported per file rather than killing
the run — but the SHIPPED artifacts are `panic = "abort"` and take the host process
down on that same input, so the caught panic would land in the run's mildest bucket
while describing the release's harshest failure. Ungated it reads as one more
`errors` WARN at exit 0 in `corpus:compare:format`, and as a dimmed `parse-fail
skipped` line in `corpus:compare:parse` — where only `--all`'s exact
`CORPUS_PARSE_TSV_ERRORS_PIN` notices, and then as "a new over-rejection". The gate is `gate_on_panics` in `lib/compare_cli.ts` (each tool
passes the failures that could be tsv's; the classification is shared, so the two
can't answer this question differently) over `is_native_panic_error` in
`lib/divergence/panic_errors.ts`, matched against the message's FIRST LINE so a
rejection's source code frame can never fabricate the verdict; only tsv can produce
those shapes, since the oracle on the other side is JS. Classification runs BEFORE
`check_expected_error` — those patterns key on file *content*, so a panic on a file
that also happens to hold SCSS would otherwise file as an expected error and vanish
from the report entirely.

## Divergence Detection

Automatically detects known divergence patterns from the `conformance_prettier*.md`
family. Internals, the pattern registry, and the pending-work taxonomy live in
[../../docs/divergence_detector.md](../../docs/divergence_detector.md); this is the
operational summary.

- **Safety checks**: differential character-frequency comparison vs prettier detects
  data loss — reports only the semantic chars our output drops/adds **beyond** what
  prettier does (shared normalizations cancel).
- **Pattern detection**: hunk-aware — patterns must explain specific diff hunks, not
  just match global file properties. Detectors live in `lib/divergence/`
  (`patterns.ts`, tested by `patterns_test.ts`); each pattern carries
  `conformance_sections` (which doc sections it covers) and `fixtures` (an explicit
  assertion that it detects those `*_prettier_divergence` fixtures, gated by
  `test:deno`).
- **Classification**: `known` (all hunks explained), `partial` (some hunks
  unexplained), `unknown` (needs investigation), `SAFETY` (data loss).

```bash
# Detection audit: runs every pattern against every documented fixture's committed
# prettier forms. Coverage is COMPUTED, not read out of the fixtures[] arrays (those
# are explicit assertions, gated by test:deno, and drift from what the detectors
# actually see). Exits 1 on a genuine gap; listing drift is bookkeeping.
deno task divergence:audit [--json]

# Deno test suite — the divergence detectors, gated by `deno task check`. Pattern
# positive/negative overmatch-rejection cases, safety differential cases, and a
# behavioral fixture-coverage audit driving each detector against its own committed
# fixtures (input == ours, output_prettier == prettier), failing if a pattern stops
# claiming a hunk in a fixture it lists. Dependency-free (`node:assert` + relative
# imports), so CI runs them on a clean checkout with no `bench:install` — which is
# why they're in the core `check` gate.
deno task test:deno

# The canonical-oracle test (NOT gated — needs prettier/svelte, so run after
# `bench:install`): asserts the prettier baseline formats with a filepath, so `.ts`
# single-type-param arrows stay `<T>` and `.svelte` ones get `<T,>`, and the `.js` →
# babel / `.ts` → typescript parser routing holds.
deno task test:deno:canonical

# Typechecks the JS/TS the repo owns — this harness, `scripts/`, and the tsv_debug
# Deno sidecar — which `deno task typecheck` (cargo) does not see. Takes DIRECTORIES,
# so a new subdirectory is covered the day it appears. NOT gated, for the same reason
# as the line above: the harness imports npm by bare specifier, and CI's `check` job
# installs no node_modules. Run it after a harness, scripts, or sidecar change.
deno task typecheck:js
```

"Documented" = every `*_prettier_divergence`-suffixed fixture linked from the
`conformance_prettier*.md` family in any of its three anchor formats (table rows,
list items, prose paragraphs); non-divergence fixture links (match/contrast anchors)
don't count. Coverage is partial by design —
[divergence_detector.md §Traceability](../../docs/divergence_detector.md#traceability).

Two headline numbers answer different questions and must not be conflated:
**detection** is measured (the audit runs `detect_divergences`, the same classifier
the corpus comparison uses — so the language filter and the three-level hunk
coverage are identical); **listed** is bookkeeping (what the `fixtures[]` arrays
say). `partial` is counted apart from `explained` for the same reason the corpus
classifies it apart from `known`: a pattern IS attached, so a binary
detected/undetected metric would read it as covered while hunks go unexplained,
re-introducing at the audit level exactly the masking hunk-aware detection exists to
prevent.

**The report is the work-list** — undetected, partial, ungradeable, and the
(deliberately non-backlog) unlisted bookkeeping. Read the counts live rather than
from any doc;
[divergence_detector.md §Pending work](../../docs/divergence_detector.md#pending-work)
explains what each bucket means and which are worth closing. The subset of partial
fixtures that a pattern also *lists* is
ratcheted by `KNOWN_PARTIAL` in `fixture_coverage_test.ts` and gated in `deno task
check`: a listed fixture going partial fails, and a stale entry fails too, so it
mirrors the live set and can only shrink.

## Benchmark Commands

```bash
deno task bench:install   # one-time: install harness npm deps

# Run benchmarks (builds the runtime's artifacts automatically).
deno task bench           # full refresh: perf ×3 + compose + node conformance coverage
deno task bench:perf      # perf surface only: all three runtimes + compose
deno task bench:deno      # Deno only (no node/bun needed)
deno task bench:node      # Node only
deno task bench:bun       # Bun only (reuses the Node artifacts — N-API + nodejs-target WASM)
deno task bench:compose   # fold whatever report.{deno,node,bun}.json exist → report.{json,md}

deno task bench:conformance      # harvest + build:bench:node + the coverage run
deno task bench:conformance:run  # skip harvest + rebuild (freshness-guarded)

# Run without rebuilding — guarded against stale artifacts (§Artifact Freshness Guard)
deno task bench:deno:run   # also :node:run / :bun:run

# Flags (shown for :deno:run; same for the others)
deno task bench:deno:run -- --json           # JSON output (CI/tooling)
deno task bench:deno:run -- --markdown       # Markdown tables
deno task bench:deno:run -- --verbose        # per-file skip detail (paths + errors)
deno task bench:deno:run -- --save-report    # force-overwrite the committed report on a
                                             # limited/filtered run (full runs overwrite anyway;
                                             # the timestamped pair is always written)
deno task bench:deno:run -- --save-baseline     # save current results as baseline
deno task bench:deno:run -- --compare-baseline  # compare against saved baseline

# Wipe local-only bench state (gitignored): baseline.json, timestamped results
# pairs, and the harvest caches. Preserves the committed report.* files (the glob is
# anchored on a leading digit — timestamped files start with a year).
deno task bench:clean

# Environment variables (any runtime's :run)
BENCH_LIMIT=5           # files per language (default: all)
BENCH_FILTER=zzz        # path pattern (default: none)
BENCH_DURATION=10000    # ms per benchmark (default: 5000; conformance mode: 15000)
BENCH_WARMUP=10         # warmup iterations (default: 3; slow >5s-per-sweep tasks tier to 1
                        # unless set explicitly)
BENCH_MODE=union        # per-impl iteration (default: intersection)
BENCH_CORPUS=conformance  # corpus/surface selector (default: perf)
BENCH_STALE_OK=1        # run despite stale artifacts (default: off)
BENCH_COVERAGE_ONLY=1   # coverage-only run, no timed phase (what bench:conformance:run sets)
BENCH_FORCED_ASYNC=1    # add the tsv-forced-async control row (diagnostic; default: off)
BENCH_GC=1              # call globalThis.gc() between iterations (default: off — not a
                        # uniform bias; see docs/benchmarks.md)
BENCH_ALLOW_MISSING=1   # tolerate a partial corpus
```

`deno task bench` regenerates EVERY committed artifact the site consumes, reusing
the node artifacts the perf half just built for the coverage run. It FAILS FAST if
node or bun isn't installed — `bench:runtimes` preflights `bench:perf`, ahead of the
~8 minutes it would otherwise take to discover the miss (by which point two of the
three siblings have been regenerated and `bench:compose` skipped, leaving the
committed combined report stale against fresh siblings). ⚠️ Its node arm asks what
the binary IS, not whether the name resolves: `deno task` prepends its node-compat
shim (`~/.cache/deno/node_compat_bin/node` → the deno binary) to PATH, so `which
node` succeeds inside every deno task on a machine with no node — and that shim RUNS
the harness, where `current_runtime()` reports `deno` and `bench:node` overwrites
`report.deno.*` rather than producing a node sibling. `globalThis.Deno` is the tell
the shim cannot hide. Deno is
the only hard dependency, so without node and/or bun run the per-runtime tasks you
DO have — each writes its own sibling and `bench:compose` folds whatever exists.

**Conformance measurement** is per-tool PARSE COVERAGE over the fixtures-only
`conformance` view → `report.conformance.node.{json,md}`. Two things are specific to
this surface. **`tsc` is a row here and only here** (`lib/tsc.ts`): the language's
own parser is a verdict, not a speed, so putting it in the published throughput
tables would misread it — flipping it on for perf is a one-word change at its
registration site. And the report carries a **per-source coverage table** under each
group's aggregate line, because the aggregate blends corpora that answer different
questions: on the tsc corpus `tsc` is the ORACLE (100% by construction — the harvest
keeps exactly what it accepts), the way `svelte/compiler` is on the Svelte set,
while on test262 and the prettier suites it is an independent parser. Read the
source rows; the aggregate is a summary, not the finding. **Coverage-only +
node-only by design** (`BENCH_COVERAGE_ONLY=1`): coverage is a pre-flight product,
so the timed phase is skipped, and it's runtime-invariant (same parser engine — the
site folds a tool's native/wasm variants into one per-engine row), so one node run
is the whole surface. Entries carry null timing; no throughput/comparison sections;
baseline save/compare are no-ops. Skipping the timed phase reclaims a fixed ≥8
full-corpus sweeps/row (3 warmup + ≥5 measured) that no consumer reads. The timed
parse-throughput over this adversarial corpus has no consumer, so no task produces
it; to investigate ad-hoc run `BENCH_CORPUS=conformance node benches/js/bench.ts`
(coverage flag unset) — it overwrites `report.conformance.node.*`, so re-run
`bench:conformance:run` after to restore the committed report.

### Harvests

```bash
deno task bench:harvest            # all five
deno task bench:harvest:wpt        # ../wpt/css <style> blocks → .cache/wpt_css
deno task bench:harvest:test262    # graded positives → .cache/test262_files.json (runs cargo)
deno task bench:harvest:ts-repo    # tsc-corpus valid + rejects lists → .cache/ts_repo_{files,rejects}.json
deno task bench:harvest:svelte-rejects  # svelte/compiler-rejected Svelte files
                                        # → .cache/svelte_parse_rejects.json
deno task bench:harvest:svelte-styles   # perf-view .svelte <style> blocks, concatenated per
                                        # repo → .cache/svelte_styles/<repo>.css
```

Idempotent; warn-and-skip when the source checkout is absent. The first four are
FRESHNESS-STAMPED (`lib/harvest_stamp.ts`): a harvest whose stamped inputs — the
source checkout COMMIT(s) + the pinned count + oracle pins — are unchanged skips
instantly (the test262 leg saves a ~1 min release-mode grade; the ts-repo leg stamps
the tsc VERSION too, since tsc is its oracle and a bump can move a file between its
two lists with the checkout unchanged); pass `--force` after
changing harvest/grading LOGIC, which the stamp can't see. `svelte-styles` is NOT
stamped (its sources are the live dev repos; the walk is ~2 s, always re-harvests,
rewrites only changed files) and is also chained at the start of `bench:perf` so
perf runs measure a fresh cache. All are chained into the `bench:conformance` build
tasks; run standalone after a `../wpt` or `../test262` update — and EXPECT the
pinned harvest count to trip after a source pull
([../../docs/gate_counts.md](../../docs/gate_counts.md)): re-pin in
`lib/gate_counts.ts` deliberately.

### Report files

Each runtime saves to `benches/js/results/` as timestamped files plus a committed
`report.<runtime>.{json,md}` pair. The conformance surface writes
`report.conformance.node.{json,md}` instead — a separate committed surface that
never clobbers the perf reports and is invisible to `bench:compose` (which globs the
exact perf filenames). To publish to tsv.fuz.dev, run `npm run update-benchmarks` in
`../tsv.fuz.dev` — its copy list names these files exactly, so renaming a report
artifact means updating that script in the same change. Its
`src/routes/docs/benchmarks/benchmark_data.ts` likewise MIRRORS the JSON shape
below, field for field and version note for version note, so a new top-level field
here is a change there too — it declares them optional and degrades on an older
report, which is what makes the drift silent rather than loud.

The committed JSON (per-runtime `version: 13` — the combined compose report carries
its own `version: 12`; coverage-only runs add `coverage_by_source`) carries, beyond
timing stats: top-level
`runtime`; a `machine` block (`cpu_model` + `os`/`arch` + `runtime_version` — the
numbers are machine-relative, so this travels with them; excludes hostname and
volatile fields so it doesn't churn); `corpus_kind` (`perf` | `conformance`);
per-language `corpus` totals; `corpus_sources` (per-entry loaded file counts + a
`by_language` split summing to `files` — the composition disclosure); `versions`;
and `binary_sizes` (each with `gzip_bytes`). Each `entries[]` row adds `runtime`,
`files_processed`/`files_total` (per-impl preflight coverage — the `Coverage:` line)
and `files_iterated` (the timed set — the `Files (intersection):` count).

**Null timing is not exclusive to a coverage-only report:** a coverage-only ROW
(`rsvelte-fmt`) carries null stats inside an otherwise fully-timed perf report, and
is identifiable by `files_iterated: null` — it was timed on nothing, rather than
timed on the group's intersection. A consumer that reads `entries[]` as speeds must
skip a row with null `ops_per_second`, not treat it as a zero. Top-level
`suppressed_noise` records silenced third-party stderr crashes as `{pattern:
count}`; top-level `output_digest_ungraded` records files a byte-graded row
ACCEPTED whose output the byte-parity check could not digest, as `{"<group>/<row>":
count}` — the one known cause is a pathologically deep AST overflowing V8's
recursive `JSON.stringify` (tsc's `binderBinaryExpressionStress.ts`), and it is the
one field that records a measurement the run could NOT make, so a growing count is
the byte check quietly covering less; top-level `variant_parity` records any
same-engine pair (two bindings, or one binding under two options) whose
pre-flight accept sets disagreed (`[]` when healthy — a non-empty list in a
committed report is a binding-boundary bug surfacing in the diff); top-level
`unavailable` records each optional impl that failed to init, as `{impl, reason,
rows}` — the ⚠ init line's label, the load error's first line, and the ROW names its
absence removed from this surface (`[]` on a full machine; under Bun the two known
per-runtime load failures land there as `OXC WASM → [oxc-parser-wasm]` and `Biome →
[biome-wasm]`, §Cross-Runtime). The
three answer escalating
questions about the same surface — noise silenced, a row behaving wrongly, a row
NOT THERE — and the last is the one a table can't ask, since an impl that stops
loading takes its column out of every table and the ⚠ init line lives only in the
run's output.

**Three impls can never appear there, because they are REQUIRED**: `canonical`
(the oracle) and tsv's own `native` + `wasm`. A load failure in any of them throws
out of `init_implementations` (`init_required`) instead of joining `unavailable`,
and their slots are correspondingly non-`undefined` in `ImplementationSet` — a
broken tree, not a machine coming up short. Before that, a wasm bundle that was
present but wouldn't load published a report with every `tsv_wasm-*` row silently
gone behind one ⚠ line, and five diagnostics each hand-rolled their own
`if (!impls.native) throw`. Note the division of labour with the freshness guard:
`check_artifact_freshness` makes a MISSING artifact fatal, a present-yet-unloadable
one surfaces only here. The expected-`unavailable` set is never tsv on any runtime
(under Bun it is biome + oxc-parser-wasm), so nothing legitimate is refused.

**`rows` is the joinable half, and the reason it exists.** Every other identity the
report publishes is a row name (`entries[].name`, `variant_parity.impl`/`.sibling`,
`report.ts`'s `DISPLAY_ORDER`), so a consumer asking "is this blank cell a load
failure?" holds a row name — which the init LABEL matches for no impl whose label
differs from its row (`Biome` vs `biome-wasm`), and cannot match at all where one
impl backs several rows (`native` backs four; `oxc` backs `oxc-parser` and `oxfmt`).
`rows` is DERIVED, never mapped: `init_implementations` keeps each failed impl's
constructed-but-uninitialized instance in `complete`, and `get_defined_rows` asks
the one task registry against that set (sound because the gates it evaluates —
`parse_languages`/`format_languages`, `format`/`parse_internal` — are
construction-time facts, not init state). It is SURFACE-scoped for the same reason
the disclosures are: a `tsc` failure costs the perf surface no row, a `yuku` failure
costs the conformance surface none, and an empty `rows` says exactly that — the
machine is short while the tables are whole. The composer folds these into
`unavailable_by_runtime[].rows`.
Top-level `binary_sizes_absent` names the artifacts the size table reached for and
did not find — that table is the one section whose COMPOSITION varies by machine
(a row exists only for a built artifact), so a tsv variant listed there usually
just means its optional build task wasn't run, while a third-party label means its
package shipped nothing where `binary_sizes.ts` looked.
`report.<runtime>.md` renders coverage/iterated as prose; the per-entry numbers,
`suppressed_noise`, `variant_parity`, `unavailable`, and `binary_sizes_absent` are
JSON-only.

The conformance report's **Excluded here:** / **Added here:** disclosures are
authored prose whose CLAIM is checked: `surface_disclosure_lines` (bench.ts) throws
if the table says a row is excluded and this surface registers it, or vice versa.
The policy itself lives at the `corpus_kind` conditions in `lib/implementations.ts`,
so the check is what keeps the published sentence from outliving the code —
re-enabling yuku's N-API row after an upstream fix fails the run until the
disclosure is updated. It asks the task REGISTRY (`get_defined_rows`), not the
rows a run measured, and asks it at init: a corpus filter can empty a whole group,
and grading that as policy drift failed partial runs at report time, after their
work and with nothing written. The registry is asked the **availability-independent**
question (`impls.complete`) — asked of the live set instead, an `excluded` claim
passes vacuously whenever the impl merely failed to load, so a re-enabled row on a
machine whose binding didn't install would publish the stale sentence with the guard
silent. One absence is exempt — an **added** row whose impl
never initialized is this machine coming up short (already in `unavailable`), so the
run warns and drops that line instead of failing.

**Two more registry-checked claims, both warnings.** `report.ts` holds two checked
hand-maintained lists that a new impl has to reach, and each is asked the same
availability-independent question at init (`get_defined_rows`), one direction only
(a listed row absent from a surface is not drift — each surface registers its own
subset): `DISPLAY_ORDER`, where an unlisted row sorts silently to the end of every
table (`rows_missing_from_display_order`), and `COMPARISON_SECTIONS` — the
Comparisons tables' per-tier opponent lists — where an unlisted row gets no
comparison cell at all (`rows_missing_from_comparisons`, cleared by an entry in
`COMPARISON_EXCLUSIONS` for a row that belongs in none). Both WARN rather than
throw: an absent row understates a table, where a stale `SURFACE_DISCLOSURES`
sentence asserts something false. The comparison guard exists because its drift is
the quietest of the three — a missing cell looks like nothing — and `swc`,
`postcss`, `rsvelte-parse` and `malva-wasm` were each registered, preflighted and
timed at full coverage while appearing in no comparison. A section's opponents each
carry their own fairness note, rendered iff that opponent produced a cell, so the
prose can't drift from the table either.

A **third** row list in the same module is deliberately unchecked: the curated
payload-matched lines in `generate_summary_report` (`tsv-json-no-locations` vs
`oxc-parser`, and the rest). Its membership is an ARGUMENT — this tsv wire and
that opponent emit the same product — not a completeness claim: most rows have no
payload-matched partner and never will, so a guard there could only be a warning
nobody clears. A new impl still has to be considered against it; `swc` and `postcss`
were, and are absent on purpose (docs/benchmarks.md §Fairness caveats).

## Artifact Freshness Guard

The rebuild-skipping tasks (`bench:{deno,node,bun}:run`, `bench:conformance:run`,
`corpus:compare:{format,parse}:run`,
`conformance:{svelte-fixtures,ts-fixtures,ts-repo}:run`, and `smoke`) skip the
rebuild so you can iterate on the
harness without paying the wasm-pack cost — at the risk of silently measuring a
binary older than current source (a CSS run once reported `146/183` against a stale
`.so` that should have been `155/183`). `lib/check_artifact_freshness.ts` guards
this: before a run touches the executed artifacts (the runtime's native binding +
WASM bundle — Deno: FFI + `pkg/all/deno`; Node: N-API + `pkg/all/nodejs`, the pair
`check_executed_artifacts` composes for bench and smoke alike; the corpus tools run
no WASM, so they guard `native_artifact_check()` alone), it
compares their mtimes against the crate sources feeding them (plus the workspace
`Cargo.lock`, so dependency bumps trip it too) and **aborts (exit 1)** if any is
stale or missing. The build-first tasks rebuild first, so they pass for free.
`BENCH_STALE_OK=1` downgrades a _stale_ artifact to a `⚠` warning (a _missing_ one
stays fatal); see the module doc for why stale is a hard error by default.

**The build-side sibling: fresh builds SKIP.** The four wasm-pack bench build tasks
(`build:wasm:deno`, `build:wasm:parse:deno`, `build:wasm:all:deno`,
`build:wasm:all:nodejs`) ride `scripts/run_if_stale.ts`, which skips wasm-pack when
the bundle's `.wasm` is already newer than every source feeding it (the guard's
`CORE_CRATES` + `WASM_CRATES` — `tsv_wasm` plus the `tsv_ignore`/`tsv_discover`
crates the bundle links but the FFI / N-API don't; imported, so the two sides can't
drift; dev-tooling crates deliberately excluded so `tsv_debug` edits don't force
wasm rebuilds — plus the workspace `Cargo.toml` + `Cargo.lock` and `deno.json`, so
editing a build task's flags re-triggers it). Rationale: wasm-pack re-runs wasm-opt
(~8–27s per bundle) even when cargo is a fully-cached no-op, so a source-unchanged
`deno task bench` would otherwise pay ~90s of pure wasm-opt. What the check CANNOT see is a
toolchain change (wasm-pack / wasm-opt / rustc upgrade) — after one of those, force
with `TSV_BUILD_FORCE=1`. The publish path never skips (`publish.ts` sets
`TSV_BUILD_FORCE=1` around `build:packages`); the `build:npm:*` tasks are
deliberately unwrapped.

```bash
# After editing a crate, the fast/correct paths:
deno task bench:deno                             # rebuilds, then runs — always fresh
deno task build:ffi && deno task bench:deno:run  # rebuild just what you changed, then :run
BENCH_STALE_OK=1 deno task bench:deno:run        # deliberately measure the current (stale) binary
```

## Smoke Test

`deno task smoke` runs a fast sanity check on every formatter and parser (trivial
fixed inputs, non-throwing + non-empty + idempotent), exiting non-zero on any
failure. Use it to catch "implementation totally broken" before running the full
bench; `corpus_compare_format` is still the real correctness gate. Runtime-neutral
like the bench — `smoke` (Deno), `smoke:node`, `smoke:bun` each load that runtime's
own native + WASM artifacts, so an impl-load break is caught per runtime (it's how
the Bun biome-load issue surfaced). Like the `:run` tasks it skips the rebuild and
is freshness-guarded (rebuild with `deno task build:bench`, or `BENCH_STALE_OK=1`).

## Corpus

One tagged entry list (`lib/corpus.ts` `CORPUS_ENTRIES`, paths relative to the
project root). Every entry is `{path|files_from, tier, extensions?, skip?,
optional?}` with a tier of `real`, `framework`, `prettier_fixture`, or `suite`, and
each consumer selects a **view**. Extensions: `.svelte`, `.ts`, `.js`, `.css`,
`.html` (treated as Svelte; only loaded by entries that opt in).

**Reproducible vs live (the gate split).** `framework` + `prettier_fixture` are
version-pinned checkouts (`GATE_CHECKOUT_COMMITS`, verified by `pins:audit`) — the
loader tags their files `reproducible: true` (`REPRODUCIBLE_TIERS`). The `real` tier
is the author's LIVE dev repos (zzz, fuz\_\*, gro, the personal sites), unversioned
working trees. The **format count pins (match/unknown/partial) gate on the
reproducible subset only**, so a `pins:audit`-aligned machine measures them exactly;
live-repo divergences are a **non-gating WARN**. **SAFETY (content loss) still gates
over every file.** An aggregate pin spanning the live tier is a re-pin treadmill: it
drifts with dev-repo churn (measured — re-pinned 3× in 2 days, and the pin commit
couldn't reproduce its own number).

- **`perf`** (~3,200 files) — `real` + `framework`, all real code: application &
  library source (the fuz.dev repos' `src/` — zzz, the fuz ecosystem, gro,
  svelte-docinfo, tsv.fuz.dev — plus the author's public SvelteKit sites) plus
  upstream framework source (kit, svelte, and the svelte.dev subpaths). `.d.ts`
  files are IN scope (the product formats them; declaration-heavy shapes carry real
  divergence signal), and the curated entries skip the `/build/`+`/dist/`
  build-output pruning (a `build/` segment inside a reviewed `src/` tree is real
  source, e.g. kit's `src/exports/vite/build/`; `DirectoryLoader`'s arbitrary-path
  scans still prune both). The CSS set additionally carries the `svelte_styles`
  per-repo concats harvested from those repos' `<style>` blocks. Fixture subtrees
  are pruned (`fixtures` segments anywhere; `samples` under a `test` segment) while
  `*.test.ts` files stay — tests are real code. This is what `deno task bench`
  measures, so throughput reflects real code, not formatter edge-case suites. **This
  framing is the source of truth for the public benchmark page's "What's measured"
  prose — keep them in sync.** Because it's code that ships, every in-scope tool must
  process every file: after the perf pre-flight, `bench.ts` HARD-FAILS on any
  per-file failure not excused by `lib/perf_omit.ts` (`PERF_OMITS` — kept minimal;
  the current entries all tolerate third-party limitations on declaration-file-only
  syntax, e.g. acorn-typescript has no `.d.ts` mode). A silent skip would let
  coverage quietly erode; that invariant is what makes the perf/conformance split
  meaningful. The list is a **RATCHET**, graded in both directions: a full-corpus
  run also fails on an entry that excused NOTHING, the same ledger-freshness
  discipline `lib/fixtures_gate.ts` applies to its sanction / known-gap lists — so
  a tolerance can't outlive the failure it was written for. The entries must also
  be **DISJOINT**, checked on any run (not just a full one) because the overlap is
  OBSERVED rather than inferred: a failure two entries both claim fails, since
  neither is then the entry that describes it. That check is what makes the
  staleness direction trustworthy — a first-match reading credits only the earlier
  of an overlapping pair, and the shadowed entry then reports as stale while its
  failure is live, the inverse of what happened (every match is credited, so the
  misreport is unreachable either way). Structural disjointness is not checkable at
  all: both predicates are substring tests, so for any two entries some string
  contains both fragments. An entry can still be written too BROADLY without
  reaching another's failure, which nothing catches: it stays used on its original
  failure and goes on absorbing whatever arrives beneath it, so keeping each `path`
  narrow enough to name one file stays the author's job.
  Staleness is asked only where the run could have exercised the entry, along two
  axes — the FILES (`BENCH_LIMIT` / `BENCH_FILTER` / `BENCH_ALLOW_MISSING` withhold
  the very files an entry is about, so only a full run grades that half) and the
  TASK (every alternative impl is optional, and one that fails to load registers no
  task at all, so on that machine its entries are unasked rather than stale).
- **`gates`** (~6,200 files) — `real` + `framework` + `prettier_fixture`, no perf
  prune: adds Prettier's `tests/format/{typescript,js,css,html}` suites and
  prettier-plugin-svelte's `test/` (`.html` treated as Svelte, files with a companion
  `options.json` skipped) — deliberately tricky edge cases. The reproducibility split
  is enforced downstream (see above), not by dropping files from the view. The
  correctness gates (`corpus:compare:*` `--all`, `skip_triage`, `wasm_json_probe`)
  keep this scope, since their sanction lists and documented-divergence coverage were
  reviewed against it. The `DevReposLoader` view is required at every construction
  site — the view decides what a number or gate verdict means, so there's no implicit
  default to inherit by accident.
- **`conformance`** — the hard parse cases only: the `prettier_fixture` suites + the
  parse-conformance `suite` entries — Svelte's compiler tests (with the gate-aligned
  skips: `_`-prefixed segments, `migrate/`, `output.svelte` snapshots), the wpt-css
  harvest cache, the test262 graded-positive path list (a `files_from` entry), and
  the **tsc-corpus** valid list (another `files_from`, from `harvest_ts_repo.ts`).
  Deliberately **excludes the `real` perf tier**, so the conformance coverage surface
  and the perf corpus are mutually exclusive: perf is the "every in-scope tool must
  fully process it" corpus, conformance is where sub-100% coverage is the metric.
  This is what `deno task bench:conformance` measures.

  **The tsc corpus (`ts_repo_files.json`) is the TypeScript-specific set.** Without
  it the `parse/typescript` group is ~95% test262 — ECMAScript — with prettier's ~800
  format fixtures as its only TS, so a TS parse gap moved the headline by tenths of a
  point. `../typescript/tests/cases/{conformance,compiler}` is the language's own
  corpus, and it is already a release-required, commit-pinned checkout here (the
  `conformance:ts-repo` gate reads its baselines). Its **validity filter is tsc
  itself** — the `typescript` npm package's parser plus tsc's `.errors.txt`
  baselines, both required to call a file well-formed — which keeps the filter
  tool-neutral the way test262's own metadata does for that entry. Unlike test262 it
  is NOT goal-tagged — tsc's module-vs-script reading is semantic and never gates
  syntax, so handing it to parsers that take `sourceType` as a grammar switch costs
  tsv 640 files it and tsc both accept to win back 25. Full rules, the measurement,
  and why the two validity readings must AGREE: `harvest_ts_repo.ts`.

  **Canonical-reject exclusion (Svelte only, conformance view only).** The suite
  bundles deliberately-invalid fixtures (svelte's own `compiler-errors/`, `loose-*`
  error-tolerant fixtures, preprocess inputs) plus non-Svelte HTML (prettier's
  `tests/format/html`), so a raw parse-**coverage** number scores those intentional
  rejects as failures — and makes tsv's *higher* coverage read as superiority when
  it's really tsv's deferred-early-error *permissiveness*. So the conformance view
  excludes the Svelte files `svelte/compiler` rejects (the
  `svelte_parse_rejects.json` cache, loaded by `DevReposLoader` only when `view ===
  'conformance'`). Coverage then measures fidelity on *valid* Svelte:
  svelte/compiler → 100% (it's the oracle), tsv → 100% (the svelte-fixtures gate's
  `KNOWN_GAPS` is empty; a new drop-in gap would read as sub-100% here and get
  tracked there). **Svelte only** — svelte/compiler is the parser tsv is a strict
  drop-in *for*; `acorn-typescript` **trails** modern TS/JS (its rejects include
  valid code tsv correctly parses) and `parseCss` is lenient, so neither is a
  validity oracle and TS/CSS get no reject cache. The cache is machine-local +
  regenerable (gitignored); absent = fail-open to the un-filtered corpus (disclosed
  in the load log). The **`gates` view is untouched**, so `corpus:compare:*` /
  `skip_triage` still see the error fixtures they need.

**Missing entries fail fast** — the loader checks every entry up front and throws
listing the missing paths, so a partial checkout can't silently shrink a perf number
or let a correctness gate pass while grading less than it claims. The only
exceptions: the four derived harvest caches are `optional` (warn-and-skip —
wpt/test262/ts-repo because their source checkouts are legitimately
machine-dependent, matching those harvests' `--if-present` posture; svelte_styles
because it's generated from the always-required dev repos and just may not have been
harvested yet), and `BENCH_ALLOW_MISSING=1` opts the bench into a partial corpus explicitly.
Reports carry `corpus_sources` so any tolerated gap is disclosed rather than
invisible.

## Architecture

```
benches/js/
├── package.json           # npm dep source of truth (both runtimes); install_deps drives it
├── rsvelte.oxfmtrc.json   # the two rsvelte-fmt options with no CLI flag (quotes, trailing
│                          # commas); passed with `--config`. NOT named `.oxfmtrc.json` — that
│                          # exact name is what oxfmt/rsvelte-fmt discover by walking up, so a
│                          # real one here would reach into every oxfmt-backed row
├── package-lock.json      # npm lock (committed for reproducibility)
├── deno.json              # nodeModulesDir: manual + lock: false (npm from package.json)
├── install_deps.ts        # `bench:install`: npm install + force-fetch the oxc wasi binding
├── harvest_test262.ts     # `bench:harvest:test262`: graded positives → .cache (Deno-only)
├── harvest_ts_repo.ts     # `bench:harvest:ts-repo`: the tsc corpus's valid + rejects lists →
│                          # .cache (Deno-only; tsc itself is the validity oracle)
├── bench.ts               # Benchmark entry point (runtime-neutral)
├── conformance.ts         # Single-process pre-release aggregate driver: all six legs, one
│                          # module cache
├── smoke.ts               # Smoke test for formatters and parsers (runtime-neutral)
├── compose_reports.ts     # Fold report.{deno,node,bun}.json → combined report.{json,md}
├── idempotency_sweep.ts   # F1 sweep over the `perf` view (drives tsv_debug `fuzz --iterations 0`)
├── corpus_audit.ts        # `audit:corpus`: the pure-Rust content-loss / robustness legs over
│                          # real code (§Gate map)
├── corpus_compare_format.ts  # Formatting comparison vs prettier (Deno-only entry point)
├── corpus_compare_parse.ts   # Parse/AST comparison vs canonical parsers (Deno-only entry point)
├── divergence_audit.ts    # Divergence audit entry point (Deno-only)
├── diagnostics/           # diagnostic scripts — see §Diagnostic scripts
├── results/baseline.json  # Saved baseline for regression detection (gitignored)
└── lib/
    ├── binary_sizes.ts    # Binary/WASM size collection and reporting
    ├── biome.ts           # Biome WASM wrapper (Svelte, TypeScript, CSS)
    ├── canonical.ts       # Prettier + Svelte parser wrappers
    ├── check_artifact_freshness.ts # Native/WASM artifact staleness guard (§Artifact Freshness Guard)
    ├── check_node_modules.ts # node_modules preflight: exists + every exact pin (and the oxc wasi binding) matches installed
    ├── compare_cli.ts     # Shared scaffolding for the corpus_compare_* entry points
    ├── corpus.ts          # DevReposLoader + DirectoryLoader (load/stream; node: builtins)
    ├── corpus_repos.ts    # Per-source repo origin + commit, DETECTED from each checkout, so the
    │                      # report's source links pin to the measured code
    ├── diff.ts            # Line-based diff utilities (LCS algorithm)
    ├── dprint.ts          # dprint WASM wrapper (TypeScript/JS only; the engine `deno fmt` runs)
    ├── ffi.ts             # Deno.dlopen bindings (NativeImplementation — Deno native)
    ├── fixtures_gate.ts   # Shared per-language parse-conformance gate engine
    ├── format_config_probe.ts # Behavioral "did the pinned layout config LAND" check —
    │                      # one probe source + grading arm PER LANGUAGE, shared by prettier
    │                      # (the baseline) and the format impls with no config-diagnostic
    │                      # channel (biome, oxfmt); unit-tested by format_config_probe_test.ts
    ├── gate_counts.ts     # Pinned gate counts — see ../../docs/gate_counts.md
    ├── harvest_stamp.ts   # Harvest freshness stamps (source commit + pins)
    ├── implementations.ts # Implementation registry (branches native FFI vs N-API by runtime)
    ├── malva.ts           # malva WASM wrapper (CSS only; dprint's CSS plugin, shared formatter host)
    ├── napi.ts            # process.dlopen bindings (NapiImplementation — Node/Bun native)
    ├── oxc.ts             # OXC native wrappers (oxc-parser + oxfmt)
    ├── oxc_wasm.ts        # OXC WASM wrapper (oxc-parser via wasm32-wasi; per-runtime entry)
    ├── parse_sanctions.ts # Shared parse-parity vocabulary: Sanction (keep) + KnownGap (fix)
    ├── perf_omit.ts       # PERF_OMITS — the only excused per-file failures on the perf view
    ├── postcss.ts         # postcss wrapper (parse-only, CSS — the parser behind prettier's CSS printer)
    ├── prettier_cache.ts  # Content-addressed prettier-output cache for the format comparison
    ├── reject_probe.ts    # Behavioral "does this binding still REPORT a rejection" check,
    │                      # shared by tsv's three front-ends (FFI decides by error-envelope
    │                      # prefix, so a changed envelope would fabricate 100% coverage)
    ├── report.ts          # Summary report generation
    ├── rsvelte.ts         # rsvelte-fmt wrapper (Svelte only; COVERAGE-ONLY, never timed)
    ├── rsvelte_parse.ts   # rsvelte PARSE wrapper (N-API addon — a DIFFERENT package from rsvelte.ts,
    │                      # and unlike it in-process, so it IS timed; 2 rows on parse/svelte)
    ├── runtime.ts         # Cross-runtime helpers: current_runtime / os / arch normalizers, plus the
    │                      # artifact-naming pair every loader AND guard shares — native_library_filename
    │                      # and wasm_target (the pkg/<variant>/<target>/ segment)
    ├── swc.ts             # swc wrapper (parse-only, TS/JS; both surfaces — goal axis is `isModule`)
    ├── ts_repo.ts         # Shared `../typescript`-corpus vocabulary: discovery + the baseline
    │                      # key/grammar-error rules (the ts-repo GATE and the harvest both read it,
    │                      # so they can't drift on what a parse unit is or what tsc's baselines say;
    │                      # they scope themselves along two DECLARED axes — root + DeclarationPolicy)
    ├── tsc.ts             # tsc wrapper (parse-only, conformance surface only) + the shared
    │                      # `typescript` loader and parse call the harvest reuses
    ├── types.ts           # Shared types + `BaseImplementation` (the language-support pair)
    ├── versions.ts        # Version loading from package.json
    ├── wasm.ts            # WASM module loader (WasmImplementation — deno/nodejs target)
    ├── yuku.ts            # yuku-parser wrapper, BOTH bindings from one class (parse-only)
    └── divergence/        # Divergence detection module
        ├── mod.ts         # Main exports
        ├── safety.ts      # Safety check (differential char-frequency vs prettier)
        ├── patterns.ts    # Known divergence pattern detectors (with traceability)
        ├── panic_errors.ts    # Native-panic classification (shared by both corpus tools)
        ├── expected_errors.ts # Expected-error fixtures (parse-rejection cases)
        └── validation.ts  # Audit: cross-ref patterns vs conformance_prettier*.md
```

## Error Tracking

Benchmark failures are recorded during the up-front pre-flight pass (each task runs
once per file untimed). The timed loop then iterates the pre-filtered intersection
(or per-impl success set under `BENCH_MODE=union`), so throws during measurement
would be real bugs — they're allowed to propagate rather than being silently
catalogued.

Two surfaces summarize what was skipped: the **effective corpus report** (per-benchmark
coverage rate, e.g. `⚠ biome 500/660 files (76%)`) and the **skipped files report**
(total + per-benchmark counts, always shown). Per-file detail (paths, error messages,
failure sets) is opt-in via `--verbose`, since most universal-tsv failures are
unsupported-syntax fixtures (SCSS in `.css`, JSX in `.js`, early-stage proposals).
When verbose, entries sort ascending by failure-set size so rare / impl-specific
failures land at the top, and the `Failed in:` line collapses to `all tsv variants`
when the failure set matches the canonical 6-element pattern. All labels use display
names (`tsv-json`, `acorn-typescript`) rather than internal trackingKeys. If an impl
fails on many files (e.g. WASM panics corrupting internal state), the coverage report
and skip counts make it visible without `--verbose`.

## Known Issues

- **Corpus SAFETY robustness under `--all` load.** The safety check is differential
  vs prettier — it iterates the characters _ours_ deviates on and uses prettier only
  as a subtrahend — so the two sides can only fail in **opposite** directions, and
  each is guarded in-harness:
  1. **Native-side corruption would fabricate a violation** (only `ours`-side
     corruption can fake a loss). `lib/ffi.ts` uses explicit `pointer` params +
     persistent externalized marshalling buffers, and every SAFETY finding is
     **self-verified at the verdict**: `corpus_compare_format.ts` re-runs the native
     format and requires byte-identity before recording it — corruption surfaces as a
     loud per-file `native format nondeterminism` error, never as a silent SAFETY
     count.
  2. **A prettier empty-output miss would mask a violation** (never fabricate one —
     an empty `prettier` inflates `prettier_excess`, which only cancels `ours`'s
     deltas). The in-process prettier (`lib/canonical.ts` — a separate host from the
     `tsv_debug` Rust sidecar) can intermittently return empty output under load;
     guarded three ways: `corpus_compare_format.ts` errors on semantically-empty
     prettier output for non-empty source; the prettier cache neither stores nor
     returns semantically-empty entries; and the Rust sidecar's `run_prettier`
     returns a hard `DenoError::EmptyOutput` instead of `Ok("")`. Deliberately **no
     retry** anywhere: a flaky oracle must stay loud.

  **Triage:** a SAFETY finding reproduces by construction (two in-run native runs
  agreed), so treat it as real; confirm root cause with the **native CLI** (`tsv
  format <file>` is deterministic) and diff semantic chars vs prettier. A `native
  format nondeterminism` or prettier-miss **error** is the environment acting up —
  re-run to clear it, and investigate if it persists. For "did my change regress?",
  diff the sorted `.safety[].path` lists before/after (a real regression is a _new
  path_, not a count bump); a change scoped to one printer/crate can't lose content
  in unrelated languages.
- **Parse benchmark overhead**: JSON materialization, not parsing, dominates the
  `-json` rows (see `results/report.<runtime>.md` for current ratios). Use
  `tsv-internal` for raw parse speed. Both the native and WASM rows go through
  `convert_ast_json_string` — the wire-JSON writer emitting directly from the
  internal AST in one walk, no intermediate `serde_json::Value` or typed public tree
  ([../../docs/architecture.md §Closed Scope, Open
  Convention](../../docs/architecture.md#closed-scope-open-convention)). They differ
  only at the boundary: native crosses via FFI copy + `JSON.parse` in JS; WASM
  decodes the string across the boundary and runs the engine's `JSON.parse` from Rust
  via `js_sys` (measurably faster than a `serde_wasm_bindgen`-built object graph).
  Rust-side parse-vs-write timing: `cargo run --release -p tsv_debug -- json_profile
  <paths>`; `wasm_json_probe.ts` covers the end-to-end view including the JS boundary.
- **The yuku-parser N-API binding SEGFAULTS on long braced-escape identifiers, so
  its row is conformance-excluded.** An identifier built from a run of braced unicode
  escapes faults the host process inside the Zig parse call once the decoded
  identifier passes ~300 bytes — `parse('var _' + '\u{11A01}'.repeat(75) + ';')`
  crashes, `repeat(74)` throws an ordinary `ParseFailed`. Non-braced escapes
  (`\uXXXX`) and literal non-ASCII identifiers are unaffected at any length, and so
  is the wasm binding (the overrun stays inside linear memory; it parses the same
  inputs cleanly — itself a variant-parity divergence, except the process dies before
  `check_variant_parity` can report it). test262's
  `language/identifiers/part-unicode-*-{,class-}escaped.js` are exactly this shape,
  so the conformance corpus kills the whole run mid-preflight; the perf corpus has no
  such identifiers. A skip list is not a workaround: **which** files of that family
  fault is heap-layout dependent (a sweep skipping the 17 observed crashers faulted
  on an 18th that had survived it), so the screen is neither cacheable nor
  reproducible. Hence the row — not the files — is dropped on that surface
  (`get_benchmark_tasks`, keyed on `BenchmarkTaskOptions.corpus_kind`), disclosed in
  the conformance report's `**Excluded here:**` line — a disclosure whose claim is
  CHECKED against the registry (§Report files), so re-adding the row without
  updating the table fails the run. Revisit on a yuku bump: re-add the row and run
  `deno task bench:conformance`. The fault survives at the pinned version — the
  `repeat(74)`/`repeat(75)` boundary above reproduces exactly as written — so the
  exclusion still earns its place; re-probe rather than assume on the next bump.
- **The oxc WASI binding's `errors` getter is CONSUME-ONCE.** On
  `@oxc-parser/binding-wasm32-wasi`, the first access to `result.errors` returns the
  real error array; every later access returns `[]` (the native `oxc-parser` package
  caches, so only the WASI path behaves this way). Any double-access check
  (`result.errors && result.errors.length`) therefore never fires — invalid input
  silently yields an empty `Program` (`end: 0` inside the `{node, fixes}` wrapper)
  and counts as parsed, which once fabricated a 100% `oxc-parser-wasm`
  conformance-coverage row while native oxc-parser correctly rejected 245 files.
  Rule: read getter-backed napi-WASI result fields **once into a local**
  (`lib/oxc_wasm.ts` does; `lib/oxc.ts` mirrors the form defensively). Two guards
  exist: the single-read pattern at the wrappers, and `bench.ts`'s
  `check_variant_parity` — after pre-flight, same-engine pairs
  (tsv↔tsv_wasm variants, oxc-parser↔oxc-parser-wasm, yuku-parser↔yuku-parser-wasm,
  rsvelte-parse↔rsvelte-parse-skip-expr-loc) are compared file-for-file and
  any accept-set divergence prints a `⚠ variant parity` warning (same engine ⇒ a
  divergence is a binding-boundary bug, not an engine difference).
  ⚠️ **The tsv pair is graded on a second, STRONGER axis, and that one is FATAL**: an
  accept set only records whether a file threw, so it is blind to two bindings of one
  engine returning different CONTENT. For the rows `sibling_outputs_must_match` names
  (tsv's own — the third-party pairs keep the warning, and rsvelte's option pair is
  excluded because its variant drops payload by design), pre-flight digests each
  output and a byte divergence over the files both accepted exits non-zero. Under Node
  and Bun the native row IS the N-API addon on the `napi` profile, which makes this
  the standing correctness check on the artifact the native npm packages ship.
- **The oxc WASI binding also CAPS the `oxc-parser` pin.** It is force-fetched in
  lockstep with `oxc-parser` (§Cross-Runtime), so the pin decides which binding
  installs — and past the version named in `package.json`'s `//oxc-wasi` note the
  binding fails to load under both Deno and Node (`this.bridge.setLastError is not a
  function`: it declares an alpha `@emnapi/core` that npm installs NESTED, while the
  hoisted `@napi-rs/wasm-runtime` it also imports resolves the hoisted 1.x, so the
  two halves disagree). Same shape as the two per-runtime load failures above but on
  the VERSION axis, and equally silent in the TABLES: an unloadable impl is absent,
  not fatal, so the row simply leaves both surfaces — only `unavailable` records the
  cause. Probe the CANDIDATE binding before raising the pin
  (../../docs/benchmarks.md §Updating dependencies carries the commands — a bare
  import resolves the INSTALLED binding and so always passes); hoisting
  the alpha to the tree root works and was deliberately declined.
- **TypeScript canonical parser**: acorn-typescript fails on some modern syntax
  (files skipped) — and the reverse, files tsv fails that acorn accepts, is a known
  parse gap.
- **prettier-plugin-svelte verbatim fallback**: when the embedded formatter throws on
  any construct in a `<script>` block (e.g. `@(a?.b)()` decorators crash prettier's
  typescript parser), the plugin emits the whole block verbatim — a corpus diff on
  such a file is prettier's error fallback, not a real style divergence. See
  ../../docs/conformance_prettier.md §Tooling for the triage procedure.
- **oxfmt × Deno timer interaction (workaround in place)**: once `oxfmt.format` runs
  once, Deno's timer wheel processes exactly one further `setTimeout` callback and
  then stalls all subsequent timers indefinitely. Repro: `await
  import('oxfmt').then((m) => m.format('file.ts', 'x=1', {useTabs:true}))` followed
  by two `new Promise((r) => setTimeout(r, 50))` — the first resolves, the second
  never does. Independent of oxfmt version (reproduced with 0.28.0, 0.50.0, 0.53.0,
  0.57.0 on Deno 2.8.3), so the regression is on the Deno / napi-rs side; re-test the
  repro before ever removing the workaround. In `bench.ts` oxfmt is invoked
  per-iteration during the `format/*` loops; the leak shows up at the next inter-task
  `await wait(cooldown_ms)`, which never fires. Workaround: `cooldown_ms: 0` in
  `run_benchmark_group`'s `Benchmark` config. Async measurement loops (`prettier`,
  `oxfmt` itself) are unaffected because their per-iteration awaits resolve via
  microtasks, not timers. The inter-task SETTLE the cooldown used to supply is not
  lost with it: each task's untimed `setup` forces a major GC (`settle_heap`), which
  is timer-free and uniform across the three runtimes — a runtime-conditional
  cooldown would put a settle under Node/Bun and none under Deno, biasing the very
  cross-runtime ratios this design exists to read (../../docs/benchmarks.md
  §Fairness caveats).
- **wasm-opt** runs with explicit feature flags in `crates/tsv_wasm/Cargo.toml` —
  Rust 2024's bulk-memory and nontrapping-float-to-int ops, plus the simd128 and
  multivalue features the `.cargo/config.toml` rustflags enable, are passed by name
  to wasm-opt v117 (it rejects instructions whose features aren't named), giving
  ~−2% gzipped on the WASM bundle.

## Diagnostic scripts

These live under `diagnostics/`. Most are ad-hoc, not wired into `deno task`. Each
module's own doc comment is the full reference (rationale, findings, exact run
command); the table is the index. Some import the canonical parser / oracle
(`acorn`, `svelte/compiler`, `oxc-parser`) by bare specifier, so pass `--config
benches/js/deno.json` to resolve them from `node_modules`; all run from the repo
root (corpus/artifact paths are CWD-relative). The usual permission set is
`--allow-ffi --allow-read --allow-env --allow-net --allow-sys`.

**Any diagnostic that calls `init_implementations` needs BOTH tsv artifacts built**
— the runtime's native binding *and* its `pkg/all/<target>` WASM bundle — because
those two plus `canonical` are REQUIRED and a load failure in any of them throws
(§Report files). That includes the ones that measure only the native path
(`no_locations_parity`, `reconstruct_vs_materialize`, `skip_triage`), where an
unbuilt bundle otherwise fails a run that would never have touched it, with a WASM
error naming nothing the script is about: `deno task build:ffi && deno task
build:wasm:all:deno` first (these three read the `release` FFI, not `corpus`). The
two with `deno task` entries — `css:over-acceptance` and `ts-repo:over-acceptance`
— build what they need themselves.

Six live here but are documented above: the parse-conformance gates
(`svelte_fixtures_compare.ts`, `ts_fixtures_compare.ts`, `ts_repo_compare.ts` →
[§Parse-Conformance Gates](#parse-conformance-gates)) and the harvests
(`wpt_css_harvest.ts`, `svelte_reject_harvest.ts`, `svelte_styles_harvest.ts` →
[§Harvests](#harvests)).

| Script | What it does | Task |
| --- | --- | --- |
| `corpus_stats.ts` | corpus/candidate-dir size + language + degenerate-case stats (reuses `lib/corpus.ts` filters via `stream_perf_candidate`) | `corpus:stats` |
| `skip_triage.ts` | parse-**parity** gate: buckets every corpus file by *asymmetry* (`parity` / `sanctioned_over_rejection` / `over_acceptance` / `unexpected_over_rejection`), exiting 1 only on the last. Takes an optional corpus dir (defaults to the dev repos); point it at Svelte's adversarial `tests/` for the residual gap list | — |
| `test262_compare.ts` | test262 differential, tsv vs oxc-parser, from `tsv_debug test262 --emit-manifest`. Surfaces positive tsv real-bug candidates + negative early-error gaps; numbers move with the pinned oxc version. No biome — its js-api has no parser to grade. See `docs/conformance_test262.md` §Differential | — |
| `css_over_acceptance.ts` | the `parse/css` sibling of the row below — per-tool accepts over the files `svelte/compiler`'s `parseCss` rejects, computed live from the conformance corpus (no harvest cache: nothing else consumes the list). Its reason to exist is sharper than the TS one's, because CSS is the surface that CANNOT filter to valid inputs — `parseCss` accepts malformed CSS and rejects valid modern CSS it doesn't implement, so filtering would drop files tsv also fails. The reject count is PINNED (`CSS_REJECTS_PIN`), which is what makes the reference row's grammar moving visible instead of silently reshaping the published coverage. The `svelte/compiler` row must read 0 (it built the list) | `css:over-acceptance` |
| `ts_repo_over_acceptance.ts` | per-tool OVER-ACCEPTANCE over the tsc corpus — the files tsc's own PARSER rejects (`.cache/ts_repo_rejects.json`). The axis coverage structurally cannot show: coverage counts accepts, so it can only reward permissiveness, and every conformance corpus is therefore filtered to VALID inputs. Read inverted (lower is better) and as a PROFILE, not a gate — a deferred early error is a documented tsv posture, and the per-file gate on tsv alone is `conformance:ts-repo`. The `tsc` row must read 0 (it built the list); anything else fails the run as a stale cache | `ts-repo:over-acceptance` |
| `biome_oxfmt_diff.ts` | 4-way formatter differential (tsv vs prettier vs biome-wasm vs oxfmt) so a tsv-vs-prettier divergence can be bucketed *tsv alone* (candidate bug) vs *tsv + another agree* (candidate sanctioned divergence). Prettier is routed through the **typescript** parser, never babel | — |
| `no_locations_parity.ts` | proves the `no-locations` wire is losslessly reconstructible (TS exact; two Svelte non-derivable cases classified, not failed). The reference reconstruction a consumer would use | — |
| `reconstruct_vs_materialize.ts` | its **perf** sibling: is it faster to materialize `loc` in Rust or reconstruct it in JS? (Finding: reconstruct wins.) Feeds the committed report's consumer-side note | — |
| `wasm_json_probe.ts` | splits parse cost into pure-parse vs materialization for native + WASM, isolating JS-side `JSON.parse` | — |
| `wasm_format_probe.ts` | WASM **format** wall-time A/B at single-digit-% resolution (paired discipline: interleaved pairs, in-run A/A noise floor, byte-identity gate) | — |
| `wasm_memory_probe.ts` | WASM **linear-memory high-water** for `format()` — the axis the wall-time probe can't see, and the gate for doc-IR memory work. `--cold` (per-file cold-start peak) or default steady-state | — |
