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
 * - Alternatives: oxc-parser, oxfmt, biome-wasm (for comparison)
 *
 * Run with: deno task bench:deno:run (Deno) or deno task bench:node:run (Node).
 * The same body runs under both — it detects the runtime and writes a
 * runtime-labeled report (report.deno.* / report.node.*). See benches/js/CLAUDE.md.
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
 * Latest results are also written to benches/js/results/report.<runtime>.{json,md} (committed to git).
 *
 * Environment variables:
 *   BENCH_LIMIT         Limit files per language (default: all)
 *   BENCH_FILTER        Filter files by path pattern (default: none)
 *   BENCH_DURATION      Duration per benchmark in ms (default: 5000)
 *   BENCH_WARMUP        Warmup iterations (default: 3)
 *   BENCH_MODE          'intersection' (default) | 'union' — iteration corpus mode
 *   BENCH_GC            Set to 1 to force a major GC between every iteration
 *                       (default: off; see Fairness Caveats for the trade-off)
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
import { Benchmark } from '@fuzdev/fuz_util/benchmark.ts';
import type { BenchmarkResult } from '@fuzdev/fuz_util/benchmark_types.ts';
import {
	benchmark_baseline_compare,
	benchmark_baseline_format,
	benchmark_baseline_save,
} from '@fuzdev/fuz_util/benchmark_baseline.ts';
import { spawn_out } from '@fuzdev/fuz_util/process.ts';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { argv, env, exit } from 'node:process';
import { fileURLToPath } from 'node:url';
import { DevReposLoader, group_by_language } from './lib/corpus.ts';
import {
	canonical_parser_label,
	get_alternative_versions,
	get_benchmark_tasks,
	init_implementations,
} from './lib/implementations.ts';
import {
	type EffectiveCorpusEntry,
	generate_comparison_markdown,
	generate_comparison_summary,
	generate_effective_corpus_report,
	generate_group_bench_table_markdown,
	generate_group_coverage_markdown,
	generate_group_files_markdown,
	generate_group_throughput_markdown,
	generate_json_overhead_note,
	generate_skipped_files_markdown,
	generate_skipped_files_report,
	generate_summary_report,
	generate_versions_info,
	type GroupResults,
} from './lib/report.ts';
import {
	type BinarySize,
	collect_binary_sizes,
	generate_binary_size_markdown,
	generate_binary_size_report,
} from './lib/binary_sizes.ts';
import { type Language, LANGUAGES, type SourceFile } from './lib/types.ts';
import { check_artifact_freshness, wasm_artifact_path } from './lib/check_artifact_freshness.ts';
import { get_library_path } from './lib/ffi.ts';
import { get_napi_library_path } from './lib/napi.ts';
import { current_runtime, type Runtime } from './lib/runtime.ts';

/** The JS runtime executing this bench — labels the report siblings
 * (`report.deno.*` / `report.node.*`) and every row's `runtime` field, and
 * selects the runtime-specific native (FFI vs N-API) + WASM (deno vs nodejs
 * target) artifacts below. The same bench body runs under both. */
const RUNTIME = current_runtime();

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
	verbose: z.boolean().default(false),
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
		parsed.error.issues.map((i: { message: string }) => i.message).join(', '),
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
	verbose: parsed.data.verbose,
};

// In JSON/markdown mode, progress goes to stderr so stdout is clean structured output
const structured_output = args.json || args.markdown;

function log(...messages: unknown[]): void {
	if (structured_output) {
		console.error(...messages);
	} else {
		console.log(...messages);
	}
}

//
// stderr noise suppression
//
// Several third-party impls write to stderr directly during failure paths,
// bypassing our per-file try/catch:
//
// - `prettier-plugin-svelte`/`prettier-plugin-oxfmt` log via `console.error`
//   inside their babel-parser-fallback chain before re-throwing. The
//   exception is caught and recorded as a skip; the console.error has
//   already flushed.
// - `biome` (WASM) uses `console_error_panic_hook` to write Rust panic
//   text to stderr when an internal AST cast fails. Same shape: panic
//   surfaces through wasm-bindgen as a thrown JS error we catch, but
//   the panic hook has already written.
//
// Skips are already disclosed in the Skipped Files report. The console
// output is pure noise. Filter by substring match against the wrapped
// `console.error`. Patterns are intentionally narrow so unrelated
// errors still surface.
const NOISE_PATTERNS = [
	// oxfmt 0.50 wraps the call site in backticks (`oxfmt::textToDoc()`),
	// so match the unwrapped function name to survive minor wording shifts.
	'oxfmt::textToDoc',
	'panicked at crates/biome_rowan',
];
const original_console_error = console.error.bind(console);
const suppressed_noise = new Map<string, number>();
console.error = (...args: unknown[]): void => {
	const probe = args
		.map((a) => (a instanceof Error ? a.message : typeof a === 'string' ? a : ''))
		.join(' ');
	for (const pattern of NOISE_PATTERNS) {
		if (probe.includes(pattern)) {
			suppressed_noise.set(pattern, (suppressed_noise.get(pattern) ?? 0) + 1);
			return;
		}
	}
	original_console_error(...args);
};

//
// Configuration
//

/** Parse optional non-negative integer from env var; malformed values fall back to undefined. */
const env_int = (name: string): number | undefined => {
	const val = env[name];
	if (!val) return undefined;
	const n = parseInt(val, 10);
	return Number.isFinite(n) && n >= 0 ? n : undefined;
};

/** Limit files per language (default: all) */
const MAX_FILES_PER_LANGUAGE = env_int('BENCH_LIMIT');

/** Filter files by path pattern (default: none) */
const FILE_FILTER = env.BENCH_FILTER;

/** Duration per benchmark in ms (default: 5000) */
const BENCH_DURATION = env_int('BENCH_DURATION') ?? 5000;

/** Number of warmup iterations (default: 3) */
const BENCH_WARMUP = env_int('BENCH_WARMUP') ?? 3;

/**
 * Enable the per-iteration forced-GC hook (default: off — measures realistic
 * throughput where GC happens opportunistically, matching real-world usage).
 * Set `BENCH_GC=1` to force a major GC between every iteration; useful for
 * stabilizing high-allocation workloads at the cost of penalizing efficient
 * low-allocation paths. See `CLAUDE.md` → Fairness Caveats for the trade-off.
 */
const BENCH_GC = env.BENCH_GC === '1';

/**
 * Include the `tsv-forced-async` control row (default off). Same native engine
 * as `tsv`, routed through the awaited async path, to re-confirm that the
 * per-file await tax the async-only impls (`prettier`, `oxfmt`) pay is below the
 * noise floor. Kept opt-in so the noise-level row stays out of the published
 * report and the regression baseline; set `BENCH_FORCED_ASYNC=1` to enable.
 * See `BenchmarkTaskOptions.forced_async`.
 */
const BENCH_FORCED_ASYNC = env.BENCH_FORCED_ASYNC === '1';

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
const USE_INTERSECTION = BENCH_MODE !== 'union';

/** Maximum length of error message to display (longer messages are truncated) */
const MAX_ERROR_MESSAGE_LENGTH = 200;

/**
 * Baseline storage directory. Passed to `benchmark_baseline_save` /
 * `_compare`; the library calls `mkdir(path, { recursive: true })` and
 * writes `baseline.json` inside, so the file lands at
 * `./benches/js/results/baseline.json`. Moved into `results/` (from its
 * pre-0.60 location at `./benches/js/baseline.json`) so the library's
 * mkdir is covered by the existing `--allow-write=benches/js/results`
 * permission without widening write scope to the whole benches tree.
 */
const BASELINE_DIR = './benches/js/results';

/** Results directory for comparison JSON files */
const RESULTS_DIR = './benches/js/results';

//
// Setup
//

log('Loading corpus...\n');
const corpus_loader = new DevReposLoader();
// Drain `stream()` directly instead of `load()` so we skip the loader's
// own corpus summary — bench.ts prints its own tighter one below that
// includes byte counts and (when applicable) limit annotations.
const files: SourceFile[] = [];
for await (const file of corpus_loader.stream(log)) {
	files.push(file);
}
const by_language = group_by_language(files);

// Preserve total counts before limiting
const total_file_counts = {
	svelte: by_language.svelte.length,
	typescript: by_language.typescript.length,
	css: by_language.css.length,
};

// Apply file filter and limit
function limit_files(files: SourceFile[]): SourceFile[] {
	const filtered = FILE_FILTER ? files.filter((f) => f.path.includes(FILE_FILTER)) : files;
	return MAX_FILES_PER_LANGUAGE ? filtered.slice(0, MAX_FILES_PER_LANGUAGE) : filtered;
}

const svelte_files = limit_files(by_language.svelte);
const ts_files = limit_files(by_language.typescript);
const css_files = limit_files(by_language.css);

// Track if corpus is limited
const is_limited = MAX_FILES_PER_LANGUAGE !== undefined || FILE_FILTER !== undefined;

// Calculate total bytes per language for throughput metrics
const bytes_by_language: Record<Language, number> = {
	svelte: svelte_files.reduce((sum, f) => sum + f.bytes, 0),
	typescript: ts_files.reduce((sum, f) => sum + f.bytes, 0),
	css: css_files.reduce((sum, f) => sum + f.bytes, 0),
};

/**
 * Format bytes/sec as MB/s. Always MB/s, even for sub-1-MB values
 * (renders as e.g. `0.4 MB/s`) so a column of throughput numbers scans
 * uniformly without unit-switching mid-table.
 */
function format_throughput(bytes_per_sec: number): string {
	return `${(bytes_per_sec / 1_000_000).toFixed(1)} MB/s`;
}

// Compact corpus summary: file counts + MB per language + total. When
// limited, each line reads `N of M files` so the subset is obvious.
const total_files = svelte_files.length + ts_files.length + css_files.length;
const total_bytes = bytes_by_language.svelte + bytes_by_language.typescript + bytes_by_language.css;
const fmt_count = (
	n: number,
	total: number,
) => (is_limited && n !== total ? `${n} of ${total}` : `${n}`);
const fmt_bytes = (b: number) => `${(b / 1_000_000).toFixed(1)} MB`;
log(`Corpus:`);
log(
	`  Svelte:      ${fmt_count(svelte_files.length, total_file_counts.svelte).padEnd(11)} files (${
		fmt_bytes(bytes_by_language.svelte)
	})`,
);
log(
	`  TypeScript:  ${fmt_count(ts_files.length, total_file_counts.typescript).padEnd(11)} files (${
		fmt_bytes(bytes_by_language.typescript)
	})`,
);
log(
	`  CSS:         ${fmt_count(css_files.length, total_file_counts.css).padEnd(11)} files (${
		fmt_bytes(bytes_by_language.css)
	})`,
);
log(`  Total:       ${String(total_files).padEnd(11)} files (${fmt_bytes(total_bytes)})`);
log();

// Refuse to measure stale binaries (the `:run` tasks skip the rebuild). See
// lib/check_artifact_freshness.ts; override with BENCH_STALE_OK=1. The native +
// WASM artifacts are runtime-specific: Deno executes the FFI library + the
// `deno`-target WASM bundle; Node/Bun execute the N-API addon + the `nodejs`
// target. `wasm_artifact_path` resolves the runtime's bundle itself.
const native_check = RUNTIME === 'deno'
	? {
		label: `FFI (${env.TSV_FFI_PROFILE ?? 'release'})`,
		path: get_library_path(),
		binding_crates: ['tsv_ffi'],
		rebuild: 'deno task build:ffi',
	}
	: {
		label: 'N-API',
		path: get_napi_library_path(),
		binding_crates: ['tsv_napi'],
		rebuild: 'deno task build:napi',
	};
const wasm_target = RUNTIME === 'deno' ? 'deno' : 'nodejs';
await check_artifact_freshness([
	native_check,
	{
		label: `WASM (all/${wasm_target})`,
		path: wasm_artifact_path('all'),
		binding_crates: ['tsv_wasm'],
		rebuild: `deno task build:wasm:all:${wasm_target}`,
	},
]);

// Friendly preflight: the canonical impls (prettier + svelte/compiler) resolve
// from the harness `node_modules`; without it, init fails with an opaque
// module-resolution error. Point the user at the one installer instead. (Cheap
// existence check — `bench:install` populates the tree both runtimes consume.)
const node_modules_path = fileURLToPath(new URL('./node_modules', import.meta.url));
try {
	await stat(node_modules_path);
} catch {
	console.error(
		`\n✗ Harness dependencies not installed (${node_modules_path} missing).\n` +
			`  Run 'deno task bench:install' first — it installs the npm deps + the oxc wasi binding.\n`,
	);
	exit(1);
}

// Initialize implementations
const impls = await init_implementations({ logger: log });

//
// Benchmark Helpers
//

//
// Per-impl tracking maps (keyed by tracking_key, e.g. `parse/svelte/native`).
//
// Populated by the **untimed pre-flight pass** before each group's timed
// bench run. The pre-flight records each impl's success/skip set; the timed
// loop then iterates either the per-group all-N intersection (default) or
// each impl's preflight success set (`BENCH_MODE=union`).
//
// `successful_files` and `skipped_files` always reflect preflight results,
// independent of the iteration mode — they are the source of truth for
// coverage disclosure. `effective_corpus_bytes` and `iterated_file_count` are
// updated to reflect what was actually timed (intersection or per-impl).
//

/** Files an impl successfully processed during pre-flight, keyed by tracking_key. */
const successful_files: Map<string, Set<string>> = new Map();
/** Files an impl failed on during pre-flight, with the error message. */
const skipped_files: Map<string, Map<string, string>> = new Map();
/** Effective corpus size per benchmark (processed / total files). */
const effective_corpus_size: Map<string, { processed: number; total: number }> = new Map();
/** Effective corpus bytes per benchmark — used for honest throughput math. */
const effective_corpus_bytes: Map<string, number> = new Map();
/**
 * Files actually iterated by the timed loop per task. Distinct from
 * `effective_corpus_size` (which records preflight success — disclosure-only
 * coverage info): in `intersection` mode this is the per-group all-N
 * intersection (uniform across tasks in a group); in `union` mode it's the
 * task's preflight success set. Used by the bench-table `Nx (Mf)` annotation
 * and the Comparisons table's pairwise file counts.
 */
const iterated_file_count: Map<string, number> = new Map();
/**
 * Wall-clock ms for one preflight pass per task (iterating every file once).
 * Used to tier per-task `min_iterations` so slow tasks (multi-second per pass)
 * get a higher sample-size floor for trustworthy percentile/CI math, while
 * fast tasks rely on `duration_ms` to drive sample count.
 */
const preflight_elapsed_ms: Map<string, number> = new Map();
/**
 * Map result.name → tracking_key per group, so the markdown report can look up
 * coverage/throughput by display name (the bench library doesn't surface tracking_key).
 */
const task_tracking_by_group: Map<string, Map<string, string>> = new Map();

function record_skip(bench_name: string, file_path: string, error: unknown): void {
	if (!skipped_files.has(bench_name)) {
		skipped_files.set(bench_name, new Map());
	}
	const bench_map = skipped_files.get(bench_name)!;
	if (bench_map.has(file_path)) return;
	const error_msg = error instanceof Error ? error.message : String(error);
	bench_map.set(file_path, error_msg);
}

/**
 * Iterate files and run `process_fn` for each. The iteration list is
 * pre-filtered to files this task succeeded on during pre-flight (or the
 * group's all-N intersection in `intersection` mode), so throws are real
 * bugs — let them propagate to surface as benchmark errors rather than
 * silently catalog.
 */
function process_corpus(files: SourceFile[], process_fn: (file: SourceFile) => void): void {
	for (const file of files) {
		process_fn(file);
	}
}

/** Async variant of `process_corpus`. */
async function process_corpus_async(
	files: SourceFile[],
	process_fn: (file: SourceFile) => Promise<void>,
): Promise<void> {
	for (const file of files) {
		await process_fn(file);
	}
}

/** Files by language lookup */
const files_by_language: Record<Language, SourceFile[]> = {
	svelte: svelte_files,
	typescript: ts_files,
	css: css_files,
};

/**
 * Run every task once per file untimed to discover each impl's effective
 * corpus. Populates `successful_files`, `skipped_files`, and
 * `effective_corpus_size` so the caller can compute the per-group iteration
 * set (intersection or per-impl) and the report can disclose coverage.
 *
 * Cost: O(impls × files), each call is one parse/format. Small relative
 * to the timed loop (which iterates the same files for 5s+ per task).
 */
async function run_preflight(
	tasks: ReturnType<typeof get_benchmark_tasks>,
	files: SourceFile[],
	language: Language,
): Promise<void> {
	for (let i = 0; i < tasks.length; i++) {
		const task = tasks[i];
		const success = new Set<string>();
		let bytes = 0;
		const start_ms = performance.now();
		for (const file of files) {
			try {
				if (task.is_async) {
					await task.run_async!(file.content, language);
				} else {
					task.run(file.content, language);
				}
				success.add(file.path);
				bytes += file.bytes;
			} catch (e) {
				record_skip(task.tracking_key, file.path, e);
			}
		}
		const elapsed_ms = performance.now() - start_ms;
		successful_files.set(task.tracking_key, success);
		effective_corpus_size.set(task.tracking_key, { processed: success.size, total: files.length });
		effective_corpus_bytes.set(task.tracking_key, bytes);
		preflight_elapsed_ms.set(task.tracking_key, elapsed_ms);
		log(`  [${i + 1}/${tasks.length}] ${task.name}: ${success.size}/${files.length} files`);
	}
}

//
// Run Benchmarks
//

const all_group_results: GroupResults[] = [];

/**
 * Per-group setup captured during the up-front pre-flight pass. Reused by
 * `run_benchmark_group` so the timed loop is purely measurement.
 */
interface GroupSetup {
	tasks: ReturnType<typeof get_benchmark_tasks>;
	filtered_files_by_task: Map<string, SourceFile[]>;
}
const group_setups: Map<string, GroupSetup> = new Map();

/**
 * Run pre-flight + iteration-set computation for one group. Populates
 * `successful_files`, `skipped_files`, `effective_corpus_size`,
 * `effective_corpus_bytes`, `iterated_file_count`, and `task_tracking_by_group`,
 * and stashes the per-group setup in `group_setups` for the timed pass.
 *
 * Doing this for every group up front (before any timed run) means the
 * coverage picture lands in the terminal/report before any 5s+ timed
 * benchmark starts — easier to spot a broken impl early.
 */
async function run_preflight_group(
	operation: 'parse' | 'format',
	language: Language,
): Promise<void> {
	const files = files_by_language[language];
	if (files.length === 0) return;

	const group_name = `${operation}/${language}`;
	log(`\n· ${group_name}`);

	const tasks = get_benchmark_tasks(impls, operation, language, {
		forced_async: BENCH_FORCED_ASYNC,
	});
	await run_preflight(tasks, files, language);

	const task_tracking = new Map<string, string>();
	for (const task of tasks) {
		task_tracking.set(task.name, task.tracking_key);
	}
	task_tracking_by_group.set(group_name, task_tracking);

	// Build each task's iteration file list. In `intersection` mode (default)
	// every task in the group iterates the same all-N intersection, making
	// timing ratios within the group apples-to-apples. In `union` mode each
	// task iterates its own preflight success set — ratios then reflect
	// different file sets per impl, useful for auditing what intersection
	// mode hides.
	const filtered_files_by_task = new Map<string, SourceFile[]>();
	if (USE_INTERSECTION) {
		let intersection: Set<string> | null = null;
		for (const task of tasks) {
			const success_set = successful_files.get(task.tracking_key) ?? new Set<string>();
			if (intersection === null) {
				intersection = new Set(success_set);
			} else {
				for (const path of intersection) {
					if (!success_set.has(path)) intersection.delete(path);
				}
			}
		}
		const intersection_list = files.filter((f) => (intersection ?? new Set<string>()).has(f.path));
		for (const task of tasks) {
			filtered_files_by_task.set(task.tracking_key, intersection_list);
		}
		log(`  Intersection: ${intersection_list.length}/${files.length} files`);
	} else {
		for (const task of tasks) {
			const success_set = successful_files.get(task.tracking_key) ?? new Set<string>();
			filtered_files_by_task.set(
				task.tracking_key,
				files.filter((f) => success_set.has(f.path)),
			);
		}
	}

	// Overwrite preflight-derived byte counts with iteration byte counts so
	// throughput math (`ops_per_sec × effective_corpus_bytes`) reflects what was
	// actually measured. Also record per-task iteration size for the
	// `Nx (Mf)` annotation in the bench-table `vs baseline` column.
	for (const task of tasks) {
		const task_files = filtered_files_by_task.get(task.tracking_key)!;
		effective_corpus_bytes.set(task.tracking_key, task_files.reduce((sum, f) => sum + f.bytes, 0));
		iterated_file_count.set(task.tracking_key, task_files.length);
	}

	group_setups.set(group_name, { tasks, filtered_files_by_task });
}

/** Run the timed measurement loop for one group using its stashed pre-flight setup. */
async function run_benchmark_group(
	operation: 'parse' | 'format',
	language: Language,
): Promise<void> {
	const group_name = `${operation}/${language}`;
	const setup = group_setups.get(group_name);
	if (!setup) return;
	const { tasks, filtered_files_by_task } = setup;
	const task_tracking = task_tracking_by_group.get(group_name) ?? new Map<string, string>();

	log(`\n▶ ${group_name}`);

	const bench = new Benchmark({
		duration_ms: BENCH_DURATION,
		warmup_iterations: BENCH_WARMUP,
		// Suite floor — overridden per-task below for slow paths. 5 keeps fast
		// tasks duration-bound (they hit BENCH_DURATION long before any floor)
		// while ensuring even the very slow ones don't fall to a degenerate
		// n=3 where p99 collapses to `max` and Welch's t-test has unstable DOF.
		min_iterations: 5,
		// oxfmt's async napi binding leaks state into Deno's timer wheel:
		// after the first oxfmt.format call, exactly one further setTimeout
		// fires and then all subsequent timers stall forever. The default
		// 100ms inter-task cooldown is the only timer-dependent await in
		// the loop, so dropping it sidesteps the hang.
		// See benches/js/CLAUDE.md → Known Issues.
		cooldown_ms: 0,
		on_iteration: BENCH_GC ? () => globalThis.gc?.() : undefined,
		on_task_complete: (result: BenchmarkResult, index: number, total: number) => {
			const ops_per_sec = result.stats.ops_per_second.toFixed(1);
			// Throughput uses effective bytes (this impl's success set) so
			// the displayed MB/s is what this impl actually achieved, not
			// what it would have done on the full corpus.
			const tracking_key = task_tracking.get(result.name);
			const effective_bytes = tracking_key ? effective_corpus_bytes.get(tracking_key) ?? 0 : 0;
			// Mirror the report-path guard (`generate_group_throughput_markdown`):
			// with an empty intersection the MB/s figure is a misleading `0.0 MB/s`
			// while ops/sec is real, so print `—` instead of a fake throughput.
			const throughput = effective_bytes === 0
				? '—'
				: format_throughput(result.stats.ops_per_second * effective_bytes);
			log(`  [${index + 1}/${total}] ${result.name}: ${ops_per_sec} ops/sec (${throughput})`);
		},
	});

	for (const task of tasks) {
		const task_files = filtered_files_by_task.get(task.tracking_key)!;
		// Tier per-task `min_iterations` based on preflight pass time. The
		// suite floor (5) handles most cases; very slow tasks (>5s/pass —
		// prettier on the full TS corpus, oxfmt full passes) get a bump to 7
		// because at n=5 their p75/p90 still sit too close to max and the
		// Welch DOF is on the edge. Above that we don't keep climbing: each
		// extra iteration on a 14s/pass task costs another 14s of wall clock.
		const preflight_ms = preflight_elapsed_ms.get(task.tracking_key) ?? 0;
		const min_iter = preflight_ms > 5000 ? 7 : undefined;
		const base_task = { name: task.name, min_iterations: min_iter };
		if (task.is_async) {
			bench.add({
				...base_task,
				fn: async () => {
					await process_corpus_async(task_files, async (f) => {
						await task.run_async!(f.content, language);
					});
				},
				async: true,
			});
		} else {
			bench.add({
				...base_task,
				fn: () => {
					process_corpus(task_files, (f) => task.run(f.content, language));
				},
				async: false,
			});
		}
	}

	const results = await bench.run();
	all_group_results.push({ name: group_name, results });
}

// Two-phase run: pre-flight every group up front (so the coverage picture
// lands before any 5s+ timed run starts), then time every group.
log('Pre-flight (discover coverage + exclude failing files before timing):');
for (const lang of LANGUAGES) {
	await run_preflight_group('parse', lang);
	await run_preflight_group('format', lang);
}

log('\nRunning benchmarks:');
for (const lang of LANGUAGES) {
	await run_benchmark_group('parse', lang);
	await run_benchmark_group('format', lang);
}

//
// Baseline Handling
//

interface BaselineEntry {
	name: string;
	group: string;
	mean_ns: number;
	p50_ns: number;
	p75_ns: number;
	p90_ns: number;
	p95_ns: number;
	p99_ns: number;
	min_ns: number;
	max_ns: number;
	std_dev_ns: number;
	cv: number;
	ops_per_second: number;
	sample_size: number;
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
	 * The JS runtime that produced this row (`deno` | `node` | `bun`). Every row
	 * carries it so a reader never has to guess what produced a number — the
	 * runtime-labeled sibling reports (`report.deno.*` / `report.node.*`) compose
	 * at the display layer (tsv.fuz.dev), and a per-runtime delta on the same row
	 * is the detector for a runtime-specific measurement artifact.
	 */
	runtime: Runtime;
}

/** Package versions used in the benchmark run */
interface BaselineVersions {
	/** tsv's own version, from `Cargo.toml` `[workspace.package]` (the binary under test). */
	tsv: string;
	svelte: string;
	acorn: string;
	acorn_ts: string;
	prettier: string;
	prettier_svelte: string;
	oxc_parser?: string;
	oxfmt?: string;
	biome?: string;
}

interface Baseline {
	version: number;
	/** The JS runtime that produced this report (`deno` | `node` | `bun`). Mirrors
	 * the per-row `runtime` and matches the `report.<runtime>.{json,md}` filename. */
	runtime: Runtime;
	timestamp: string;
	git_commit: string | null;
	corpus: {
		svelte: number;
		typescript: number;
		css: number;
	};
	versions: BaselineVersions;
	binary_sizes: BinarySize[];
	entries: BaselineEntry[];
	/**
	 * Counts of stderr noise from third-party impls that the harness silenced
	 * during the run, keyed by message pattern (e.g. `oxfmt::textToDoc`). Surfaced
	 * machine-readably so silenced upstream crashes don't vanish; not rendered in
	 * the markdown report (counts are run-variant and would churn the committed
	 * report). Empty `{}` when nothing was suppressed.
	 */
	suppressed_noise: Record<string, number>;
}

/**
 * Read tsv's own version from the workspace `Cargo.toml` (`[workspace.package]`),
 * the single source of truth every crate inherits via `version.workspace = true`
 * and that the published npm packages move together at. Returns `'unknown'` if it
 * can't be read or parsed.
 */
async function get_tsv_version(): Promise<string> {
	try {
		const cargo_toml_path = fileURLToPath(new URL('../../Cargo.toml', import.meta.url));
		const content = await readFile(cargo_toml_path, 'utf8');
		// Match the line-leading `version = "..."` inside the `[workspace.package]` section.
		// `^version` (multiline) avoids matching a `rust-version = "..."` MSRV pin; `[^[]*?`
		// bounds the search to the section by stopping at the next `[` heading.
		const match = content.match(/\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/m);
		if (match) return match[1];
	} catch {
		// Ignore
	}
	return 'unknown';
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

/** Build results data from current benchmark run */
async function build_results_data(
	groups: GroupResults[],
	corpus: { svelte: number; typescript: number; css: number },
	versions: BaselineVersions,
	binary_sizes: BinarySize[],
): Promise<Baseline> {
	const entries: BaselineEntry[] = [];
	for (const group of groups) {
		// Resolve per-impl preflight coverage (the markdown `Coverage:` line) via
		// the same display-name → tracking_key map the report uses.
		const tracking = task_tracking_by_group.get(group.name);
		for (const result of group.results) {
			const tracking_key = tracking?.get(result.name);
			const coverage = tracking_key ? effective_corpus_size.get(tracking_key) : undefined;
			const iterated = tracking_key ? iterated_file_count.get(tracking_key) : undefined;
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
				files_processed: coverage?.processed ?? null,
				files_total: coverage?.total ?? null,
				files_iterated: iterated ?? null,
				runtime: RUNTIME,
			});
		}
	}

	return {
		// Bumped 4 → 5 for the added `runtime` field (top-level + per row).
		version: 5,
		runtime: RUNTIME,
		timestamp: new Date().toISOString(),
		git_commit: await get_git_commit(),
		corpus,
		versions,
		binary_sizes: binary_sizes,
		entries,
		suppressed_noise: Object.fromEntries(suppressed_noise),
	};
}

/** Format bytes as MB with one decimal */
function format_mb(bytes: number): string {
	return `${(bytes / 1_000_000).toFixed(1)} MB`;
}

/** Generate a full markdown report from benchmark data */
function generate_markdown_report(
	groups: GroupResults[],
	binary_sizes: BinarySize[],
	corpus: { svelte: number; typescript: number; css: number },
	corpus_bytes: Record<Language, number>,
	versions: BaselineVersions,
	timestamp: string,
	git_commit: string | null,
	task_tracking: Map<string, Map<string, string>>,
	effective_size: Map<string, EffectiveCorpusEntry>,
	effective_bytes: Map<string, number>,
	iterated_counts: Map<string, number>,
	skipped: Map<string, Map<string, string>>,
): string {
	const lines: string[] = [];
	lines.push('# tsv benchmark results\n');
	const commit_str = git_commit ? ` (${git_commit})` : '';
	lines.push(`**Runtime:** ${RUNTIME}\n`);
	lines.push(`**Date:** ${timestamp} — tsv ${versions.tsv}${commit_str}\n`);

	const total_files = corpus.svelte + corpus.typescript + corpus.css;
	const total_bytes = corpus_bytes.svelte + corpus_bytes.typescript + corpus_bytes.css;
	lines.push(
		`**Corpus:** ${corpus.svelte} Svelte (${format_mb(corpus_bytes.svelte)}), ` +
			`${corpus.typescript} TypeScript (${format_mb(corpus_bytes.typescript)}), ` +
			`${corpus.css} CSS (${format_mb(corpus_bytes.css)}) — ` +
			`${total_files} files, ${format_mb(total_bytes)} total\n`,
	);

	// Versions
	const version_parts = [
		`svelte@${versions.svelte}`,
		`acorn@${versions.acorn}`,
		`acorn-typescript@${versions.acorn_ts}`,
		`prettier@${versions.prettier}`,
		`prettier-plugin-svelte@${versions.prettier_svelte}`,
	];
	if (versions.oxc_parser) version_parts.push(`oxc-parser@${versions.oxc_parser}`);
	if (versions.oxfmt) version_parts.push(`oxfmt@${versions.oxfmt}`);
	if (versions.biome) version_parts.push(`@biomejs/wasm-bundler@${versions.biome}`);
	lines.push(`**Versions:** ${version_parts.join(', ')}\n`);

	lines.push(
		'**Methodology:** Single-threaded — every implementation formats/parses one file at a time, ' +
			'measured sequentially with no cross-file parallelism. The numbers are per-file, single-core ' +
			'latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.\n',
	);

	for (const group of groups) {
		if (group.results.length === 0) continue;
		const [operation, language] = group.name.split('/') as ['parse' | 'format', Language];
		// Use the canonical reference as the bench-table baseline. Without this,
		// the library picks the fastest task (often `tsv-internal`, a non-public
		// optimization variant) which is not the comparison readers want.
		const baseline = operation === 'format' ? 'prettier' : canonical_parser_label(language);
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
			generate_group_bench_table_markdown(group.results, baseline_exists ? baseline : undefined),
		);
		lines.push('');

		const files = generate_group_files_markdown(group_iterated_counts);
		if (files) lines.push(files, '');

		const throughput = generate_group_throughput_markdown(group.results, tracking, effective_bytes);
		if (throughput) lines.push(throughput, '');

		const coverage = generate_group_coverage_markdown(group.results, tracking, effective_size);
		if (coverage) lines.push(coverage, '');

		if (operation === 'parse') {
			const json_note = generate_json_overhead_note(group.results);
			if (json_note) lines.push(json_note, '');
		}
	}

	// Convention note: every `Nx` in this report is speedup form — values > 1
	// mean self is faster than the opponent. File counts are surfaced per
	// group (Files / Coverage lines) and per row in the Comparisons tables.
	lines.push(
		'_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._\n',
	);

	const binary_size_markdown = generate_binary_size_markdown(binary_sizes);
	if (binary_size_markdown) {
		lines.push(binary_size_markdown);
		lines.push('');
	}

	const comparison_markdown = generate_comparison_markdown(
		groups,
		LANGUAGES,
		iterated_counts,
		task_tracking,
	);
	if (comparison_markdown) {
		lines.push(comparison_markdown);
		lines.push('');
	}

	const skipped_markdown = generate_skipped_files_markdown(
		skipped,
		MAX_ERROR_MESSAGE_LENGTH,
		args.verbose,
		task_tracking,
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
 * `report.<runtime>.{json,md}` when `write_report` is true — gated by the caller
 * so that partial runs (BENCH_LIMIT, BENCH_FILTER) don't clobber the committed
 * canonical report. Every filename is runtime-suffixed so a Deno run and a Node
 * run write sibling files (`report.deno.*` / `report.node.*`) instead of one
 * clobbering the other.
 */
async function save_results(
	data: Baseline,
	groups: GroupResults[],
	binary_sizes: BinarySize[],
	write_report: boolean,
): Promise<string> {
	await mkdir(RESULTS_DIR, { recursive: true });
	const timestamp = data.timestamp.replace(/[:.]/g, '-').slice(0, 19);
	const commit = data.git_commit ?? 'unknown';
	const base_path = `${RESULTS_DIR}/${timestamp}_${commit}.${RUNTIME}`;

	const markdown = generate_markdown_report(
		groups,
		binary_sizes,
		data.corpus,
		bytes_by_language,
		data.versions,
		data.timestamp,
		data.git_commit,
		task_tracking_by_group,
		effective_corpus_size,
		effective_corpus_bytes,
		iterated_file_count,
		skipped_files,
	);

	const json = JSON.stringify(data, null, '\t');
	const writes: Promise<void>[] = [
		writeFile(`${base_path}.json`, json),
		writeFile(`${base_path}.md`, markdown),
	];
	if (write_report) {
		writes.push(
			writeFile(`${RESULTS_DIR}/report.${RUNTIME}.json`, json),
			writeFile(`${RESULTS_DIR}/report.${RUNTIME}.md`, markdown),
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
 * context the old custom baseline used to carry).
 */
function build_baseline_metadata(data: Baseline): Record<string, unknown> {
	return {
		corpus: data.corpus,
		versions: data.versions,
		binary_sizes: data.binary_sizes,
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
		metadata: build_baseline_metadata(data),
	});
	log(`Baseline saved to ${BASELINE_DIR}/baseline.json`);
}

/**
 * Compare current results against the stored baseline. Uses Welch's t-test
 * (via `benchmark_baseline_compare`) for significance, methodology-change
 * detection for per-task budget drift, and OR-gated noise warnings on
 * high-cv or high-outlier-ratio rows. The flat ±5% ops/sec gate that lived
 * here previously is gone — see `benchmark_baseline_compare` and the
 * fairness caveats in benches/js/CLAUDE.md.
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
			staleness_warning_days: 7,
		},
	);

	if (!comparison.baseline_found) {
		console.error(
			`\nNo baseline found at ${BASELINE_DIR}/baseline.json. Run with --save-baseline first.`,
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
	const corpus_match = baseline_corpus &&
		baseline_corpus.svelte === current.corpus.svelte &&
		baseline_corpus.typescript === current.corpus.typescript &&
		baseline_corpus.css === current.corpus.css;
	if (baseline_corpus && !corpus_match) {
		log(`\n⚠️  Corpus size differs from baseline:`);
		log(
			`   Baseline: svelte=${baseline_corpus.svelte}, ts=${baseline_corpus.typescript}, css=${baseline_corpus.css}`,
		);
		log(
			`   Current:  svelte=${current.corpus.svelte}, ts=${current.corpus.typescript}, css=${current.corpus.css}`,
		);
	}

	log('');
	log(benchmark_baseline_format(comparison));
}

//
// Output
//

// Collect binary sizes once (used by all output paths). Versions no longer
// thread through — bindings live in node_modules (flat, no version dir). The
// tsv napi `.node` row is existence-gated (omitted when unbuilt), so no flag.
const binary_sizes = await collect_binary_sizes({
	has_native: !!impls.native,
	has_wasm: !!impls.wasm,
	has_oxc: !!impls.oxc,
	has_biome: !!impls.biome,
});

// Build results data (used by all output paths and always saved)
const corpus = {
	svelte: svelte_files.length,
	typescript: ts_files.length,
	css: css_files.length,
};
const alt_versions = get_alternative_versions(impls);
const v = impls.versions.canonical;
const versions: BaselineVersions = {
	tsv: await get_tsv_version(),
	svelte: v.svelte,
	acorn: v.acorn,
	acorn_ts: v['@sveltejs/acorn-typescript'],
	prettier: v.prettier,
	prettier_svelte: v['prettier-plugin-svelte'],
	...alt_versions,
};
const results_data = await build_results_data(all_group_results, corpus, versions, binary_sizes);

if (args.json) {
	// JSON output (same structure as saved results)
	console.log(JSON.stringify(results_data, null, '\t'));
} else if (args.markdown) {
	console.log(
		generate_markdown_report(
			all_group_results,
			binary_sizes,
			corpus,
			bytes_by_language,
			versions,
			results_data.timestamp,
			results_data.git_commit,
			task_tracking_by_group,
			effective_corpus_size,
			effective_corpus_bytes,
			iterated_file_count,
			skipped_files,
		),
	);
} else {
	// Standard text output
	console.log(generate_summary_report(all_group_results, LANGUAGES));

	console.log(generate_versions_info(versions));

	const effective_corpus_report = generate_effective_corpus_report(
		effective_corpus_size,
		task_tracking_by_group,
	);
	if (effective_corpus_report) {
		console.log(effective_corpus_report);
	}

	const skipped_report = generate_skipped_files_report(
		skipped_files,
		MAX_ERROR_MESSAGE_LENGTH,
		args.verbose,
		task_tracking_by_group,
	);
	if (skipped_report) {
		console.log(skipped_report);
	}

	const binary_size_report = generate_binary_size_report(binary_sizes);
	if (binary_size_report) {
		console.log(binary_size_report);
	}

	// Compact comparison summary
	console.log(
		generate_comparison_summary(
			all_group_results,
			LANGUAGES,
			iterated_file_count,
			task_tracking_by_group,
		),
	);

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
const write_report = args.save_report || !is_limited;
const results_path = await save_results(
	results_data,
	all_group_results,
	binary_sizes,
	write_report,
);
log(`\nResults saved to:`);
log(`  ${results_path}.json`);
log(`  ${results_path}.md`);
if (write_report) {
	log(`Canonical report updated:`);
	log(`  ${RESULTS_DIR}/report.${RUNTIME}.json`);
	log(`  ${RESULTS_DIR}/report.${RUNTIME}.md`);
} else {
	log(`Skipped canonical report (limited run — pass --save-report to override)`);
}

// Handle baseline operations
if (args.save_baseline) {
	await save_baseline(results_data);
}

if (args.compare_baseline) {
	await compare_baseline(results_data);
}
