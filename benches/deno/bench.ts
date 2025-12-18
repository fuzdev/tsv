/**
 * TSV Benchmark Suite
 *
 * Compares parsing and formatting performance across three implementations:
 * - Canonical: prettier + svelte/compiler (JavaScript baseline)
 * - Native: tsv via FFI (Rust, maximum performance)
 * - WASM: tsv compiled to WebAssembly (portable, near-native)
 *
 * Run with: deno bench --allow-ffi --allow-read --allow-env benches/deno/bench.ts
 */

import { groupByLanguage, loadCorpus } from './lib/corpus.ts';
import { NativeImplementation } from './lib/ffi.ts';
import { WasmImplementation } from './lib/wasm.ts';
import { CanonicalImplementation, VERSIONS } from './lib/canonical.ts';
import { chooseTimeUnit, createBar, formatTimeWithUnit } from './lib/format.ts';
import type { Language, SourceFile } from './lib/types.ts';

// ============================================================================
// Configuration
// ============================================================================

/** Parse optional integer from env var */
const envInt = (name: string): number | undefined => {
	const val = Deno.env.get(name);
	return val ? parseInt(val) : undefined;
};

/** Skip implementations that aren't built yet */
const SKIP_MISSING_IMPLEMENTATIONS = true;

/** Limit files per language (default: all) */
const MAX_FILES_PER_LANGUAGE = envInt('BENCH_LIMIT');

/** Filter files by path pattern (default: none) */
const FILE_FILTER = Deno.env.get('BENCH_FILTER');

/** Number of iterations per benchmark (default: Deno auto) */
const BENCH_ITERATIONS = envInt('BENCH_ITERATIONS');

/** Number of warmup iterations (default: Deno auto) */
const BENCH_WARMUP = envInt('BENCH_WARMUP');

/** Maximum length of error message to display (longer messages are truncated) */
const MAX_ERROR_MESSAGE_LENGTH = 200;

/** Languages to benchmark */
const LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

// ============================================================================
// Setup
// ============================================================================

console.log('Loading corpus...\n');
const { files } = await loadCorpus();
const byLanguage = groupByLanguage(files);

// Apply file filter and limit
function limitFiles(files: SourceFile[]): SourceFile[] {
	const filtered = FILE_FILTER ? files.filter((f) => f.path.includes(FILE_FILTER)) : files;
	return MAX_FILES_PER_LANGUAGE ? filtered.slice(0, MAX_FILES_PER_LANGUAGE) : filtered;
}

const svelteFiles = limitFiles(byLanguage.svelte);
const tsFiles = limitFiles(byLanguage.typescript);
const cssFiles = limitFiles(byLanguage.css);

console.log(`\nBenchmarking with:`);
console.log(`  Svelte: ${svelteFiles.length} files`);
console.log(`  TypeScript: ${tsFiles.length} files`);
console.log(`  CSS: ${cssFiles.length} files`);
console.log();

// Initialize implementations
const canonical = new CanonicalImplementation();
const native = new NativeImplementation();
const wasm = new WasmImplementation();

/**
 * Initialize an implementation, logging status
 * @param impl Implementation to initialize
 * @param name Display name
 * @param required Whether initialization failure should throw
 * @returns Whether initialization succeeded
 */
async function initImplementation(
	impl: { init(): Promise<void> },
	name: string,
	required: boolean,
): Promise<boolean> {
	try {
		await impl.init();
		console.log(`  ✓ ${name}`);
		return true;
	} catch (e) {
		if (required) {
			console.error(`  ✗ ${name}: ${e}`);
			throw e;
		} else if (SKIP_MISSING_IMPLEMENTATIONS) {
			console.warn(`  ⚠ ${name}: ${e}`);
			return false;
		} else {
			throw e;
		}
	}
}

console.log('Initializing implementations...');

await initImplementation(canonical, 'Canonical (prettier + svelte/compiler)', true);
const nativeAvailable = await initImplementation(native, 'Native (FFI)', false);
const wasmAvailable = await initImplementation(wasm, 'WASM', false);

console.log();

// ============================================================================
// Benchmark Helpers
// ============================================================================

/** Build Deno.bench options with optional iteration/warmup config */
function benchOptions(opts: Deno.BenchDefinition): Deno.BenchDefinition {
	return {
		...opts,
		...(BENCH_ITERATIONS !== undefined && { n: BENCH_ITERATIONS }),
		...(BENCH_WARMUP !== undefined && { warmup: BENCH_WARMUP }),
	};
}

// Track skipped files for reporting
const skippedFiles: Map<string, Map<string, string>> = new Map();

function recordSkip(benchName: string, filePath: string, error: unknown): void {
	if (!skippedFiles.has(benchName)) {
		skippedFiles.set(benchName, new Map());
	}
	const errorMsg = error instanceof Error ? error.message : String(error);
	skippedFiles.get(benchName)!.set(filePath, errorMsg);
}

/** Generic benchmark runner - processes all files with the given function */
function runBenchmark(
	files: SourceFile[],
	processFn: (file: SourceFile) => void,
	benchName: string,
): void {
	const start = performance.now();
	for (const file of files) {
		try {
			processFn(file);
		} catch (e) {
			recordSkip(benchName, file.path, e);
		}
	}
	recordTiming(benchName, performance.now() - start);
}

/** Async benchmark runner - for implementations that return promises */
async function runBenchmarkAsync(
	files: SourceFile[],
	processFn: (file: SourceFile) => Promise<void>,
	benchName: string,
): Promise<void> {
	const start = performance.now();
	for (const file of files) {
		try {
			await processFn(file);
		} catch (e) {
			recordSkip(benchName, file.path, e);
		}
	}
	recordTiming(benchName, performance.now() - start);
}

/** Construct benchmark name */
function benchmarkName(
	operation: 'parse' | 'format',
	language: Language,
	impl: 'canonical' | 'native' | 'wasm',
): string {
	return `${operation}/${language}/${impl}`;
}

/** Register parse benchmarks for a language */
function registerParseBenchmarks(
	files: SourceFile[],
	language: Language,
	implementations: { canonical?: typeof canonical; native?: typeof native; wasm?: typeof wasm },
): void {
	if (files.length === 0) return;

	const group = `parse-${language}`;
	const { canonical, native, wasm } = implementations;
	let hasBaseline = false;

	// Canonical (baseline) - if available
	if (canonical) {
		const name = benchmarkName('parse', language, 'canonical');
		Deno.bench(benchOptions({
			name,
			group,
			baseline: true,
			fn() {
				runBenchmark(files, (f) => canonical.parse(f.content, language), name);
			},
		}));
		hasBaseline = true;
	}

	// Native (baseline if no canonical)
	if (native) {
		const isBaseline = !hasBaseline;
		const name = benchmarkName('parse', language, 'native');

		Deno.bench(benchOptions({
			name,
			group,
			baseline: isBaseline,
			fn() {
				runBenchmark(files, (f) => native.parse(f.content, language), name);
			},
		}));
	}

	// WASM
	if (wasm) {
		const name = benchmarkName('parse', language, 'wasm');
		Deno.bench(benchOptions({
			name,
			group,
			fn() {
				runBenchmark(files, (f) => wasm.parse(f.content, language), name);
			},
		}));
	}
}

/** Register internal parse benchmarks (pure parsing, no conversion/serialization) */
function registerParseInternalBenchmarks(
	files: SourceFile[],
	language: Language,
	implementations: { native?: typeof native; wasm?: typeof wasm },
): void {
	if (files.length === 0) return;

	const { native, wasm } = implementations;
	if (!native && !wasm) return;

	const group = `parse-internal-${language}`;

	// Native (baseline)
	if (native) {
		const name = `parse-internal/${language}/native`;
		Deno.bench(benchOptions({
			name,
			group,
			baseline: true,
			fn() {
				runBenchmark(files, (f) => native.parseInternal(f.content, language), name);
			},
		}));
	}

	// WASM
	if (wasm) {
		const name = `parse-internal/${language}/wasm`;
		Deno.bench(benchOptions({
			name,
			group,
			fn() {
				runBenchmark(files, (f) => wasm.parseInternal(f.content, language), name);
			},
		}));
	}
}

/** Register format benchmarks for a language */
function registerFormatBenchmarks(
	files: SourceFile[],
	language: Language,
	implementations: { canonical?: typeof canonical; native?: typeof native; wasm?: typeof wasm },
): void {
	if (files.length === 0) return;

	const { canonical, native, wasm } = implementations;
	if (!canonical) return; // Canonical is required for format benchmarks (baseline)

	const group = `format-${language}`;

	// Canonical (baseline)
	{
		const name = benchmarkName('format', language, 'canonical');
		Deno.bench(benchOptions({
			name,
			group,
			baseline: true,
			async fn() {
				await runBenchmarkAsync(
					files,
					async (f) => {
						await canonical.formatAsync(f.content, language);
					},
					name,
				);
			},
		}));
	}

	// Native
	if (native) {
		const name = benchmarkName('format', language, 'native');
		Deno.bench(benchOptions({
			name,
			group,
			fn() {
				runBenchmark(files, (f) => native.format(f.content, language), name);
			},
		}));
	}

	// WASM
	if (wasm) {
		const name = benchmarkName('format', language, 'wasm');
		Deno.bench(benchOptions({
			name,
			group,
			fn() {
				runBenchmark(files, (f) => wasm.format(f.content, language), name);
			},
		}));
	}
}

/** Collect actual benchmark timing data */
interface BenchmarkTiming {
	totalTime: number;
	count: number;
}

/** Aggregated error info for skip reporting */
interface FileError {
	filePath: string;
	error: string;
	benchmarks: string[];
}

const benchmarkTimings = new Map<string, BenchmarkTiming>();

function recordTiming(benchKey: string, timeMs: number): void {
	const existing = benchmarkTimings.get(benchKey);
	if (existing) {
		existing.totalTime += timeMs;
		existing.count++;
	} else {
		benchmarkTimings.set(benchKey, { totalTime: timeMs, count: 1 });
	}
}

function getAverageTiming(benchKey: string): number | undefined {
	const timing = benchmarkTimings.get(benchKey);
	return timing ? timing.totalTime / timing.count : undefined;
}

/** Collect all timing values from benchmarks */
function collectAllTimings(): number[] {
	const times: number[] = [];
	for (const lang of LANGUAGES) {
		const keys = [
			`parse/${lang}/canonical`,
			`parse/${lang}/native`,
			`parse/${lang}/wasm`,
			`parse-internal/${lang}/native`,
			`parse-internal/${lang}/wasm`,
			`format/${lang}/canonical`,
			`format/${lang}/native`,
			`format/${lang}/wasm`,
		];
		for (const key of keys) {
			const t = getAverageTiming(key);
			if (t !== undefined) times.push(t);
		}
	}
	return times;
}

/** Get canonical parser label for a language */
function canonicalParserLabel(lang: Language): string {
	switch (lang) {
		case 'svelte':
			return 'svelte/compiler';
		case 'typescript':
			return 'acorn-ts';
		case 'css':
			return 'svelte/compiler';
	}
}

/** Report summary statistics after benchmarks */
function reportSummary(): void {
	console.log('=== Benchmark Summary ===\n');

	// Determine time unit based on smallest timing (so all values >= 1 in chosen unit)
	const allTimes = collectAllTimings();
	const minTime = allTimes.length > 0 ? Math.min(...allTimes) : 0;
	const unit = chooseTimeUnit(minTime);
	const fmt = (ms: number) => formatTimeWithUnit(ms, unit);

	// Parse performance comparison (includes internal timings to show JSON overhead)
	console.log('Parse Performance:');
	for (const lang of LANGUAGES) {
		const canonicalTime = getAverageTiming(`parse/${lang}/canonical`);
		const nativeTime = getAverageTiming(`parse/${lang}/native`);
		const nativeInternalTime = getAverageTiming(`parse-internal/${lang}/native`);
		const wasmTime = getAverageTiming(`parse/${lang}/wasm`);
		const wasmInternalTime = getAverageTiming(`parse-internal/${lang}/wasm`);

		// Need at least native time to show anything
		if (!nativeTime) continue;

		const times = [canonicalTime, nativeTime, wasmTime].filter((t): t is number => t !== undefined);
		const langMaxTime = Math.max(...times);
		const canonicalLabel = canonicalParserLabel(lang);
		const baseline = canonicalTime || nativeTime;

		console.log(`\n  ${lang}:`);

		// Canonical (if available) - no comparison, it's the baseline
		if (canonicalTime) {
			console.log(
				`    ${canonicalLabel.padEnd(17)} ${createBar(canonicalTime, langMaxTime)} ${
					fmt(canonicalTime)
				}`,
			);
		}

		// tsv wasm (with JSON serialization) - compare to canonical or native
		if (wasmTime) {
			const ratio = baseline / wasmTime;
			let comparison: string;
			if (canonicalTime) {
				comparison = ratio >= 1
					? `(${ratio.toFixed(1)}x faster)`
					: `(${(1 / ratio).toFixed(1)}x slower)`;
			} else {
				const nativeRatio = nativeTime / wasmTime;
				comparison = nativeRatio >= 1
					? `(${nativeRatio.toFixed(1)}x vs tsv-json)`
					: `(${(1 / nativeRatio).toFixed(1)}x slower than tsv-json)`;
			}
			console.log(
				`    ${'tsv-wasm-json'.padEnd(17)} ${createBar(wasmTime, langMaxTime)} ${
					fmt(wasmTime)
				} ${comparison}`,
			);
		}

		// tsv native (with JSON serialization) - compare to canonical
		if (canonicalTime) {
			const ratio = canonicalTime / nativeTime;
			const comparison = ratio >= 1
				? `(${ratio.toFixed(1)}x faster)`
				: `(${(1 / ratio).toFixed(1)}x slower)`;
			console.log(
				`    ${'tsv-json'.padEnd(17)} ${createBar(nativeTime, langMaxTime)} ${fmt(nativeTime)} ${comparison}`,
			);
		} else {
			console.log(
				`    ${'tsv-json'.padEnd(17)} ${createBar(nativeTime, langMaxTime)} ${fmt(nativeTime)}`,
			);
		}

		// tsv wasm internal (pure parse, no JSON)
		if (wasmInternalTime && wasmTime) {
			const jsonOverhead = wasmTime / wasmInternalTime;
			console.log(
				`    ${'tsv-wasm-internal'.padEnd(17)} ${createBar(wasmInternalTime, langMaxTime)} ${
					fmt(wasmInternalTime)
				} (${jsonOverhead.toFixed(1)}x JSON overhead)`,
			);
		}

		// tsv native internal (pure parse, no JSON)
		if (nativeInternalTime) {
			const jsonOverhead = nativeTime / nativeInternalTime;
			console.log(
				`    ${'tsv-internal'.padEnd(17)} ${createBar(nativeInternalTime, langMaxTime)} ${
					fmt(nativeInternalTime)
				} (${jsonOverhead.toFixed(1)}x JSON overhead)`,
			);
		}
	}

	// Format performance comparison
	console.log('\n\nFormat Performance:');
	for (const lang of LANGUAGES) {
		const canonicalTime = getAverageTiming(`format/${lang}/canonical`);
		const nativeTime = getAverageTiming(`format/${lang}/native`);
		const wasmTime = getAverageTiming(`format/${lang}/wasm`);

		if (canonicalTime && nativeTime && nativeTime > 0) {
			const langMaxTime = Math.max(canonicalTime, nativeTime, wasmTime || 0);
			const nativeSpeedup = canonicalTime / nativeTime;
			const wasmSpeedup = (wasmTime && wasmTime > 0) ? canonicalTime / wasmTime : 0;

			console.log(`\n  ${lang}:`);
			console.log(
				`    ${'prettier'.padEnd(8)} ${createBar(canonicalTime, langMaxTime)} ${
					fmt(canonicalTime)
				}`,
			);
			console.log(
				`    ${'tsv'.padEnd(8)} ${createBar(nativeTime, langMaxTime)} ${fmt(nativeTime)} (${
					nativeSpeedup.toFixed(1)
				}x faster)`,
			);
			if (wasmTime && wasmSpeedup > 0) {
				console.log(
					`    ${'tsv-wasm'.padEnd(8)} ${createBar(wasmTime, langMaxTime)} ${fmt(wasmTime)} (${
						wasmSpeedup.toFixed(1)
					}x faster)`,
				);
			}
		}
	}

	// File counts by language
	console.log('\n\nCorpus:');
	console.log(`  Svelte:      ${svelteFiles.length} files`);
	console.log(`  TypeScript:  ${tsFiles.length} files`);
	console.log(`  CSS:         ${cssFiles.length} files`);

	// Canonical implementation versions
	console.log('\n\nCanonical Implementations:');
	console.log(`  svelte/compiler:            ${VERSIONS.svelte}`);
	console.log(
		`  acorn + acorn-typescript:   ${VERSIONS.acorn} + ${VERSIONS['@sveltejs/acorn-typescript']}`,
	);
	console.log(`  prettier:                   ${VERSIONS.prettier}`);
	console.log(`  prettier-plugin-svelte:     ${VERSIONS['prettier-plugin-svelte']}`);
}

/** Report skipped files after benchmarks complete */
function reportSkippedFiles(): void {
	if (skippedFiles.size === 0) return;

	// Aggregate errors by file path and error message (nested maps avoid delimiter issues)
	// Structure: filePath -> error -> benchmarks[]
	const fileErrorMap = new Map<string, Map<string, string[]>>();

	// Collect all errors, grouping by file path and error message
	for (const [benchName, filesMap] of skippedFiles) {
		for (const [filePath, error] of filesMap) {
			if (!fileErrorMap.has(filePath)) {
				fileErrorMap.set(filePath, new Map());
			}
			const errorMap = fileErrorMap.get(filePath)!;
			if (!errorMap.has(error)) {
				errorMap.set(error, []);
			}
			errorMap.get(error)!.push(benchName);
		}
	}

	// Flatten to array for sorting
	const allErrors: FileError[] = [];
	for (const [filePath, errorMap] of fileErrorMap) {
		for (const [error, benchmarks] of errorMap) {
			allErrors.push({ filePath, error, benchmarks });
		}
	}

	// Sort by number of affected benchmarks (most problematic first), then by file path
	const sortedErrors = allErrors.sort((a, b) => {
		const benchDiff = b.benchmarks.length - a.benchmarks.length;
		return benchDiff !== 0 ? benchDiff : a.filePath.localeCompare(b.filePath);
	});

	// Count skips by language
	const skipsByLang = { svelte: 0, typescript: 0, css: 0 };
	for (const { filePath } of sortedErrors) {
		if (filePath.endsWith('.svelte')) skipsByLang.svelte++;
		else if (filePath.endsWith('.ts') || filePath.endsWith('.js')) skipsByLang.typescript++;
		else if (filePath.endsWith('.css')) skipsByLang.css++;
	}

	console.log('\n--- Skipped Files ---');
	console.log(`Total unique file+error combinations: ${sortedErrors.length}`);
	console.log(`  Svelte:      ${skipsByLang.svelte} files skipped`);
	console.log(`  TypeScript:  ${skipsByLang.typescript} files skipped`);
	console.log(`  CSS:         ${skipsByLang.css} files skipped\n`);

	for (const { filePath, error, benchmarks } of sortedErrors) {
		console.log(filePath);
		const truncated = error.length > MAX_ERROR_MESSAGE_LENGTH;
		const displayError = truncated ? error.slice(0, MAX_ERROR_MESSAGE_LENGTH) + '...' : error;
		console.log(`  Error: ${displayError}`);
		if (benchmarks.length === 1) {
			console.log(`  Failed in: ${benchmarks[0]}`);
		} else {
			console.log(`  Failed in ${benchmarks.length} benchmarks: ${benchmarks.join(', ')}`);
		}
		console.log();
	}
}

// ============================================================================
// Register Benchmarks
// ============================================================================

// Prepare implementation sets
const allImplementations = {
	canonical,
	...(nativeAvailable && { native }),
	...(wasmAvailable && { wasm }),
};

const nativeWasmImplementations = {
	...(nativeAvailable && { native }),
	...(wasmAvailable && { wasm }),
};

// Register benchmarks in logical groups for clearer output

// Parse benchmarks (with conversion + JSON serialization)
registerParseBenchmarks(svelteFiles, 'svelte', allImplementations);
registerParseBenchmarks(tsFiles, 'typescript', allImplementations);
registerParseBenchmarks(cssFiles, 'css', allImplementations);

// Internal parse benchmarks (pure parsing, no conversion)
registerParseInternalBenchmarks(svelteFiles, 'svelte', nativeWasmImplementations);
registerParseInternalBenchmarks(tsFiles, 'typescript', nativeWasmImplementations);
registerParseInternalBenchmarks(cssFiles, 'css', nativeWasmImplementations);

// Format benchmarks
registerFormatBenchmarks(svelteFiles, 'svelte', allImplementations);
registerFormatBenchmarks(tsFiles, 'typescript', allImplementations);
registerFormatBenchmarks(cssFiles, 'css', allImplementations);

// ============================================================================
// Cleanup & Reporting
// ============================================================================

// Report summary and skipped files after all benchmarks complete
// Note: Using 'beforeunload' event (browser API supported by Deno) for reporting.
// Limitation: May not fire reliably on SIGINT or forced termination, but will
// work for normal benchmark completion. Deno.bench doesn't provide a cleanup hook.
globalThis.addEventListener('beforeunload', () => {
	console.log(); // Add spacing after benchmark tables
	reportSummary();
	reportSkippedFiles();
});

// Process will exit after benchmarks complete, releasing all resources.
