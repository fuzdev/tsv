/**
 * TSV Benchmark Suite
 *
 * Compares parsing and formatting performance across three implementations:
 * - Canonical: prettier + svelte/compiler (JavaScript baseline)
 * - Native: tsv via FFI (Rust, maximum performance)
 * - WASM: tsv compiled to WebAssembly (portable, near-native)
 *
 * Run with: deno task bench:run
 *
 * CLI options:
 *   --json              Output results as JSON
 *   --markdown          Output results as Markdown
 *   --save-baseline     Save results as baseline for regression detection
 *   --compare-baseline  Compare against saved baseline
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

import { Benchmark } from '@fuzdev/fuz_util/benchmark.js';
import { benchmark_format_markdown } from '@fuzdev/fuz_util/benchmark_format.js';
import { DevReposLoader, groupByLanguage } from './lib/corpus.ts';
import {
	canonicalParserLabel,
	getAlternativeVersions,
	getBenchmarkTasks,
	getFormattersForValidation,
	initImplementations,
	VERSIONS,
} from './lib/implementations.ts';
import {
	generateCorpusInfo,
	generateEffectiveCorpusReport,
	generateSkippedFilesReport,
	generateSummaryReport,
	type GroupResults,
} from './lib/report.ts';
import type { Language, SourceFile } from './lib/types.ts';

// ============================================================================
// CLI Arguments
// ============================================================================

const args = {
	json: Deno.args.includes('--json'),
	markdown: Deno.args.includes('--markdown'),
	saveBaseline: Deno.args.includes('--save-baseline'),
	compareBaseline: Deno.args.includes('--compare-baseline'),
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

// ============================================================================
// Configuration
// ============================================================================

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

/** Maximum length of error message to display (longer messages are truncated) */
const MAX_ERROR_MESSAGE_LENGTH = 200;

/** Languages to benchmark */
const LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

/** Baseline file path */
const BASELINE_PATH = 'benches/deno/baseline.json';

// ============================================================================
// Setup
// ============================================================================

log('Loading corpus...\n');
const corpusLoader = new DevReposLoader();
const { files } = await corpusLoader.load(log);
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

// ============================================================================
// Formatter Validation
// ============================================================================

// Validate formatters before benchmarking
{
	log('Validating formatters...\n');

	const formatters = getFormattersForValidation(impls);

	// Helper to call formatter (sync or async) - returns Promise for uniform handling
	const callFormatter = (
		formatter: (typeof formatters)[0],
		source: string,
		lang: Language,
	): Promise<string> => {
		if (formatter.isAsync) {
			return formatter.formatAsync!(source, lang);
		}
		return Promise.resolve(formatter.format!(source, lang));
	};

	// Unformatted test content - must be changed by a working formatter
	const unformatted: Record<Language, string> = {
		svelte: '<script>const x=1</script>\n<div class="a"   ></div>',
		typescript: 'const x:number=1;function foo(a:string,b:number){return a+b}',
		css: '.foo{color:red;display:flex}',
	};

	let hasErrors = false;

	for (const lang of LANGUAGES) {
		log(`  ${lang}:`);

		for (const formatter of formatters) {
			if (!formatter.supportsLanguage(lang)) {
				log(`    ${formatter.name.padEnd(12)} skipped (unsupported)`);
				continue;
			}

			const errors: string[] = [];

			// Test 1: Does it actually format? (output differs from unformatted input)
			try {
				const input = unformatted[lang];
				const output = await callFormatter(formatter, input, lang);
				if (input === output) {
					errors.push('no change on unformatted input');
				}
			} catch (e) {
				const msg = e instanceof Error ? e.message : String(e);
				errors.push(`format error: ${msg.slice(0, 50)}`);
			}

			// Test 2: Is it idempotent? (format(format(x)) == format(x))
			try {
				const input = unformatted[lang];
				const first = await callFormatter(formatter, input, lang);
				const second = await callFormatter(formatter, first, lang);
				if (first !== second) {
					errors.push('not idempotent');
				}
			} catch (e) {
				const msg = e instanceof Error ? e.message : String(e);
				errors.push(`idempotent error: ${msg.slice(0, 50)}`);
			}

			if (errors.length > 0) {
				hasErrors = true;
				log(`    ${formatter.name.padEnd(12)} ✗ ${errors.join(', ')}`);
			} else {
				log(`    ${formatter.name.padEnd(12)} ✓ formats + idempotent`);
			}
		}
	}

	log('');
	if (hasErrors) {
		log('⚠️  Some formatters failed validation. Results may be unreliable.\n');
	}
}

// ============================================================================
// Benchmark Helpers
// ============================================================================

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
	effectiveCorpusSize.set(benchName, { processed, total });
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

// ============================================================================
// Run Benchmarks
// ============================================================================

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
		on_iteration: () => globalThis.gc?.(),
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

// ============================================================================
// Baseline Handling
// ============================================================================

interface BaselineEntry {
	name: string;
	group: string;
	mean_ns: number;
	p50_ns: number;
	std_dev_ns: number;
	ops_per_second: number;
	sample_size: number;
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

/** Save current results as baseline */
async function saveBaseline(): Promise<void> {
	const entries: BaselineEntry[] = [];
	for (const group of allGroupResults) {
		for (const result of group.results) {
			entries.push({
				name: result.name,
				group: group.name,
				mean_ns: result.stats.mean_ns,
				p50_ns: result.stats.p50_ns,
				std_dev_ns: result.stats.std_dev_ns,
				ops_per_second: result.stats.ops_per_second,
				sample_size: result.stats.sample_size,
			});
		}
	}

	const baseline: Baseline = {
		version: 1,
		timestamp: new Date().toISOString(),
		git_commit: await getGitCommit(),
		corpus: {
			svelte: svelteFiles.length,
			typescript: tsFiles.length,
			css: cssFiles.length,
		},
		entries,
	};

	await Deno.writeTextFile(BASELINE_PATH, JSON.stringify(baseline, null, '\t'));
	log(`\nBaseline saved to ${BASELINE_PATH}`);
}

/** Load and compare against baseline */
async function compareBaseline(): Promise<void> {
	let baseline: Baseline;
	try {
		const content = await Deno.readTextFile(BASELINE_PATH);
		baseline = JSON.parse(content);
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
	const corpusMatch = baseline.corpus.svelte === svelteFiles.length &&
		baseline.corpus.typescript === tsFiles.length &&
		baseline.corpus.css === cssFiles.length;

	if (!corpusMatch) {
		log(`\n⚠️  Corpus size differs from baseline:`);
		log(
			`   Baseline: svelte=${baseline.corpus.svelte}, ts=${baseline.corpus.typescript}, css=${baseline.corpus.css}`,
		);
		log(`   Current:  svelte=${svelteFiles.length}, ts=${tsFiles.length}, css=${cssFiles.length}`);
	}

	// Build lookup map
	const baselineMap = new Map<string, BaselineEntry>();
	for (const entry of baseline.entries) {
		baselineMap.set(`${entry.group}/${entry.name}`, entry);
	}

	// Compare results
	const regressions: Array<{
		name: string;
		group: string;
		ratio: number;
		baseline: number;
		current: number;
	}> = [];
	const improvements: Array<{
		name: string;
		group: string;
		ratio: number;
		baseline: number;
		current: number;
	}> = [];

	for (const group of allGroupResults) {
		for (const result of group.results) {
			const key = `${group.name}/${result.name}`;
			const baselineEntry = baselineMap.get(key);
			if (!baselineEntry) continue;

			const ratio = result.stats.ops_per_second / baselineEntry.ops_per_second;

			if (ratio < 0.95) {
				// More than 5% slower
				regressions.push({
					name: result.name,
					group: group.name,
					ratio,
					baseline: baselineEntry.ops_per_second,
					current: result.stats.ops_per_second,
				});
			} else if (ratio > 1.05) {
				// More than 5% faster
				improvements.push({
					name: result.name,
					group: group.name,
					ratio,
					baseline: baselineEntry.ops_per_second,
					current: result.stats.ops_per_second,
				});
			}
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

// ============================================================================
// Output
// ============================================================================

if (args.json) {
	// JSON output
	const output = {
		timestamp: new Date().toISOString(),
		corpus: {
			svelte: svelteFiles.length,
			typescript: tsFiles.length,
			css: cssFiles.length,
		},
		groups: allGroupResults.map((g) => ({
			name: g.name,
			results: g.results.map((r) => ({
				name: r.name,
				stats: r.stats,
			})),
		})),
	};
	console.log(JSON.stringify(output, null, '\t'));
} else if (args.markdown) {
	// Markdown output using fuz_util's formatter
	console.log('# TSV Benchmark Results\n');
	console.log(`**Date:** ${new Date().toISOString()}\n`);
	console.log(
		`**Corpus:** ${svelteFiles.length} Svelte, ${tsFiles.length} TypeScript, ${cssFiles.length} CSS files\n`,
	);

	for (const group of allGroupResults) {
		if (group.results.length === 0) continue;
		console.log(`## ${group.name}\n`);
		console.log(benchmark_format_markdown(group.results));
		console.log('');
	}
} else {
	// Standard text output
	console.log(generateSummaryReport(allGroupResults, LANGUAGES));

	const altVersions = getAlternativeVersions(impls);
	console.log(
		generateCorpusInfo(
			{ svelte: svelteFiles.length, typescript: tsFiles.length, css: cssFiles.length },
			isLimited ? totalFileCounts : undefined,
			{
				svelte: VERSIONS.svelte,
				acorn: VERSIONS.acorn,
				acornTs: VERSIONS['@sveltejs/acorn-typescript'],
				prettier: VERSIONS.prettier,
				prettierSvelte: VERSIONS['prettier-plugin-svelte'],
				...altVersions,
			},
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

	console.log('\n' + '='.repeat(80));
}

// Handle baseline operations
if (args.saveBaseline) {
	await saveBaseline();
}

if (args.compareBaseline) {
	await compareBaseline();
}
