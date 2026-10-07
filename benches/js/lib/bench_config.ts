/**
 * The bench run's configuration, read from the environment — the one reading every
 * process of a run shares.
 *
 * A run is several processes (`bench.ts` §Process model): the orchestrator, one
 * pre-flight child, and a fresh child per timed row per pass. The children inherit
 * the orchestrator's environment, so importing this module is how the orchestrator
 * and the pre-flight child come to agree on what is being measured without the
 * answer being passed twice. A row's child does NOT import it: everything a timed
 * row needs rides its spec, so its process holds no more harness than its row.
 *
 * Importing this validates the selectors and exits non-zero on a bad one, in
 * whichever process reads it first.
 *
 * @module
 */

import { env, exit } from 'node:process';
import { current_runtime } from './runtime.ts';

/** The JS runtime executing this bench — labels the report siblings
 * (`report.deno.*` / `report.node.*`) and every row's `runtime` field, and
 * selects the runtime-specific native (FFI vs N-API) + WASM (deno vs nodejs
 * target) artifacts. The same bench body runs under all three. */
export const RUNTIME = current_runtime();

/** Parse optional non-negative integer from env var; malformed values fall back to undefined. */
const env_int = (name: string): number | undefined => {
	const val = env[name];
	if (!val) return undefined;
	const n = parseInt(val, 10);
	return Number.isFinite(n) && n >= 0 ? n : undefined;
};

/** Limit files per language (default: all) */
export const MAX_FILES_PER_LANGUAGE = env_int('BENCH_LIMIT');

/** Filter files by path pattern (default: none) */
export const FILE_FILTER = env.BENCH_FILTER;

/** Whether the corpus is limited — a limited run never overwrites the canonical report. */
export const IS_LIMITED = MAX_FILES_PER_LANGUAGE !== undefined || FILE_FILTER !== undefined;

/**
 * Tolerate missing corpus entries (`BENCH_ALLOW_MISSING=1`; default off — numbers
 * from a partial corpus aren't comparable to the committed reports).
 */
export const ALLOW_MISSING = env.BENCH_ALLOW_MISSING === '1';

/** Warmup iteration floor (default: 3, every row — see `warmup_iterations_for`) */
export const BENCH_WARMUP = env_int('BENCH_WARMUP') ?? 3;

/**
 * Warmup DURATION floor per row in ms (default: 5000). Warmup is an iteration
 * count in the timing library, so a fixed count warms a 25 ms row for 75 ms and a
 * 13 s row for 39 s: the fast rows entered their measured window with the JIT
 * still tiering, read as a negative `drift` on every runtime — median −1.9% on
 * rows under 50 ms — and a mean biased slow by about half of it, on exactly the
 * rows the ratios favor. Sizing warmup by TIME from the row's own pre-flight sweep
 * evens that out. The floor is 5 s, not 1 s, because tier-up is a wall-time
 * process and JSC's takes seconds: at a 1 s floor V8's rows settled (deno / node
 * fast-row drift −1.9% → −0.7…−1.0%) while bun's json and yuku rows still read
 * −6…−9% — a synthetic JSON.parse loop shows none of it, so it is the per-file
 * JS paths tiering, not the heap — and at 5 s the same rows read −0.2…+0.7%
 * (a CSS parse row: −9.0% → −0.2%). One floor on every runtime, since a
 * per-runtime warmup would be a second protocol on the rows this bench compares
 * across runtimes; the price is ~2.5 min of wall per runtime on the rows whose
 * three sweeps fall short of it, and the multi-second rows are unchanged.
 */
export const BENCH_WARMUP_MS = env_int('BENCH_WARMUP_MS') ?? 5000;

/**
 * Enable the per-iteration forced-GC hook (default: off — measures realistic
 * throughput where GC happens opportunistically, matching real-world usage).
 * Set `BENCH_GC=1` to force a major GC between every iteration; useful for
 * stabilizing high-allocation workloads at the cost of penalizing efficient
 * low-allocation paths. See `docs/benchmarks.md` §Fairness caveats for the trade-off.
 */
export const BENCH_GC = env.BENCH_GC === '1';

/**
 * How many fresh processes each row is timed in (`BENCH_PASSES`, default 3, at
 * least 1). A pass is one process per row, the rows of a group taken in a different
 * order each pass (`lib/bench_plan.ts` `pass_order`), and a row's published figures
 * pool the timings of all its passes.
 *
 * Several, because one process is one draw: a row's level can differ between two
 * fresh processes by more than it varies inside either (JSC's allocation-heavy rows
 * have sat at two levels a tenth apart), and a single pass publishes whichever it
 * drew with a quiet `cv`. Three rather than two so the pooled middle is a pass
 * rather than the gap between two that disagree. The spread across passes is
 * published per row (`pass_spread`), which is what makes the draw visible.
 */
export const BENCH_PASSES = Math.max(1, env_int('BENCH_PASSES') ?? 3);

/**
 * The sweep floor of ONE pass, every row alike. Fast rows are duration-bound (they
 * reach `BENCH_DURATION` long before any floor); the floor exists for the
 * multi-second rows, where the duration budget alone would leave a handful of
 * sweeps. Eight, because that is what the within-pass drift reading needs
 * (`DRIFT_MIN_SAMPLES` in `lib/bench_plan.ts`): with four a side no single deviant
 * sweep can be a half's median — and the multi-second rows are exactly the ones a
 * leak or a heap tipping over degrades, so nulling the reading there would blind
 * the detector where it matters most. A row's pooled sample count is therefore at
 * least this times `BENCH_PASSES`.
 */
export const PASS_MIN_ITERATIONS = 8;

/**
 * Include the `tsv-forced-async` control row (default off). Same native engine
 * as `tsv`, routed through the awaited async path, to re-confirm that the
 * per-file await tax the async-only impls (`prettier`, `oxfmt`) pay is below the
 * noise floor. Kept opt-in so the noise-level row stays out of the published
 * report and the regression baseline; set `BENCH_FORCED_ASYNC=1` to enable.
 * See `BenchmarkTaskOptions.forced_async`.
 */
export const BENCH_FORCED_ASYNC = env.BENCH_FORCED_ASYNC === '1';

/**
 * Iteration corpus mode. Default `intersection`: within each group, every
 * task is timed on the same all-N intersection (files every impl in the
 * group successfully processed in pre-flight). Comparisons across impls are
 * then apples-to-apples; one noisy impl shrinks the corpus for the whole
 * group, but the coverage report still discloses per-impl skip rates.
 *
 * Set `BENCH_MODE=union` to restore the per-impl iteration model (each task
 * runs its own preflight success set, ratios reflect different file sets) —
 * useful for reproducing pre-intersection numbers or auditing what the
 * intersection mode hides.
 */
const BENCH_MODE = env.BENCH_MODE;
if (BENCH_MODE !== undefined && BENCH_MODE !== 'intersection' && BENCH_MODE !== 'union') {
	console.error(`Invalid BENCH_MODE: ${BENCH_MODE}. Expected 'intersection' or 'union'.`);
	exit(1);
}
export const USE_INTERSECTION = BENCH_MODE !== 'union';

/** Which corpus/surface a report was produced from — see `BENCH_CORPUS`. */
export type CorpusKind = 'perf' | 'conformance';

/**
 * Corpus + surface selector. Default `perf`: the real-world corpus view, parse
 * + format groups, writing `report.<runtime>.*` — the throughput headline.
 * `BENCH_CORPUS=conformance`: the fixtures-only corpus view (prettier suites +
 * the parse-conformance suites, disjoint from the perf/real corpus; a suite with
 * a validity oracle or harness is filtered to what it calls valid — see
 * `lib/corpus.ts` `EXCLUSION_CACHES` and `lib/prettier_fixtures.ts`),
 * parse groups ONLY, writing `report.conformance.<runtime>.*` — the per-tool
 * parse coverage/throughput surface. Format impls are deliberately excluded there:
 * grading formatter behavior on the fixture suites is the correctness gates'
 * job (`corpus:compare:format`), and timing it would put prettier/oxfmt/biome
 * through tens of thousands of fixture files for numbers nothing consumes.
 */
const BENCH_CORPUS = env.BENCH_CORPUS;
if (BENCH_CORPUS !== undefined && BENCH_CORPUS !== 'perf' && BENCH_CORPUS !== 'conformance') {
	console.error(`Invalid BENCH_CORPUS: ${BENCH_CORPUS}. Expected 'perf' or 'conformance'.`);
	exit(1);
}
export const CORPUS_MODE: CorpusKind = BENCH_CORPUS === 'conformance' ? 'conformance' : 'perf';
export const IS_CONFORMANCE = CORPUS_MODE === 'conformance';

/** Operations measured this run — conformance is a parse-only surface. */
export const OPERATIONS: ('parse' | 'format')[] = IS_CONFORMANCE ? ['parse'] : ['parse', 'format'];

/**
 * Report filename tag: `report.<tag>.{json,md}` and
 * `<timestamp>_<commit>.<tag>.{json,md}`. The conformance surface writes
 * sibling files rather than clobbering the perf reports (and stays invisible
 * to `compose_reports.ts`, which globs the exact perf filenames).
 */
export const REPORT_TAG = IS_CONFORMANCE ? `conformance.${RUNTIME}` : RUNTIME;

/**
 * Duration of ONE PASS of a row in ms. The default is surface-dependent: 5000 for
 * perf, 15000 in conformance mode — there each iteration is a full sweep of
 * the much larger conformance corpus, so the slow rows need the longer
 * window for a usable sample count. `BENCH_DURATION` overrides either.
 */
export const BENCH_DURATION = env_int('BENCH_DURATION') ?? (IS_CONFORMANCE ? 15_000 : 5000);

/**
 * Coverage-only mode (`BENCH_COVERAGE_ONLY=1`): run pre-flight — which fully
 * determines per-tool parse coverage — and emit the report straight from it,
 * SKIPPING the timed benchmark phase entirely. That phase costs a fixed floor
 * of full-corpus sweeps per row per pass (the warmup floor plus
 * `PASS_MIN_ITERATIONS`) no matter how low
 * `BENCH_DURATION` goes, yet the conformance surface's coverage consumers (the
 * site's per-engine table, `derive_conformance_groups`) read only the
 * pre-flight counts — so on a coverage refresh the whole timing cost is wasted.
 * Entries are emitted with null timing stats; the output stays the same
 * `report.<tag>.{json,md}` files (coverage is what a conformance report is
 * for). Orthogonal to `BENCH_CORPUS`, but only meaningful with `conformance` —
 * in perf mode the timing IS the headline.
 */
export const COVERAGE_ONLY = env.BENCH_COVERAGE_ONLY === '1';
if (COVERAGE_ONLY && !IS_CONFORMANCE) {
	// Coverage-only is a conformance-surface mode. In perf mode it would skip the
	// timed phase and then overwrite the perf report (`report.<runtime>.json`) with
	// null-timing entries — corrupting the throughput headline. Reject the combo.
	console.error(
		'BENCH_COVERAGE_ONLY=1 requires BENCH_CORPUS=conformance (it is a conformance-only mode; ' +
			'running it in perf mode would overwrite the perf report with null-timing entries).'
	);
	exit(1);
}

/** Maximum length of error message to display (longer messages are truncated) */
export const MAX_ERROR_MESSAGE_LENGTH = 200;

/**
 * Baseline storage directory. Passed to `benchmark_baseline_save` /
 * `_compare`; the library calls `mkdir(path, { recursive: true })` and
 * writes `baseline.json` inside, so the file lands at
 * `./benches/js/results/baseline.json`. Moved into `results/` (from its
 * pre-0.60 location at `./benches/js/baseline.json`) so the library's
 * mkdir is covered by the existing `--allow-write=benches/js/results`
 * permission without widening write scope to the whole benches tree.
 */
export const BASELINE_DIR = './benches/js/results';

/** Results directory for comparison JSON files */
export const RESULTS_DIR = './benches/js/results';

/**
 * The run's scratch directory: what its processes hand each other (a child's spec
 * and result, the file sets the timed rows read). Inside `RESULTS_DIR` because that
 * is the one place the Deno tasks may write; per runtime, since the three runtimes'
 * runs are separate invocations; created empty by the orchestrator and removed when
 * it exits.
 */
export const RUN_DIR = `${RESULTS_DIR}/.run-${RUNTIME}`;

/**
 * This run's task-registry options, in one place: the row-composition guards, the
 * pre-flight and every timed row's process must ask the registry the SAME question,
 * and two spellings of `{forced_async, corpus_kind}` were free to drift into asking
 * different ones.
 */
export const TASK_OPTIONS = { forced_async: BENCH_FORCED_ASYNC, corpus_kind: CORPUS_MODE } as const;
