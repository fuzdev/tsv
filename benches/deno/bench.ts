/**
 * TSV Benchmark Suite
 *
 * Compares parsing and formatting performance across implementations.
 * All benchmarks are single-threaded: files processed sequentially, no parallelism.
 *
 * Implementations:
 * - Canonical: prettier + svelte/compiler (JavaScript baseline)
 * - Native: tsv via FFI (Rust, maximum performance)
 * - WASM: tsv compiled to WASM (portable, near-native)
 * - Alternatives: oxc-parser, oxfmt, biome-wasm (for comparison)
 *
 * Run with: deno task bench:run
 *
 * CLI options:
 *   --json              Output results as JSON
 *   --markdown          Output results as Markdown
 *   --save-baseline     Also save results as baseline for regression detection
 *   --compare-baseline  Compare against saved baseline
 *   --verbose           Include per-file skip detail (paths + errors + failure sets)
 *
 * Results are always saved to benches/deno/results/<timestamp>_<commit>.{json,md}.
 * Latest results are also written to benches/deno/results/report.{json,md} (committed to git).
 *
 * Environment variables:
 *   BENCH_LIMIT         Limit files per language (default: all)
 *   BENCH_FILTER        Filter files by path pattern (default: none)
 *   BENCH_DURATION      Duration per benchmark in ms (default: 5000)
 *   BENCH_WARMUP        Warmup iterations (default: 3)
 */

// Type declaration for V8's gc function (available with --expose-gc)
declare global {
	var gc: (() => void) | undefined;
}

import { z } from 'zod';
import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.js';
import { Benchmark } from '@fuzdev/fuz_util/benchmark.js';
import type { BenchmarkResult } from '@fuzdev/fuz_util/benchmark_types.js';
import {
	benchmark_baseline_compare,
	benchmark_baseline_format,
	benchmark_baseline_save,
} from '@fuzdev/fuz_util/benchmark_baseline.js';
import { DevReposLoader, groupByLanguage } from './lib/corpus.ts';
import {
	canonicalParserLabel,
	getAlternativeVersions,
	getBenchmarkTasks,
	initImplementations,
} from './lib/implementations.ts';
import {
	type EffectiveCorpusEntry,
	generateComparisonMarkdown,
	generateComparisonSummary,
	generateEffectiveCorpusReport,
	generateGroupBenchTableMarkdown,
	generateGroupCoverageMarkdown,
	generateGroupFilesMarkdown,
	generateGroupThroughputMarkdown,
	generateJsonOverheadNote,
	generateSkippedFilesMarkdown,
	generateSkippedFilesReport,
	generateSummaryReport,
	generateVersionsInfo,
	type GroupResults,
} from './lib/report.ts';
import {
	type BinarySize,
	collectBinarySizes,
	generateBinarySizeMarkdown,
	generateBinarySizeReport,
} from './lib/binary_sizes.ts';
import { type Language, LANGUAGES, type SourceFile } from './lib/types.ts';

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

// Strip leading -- from deno task passthrough
const raw_argv = Deno.args[0] === '--' ? Deno.args.slice(1) : Deno.args;
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
	Deno.exit(1);
}

if (parsed.data._.length > 0) {
	console.error(`Unexpected positional arguments: ${parsed.data._.join(', ')}`);
	Deno.exit(1);
}

const args = {
	json: parsed.data.json,
	markdown: parsed.data.markdown,
	saveBaseline: parsed.data['save-baseline'],
	compareBaseline: parsed.data['compare-baseline'],
	saveReport: parsed.data['save-report'],
	verbose: parsed.data.verbose,
};

// In JSON/markdown mode, progress goes to stderr so stdout is clean structured output
const structuredOutput = args.json || args.markdown;

function log(...messages: unknown[]): void {
	if (structuredOutput) {
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
const originalConsoleError = console.error.bind(console);
const suppressedNoise = new Map<string, number>();
console.error = (...args: unknown[]): void => {
	const probe = args
		.map((a) => (a instanceof Error ? a.message : typeof a === 'string' ? a : ''))
		.join(' ');
	for (const pattern of NOISE_PATTERNS) {
		if (probe.includes(pattern)) {
			suppressedNoise.set(pattern, (suppressedNoise.get(pattern) ?? 0) + 1);
			return;
		}
	}
	originalConsoleError(...args);
};

//
// Configuration
//

/** Parse optional integer from env var */
const envInt = (name: string): number | undefined => {
	const val = Deno.env.get(name);
	return val ? parseInt(val) : undefined;
};

/** Limit files per language (default: all) */
const MAX_FILES_PER_LANGUAGE = envInt('BENCH_LIMIT');

/** Filter files by path pattern (default: none) */
const FILE_FILTER = Deno.env.get('BENCH_FILTER');

/** Duration per benchmark in ms (default: 5000) */
const BENCH_DURATION = envInt('BENCH_DURATION') ?? 5000;

/** Number of warmup iterations (default: 3) */
const BENCH_WARMUP = envInt('BENCH_WARMUP') ?? 3;

/**
 * Enable the per-iteration forced-GC hook (default: off — measures realistic
 * throughput where GC happens opportunistically, matching real-world usage).
 * Set `BENCH_GC=1` to force a major GC between every iteration; useful for
 * stabilizing high-allocation workloads at the cost of penalizing efficient
 * low-allocation paths. See `CLAUDE.md` → Fairness Caveats for the trade-off.
 */
const BENCH_GC = Deno.env.get('BENCH_GC') === '1';

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
const BENCH_MODE = Deno.env.get('BENCH_MODE');
if (BENCH_MODE !== undefined && BENCH_MODE !== 'intersection' && BENCH_MODE !== 'union') {
	console.error(`Invalid BENCH_MODE: ${BENCH_MODE}. Expected 'intersection' or 'union'.`);
	Deno.exit(1);
}
const USE_INTERSECTION = BENCH_MODE !== 'union';

/** Maximum length of error message to display (longer messages are truncated) */
const MAX_ERROR_MESSAGE_LENGTH = 200;

/**
 * Baseline storage directory. Passed to `benchmark_baseline_save` /
 * `_compare`; the library calls `mkdir(path, { recursive: true })` and
 * writes `baseline.json` inside, so the file lands at
 * `./benches/deno/results/baseline.json`. Moved into `results/` (from its
 * pre-0.60 location at `./benches/deno/baseline.json`) so the library's
 * mkdir is covered by the existing `--allow-write=benches/deno/results`
 * permission without widening write scope to the whole benches tree.
 */
const BASELINE_DIR = './benches/deno/results';

/** Results directory for comparison JSON files */
const RESULTS_DIR = './benches/deno/results';

//
// Setup
//

log('Loading corpus...\n');
const corpusLoader = new DevReposLoader();
// Drain `stream()` directly instead of `load()` so we skip the loader's
// own corpus summary — bench.ts prints its own tighter one below that
// includes byte counts and (when applicable) limit annotations.
const files: SourceFile[] = [];
for await (const file of corpusLoader.stream(log)) {
	files.push(file);
}
const byLanguage = groupByLanguage(files);

// Preserve total counts before limiting
const totalFileCounts = {
	svelte: byLanguage.svelte.length,
	typescript: byLanguage.typescript.length,
	css: byLanguage.css.length,
};

// Apply file filter and limit
function limitFiles(files: SourceFile[]): SourceFile[] {
	const filtered = FILE_FILTER ? files.filter((f) => f.path.includes(FILE_FILTER)) : files;
	return MAX_FILES_PER_LANGUAGE ? filtered.slice(0, MAX_FILES_PER_LANGUAGE) : filtered;
}

const svelteFiles = limitFiles(byLanguage.svelte);
const tsFiles = limitFiles(byLanguage.typescript);
const cssFiles = limitFiles(byLanguage.css);

// Track if corpus is limited
const isLimited = MAX_FILES_PER_LANGUAGE !== undefined || FILE_FILTER !== undefined;

// Calculate total bytes per language for throughput metrics
const bytesByLanguage: Record<Language, number> = {
	svelte: svelteFiles.reduce((sum, f) => sum + f.bytes, 0),
	typescript: tsFiles.reduce((sum, f) => sum + f.bytes, 0),
	css: cssFiles.reduce((sum, f) => sum + f.bytes, 0),
};

/**
 * Format bytes/sec as MB/s. Always MB/s, even for sub-1-MB values
 * (renders as e.g. `0.4 MB/s`) so a column of throughput numbers scans
 * uniformly without unit-switching mid-table.
 */
function formatThroughput(bytesPerSec: number): string {
	return `${(bytesPerSec / 1_000_000).toFixed(1)} MB/s`;
}

// Compact corpus summary: file counts + MB per language + total. When
// limited, each line reads `N of M files` so the subset is obvious.
const totalFiles = svelteFiles.length + tsFiles.length + cssFiles.length;
const totalBytes = bytesByLanguage.svelte + bytesByLanguage.typescript + bytesByLanguage.css;
const fmtCount = (
	n: number,
	total: number,
) => (isLimited && n !== total ? `${n} of ${total}` : `${n}`);
const fmtBytes = (b: number) => `${(b / 1_000_000).toFixed(1)} MB`;
log(`Corpus:`);
log(
	`  Svelte:      ${fmtCount(svelteFiles.length, totalFileCounts.svelte).padEnd(11)} files (${
		fmtBytes(bytesByLanguage.svelte)
	})`,
);
log(
	`  TypeScript:  ${fmtCount(tsFiles.length, totalFileCounts.typescript).padEnd(11)} files (${
		fmtBytes(bytesByLanguage.typescript)
	})`,
);
log(
	`  CSS:         ${fmtCount(cssFiles.length, totalFileCounts.css).padEnd(11)} files (${
		fmtBytes(bytesByLanguage.css)
	})`,
);
log(`  Total:       ${String(totalFiles).padEnd(11)} files (${fmtBytes(totalBytes)})`);
log();

// Initialize implementations
const impls = await initImplementations({ logger: log });

//
// Benchmark Helpers
//

//
// Per-impl tracking maps (keyed by trackingKey, e.g. `parse/svelte/native`).
//
// Populated by the **untimed pre-flight pass** before each group's timed
// bench run. The pre-flight records each impl's success/skip set; the timed
// loop then iterates either the per-group all-N intersection (default) or
// each impl's preflight success set (`BENCH_MODE=union`).
//
// `successfulFiles` and `skippedFiles` always reflect preflight results,
// independent of the iteration mode — they are the source of truth for
// coverage disclosure. `effectiveCorpusBytes` and `iteratedFileCount` are
// updated to reflect what was actually timed (intersection or per-impl).
//

/** Files an impl successfully processed during pre-flight, keyed by trackingKey. */
const successfulFiles: Map<string, Set<string>> = new Map();
/** Files an impl failed on during pre-flight, with the error message. */
const skippedFiles: Map<string, Map<string, string>> = new Map();
/** Effective corpus size per benchmark (processed / total files). */
const effectiveCorpusSize: Map<string, { processed: number; total: number }> = new Map();
/** Effective corpus bytes per benchmark — used for honest throughput math. */
const effectiveCorpusBytes: Map<string, number> = new Map();
/**
 * Files actually iterated by the timed loop per task. Distinct from
 * `effectiveCorpusSize` (which records preflight success — disclosure-only
 * coverage info): in `intersection` mode this is the per-group all-N
 * intersection (uniform across tasks in a group); in `union` mode it's the
 * task's preflight success set. Used by the bench-table `Nx (Mf)` annotation
 * and the Comparisons table's pairwise file counts.
 */
const iteratedFileCount: Map<string, number> = new Map();
/**
 * Wall-clock ms for one preflight pass per task (iterating every file once).
 * Used to tier per-task `min_iterations` so slow tasks (multi-second per pass)
 * get a higher sample-size floor for trustworthy percentile/CI math, while
 * fast tasks rely on `duration_ms` to drive sample count.
 */
const preflightElapsedMs: Map<string, number> = new Map();
/**
 * Map result.name → trackingKey per group, so the markdown report can look up
 * coverage/throughput by display name (the bench library doesn't surface trackingKey).
 */
const taskTrackingByGroup: Map<string, Map<string, string>> = new Map();

function recordSkip(benchName: string, filePath: string, error: unknown): void {
	if (!skippedFiles.has(benchName)) {
		skippedFiles.set(benchName, new Map());
	}
	const benchMap = skippedFiles.get(benchName)!;
	if (benchMap.has(filePath)) return;
	const errorMsg = error instanceof Error ? error.message : String(error);
	benchMap.set(filePath, errorMsg);
}

/**
 * Iterate files and run `processFn` for each. The iteration list is
 * pre-filtered to files this task succeeded on during pre-flight (or the
 * group's all-N intersection in `intersection` mode), so throws are real
 * bugs — let them propagate to surface as benchmark errors rather than
 * silently catalog.
 */
function processCorpus(files: SourceFile[], processFn: (file: SourceFile) => void): void {
	for (const file of files) {
		processFn(file);
	}
}

/** Async variant of `processCorpus`. */
async function processCorpusAsync(
	files: SourceFile[],
	processFn: (file: SourceFile) => Promise<void>,
): Promise<void> {
	for (const file of files) {
		await processFn(file);
	}
}

/** Files by language lookup */
const filesByLanguage: Record<Language, SourceFile[]> = {
	svelte: svelteFiles,
	typescript: tsFiles,
	css: cssFiles,
};

/**
 * Run every task once per file untimed to discover each impl's effective
 * corpus. Populates `successfulFiles`, `skippedFiles`, and
 * `effectiveCorpusSize` so the caller can compute the per-group iteration
 * set (intersection or per-impl) and the report can disclose coverage.
 *
 * Cost: O(impls × files), each call is one parse/format. Small relative
 * to the timed loop (which iterates the same files for 5s+ per task).
 */
async function runPreflight(
	tasks: ReturnType<typeof getBenchmarkTasks>,
	files: SourceFile[],
	language: Language,
): Promise<void> {
	for (let i = 0; i < tasks.length; i++) {
		const task = tasks[i];
		const success = new Set<string>();
		let bytes = 0;
		const startMs = performance.now();
		for (const file of files) {
			try {
				if (task.isAsync) {
					await task.runAsync!(file.content, language);
				} else {
					task.run(file.content, language);
				}
				success.add(file.path);
				bytes += file.bytes;
			} catch (e) {
				recordSkip(task.trackingKey, file.path, e);
			}
		}
		const elapsedMs = performance.now() - startMs;
		successfulFiles.set(task.trackingKey, success);
		effectiveCorpusSize.set(task.trackingKey, { processed: success.size, total: files.length });
		effectiveCorpusBytes.set(task.trackingKey, bytes);
		preflightElapsedMs.set(task.trackingKey, elapsedMs);
		log(`  [${i + 1}/${tasks.length}] ${task.name}: ${success.size}/${files.length} files`);
	}
}

//
// Run Benchmarks
//

const allGroupResults: GroupResults[] = [];

/**
 * Per-group setup captured during the up-front pre-flight pass. Reused by
 * `runBenchmarkGroup` so the timed loop is purely measurement.
 */
interface GroupSetup {
	tasks: ReturnType<typeof getBenchmarkTasks>;
	filteredFilesByTask: Map<string, SourceFile[]>;
}
const groupSetups: Map<string, GroupSetup> = new Map();

/**
 * Run pre-flight + iteration-set computation for one group. Populates
 * `successfulFiles`, `skippedFiles`, `effectiveCorpusSize`,
 * `effectiveCorpusBytes`, `iteratedFileCount`, and `taskTrackingByGroup`,
 * and stashes the per-group setup in `groupSetups` for the timed pass.
 *
 * Doing this for every group up front (before any timed run) means the
 * coverage picture lands in the terminal/report before any 5s+ timed
 * benchmark starts — easier to spot a broken impl early.
 */
async function runPreflightGroup(operation: 'parse' | 'format', language: Language): Promise<void> {
	const files = filesByLanguage[language];
	if (files.length === 0) return;

	const groupName = `${operation}/${language}`;
	log(`\n· ${groupName}`);

	const tasks = getBenchmarkTasks(impls, operation, language);
	await runPreflight(tasks, files, language);

	const taskTracking = new Map<string, string>();
	for (const task of tasks) {
		taskTracking.set(task.name, task.trackingKey);
	}
	taskTrackingByGroup.set(groupName, taskTracking);

	// Build each task's iteration file list. In `intersection` mode (default)
	// every task in the group iterates the same all-N intersection, making
	// timing ratios within the group apples-to-apples. In `union` mode each
	// task iterates its own preflight success set — ratios then reflect
	// different file sets per impl, useful for auditing what intersection
	// mode hides.
	const filteredFilesByTask = new Map<string, SourceFile[]>();
	if (USE_INTERSECTION) {
		let intersection: Set<string> | null = null;
		for (const task of tasks) {
			const successSet = successfulFiles.get(task.trackingKey) ?? new Set<string>();
			if (intersection === null) {
				intersection = new Set(successSet);
			} else {
				for (const path of intersection) {
					if (!successSet.has(path)) intersection.delete(path);
				}
			}
		}
		const intersectionList = files.filter((f) => (intersection ?? new Set<string>()).has(f.path));
		for (const task of tasks) {
			filteredFilesByTask.set(task.trackingKey, intersectionList);
		}
		log(`  Intersection: ${intersectionList.length}/${files.length} files`);
	} else {
		for (const task of tasks) {
			const successSet = successfulFiles.get(task.trackingKey) ?? new Set<string>();
			filteredFilesByTask.set(
				task.trackingKey,
				files.filter((f) => successSet.has(f.path)),
			);
		}
	}

	// Overwrite preflight-derived byte counts with iteration byte counts so
	// throughput math (`ops_per_sec × effectiveCorpusBytes`) reflects what was
	// actually measured. Also record per-task iteration size for the
	// `Nx (Mf)` annotation in the bench-table `vs baseline` column.
	for (const task of tasks) {
		const taskFiles = filteredFilesByTask.get(task.trackingKey)!;
		effectiveCorpusBytes.set(task.trackingKey, taskFiles.reduce((sum, f) => sum + f.bytes, 0));
		iteratedFileCount.set(task.trackingKey, taskFiles.length);
	}

	groupSetups.set(groupName, { tasks, filteredFilesByTask });
}

/** Run the timed measurement loop for one group using its stashed pre-flight setup. */
async function runBenchmarkGroup(operation: 'parse' | 'format', language: Language): Promise<void> {
	const groupName = `${operation}/${language}`;
	const setup = groupSetups.get(groupName);
	if (!setup) return;
	const { tasks, filteredFilesByTask } = setup;
	const taskTracking = taskTrackingByGroup.get(groupName) ?? new Map<string, string>();

	log(`\n▶ ${groupName}`);

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
		// See benches/deno/CLAUDE.md → Known Issues.
		cooldown_ms: 0,
		on_iteration: BENCH_GC ? () => globalThis.gc?.() : undefined,
		on_task_complete: (result, index, total) => {
			const opsPerSec = result.stats.ops_per_second.toFixed(1);
			// Throughput uses effective bytes (this impl's success set) so
			// the displayed MB/s is what this impl actually achieved, not
			// what it would have done on the full corpus.
			const trackingKey = taskTracking.get(result.name);
			const effectiveBytes = trackingKey ? effectiveCorpusBytes.get(trackingKey) ?? 0 : 0;
			const throughput = formatThroughput(result.stats.ops_per_second * effectiveBytes);
			log(`  [${index + 1}/${total}] ${result.name}: ${opsPerSec} ops/sec (${throughput})`);
		},
	});

	for (const task of tasks) {
		const taskFiles = filteredFilesByTask.get(task.trackingKey)!;
		// Tier per-task `min_iterations` based on preflight pass time. The
		// suite floor (5) handles most cases; very slow tasks (>5s/pass —
		// prettier on the full TS corpus, oxfmt full passes) get a bump to 7
		// because at n=5 their p75/p90 still sit too close to max and the
		// Welch DOF is on the edge. Above that we don't keep climbing: each
		// extra iteration on a 14s/pass task costs another 14s of wall clock.
		const preflightMs = preflightElapsedMs.get(task.trackingKey) ?? 0;
		const minIter = preflightMs > 5000 ? 7 : undefined;
		const baseTask = { name: task.name, min_iterations: minIter };
		if (task.isAsync) {
			bench.add({
				...baseTask,
				fn: async () => {
					await processCorpusAsync(taskFiles, async (f) => {
						await task.runAsync!(f.content, language);
					});
				},
				async: true,
			});
		} else {
			bench.add({
				...baseTask,
				fn: () => {
					processCorpus(taskFiles, (f) => task.run(f.content, language));
				},
				async: false,
			});
		}
	}

	const results = await bench.run();
	allGroupResults.push({ name: groupName, results });
}

// Two-phase run: pre-flight every group up front (so the coverage picture
// lands before any 5s+ timed run starts), then time every group.
log('Pre-flight (discover coverage before timing):');
for (const lang of LANGUAGES) {
	await runPreflightGroup('parse', lang);
	await runPreflightGroup('format', lang);
}

log('\nRunning benchmarks:');
for (const lang of LANGUAGES) {
	await runBenchmarkGroup('parse', lang);
	await runBenchmarkGroup('format', lang);
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
}

/** Package versions used in the benchmark run */
interface BaselineVersions {
	svelte: string;
	acorn: string;
	acornTs: string;
	prettier: string;
	prettierSvelte: string;
	oxcParser?: string;
	oxfmt?: string;
	biome?: string;
}

interface Baseline {
	version: number;
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
}

/** Get current git commit hash */
async function getGitCommit(): Promise<string | null> {
	try {
		const cmd = new Deno.Command('git', {
			args: ['rev-parse', 'HEAD'],
			stdout: 'piped',
			stderr: 'null',
		});
		const output = await cmd.output();
		if (output.success) {
			return new TextDecoder().decode(output.stdout).trim().slice(0, 8);
		}
	} catch {
		// Ignore
	}
	return null;
}

/** Build results data from current benchmark run */
async function buildResultsData(
	groups: GroupResults[],
	corpus: { svelte: number; typescript: number; css: number },
	versions: BaselineVersions,
	binarySizes: BinarySize[],
): Promise<Baseline> {
	const entries: BaselineEntry[] = [];
	for (const group of groups) {
		for (const result of group.results) {
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
			});
		}
	}

	return {
		version: 2,
		timestamp: new Date().toISOString(),
		git_commit: await getGitCommit(),
		corpus,
		versions,
		binary_sizes: binarySizes,
		entries,
	};
}

/** Format bytes as MB with one decimal */
function formatMB(bytes: number): string {
	return `${(bytes / 1_000_000).toFixed(1)} MB`;
}

/** Generate a full markdown report from benchmark data */
function generateMarkdownReport(
	groups: GroupResults[],
	binarySizes: BinarySize[],
	corpus: { svelte: number; typescript: number; css: number },
	corpusBytes: Record<Language, number>,
	versions: BaselineVersions,
	timestamp: string,
	gitCommit: string | null,
	taskTracking: Map<string, Map<string, string>>,
	effectiveSize: Map<string, EffectiveCorpusEntry>,
	effectiveBytes: Map<string, number>,
	iteratedCounts: Map<string, number>,
	skipped: Map<string, Map<string, string>>,
): string {
	const lines: string[] = [];
	lines.push('# TSV Benchmark Results\n');
	const commitStr = gitCommit ? ` (${gitCommit})` : '';
	lines.push(`**Date:** ${timestamp}${commitStr}\n`);

	const totalFiles = corpus.svelte + corpus.typescript + corpus.css;
	const totalBytes = corpusBytes.svelte + corpusBytes.typescript + corpusBytes.css;
	lines.push(
		`**Corpus:** ${corpus.svelte} Svelte (${formatMB(corpusBytes.svelte)}), ` +
			`${corpus.typescript} TypeScript (${formatMB(corpusBytes.typescript)}), ` +
			`${corpus.css} CSS (${formatMB(corpusBytes.css)}) — ` +
			`${totalFiles} files, ${formatMB(totalBytes)} total\n`,
	);

	// Versions
	const versionParts = [
		`svelte@${versions.svelte}`,
		`acorn@${versions.acorn}`,
		`acorn-typescript@${versions.acornTs}`,
		`prettier@${versions.prettier}`,
		`prettier-plugin-svelte@${versions.prettierSvelte}`,
	];
	if (versions.oxcParser) versionParts.push(`oxc-parser@${versions.oxcParser}`);
	if (versions.oxfmt) versionParts.push(`oxfmt@${versions.oxfmt}`);
	if (versions.biome) versionParts.push(`@biomejs/wasm-bundler@${versions.biome}`);
	lines.push(`**Versions:** ${versionParts.join(', ')}\n`);

	for (const group of groups) {
		if (group.results.length === 0) continue;
		const [operation, language] = group.name.split('/') as ['parse' | 'format', Language];
		// Use the canonical reference as the bench-table baseline. Without this,
		// the library picks the fastest task (often `tsv-internal`, a non-public
		// optimization variant) which is not the comparison readers want.
		const baseline = operation === 'format' ? 'prettier' : canonicalParserLabel(language);
		const baselineExists = group.results.some((r) => r.name === baseline);

		const tracking = taskTracking.get(group.name);
		// Build display-name → iterated-count map for this group, so the table
		// renderer can append `(Mf)` to each row's `vs baseline` cell.
		const groupIteratedCounts = new Map<string, number>();
		if (tracking) {
			for (const [displayName, trackingKey] of tracking) {
				const m = iteratedCounts.get(trackingKey);
				if (m !== undefined) groupIteratedCounts.set(displayName, m);
			}
		}

		lines.push(`## ${group.name}\n`);
		lines.push(
			generateGroupBenchTableMarkdown(group.results, baselineExists ? baseline : undefined),
		);
		lines.push('');

		const files = generateGroupFilesMarkdown(groupIteratedCounts);
		if (files) lines.push(files, '');

		const throughput = generateGroupThroughputMarkdown(group.results, tracking, effectiveBytes);
		if (throughput) lines.push(throughput, '');

		const coverage = generateGroupCoverageMarkdown(group.results, tracking, effectiveSize);
		if (coverage) lines.push(coverage, '');

		if (operation === 'parse') {
			const jsonNote = generateJsonOverheadNote(group.results);
			if (jsonNote) lines.push(jsonNote, '');
		}
	}

	// Convention note: every `Nx` in this report is speedup form — values > 1
	// mean self is faster than the opponent. File counts are surfaced per
	// group (Files / Coverage lines) and per row in the Comparisons tables.
	lines.push(
		'_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._\n',
	);

	const binarySizeMarkdown = generateBinarySizeMarkdown(binarySizes);
	if (binarySizeMarkdown) {
		lines.push(binarySizeMarkdown);
		lines.push('');
	}

	const comparisonMarkdown = generateComparisonMarkdown(
		groups,
		LANGUAGES,
		iteratedCounts,
		taskTracking,
	);
	if (comparisonMarkdown) {
		lines.push(comparisonMarkdown);
		lines.push('');
	}

	const skippedMarkdown = generateSkippedFilesMarkdown(
		skipped,
		MAX_ERROR_MESSAGE_LENGTH,
		args.verbose,
		taskTracking,
	);
	if (skippedMarkdown) {
		lines.push(skippedMarkdown);
		lines.push('');
	}

	return lines.join('\n');
}

/**
 * Save results to the results directory.
 *
 * Always writes a timestamped pair. Only overwrites the canonical
 * `report.{json,md}` when `writeReport` is true — gated by the caller so
 * that partial runs (BENCH_LIMIT, BENCH_FILTER) don't clobber the
 * committed canonical report.
 */
async function saveResults(
	data: Baseline,
	groups: GroupResults[],
	binarySizes: BinarySize[],
	writeReport: boolean,
): Promise<string> {
	await Deno.mkdir(RESULTS_DIR, { recursive: true });
	const timestamp = data.timestamp.replace(/[:.]/g, '-').slice(0, 19);
	const commit = data.git_commit ?? 'unknown';
	const basePath = `${RESULTS_DIR}/${timestamp}_${commit}`;

	const markdown = generateMarkdownReport(
		groups,
		binarySizes,
		data.corpus,
		bytesByLanguage,
		data.versions,
		data.timestamp,
		data.git_commit,
		taskTrackingByGroup,
		effectiveCorpusSize,
		effectiveCorpusBytes,
		iteratedFileCount,
		skippedFiles,
	);

	const json = JSON.stringify(data, null, '\t');
	const writes: Promise<void>[] = [
		Deno.writeTextFile(`${basePath}.json`, json),
		Deno.writeTextFile(`${basePath}.md`, markdown),
	];
	if (writeReport) {
		writes.push(
			Deno.writeTextFile(`${RESULTS_DIR}/report.json`, json),
			Deno.writeTextFile(`${RESULTS_DIR}/report.md`, markdown),
		);
	}
	await Promise.all(writes);

	return basePath;
}

/**
 * Flatten `allGroupResults` into a single list with namespaced names. The
 * fuz_util baseline module joins by `result.name` and our task names repeat
 * across groups (`tsv` lives in `format/svelte`, `format/typescript`,
 * `format/css`). Without namespacing, the last write wins and three groups
 * collapse into one.
 */
function flattenResultsForBaseline(groups: GroupResults[]): BenchmarkResult[] {
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
function buildBaselineMetadata(data: Baseline): Record<string, unknown> {
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
async function saveBaseline(data: Baseline): Promise<void> {
	await benchmark_baseline_save(flattenResultsForBaseline(allGroupResults), {
		path: BASELINE_DIR,
		metadata: buildBaselineMetadata(data),
	});
	log(`Baseline saved to ${BASELINE_DIR}/baseline.json`);
}

/**
 * Compare current results against the stored baseline. Uses Welch's t-test
 * (via `benchmark_baseline_compare`) for significance, methodology-change
 * detection for per-task budget drift, and OR-gated noise warnings on
 * high-cv or high-outlier-ratio rows. The flat ±5% ops/sec gate that lived
 * here previously is gone — see `benchmark_baseline_compare` and the
 * fairness caveats in benches/deno/CLAUDE.md.
 */
async function compareBaseline(current: Baseline): Promise<void> {
	const comparison = await benchmark_baseline_compare(flattenResultsForBaseline(allGroupResults), {
		path: BASELINE_DIR,
		// 1.0 means "any statistically significant slowdown counts." Tune
		// upward (e.g. 1.05) to suppress trivial regressions in CI without
		// losing the practical-significance gate already inside the Welch
		// comparison (`min_percent_difference` default 0.10).
		regression_threshold: 1.0,
		// Mark the baseline stale after a week so a long-untouched baseline
		// doesn't quietly mask drift accumulated over months.
		staleness_warning_days: 7,
	});

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
	const baselineCorpus = meta?.corpus;
	const corpusMatch = baselineCorpus &&
		baselineCorpus.svelte === current.corpus.svelte &&
		baselineCorpus.typescript === current.corpus.typescript &&
		baselineCorpus.css === current.corpus.css;
	if (baselineCorpus && !corpusMatch) {
		log(`\n⚠️  Corpus size differs from baseline:`);
		log(
			`   Baseline: svelte=${baselineCorpus.svelte}, ts=${baselineCorpus.typescript}, css=${baselineCorpus.css}`,
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

// Collect binary sizes once (used by all output paths)
const binarySizes = await collectBinarySizes(impls.versions, {
	hasNative: !!impls.native,
	hasWasm: !!impls.wasm,
	hasOxc: !!impls.oxc,
	hasBiome: !!impls.biome,
});

// Build results data (used by all output paths and always saved)
const corpus = {
	svelte: svelteFiles.length,
	typescript: tsFiles.length,
	css: cssFiles.length,
};
const altVersions = getAlternativeVersions(impls);
const v = impls.versions.canonical;
const versions: BaselineVersions = {
	svelte: v.svelte,
	acorn: v.acorn,
	acornTs: v['@sveltejs/acorn-typescript'],
	prettier: v.prettier,
	prettierSvelte: v['prettier-plugin-svelte'],
	...altVersions,
};
const resultsData = await buildResultsData(allGroupResults, corpus, versions, binarySizes);

if (args.json) {
	// JSON output (same structure as saved results)
	console.log(JSON.stringify(resultsData, null, '\t'));
} else if (args.markdown) {
	console.log(
		generateMarkdownReport(
			allGroupResults,
			binarySizes,
			corpus,
			bytesByLanguage,
			versions,
			resultsData.timestamp,
			resultsData.git_commit,
			taskTrackingByGroup,
			effectiveCorpusSize,
			effectiveCorpusBytes,
			iteratedFileCount,
			skippedFiles,
		),
	);
} else {
	// Standard text output
	console.log(generateSummaryReport(allGroupResults, LANGUAGES));

	console.log(generateVersionsInfo(versions));

	const effectiveCorpusReport = generateEffectiveCorpusReport(
		effectiveCorpusSize,
		taskTrackingByGroup,
	);
	if (effectiveCorpusReport) {
		console.log(effectiveCorpusReport);
	}

	const skippedReport = generateSkippedFilesReport(
		skippedFiles,
		MAX_ERROR_MESSAGE_LENGTH,
		args.verbose,
		taskTrackingByGroup,
	);
	if (skippedReport) {
		console.log(skippedReport);
	}

	const binarySizeReport = generateBinarySizeReport(binarySizes);
	if (binarySizeReport) {
		console.log(binarySizeReport);
	}

	// Compact comparison summary
	console.log(
		generateComparisonSummary(allGroupResults, LANGUAGES, iteratedFileCount, taskTrackingByGroup),
	);

	console.log('\n' + '='.repeat(80));
}

// Surface suppressed stderr noise counts so silenced upstream bugs don't
// just vanish. Counts are accurate even when individual messages aren't.
if (suppressedNoise.size > 0) {
	log('');
	log('Suppressed stderr noise from upstream impls:');
	for (const [pattern, count] of suppressedNoise) {
		log(`  ${count}× ${pattern}`);
	}
}

// Always save the timestamped pair; only overwrite the canonical
// `report.{json,md}` on full-corpus runs or when --save-report is set.
const writeReport = args.saveReport || !isLimited;
const resultsPath = await saveResults(resultsData, allGroupResults, binarySizes, writeReport);
log(`\nResults saved to:`);
log(`  ${resultsPath}.json`);
log(`  ${resultsPath}.md`);
if (writeReport) {
	log(`Canonical report updated:`);
	log(`  ${RESULTS_DIR}/report.json`);
	log(`  ${RESULTS_DIR}/report.md`);
} else {
	log(`Skipped canonical report (limited run — pass --save-report to override)`);
}

// Handle baseline operations
if (args.saveBaseline) {
	await saveBaseline(resultsData);
}

if (args.compareBaseline) {
	await compareBaseline(resultsData);
}
