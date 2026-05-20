/**
 * Summary report generation for benchmark results
 */

import type { BenchmarkResult } from '@fuzdev/fuz_util/benchmark_types.js';
import { benchmark_format_number } from '@fuzdev/fuz_util/benchmark_format.js';
import { time_format, time_unit_detect_best, TIME_UNIT_DISPLAY } from '@fuzdev/fuz_util/time.js';

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
	lines.push('BENCHMARK SUMMARY  (every `Nx` is speedup form — >1 means faster than baseline)');
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

	/**
	 * Speedup-form comparison: `baseline_ns / current_ns` — values > 1 mean
	 * `current` is faster, < 1 mean slower. Single convention so the reader
	 * doesn't context-switch between "Nx faster" and "Nx slower" framings.
	 */
	function formatComparison(baseline: number, current: number): string {
		const ratio = baseline / current;
		return `(${ratio.toFixed(2)}x)`;
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

/**
 * Skipped files terminal report. Always shows totals + per-benchmark
 * counts (signal). Per-file detail (paths + errors + failure sets) is
 * opt-in via `verbose` since for typical use it's mostly unsupported-syntax
 * fixtures, not actionable bugs.
 */
export function generateSkippedFilesReport(
	skippedFiles: Map<string, Map<string, string>>,
	maxErrorLength = 200,
	verbose = false,
	taskTrackingByGroup?: Map<string, Map<string, string>>,
): string | null {
	if (skippedFiles.size === 0) return null;

	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('SKIPPED FILES:');

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
		lang: SkipLang;
	}

	function classifyLang(path: string): SkipLang {
		if (path.endsWith('.svelte') || path.endsWith('.html')) return 'svelte';
		if (path.endsWith('.ts') || path.endsWith('.js')) return 'typescript';
		if (path.endsWith('.css')) return 'css';
		return 'other';
	}

	const allErrors: FileError[] = [];
	for (const [filePath, errorMap] of fileErrorMap) {
		const lang = classifyLang(filePath);
		for (const [error, benchmarks] of errorMap) {
			allErrors.push({ filePath, error, benchmarks, lang });
		}
	}
	// Ascending by failure-set size — rare/impl-specific failures first.
	const sortedErrors = allErrors.sort((a, b) => {
		const benchDiff = a.benchmarks.length - b.benchmarks.length;
		return benchDiff !== 0 ? benchDiff : a.filePath.localeCompare(b.filePath);
	});

	const skipsByLang = { svelte: 0, typescript: 0, css: 0 };
	for (const { lang } of sortedErrors) {
		if (lang !== 'other') skipsByLang[lang]++;
	}

	lines.push(`Total unique file+error combinations: ${sortedErrors.length}`);
	lines.push(`  Svelte:      ${skipsByLang.svelte} files skipped`);
	lines.push(`  TypeScript:  ${skipsByLang.typescript} files skipped`);
	lines.push(`  CSS:         ${skipsByLang.css} files skipped`);

	// Per-benchmark skip counts (always shown). Display names instead of
	// trackingKeys so the labels match the bench tables.
	const perBench: { name: string; skips: number }[] = [];
	for (const [benchName, filesMap] of skippedFiles) {
		perBench.push({ name: benchName, skips: filesMap.size });
	}
	perBench.sort((a, b) => b.skips - a.skips);
	if (perBench.length > 0) {
		lines.push('');
		lines.push('Per-benchmark skip counts:');
		for (const { name, skips } of perBench) {
			lines.push(`  ${trackingKeyDisplay(name, taskTrackingByGroup)}: ${skips}`);
		}
	}

	if (!verbose) {
		lines.push('');
		lines.push('(Per-file detail omitted. Re-run with `--verbose` for paths + errors.)');
		return lines.join('\n');
	}

	lines.push('');
	for (const { filePath, error, benchmarks, lang } of sortedErrors.slice(0, 10)) {
		lines.push(filePath);
		const truncated = error.length > maxErrorLength;
		const displayError = truncated ? error.slice(0, maxErrorLength) + '...' : error;
		lines.push(`  Error: ${displayError}`);
		const failedIn = isUniversalTsvFailure(lang, benchmarks)
			? 'all tsv variants'
			: benchmarks.map((b) => trackingKeyDisplay(b, taskTrackingByGroup)).join(', ');
		const prefix = benchmarks.length === 1
			? 'Failed in'
			: `Failed in ${benchmarks.length} benchmarks`;
		lines.push(`  ${prefix}: ${failedIn}`);
		lines.push('');
	}

	if (sortedErrors.length > 10) {
		lines.push(`  ... and ${sortedErrors.length - 10} more (sorted rarest failure-set first)`);
	}

	return lines.join('\n');
}

/**
 * Versions block for the terminal run. (Corpus counts already print at the
 * top of the run, so this used to duplicate them — now versions only.)
 */
export function generateVersionsInfo(versions: {
	svelte: string;
	acorn: string;
	acornTs: string;
	prettier: string;
	prettierSvelte: string;
	oxcParser?: string;
	oxfmt?: string;
	biome?: string;
}): string {
	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
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

/** A single comparison row (e.g., "format svelte: 13.6x prettier (240f), 0.92x oxfmt (240f)") */
interface ComparisonRow {
	operation: 'format' | 'parse';
	language: Language;
	/** Iterated file count for the self impl in this group (intersection size in default mode). */
	files: number | undefined;
	/** Comparisons to other implementations, e.g., [{name: "prettier", ratio: 13.6}] */
	comparisons: { name: string; ratio: number }[];
}

/** Comparison data for a section (native or wasm) */
interface ComparisonSection {
	label: string;
	rows: ComparisonRow[];
}

/** Resolve the iterated file count for a (group, displayName) pair via taskTracking. */
function lookupIterated(
	groupName: string,
	displayName: string,
	iteratedCounts: Map<string, number> | undefined,
	taskTrackingByGroup: Map<string, Map<string, string>> | undefined,
): number | undefined {
	if (!iteratedCounts || !taskTrackingByGroup) return undefined;
	const trackingKey = taskTrackingByGroup.get(groupName)?.get(displayName);
	if (!trackingKey) return undefined;
	return iteratedCounts.get(trackingKey);
}

/** Format ratio as "Nx" (other_time / tsv_time) */
function formatRatio(r: number): string {
	return r >= 10 ? `${r.toFixed(1)}x` : `${r.toFixed(2)}x`;
}

/**
 * Per-group benchmark table in speedup-form markdown. Mirrors the column
 * layout of `benchmark_format_markdown` (Task Name, ops/sec, percentiles,
 * min/max, vs baseline) but inverts the ratio: cells show
 * `r.ops_per_second / baseline.ops_per_second`, so `2.5x` means "this row is
 * 2.5× faster than baseline." The iterated file count is rendered as a
 * group-level annotation (see `generateGroupFilesMarkdown`) rather than per
 * cell — same value across all rows in default intersection mode, so the
 * repetition was pure noise.
 */
export function generateGroupBenchTableMarkdown(
	results: BenchmarkResult[],
	baseline: string | undefined,
): string {
	if (results.length === 0) return '(no results)';

	const meanTimes = results.map((r) => r.stats.mean_ns);
	const unit = time_unit_detect_best(meanTimes);
	const unitStr = TIME_UNIT_DISPLAY[unit];

	let baselineOps: number;
	let vsHeader: string;
	if (baseline !== undefined && results.some((r) => r.name === baseline)) {
		baselineOps = results.find((r) => r.name === baseline)!.stats.ops_per_second;
		vsHeader = `vs ${baseline} (speedup)`;
	} else {
		baselineOps = Math.max(...results.map((r) => r.stats.ops_per_second));
		vsHeader = 'vs Best (speedup)';
	}

	const rows: string[][] = [];
	rows.push([
		'Task Name',
		'ops/sec',
		'n',
		`p50 (${unitStr})`,
		`p75 (${unitStr})`,
		`p90 (${unitStr})`,
		`p95 (${unitStr})`,
		`p99 (${unitStr})`,
		`min (${unitStr})`,
		`max (${unitStr})`,
		vsHeader,
	]);

	for (const r of results) {
		const fmt = (ns: number) => time_format(ns, unit, 2).replace(unitStr, '').trim();
		const isBaseline = r.stats.ops_per_second === baselineOps;
		const speedup = r.stats.ops_per_second / baselineOps;
		const vsCell = isBaseline ? 'baseline' : formatRatio(speedup);
		// p95/p99 from <10 samples is essentially `max` (R-7 interpolation
		// collapses to the last sorted index). Render `—` so readers don't
		// misread interpolated noise as tail-latency data.
		const tailCell = (ns: number) => (r.stats.sample_size < 10 ? '—' : fmt(ns));
		rows.push([
			r.name,
			benchmark_format_number(r.stats.ops_per_second, 2),
			String(r.stats.sample_size),
			fmt(r.stats.p50_ns),
			fmt(r.stats.p75_ns),
			fmt(r.stats.p90_ns),
			tailCell(r.stats.p95_ns),
			tailCell(r.stats.p99_ns),
			fmt(r.stats.min_ns),
			fmt(r.stats.max_ns),
			vsCell,
		]);
	}

	const widths = rows[0].map((_, i) => Math.max(...rows.map((row) => row[i].length)));
	const lines: string[] = [];
	const renderRow = (row: string[]) =>
		'| ' + row.map((c, i) => c.padEnd(widths[i])).join(' | ') + ' |';
	lines.push(renderRow(rows[0]));
	lines.push('| ' + widths.map((w) => '-'.repeat(w)).join(' | ') + ' |');
	for (let i = 1; i < rows.length; i++) {
		lines.push(renderRow(rows[i]));
	}
	return lines.join('\n');
}

/**
 * Build comparison data from benchmark results.
 *
 * Ratios are computed from timed ops/sec — in default `intersection` mode the
 * comparison is apples-to-apples within each group (every impl ran on the
 * same files). The `(Mf)` annotation is the self impl's iterated file count
 * for that group (the per-group intersection size in default mode; the
 * impl's preflight success set size in `BENCH_MODE=union`).
 */
function buildComparisonData(
	allGroupResults: GroupResults[],
	languages: Language[],
	iteratedCounts: Map<string, number> | undefined,
	taskTrackingByGroup: Map<string, Map<string, string>> | undefined,
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
		const groupName = `format/${lang}`;
		const tsvNs = getMeanNs(groupName, 'tsv');
		const prettierNs = getMeanNs(groupName, CANONICAL_FORMATTER);
		if (tsvNs === null || prettierNs === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'prettier', ratio: ratio(tsvNs, prettierNs) },
		];
		const oxfmtNs = getMeanNs(groupName, 'oxfmt');
		if (oxfmtNs !== null) comparisons.push({ name: 'oxfmt', ratio: ratio(tsvNs, oxfmtNs) });

		nativeRows.push({
			operation: 'format',
			language: lang,
			files: lookupIterated(groupName, 'tsv', iteratedCounts, taskTrackingByGroup),
			comparisons,
		});
	}

	for (const lang of languages) {
		const groupName = `parse/${lang}`;
		const tsvNs = getMeanNs(groupName, 'tsv-json');
		const canonicalParseName = CANONICAL_PARSERS[lang];
		const canonicalNs = getMeanNs(groupName, canonicalParseName);
		if (tsvNs === null || canonicalNs === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'svelte', ratio: ratio(tsvNs, canonicalNs) },
		];
		const oxcNs = getMeanNs(groupName, 'oxc-parser');
		if (oxcNs !== null) comparisons.push({ name: 'oxc-parser', ratio: ratio(tsvNs, oxcNs) });

		nativeRows.push({
			operation: 'parse',
			language: lang,
			files: lookupIterated(groupName, 'tsv-json', iteratedCounts, taskTrackingByGroup),
			comparisons,
		});
	}

	if (nativeRows.length > 0) {
		sections.push({ label: 'tsv (native)', rows: nativeRows });
	}

	// WASM comparisons
	const wasmRows: ComparisonRow[] = [];

	for (const lang of languages) {
		const groupName = `format/${lang}`;
		const tsvWasmNs = getMeanNs(groupName, 'tsv_wasm');
		const prettierNs = getMeanNs(groupName, CANONICAL_FORMATTER);
		if (tsvWasmNs === null || prettierNs === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'prettier', ratio: ratio(tsvWasmNs, prettierNs) },
		];
		const biomeNs = getMeanNs(groupName, 'biome-wasm');
		if (biomeNs !== null) {
			comparisons.push({ name: 'biome-wasm', ratio: ratio(tsvWasmNs, biomeNs) });
		}

		wasmRows.push({
			operation: 'format',
			language: lang,
			files: lookupIterated(groupName, 'tsv_wasm', iteratedCounts, taskTrackingByGroup),
			comparisons,
		});
	}

	for (const lang of languages) {
		const groupName = `parse/${lang}`;
		const tsvWasmNs = getMeanNs(groupName, 'tsv_wasm-json');
		const canonicalParseName = CANONICAL_PARSERS[lang];
		const canonicalNs = getMeanNs(groupName, canonicalParseName);
		if (tsvWasmNs === null || canonicalNs === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'svelte', ratio: ratio(tsvWasmNs, canonicalNs) },
		];
		const oxcWasmNs = getMeanNs(groupName, 'oxc-parser-wasm');
		if (oxcWasmNs !== null) {
			comparisons.push({ name: 'oxc-parser-wasm', ratio: ratio(tsvWasmNs, oxcWasmNs) });
		}

		wasmRows.push({
			operation: 'parse',
			language: lang,
			files: lookupIterated(groupName, 'tsv_wasm-json', iteratedCounts, taskTrackingByGroup),
			comparisons,
		});
	}

	if (wasmRows.length > 0) {
		sections.push({ label: 'tsv_wasm', rows: wasmRows });
	}

	return sections;
}

/**
 * Generate compact comparison summary (plain text).
 *
 * Ratios are speedup form (other_time / self_time): >1 means tsv is faster.
 * Parse canonical is labeled "svelte" (wraps acorn-typescript for TS).
 * Each cell carries an `(Mf)` annotation — the iterated file count timing
 * reflects.
 */
export function generateComparisonSummary(
	allGroupResults: GroupResults[],
	languages: Language[],
	iteratedCounts?: Map<string, number>,
	taskTrackingByGroup?: Map<string, Map<string, string>>,
): string {
	const sections = buildComparisonData(
		allGroupResults,
		languages,
		iteratedCounts,
		taskTrackingByGroup,
	);
	const lines: string[] = [];

	// (Nf) is uniform across cells in default intersection mode and describes
	// the self impl in union mode — either way it belongs on the row label,
	// not on each opponent cell. Pad to the widest label so ratios align.
	const buildLabel = (row: ComparisonRow): string => {
		const filesSuffix = row.files !== undefined ? ` (${row.files}f)` : '';
		return `  ${row.operation.padEnd(7)}${row.language}${filesSuffix}:`;
	};
	let labelWidth = 0;
	for (const section of sections) {
		for (const row of section.rows) {
			labelWidth = Math.max(labelWidth, buildLabel(row).length + 1);
		}
	}

	for (const section of sections) {
		lines.push('');
		lines.push('-'.repeat(80));
		lines.push(`COMPARISONS to ${section.label}:`);

		for (const row of section.rows) {
			const label = buildLabel(row).padEnd(labelWidth);
			const ratios = row.comparisons.map((c) => `${formatRatio(c.ratio)} ${c.name}`).join(', ');
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
	lines.push('  (`Nx` = self is N× faster; `(Mf)` = files the timing reflects)');
	lines.push('  (parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts)');
	if (hasNativeOxc || hasWasmOxc) {
		lines.push(
			'  (oxc-parser returns a lazy proxy backed by raw buffer; tsv-json eagerly',
		);
		lines.push(
			'   materializes the full JS AST tree — comparison is not apples-to-apples)',
		);
		lines.push(
			'  (oxc-parser-wasm can outpace oxc-parser native here: NAPI marshalling adds',
		);
		lines.push(
			'   per-call cost that wasm-bindgen avoids — not a WASM-vs-native verdict)',
		);
	}
	lines.push('  (format groups include parse time — each formatter parses internally)');

	return lines.join('\n');
}

/**
 * Generate comparison summary as markdown table.
 *
 * Ratios are speedup form (other_time / self_time): >1 means self is faster.
 * `(Mf)` is the iterated file count for the self impl in that group.
 */
export function generateComparisonMarkdown(
	allGroupResults: GroupResults[],
	languages: Language[],
	iteratedCounts?: Map<string, number>,
	taskTrackingByGroup?: Map<string, Map<string, string>>,
): string | null {
	const sections = buildComparisonData(
		allGroupResults,
		languages,
		iteratedCounts,
		taskTrackingByGroup,
	);
	if (sections.length === 0) return null;

	const lines: string[] = [];

	for (const section of sections) {
		lines.push(`## Comparisons to ${section.label} (speedup)\n`);
		lines.push('| Benchmark | Comparisons |');
		lines.push('| --- | --- |');

		for (const row of section.rows) {
			const filesSuffix = row.files !== undefined ? ` (${row.files}f)` : '';
			const label = `${row.operation} ${row.language}${filesSuffix}`;
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
		'`Nx` is speedup — self is N× faster than the named opponent',
		"`(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`)",
		'Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts',
	];
	if (hasNativeOxc || hasWasmOxc) {
		notes.push(
			'oxc-parser returns a lazy proxy backed by raw buffer; tsv-json eagerly materializes the full JS AST tree — comparison is not apples-to-apples',
		);
		notes.push(
			'oxc-parser-wasm can outpace oxc-parser native here: NAPI marshalling adds per-call cost that wasm-bindgen avoids — not a WASM-vs-native verdict',
		);
	}
	notes.push('Format groups include parse time — each formatter parses internally');

	lines.push('_' + notes.join('. ') + '._');

	return lines.join('\n');
}

/** Effective corpus size info for a benchmark */
export interface EffectiveCorpusEntry {
	processed: number;
	total: number;
}

/**
 * One-line per-group throughput summary in MB/s.
 *
 * Uses per-implementation effective bytes (only files that succeeded) so
 * implementations with high skip rates aren't compared against the full
 * corpus byte total they didn't actually process. Returns null when tracking
 * info is unavailable.
 */
export function generateGroupThroughputMarkdown(
	results: BenchmarkResult[],
	tracking: Map<string, string> | undefined,
	effectiveCorpusBytes: Map<string, number>,
): string | null {
	if (!tracking || results.length === 0) return null;
	const parts: string[] = [];
	for (const r of results) {
		const trackingKey = tracking.get(r.name);
		if (!trackingKey) continue;
		const effectiveBytes = effectiveCorpusBytes.get(trackingKey);
		if (effectiveBytes === undefined || effectiveBytes === 0) continue;
		const mbPerSec = (r.stats.ops_per_second * effectiveBytes) / 1_000_000;
		parts.push(`${r.name} ${mbPerSec.toFixed(1)} MB/s`);
	}
	if (parts.length === 0) return null;
	return `**Throughput:** ${parts.join(', ')}`;
}

/**
 * Group-level files-iterated annotation. Emitted in intersection mode (every
 * impl ran the same files) so the reader sees the sample size once per
 * group. In union mode the per-impl Coverage line already discloses the
 * varying counts, so this returns null to avoid duplicating that info.
 */
export function generateGroupFilesMarkdown(
	iteratedCounts: Map<string, number> | undefined,
): string | null {
	if (!iteratedCounts || iteratedCounts.size === 0) return null;
	const values = [...iteratedCounts.values()];
	const uniform = values.every((v) => v === values[0]);
	if (!uniform) return null;
	return `**Files (intersection):** ${values[0]}`;
}

/**
 * One-line per-group coverage summary. Only emitted when implementations
 * diverge — if every participating impl processed 100% of files there's
 * nothing to disclose.
 */
export function generateGroupCoverageMarkdown(
	results: BenchmarkResult[],
	tracking: Map<string, string> | undefined,
	effectiveCorpusSize: Map<string, EffectiveCorpusEntry>,
): string | null {
	if (!tracking || results.length === 0) return null;
	const entries: { name: string; processed: number; total: number }[] = [];
	for (const r of results) {
		const trackingKey = tracking.get(r.name);
		if (!trackingKey) continue;
		const e = effectiveCorpusSize.get(trackingKey);
		if (!e) continue;
		entries.push({ name: r.name, processed: e.processed, total: e.total });
	}
	const allFull = entries.length > 0 && entries.every((e) => e.processed === e.total);
	if (allFull || entries.length === 0) return null;
	// Section presence already signals "some impl skipped"; per-row ⚠ added
	// no signal when every row was sub-100% (the common case).
	const parts = entries.map((e) => {
		// Floor (not round) so 99.85% doesn't render as "(100%)" — which
		// would look self-contradictory next to a non-100% count.
		const pct = e.processed === e.total ? 100 : Math.floor((e.processed / e.total) * 100);
		return `${e.name} ${e.processed}/${e.total} (${pct}%)`;
	});
	return `**Coverage:** ${parts.join(', ')}`;
}

/**
 * One-line JSON serialization overhead note for parse groups.
 *
 * Compares the `-json` variants (which materialize the full AST as JS objects)
 * against the matching `-internal` variants (parse only, no serialization).
 * Ratio is `json_ns / internal_ns` — read as "the JSON variant takes Nx as
 * long as the internal one." Not speedup form (this is intrinsically an
 * overhead/cost ratio, where higher = more expensive); the label spells out
 * the direction.
 */
export function generateJsonOverheadNote(results: BenchmarkResult[]): string | null {
	const pairs = [
		['tsv-internal', 'tsv-json'],
		['tsv_wasm-internal', 'tsv_wasm-json'],
	] as const;
	const notes: string[] = [];
	for (const [internalName, jsonName] of pairs) {
		const internal = results.find((r) => r.name === internalName);
		const json = results.find((r) => r.name === jsonName);
		if (!internal || !json) continue;
		const overhead = json.stats.mean_ns / internal.stats.mean_ns;
		notes.push(`${jsonName} ${overhead.toFixed(1)}x ${internalName}`);
	}
	if (notes.length === 0) return null;
	return `**JSON overhead** (json_ns / internal_ns, higher = more cost): ${notes.join(', ')}`;
}

/**
 * Generate the skipped files list as a markdown section.
 *
 * Splits the file list into per-language buckets so that one noisy language
 * (typically CSS, where prettier's test fixtures contain many SCSS/Less
 * inputs) doesn't bury skips in the other languages. Within each bucket,
 * entries are sorted by "number of benchmarks affected, descending" so the
 * most cross-cutting failures surface first.
 */
type SkipLang = 'svelte' | 'typescript' | 'css' | 'other';

/**
 * The "universal tsv failure" pattern per language — the 6 trackingKeys
 * that fail together on unsupported-syntax fixtures (SCSS, JSX in .js,
 * stage-1 proposals, etc.). When a file's failure set matches this
 * exactly, the per-file `Failed in:` list collapses to one short label;
 * anything else is rendered explicitly because it might be an
 * impl-specific bug worth chasing.
 */
function tsvUniversalSet(lang: Exclude<SkipLang, 'other'>): Set<string> {
	return new Set([
		`parse/${lang}/native`,
		`parse/${lang}/wasm`,
		`parse/${lang}/native-internal`,
		`parse/${lang}/wasm-internal`,
		`format/${lang}/native`,
		`format/${lang}/wasm`,
	]);
}

function isUniversalTsvFailure(lang: SkipLang, benchmarks: string[]): boolean {
	if (lang === 'other') return false;
	const universal = tsvUniversalSet(lang);
	if (benchmarks.length !== universal.size) return false;
	for (const b of benchmarks) if (!universal.has(b)) return false;
	return true;
}

/**
 * Resolve a trackingKey (`parse/svelte/native`) to a display label
 * (`parse/svelte: tsv-json`). Falls back to the raw trackingKey when the
 * mapping isn't available — readers still see something useful.
 */
function trackingKeyDisplay(
	trackingKey: string,
	taskTrackingByGroup: Map<string, Map<string, string>> | undefined,
): string {
	if (!taskTrackingByGroup) return trackingKey;
	const parts = trackingKey.split('/');
	if (parts.length < 3) return trackingKey;
	const groupName = `${parts[0]}/${parts[1]}`;
	const tracking = taskTrackingByGroup.get(groupName);
	if (!tracking) return trackingKey;
	for (const [displayName, key] of tracking) {
		if (key === trackingKey) return `${groupName}: ${displayName}`;
	}
	return trackingKey;
}

export function generateSkippedFilesMarkdown(
	skippedFiles: Map<string, Map<string, string>>,
	maxErrorLength = 200,
	verbose = false,
	taskTrackingByGroup?: Map<string, Map<string, string>>,
): string | null {
	if (skippedFiles.size === 0) return null;

	interface FileError {
		filePath: string;
		error: string;
		benchmarks: string[];
		lang: SkipLang;
	}

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

	function classifyLang(path: string): SkipLang {
		if (path.endsWith('.svelte') || path.endsWith('.html')) return 'svelte';
		if (path.endsWith('.ts') || path.endsWith('.js')) return 'typescript';
		if (path.endsWith('.css')) return 'css';
		return 'other';
	}

	const allErrors: FileError[] = [];
	for (const [filePath, errorMap] of fileErrorMap) {
		const lang = classifyLang(filePath);
		for (const [error, benchmarks] of errorMap) {
			allErrors.push({ filePath, error, benchmarks, lang });
		}
	}
	// Sort ascending by failure-set size (rare/impl-specific first), then
	// alphabetical. Files that fail in every tsv variant are usually
	// unsupported-syntax fixtures — push them to the bottom so actionable
	// bugs surface at the top.
	const sortFn = (a: FileError, b: FileError): number => {
		const benchDiff = a.benchmarks.length - b.benchmarks.length;
		return benchDiff !== 0 ? benchDiff : a.filePath.localeCompare(b.filePath);
	};

	const byLang = {
		svelte: allErrors.filter((e) => e.lang === 'svelte').sort(sortFn),
		typescript: allErrors.filter((e) => e.lang === 'typescript').sort(sortFn),
		css: allErrors.filter((e) => e.lang === 'css').sort(sortFn),
	};

	// Per-benchmark skip totals, sorted descending. Lets the reader see
	// "which implementation is the noisy one" at a glance.
	const perBench: { name: string; skips: number }[] = [];
	for (const [benchName, filesMap] of skippedFiles) {
		perBench.push({ name: benchName, skips: filesMap.size });
	}
	perBench.sort((a, b) => b.skips - a.skips);

	const lines: string[] = [];
	lines.push('## Skipped Files\n');
	lines.push(
		`${allErrors.length} unique file+error combinations — Svelte ${byLang.svelte.length}, TypeScript ${byLang.typescript.length}, CSS ${byLang.css.length}.\n`,
	);

	if (perBench.length > 0) {
		lines.push('**Per-benchmark skip counts:**');
		for (const { name, skips } of perBench) {
			lines.push(`- ${trackingKeyDisplay(name, taskTrackingByGroup)}: ${skips}`);
		}
		lines.push('');
	}

	if (!verbose) {
		lines.push(
			'_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._',
		);
		return lines.join('\n').trimEnd();
	}

	const TOP_N_PER_LANG = 10;
	function renderEntry(e: FileError): string[] {
		const truncated = e.error.length > maxErrorLength;
		const displayError = (truncated ? e.error.slice(0, maxErrorLength) + '…' : e.error)
			.replace(/`/g, '\\`')
			.replace(/\n/g, ' ');
		const failedIn = isUniversalTsvFailure(e.lang, e.benchmarks)
			? 'all tsv variants'
			: e.benchmarks.map((b) => trackingKeyDisplay(b, taskTrackingByGroup)).join(', ');
		return [
			`- \`${e.filePath}\``,
			`  - Error: ${displayError}`,
			`  - Failed in: ${failedIn}`,
		];
	}

	function renderBucket(label: string, entries: FileError[]): void {
		if (entries.length === 0) return;
		const more = entries.length > TOP_N_PER_LANG
			? ` (showing top ${TOP_N_PER_LANG} of ${entries.length}, sorted rarest failure-set first)`
			: '';
		lines.push(`### ${label}${more}\n`);
		for (const e of entries.slice(0, TOP_N_PER_LANG)) {
			lines.push(...renderEntry(e));
		}
		lines.push('');
	}

	renderBucket('Svelte', byLang.svelte);
	renderBucket('TypeScript', byLang.typescript);
	renderBucket('CSS', byLang.css);

	return lines.join('\n').trimEnd();
}

/**
 * Generate effective corpus report showing files actually processed per benchmark.
 *
 * `taskTrackingByGroup` is the per-group `displayName → trackingKey` map
 * captured in `bench.ts`. We invert it here to render display names
 * (e.g. `svelte/compiler`, `tsv_wasm-internal`) instead of the trackingKey
 * suffix (e.g. `canonical`, `wasm-internal`) so the labels line up with
 * the bench tables.
 */
export function generateEffectiveCorpusReport(
	effectiveCorpusSize: Map<string, EffectiveCorpusEntry>,
	taskTrackingByGroup?: Map<string, Map<string, string>>,
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

	// Build trackingKey → displayName lookup
	const trackingToDisplay = new Map<string, string>();
	if (taskTrackingByGroup) {
		for (const groupTracking of taskTrackingByGroup.values()) {
			for (const [displayName, trackingKey] of groupTracking) {
				trackingToDisplay.set(trackingKey, displayName);
			}
		}
	}

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
		// Prefer the display name when we have the tracking map; fall back
		// to the trackingKey suffix otherwise.
		const label = trackingToDisplay.get(benchName) ?? parts[2] ?? 'unknown';

		if (!grouped.has(groupKey)) {
			grouped.set(groupKey, new Map());
		}
		grouped.get(groupKey)!.set(label, entry);
	}

	// Pad column widths consistently across all groups so impl names line up.
	let maxLabelLen = 0;
	for (const impls of grouped.values()) {
		for (const label of impls.keys()) {
			if (label.length > maxLabelLen) maxLabelLen = label.length;
		}
	}

	for (const [groupName, impls] of grouped) {
		const entries = Array.from(impls.entries());
		const anySkips = entries.some(([, e]) => e.processed < e.total);
		if (!anySkips) continue;

		lines.push(`  ${groupName}:`);
		for (const [label, entry] of entries) {
			const pct = ((entry.processed / entry.total) * 100).toFixed(0);
			lines.push(
				`    ${label.padEnd(maxLabelLen)} ${entry.processed}/${entry.total} files (${pct}%)`,
			);
		}
		lines.push('');
	}

	return lines.join('\n');
}
