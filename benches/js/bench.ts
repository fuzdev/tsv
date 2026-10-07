/**
 * tsv benchmark suite
 *
 * Compares parsing and formatting performance across implementations.
 * All benchmarks are single-threaded: files processed sequentially, no parallelism.
 *
 * Implementations:
 * - Canonical: prettier + svelte/compiler (JS baseline)
 * - Native: tsv via FFI (Rust, maximum performance)
 * - WASM: tsv compiled to WASM (portable, near-native)
 * - Alternatives: oxc-parser, oxfmt, biome-wasm, dprint-wasm, yuku-parser (for comparison)
 *
 * Run with: deno task bench:deno:run (Deno), deno task bench:node:run (Node) or
 * deno task bench:bun:run (Bun). The same scripts run under all three — each detects
 * the runtime and the run writes a runtime-labeled report (report.deno.* /
 * report.node.* / report.bun.*). See benches/js/CLAUDE.md.
 *
 * ## Process model
 *
 * A run is several processes of one runtime, and this script is the one that holds
 * none of what is measured:
 *
 * 1. **This process — the orchestrator.** It loads no engine and no corpus. It
 *    starts the others one at a time and blocks while each runs, then pools what
 *    they measured and writes the report.
 * 2. **One pre-flight process** (`bench_preflight.ts`). It loads the corpus and
 *    every implementation, runs every row once over every file to learn what each
 *    accepts, grades the run's standing claims, writes each group's timed file set
 *    and a snapshot of what it learned — and exits, taking the corpus and every
 *    loaded engine with it.
 * 3. **One process per timed row per pass** (`bench_row.ts`). Each loads the one
 *    engine its row runs and that row's file set, warms, times, reports and exits.
 *    A row is timed in `BENCH_PASSES` such processes; each pass takes a group's rows
 *    in a different order (`lib/bench_plan.ts` `pass_order`), and a row's published
 *    statistics pool the timings of all its passes.
 *
 * Why: timed back to back in one process, the rows moved each other's numbers.
 * Engines that share code shape each other's type feedback; the collector sizes its
 * young generation from whatever has been allocating; a wasm heap one row grew
 * stays grown for the next. A forced collection between rows resets where a row
 * starts, not the regime it runs in, and a fixed order turns all of it into a
 * per-position bias. A process per row removes the shared state rather than
 * bounding it; several passes in different orders balance what the machine itself
 * still carries from one row to the next, and their spread is published
 * (`pass_spread` per row, `process_noise` for the run).
 *
 * CLI options:
 *   --json              Output results as JSON
 *   --markdown          Output results as Markdown
 *   --save-baseline     Also save results as baseline for regression detection
 *   --compare-baseline  Compare against saved baseline
 *   --save-report       Overwrite the canonical report.<runtime>.{json,md} even on a limited run
 *   --verbose           Include per-file skip detail (paths + errors + failure sets)
 *
 * Results are always saved to benches/js/results/<timestamp>_<commit>.<runtime>.{json,md}.
 * Latest results are also written to benches/js/results/report.<runtime>.{json,md} (committed
 * to git). Conformance runs (BENCH_CORPUS=conformance) tag both filenames with `conformance.`
 * before the runtime (report.conformance.<runtime>.{json,md}).
 *
 * Environment variables (read in lib/bench_config.ts; every process of the run
 * inherits them):
 *   BENCH_LIMIT         Limit files per language (default: all)
 *   BENCH_FILTER        Filter files by path pattern (default: none)
 *   BENCH_PASSES        Fresh processes each row is timed in (default: 3)
 *   BENCH_DURATION      Duration of ONE PASS of a row in ms (default: 5000; 15000 in
 *                       conformance mode — full-corpus sweeps per iteration)
 *   BENCH_WARMUP        Warmup iteration FLOOR per pass (default: 3); every pass also
 *                       warms for at least BENCH_WARMUP_MS, whichever is more
 *   BENCH_WARMUP_MS     Warmup duration floor per pass in ms (default: 5000) — a
 *                       fast row's three sweeps were ~50 ms of warmup, and the
 *                       JIT was still tiering through the measured window; JSC
 *                       keeps tiering for seconds of wall time, so 1 s was not enough
 *   BENCH_MODE          'intersection' (default) | 'union' — iteration corpus mode
 *   BENCH_CORPUS        'perf' (default) | 'conformance' — corpus + surface selector:
 *                       perf = real-world corpus, parse + format groups (every in-scope
 *                         tool must fully process it — unlisted pre-flight failures hard-fail;
 *                         see lib/perf_omit.ts);
 *                       conformance = fixtures-only view (the prettier + parse-conformance
 *                         suites, excluding the perf/real corpus, each less its exclusion
 *                         caches and filters — lib/corpus.ts), parse groups only
 *                       (see benches/js/CLAUDE.md §Corpus)
 *   BENCH_COVERAGE_ONLY Set to 1 to emit coverage from pre-flight and SKIP the
 *                       timed phase (requires BENCH_CORPUS=conformance). Default off
 *   BENCH_ALLOW_MISSING Set to 1 to tolerate missing corpus repos (default: off —
 *                       a missing required entry fails fast, since numbers from a
 *                       partial corpus aren't comparable to the committed reports)
 *   BENCH_GC            Set to 1 to force a major GC between every iteration
 *                       (default: off; see docs/benchmarks.md §Fairness caveats
 *                       for the trade-off)
 *   BENCH_STALE_OK      Set to 1 to run despite stale artifacts (default: off;
 *                       see lib/check_artifact_freshness.ts)
 *   BENCH_FORCED_ASYNC  Set to 1 to add the `tsv-forced-async` control row
 *                       (default: off; diagnostic — async-tax measurement)
 */

// Type declaration for V8's gc function (available with --expose-gc)
declare global {
	var gc: (() => void) | undefined;
}

import { z } from 'zod';
import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import { BenchmarkStats } from '@fuzdev/fuz_util/benchmark_stats.ts';
import type { BenchmarkResult } from '@fuzdev/fuz_util/benchmark_types.ts';
import {
	benchmark_baseline_compare,
	benchmark_baseline_format,
	benchmark_baseline_save
} from '@fuzdev/fuz_util/benchmark_baseline.ts';
import { spawn_out } from '@fuzdev/fuz_util/process.ts';
import { mkdirSync, rmSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import process, { argv, exit } from 'node:process';
import { fileURLToPath } from 'node:url';
import { run_bench_child } from './lib/bench_child.ts';
import {
	BASELINE_DIR,
	BENCH_DURATION,
	BENCH_GC,
	BENCH_PASSES,
	BENCH_WARMUP,
	BENCH_WARMUP_MS,
	CORPUS_MODE,
	type CorpusKind,
	COVERAGE_ONLY,
	IS_CONFORMANCE,
	IS_LIMITED,
	MAX_ERROR_MESSAGE_LENGTH,
	OPERATIONS,
	PASS_MIN_ITERATIONS,
	REPORT_TAG,
	RESULTS_DIR,
	RUN_DIR,
	RUNTIME,
	TASK_OPTIONS
} from './lib/bench_config.ts';
import {
	pass_order,
	type PassSummary,
	type ProcessNoise,
	summarize_passes,
	summarize_process_noise,
	warmup_iterations_for
} from './lib/bench_plan.ts';
import type {
	PreflightGroup,
	PreflightRow,
	PreflightSnapshot,
	PreflightSpec,
	RowResult,
	RowSpec,
	VariantParityFinding
} from './lib/bench_protocol.ts';
import {
	type BinarySize,
	type CollectedBinarySizes,
	generate_binary_size_markdown,
	generate_binary_size_report
} from './lib/binary_sizes.ts';
import { type CorpusRepoRef, type CorpusSource, format_mb } from './lib/corpus.ts';
import type { UnavailableImpl } from './lib/implementations.ts';
import { generate_group_omissions_markdown, type GroupOmissions } from './lib/perf_omit.ts';
import {
	alternative_version_parts,
	type CoverageBySource,
	generate_comparison_markdown,
	generate_comparison_summary,
	generate_coverage_by_source_markdown,
	generate_coverage_only_markdown,
	generate_effective_corpus_report,
	generate_group_bench_table_markdown,
	generate_group_coverage_markdown,
	generate_group_coverage_only_markdown,
	generate_group_files_markdown,
	generate_group_throughput_markdown,
	generate_json_overhead_note,
	generate_locations_note,
	generate_skipped_files_markdown,
	generate_skipped_files_report,
	generate_summary_report,
	generate_versions_info,
	type GroupResults,
	parse_payload_tier,
	type PayloadTier,
	type ReportVersions,
	type SourceCoverageCell
} from './lib/report.ts';
import { current_machine, type Machine, type Runtime } from './lib/runtime.ts';
import {
	CANONICAL_FORMATTER_ROW,
	CANONICAL_PARSER_ROWS,
	type Language,
	LANGUAGES
} from './lib/types.ts';

//
// CLI Arguments
//

const Args_schema = z.strictObject({
	_: z.array(z.string()).default([]),
	json: z.boolean().default(false),
	markdown: z.boolean().default(false),
	'save-baseline': z.boolean().default(false),
	'compare-baseline': z.boolean().default(false),
	'save-report': z.boolean().default(false),
	verbose: z.boolean().default(false)
});

// Strip leading -- from deno task passthrough. `argv.slice(2)` (node:process) is
// the cross-runtime equivalent of `Deno.args` — Deno exposes the same shape.
const cli_args = argv.slice(2);
const raw_argv = cli_args[0] === '--' ? cli_args.slice(1) : cli_args;
const parsed_argv = argv_parse(raw_argv);
const parsed = args_parse(parsed_argv, Args_schema);

if (!parsed.success) {
	const known = Object.keys(Args_schema.shape)
		.filter((k) => k !== '_')
		.map((k) => `--${k}`);
	console.error(
		'Invalid arguments:',
		parsed.error.issues.map((i: { message: string }) => i.message).join(', ')
	);
	console.error(`Known flags: ${known.join(', ')}`);
	exit(1);
}

if (parsed.data._.length > 0) {
	console.error(`Unexpected positional arguments: ${parsed.data._.join(', ')}`);
	exit(1);
}

const args = {
	json: parsed.data.json,
	markdown: parsed.data.markdown,
	save_baseline: parsed.data['save-baseline'],
	compare_baseline: parsed.data['compare-baseline'],
	save_report: parsed.data['save-report'],
	verbose: parsed.data.verbose
};

// Baseline statistics requested — raises a one-pass run's sweep floor (see
// `pass_min_iterations`) so the Welch comparisons run on usable sample sizes.
const baselining = args.save_baseline || args.compare_baseline;

// In JSON/markdown mode, progress goes to stderr so stdout is clean structured output
const structured_output = args.json || args.markdown;

function log(...messages: unknown[]): void {
	if (structured_output) {
		console.error(...messages);
	} else {
		console.log(...messages);
	}
}

// Baselines are a perf-surface tool (Welch-t regression detection on
// throughput, one corpus-blind baseline.json). Conformance-mode changes are
// coverage moves, reviewed via the committed report diff — sharing the
// baseline file would cross-contaminate the perf history.
if (IS_CONFORMANCE && (args.save_baseline || args.compare_baseline)) {
	console.error(
		'Baseline flags are perf-corpus only — drop --save-baseline/--compare-baseline ' +
			'or run without BENCH_CORPUS=conformance.'
	);
	exit(1);
}

//
// Run
//

/**
 * Start one child of the run and return what it reported, or end the run. A child
 * that fails has already said why on this terminal, so the orchestrator adds one
 * line naming which child it was and exits non-zero — nothing is written, since a
 * report missing a row (or its pre-flight) would not be the report of this run.
 */
function run_child<TResult>(
	script: Parameters<typeof run_bench_child>[0],
	spec: Parameters<typeof run_bench_child>[1],
	spec_path: string,
	label: string
): TResult {
	try {
		return run_bench_child<TResult>(script, spec, spec_path, RESULTS_DIR, label);
	} catch (e) {
		console.error(`\n✗ ${e instanceof Error ? e.message : String(e)}`);
		return exit(1);
	}
}

// The run's scratch directory (`RUN_DIR`): created empty, so nothing a previous run
// left can be read as this one's, and removed when this process exits. The signal
// handlers exist so that an interrupt reaches the `exit` listener.
//
// What an interrupt does depends on who it reaches. Ctrl-C in a terminal goes to
// the whole foreground group: the running child dies of it, `run_child` reports that
// and exits. A signal sent to THIS process alone is held while it blocks on a child
// — a blocked process runs no handler — and taken when that child returns
// (`yield_to_signals`), so the run stops after the row in flight, not mid-row.
rmSync(RUN_DIR, { recursive: true, force: true });
mkdirSync(RUN_DIR, { recursive: true });
process.on('exit', () => rmSync(RUN_DIR, { recursive: true, force: true }));
process.on('SIGINT', () => exit(130));
process.on('SIGTERM', () => exit(143));

/**
 * Let a pending signal handler run. The timed phase is one blocking spawn after
 * another, and a loop that never returns to the event loop would hold a signal
 * until the whole run finished.
 */
const yield_to_signals = (): Promise<void> => new Promise((resolve) => setImmediate(resolve));

//
// Pre-flight
//

const preflight_spec: PreflightSpec = {
	result_path: `${RUN_DIR}/preflight.result.json`,
	log_to_stderr: structured_output,
	run_dir: RUN_DIR
};
const snapshot = run_child<PreflightSnapshot>(
	'bench_preflight.ts',
	preflight_spec,
	`${RUN_DIR}/preflight.spec.json`,
	'pre-flight'
);

//
// Per-row pre-flight state (keyed by tracking_key, e.g. `parse/svelte/native`),
// rebuilt from the snapshot in the shapes the report reads.
//
// `effective_corpus_size` and `skipped_files` always reflect pre-flight results,
// independent of the iteration mode — they are the source of truth for coverage
// disclosure. `effective_corpus_bytes` and `iterated_file_count` reflect what was
// actually timed (intersection or per-impl).
//

/** Map row name → tracking_key per group, so the report can look a row up by display name. */
const task_tracking_by_group: Map<string, Map<string, string>> = new Map();
/** Effective corpus size per row (processed / total files). */
const effective_corpus_size: Map<string, { processed: number; total: number }> = new Map();
/** Effective corpus bytes per row — used for honest throughput math. */
const effective_corpus_bytes: Map<string, number> = new Map();
/**
 * Files a timed row sweeps. Distinct from `effective_corpus_size` (which records
 * pre-flight success — disclosure-only coverage info): in `intersection` mode this
 * is the per-group all-N intersection (uniform across the rows of a group); in
 * `union` mode it's the row's own pre-flight success set. Used by the bench-table
 * `Nx (Mf)` annotation and the Comparisons table's pairwise file counts. A row
 * that is not timed has NO entry, so its `files_iterated` reads `null`.
 */
const iterated_file_count: Map<string, number> = new Map();
/**
 * Per-row digest of the SORTED timed path set (see `BaselineEntry.files_iterated_digest`),
 * recorded beside `iterated_file_count` for the same rows.
 */
const iterated_files_digest: Map<string, string> = new Map();
/**
 * Tracking keys of coverage-only rows — measured in pre-flight, never timed. See
 * `BenchmarkTask.coverage_only`.
 */
const coverage_only_keys: Set<string> = new Set();

for (const group of snapshot.groups) {
	const task_tracking = new Map<string, string>();
	for (const row of group.rows) {
		task_tracking.set(row.name, row.tracking_key);
		if (row.coverage_only) coverage_only_keys.add(row.tracking_key);
		effective_corpus_size.set(row.tracking_key, { processed: row.processed, total: row.total });
		effective_corpus_bytes.set(row.tracking_key, row.effective_bytes);
		if (row.timed) {
			iterated_file_count.set(row.tracking_key, row.timed.files);
			iterated_files_digest.set(row.tracking_key, row.timed.digest);
		}
	}
	task_tracking_by_group.set(group.name, task_tracking);
}

/** Files a row failed on during pre-flight, with the error message. */
const skipped_files: Map<string, Map<string, string>> = new Map(
	Object.entries(snapshot.skipped_files).map(([tracking_key, by_path]) => [
		tracking_key,
		new Map(Object.entries(by_path))
	])
);

/** The per-source coverage tables' data, back in the shape `lib/report.ts` renders. */
const coverage_by_source: CoverageBySource = new Map(
	Object.entries(snapshot.coverage_by_source ?? {}).map(([group, by_source]) => [
		group,
		new Map(
			Object.entries(by_source).map(([source, cells]) => [source, new Map(Object.entries(cells))])
		)
	])
);

/**
 * Third-party stderr noise the run's processes silenced, by pattern — pre-flight's
 * counts, plus each timed row's as it reports (`lib/bench_sweep.ts`
 * `suppress_stderr_noise`).
 */
const suppressed_noise: Map<string, number> = new Map(Object.entries(snapshot.suppressed_noise));

//
// Timed phase
//

/**
 * Format bytes/sec as MB/s. Always MB/s, even for sub-1-MB values
 * (renders as e.g. `0.4 MB/s`) so a column of throughput numbers scans
 * uniformly without unit-switching mid-table. Decimal (1e6), the same convention
 * as `lib/corpus.ts`'s `format_mb` — the corpus size and a rate over it are read
 * against each other, so they cannot be denominated differently.
 */
function format_throughput(bytes_per_sec: number): string {
	return `${(bytes_per_sec / 1_000_000).toFixed(1)} MB/s`;
}

/**
 * `<sweeps/sec> (<MB/s>)` for a mean sweep time. Throughput uses the row's effective
 * bytes (what it was timed on), and prints `—` rather than a misleading `0.0 MB/s`
 * when the timed set is empty while sweeps/sec is real.
 */
function format_rate(tracking_key: string, mean_ns: number): string {
	const sweeps_per_second = 1e9 / mean_ns;
	const effective_bytes = effective_corpus_bytes.get(tracking_key) ?? 0;
	const throughput =
		effective_bytes === 0 ? '—' : format_throughput(sweeps_per_second * effective_bytes);
	return `${sweeps_per_second.toFixed(1)} sweeps/sec (${throughput})`;
}

/**
 * The pooled sample count a baseline comparison wants under every row: the Welch
 * p-values feeding regression verdicts sit in an unstable-DOF regime at n≈4-7 (the
 * timing library's own n=30 floor exists to avoid it).
 */
const BASELINE_MIN_SAMPLES = 10;

/**
 * The sweep floor of one pass of every row — ONE protocol per row on every runtime,
 * with no tier keyed on a row's own timing: a row straddling a timing threshold
 * takes one tier under one runtime and the other under the next, which is two
 * protocols published as one runtime ratio. `PASS_MIN_ITERATIONS`, unless a
 * baseline is being saved or compared in a run of so few passes that the pooled
 * count would fall short of `BASELINE_MIN_SAMPLES`.
 */
const pass_min_iterations = Math.max(
	PASS_MIN_ITERATIONS,
	baselining ? Math.ceil(BASELINE_MIN_SAMPLES / BENCH_PASSES) : 0
);

/** What the timed phase learned about a row beyond its pooled `BenchmarkResult`. */
interface TimedRow {
	/** The row's passes, pooled — `lib/bench_plan.ts` `summarize_passes`. */
	summary: PassSummary;
	/** Warmup sweeps each pass ran. */
	warmup_iterations: number;
	/** The median, over the passes, of the heap each pass's warmup began from. */
	settled_heap_bytes: number;
}

/** Every timed row's `TimedRow`, by tracking_key. */
const timed_rows: Map<string, TimedRow> = new Map();

const all_group_results: GroupResults[] = [];

/** How many row processes the run has started — names each one's spec and result. */
let row_processes = 0;

/** Time one pass of one row in a process of its own. */
function time_row_pass(group: PreflightGroup, row: PreflightRow, pass: number): RowResult {
	const timed = row.timed!;
	const id = row_processes++;
	const spec: RowSpec = {
		result_path: `${RUN_DIR}/row_${id}.result.json`,
		log_to_stderr: structured_output,
		row: {
			operation: group.operation,
			language: group.language,
			name: row.name,
			impl: row.impl
		},
		tracking_key: row.tracking_key,
		task_options: TASK_OPTIONS,
		file_set: timed.file_set,
		files_digest: timed.digest,
		script_only: timed.script_only,
		duration_ms: BENCH_DURATION,
		// Sized by TIME from the row's own pre-flight sweep, once, so every pass of the
		// row — and the same row under another runtime — warms by one rule.
		warmup_iterations: warmup_iterations_for(timed.preflight_ms, BENCH_WARMUP, BENCH_WARMUP_MS),
		min_iterations: pass_min_iterations,
		gc_each_iteration: BENCH_GC
	};
	return run_child<RowResult>(
		'bench_row.ts',
		spec,
		`${RUN_DIR}/row_${id}.spec.json`,
		`${group.name}/${row.name} (pass ${pass + 1}/${BENCH_PASSES})`
	);
}

/**
 * Time one group: `BENCH_PASSES` passes over its timed rows, each row of each pass
 * in a fresh process, then pool every row's passes into one `BenchmarkResult`.
 *
 * The passes of a group run back to back, so whatever the machine does over the
 * length of a run (a governor, a neighbour, the weather) falls on the rows a ratio
 * compares rather than between them.
 */
async function time_group(group: PreflightGroup): Promise<void> {
	const rows = group.rows.filter((row) => row.timed !== null);
	log(`\n▶ ${group.name}`);

	const passes = new Map<string, RowResult[]>(rows.map((row) => [row.tracking_key, []]));
	for (let pass = 0; pass < BENCH_PASSES; pass++) {
		if (rows.length > 0) log(`  pass ${pass + 1}/${BENCH_PASSES}`);
		const order = pass_order(rows, pass, BENCH_PASSES);
		for (let i = 0; i < order.length; i++) {
			const row = order[i];
			const result = time_row_pass(group, row, pass);
			passes.get(row.tracking_key)!.push(result);
			for (const [pattern, count] of Object.entries(result.suppressed_noise)) {
				suppressed_noise.set(pattern, (suppressed_noise.get(pattern) ?? 0) + count);
			}
			// the pass's own cleaned mean — the estimator the pooled line below uses
			const { mean_ns } = new BenchmarkStats(result.timings_ns);
			log(`    [${i + 1}/${order.length}] ${row.name}: ${format_rate(row.tracking_key, mean_ns)}`);
			await yield_to_signals();
		}
	}

	// Pooled, in registration order — the order every table and the baseline read.
	const results: BenchmarkResult[] = rows.map((row) => {
		const row_passes = passes.get(row.tracking_key)!;
		const summary = summarize_passes(row_passes.map((p) => p.timings_ns));
		const heaps = row_passes.map((p) => p.settled_heap_bytes).sort((a, b) => a - b);
		timed_rows.set(row.tracking_key, {
			summary,
			// one protocol per row, so every pass reports the same count
			warmup_iterations: row_passes[0].warmup_iterations,
			settled_heap_bytes: heaps[Math.floor(heaps.length / 2)]
		});
		return {
			name: row.name,
			stats: new BenchmarkStats(summary.timings_ns),
			iterations: summary.timings_ns.length,
			total_time_ms: row_passes.reduce((sum, p) => sum + p.total_time_ms, 0),
			timings_ns: summary.timings_ns,
			// the per-pass protocol, identical across the row's passes
			budget: row_passes[0].budget
		};
	});
	if (BENCH_PASSES > 1 && results.length > 0) {
		log(`  pooled`);
		for (const result of results) {
			const tracking_key = task_tracking_by_group.get(group.name)!.get(result.name)!;
			const { pass_spread } = timed_rows.get(tracking_key)!.summary;
			log(
				`    ${result.name}: ${format_rate(tracking_key, result.stats.mean_ns)} · ` +
					`pass spread ${(pass_spread * 100).toFixed(1)}%`
			);
		}
	}
	all_group_results.push({ name: group.name, results });
}

if (COVERAGE_ONLY) {
	log(
		'\nCoverage-only mode: skipping the timed benchmark phase (coverage is a pre-flight product).'
	);
} else {
	// Both the pre-warmup settle and the opt-in per-iteration hook go through
	// `globalThis.gc`, which exists only when the runtime was started with
	// `--expose-gc` (every timed `bench:*:run` task passes it, and a row's process is
	// started with this one's flags). A silent no-op would remove a control while the
	// numbers still looked publishable, so say so — here rather than at startup, since
	// the coverage-only run has no timed phase to bias and its task passes no such flag.
	if (typeof globalThis.gc !== 'function') {
		log(
			'⚠ globalThis.gc is unavailable (no --expose-gc): rows warm up from an unsettled heap' +
				(BENCH_GC ? ', and BENCH_GC=1 is inert' : '')
		);
	}

	log(
		`\nRunning benchmarks (${BENCH_PASSES} pass${BENCH_PASSES === 1 ? '' : 'es'}, ` +
			`every row of every pass in a fresh process):`
	);
	for (const group of snapshot.groups) await time_group(group);
}

/**
 * The run's between-process noise (`lib/bench_plan.ts` `summarize_process_noise`):
 * how far two fresh processes of the same row sat apart, over every timed row.
 * `null` when nothing was timed or the run made a single pass.
 */
const process_noise: ProcessNoise | null = summarize_process_noise(
	[...timed_rows.values()].map((row) => row.summary.pass_p50_ns)
);
if (process_noise !== null) {
	const pct = (v: number): string => `${(v * 100).toFixed(1)}%`;
	log(
		`\nProcess noise (two fresh processes of one row, ${process_noise.pairs} pairs): ` +
			`median ${pct(process_noise.median)}, p95 ${pct(process_noise.p95)}, max ${pct(process_noise.max)}`
	);
}

/**
 * The coefficient-of-variation above which a timed row's number is disclosed as
 * UNSTABLE rather than published bare.
 *
 * Every other shortfall this report can carry says so — `unavailable`,
 * `binary_sizes_absent`, `suppressed_noise`, `output_digest_ungraded`, the `⚠ files`
 * per-group note. How stable the timing itself was is the one property that never
 * did, and it is the property every published `Nx` rests on.
 *
 * 10% is ~3× the measured p90. Across the three committed perf reports (128 timed
 * rows) cv runs median 1.0%, p90 3.1% — so ordinary variation is nowhere near this,
 * and a row that trips it is doing something other than varying (the live rows are the
 * per-runtime reports' §Unstable Rows; a restated value here would only go stale — the
 * calibration figures are `entries[].cv` in the committed reports). Deliberately tighter
 * than `benchmark_baseline_compare`'s 30% noise gate, which answers a different
 * question (is a REGRESSION real) on a run this one never makes: that path needs
 * `--compare-baseline`, so a plain `deno task bench` reaches no stability check at all.
 */
const UNSTABLE_CV_THRESHOLD = 0.1;

/**
 * The `|drift|` past which a row is unstable regardless of its cv (see
 * `BaselineEntry.drift`). 5% is well outside stationary variation (a stationary
 * row's two half-medians agree to well under 1% at any sample count the bench
 * reaches) and well inside the regime this exists for: the case that motivated it
 * stepped +25–45% mid-row and, at most sample counts, published a cleaned cv under 6%.
 */
const UNSTABLE_DRIFT_THRESHOLD = 0.05;

/**
 * Below this many raw timings the RAW cv also trips a row. With few samples one
 * deviant sweep is a real share of the measurement — and is exactly what the MAD
 * cleaner's keep-closest fallback blends into the mean — so a raw cv past the
 * threshold there is the disclosure the cleaned cv withheld. With hundreds of
 * samples the raw cv is dominated by isolated pauses (one 80 ms GC among 600 × 8 ms
 * sweeps reads 35%) that the cleaner rightly removes and the upper percentiles
 * already report; there the drift statistic, not the raw cv, is the drift detector.
 */
const RAW_CV_SAMPLE_CEILING = 30;

/**
 * The `pass_spread` past which a row is unstable regardless of the rest (see
 * `BaselineEntry.pass_spread`): the row's passes — fresh processes of the same
 * thing — sat further apart than this, so its level depends on the process it was
 * drawn in, and the pooled mean sits between levels rather than on one.
 *
 * The same 5% as `UNSTABLE_DRIFT_THRESHOLD`, and for the same kind of reason: both
 * are a level shift read from medians, one inside a process and one between two.
 */
// TODO: calibrate against `process_noise` once full runs under all three runtimes
// exist — the threshold wants to sit well clear of the run's own p95.
const UNSTABLE_PASS_SPREAD_THRESHOLD = 0.05;

/**
 * Timed rows whose measurement was too noisy to read at face value, worst first.
 *
 * Ratios are the report's product and each one divides two of these means, so an
 * unstable row silently widens every comparison it appears in — including the
 * cross-runtime table, whose whole subject is small per-runtime deltas.
 */
function unstable_rows(data: Baseline): Array<{
	label: string;
	cv: number;
	cv_raw: number | null;
	drift: number | null;
	pass_spread: number | null;
	samples: number | null;
	raw_samples: number | null;
}> {
	// Four readings, any one of which trips the row: the cleaned cv (ordinary noise),
	// the RAW cv (a second mode the cleaner deleted), the drift (a cost that moved
	// while a process was measured) and the pass spread (a level that differs between
	// processes). The cleaned cv alone was blind to the last three — a bimodal row can
	// clean to a quiet cv over a mean that is neither mode.
	return data.entries
		.filter(
			(e) =>
				e.cv !== null &&
				(e.cv >= UNSTABLE_CV_THRESHOLD ||
					(e.cv_raw !== null &&
						e.cv_raw >= UNSTABLE_CV_THRESHOLD &&
						e.raw_sample_size !== null &&
						e.raw_sample_size < RAW_CV_SAMPLE_CEILING) ||
					(e.drift !== null && Math.abs(e.drift) >= UNSTABLE_DRIFT_THRESHOLD) ||
					(e.pass_spread !== null && e.pass_spread >= UNSTABLE_PASS_SPREAD_THRESHOLD))
		)
		.map((e) => ({
			label: `${e.group}/${e.name}`,
			cv: e.cv as number,
			cv_raw: e.cv_raw,
			drift: e.drift,
			pass_spread: e.pass_spread,
			samples: e.sample_size ?? null,
			raw_samples: e.raw_sample_size ?? null
		}))
		.sort(
			(a, b) =>
				Math.max(b.cv, b.cv_raw ?? 0, Math.abs(b.drift ?? 0), b.pass_spread ?? 0) -
				Math.max(a.cv, a.cv_raw ?? 0, Math.abs(a.drift ?? 0), a.pass_spread ?? 0)
		);
}

//
// Baseline Handling
//

interface BaselineEntry {
	name: string;
	group: string;
	// Timing stats, over the POOLED timings of the row's passes (every pass a fresh
	// process — `passes` below). `null` in a coverage-only run
	// (`BENCH_COVERAGE_ONLY=1`), which skips the timed phase and emits coverage from
	// pre-flight alone. A timed run always fills them.
	mean_ns: number | null;
	p50_ns: number | null;
	p75_ns: number | null;
	p90_ns: number | null;
	p95_ns: number | null;
	p99_ns: number | null;
	min_ns: number | null;
	max_ns: number | null;
	std_dev_ns: number | null;
	cv: number | null;
	ops_per_second: number | null;
	sample_size: number | null;
	/**
	 * Stability read from the RAW timings, which the cleaned `cv` above cannot see:
	 * `cv_raw` is std_dev / mean over every pooled timing before outlier removal, and
	 * `drift` is median(second half) / median(first half) − 1 in iteration order —
	 * medians, so an isolated pause among hundreds of samples does not read as drift,
	 * while a level shift (four fast sweeps then three slow) still does. A row whose
	 * cost changes WHILE it is measured — a wasm heap that leaks per sweep and tips the
	 * engine into a slower regime mid-row — reads as drift here while its cleaned `cv`
	 * can read as quiet: the MAD cleaner deletes or blends a second mode rather than
	 * reporting it, so the cleaned mean is then neither mode and `cv` says nothing.
	 *
	 * `drift` is taken WITHIN each pass — it is only meaningful over one process's
	 * timings — and the row publishes the pass furthest from zero, signed. A level
	 * shift BETWEEN two passes is not drift and is not counted here: that is
	 * `pass_spread`.
	 *
	 * `raw_sample_size` is the pooled timing count before cleaning (`sample_size` is
	 * after); `outlier_ratio` is the share the cleaner removed. `null` on a
	 * coverage-only row.
	 */
	cv_raw: number | null;
	drift: number | null;
	raw_sample_size: number | null;
	outlier_ratio: number | null;
	/**
	 * The protocol ONE PASS of this row ran under — its warmup sweeps and its sweep
	 * floor — so two runtimes that ran one row differently would be legible as two
	 * protocols rather than as one ratio. Per pass: a floor-bound row's
	 * `raw_sample_size` is `min_iterations × passes`. `null` on a coverage-only row.
	 */
	warmup_iterations: number | null;
	min_iterations: number | null;
	/**
	 * How many fresh processes the row was timed in (`BENCH_PASSES`), each pass's
	 * median sweep time in pass order, and how far apart those medians sat — the
	 * largest over the smallest, minus one (`lib/bench_plan.ts` `summarize_passes`).
	 *
	 * `pass_spread` is the reading no single process can make. Each pass is one draw
	 * of the row in a process that holds nothing else, so the spread is what another
	 * draw could have published — and a row whose passes disagree by more than its
	 * `cv` within any one of them has a level that depends on the process, which a
	 * single-process `cv` reports as quiet. `0` for a one-pass run. `null` on a
	 * coverage-only row.
	 *
	 * Since `version` 21.
	 */
	passes: number | null;
	pass_p50_ns: number[] | null;
	pass_spread: number | null;
	/**
	 * The JS heap (`heapUsed`, bytes) the row's warmup began from, read straight after
	 * a full collection in the row's own process — its engine and its file set, nothing
	 * else — and the median over the row's passes. A diagnostic, not a measurement:
	 * under JSC a pure-JS row's level depends on the heap it starts from, so it is what
	 * to compare first when one bun row reads differently across two runs — equal here
	 * means the heap is not why. Comparable across RUNS of one runtime, never across
	 * runtimes: JSC's figure counts the memory its heap answers for (wasm linear
	 * memories, buffers), V8's does not. `null` on a coverage-only row.
	 */
	settled_heap_bytes: number | null;
	/**
	 * sha1 (first 12 hex digits) of the newline-joined sorted paths this row was timed
	 * on — what `compose_reports.ts` compares across runtimes, since equal
	 * `files_iterated` COUNTS never proved equal sets. `null` when nothing was timed.
	 */
	files_iterated_digest: string | null;
	/**
	 * Files this impl successfully processed during preflight / the language's
	 * total discovered files — the per-impl `Coverage:` line in the markdown
	 * report, surfaced here so consumers can see which libs support which parts
	 * of the corpus without parsing prose. `null` when tracking is unavailable
	 * (e.g. a result with no resolvable tracking_key). Note: this is preflight
	 * support, not the timed set — in `intersection` mode the timed file count
	 * is the smaller per-group intersection.
	 */
	files_processed: number | null;
	files_total: number | null;
	/**
	 * Files this impl was actually timed on — the per-group `Files (intersection):`
	 * set in default mode (uniform across a group), or the impl's own preflight
	 * success set under `BENCH_MODE=union`. Distinct from `files_processed`
	 * (preflight support): this is what the `ops_per_second`/throughput numbers
	 * reflect. `null` when tracking is unavailable.
	 */
	files_iterated: number | null;
	/**
	 * What a PARSE row hands JS in its group's language (`report.ts` `PayloadTier`) —
	 * a ratio between two rows of one parse group is payload-matched iff these are equal
	 * and not `own_shape`. `null` on every format row (the product is a string either
	 * way) and on a parse row the tier table does not list in that language, which the
	 * run warns about at init.
	 *
	 * Since `version` 16; per language, with `drop_in_superset`, since 20.
	 */
	payload: PayloadTier | null;
	/**
	 * The JS runtime that produced this row (`deno` | `node` | `bun`). Every row
	 * carries it so a reader never has to guess what produced a number — the
	 * runtime-labeled sibling reports (`report.deno.*` / `report.node.*`) compose
	 * at the display layer (tsv.fuz.dev), and a per-runtime delta on the same row
	 * is the detector for a runtime-specific measurement artifact.
	 */
	runtime: Runtime;
}

/**
 * Package versions used in the benchmark run — the report's `ReportVersions`
 * (canonical oracles + whichever alternatives loaded) plus tsv's own. Adding an
 * impl means extending `AlternativeVersionInfo` in `lib/report.ts`, one place,
 * rather than three hand-kept field lists.
 */
interface BaselineVersions extends ReportVersions {
	/** tsv's own version, from `Cargo.toml` `[workspace.package]` (the binary under test). */
	tsv: string;
}

/**
 * The report's schema version — every consumer reads it to tell "this producer did
 * not record that field" from "there was nothing to record".
 *
 * BUMP IT whenever a field is added, removed, or has its meaning or key names
 * changed, and say what the new number means in one line below. `compose_reports.ts`
 * carries its own, for the combined report, on the same rule; `../tsv.fuz.dev`'s
 * `benchmark_data.ts` mirrors this shape field for field and degrades on an older
 * report, so a bump here is a change there too.
 *
 * "A field" means anywhere in the emitted shape, NESTED ONES INCLUDED — a new key
 * on `Machine` or `CorpusSource` is as much a schema change as a new key on
 * `Baseline`, and the quieter one, since the `Baseline` field holding it doesn't
 * move. `Machine` is the case with a second reader: `compose_reports.ts` imports
 * that type to describe sibling reports written by older benches, so a field added
 * there is absent from data the composer already reads (see `Machine`).
 *
 * 13: `output_digest_ungraded` — files a byte-graded row accepted whose output the
 * byte-parity check could not digest, keyed `"<group>/<row>"`. Records a
 * measurement the run could not make, where every other field records one it did.
 *
 * 14: `corpus_snapshot` — the `fuzdev/corpora` checkout (URL + commit) every
 * `real`/`framework` source was read from: one roll-up commit for the whole
 * real-code corpus, so the numbers are reproducible with one clone. Absent on a
 * conformance-only run (no real code) or when the snapshot isn't checked out. A
 * COMMIT, where the gate pin (`GATE_CHECKOUT_IDS['../corpora']`) is that
 * checkout's `collections/` TREE id: a reader clones a commit and the commit fixes
 * the tree, so the two agree by construction — but several commits can name one
 * tree (the snapshot repo's own tooling commits do), so this field moving between
 * two reports does not by itself mean the corpus did.
 *
 * 15: per-row stability read from the RAW timings — `cv_raw`, `drift`,
 * `raw_sample_size`, `outlier_ratio` — beside the cleaned `cv`; the protocol the row
 * ran under (`warmup_iterations`, `min_iterations`); and `files_iterated_digest`, a
 * hash of the timed path set. Every one answers a question the 14-shape could not: a
 * row whose cost CHANGED while it was measured (a leaking wasm heap) has its second
 * mode deleted or blended by the MAD cleaner and can publish a quiet `cv` over a mean
 * that is neither mode — `drift` and `cv_raw` see the raw series the cleaner does not
 * report; two runtimes tiering one row differently are two protocols, not one ratio;
 * and equal `files_iterated` counts never proved equal sets.
 *
 * 16: `omissions` — per timed group, the files and bytes its intersection left out
 * and the rows that left them, by `PerfOmitCategory` (perf surface only); and a
 * per-row `payload` tier on the parse rows, so an `Nx` built from two rows can say
 * whether their products match.
 *
 * 17: `binary_sizes[].kind` gains `js` — the canonical toolchain's size rows, which
 * are minified JS bundles the harness builds itself (`lib/canonical_bundles.ts`)
 * where every `wasm`/`native` row is a file a package ships.
 *
 * 18: per-row `settled_heap_bytes` — the JS heap each row's warmup began from. JSC
 * paces its collections by live heap size, so a pure-JS row's level under bun depends
 * on it; the field shows whether two runs of a row started from the same place.
 *
 * 19: `coverage_by_source` cells gain `script_only` — of `processed`, the files a
 * goal-fallback source accepted only on its Script retry (`SourceFile.goal_fallback`;
 * conformance surface only, absent when the retry added none), and conformance
 * reports gain `exclusion_caches` — each exclusion cache the view applied, by label,
 * its size or `null` when absent.
 *
 * 20: `payload` is keyed on the row IN ITS GROUP'S LANGUAGE and gains
 * `drop_in_superset`: the CSS oracle row reads `span_only` (`parseCss` emits no `loc`),
 * and the `+locations` rows read `drop_in_superset` on Svelte and CSS, where their
 * `loc` on every positioned object is a superset of the oracle's. A consumer that
 * matched 19's tiers by row name alone called both pairs matched. tsv's parse rows
 * are also named for the packages' API from 20: the default parse is `tsv` /
 * `tsv-wasm` (the format rows' names, in the parse groups) and `{locations: true}`
 * is `tsv+locations` / `tsv-wasm+locations`, so a row name identifies a row only
 * together with its group.
 *
 * 21: every timed row is measured in fresh PROCESSES, several passes of them, and
 * its statistics pool the passes (`bench.ts` §Process model). Per row: `passes`,
 * `pass_p50_ns` and `pass_spread` (how far the row's passes sat apart). Top level:
 * `process_noise` (the same reading over the whole run). Three existing per-row
 * fields change MEANING, not name: `min_iterations` and `warmup_iterations` are per
 * pass, so a floor-bound row's `raw_sample_size` is `min_iterations × passes` where
 * it was `min_iterations`; `drift` is the within-pass drift furthest from zero
 * (a shift between passes is `pass_spread`'s); and `settled_heap_bytes` is the heap
 * of the row's own process, the median over its passes. The canonical rows no longer
 * carry a higher sweep floor than the rest — every row's floor is the same per pass.
 */
const REPORT_SCHEMA_VERSION = 21;

interface Baseline {
	/** See `REPORT_SCHEMA_VERSION`. */
	version: number;
	/** The JS runtime that produced this report (`deno` | `node` | `bun`). Mirrors
	 * the per-row `runtime` and matches the `report.<runtime>.{json,md}` filename. */
	runtime: Runtime;
	/**
	 * Which corpus/surface produced this report: `perf` (real-world corpus,
	 * parse + format groups — `report.<runtime>.*`) or `conformance`
	 * (fixtures-only corpus, disjoint from perf, a suite with a validity oracle or
	 * harness filtered to what it calls valid, parse groups only —
	 * `report.conformance.<runtime>.*`). See `BENCH_CORPUS`.
	 *
	 * Since `version` 6.
	 */
	corpus_kind: CorpusKind;
	timestamp: string;
	git_commit: string | null;
	/**
	 * The machine that produced this report — CPU model, OS/arch, and the
	 * runtime's own version. The throughput numbers are machine-relative, so
	 * without this a report copied to the site (or diffed against an older one)
	 * can't distinguish a code change from a different box. See `Machine`.
	 *
	 * Since `version` 7.
	 */
	machine: Machine;
	corpus: {
		svelte: number;
		typescript: number;
		css: number;
	};
	/**
	 * Per-entry corpus composition (entry path + loaded file count). Missing
	 * entries (an absent `../wpt` or `../test262` checkout, an unbuilt harvest
	 * cache) are silently skipped by the loader, so without this a report
	 * produced on a partial machine would be indistinguishable from a full one.
	 */
	corpus_sources: CorpusSource[];
	/**
	 * The exclusion caches the conformance view applied, by label, each its size
	 * (held to its exact pin) — `null` for an absent one, which only a
	 * `BENCH_ALLOW_MISSING=1` run can publish (`enforce_exclusion_caches`), so a
	 * consumer can refuse it. Conformance reports only; since `version` 19.
	 */
	exclusion_caches?: Record<string, number | null>;
	/**
	 * The real-code snapshot the `real`/`framework` sources were read from — the
	 * `fuzdev/corpora` checkout at its commit (`subpath` empty). One roll-up commit
	 * for every real-code source (each source's own `repo` names the upstream that
	 * commit vendored); the commit, not the `collections/` tree id the corpus gates
	 * pin, since a tree id names bytes but cannot be cloned (see
	 * `REPORT_SCHEMA_VERSION` 14). Absent when there is no real code in the corpus
	 * (conformance runs) or the snapshot isn't checked out.
	 */
	corpus_snapshot?: CorpusRepoRef;
	/**
	 * Per-corpus-source coverage — `group → source → impl → {processed, total}`,
	 * the machine-readable half of the report's per-source tables. Present on
	 * COVERAGE-ONLY (conformance) runs only; `undefined` on the perf surface, where
	 * every cell would read 100% by construction. Read these rather than a group's
	 * aggregate: the aggregate blends corpora that answer different questions, and
	 * on a corpus filtered by its own canonical parser that parser's row is 100% by
	 * construction rather than by achievement.
	 *
	 * Since `version` 8.
	 */
	coverage_by_source?: Record<string, Record<string, Record<string, SourceCoverageCell>>>;
	versions: BaselineVersions;
	binary_sizes: BinarySize[];
	/**
	 * Labels the size table reached for and did not find (see
	 * `CollectedBinarySizes.absent`). Empty `[]` when every expected artifact was
	 * on disk.
	 *
	 * The size table is the one section whose COMPOSITION is machine-dependent —
	 * rows exist only for built artifacts — so without this a report from a
	 * partially-built tree is indistinguishable from one where an artifact stopped
	 * being produced. A tsv variant listed here usually just means its optional
	 * build task wasn't run; a third-party label means its package shipped nothing
	 * where this module looked.
	 *
	 * Since `version` 11.
	 */
	binary_sizes_absent: string[];
	/**
	 * Per timed group, what its intersection LEFT OUT and which rows left it out —
	 * files and BYTES, against the group's totals, with each row's failures by
	 * `PerfOmitCategory` (see `GroupOmissions`). A file any timed row fails leaves
	 * EVERY row's timed set, so one tool's omit moves every number in the group, and
	 * a file count understates it (one harvested stylesheet can be a visible share of a
	 * group's bytes).
	 * A group nothing failed is listed with zeroes, so an absent group means
	 * "not measured" rather than "nothing omitted".
	 *
	 * Perf surface, intersection mode, timed runs only; `undefined` elsewhere.
	 *
	 * Since `version` 16.
	 */
	omissions?: GroupOmissions[];
	entries: BaselineEntry[];
	/**
	 * Counts of stderr noise from third-party impls that the harness silenced
	 * during the run, keyed by message pattern (e.g. `oxfmt::textToDoc`). Surfaced
	 * machine-readably so silenced upstream crashes don't vanish; not rendered in
	 * the markdown report (counts are run-variant and would churn the committed
	 * report). Empty `{}` when nothing was suppressed.
	 */
	suppressed_noise: Record<string, number>;
	/**
	 * Files a byte-graded row ACCEPTED but whose output could not be digested, as
	 * `{"<group>/<row>": count}` — so `check_variant_parity`'s byte half did not
	 * grade them. Empty `{}` when every accepted output was gradeable, which is the
	 * healthy state.
	 *
	 * JSON-only, like `suppressed_noise`, and here for the same reason: it records a
	 * measurement the run could NOT make. The one known cause is a pathologically
	 * deep AST overflowing V8's recursive `JSON.stringify` (see `output_digest`);
	 * a count that grows is the byte check quietly covering less.
	 */
	output_digest_ungraded: Record<string, number>;
	/**
	 * Same-engine pairs — one engine behind two bindings, or one binding under two
	 * options — whose pre-flight accept sets or output bytes
	 * disagreed (see `check_variant_parity`). Empty `[]` when every pair agrees
	 * — the healthy state. JSON-only, like `suppressed_noise`: a non-empty list
	 * in a committed report is a binding-boundary bug surfacing at review time.
	 */
	variant_parity: VariantParityFinding[];
	/**
	 * Optional impls that failed to initialize on the machine that produced this
	 * report, with the first line of each load error (see `init_optional`). Empty
	 * `[]` on a full machine — the healthy state.
	 *
	 * JSON-only, like the two fields above, and here for the same reason one step
	 * further out: those record a row behaving wrongly, this records a row that is
	 * NOT THERE. An impl that stops loading takes its column out of every table
	 * silently as far as the committed report is concerned — the ⚠ init line lives
	 * only in the terminal scroll — so without this a binding broken by a dep bump
	 * reads as a report that simply never had that tool.
	 *
	 * Each entry names the ROWS the failure removed, not just the impl that failed:
	 * a reader (or the site's cross-runtime table) holds a row name and nothing
	 * else, and the init label matches none of them. See `UnavailableImpl`.
	 */
	unavailable: UnavailableImpl[];
	/**
	 * How much two fresh processes of the SAME row disagreed, over every timed row of
	 * the run — `pairs` (row, pass pair) comparisons, with the `median`, `p95` and
	 * `max` of the slower pass median over the faster, minus one
	 * (`lib/bench_plan.ts` `summarize_process_noise`). A process-level A/A the passes
	 * give for free, and the bound to read a small ratio against: it describes one
	 * process against one, and a published row pools all its passes, so a difference
	 * inside `p95` is not one this run measured.
	 *
	 * `null` when nothing was timed (a coverage-only run) or the run made one pass.
	 *
	 * Since `version` 21.
	 */
	process_noise: ProcessNoise | null;
}

/**
 * Read tsv's own version from the workspace `Cargo.toml` (`[workspace.package]`),
 * the single source of truth every crate inherits via `version.workspace = true`
 * and that the published npm packages move together at.
 *
 * THROWS if the file can't be read or the version can't be found — the same
 * posture as `lib/versions.ts`, for the same reason. This string labels the
 * committed report's header and its `versions.tsv` (which `compose_reports.ts`
 * reads), so a defaulted `'unknown'` would not degrade gracefully: it would
 * publish a report that names no version, from a file that is always present in
 * this repo. A miss means the regex stopped matching a reshuffled `Cargo.toml`,
 * which is a bug to fix rather than a state to label.
 */
async function get_tsv_version(): Promise<string> {
	const cargo_toml_path = fileURLToPath(new URL('../../Cargo.toml', import.meta.url));
	const content = await readFile(cargo_toml_path, 'utf8');
	// Match the line-leading `version = "..."` inside the `[workspace.package]` section.
	// `^version` (multiline) avoids matching a `rust-version = "..."` MSRV pin; `[^[]*?`
	// bounds the search to the section by stopping at the next `[` heading.
	const match = content.match(/\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/m);
	if (!match) {
		throw new Error(
			`Cargo.toml has no [workspace.package] version (${cargo_toml_path}) — the report labels ` +
				`every number with it, so it cannot be defaulted`
		);
	}
	return match[1];
}

/** Get current git commit hash */
async function get_git_commit(): Promise<string | null> {
	try {
		const { result, stdout } = await spawn_out('git', ['rev-parse', 'HEAD']);
		if (result.ok && stdout) {
			return stdout.trim().slice(0, 8);
		}
	} catch {
		// Ignore
	}
	return null;
}

/** Null timing stats — the coverage-only entry shape (no timed run happened). */
const NULL_STATS = {
	mean_ns: null,
	p50_ns: null,
	p75_ns: null,
	p90_ns: null,
	p95_ns: null,
	p99_ns: null,
	min_ns: null,
	max_ns: null,
	std_dev_ns: null,
	cv: null,
	ops_per_second: null,
	sample_size: null,
	cv_raw: null,
	drift: null,
	raw_sample_size: null,
	outlier_ratio: null,
	warmup_iterations: null,
	min_iterations: null,
	passes: null,
	pass_p50_ns: null,
	pass_spread: null,
	settled_heap_bytes: null
} as const;

/**
 * `BaselineEntry.payload` for a row: parse groups only — see the field. The group
 * names the language, which the tier is keyed on as much as the row (`PayloadTier`).
 */
function row_payload(group_name: string, row_name: string): PayloadTier | null {
	const [operation, language] = group_name.split('/') as [string, Language];
	return operation === 'parse' ? parse_payload_tier(row_name, language) : null;
}

/**
 * Coverage-only entries, synthesized from pre-flight state (no timed run). One
 * row per impl per group, carrying the per-tool coverage counts with null
 * timing — the shape `derive_conformance_groups` reads. Iterates
 * `LANGUAGES × OPERATIONS` for a stable order matching pre-flight.
 *
 * Two callers, distinguished by `only_coverage_only_tasks`: a coverage-only RUN
 * (`BENCH_COVERAGE_ONLY=1`) synthesizes every row because nothing was timed,
 * while a timed run synthesizes only the coverage-only IMPLS — the rows the
 * bench library never produced a result for (see `BenchmarkTask.coverage_only`).
 */
function build_coverage_entries(only_coverage_only_tasks: boolean): BaselineEntry[] {
	const entries: BaselineEntry[] = [];
	for (const language of LANGUAGES) {
		for (const operation of OPERATIONS) {
			const group_name = `${operation}/${language}`;
			const tracking = task_tracking_by_group.get(group_name);
			if (!tracking) continue;
			for (const [name, tracking_key] of tracking) {
				if (only_coverage_only_tasks && !coverage_only_keys.has(tracking_key)) continue;
				const coverage = effective_corpus_size.get(tracking_key);
				const iterated = iterated_file_count.get(tracking_key);
				entries.push({
					name,
					group: group_name,
					...NULL_STATS,
					files_processed: coverage?.processed ?? null,
					files_total: coverage?.total ?? null,
					files_iterated: iterated ?? null,
					files_iterated_digest: null,
					payload: row_payload(group_name, name),
					runtime: RUNTIME
				});
			}
		}
	}
	return entries;
}

/** Build results data from current benchmark run */
async function build_results_data(
	groups: GroupResults[],
	corpus: { svelte: number; typescript: number; css: number },
	versions: BaselineVersions,
	// The collector's whole answer, not its two halves re-threaded: sizes and
	// absences are one measurement of one table, and splitting them at the call
	// site is what lets a caller pass a `sizes` from one collection beside an
	// `absent` from another.
	collected_sizes: CollectedBinarySizes
): Promise<Baseline> {
	const entries: BaselineEntry[] = [];
	if (COVERAGE_ONLY) {
		entries.push(...build_coverage_entries(false));
	} else {
		for (const group of groups) {
			// Resolve per-impl preflight coverage (the markdown `Coverage:` line) via
			// the same display-name → tracking_key map the report uses.
			const tracking = task_tracking_by_group.get(group.name);
			for (const result of group.results) {
				const tracking_key = tracking?.get(result.name);
				const coverage = tracking_key ? effective_corpus_size.get(tracking_key) : undefined;
				const iterated = tracking_key ? iterated_file_count.get(tracking_key) : undefined;
				// Every result here came out of `time_group`, which records its `TimedRow`.
				const timed = tracking_key ? timed_rows.get(tracking_key) : undefined;
				if (!timed) throw new Error(`${group.name}/${result.name} has a result but no timed row`);
				entries.push({
					name: result.name,
					group: group.name,
					mean_ns: result.stats.mean_ns,
					p50_ns: result.stats.p50_ns,
					p75_ns: result.stats.p75_ns,
					p90_ns: result.stats.p90_ns,
					p95_ns: result.stats.p95_ns,
					p99_ns: result.stats.p99_ns,
					min_ns: result.stats.min_ns,
					max_ns: result.stats.max_ns,
					std_dev_ns: result.stats.std_dev_ns,
					cv: result.stats.cv,
					ops_per_second: result.stats.ops_per_second,
					sample_size: result.stats.sample_size,
					cv_raw: timed.summary.cv_raw,
					drift: timed.summary.drift,
					raw_sample_size: result.timings_ns.length,
					outlier_ratio: result.stats.outlier_ratio,
					// The sweeps each pass actually warmed for (`RowResult.warmup_iterations`):
					// a `reset_heap` row warms outside the timing library's loop, whose own
					// count is 0 for it.
					warmup_iterations: timed.warmup_iterations,
					min_iterations: result.budget.min_iterations,
					passes: timed.summary.pass_p50_ns.length,
					pass_p50_ns: timed.summary.pass_p50_ns,
					pass_spread: timed.summary.pass_spread,
					settled_heap_bytes: timed.settled_heap_bytes,
					files_processed: coverage?.processed ?? null,
					files_total: coverage?.total ?? null,
					files_iterated: iterated ?? null,
					files_iterated_digest:
						(tracking_key ? iterated_files_digest.get(tracking_key) : undefined) ?? null,
					payload: row_payload(group.name, result.name),
					runtime: RUNTIME
				});
			}
		}
		// Coverage-only impls produced no timed result, so the loop above skipped
		// them entirely. Append their rows — null timing, real coverage — so the
		// measurement they DID contribute reaches the report instead of vanishing
		// because it wasn't a throughput number.
		entries.push(...build_coverage_entries(true));
	}

	return {
		version: REPORT_SCHEMA_VERSION,
		runtime: RUNTIME,
		corpus_kind: CORPUS_MODE,
		timestamp: new Date().toISOString(),
		git_commit: await get_git_commit(),
		machine: current_machine(),
		corpus,
		corpus_sources: snapshot.corpus_sources,
		exclusion_caches: IS_CONFORMANCE
			? Object.fromEntries(snapshot.exclusion_caches.map((c) => [c.label, c.size]))
			: undefined,
		corpus_snapshot: snapshot.corpus_snapshot ?? undefined,
		// Per-source coverage, the JSON half of the markdown tables. Coverage-only
		// runs only: on the perf surface every cell would read 100% by construction
		// (an unlisted per-file failure hard-fails the run instead).
		coverage_by_source: snapshot.coverage_by_source ?? undefined,
		versions,
		binary_sizes: collected_sizes.sizes,
		binary_sizes_absent: collected_sizes.absent,
		omissions: snapshot.omissions ?? undefined,
		entries,
		suppressed_noise: Object.fromEntries(suppressed_noise),
		output_digest_ungraded: snapshot.output_digest_ungraded,
		variant_parity: snapshot.variant_parity,
		unavailable: snapshot.unavailable,
		process_noise
	};
}

/** Generate a full markdown report from benchmark data */
function generate_markdown_report(data: Baseline, groups: GroupResults[]): string {
	// Every figure the report labels comes from ONE `Baseline`, and the pre-flight
	// state is read straight from module scope (where the snapshot was unpacked)
	// rather than threaded back into a function that already sits in this module.
	//
	// This took thirteen positional parameters before, six of them those same
	// globals passed through and seven of them fields of the `data` the caller
	// already had — thirteen chances, at each of two call sites, to pair one run's
	// corpus with another's versions. Same hazard `CollectedBinarySizes` names for
	// its own two halves, an order of magnitude wider.
	const { binary_sizes, corpus, versions, timestamp, git_commit, machine } = data;
	const corpus_bytes = snapshot.bytes_by_language;
	const task_tracking = task_tracking_by_group;
	const effective_size = effective_corpus_size;
	const effective_bytes = effective_corpus_bytes;
	const iterated_counts = iterated_file_count;
	const skipped = skipped_files;
	const lines: string[] = [];
	lines.push(
		IS_CONFORMANCE ? '# tsv conformance benchmark results (parse)\n' : '# tsv benchmark results\n'
	);
	const commit_str = git_commit ? ` (${git_commit})` : '';
	lines.push(`**Runtime:** ${RUNTIME}\n`);
	lines.push(
		`**Machine:** ${machine.cpu_model} · ${machine.os}/${machine.arch} · ` +
			`${RUNTIME} ${machine.runtime_version}\n`
	);
	const conformance_note = COVERAGE_ONLY
		? 'conformance — fixtures-only corpus (disjoint from perf; a suite with a validity oracle or harness filtered to what it calls valid), parse groups only; per-tool Coverage lines only (coverage-only run — timed throughput skipped)'
		: 'conformance — fixtures-only corpus (disjoint from perf; a suite with a validity oracle or harness filtered to what it calls valid), parse groups only; the headline is the per-tool Coverage lines (parse success over the valid set), with throughput measured on the all-tools-pass intersection';
	lines.push(
		`**Corpus kind:** ${
			IS_CONFORMANCE ? conformance_note : 'perf — real-world code only (fixture suites excluded)'
		}\n`
	);
	lines.push(`**Date:** ${timestamp} — tsv ${versions.tsv}${commit_str}\n`);

	const total_files = corpus.svelte + corpus.typescript + corpus.css;
	const total_bytes = corpus_bytes.svelte + corpus_bytes.typescript + corpus_bytes.css;
	lines.push(
		`**Corpus:** ${corpus.svelte} Svelte (${format_mb(corpus_bytes.svelte)}), ` +
			`${corpus.typescript} TypeScript (${format_mb(corpus_bytes.typescript)}), ` +
			`${corpus.css} CSS (${format_mb(corpus_bytes.css)}) — ` +
			`${total_files} files, ${format_mb(total_bytes)} total\n`
	);
	// The one commit that reproduces every real-code source below (each of them a
	// collection of the snapshot, linked to its own upstream in the sources table).
	if (data.corpus_snapshot) {
		const snap = data.corpus_snapshot;
		lines.push(
			`**Corpus snapshot:** [${snap.slug}@${snap.commit.slice(0, 9)}](${snap.url}/tree/${snap.commit}) — ` +
				`the real-code sources are its collections, vendored at this commit\n`
		);
	}
	if (data.corpus_sources.length > 0) {
		lines.push(
			`**Sources:** ${data.corpus_sources.map((s) => `${s.path} (${s.files})`).join(', ')}\n`
		);
	}
	if (snapshot.exclusion_caches.length > 0) {
		lines.push(
			`**Excluded by cache:** ${snapshot.exclusion_caches
				.map((c) => `${c.label} (${c.size ?? 'ABSENT — not comparable'})`)
				.join(', ')}\n`
		);
	}

	// Versions
	const version_parts = [
		`svelte@${versions.svelte}`,
		`acorn@${versions.acorn}`,
		`acorn-typescript@${versions.acorn_ts}`,
		`prettier@${versions.prettier}`,
		`prettier-plugin-svelte@${versions.prettier_svelte}`
	];
	version_parts.push(...alternative_version_parts(versions));
	lines.push(`**Versions:** ${version_parts.join(', ')}\n`);

	// A row absent from a coverage report reads as "not measured"; say why. Each
	// claim was CHECKED against this surface's registry by the pre-flight process,
	// before the run's work — see `SURFACE_DISCLOSURES` in `bench_preflight.ts`.
	lines.push(...snapshot.surface_disclosure_prose);

	lines.push(
		'**Methodology:** Single-threaded — every implementation formats/parses one file at a time, ' +
			'measured sequentially with no cross-file parallelism. One timed iteration is one full sweep ' +
			'over the group\u2019s iterated file set, so the absolute columns (sweeps/sec, p50\u2013p99, min/max) ' +
			'are per-sweep, not per-file — divide by the group\u2019s file count (the Files lines / `(Mf)` ' +
			'annotations) for per-file figures; ratios and MB/s are denominated consistently either way. ' +
			'This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.\n'
	);
	if (!COVERAGE_ONLY) {
		const noise = data.process_noise;
		const pct = (v: number): string => `${(v * 100).toFixed(1)}%`;
		lines.push(
			`**Isolation:** every row is timed in a process of its own that loads that row’s engine ` +
				`and nothing else measured, ${BENCH_PASSES} time${BENCH_PASSES === 1 ? '' : 's'} over ` +
				`(passes), each pass taking a group’s rows in a different order; a row’s statistics pool ` +
				`its passes.` +
				(noise === null
					? ''
					: ` Two fresh processes of the same row sat ${pct(noise.median)} apart at the median, ` +
						`${pct(noise.p95)} at the 95th percentile and ${pct(noise.max)} at most ` +
						`(${noise.pairs} pass pairs) — a ratio inside that is not a difference this run measured.`) +
				'\n'
		);
	}

	// Coverage-only run: no timed groups exist, so render the per-tool coverage
	// tables straight from pre-flight state (the loop over timed groups below no-ops).
	if (COVERAGE_ONLY) {
		lines.push(
			...generate_coverage_only_markdown(LANGUAGES, OPERATIONS, task_tracking, effective_size),
			...generate_coverage_by_source_markdown(LANGUAGES, OPERATIONS, coverage_by_source)
		);
		lines.push(
			'**The test262 source is tsv-scope-filtered, and it favors tsv.** The cache the ' +
				'`test262` source reads is the expected-positive subset of the tests tsv’s own runner ' +
				'GRADES — `test262 --emit-manifest` (`crates/tsv_debug/src/test262/`) drops the tests ' +
				'outside tsv’s scope before the split, every Annex B `noStrict` positive among them, a ' +
				'grammar tsv declines as a non-browser host and that acorn, oxc, swc, tsc and yuku all ' +
				'parse. So tsv reads 100% on that source by construction, the way tsc does on the tsc ' +
				'corpus and svelte/compiler on the Svelte set, and a rival’s number there is its rate on ' +
				'tsv’s slice, not on test262. The positive/negative split itself is tool-neutral; the ' +
				'graded subset it starts from is not.\n'
		);
	}

	for (const group of groups) {
		if (group.results.length === 0) continue;
		const [operation, language] = group.name.split('/') as ['parse' | 'format', Language];
		// Use the canonical reference as the bench-table baseline. Without this,
		// the library picks the fastest task (often `tsv-internal`, a non-public
		// optimization variant) which is not the comparison readers want.
		const baseline =
			operation === 'format' ? CANONICAL_FORMATTER_ROW : CANONICAL_PARSER_ROWS[language];
		const baseline_exists = group.results.some((r) => r.name === baseline);

		const tracking = task_tracking.get(group.name);
		// Build display-name → iterated-count map for this group, so the table
		// renderer can append `(Mf)` to each row's `vs baseline` cell.
		const group_iterated_counts = new Map<string, number>();
		if (tracking) {
			for (const [display_name, tracking_key] of tracking) {
				const m = iterated_counts.get(tracking_key);
				if (m !== undefined) group_iterated_counts.set(display_name, m);
			}
		}

		lines.push(`## ${group.name}\n`);
		lines.push(
			generate_group_bench_table_markdown(group.results, baseline_exists ? baseline : undefined)
		);
		lines.push('');

		const files = generate_group_files_markdown(group_iterated_counts);
		if (files) lines.push(files, '');

		const throughput = generate_group_throughput_markdown(group.results, tracking, effective_bytes);
		if (throughput) lines.push(throughput, '');

		const coverage = generate_group_coverage_markdown(group.results, tracking, effective_size);
		if (coverage) lines.push(coverage, '');

		const omitted = generate_group_omissions_markdown(
			data.omissions?.find((o) => o.group === group.name)
		);
		if (omitted) lines.push(omitted, '');

		// Coverage-only impls have no row in the tables above (nothing timed them),
		// so their measurement is rendered here or nowhere.
		const coverage_only_names = tracking
			? [...tracking].filter(([, key]) => coverage_only_keys.has(key)).map(([name]) => name)
			: [];
		const coverage_only = generate_group_coverage_only_markdown(
			coverage_only_names,
			tracking,
			effective_size
		);
		if (coverage_only) lines.push(coverage_only, '');

		if (operation === 'parse') {
			const json_note = generate_json_overhead_note(group.results);
			if (json_note) lines.push(json_note, '');
		}
	}

	// Convention note + Comparisons table are throughput-only — skip them in a
	// coverage-only run (no `Nx` speedups exist).
	if (!COVERAGE_ONLY) {
		// Convention note: every `Nx` in this report is speedup form — values > 1
		// mean self is faster than the opponent. File counts are surfaced per
		// group (Files / Coverage lines) and per row in the Comparisons tables.
		lines.push(
			'_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._\n'
		);
	}

	const binary_size_markdown = generate_binary_size_markdown(binary_sizes);
	if (binary_size_markdown) {
		lines.push(binary_size_markdown);
		lines.push('');
	}

	if (!COVERAGE_ONLY) {
		const comparison_markdown = generate_comparison_markdown(
			groups,
			LANGUAGES,
			iterated_counts,
			task_tracking
		);
		if (comparison_markdown) {
			lines.push(comparison_markdown);
			lines.push('');
		}
		// Consumer-side `{locations: true}` cost note, computed from this run's rows and
		// sitting with the parse comparison since it's about the span-only wire.
		const locations_note = generate_locations_note(groups);
		if (locations_note) lines.push(locations_note, '');
	}

	// Stability disclosure — see `UNSTABLE_CV_THRESHOLD`. Sits with the other
	// shortfall sections rather than in the tables: it qualifies a number that is
	// already printed, and a reader comparing two runtimes' columns needs to know
	// which of them was measured on shaky ground.
	const unstable = unstable_rows(data);
	if (unstable.length > 0) {
		lines.push('## Unstable Rows');
		lines.push('');
		lines.push(
			`${unstable.length} timed row(s) were not stable: a cv past ` +
				`${(UNSTABLE_CV_THRESHOLD * 100).toFixed(0)}% (std_dev / mean — \`cv\` after outlier ` +
				`removal; \`cv (raw)\` before it, which counts only under ${RAW_CV_SAMPLE_CEILING} raw ` +
				`samples, where one deviant sweep is a real share of the row), a drift past ` +
				`${(UNSTABLE_DRIFT_THRESHOLD * 100).toFixed(0)}% (within one pass, the median of the second ` +
				`half of the timings against the first's — a cost that moved WHILE the row was measured, ` +
				`which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the ` +
				`row's worst pass is shown), or a pass spread past ` +
				`${(UNSTABLE_PASS_SPREAD_THRESHOLD * 100).toFixed(0)}% (the row's slowest pass median over ` +
				`its fastest — each pass is a fresh process, so this is a level that depends on the process ` +
				`the row was drawn in). The drift's sign ` +
				`names the mechanism: negative means the row got FASTER while measured (still warming up — ` +
				`under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, ` +
				`thermal). Every \`Nx\` involving ` +
				`one of these divides a mean that may be neither mode — read it as approximate, and ` +
				`re-run before drawing a conclusion from it; a longer window does not converge a drifting ` +
				`row, it moves the answer.`
		);
		lines.push('');
		lines.push('| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |');
		lines.push('| --- | ---: | ---: | ---: | ---: | ---: |');
		const pct = (v: number | null): string => (v === null ? '—' : `${(v * 100).toFixed(1)}%`);
		for (const u of unstable) {
			lines.push(
				`| ${u.label} | ${pct(u.cv)} | ${pct(u.cv_raw)} | ${u.drift === null ? '—' : `${u.drift >= 0 ? '+' : ''}${(u.drift * 100).toFixed(1)}%`} | ` +
					`${pct(u.pass_spread)} | ${u.samples ?? '—'}/${u.raw_samples ?? '—'} |`
			);
		}
		lines.push('');
	}

	const skipped_markdown = generate_skipped_files_markdown(
		skipped,
		MAX_ERROR_MESSAGE_LENGTH,
		args.verbose,
		task_tracking
	);
	if (skipped_markdown) {
		lines.push(skipped_markdown);
		lines.push('');
	}

	return lines.join('\n');
}

/**
 * Save results to the results directory.
 *
 * Always writes a timestamped pair. Only overwrites the canonical
 * `report.<tag>.{json,md}` when `write_report` is true — gated by the caller
 * so that partial runs (BENCH_LIMIT, BENCH_FILTER) don't clobber the committed
 * canonical report. Every filename carries `REPORT_TAG` — runtime-suffixed
 * (`report.deno.*` / `report.node.*`), with conformance runs adding a
 * `conformance.` prefix to the tag — so sibling surfaces never clobber each
 * other.
 */
async function save_results(
	data: Baseline,
	groups: GroupResults[],
	write_report: boolean
): Promise<string> {
	await mkdir(RESULTS_DIR, { recursive: true });
	const timestamp = data.timestamp.replace(/[:.]/g, '-').slice(0, 19);
	const commit = data.git_commit ?? 'unknown';
	const base_path = `${RESULTS_DIR}/${timestamp}_${commit}.${REPORT_TAG}`;

	const markdown = generate_markdown_report(data, groups);

	const json = JSON.stringify(data, null, '\t');
	const writes: Promise<void>[] = [
		writeFile(`${base_path}.json`, json),
		writeFile(`${base_path}.md`, markdown)
	];
	if (write_report) {
		writes.push(
			writeFile(`${RESULTS_DIR}/report.${REPORT_TAG}.json`, json),
			writeFile(`${RESULTS_DIR}/report.${REPORT_TAG}.md`, markdown)
		);
	}
	await Promise.all(writes);

	return base_path;
}

/**
 * Flatten `all_group_results` into a single list with namespaced names. The
 * fuz_util baseline module joins by `result.name` and our task names repeat
 * across groups (`tsv` lives in `format/svelte`, `format/typescript`,
 * `format/css`). Without namespacing, the last write wins and three groups
 * collapse into one.
 */
function flatten_results_for_baseline(groups: GroupResults[]): BenchmarkResult[] {
	const out: BenchmarkResult[] = [];
	for (const group of groups) {
		for (const r of group.results) {
			out.push({ ...r, name: `${group.name}/${r.name}` });
		}
	}
	return out;
}

/**
 * Build the `metadata` bag persisted alongside the library's baseline.
 * Round-trips on `_load` and surfaces as `baseline_metadata` on `_compare` —
 * the library doesn't interpret these fields, we use them ourselves to warn
 * on corpus drift (and to display the same `corpus`/`versions`/`binary_sizes`
 * context).
 */
function build_baseline_metadata(data: Baseline): Record<string, unknown> {
	return {
		corpus: data.corpus,
		versions: data.versions,
		binary_sizes: data.binary_sizes
	};
}

/** Shape of our metadata in the baseline file (best-effort, validated lazily). */
interface BaselineMeta {
	corpus?: { svelte?: number; typescript?: number; css?: number };
}

/** Save the current run as the regression baseline. */
async function save_baseline(data: Baseline): Promise<void> {
	await benchmark_baseline_save(flatten_results_for_baseline(all_group_results), {
		path: BASELINE_DIR,
		metadata: build_baseline_metadata(data)
	});
	log(`Baseline saved to ${BASELINE_DIR}/baseline.json`);
}

/**
 * Compare current results against the stored baseline. Uses Welch's t-test
 * (via `benchmark_baseline_compare`) for significance, methodology-change
 * detection for per-task budget drift, and OR-gated noise warnings on
 * high-cv or high-outlier-ratio rows. There is deliberately no flat ±5% ops/sec
 * gate — see `benchmark_baseline_compare` and the
 * fairness caveats in docs/benchmarks.md.
 */
async function compare_baseline(current: Baseline): Promise<void> {
	const comparison = await benchmark_baseline_compare(
		flatten_results_for_baseline(all_group_results),
		{
			path: BASELINE_DIR,
			// 1.0 means "any statistically significant slowdown counts." Tune
			// upward (e.g. 1.05) to suppress trivial regressions in CI without
			// losing the practical-significance gate already inside the Welch
			// comparison (`min_percent_difference` default 0.10).
			regression_threshold: 1.0,
			// Mark the baseline stale after a week so a long-untouched baseline
			// doesn't quietly mask drift accumulated over months.
			staleness_warning_days: 7
		}
	);

	if (!comparison.baseline_found) {
		console.error(
			`\nNo baseline found at ${BASELINE_DIR}/baseline.json. Run with --save-baseline first.`
		);
		return;
	}

	log('\n' + '='.repeat(80));
	log('BASELINE COMPARISON');
	log('='.repeat(80));

	// Corpus-drift warning — the library carries our metadata verbatim but
	// doesn't compare it. Walk it ourselves so a corpus that grew or shrunk
	// between baseline and current is still surfaced (the per-task results
	// would silently move with the corpus otherwise).
	const meta = comparison.baseline_metadata as BaselineMeta | null;
	const baseline_corpus = meta?.corpus;
	const corpus_match =
		baseline_corpus &&
		baseline_corpus.svelte === current.corpus.svelte &&
		baseline_corpus.typescript === current.corpus.typescript &&
		baseline_corpus.css === current.corpus.css;
	if (baseline_corpus && !corpus_match) {
		log(`\n⚠️  Corpus size differs from baseline:`);
		log(
			`   Baseline: svelte=${baseline_corpus.svelte}, ts=${baseline_corpus.typescript}, css=${baseline_corpus.css}`
		);
		log(
			`   Current:  svelte=${current.corpus.svelte}, ts=${current.corpus.typescript}, css=${current.corpus.css}`
		);
	}

	log('');
	log(benchmark_baseline_format(comparison));
}

//
// Output
//

// The size table, collected by the pre-flight process (it gates each third-party
// row on the impl having loaded, which only that process knows).
const collected_sizes = snapshot.binary_sizes;
const binary_sizes = collected_sizes.sizes;

// Build results data (used by all output paths and always saved)
const versions: BaselineVersions = { tsv: await get_tsv_version(), ...snapshot.versions };
const results_data = await build_results_data(
	all_group_results,
	snapshot.corpus,
	versions,
	collected_sizes
);

if (args.json) {
	// JSON output (same structure as saved results)
	console.log(JSON.stringify(results_data, null, '\t'));
} else if (args.markdown) {
	console.log(generate_markdown_report(results_data, all_group_results));
} else {
	// Standard text output. The timed summary is empty in coverage-only mode —
	// the effective-corpus report below carries the coverage picture instead.
	if (!COVERAGE_ONLY) {
		console.log(generate_summary_report(all_group_results, LANGUAGES));
	}

	console.log(generate_versions_info(versions));

	const effective_corpus_report = generate_effective_corpus_report(
		effective_corpus_size,
		task_tracking_by_group,
		COVERAGE_ONLY
	);
	if (effective_corpus_report) {
		console.log(effective_corpus_report);
	}

	const skipped_report = generate_skipped_files_report(
		skipped_files,
		MAX_ERROR_MESSAGE_LENGTH,
		args.verbose,
		task_tracking_by_group
	);
	if (skipped_report) {
		console.log(skipped_report);
	}

	const binary_size_report = generate_binary_size_report(binary_sizes);
	if (binary_size_report) {
		console.log(binary_size_report);
	}

	// Compact comparison summary (throughput-only — nothing to compare in
	// coverage-only mode)
	if (!COVERAGE_ONLY) {
		console.log(
			generate_comparison_summary(
				all_group_results,
				LANGUAGES,
				iterated_file_count,
				task_tracking_by_group
			)
		);
	}

	console.log('\n' + '='.repeat(80));
}

// Surface suppressed stderr noise counts so silenced upstream bugs don't
// just vanish. Counts are accurate even when individual messages aren't.
if (suppressed_noise.size > 0) {
	log('');
	log('Suppressed stderr noise from upstream impls:');
	for (const [pattern, count] of suppressed_noise) {
		log(`  ${count}× ${pattern}`);
	}
}

// Always save the timestamped pair; only overwrite the canonical
// `report.<runtime>.{json,md}` on full-corpus runs or when --save-report is set.
const write_report = args.save_report || !IS_LIMITED;
const results_path = await save_results(results_data, all_group_results, write_report);
log(`\nResults saved to:`);
log(`  ${results_path}.json`);
log(`  ${results_path}.md`);
if (write_report) {
	log(`Canonical report updated:`);
	log(`  ${RESULTS_DIR}/report.${REPORT_TAG}.json`);
	log(`  ${RESULTS_DIR}/report.${REPORT_TAG}.md`);
	// A limited corpus withholds this file (above); a machine missing an impl does
	// NOT — and shouldn't, since `unavailable` is non-empty by design on a runtime
	// that can't load an impl. But the two are the
	// same KIND of diminished measurement, and this one leaves no trace in the
	// table it thins: the row is simply gone, and the ⚠ init lines are far up the
	// scroll. So name the shortfall at the moment the file is published.
	// Named by ROW, since the rows are what the published tables are missing — an
	// impl whose absence costs this surface no row (`rows: []`) is a shortfall of
	// the machine, not of the file, and says so.
	if (results_data.unavailable.length > 0) {
		const cost = results_data.unavailable
			.map(
				(u) => `${u.impl} (${u.rows.length > 0 ? u.rows.join(', ') : 'no rows on this surface'})`
			)
			.join('; ');
		log(
			`  ⚠ published without ${results_data.unavailable.length} impl(s) that failed to load: ` +
				`${cost} (recorded in \`unavailable\`)`
		);
	}
	if (results_data.binary_sizes_absent.length > 0) {
		log(
			`  ⚠ size table missing ${results_data.binary_sizes_absent.length} artifact(s): ` +
				`${results_data.binary_sizes_absent.join(', ')} (recorded in \`binary_sizes_absent\`)`
		);
	}
	// Named here for the same reason as the two above: this file is about to be
	// committed, and the shortfall it carries leaves no trace in the tables — an
	// unstable row prints exactly like a stable one. See `UNSTABLE_CV_THRESHOLD`.
	const unstable_published = unstable_rows(results_data);
	if (unstable_published.length > 0) {
		log(
			`  ⚠ ${unstable_published.length} unstable row(s) (cv ≥ ${(UNSTABLE_CV_THRESHOLD * 100).toFixed(0)}%, ` +
				`|drift| ≥ ${(UNSTABLE_DRIFT_THRESHOLD * 100).toFixed(0)}%, ` +
				`or pass spread ≥ ${(UNSTABLE_PASS_SPREAD_THRESHOLD * 100).toFixed(0)}%): ` +
				`${unstable_published
					.map(
						(u) =>
							`${u.label} cv ${(u.cv * 100).toFixed(1)}%` +
							(u.drift === null
								? ''
								: ` drift ${u.drift >= 0 ? '+' : ''}${(u.drift * 100).toFixed(1)}%`) +
							(u.pass_spread === null ? '' : ` spread ${(u.pass_spread * 100).toFixed(1)}%`)
					)
					.join(', ')} ` +
				`(per-entry \`cv\` / \`cv_raw\` / \`drift\` / \`pass_spread\`; §Unstable Rows in the md)`
		);
	}
} else {
	log(`Skipped canonical report (limited run — pass --save-report to override)`);
}

// Handle baseline operations. These always have timing: the only coverage-only
// mode is conformance, and conformance runs with baseline flags were rejected
// up front (baseline flags are perf-corpus only).
if (args.save_baseline) {
	await save_baseline(results_data);
}

if (args.compare_baseline) {
	await compare_baseline(results_data);
}
