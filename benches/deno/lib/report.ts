/**
 * Summary report generation for benchmark results
 */

import type { BenchmarkResult } from '@fuzdev/fuz_util/benchmark_types.js';
import { time_format, time_unit_detect_best } from '@fuzdev/fuz_util/time.js';

import type { Language } from './types.ts';

/** Results from a benchmark group */
export interface GroupResults {
	name: string;
	results: BenchmarkResult[];
}

/** Create a visual bar for comparison (based on time - slower = longer bar) */
function createBar(value: number, max: number, width = 40): string {
	const filled = Math.round((value / max) * width);
	return '█'.repeat(filled) + '░'.repeat(width - filled);
}

/** Known canonical parser names by language */
const CANONICAL_PARSERS: Record<Language, string> = {
	svelte: 'svelte/compiler',
	typescript: 'acorn-typescript',
	css: 'svelte/compiler',
};

/** Known canonical formatter name */
const CANONICAL_FORMATTER = 'prettier';

/** Internal parse variants (for measuring JSON overhead) */
const INTERNAL_PARSE_VARIANTS = ['tsv-internal', 'tsv_wasm-internal'];

/**
 * Stable display order for implementations.
 * Order: canonical → tsv variants → third-party alternatives (alphabetical)
 */
const DISPLAY_ORDER = [
	// Canonical (shown separately, but included for completeness)
	'svelte/compiler',
	'acorn-typescript',
	'prettier',
	// TSV variants
	'tsv-json',
	'tsv_wasm-json',
	'tsv',
	'tsv_wasm',
	// Internal variants (shown separately)
	'tsv-internal',
	'tsv_wasm-internal',
	// Third-party alternatives (alphabetical)
	'biome-wasm',
	'oxc-parser',
	'oxc-parser-wasm',
	'oxfmt',
];

/** Sort results by stable display order */
function sortByDisplayOrder(results: BenchmarkResult[]): BenchmarkResult[] {
	return [...results].sort((a, b) => {
		const aIndex = DISPLAY_ORDER.indexOf(a.name);
		const bIndex = DISPLAY_ORDER.indexOf(b.name);
		// Unknown items go to the end
		const aOrder = aIndex === -1 ? DISPLAY_ORDER.length : aIndex;
		const bOrder = bIndex === -1 ? DISPLAY_ORDER.length : bIndex;
		return aOrder - bOrder;
	});
}

/** Generate the summary report */
export function generateSummaryReport(
	allGroupResults: GroupResults[],
	languages: Language[],
): string {
	const lines: string[] = [];

	lines.push('');
	lines.push('='.repeat(80));
	lines.push('BENCHMARK SUMMARY');
	lines.push('='.repeat(80));

	/** Get results for a specific group */
	function getGroupResults(name: string): BenchmarkResult[] {
		return allGroupResults.find((g) => g.name === name)?.results ?? [];
	}

	// Collect all times for consistent unit selection
	const allMeanTimes: number[] = [];
	for (const group of allGroupResults) {
		for (const result of group.results) {
			allMeanTimes.push(result.stats.mean_ns);
		}
	}
	const unit = time_unit_detect_best(allMeanTimes);
	const fmt = (ns: number) => time_format(ns, unit, 2);

	/** Format speedup/slowdown comparison string */
	function formatComparison(baseline: number, current: number): string {
		const ratio = baseline / current;
		if (ratio >= 1) {
			return `(${ratio.toFixed(1)}x faster)`;
		}
		return `(${(1 / ratio).toFixed(1)}x slower)`;
	}

	// Parse performance comparison
	lines.push('');
	lines.push('Parse Performance:');
	for (const lang of languages) {
		const results = getGroupResults(`parse/${lang}`);
		if (results.length === 0) continue;

		const canonicalName = CANONICAL_PARSERS[lang];
		const canonicalResult = results.find((r) => r.name === canonicalName);

		// Get main results (excluding internal variants)
		const mainResults = results.filter((r) => !INTERNAL_PARSE_VARIANTS.includes(r.name));
		// Get internal variants
		const internalResults = results.filter((r) => INTERNAL_PARSE_VARIANTS.includes(r.name));

		if (mainResults.length === 0) continue;

		// Calculate max time for bar scaling (main results only)
		const maxTime = Math.max(...mainResults.map((r) => r.stats.mean_ns));
		const baseline = canonicalResult?.stats.mean_ns ?? mainResults[0].stats.mean_ns;

		// Find the longest name for padding
		const maxNameLen = Math.max(...results.map((r) => r.name.length), 17);

		lines.push('');
		lines.push(`  ${lang}:`);

		// Show canonical first (baseline)
		if (canonicalResult) {
			lines.push(
				`    ${canonicalResult.name.padEnd(maxNameLen)} ${
					createBar(canonicalResult.stats.mean_ns, maxTime)
				} ${fmt(canonicalResult.stats.mean_ns)}`,
			);
		}

		// Show alternatives in stable display order (tsv variants, then third-party)
		const alternatives = sortByDisplayOrder(
			mainResults.filter((r) => r.name !== canonicalName),
		);

		for (const result of alternatives) {
			const comparison = formatComparison(baseline, result.stats.mean_ns);
			lines.push(
				`    ${result.name.padEnd(maxNameLen)} ${createBar(result.stats.mean_ns, maxTime)} ${
					fmt(result.stats.mean_ns)
				} ${comparison}`,
			);
		}

		// Show internal variants (JSON overhead measurement)
		for (const internalResult of sortByDisplayOrder(internalResults)) {
			// Find the corresponding JSON variant
			const jsonName = internalResult.name.includes('wasm') ? 'tsv_wasm-json' : 'tsv-json';
			const jsonResult = results.find((r) => r.name === jsonName);

			if (jsonResult) {
				const jsonOverhead = jsonResult.stats.mean_ns / internalResult.stats.mean_ns;
				lines.push(
					`    ${internalResult.name.padEnd(maxNameLen)} ${
						createBar(internalResult.stats.mean_ns, maxTime)
					} ${fmt(internalResult.stats.mean_ns)} (${jsonOverhead.toFixed(1)}x JSON overhead)`,
				);
			}
		}
	}

	// Format performance comparison
	lines.push('');
	lines.push('');
	lines.push('Format Performance:');
	for (const lang of languages) {
		const results = getGroupResults(`format/${lang}`);
		if (results.length === 0) continue;

		const canonicalResult = results.find((r) => r.name === CANONICAL_FORMATTER);
		if (!canonicalResult) continue;

		// Calculate max time for bar scaling
		const maxTime = Math.max(...results.map((r) => r.stats.mean_ns));
		const baseline = canonicalResult.stats.mean_ns;

		// Find the longest name for padding
		const maxNameLen = Math.max(...results.map((r) => r.name.length), 8);

		lines.push('');
		lines.push(`  ${lang}:`);

		// Show canonical first (baseline)
		lines.push(
			`    ${canonicalResult.name.padEnd(maxNameLen)} ${
				createBar(canonicalResult.stats.mean_ns, maxTime)
			} ${fmt(canonicalResult.stats.mean_ns)}`,
		);

		// Show alternatives in stable display order (tsv variants, then third-party)
		const alternatives = sortByDisplayOrder(
			results.filter((r) => r.name !== CANONICAL_FORMATTER),
		);

		for (const result of alternatives) {
			const comparison = formatComparison(baseline, result.stats.mean_ns);
			lines.push(
				`    ${result.name.padEnd(maxNameLen)} ${createBar(result.stats.mean_ns, maxTime)} ${
					fmt(result.stats.mean_ns)
				} ${comparison}`,
			);
		}
	}

	return lines.join('\n');
}

/** Generate skipped files report */
export function generateSkippedFilesReport(
	skippedFiles: Map<string, Map<string, string>>,
	maxErrorLength = 200,
): string | null {
	if (skippedFiles.size === 0) return null;

	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('SKIPPED FILES:');

	// Aggregate errors by file path and error message
	const fileErrorMap = new Map<string, Map<string, string[]>>();

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

	interface FileError {
		filePath: string;
		error: string;
		benchmarks: string[];
	}

	const allErrors: FileError[] = [];
	for (const [filePath, errorMap] of fileErrorMap) {
		for (const [error, benchmarks] of errorMap) {
			allErrors.push({ filePath, error, benchmarks });
		}
	}

	// Sort by number of affected benchmarks
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

	lines.push(`Total unique file+error combinations: ${sortedErrors.length}`);
	lines.push(`  Svelte:      ${skipsByLang.svelte} files skipped`);
	lines.push(`  TypeScript:  ${skipsByLang.typescript} files skipped`);
	lines.push(`  CSS:         ${skipsByLang.css} files skipped`);
	lines.push('');

	for (const { filePath, error, benchmarks } of sortedErrors.slice(0, 10)) {
		lines.push(filePath);
		const truncated = error.length > maxErrorLength;
		const displayError = truncated ? error.slice(0, maxErrorLength) + '...' : error;
		lines.push(`  Error: ${displayError}`);
		if (benchmarks.length === 1) {
			lines.push(`  Failed in: ${benchmarks[0]}`);
		} else {
			lines.push(`  Failed in ${benchmarks.length} benchmarks: ${benchmarks.join(', ')}`);
		}
		lines.push('');
	}

	if (sortedErrors.length > 10) {
		lines.push(`  ... and ${sortedErrors.length - 10} more`);
	}

	return lines.join('\n');
}

/** Format file count, showing "X of Y" when limited */
function formatFileCount(count: number, total?: number): string {
	if (total !== undefined && total !== count) {
		return `${count} of ${total} files`;
	}
	return `${count} files`;
}

/** Generate corpus info section */
export function generateCorpusInfo(
	fileCounts: { svelte: number; typescript: number; css: number },
	totalFileCounts: { svelte: number; typescript: number; css: number } | undefined,
	versions: {
		svelte: string;
		acorn: string;
		acornTs: string;
		prettier: string;
		prettierSvelte: string;
		oxcParser?: string;
		oxfmt?: string;
		biome?: string;
	},
): string {
	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('Corpus:');
	lines.push(`  Svelte:      ${formatFileCount(fileCounts.svelte, totalFileCounts?.svelte)}`);
	lines.push(
		`  TypeScript:  ${formatFileCount(fileCounts.typescript, totalFileCounts?.typescript)}`,
	);
	lines.push(`  CSS:         ${formatFileCount(fileCounts.css, totalFileCounts?.css)}`);

	lines.push('');
	lines.push('Versions:');
	lines.push(
		`  svelte@${versions.svelte}, acorn@${versions.acorn}, @sveltejs/acorn-typescript@${versions.acornTs}`,
	);
	lines.push(`  prettier@${versions.prettier}, prettier-plugin-svelte@${versions.prettierSvelte}`);

	const altVersions: string[] = [];
	if (versions.oxcParser) altVersions.push(`oxc-parser@${versions.oxcParser}`);
	if (versions.oxfmt) altVersions.push(`oxfmt@${versions.oxfmt}`);
	if (versions.biome) altVersions.push(`@biomejs/wasm-bundler@${versions.biome}`);

	if (altVersions.length > 0) {
		lines.push(`  ${altVersions.join(', ')}`);
	}

	return lines.join('\n');
}

/** A single comparison row (e.g., "format svelte: 13.6x prettier, 0.92x oxfmt") */
interface ComparisonRow {
	operation: 'format' | 'parse';
	language: Language;
	/** Comparisons to other implementations, e.g., [{name: "prettier", ratio: 13.6}] */
	comparisons: { name: string; ratio: number }[];
}

/** Comparison data for a section (native or wasm) */
interface ComparisonSection {
	label: string;
	rows: ComparisonRow[];
}

/** Format ratio as "Nx" (other_time / tsv_time) */
function formatRatio(r: number): string {
	return r >= 10 ? `${r.toFixed(1)}x` : `${r.toFixed(2)}x`;
}

/**
 * Build comparison data from benchmark results.
 * Extracts ratios for native and wasm sections.
 */
function buildComparisonData(
	allGroupResults: GroupResults[],
	languages: Language[],
): ComparisonSection[] {
	function getMeanNs(groupName: string, taskName: string): number | null {
		const group = allGroupResults.find((g) => g.name === groupName);
		if (!group) return null;
		const result = group.results.find((r) => r.name === taskName);
		return result?.stats.mean_ns ?? null;
	}

	function ratio(tsvNs: number, otherNs: number): number {
		return otherNs / tsvNs;
	}

	const sections: ComparisonSection[] = [];

	// Native comparisons
	const nativeRows: ComparisonRow[] = [];

	for (const lang of languages) {
		const tsvNs = getMeanNs(`format/${lang}`, 'tsv');
		const prettierNs = getMeanNs(`format/${lang}`, CANONICAL_FORMATTER);
		if (tsvNs === null || prettierNs === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'prettier', ratio: ratio(tsvNs, prettierNs) },
		];
		const oxfmtNs = getMeanNs(`format/${lang}`, 'oxfmt');
		if (oxfmtNs !== null) comparisons.push({ name: 'oxfmt', ratio: ratio(tsvNs, oxfmtNs) });

		nativeRows.push({ operation: 'format', language: lang, comparisons });
	}

	for (const lang of languages) {
		const tsvNs = getMeanNs(`parse/${lang}`, 'tsv-json');
		const canonicalParseName = CANONICAL_PARSERS[lang];
		const canonicalNs = getMeanNs(`parse/${lang}`, canonicalParseName);
		if (tsvNs === null || canonicalNs === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'svelte', ratio: ratio(tsvNs, canonicalNs) },
		];
		const oxcNs = getMeanNs(`parse/${lang}`, 'oxc-parser');
		if (oxcNs !== null) comparisons.push({ name: 'oxc-parser', ratio: ratio(tsvNs, oxcNs) });

		nativeRows.push({ operation: 'parse', language: lang, comparisons });
	}

	if (nativeRows.length > 0) {
		sections.push({ label: 'tsv (native)', rows: nativeRows });
	}

	// WASM comparisons
	const wasmRows: ComparisonRow[] = [];

	for (const lang of languages) {
		const tsvWasmNs = getMeanNs(`format/${lang}`, 'tsv_wasm');
		const prettierNs = getMeanNs(`format/${lang}`, CANONICAL_FORMATTER);
		if (tsvWasmNs === null || prettierNs === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'prettier', ratio: ratio(tsvWasmNs, prettierNs) },
		];
		const biomeNs = getMeanNs(`format/${lang}`, 'biome-wasm');
		if (biomeNs !== null) {
			comparisons.push({ name: 'biome-wasm', ratio: ratio(tsvWasmNs, biomeNs) });
		}

		wasmRows.push({ operation: 'format', language: lang, comparisons });
	}

	for (const lang of languages) {
		const tsvWasmNs = getMeanNs(`parse/${lang}`, 'tsv_wasm-json');
		const canonicalParseName = CANONICAL_PARSERS[lang];
		const canonicalNs = getMeanNs(`parse/${lang}`, canonicalParseName);
		if (tsvWasmNs === null || canonicalNs === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'svelte', ratio: ratio(tsvWasmNs, canonicalNs) },
		];
		const oxcWasmNs = getMeanNs(`parse/${lang}`, 'oxc-parser-wasm');
		if (oxcWasmNs !== null) {
			comparisons.push({ name: 'oxc-parser-wasm', ratio: ratio(tsvWasmNs, oxcWasmNs) });
		}

		wasmRows.push({ operation: 'parse', language: lang, comparisons });
	}

	if (wasmRows.length > 0) {
		sections.push({ label: 'tsv_wasm', rows: wasmRows });
	}

	return sections;
}

/**
 * Generate compact comparison summary (plain text).
 *
 * Ratios are other_time/tsv_time: >1 means tsv is faster.
 * Parse canonical is labeled "svelte" (wraps acorn-typescript for TS).
 */
export function generateComparisonSummary(
	allGroupResults: GroupResults[],
	languages: Language[],
): string {
	const sections = buildComparisonData(allGroupResults, languages);
	const lines: string[] = [];
	const labelWidth = 22;

	for (const section of sections) {
		lines.push('');
		lines.push('-'.repeat(80));
		lines.push(`COMPARISONS to ${section.label}:`);

		for (const row of section.rows) {
			const label = `  ${row.operation.padEnd(7)}${row.language}:`.padEnd(labelWidth);
			const ratios = row.comparisons
				.map((c) => `${formatRatio(c.ratio)} ${c.name}`)
				.join(', ');
			lines.push(label + ratios);
		}
	}

	// Fairness notes (only shown when oxc-parser data is present)
	const hasNativeOxc = sections.some((s) =>
		s.label === 'tsv (native)' &&
		s.rows.some((r) => r.comparisons.some((c) => c.name === 'oxc-parser'))
	);
	const hasWasmOxc = sections.some((s) =>
		s.label === 'tsv_wasm' &&
		s.rows.some((r) => r.comparisons.some((c) => c.name === 'oxc-parser-wasm'))
	);

	lines.push('');
	lines.push('  (parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts)');
	if (hasNativeOxc || hasWasmOxc) {
		lines.push(
			'  (oxc-parser returns a lazy proxy backed by raw buffer; tsv-json eagerly',
		);
		lines.push(
			'   materializes the full JS AST tree — comparison is not apples-to-apples)',
		);
	}
	lines.push('  (format groups include parse time — each formatter parses internally)');

	return lines.join('\n');
}

/**
 * Generate comparison summary as markdown table.
 */
export function generateComparisonMarkdown(
	allGroupResults: GroupResults[],
	languages: Language[],
): string | null {
	const sections = buildComparisonData(allGroupResults, languages);
	if (sections.length === 0) return null;

	const lines: string[] = [];

	for (const section of sections) {
		lines.push(`## Comparisons to ${section.label}\n`);
		lines.push('| Benchmark | Comparisons |');
		lines.push('| --- | --- |');

		for (const row of section.rows) {
			const label = `${row.operation} ${row.language}`;
			const ratios = row.comparisons
				.map((c) => `**${formatRatio(c.ratio)}** ${c.name}`)
				.join(', ');
			lines.push(`| ${label} | ${ratios} |`);
		}

		lines.push('');
	}

	// Fairness notes (only shown when oxc-parser data is present)
	const hasNativeOxc = sections.some((s) =>
		s.label === 'tsv (native)' &&
		s.rows.some((r) => r.comparisons.some((c) => c.name === 'oxc-parser'))
	);
	const hasWasmOxc = sections.some((s) =>
		s.label === 'tsv_wasm' &&
		s.rows.some((r) => r.comparisons.some((c) => c.name === 'oxc-parser-wasm'))
	);

	const notes: string[] = [
		'Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts',
	];
	if (hasNativeOxc || hasWasmOxc) {
		notes.push(
			'oxc-parser returns a lazy proxy backed by raw buffer; tsv-json eagerly materializes the full JS AST tree — comparison is not apples-to-apples',
		);
	}
	notes.push('Format groups include parse time — each formatter parses internally');

	lines.push('_' + notes.join('. ') + '_');

	return lines.join('\n');
}

/** Effective corpus size info for a benchmark */
export interface EffectiveCorpusEntry {
	processed: number;
	total: number;
}

/** Generate effective corpus report showing files actually processed per benchmark */
export function generateEffectiveCorpusReport(
	effectiveCorpusSize: Map<string, EffectiveCorpusEntry>,
): string | null {
	// Check if any benchmarks had skipped files
	let hasSkips = false;
	for (const { processed, total } of effectiveCorpusSize.values()) {
		if (processed < total) {
			hasSkips = true;
			break;
		}
	}

	if (!hasSkips) return null;

	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('EFFECTIVE CORPUS SIZE (files actually processed per iteration):');
	lines.push('');
	lines.push('⚠️  Some benchmarks processed fewer files due to errors.');
	lines.push('   Comparisons between implementations with different skip rates may be unfair.');
	lines.push('');

	// Group by operation/language
	const grouped = new Map<string, Map<string, EffectiveCorpusEntry>>();
	for (const [benchName, entry] of effectiveCorpusSize) {
		// benchName format: "parse/svelte/canonical" or "format/typescript/native"
		const parts = benchName.split('/');
		const groupKey = parts.slice(0, 2).join('/'); // "parse/svelte"
		const implName = parts[2] || 'unknown';

		if (!grouped.has(groupKey)) {
			grouped.set(groupKey, new Map());
		}
		grouped.get(groupKey)!.set(implName, entry);
	}

	for (const [groupName, impls] of grouped) {
		const entries = Array.from(impls.entries());
		const anySkips = entries.some(([, e]) => e.processed < e.total);
		if (!anySkips) continue;

		lines.push(`  ${groupName}:`);
		for (const [implName, entry] of entries) {
			const pct = ((entry.processed / entry.total) * 100).toFixed(0);
			const status = entry.processed === entry.total ? '✓' : '⚠';
			lines.push(
				`    ${status} ${implName.padEnd(15)} ${entry.processed}/${entry.total} files (${pct}%)`,
			);
		}
		lines.push('');
	}

	return lines.join('\n');
}
