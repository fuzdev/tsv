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
import { benchmark_format_markdown } from '@fuzdev/fuz_util/benchmark_format.js';
import { DevReposLoader, groupByLanguage } from './lib/corpus.ts';
import {
	canonicalParserLabel,
	getAlternativeVersions,
	getBenchmarkTasks,
	initImplementations,
} from './lib/implementations.ts';
import {
	generateComparisonMarkdown,
	generateComparisonSummary,
	generateCorpusInfo,
	generateEffectiveCorpusReport,
	generateSkippedFilesReport,
	generateSummaryReport,
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

/** Maximum length of error message to display (longer messages are truncated) */
const MAX_ERROR_MESSAGE_LENGTH = 200;

/** Baseline file path */
const BASELINE_PATH = './benches/deno/baseline.json';

/** Results directory for comparison JSON files */
const RESULTS_DIR = './benches/deno/results';

//
// Setup
//

log('Loading corpus...\n');
const corpusLoader = new DevReposLoader();
const files = await corpusLoader.load(log);
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

/** Format bytes/sec as human-readable throughput */
function formatThroughput(bytesPerSec: number): string {
	if (bytesPerSec >= 1_000_000_000) {
		return `${(bytesPerSec / 1_000_000_000).toFixed(1)} GB/s`;
	} else if (bytesPerSec >= 1_000_000) {
		return `${(bytesPerSec / 1_000_000).toFixed(1)} MB/s`;
	} else if (bytesPerSec >= 1_000) {
		return `${(bytesPerSec / 1_000).toFixed(1)} KB/s`;
	}
	return `${bytesPerSec.toFixed(0)} B/s`;
}

log(`Benchmarking with:`);
log(`  Svelte: ${svelteFiles.length} files (${(bytesByLanguage.svelte / 1024).toFixed(0)} KB)`);
log(`  TypeScript: ${tsFiles.length} files (${(bytesByLanguage.typescript / 1024).toFixed(0)} KB)`);
log(`  CSS: ${cssFiles.length} files (${(bytesByLanguage.css / 1024).toFixed(0)} KB)`);
log();

// Initialize implementations
const impls = await initImplementations({ logger: log });

//
// Benchmark Helpers
//

// Track skipped files for reporting
const skippedFiles: Map<string, Map<string, string>> = new Map();
// Track effective corpus size per benchmark (files actually processed)
const effectiveCorpusSize: Map<string, { processed: number; total: number }> = new Map();

function recordSkip(benchName: string, filePath: string, error: unknown): void {
	if (!skippedFiles.has(benchName)) {
		skippedFiles.set(benchName, new Map());
	}
	const errorMsg = error instanceof Error ? error.message : String(error);
	skippedFiles.get(benchName)!.set(filePath, errorMsg);
}

function recordCorpusSize(benchName: string, processed: number, total: number): void {
	// Idempotent: every measured iteration re-counts the same files (same
	// content, same fn), so only the first record matters. Avoids hundreds
	// of redundant Map.set calls per task during the inner measurement loop.
	if (!effectiveCorpusSize.has(benchName)) {
		effectiveCorpusSize.set(benchName, { processed, total });
	}
}

/** Process all files with the given function, tracking errors and effective corpus size */
function processCorpus(
	files: SourceFile[],
	processFn: (file: SourceFile) => void,
	benchName: string,
): void {
	let processed = 0;
	for (const file of files) {
		try {
			processFn(file);
			processed++;
		} catch (e) {
			recordSkip(benchName, file.path, e);
		}
	}
	recordCorpusSize(benchName, processed, files.length);
}

/** Process all files with async function, tracking errors and effective corpus size */
async function processCorpusAsync(
	files: SourceFile[],
	processFn: (file: SourceFile) => Promise<void>,
	benchName: string,
): Promise<void> {
	let processed = 0;
	for (const file of files) {
		try {
			await processFn(file);
			processed++;
		} catch (e) {
			recordSkip(benchName, file.path, e);
		}
	}
	recordCorpusSize(benchName, processed, files.length);
}

/** Files by language lookup */
const filesByLanguage: Record<Language, SourceFile[]> = {
	svelte: svelteFiles,
	typescript: tsFiles,
	css: cssFiles,
};

//
// Run Benchmarks
//

const allGroupResults: GroupResults[] = [];

/** Run benchmarks for a specific operation and language */
async function runBenchmarkGroup(operation: 'parse' | 'format', language: Language): Promise<void> {
	const files = filesByLanguage[language];
	if (files.length === 0) return;

	const groupName = `${operation}/${language}`;
	log(`\n▶ ${groupName}`);

	const corpusBytes = bytesByLanguage[language];

	const bench = new Benchmark({
		duration_ms: BENCH_DURATION,
		warmup_iterations: BENCH_WARMUP,
		min_iterations: 3,
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
			const throughput = formatThroughput(result.stats.ops_per_second * corpusBytes);
			log(`  [${index + 1}/${total}] ${result.name}: ${opsPerSec} ops/sec (${throughput})`);
		},
	});

	// Get all benchmark tasks for this operation/language from the registry
	const tasks = getBenchmarkTasks(impls, operation, language);

	for (const task of tasks) {
		if (task.isAsync) {
			bench.add({
				name: task.name,
				fn: async () => {
					await processCorpusAsync(
						files,
						async (f) => {
							await task.runAsync!(f.content, language);
						},
						task.trackingKey,
					);
				},
				async: true,
			});
		} else {
			bench.add(task.name, () => {
				processCorpus(files, (f) => task.run(f.content, language), task.trackingKey);
			});
		}
	}

	const results = await bench.run();
	allGroupResults.push({ name: groupName, results });

	// Show detailed table for each group (not in structured output mode)
	if (!structuredOutput && results.length > 0) {
		const baseline = operation === 'format' ? 'prettier' : canonicalParserLabel(language);

		log('');
		log(
			bench.table({
				groups: [
					{
						name: groupName,
						filter: () => true, // all results in this group
						baseline,
					},
				],
			}),
		);
	}
}

// Run all benchmark groups
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

interface BaselineComparison {
	name: string;
	group: string;
	ratio: number;
	baseline: number;
	current: number;
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

/** Generate a full markdown report from benchmark data */
function generateMarkdownReport(
	groups: GroupResults[],
	binarySizes: BinarySize[],
	corpus: { svelte: number; typescript: number; css: number },
	versions: BaselineVersions,
	timestamp: string,
	gitCommit: string | null,
): string {
	const lines: string[] = [];
	lines.push('# TSV Benchmark Results\n');
	const commitStr = gitCommit ? ` (${gitCommit})` : '';
	lines.push(`**Date:** ${timestamp}${commitStr}\n`);
	lines.push(
		`**Corpus:** ${corpus.svelte} Svelte, ${corpus.typescript} TypeScript, ${corpus.css} CSS files\n`,
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
		lines.push(`## ${group.name}\n`);
		lines.push(benchmark_format_markdown(group.results));
		lines.push('');
	}

	const binarySizeMarkdown = generateBinarySizeMarkdown(binarySizes);
	if (binarySizeMarkdown) {
		lines.push(binarySizeMarkdown);
		lines.push('');
	}

	const comparisonMarkdown = generateComparisonMarkdown(groups, LANGUAGES);
	if (comparisonMarkdown) {
		lines.push(comparisonMarkdown);
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
		data.versions,
		data.timestamp,
		data.git_commit,
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

/** Save current results as baseline */
async function saveBaseline(data: Baseline): Promise<void> {
	await Deno.writeTextFile(BASELINE_PATH, JSON.stringify(data, null, '\t'));
	log(`Baseline saved to ${BASELINE_PATH}`);
}

/** Load and compare against baseline */
async function compareBaseline(current: Baseline): Promise<void> {
	let baseline: Baseline;
	try {
		const content = await Deno.readTextFile(BASELINE_PATH);
		const parsed = JSON.parse(content);
		// Backward compat: v1 baselines don't have binary_sizes or versions
		baseline = {
			...parsed,
			binary_sizes: parsed.binary_sizes ?? [],
			versions: parsed.versions ?? {},
		};
	} catch {
		console.error(`\nNo baseline found at ${BASELINE_PATH}. Run with --save-baseline first.`);
		return;
	}

	log('\n' + '='.repeat(80));
	log('BASELINE COMPARISON');
	log('='.repeat(80));
	log(`\nBaseline from: ${baseline.timestamp}`);
	if (baseline.git_commit) {
		log(`Baseline commit: ${baseline.git_commit}`);
	}

	// Check corpus size match
	const corpusMatch = baseline.corpus.svelte === current.corpus.svelte &&
		baseline.corpus.typescript === current.corpus.typescript &&
		baseline.corpus.css === current.corpus.css;

	if (!corpusMatch) {
		log(`\n⚠️  Corpus size differs from baseline:`);
		log(
			`   Baseline: svelte=${baseline.corpus.svelte}, ts=${baseline.corpus.typescript}, css=${baseline.corpus.css}`,
		);
		log(
			`   Current:  svelte=${current.corpus.svelte}, ts=${current.corpus.typescript}, css=${current.corpus.css}`,
		);
	}

	// Build lookup maps
	const baselineMap = new Map<string, BaselineEntry>();
	for (const entry of baseline.entries) {
		baselineMap.set(`${entry.group}/${entry.name}`, entry);
	}

	const currentMap = new Map<string, BaselineEntry>();
	for (const entry of current.entries) {
		currentMap.set(`${entry.group}/${entry.name}`, entry);
	}

	// Compare results
	const regressions: BaselineComparison[] = [];
	const improvements: BaselineComparison[] = [];

	for (const [key, baselineEntry] of baselineMap) {
		const currentEntry = currentMap.get(key);
		if (!currentEntry) continue;

		const ratio = currentEntry.ops_per_second / baselineEntry.ops_per_second;

		if (ratio < 0.95) {
			// More than 5% slower
			regressions.push({
				name: currentEntry.name,
				group: currentEntry.group,
				ratio,
				baseline: baselineEntry.ops_per_second,
				current: currentEntry.ops_per_second,
			});
		} else if (ratio > 1.05) {
			// More than 5% faster
			improvements.push({
				name: currentEntry.name,
				group: currentEntry.group,
				ratio,
				baseline: baselineEntry.ops_per_second,
				current: currentEntry.ops_per_second,
			});
		}
	}

	if (regressions.length > 0) {
		log('\n❌ Regressions (>5% slower):');
		for (const r of regressions.sort((a, b) => a.ratio - b.ratio)) {
			const pct = ((1 - r.ratio) * 100).toFixed(1);
			log(
				`   ${r.group}/${r.name}: ${pct}% slower (${r.baseline.toFixed(1)} → ${
					r.current.toFixed(
						1,
					)
				} ops/sec)`,
			);
		}
	}

	if (improvements.length > 0) {
		log('\n✅ Improvements (>5% faster):');
		for (const r of improvements.sort((a, b) => b.ratio - a.ratio)) {
			const pct = ((r.ratio - 1) * 100).toFixed(1);
			log(
				`   ${r.group}/${r.name}: ${pct}% faster (${r.baseline.toFixed(1)} → ${
					r.current.toFixed(
						1,
					)
				} ops/sec)`,
			);
		}
	}

	if (regressions.length === 0 && improvements.length === 0) {
		log('\n✓ No significant changes from baseline (within ±5%)');
	}
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
			versions,
			resultsData.timestamp,
			resultsData.git_commit,
		),
	);
} else {
	// Standard text output
	console.log(generateSummaryReport(allGroupResults, LANGUAGES));

	console.log(
		generateCorpusInfo(
			corpus,
			isLimited ? totalFileCounts : undefined,
			versions,
		),
	);

	const effectiveCorpusReport = generateEffectiveCorpusReport(effectiveCorpusSize);
	if (effectiveCorpusReport) {
		console.log(effectiveCorpusReport);
	}

	const skippedReport = generateSkippedFilesReport(skippedFiles, MAX_ERROR_MESSAGE_LENGTH);
	if (skippedReport) {
		console.log(skippedReport);
	}

	const binarySizeReport = generateBinarySizeReport(binarySizes);
	if (binarySizeReport) {
		console.log(binarySizeReport);
	}

	// Compact comparison summary
	console.log(generateComparisonSummary(allGroupResults, LANGUAGES));

	console.log('\n' + '='.repeat(80));
}

// Always save the timestamped pair; only overwrite the canonical
// `report.{json,md}` on full-corpus runs or when --save-report is set.
const writeReport = args.saveReport || !isLimited;
const resultsPath = await saveResults(resultsData, allGroupResults, binarySizes, writeReport);
log(`\nResults saved to ${resultsPath}.json and ${resultsPath}.md`);
if (writeReport) {
	log(`Canonical report updated: ${RESULTS_DIR}/report.{json,md}`);
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
