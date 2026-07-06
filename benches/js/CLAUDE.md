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
  has a top-level `runtime` and `version: 6`. `deno task bench:compose` (run at
  the end of `deno task bench:perf`) then folds the siblings into a compact combined
  `results/report.{json,md}` — the cross-runtime view tsv.fuz.dev consumes
  (`compose_reports.ts`; a per-runtime delta on a row is the headline). The
  composer records per-source provenance (runtime, commit, timestamp, tsv
  version — in the JSON `sources[]` and the md header) and flags **mixed
  vintages** loudly (md banner + stderr + `mixed_vintage` in the JSON) when the
  siblings come from different commits/versions — it folds whatever exists, so
  a fresh `report.deno.*` next to a stale `report.node.*` would otherwise read
  as a runtime effect. It also annotates any row whose per-runtime
  intersections differ (`⚠ files a/b/c`) — each runtime times the files *its*
  impls passed preflight on, so unequal counts mean a sliver of the ratio is
  file-set, not runtime. The
  conformance surface (`BENCH_CORPUS=conformance`) writes its own
  `report.conformance.node.*` (coverage-only + node-only — see §Corpus), outside
  the compose glob.
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
`bench:install`. Every harness entry point (bench, smoke, the corpus/conformance
tools) preflights `node_modules` via `lib/check_node_modules.ts`: missing is
fatal with the installer hint, and **stale** — `package.json` newer than npm's
`.package-lock.json` install stamp, i.e. pins bumped without a reinstall — is
fatal too (`BENCH_STALE_OK=1` downgrades stale to a warning), so a run can't
silently measure old installed versions under new labels.

**oxc-parser-wasm runs under Deno and Node.** Its binding ships two entries — a
fetch-based browser entry (`parser.wasi-browser.js`) that Deno needs, and the
default `node:wasi` entry that Node needs — so `oxc_wasm.ts` picks the right
one per runtime (`current_runtime()`). Under Bun the `node:wasi` entry fails to
load, so the Bun report has no oxc-parser-wasm row (same class as the biome-wasm
Bun-load issue). (Node also has oxc-parser **native**, the more relevant Node
number, regardless.)

## Corpus Comparison

Compare formatting output against Prettier on arbitrary codebases:

```bash
# The gates corpus view (~6,000 files: real repos + prettier suites — see §Corpus)
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

**Prettier-output cache.** The format comparison's dominant cost is prettier
over ~6k mostly-unchanged files, so its oracle calls go through a
content-addressed cache (`lib/prettier_cache.ts`, `.cache/prettier/`): keyed on
the source content + parser/filepath routing + the full options + the
canonical-5 pins (incl. svelte, the plugin's peer) + `PRETTIER_DEBUG` + a
schema constant — a hit is exactly equivalent to a live run. Success-only
(errors and semantically-empty outputs never cached — `put` rejects
whitespace-only, and `get` treats a stored whitespace-only entry as a miss —
so the prettier-miss heisenbug can't poison it, and cached hits remove the
prettier-side flake from repeat runs entirely; the tsv/FFI side stays live).
The run reports `prettier cache: N
hits / M misses`. Scope: this tool + the conformance driver only — never the
bench (it times prettier), never the fixture validator (live by design).
`TSV_PRETTIER_CACHE=0` disables; `deno task bench:clean` wipes.

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
cases and the wire-JSON writer is the sole emission path, so a writer bug
(e.g. an untranslated position field) on an uncurated shape is invisible
without a canonical-parser comparison at corpus scale.

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

## Svelte-Fixtures Parse Conformance

`deno task conformance:svelte-fixtures` runs tsv's Svelte parser against
**Svelte's own compiler test suite** (`../svelte/packages/svelte/tests`) — the
drop-in-parser analog of test262 (JS) and the WPT harness (CSS). It's a
periodic (non-`check`) gate; `diagnostics/svelte_fixtures_compare.ts` is the
entry.

```bash
deno task conformance:svelte-fixtures            # builds corpus FFI, then runs
deno task conformance:svelte-fixtures:run        # skip rebuild (freshness-guarded)
deno task conformance:svelte-fixtures:run -v     # + per-file known-gap / AST-group detail
deno task conformance:svelte-fixtures:run --json 2>/dev/null > report.json
deno task conformance:svelte-fixtures:run ../svelte/packages/svelte/tests/parser-modern  # a subtree
```

**Oracle = the LIVE modern Svelte parser** (`svelte/compiler` `parse(src,
{modern:true})`), not the committed fixture artifacts — `parser-legacy`'s
`output.json` is the *legacy* AST, `compiler-errors/_config.js` encodes
*compiler* (analysis-stage) verdicts, and `css` ships compiled CSS, so none is a
correct oracle for a drop-in *modern-parser* replacement. Using the live modern
parser also makes the two trap partitions resolve for free: `loose-*` inputs
throw under the non-loose oracle (→ parity), and analysis-stage `compiler-errors`
parse fine on both sides (→ never miscounted as a tsv bug).

**Scope**: the canonical `.svelte` INPUTs (`input.svelte`/`main.svelte`/
`index.svelte`), skipping generated `_`-prefixed artifacts, `output.svelte`
dups, and the `migrate/` tree (Svelte-4 migrator inputs, not modern-parse
targets). `.svelte.js`/`.ts`/`.css` are out of scope (test262 / wpt cover those).

Two comparisons per input:

- **Verdict parity** (the enforced gate) — buckets over-rejections (tsv rejects
  what Svelte accepts) into `SANCTIONED` (tsv correctly stricter; input is
  invalid Svelte the parser is merely lenient about), `KNOWN_GAPS` (tsv wrong; a
  tracked drop-in gap that must only shrink), and `unexpected` (a NEW gap —
  **exits 1**). Both lists are reviewed in-file allowlists in
  `svelte_fixtures_compare.ts`. `over_acceptance` (tsv accepts, Svelte rejects) is
  a deferred early-error, reported not gated. Green at baseline = every gap is
  sanctioned or tracked.
- **AST-shape** (report-only) — for inputs both accept, deep-diffs tsv's wire AST
  vs the Svelte AST via the SHARED `corpus_compare_parse.ts` engine (`diff_asts`
  + `DOCUMENTED_MATCHERS`, imported — that module is now `import.meta.main`-guarded
  so importing it doesn't run the CLI). The adversarial fixture tree exposes many
  edge divergences; triaging each into the shared `DOCUMENTED_MATCHERS` (which
  also shrinks the `corpus:compare:parse` count) or fixing it as a writer bug is a
  tracked campaign, so this half does **not** gate yet.

## TypeScript-Fixtures Parse Conformance

`deno task conformance:ts-fixtures` runs tsv's TypeScript parser against
**acorn-typescript's own test suite** (`../acorn-typescript/test`, ~200
adversarial `input.ts` fixtures) — the TS analog of the Svelte gate above (and of
test262 / WPT). tsv is a drop-in for acorn + acorn-typescript, so that parser's
own regression corpus is the natural TS edge-case oracle: the shape real-world
code (`corpus:compare:parse`) can't reach. Periodic (non-`check`) gate;
`diagnostics/ts_fixtures_compare.ts` is the entry.

```bash
deno task conformance:ts-fixtures            # builds corpus FFI, then runs
deno task conformance:ts-fixtures:run        # skip rebuild (freshness-guarded)
deno task conformance:ts-fixtures:run -v     # + per-file known-gap / AST-group detail
deno task conformance:ts-fixtures:run --json 2>/dev/null > report.json
deno task conformance:ts-fixtures:run ../acorn-typescript/test/class_accessor  # a subtree
```

**Oracle = the LIVE `@sveltejs/acorn-typescript` parser** (pinned in
`package.json` / `sidecar.ts`), not the committed `expected.json` artifacts — same
reasoning as the Svelte gate: a committed artifact can drift from the pinned
version that defines fixture correctness, and the live parser is exactly what
`corpus:compare:parse` diffs against, so the two stay consistent by construction.

**Shared gate hygiene (both fixtures gates, `lib/fixtures_gate.ts`).** The suite
INPUTS come from the sibling checkout while the grading parser is the pinned npm
oracle, so full-suite runs compare the checkout's `package.json` version against
the pin and **warn on skew** (non-fatal — a checkout tracking upstream main is
legitimate, but silent divergence isn't). Full-suite runs also **freshness-check
the ledgers**: a `SANCTIONED`/`KNOWN_GAPS` entry that matched no over-rejection
fails the run (delete it when its gap is fixed; update the pattern on an upstream
rename) — the same mirror-the-live-corpus discipline as `scan_audit`'s ALLOW list.
Subtree runs skip both checks.

**Scope**: every `input.ts` under the suite root (the `*.test.ts` / `utils.ts`
harness files are excluded by basename). `.tsx`/JSX fixtures parse as ordinary
`.ts` here — tsv and acorn (module mode, no JSX plugin) both reject them, so they
land in `parity`. Strict about setup: a missing `../acorn-typescript` checkout
(0 scanned) **FAILS** — a run that graded nothing must not read as a pass. The
tolerance point for machines without the checkout is `publish.ts` Step 3b's
preflight probe, which skips the whole aggregate before the gates run (warn on
dry-run, **blocking on `--wetrun`** — only `--no-check` releases without gates).

Two comparisons per input, same structure as the Svelte gate:

- **Verdict parity** (the enforced gate) — over-rejections bucket into
  `SANCTIONED` (tsv over-rejects *deliberately* — deprecated syntax it declines,
  e.g. import assertions `assert {…}`, or input its own grammar rejects;
  `TS_FIXTURE_SANCTIONS` in `lib/parse_sanctions.ts`), `KNOWN_GAPS` (tsv wrong; a
  tracked drop-in gap that must only shrink; in `ts_fixtures_compare.ts`), and
  `unexpected` (a NEW gap — **exits 1**). `over_acceptance` (tsv accepts, acorn
  rejects) is a deferred early-error, reported not gated.
- **AST-shape** (report-only) — for inputs both accept, deep-diffs tsv's wire AST
  vs the acorn AST via the SHARED `corpus_compare_parse.ts` engine. Unlike the
  Svelte tree's large backlog, this corpus is near-clean, so promoting AST-shape
  to a gate once the undocumented-group count hits 0 is a natural follow-up.

**Broadening — `conformance:ts-repo` (the official `typescript` compiler corpus).**
`deno task conformance:ts-repo` (`diagnostics/ts_repo_compare.ts`) runs tsv's TS
parser over `../typescript/tests/cases/conformance/parser` (~800 single-file `.ts`)
using **tsc's OWN baselines as the validity oracle** — a `tests/baselines/reference/<name>.errors.txt`
with a `TS1xxx` code = tsc's parser rejects (→ tsv correctly stricter), no `TS1xxx`
= tsc accepts (→ a tsv reject is a real gap). tsc is authoritative because acorn-ts
(tsv's *target*) is itself over-lenient; using tsc's baselines auto-resolves those
leniency cases to reject-parity (no sanction needed), and acorn's verdict sub-labels
each gap (`gap` = acorn-confirmed → gates; `gap_beyond_acorn` = acorn also rejects, a
mixed acorn-gap / early-error-timing surface → reported, not gated). **In the blocking
`conformance` aggregate** (promoted once its baseline hit 0 untracked gaps), tracked
SEPARATELY from the acorn-suite gate (own `KNOWN_GAPS`). `.tsx` and `@filename`
multi-file tests are skipped. Baseline: 768 scanned, 0 untracked gaps.
Setup posture: strict — a missing `../typescript` checkout, a partial checkout
(baselines or corpus subtree missing), or an empty scan all FAIL rather than
green-skipping (the baselines are the oracle; publish Step 3b's probe is the
tolerance point). Full-corpus runs freshness-check `KNOWN_GAPS` (stale entries fail).

**Pre-release aggregate — `deno task conformance`.** The three parse-conformance
gates (svelte-fixtures, ts-fixtures, ts-repo), plus
`corpus:compare:parse --all` and `corpus:compare:format --all`, are the
release-cadence correctness gates that run against external oracles (and so can't
live in `deno task check`). `deno task conformance` builds the corpus FFI once and
runs all five legs in **ONE process** (`conformance.ts`, the driver): the canonical
oracle modules (prettier, the svelte plugin, svelte/compiler, acorn, acorn-ts) load
once via the module cache instead of once per leg, each leg gets a timing line, and
failure semantics match a `&&` chain exactly (every leg exits the process on a
finding — fail-fast). The driver takes no arguments; the per-leg tasks remain the
scoped/triage entries. It is wired into `scripts/publish.ts` **Step 3b**
(skipped by `--no-check`). Step 3b preflights the oracles (`../svelte`,
`../acorn-typescript`, `../typescript`, this dir's `node_modules`): a missing one
**FAILS a `--wetrun`** (only the explicit `--no-check` releases without gates),
warn-and-skips a dry-run, and any skip is re-warned in the run's final summary.
The gates themselves fail closed on a missing checkout (0 scanned = FAIL), so a
manual `deno task conformance` can't green-skip a leg. `corpus:compare:format` there gates on **SAFETY**
(data loss) — the ~8% intentional style divergences are non-blocking WARNs, and
every SAFETY finding is self-verified in-run (the native format is re-run and
must reproduce byte-identically; nondeterminism surfaces as a loud per-file
error instead — see §Known Issues). Both corpus tools also fail (exit 1)
on a run that compared nothing: an empty scope (`No files found`) or an
every-file-errored / every-file-parse-fail-skipped run is a systemic failure —
sidecar/FFI down or a wrong corpus — never a pass. `test262` (needs `../test262`) and the
CSS-WPT harvest stay manual, outside the automated step.

## Pinned gate counts

The gates and harvests enforce **committed expected counts** so any change in
what gets graded — a gutted or refreshed suite checkout, a discovery bug, a tsv
behavior change, a systemic sidecar/FFI failure eating a whole language — fails
loudly instead of shifting inside a green run. This is
`scripts/validate_artifacts.ts`'s tight-bounds philosophy applied to counts:
every real move in a number is a deliberate, visible edit.

**Where the numbers live:**

- `lib/gate_counts.ts` — every Deno-side count, one per consumer: the fixtures
  gates (`scanned` + `both_accept`), ts-repo (`scanned` + `accept_parity`),
  `corpus:compare:parse --all` (minimum per-language `compared` + EXACT
  per-language tsv-side parse-failure counts), `corpus:compare:format --all`
  (minimum per-language `match` + EXACT per-language `unknown`/`partial`
  counts — the un-triaged divergence backlog is pinned, so a new unexplained
  divergence fails until fixed/cataloged, and a shrink is re-pinned to record
  the win), and the three harvests (wpt block count, test262 positive count,
  svelte-rejects count).
- Rust-side counts are consts in their commands — grep `REGRESSION PIN`:
  test262 (discovered + graded-manifest), `fixtures_validate` (total fixtures —
  protects the primary gate against a discovery collapse), and `swallow_audit`
  (formatted files — closes its vacuous-pass).

**Semantics — three pin categories, chosen per surface:**

- **Exact pins** (mismatch in either direction fails): the fixtures gates,
  ts-repo, test262, and the harvests. Their inputs are pinned checkouts
  (version-gated by `pins:audit`) or deliberately-updated ones, so the counts
  are deterministic — a drop is a regression or gutted input, a rise is a suite
  refresh or behavior change; both must be re-pinned deliberately. No slack:
  slack lets small regressions creep and silently widens after every refresh.
- **Minimums at the exact measured value** (shrink fails, growth passes): the
  two non-deterministic-growth cases. (1) The `corpus:compare:* --all` corpus is
  LIVE dev repos that grow with ordinary work — re-pin to current when touching
  the corpus (e.g. at release) so the minimum stays tight. (2) The
  committed-fixtures audits (`fixtures_validate`, `swallow_audit`): additions
  are ordinary reviewed diffs (a per-fixture-PR counter bump in `deno task
  check` would be pure ceremony), while shrinkage is the discovery regression
  the pin guards.
- **Failure-bucket pins** (exact `!==`, but on the live corpus rather than a
  deterministic input): the `corpus:compare:* --all` triage buckets —
  per-language `unknown`/`partial` divergences and tsv-side parse failures. A
  rise fails until triaged (fix it, add a divergence detector/sanction, or
  consciously re-pin a legitimately-unsupported new corpus file); a drop also
  fails, so a fixed divergence ratchets the pin DOWN deliberately and the win
  stays recorded.

Pins apply only to FULL runs (default suite root, `--all`, default harvest
source) — subtree and filtered runs legitimately grade a slice. Harvest pins
fail **before** writing, so a wrong cache never replaces a good one. CI runs
only the committed-tree pins (`check.yml` is a clean checkout — no sibling
clones); the rest are dev-machine gates at conformance/publish cadence.

**Update ritual** (same as the artifact size bounds): the failure message
prints expected vs got — update the constant + its measured-on comment in the
same change, and say why in the commit. Record the checkout **commit** in the
comment (`git -C ../<repo> rev-parse --short HEAD`) — upstream version files
only bump at release, so the commit is the only precise statement of what a
pin was measured against. When re-pinning after a suite refresh, glance at the
full bucket table, not just the changed number — a count move can mask
offsetting changes (the per-file gates — unexpected over-rejections, stale
ledgers, SAFETY — catch tsv-side regressions independently, but the glance is
cheap). Never re-pin to absorb an unexplained move — that is the regression
the pin exists to catch.

**Why both the pins AND pins:audit's checkout alignment exist:** they guard
different granularities. Checkout alignment compares `package.json` versions —
but an upstream repo's version only bumps at release, so commits landing
between releases change the SUITE without changing the version (proven on day
one: a `../svelte` pull added one test fixture at the same `5.56.4` version —
the count pin caught it; the version check couldn't). Conversely the count
pins can't tell 5.56.1 from 5.56.4 if the counts happen to coincide. Version
alignment catches release-level skew; count pins catch commit-level suite
drift within a version window.

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
# single-type-param arrows stay `<T>` and `.svelte` ones get `<T,>`, and the `.js` →
# babel / `.ts` → typescript parser routing (`.js` preserves a JSDoc `@type` cast,
# `.ts` strips it).
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
The conformance surface (`BENCH_CORPUS=conformance`, coverage-only + node-only)
writes `report.conformance.node.{json,md}` instead — a separate committed
surface that never clobbers the perf reports and is invisible to
`bench:compose` (which globs the exact perf filenames). To publish benchmarks
to tsv.fuz.dev, run `npm run update-benchmarks` in ~/dev/tsv.fuz.dev — its copy
list names these report files exactly, so renaming a report artifact means
updating that script in the same change.

The committed `report.<runtime>.json` (baseline `version: 6`) carries, beyond
timing stats: a top-level `runtime`, `corpus_kind` (`perf` | `conformance` —
which corpus/surface produced it), per-language `corpus` totals,
`corpus_sources` (per-entry loaded file counts + a `by_language`
svelte/typescript/css split summing to `files` — the composition disclosure;
see [Corpus](#corpus)), `versions`, and `binary_sizes` (each with
`gzip_bytes`). A coverage-only conformance report (see §Corpus) has null
`entries[]` timing stats. Each `entries[]` row adds a `runtime` field,
`files_processed`/`files_total` (per-impl preflight coverage — the `Coverage:`
line) and `files_iterated` (the timed set — the `Files (intersection):`
count). A top-level `suppressed_noise` map records silenced third-party stderr
crashes as `{pattern: count}`. `report.<runtime>.md` renders coverage/iterated
as prose; the per-entry numbers and `suppressed_noise` are JSON-only.

```bash
deno task bench:install   # one-time: install harness npm deps (see Cross-Runtime above)

# Run benchmarks (builds the runtime's bench artifacts automatically).
# `bench` regenerates EVERY committed artifact the site consumes: the perf
# surface across all three runtimes + compose, then the node conformance
# COVERAGE report (bench:conformance:run — coverage-only + node-only by design;
# coverage is a pre-flight product and runtime-invariant, and the site reads
# only its coverage counts). It reuses the node artifacts the perf half just built. It
# FAILS FAST if node or bun isn't installed (the `&&` chain stops at the
# missing binary). Deno is the only hard dependency, so if you don't have
# node and/or bun, run the per-runtime tasks you DO have — each writes its
# own report.<runtime>.* sibling, and `bench:compose` folds whatever exists.
deno task bench           # full refresh: perf ×3 + compose + node conformance coverage
deno task bench:perf      # perf surface only: all three runtimes + compose
deno task bench:deno      # Deno only (no node/bun needed)
deno task bench:node      # Node only (needs node)
deno task bench:bun       # Bun only (needs bun; reuses the Node artifacts — N-API + nodejs-target WASM)
deno task bench:compose   # Fold whatever report.{deno,node,bun}.json exist → combined report.{json,md}

# Conformance measurement — per-tool PARSE COVERAGE over the full
# fixtures-included corpus (the `conformance` view; parse groups only, no format
# impls) → report.conformance.node.{json,md}. COVERAGE-ONLY + NODE-ONLY by design
# (BENCH_COVERAGE_ONLY=1): coverage is a pre-flight product, so the timed phase is
# skipped, and it's runtime-invariant (same parser engine — the site folds a tool's
# native/wasm variants into one per-engine row), so one node run is the whole surface.
# Entries carry null timing; no throughput/comparison sections; baseline save/compare
# are no-ops. Skipping the timed phase reclaims a fixed ≥8 full-corpus sweeps/row
# (3 warmup + ≥5 measured) that no consumer reads.
deno task bench:conformance        # harvest + build:bench:node + coverage run
deno task bench:conformance:run    # skip harvest + rebuild (freshness-guarded)
# The timed parse-throughput over this adversarial corpus has no consumer (the site
# reads coverage; `bench:compose` excludes conformance), so no task produces it. To
# investigate it ad-hoc: `BENCH_CORPUS=conformance node benches/js/bench.ts` (coverage
# flag unset) — it overwrites report.conformance.node.*, so re-run bench:conformance:run
# after to restore the committed coverage report.

# Harvest the derived suite caches for the conformance corpus (idempotent;
# warn-and-skip when the source checkout is absent). FRESHNESS-STAMPED
# (lib/harvest_stamp.ts): a harvest whose stamped inputs — the source checkout
# COMMIT(s) + the pinned count + oracle pins — are unchanged skips instantly
# (the test262 leg saves a ~1 min release-mode grade); pass --force after
# changing harvest/grading LOGIC, which the stamp can't see. Chained into the
# bench:conformance build tasks; run standalone after a ../wpt or ../test262
# update — EXPECT the pinned harvest count to trip after a source pull
# (§Pinned gate counts): re-pin in lib/gate_counts.ts deliberately.
deno task bench:harvest            # all three harvests
deno task bench:harvest:wpt        # ../wpt/css <style> blocks → .cache/wpt_css
deno task bench:harvest:test262    # graded positives → .cache/test262_files.json (runs cargo)
deno task bench:harvest:svelte-rejects  # svelte/compiler-rejected Svelte files → .cache/svelte_parse_rejects.json
                                        # (the conformance view excludes these — see §Corpus)

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

# Wipe local-only bench state (gitignored): baseline.json, timestamped
# results pairs, and the harvest caches (benches/js/.cache). Preserves the
# committed `report.<runtime>.{json,md}` / `report.conformance.node.*`
# because the glob is anchored on a leading digit (timestamped files start
# with a year).
deno task bench:clean

# Environment variables (apply to any runtime's :run)
BENCH_LIMIT=5 deno task bench:deno:run
BENCH_FILTER=zzz deno task bench:deno:run
BENCH_DURATION=10000 deno task bench:deno:run
BENCH_WARMUP=10 deno task bench:deno:run
BENCH_MODE=union deno task bench:deno:run
BENCH_CORPUS=conformance deno task bench:deno:run
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
  whose programmatic `format` is an async napi call that may run the native
  work off the JS thread (its `tinypool` dep is CLI-only — `dist/cli.js` —
  not in the `format()` path) — still one thread of compute per file, and
  each call is fully awaited before the next, so no fan-out is exploited. This deliberately excludes the
  multi-core batch throughput a CLI gets formatting many files at once (which
  most of these tools, tsv included, could provide) — that's a different
  benchmark.
- **Different tools produce different output — speed is not conditioned on
  correctness.** The timed work is "produce _this tool's own_ formatting," not
  "produce the same bytes," and no two of these tools emit identical output.
  Every formatter IS configured to the same layout targets to the extent its
  options allow — printWidth/lineWidth 100, tabs, single quotes, no trailing
  commas — for prettier (`canonical.ts` `PRETTIER_OPTIONS`), oxfmt
  (`oxc.ts` `format_async`), and biome (`biome.ts` `applyConfiguration`),
  matching tsv's fixed config; unmatched defaults (biome's width is 80; oxfmt
  and biome default to double quotes) would make the rows wrap/rewrite
  different amounts of code, conflating config with engine speed. (oxfmt's own
  width default is already 100 — pinned anyway so a default change can't
  silently skew the rows. The options provably reach oxfmt's bundled-prettier
  fallback for css/svelte too.)
  `prettier` is the reference; `oxfmt` also targets prettier conformance, so
  `prettier` vs `oxfmt` is the closest to a same-output, same-work race. `tsv`
  tracks prettier closely but _intentionally diverges_ in documented cases (the
  `_prettier_divergence` fixtures / `conformance_prettier.md`; ~92%
  `corpus:compare:format` match, measured separately — not here). `biome` formats to
  its own style. Because residual layout decisions still differ, a format ratio
  is partly an
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
- **Self-corpus / representativeness.** The perf corpus is real-world code
  only (the fixture suites live in the `gates`/`conformance` views — see
  [Corpus](#corpus)), but it's still dominated by the author's own fuz
  ecosystem plus svelte/kit source — the same code tsv is developed and
  fixture-tuned against. Throughput tracks the syntactic mix of _this_
  corpus, so the ratios are "N× on this corpus," not universal. CSS is by
  far the weakest sample (a few dozen real files — most of the old CSS
  corpus was prettier's fixture suite), so its per-file ratios are the
  noisiest in the report.
- **Conformance-surface semantics (`BENCH_CORPUS=conformance`).** Parse-only
  by design, and the committed surface is **coverage-only** (per-tool preflight
  parse success over the fixtures-included corpus) — the timed phase is
  skipped, so there is no committed throughput. The **Svelte** set has the
  `svelte/compiler`-rejected files removed (the `bench:harvest:svelte-rejects`
  cache, see §Corpus), so Svelte coverage reads as fidelity on *valid* Svelte
  (svelte/compiler → 100%, the oracle) rather than raw success over the suite's
  deliberately-invalid error fixtures; a *higher* number is better, not "more
  permissive." TS/CSS keep the full set (acorn-ts trails, parseCss is lenient —
  neither is a validity oracle). If you run the ad-hoc timed
  variant (coverage flag unset), its throughput is over the all-tools-pass
  intersection — an adversarial corpus that's the "easy" subset
  (`BENCH_MODE=union` audits what it hides). test262 files are
  parsed at every tool's default **module** goal: none of the tsv bindings
  take a goal parameter, the canonical acorn wrapper hardcodes
  `sourceType: 'module'`, and oxc infers module from the synthetic `.ts`
  filename — so strict-script-only constructs (e.g. `await` as an
  identifier) count against every tool equally. The goal-aware per-test
  differential is `diagnostics/test262_compare.ts`, and the graded pass/fail
  conformance gates remain `tsv_debug test262` / `conformance:svelte-fixtures` —
  this surface measures coverage, it doesn't replace them.
- **Measurement-shape asymmetries (small, mostly self-cancelling).** (a) Every
  `tsv` FFI format call UTF-8-encodes the input and decodes the output back to a
  JS string (`lib/ffi.ts` — through persistent grow-only staging buffers, so the
  boundary cost is the encode/copy itself, not per-call allocation);
  `tsv_wasm` marshals strings across the JS↔WASM
  boundary. prettier pays no such boundary tax — so the published `tsv` /
  `tsv_wasm` format numbers are _conservative_ (the raw engine is faster than
  the FFI/WASM figure; the parse analogue is the `tsv-internal` vs `tsv-json`
  gap). One nuance cuts the other way: the persistent buffers amortize across
  the warm loop, so a cold one-shot consumer (format one file, exit) pays a
  first-call allocation the warm per-call figure doesn't include — negligible
  next to process/module startup, but the warm number is a warm number. (b) The async impls (`prettier`, `oxfmt`) are `await`ed per file
  (`process_corpus_async`), carrying a per-file microtask cost the sync impls
  skip — swamped by their actual format time, but real. The opt-in
  **`tsv-forced-async`** control row (`BENCH_FORCED_ASYNC=1` — the same native
  engine as `tsv`, routed through the awaited async path) quantifies this tax
  directly: the `tsv` vs `tsv-forced-async` delta is within the run-to-run noise
  floor even on a fast sub-ms-per-file engine, so the per-file await does **not**
  materially inflate the async impls' numbers — their gaps vs `tsv` are engine
  differences, not harness tax. Scope caveat: the control models a
  *microtask* await (prettier's shape — sync compute behind a resolved
  promise). `oxfmt`'s async is a napi promise whose native work may hop off
  the JS thread (its `tinypool` dep is CLI-only, not in the `format()` path);
  any such hop is part of oxfmt's binding boundary — the same way tsv's row
  includes its FFI boundary — not engine time, and this control doesn't
  isolate it. It's **off by default**: a noise-level delta would
  only add a confusing duplicate-`tsv` row to the published report and feed
  spurious flags to the regression baseline, so it's an on-demand re-confirmation
  tool, not a standing row. (Why a control and not a real sync row: `prettier` and
  `oxfmt` are async-only — neither ships a sync format API — so the tax can't be
  removed, only measured.) (c) Task return values
  are discarded uniformly for all impls; the FFI/WASM/async boundaries block
  dead-code elimination, so no impl's work is optimized away.
- **`tsv_wasm` is measured on the full build.** The WASM bench loads
  `pkg/all/deno` (the default both-features artifact, ~2.6 MB — what
  `@fuzdev/tsv_wasm` ships) for _both_ parse and format, while subset
  consumers ship the smaller `@fuzdev/tsv_format_wasm` (~2.3 MB, no convert
  layer) or `@fuzdev/tsv_parse_wasm` (~1.2 MB, no printers). The Binary
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
- **`-json` parse rows are mechanism-matched but not payload-matched; the
  `oxc-parser` "lazy" story is a myth for the path we benchmark.** In
  oxc-parser's _default_ mode (what we call), the AST is serialized to a
  JSON string in Rust and deserialized in JS — the native `oxc-parser`
  package's `index.js` `wrap()` runs `JSON.parse` on `.program` access
  (verified: `typeof program === 'object'`), exactly the model `tsv-json`
  uses (Rust → JSON string → FFI → `JSON.parse`) and `tsv_wasm-json` uses
  (Rust → JSON string → boundary decode → engine `JSON.parse` via
  `js_sys`). So the rows are like-for-like full-materialization comparisons
  in _mechanism_ — but the _deliverables_ differ: tsv emits the
  acorn/svelte drop-in AST with per-node `loc` line/column objects
  (measured: 46–48% of TS wire bytes and ~61% of its `JSON.parse` time —
  three nested objects per node), while oxc's default AST is span-only (no
  `loc`; it pads `decorators`/`optional`/`typeAnnotation` fields instead
  and still nets ~30% fewer wire bytes per source byte). Measured with
  `loc` stripped, tsv's wire is _smaller_ than oxc's and `JSON.parse`s
  _faster_, and the two Rust parse+serialize sides are at parity — so a
  large share of the row ratio is the richer deliverable the drop-in
  contract mandates, not engine speed. Two further non-obvious points this
  turned up:
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

One tagged entry list (`lib/corpus.ts` `CORPUS_ENTRIES`, paths relative to the
project root). Every entry carries a tier — `real`, `prettier_fixture`, or
`suite` — and each consumer selects a **view**:

- **`perf`** (~2,900 files) — `real` entries only: application & library
  source (the fuz.dev repos' `src/` — zzz, the fuz ecosystem, gro,
  svelte-docinfo, tsv.fuz.dev) plus upstream framework source
  (kit/packages/kit, svelte/packages/svelte, and the svelte.dev subpaths).
  Fixture subtrees inside those repos are pruned (`fixtures` segments
  anywhere; `samples` segments under a `test` segment) while `*.test.ts`
  files stay — tests are real code. This is what `deno task bench` measures,
  so throughput reflects real code, not formatter edge-case suites. This
  framing is the source of truth for the public benchmark page's "What's
  measured" prose — keep them in sync.
- **`gates`** (~6,000 files) — `real` + `prettier_fixture`, no perf prune:
  adds Prettier's `tests/format/{typescript,js,css,html}` suites and
  prettier-plugin-svelte's `test/` (`.html` treated as Svelte, files with a
  companion `options.json` skipped) — deliberately tricky edge cases.
  Exactly the pre-split default corpus: the correctness gates
  (`corpus:compare:*` `--all`, `skip_triage`, `wasm_json_probe`) keep this
  scope, since their sanction lists and documented-divergence coverage were
  reviewed against it. The `DevReposLoader` view is required at every
  construction site — the view decides what a number or gate verdict means,
  so there's no implicit default to inherit by accident.
- **`conformance`** — everything: `gates` + the parse-conformance `suite`
  entries — Svelte's compiler tests (`../svelte/packages/svelte/tests`, with
  the gate-aligned skips: `_`-prefixed segments, `migrate/`, `output.svelte`
  snapshots), the wpt-css harvest cache (`benches/js/.cache/wpt_css`, from
  `deno task bench:harvest:wpt`), and the test262 graded-positive path list
  (`benches/js/.cache/test262_files.json`, a `files_from` entry from
  `deno task bench:harvest:test262`). This is what
  `deno task bench:conformance` measures — the per-tool parse
  coverage surface (coverage-only + node-only).

  **Canonical-reject exclusion (Svelte only, conformance view only).** The
  suite deliberately bundles deliberately-invalid fixtures (svelte's own
  `compiler-errors/`, `loose-*` error-tolerant fixtures, preprocess inputs) plus
  non-Svelte HTML (prettier's `tests/format/html`), so a raw parse-**coverage**
  number scores those intentional rejects as failures — and makes tsv's *higher*
  coverage read as superiority when it's really tsv's deferred-early-error
  *permissiveness*. So the conformance view excludes the Svelte
  files `svelte/compiler` rejects — the `svelte_parse_rejects.json` cache from
  `deno task bench:harvest:svelte-rejects` (`diagnostics/svelte_reject_harvest.ts`),
  loaded by `DevReposLoader` only when `view === 'conformance'`. Coverage then
  measures fidelity on *valid* Svelte: svelte/compiler → 100% (it's the oracle),
  tsv → ~99.85% (residual = the 8 sanctioned over-rejections, `SVELTE_FIXTURE_SANCTIONS`).
  **Svelte only** — svelte/compiler is the parser tsv is a strict drop-in *for*;
  `acorn-typescript` **trails** modern TS/JS (its rejects include valid code tsv
  correctly parses) and `parseCss` is lenient, so neither is a validity oracle and
  TS/CSS get no reject cache. The cache is machine-local + regenerable (like the
  wpt/test262 caches, gitignored); absent = fail-open to the un-filtered corpus
  (disclosed in the load log). The **`gates` view is untouched**, so
  `corpus:compare:*` / `skip_triage` still see the error fixtures they need.

Extensions: `.svelte`, `.ts`, `.js`, `.css`, `.html` (treated as Svelte; only
loaded by entries that opt in).

Each entry is `{path|files_from, tier, extensions?, skip?, optional?}`.
**Missing entries fail fast** — the loader checks every entry up front and
throws listing the missing paths, so a partial checkout can't silently shrink
a perf number or let a correctness gate pass while grading less than it
claims. The only exceptions: the two derived harvest caches are `optional`
(warn-and-skip — their source checkouts `../wpt`/`../test262` are legitimately
machine-dependent, matching the harvests' own `--if-present` posture), and
`BENCH_ALLOW_MISSING=1` opts the bench into a partial corpus explicitly.
Reports carry `corpus_sources` (per-entry loaded file counts) so any tolerated
gap is disclosed rather than invisible.

## Architecture

```
benches/js/
├── package.json           # npm dep source of truth (both runtimes); install_deps drives it
├── package-lock.json      # npm lock (committed for reproducibility)
├── deno.json              # nodeModulesDir: manual + lock: false (npm from package.json; no jsr/remote deps)
├── install_deps.ts        # `bench:install`: npm install + force-fetch the oxc wasi binding
├── harvest_test262.ts     # `bench:harvest:test262`: graded positives → .cache/test262_files.json (Deno-only)
├── bench.ts               # Benchmark entry point (runtime-neutral — runs under Deno AND Node)
├── conformance.ts         # Single-process pre-release aggregate driver (deno task conformance): all five legs, one module cache
├── smoke.ts               # Smoke test for formatters and parsers (runtime-neutral: smoke / smoke:node / smoke:bun)
├── compose_reports.ts     # Fold report.{deno,node,bun}.json → combined report.{json,md} (bench:compose)
├── corpus_compare_format.ts  # Formatting comparison vs prettier (Deno-only entry point)
├── corpus_compare_parse.ts   # Parse/AST comparison vs canonical parsers (Deno-only entry point)
├── divergence_audit.ts    # Divergence audit entry point (Deno-only)
├── diagnostics/           # diagnostic scripts (most ad-hoc, not wired into `deno task` — see §Diagnostic scripts)
│   ├── skip_triage.ts        # parse-parity gate (tsv vs canonical; allowlisted over-rejections)
│   ├── svelte_fixtures_compare.ts  # Svelte-fixtures parse-conformance gate: docstring + config over lib/fixtures_gate.ts (task: conformance:svelte-fixtures)
│   ├── ts_fixtures_compare.ts  # TypeScript-fixtures parse-conformance gate: same, vs acorn-typescript's test/ suite (task: conformance:ts-fixtures)
│   ├── ts_repo_compare.ts    # TypeScript-repo parse gate vs the official tsc corpus, tsc-baselines validity oracle (task: conformance:ts-repo; in the blocking aggregate)
│   ├── test262_compare.ts    # test262 differential (tsv vs oxc-parser, from the Rust manifest)
│   ├── wpt_css_harvest.ts    # wpt <style> blocks → .cache/wpt_css (task: bench:harvest:wpt)
│   ├── svelte_reject_harvest.ts  # svelte/compiler-rejected Svelte files → .cache/svelte_parse_rejects.json (task: bench:harvest:svelte-rejects; conformance view excludes these)
│   ├── wasm_json_probe.ts    # WASM-vs-native JSON parse penalty attribution
│   ├── wasm_format_probe.ts  # WASM format wall-time A/B
│   ├── comment_dup_scan.ts   # comment-dup fixture-corpus completeness guard
│   └── acorn_dup_fuzz.ts     # comment-dup fuzz over acorn-typescript's construct corpus
├── results/baseline.json  # Saved baseline for regression detection (gitignored; written by @fuzdev/fuz_util's benchmark_baseline module)
├── lib/
│   ├── binary_sizes.ts    # Binary/WASM size collection and reporting
│   ├── biome.ts           # Biome WASM wrapper (Svelte, TypeScript, CSS)
│   ├── canonical.ts       # Prettier + Svelte parser wrappers
│   ├── check_node_modules.ts # node_modules preflight: exists + not stale vs package.json (all entry points)
│   ├── compare_cli.ts     # Shared scaffolding for the corpus_compare_* entry points
│   ├── corpus.ts          # DevReposLoader + DirectoryLoader (load/stream; node: builtins)
│   ├── gate_counts.ts     # Pinned gate counts (exact pins + live-corpus minimums + negative-bucket pins) — see §Pinned gate counts
│   ├── harvest_stamp.ts   # Harvest freshness stamps (source commit + pins) — skip unchanged re-harvests
│   ├── prettier_cache.ts  # Content-addressed prettier-output cache for the format comparison
│   ├── diff.ts            # Line-based diff utilities (LCS algorithm)
│   ├── fixtures_gate.ts   # Shared per-language parse-conformance gate engine (run_fixtures_gate; svelte + ts fixtures scripts are docstring+config over it)
│   ├── ffi.ts             # Deno.dlopen bindings (NativeImplementation — Deno native, runtime-specific)
│   ├── napi.ts            # process.dlopen bindings (NapiImplementation — Node/Bun native, runtime-specific)
│   ├── runtime.ts         # Tiny cross-runtime helpers: current_runtime / os / arch normalizers
│   ├── implementations.ts # Implementation registry (branches native FFI vs N-API by runtime)
│   ├── parse_sanctions.ts # Shared parse-parity tracking vocabulary: Sanction (keep deliberately) + KnownGap (fix eventually) types + SVELTE_/TS_FIXTURE_SANCTIONS data; used by skip_triage + all the gates
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
deno task smoke           # confirm every impl still loads + formats (36 checks)
deno check --config benches/js/deno.json benches/js/bench.ts benches/js/lib/biome.ts  # catch type-surface breakage smoke can't (e.g. a major bump renaming an options field)
deno task bench           # regenerate report.{deno,node,bun}.* + combined report.{json,md}
# commit package.json + package-lock.json + results/report.*
```

These packages are free to bump independently — they're measured against, not
baked into fixtures. A **major** bump (e.g. `@biomejs/js-api` 4→6) can change a
package's *type* surface without breaking the runtime path smoke exercises, so
the `deno check` step above is the guard for those.

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
that defines fixture correctness. Agreement across all the pin sites (sidecar
`VERSIONS` + its `npm:` imports, this dir's `package.json`, actor.rs's acorn
import-map pin) is enforced by `deno task pins:audit`
(`scripts/check_canonical_pins.ts`, gated in `deno task check`) — which also
gates **checkout alignment**: a present `../svelte` / `../acorn-typescript`
checkout whose version differs from its pin FAILS `deno task check` (absent
checkouts are skipped, so clean machines/CI still pass). Align the checkout to
the pinned tag, or bump the pins deliberately. `../prettier` is not gated (its
suites' oracle output is computed live per file and the checkout rides `-dev`
versions); `deno task doctor` reports it. Bumping any of the five is therefore not a
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

`canonical.ts` formats with a `filepath` hint (`file.ts` / `file.js` /
`file.svelte` / `file.css`) so prettier applies the same extension-specific
heuristics a real on-disk file gets — matching how `tsv_debug`'s sidecar invokes
prettier. This is load-bearing, not cosmetic, on two axes:

- **`.ts` vs `.tsx`.** Without a filepath prettier can't tell them apart and
  force-adds the JSX-disambiguating trailing comma to single-type-param arrows
  (`<T,>`) that a real `.ts` run never emits — which once manufactured ~39 phantom
  corpus divergences against `@ryanatkn` code that tsv was formatting correctly.
- **`.js` vs `.ts` parser.** The corpus collapses `.js` and `.ts` into one
  `typescript` Language (tsv formats both through its TS path), but real
  prettier-on-`.js` uses the **babel** parser (preserves JSDoc `@type` casts) where
  prettier-on-`.ts` uses **typescript** (strips them). `format_async` takes the real
  source path and routes a `.js` file through `babel` so the oracle matches a real
  on-disk `.js` run — otherwise every `.js` file carrying a JSDoc cast reads as a
  phantom `jsdoc_type_cast_parens` divergence against tsv's (correct) uniform
  preservation. `corpus_compare_format.ts` passes `file.path` for this; the
  benchmark/smoke callers omit it and fall back to the synthetic `file.<ext>`.

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

- **Main** (`oxfmt`): JS wrapper bundling Prettier internals. Depends on `tinypool`
  (CLI-only — `dist/cli.js`; the programmatic `format()` the bench calls is a direct
  async napi call, no worker pool).
- **Native bindings** (`@oxfmt/{platform}`): 8 platform variants. **No WASM variant exists.**
- **Svelte support** is experimental (added in v0.49 via oxc-project/oxc#21700);
  we enable it and let the per-file try/catch + effective-corpus report quantify coverage.
- **Language composition (what each oxfmt format row measures).** The native Rust
  formatter handles **JS/TS only**. Everything else routes through JS-side fallback
  callbacks into oxfmt's **bundled prettier** (`dist/apis-*.js` `formatFile` →
  `prettier.format`): the **css** row is bundled-prettier(postcss) work, and the
  **svelte** row is bundled-prettier + a bundled svelte plugin with
  `prettier-plugin-oxfmt` formatting the embedded `<script>` through the native
  `jsTextToDoc`. So `tsv` vs `oxfmt` is a native-vs-native engine race **on
  TypeScript only**; on css/svelte the oxfmt row is (mostly) a prettier-pipeline
  number in oxfmt packaging — which is what an oxfmt user gets, but read the
  ratios accordingly. The report ratios corroborate: oxfmt ≈ prettier on
  css/svelte, ~20× prettier on TS.

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

- **Corpus SAFETY flakiness under `--all` load (two historical heisenbugs, both
  now guarded in-harness).** Because the safety check is differential vs
  prettier — it iterates the characters _ours_ deviates on and uses prettier
  only as a subtrahend — the two sides fail in **opposite** directions:
  1. **FFI marshalling (false positive — fabricates a violation).** Deno's
     `buffer` fast-call path intermittently handed the native `.so` a
     stale/wrong source pointer under memory pressure, so **ours** read
     corrupted input and genuinely (but spuriously) dropped content — a
     real-looking `content_lost` (historically re-flagging
     `prettier/tests/format/css/numbers/numbers.css`, a documented `@include`
     divergence, as SAFETY under `--all`). Only `ours`-side corruption can fake
     a loss. Hardened twice over in `lib/ffi.ts` (explicit `pointer` params +
     persistent externalized marshalling buffers — pointers probe-verified
     stable across forced full GCs, and the original `buffer`-path corruption
     no longer reproduces on current Deno under synthetic pressure), and
     **self-verified at the verdict**: before recording any SAFETY finding,
     `corpus_compare_format.ts` re-runs the native format and requires
     byte-identity — corruption surfaces as a loud per-file
     `native format nondeterminism` error, never as a silent SAFETY count.
  2. **Prettier empty-output miss (false _negative_ — masks a violation).** The
     in-process prettier (`lib/canonical.ts` — NOT the `tsv_debug` Rust
     sidecar, a separate prettier host with the same symptom) can intermittently
     return empty output under load. That can **never fabricate** a
     `content_lost`: an empty `prettier` inflates `prettier_excess` to the whole
     source, which only cancels `ours`'s deltas — the danger is masking a real
     loss in the same file. Guarded three ways: `corpus_compare_format.ts`
     errors out on semantically-empty (whitespace-only counts) prettier output
     for non-empty source; the prettier cache neither stores nor returns
     semantically-empty entries; and the Rust sidecar's `run_prettier` returns
     a hard `DenoError::EmptyOutput` instead of `Ok("")`, so the fixture
     validator reports the miss accurately rather than as a spurious F2/F3
     content mismatch. Deliberately **no retry** anywhere: a flaky oracle must
     stay loud.

  **Triage:** a SAFETY finding now reproduces by construction (two in-run
  native runs agreed), so treat it as real; confirm root cause with the
  **native CLI** (`tsv format <file>` is deterministic) and diff semantic chars
  vs prettier. A `native format nondeterminism` or prettier-miss **error**
  is the heisenbug surfacing — re-run to clear it, and investigate the
  environment if it persists. For "did my change regress?", diff the sorted
  `.safety[].path` lists before/after (a real regression is a _new path_, not
  a count bump); a change scoped to one printer/crate can't lose content in
  unrelated languages.
- **Parse benchmark overhead**: JSON materialization, not parsing, dominates
  the `-json` rows (see `results/report.<runtime>.md` for the current per-language
  ratios). Use `tsv-internal` for raw parse speed. Both the native and WASM rows go through
  `convert_ast_json_string` — the wire-JSON writer emitting directly from the
  internal AST in one walk, no intermediate `serde_json::Value` or typed public
  tree (per-language pipeline shapes:
  [docs/architecture.md §Closed Scope, Open Convention](../../docs/architecture.md#closed-scope-open-convention)).
  They differ only at the boundary: native crosses via FFI copy +
  `JSON.parse` in JS; WASM decodes the string across the boundary and runs
  the engine's `JSON.parse` from Rust via `js_sys` (measurably faster than a
  `serde_wasm_bindgen`-built object graph). The Rust-side parse-vs-write timing
  is measured by `cargo run --release -p tsv_debug -- json_profile <paths>` —
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

- `diagnostics/skip_triage.ts` — parse-**parity** gate. Parses every corpus file with tsv +
  the canonical parser and buckets by *asymmetry*, not raw error count: `parity` (both reject —
  the healthy state, an intentional-error fixture; never gates), `sanctioned_over_rejection`
  (tsv rejects / canonical accepts, but the path is in the reviewed in-file `SANCTIONED`
  allowlist — tsv is deliberately stricter, or the input is invalid Svelte the canonical parser
  is merely lenient about), `over_acceptance` (tsv accepts / canonical rejects — a deferred
  early-error, reported not gated), and `unexpected_over_rejection` (tsv rejects valid input with
  no sanction — a real drop-in gap). Exits 1 on any `unexpected_over_rejection`, so it asserts
  parity rather than reporting a bare error total. Takes an optional corpus-directory argument
  (defaults to the ~/dev repos, where valid source should be all-parity/green); point it at
  Svelte's own adversarial `tests/` suite to see the residual gap list. Not gated in
  `deno task check` (needs the FFI + canonical sidecar). Run:
  `deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys benches/js/diagnostics/skip_triage.ts [corpus-dir]`
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
- `diagnostics/no_locations_parity.ts` — prove the `no-locations` wire is losslessly
  reconstructible: parse each corpus file the full (loc-bearing) way as the oracle, rebuild
  `loc` from `start`/`end` (UTF-16 offsets) + source via the ECMAScript (TS) / LF-only
  (Svelte) line rules, and assert equality. TS is 100% exact; the two Svelte non-derivable
  cases (the `<script>` `Program` tag-position override, the destructure `+1`-column quirk)
  are classified, not failed. Exits 1 on any unexplained mismatch. The reference
  reconstruction a `no-locations` consumer would use. Run:
  `deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys benches/js/diagnostics/no_locations_parity.ts`
- `diagnostics/reconstruct_vs_materialize.ts` — the **perf** sibling of the parity
  check above: for a consumer that needs full `loc`, is it faster to (A) get the
  full loc-bearing wire (materialized in Rust) or (B) get the smaller `no-locations`
  wire and reconstruct `loc` in JS? Times A / B (dogfoods the shipped
  `create_locator().reconstruct()` helper) / B' (no-loc parse only) end-to-end over
  the `perf` corpus (TS exact, Svelte approximate) and prints sum-of-medians + the
  A/B, A/B' ratios. Finding: B beats A (the full wire's `loc` bytes cost real
  `JSON.parse`), so pre-materializing `loc` in Rust isn't optimal for JS consumers.
  Feeds the committed report's consumer-side note (`report.ts`
  `generate_reconstruct_note`). `BENCH_LIMIT` (files/lang) + `BENCH_FILTER` (path
  substring) tune it. Run:
  `deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys benches/js/diagnostics/reconstruct_vs_materialize.ts`
- `diagnostics/wasm_format_probe.ts` — measure WASM **format** wall-time at the resolution
  the full bench folds into noise (single-digit-% changes). A/Bs two WASM builds
  (copy `pkg/all/deno` aside before editing, rebuild, pass `--baseline
  …/tsv_wasm.js`) with the ../../docs/performance.md §5 paired discipline:
  interleaved pairs, the A/A noise floor measured in the same run, `net = A/B ÷
  floor`, and a corpus byte-identity gate that aborts if the builds format
  differently. Omit `--baseline` for an A/A-only run (floor + current-build
  baseline number, no comparison). See the module doc for the full workflow.
- `diagnostics/wasm_memory_probe.ts` — measure WASM **linear-memory high-water** for `format()` — the
  memory axis `wasm_format_probe.ts` (wall) can't see, and the gate for doc-IR memory work (arena/output
  pre-size, the parked `DocNode` shrink). WASM memory only grows, so `memory.buffer.byteLength` after a
  format is the peak; the deno glue doesn't re-export `memory`, so the probe captures it by monkeypatching
  `WebAssembly.instantiateStreaming` before importing, and forces a fresh instance per file via a
  query-string cache-bust. Two modes: **`--cold`** (fresh instance/file → per-file cold-start peak +
  growth distribution, the pre-size lever gate) and default **steady-state** (one warm instance over the
  corpus → the reset-reuse high-water). A/B two builds with `--baseline …/tsv_wasm.js` (same
  copy-aside-and-rebuild workflow as `wasm_format_probe`). Human output → stderr, `--json` → stdout. Run:
  `deno run --allow-read --allow-env --allow-net --allow-sys benches/js/diagnostics/wasm_memory_probe.ts --cold`
- **wasm-opt**: runs with explicit feature flags in `crates/tsv_wasm/Cargo.toml` — Rust 2024's bulk-memory and nontrapping-float-to-int ops are passed by name to wasm-opt v117, giving ~−2% gzipped on the WASM bundle.
- **oxfmt × Deno timer interaction (workaround in place)**: once `oxfmt.format` runs once,
  Deno's timer wheel processes exactly one further `setTimeout` callback and then stalls all
  subsequent timers indefinitely. Repro:
  `await import('oxfmt').then((m) => m.format('file.ts', 'x=1', {useTabs:true}))` followed by
  two `new Promise((r) => setTimeout(r, 50))` — first resolves, second never does.
  Independent of oxfmt version (reproduced with 0.28.0, 0.50.0, 0.53.0, and 0.57.0 on Deno 2.8.3)
  so the regression is on the Deno / napi-rs side; re-test the repro before ever removing
  the workaround. In `bench.ts` oxfmt is invoked per-iteration during the `format/*` measurement
  loops; the leak shows up at the next inter-task `await wait(cooldown_ms)`, which never
  fires. Workaround: `cooldown_ms: 0` in `run_benchmark_group`'s `Benchmark` config — runs
  tasks back-to-back without the cooldown await. Async measurement loops (`prettier`,
  `oxfmt` itself) are unaffected because their per-iteration awaits resolve via microtasks,
  not timers.
