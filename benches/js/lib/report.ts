/**
 * Summary report generation for benchmark results
 */

import type { BenchmarkResult } from '@fuzdev/fuz_util/benchmark_types.ts';
import { benchmark_format_number } from '@fuzdev/fuz_util/benchmark_format.ts';
import { time_format, time_unit_detect_best, TIME_UNIT_DISPLAY } from '@fuzdev/fuz_util/time.ts';

import type { Language } from './types.ts';

/** Results from a benchmark group */
export interface GroupResults {
	name: string;
	results: BenchmarkResult[];
}

/**
 * Coverage percentage, floored (never rounded). Floor — not `.toFixed(0)` —
 * so 99.85% renders as `99%`, never `100%`: a rounded `(100%)` next to a
 * non-full file count (e.g. `659/660`) would read as self-contradictory.
 * A genuinely full set returns exactly 100.
 */
function coverage_pct(processed: number, total: number): number {
	return processed === total ? 100 : Math.floor((processed / total) * 100);
}

/**
 * Render one `**Coverage:** …` line from pre-resolved per-impl counts. Shared by
 * `generate_group_coverage_markdown` (the perf/timed per-group summary) and
 * `generate_coverage_only_markdown` (the conformance report) so the row format
 * (`name processed/total (pct%)`) lives in exactly one place.
 */
function format_coverage_line(
	rows: ReadonlyArray<{ name: string; processed: number; total: number }>,
): string {
	const parts = rows.map(
		(e) => `${e.name} ${e.processed}/${e.total} (${coverage_pct(e.processed, e.total)}%)`,
	);
	return `**Coverage:** ${parts.join(', ')}`;
}

/** Create a visual bar for comparison (based on time - slower = longer bar) */
function create_bar(value: number, max: number, width = 40): string {
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
	// tsv variants
	'tsv-json',
	'tsv-json-no-locations',
	'tsv_wasm-json',
	'tsv_wasm-json-no-locations',
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
function sort_by_display_order(results: BenchmarkResult[]): BenchmarkResult[] {
	return [...results].sort((a, b) => {
		const a_index = DISPLAY_ORDER.indexOf(a.name);
		const b_index = DISPLAY_ORDER.indexOf(b.name);
		// Unknown items go to the end
		const a_order = a_index === -1 ? DISPLAY_ORDER.length : a_index;
		const b_order = b_index === -1 ? DISPLAY_ORDER.length : b_index;
		return a_order - b_order;
	});
}

/** Generate the summary report */
export function generate_summary_report(
	all_group_results: GroupResults[],
	languages: Language[],
): string {
	const lines: string[] = [];

	lines.push('');
	lines.push('='.repeat(80));
	lines.push('BENCHMARK SUMMARY  (every `Nx` is speedup form — >1 means faster than baseline)');
	lines.push('='.repeat(80));

	/** Get results for a specific group */
	function get_group_results(name: string): BenchmarkResult[] {
		return all_group_results.find((g) => g.name === name)?.results ?? [];
	}

	// Collect all times for consistent unit selection
	const all_mean_times: number[] = [];
	for (const group of all_group_results) {
		for (const result of group.results) {
			all_mean_times.push(result.stats.mean_ns);
		}
	}
	const unit = time_unit_detect_best(all_mean_times);
	const fmt = (ns: number) => time_format(ns, unit, 2);

	/**
	 * Speedup-form comparison: `baseline_ns / current_ns` — values > 1 mean
	 * `current` is faster, < 1 mean slower. Single convention so the reader
	 * doesn't context-switch between "Nx faster" and "Nx slower" framings.
	 */
	function format_comparison(baseline: number, current: number): string {
		const ratio = baseline / current;
		return `(${ratio.toFixed(2)}x)`;
	}

	// Parse performance comparison
	// The `*-json-no-locations` rows render here as ordinary bars (they're not
	// INTERNAL_PARSE_VARIANTS). TODO: add a curated "no-locations vs oxc-parser"
	// summary line — that's the payload-matched comparison (same span-only shape),
	// where plain `tsv-json` carries the richer loc-bearing drop-in AST.
	lines.push('');
	lines.push('Parse Performance:');
	for (const lang of languages) {
		const results = get_group_results(`parse/${lang}`);
		if (results.length === 0) continue;

		const canonical_name = CANONICAL_PARSERS[lang];
		const canonical_result = results.find((r) => r.name === canonical_name);

		// Get main results (excluding internal variants)
		const main_results = results.filter((r) => !INTERNAL_PARSE_VARIANTS.includes(r.name));
		// Get internal variants
		const internal_results = results.filter((r) => INTERNAL_PARSE_VARIANTS.includes(r.name));

		if (main_results.length === 0) continue;

		// Calculate max time for bar scaling (main results only)
		const max_time = Math.max(...main_results.map((r) => r.stats.mean_ns));
		const baseline = canonical_result?.stats.mean_ns ?? main_results[0].stats.mean_ns;

		// Find the longest name for padding
		const max_name_len = Math.max(...results.map((r) => r.name.length), 17);

		lines.push('');
		lines.push(`  ${lang}:`);

		// Show canonical first (baseline)
		if (canonical_result) {
			lines.push(
				`    ${canonical_result.name.padEnd(max_name_len)} ${
					create_bar(canonical_result.stats.mean_ns, max_time)
				} ${fmt(canonical_result.stats.mean_ns)}`,
			);
		}

		// Show alternatives in stable display order (tsv variants, then third-party)
		const alternatives = sort_by_display_order(
			main_results.filter((r) => r.name !== canonical_name),
		);

		for (const result of alternatives) {
			const comparison = format_comparison(baseline, result.stats.mean_ns);
			lines.push(
				`    ${result.name.padEnd(max_name_len)} ${create_bar(result.stats.mean_ns, max_time)} ${
					fmt(result.stats.mean_ns)
				} ${comparison}`,
			);
		}

		// Show internal variants (JSON overhead measurement)
		for (const internal_result of sort_by_display_order(internal_results)) {
			// Find the corresponding JSON variant
			const json_name = internal_result.name.includes('wasm') ? 'tsv_wasm-json' : 'tsv-json';
			const json_result = results.find((r) => r.name === json_name);

			if (json_result) {
				const json_overhead = json_result.stats.mean_ns / internal_result.stats.mean_ns;
				lines.push(
					`    ${internal_result.name.padEnd(max_name_len)} ${
						create_bar(internal_result.stats.mean_ns, max_time)
					} ${fmt(internal_result.stats.mean_ns)} (${json_overhead.toFixed(1)}x JSON overhead)`,
				);
			}
		}
	}

	// Format performance comparison
	lines.push('');
	lines.push('');
	lines.push('Format Performance:');
	for (const lang of languages) {
		const results = get_group_results(`format/${lang}`);
		if (results.length === 0) continue;

		const canonical_result = results.find((r) => r.name === CANONICAL_FORMATTER);
		if (!canonical_result) continue;

		// Calculate max time for bar scaling
		const max_time = Math.max(...results.map((r) => r.stats.mean_ns));
		const baseline = canonical_result.stats.mean_ns;

		// Find the longest name for padding
		const max_name_len = Math.max(...results.map((r) => r.name.length), 8);

		lines.push('');
		lines.push(`  ${lang}:`);

		// Show canonical first (baseline)
		lines.push(
			`    ${canonical_result.name.padEnd(max_name_len)} ${
				create_bar(canonical_result.stats.mean_ns, max_time)
			} ${fmt(canonical_result.stats.mean_ns)}`,
		);

		// Show alternatives in stable display order (tsv variants, then third-party)
		const alternatives = sort_by_display_order(
			results.filter((r) => r.name !== CANONICAL_FORMATTER),
		);

		for (const result of alternatives) {
			const comparison = format_comparison(baseline, result.stats.mean_ns);
			lines.push(
				`    ${result.name.padEnd(max_name_len)} ${create_bar(result.stats.mean_ns, max_time)} ${
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
export function generate_skipped_files_report(
	skipped_files: Map<string, Map<string, string>>,
	max_error_length = 200,
	verbose = false,
	task_tracking_by_group?: Map<string, Map<string, string>>,
): string | null {
	if (skipped_files.size === 0) return null;

	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('SKIPPED FILES:');

	const file_error_map = new Map<string, Map<string, string[]>>();
	for (const [bench_name, files_map] of skipped_files) {
		for (const [file_path, error] of files_map) {
			if (!file_error_map.has(file_path)) {
				file_error_map.set(file_path, new Map());
			}
			const error_map = file_error_map.get(file_path)!;
			if (!error_map.has(error)) {
				error_map.set(error, []);
			}
			error_map.get(error)!.push(bench_name);
		}
	}

	interface FileError {
		file_path: string;
		error: string;
		benchmarks: string[];
		lang: SkipLang;
	}

	function classify_lang(path: string): SkipLang {
		if (path.endsWith('.svelte') || path.endsWith('.html')) return 'svelte';
		if (path.endsWith('.ts') || path.endsWith('.js')) return 'typescript';
		if (path.endsWith('.css')) return 'css';
		return 'other';
	}

	const all_errors: FileError[] = [];
	for (const [file_path, error_map] of file_error_map) {
		const lang = classify_lang(file_path);
		for (const [error, benchmarks] of error_map) {
			all_errors.push({ file_path, error, benchmarks, lang });
		}
	}
	// Ascending by failure-set size — rare/impl-specific failures first.
	const sorted_errors = all_errors.sort((a, b) => {
		const bench_diff = a.benchmarks.length - b.benchmarks.length;
		return bench_diff !== 0 ? bench_diff : a.file_path.localeCompare(b.file_path);
	});

	const skips_by_lang = { svelte: 0, typescript: 0, css: 0 };
	for (const { lang } of sorted_errors) {
		if (lang !== 'other') skips_by_lang[lang]++;
	}

	lines.push(`Total unique file+error combinations: ${sorted_errors.length}`);
	lines.push(`  Svelte:      ${skips_by_lang.svelte} files skipped`);
	lines.push(`  TypeScript:  ${skips_by_lang.typescript} files skipped`);
	lines.push(`  CSS:         ${skips_by_lang.css} files skipped`);

	// Per-benchmark skip counts (always shown). Display names instead of
	// tracking_keys so the labels match the bench tables.
	const per_bench: { name: string; skips: number }[] = [];
	for (const [bench_name, files_map] of skipped_files) {
		per_bench.push({ name: bench_name, skips: files_map.size });
	}
	per_bench.sort((a, b) => b.skips - a.skips);
	if (per_bench.length > 0) {
		lines.push('');
		lines.push('Per-benchmark skip counts:');
		for (const { name, skips } of per_bench) {
			lines.push(`  ${tracking_key_display(name, task_tracking_by_group)}: ${skips}`);
		}
	}

	if (!verbose) {
		lines.push('');
		lines.push('(Per-file detail omitted. Re-run with `--verbose` for paths + errors.)');
		return lines.join('\n');
	}

	lines.push('');
	for (const { file_path, error, benchmarks, lang } of sorted_errors.slice(0, 10)) {
		lines.push(file_path);
		const truncated = error.length > max_error_length;
		const display_error = truncated ? error.slice(0, max_error_length) + '...' : error;
		lines.push(`  Error: ${display_error}`);
		const failed_in = is_universal_tsv_failure(lang, benchmarks)
			? 'all tsv variants'
			: benchmarks.map((b) => tracking_key_display(b, task_tracking_by_group)).join(', ');
		const prefix = benchmarks.length === 1
			? 'Failed in'
			: `Failed in ${benchmarks.length} benchmarks`;
		lines.push(`  ${prefix}: ${failed_in}`);
		lines.push('');
	}

	if (sorted_errors.length > 10) {
		lines.push(`  ... and ${sorted_errors.length - 10} more (sorted rarest failure-set first)`);
	}

	return lines.join('\n');
}

/**
 * Versions block for the terminal run. (Corpus counts already print at the
 * top of the run, so this used to duplicate them — now versions only.)
 */
export function generate_versions_info(versions: {
	svelte: string;
	acorn: string;
	acorn_ts: string;
	prettier: string;
	prettier_svelte: string;
	oxc_parser?: string;
	oxfmt?: string;
	biome?: string;
}): string {
	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('Versions:');
	lines.push(
		`  svelte@${versions.svelte}, acorn@${versions.acorn}, @sveltejs/acorn-typescript@${versions.acorn_ts}`,
	);
	lines.push(`  prettier@${versions.prettier}, prettier-plugin-svelte@${versions.prettier_svelte}`);

	const alt_versions: string[] = [];
	if (versions.oxc_parser) alt_versions.push(`oxc-parser@${versions.oxc_parser}`);
	if (versions.oxfmt) alt_versions.push(`oxfmt@${versions.oxfmt}`);
	if (versions.biome) alt_versions.push(`@biomejs/wasm-bundler@${versions.biome}`);

	if (alt_versions.length > 0) {
		lines.push(`  ${alt_versions.join(', ')}`);
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

/** Resolve the iterated file count for a (group, display_name) pair via task_tracking. */
function lookup_iterated(
	group_name: string,
	display_name: string,
	iterated_counts: Map<string, number> | undefined,
	task_tracking_by_group: Map<string, Map<string, string>> | undefined,
): number | undefined {
	if (!iterated_counts || !task_tracking_by_group) return undefined;
	const tracking_key = task_tracking_by_group.get(group_name)?.get(display_name);
	if (!tracking_key) return undefined;
	return iterated_counts.get(tracking_key);
}

/** Format ratio as "Nx" (other_time / tsv_time) */
function format_ratio(r: number): string {
	return r >= 10 ? `${r.toFixed(1)}x` : `${r.toFixed(2)}x`;
}

/**
 * Per-group benchmark table in speedup-form markdown. Mirrors the column
 * layout of `benchmark_format_markdown` (Task Name, ops/sec, percentiles,
 * min/max, vs baseline) but inverts the ratio: cells show
 * `r.ops_per_second / baseline.ops_per_second`, so `2.5x` means "this row is
 * 2.5× faster than baseline." The iterated file count is rendered as a
 * group-level annotation (see `generate_group_files_markdown`) rather than per
 * cell — same value across all rows in default intersection mode, so the
 * repetition was pure noise.
 */
export function generate_group_bench_table_markdown(
	results: BenchmarkResult[],
	baseline: string | undefined,
): string {
	if (results.length === 0) return '(no results)';

	const mean_times = results.map((r) => r.stats.mean_ns);
	const unit = time_unit_detect_best(mean_times);
	const unit_str = TIME_UNIT_DISPLAY[unit];

	// Track the baseline by row index, not by ops/sec value. A value-equality
	// check (`r.ops === baseline_ops`) mislabels every row that ties the max
	// in the no-named-baseline branch; pinning the index labels exactly one.
	let baseline_index: number;
	let vs_header: string;
	const named_index = baseline !== undefined ? results.findIndex((r) => r.name === baseline) : -1;
	if (named_index !== -1) {
		baseline_index = named_index;
		vs_header = `vs ${baseline} (speedup)`;
	} else {
		// First row achieving the max ops/sec is the baseline; later ties are
		// labeled with their speedup (`1.00x`), not a second `baseline`.
		const max_ops = Math.max(...results.map((r) => r.stats.ops_per_second));
		baseline_index = results.findIndex((r) => r.stats.ops_per_second === max_ops);
		vs_header = 'vs Best (speedup)';
	}
	const baseline_ops = results[baseline_index].stats.ops_per_second;

	const rows: string[][] = [];
	rows.push([
		'Task Name',
		'ops/sec',
		'n',
		`p50 (${unit_str})`,
		`p75 (${unit_str})`,
		`p90 (${unit_str})`,
		`p95 (${unit_str})`,
		`p99 (${unit_str})`,
		`min (${unit_str})`,
		`max (${unit_str})`,
		vs_header,
	]);

	for (let row_index = 0; row_index < results.length; row_index++) {
		const r = results[row_index];
		const fmt = (ns: number) => time_format(ns, unit, 2).replace(unit_str, '').trim();
		const is_baseline = row_index === baseline_index;
		const speedup = r.stats.ops_per_second / baseline_ops;
		const vs_cell = is_baseline ? 'baseline' : format_ratio(speedup);
		// p95/p99 from <10 samples is essentially `max` (R-7 interpolation
		// collapses to the last sorted index). Render `—` so readers don't
		// misread interpolated noise as tail-latency data.
		const tail_cell = (ns: number) => (r.stats.sample_size < 10 ? '—' : fmt(ns));
		rows.push([
			r.name,
			benchmark_format_number(r.stats.ops_per_second, 2),
			String(r.stats.sample_size),
			fmt(r.stats.p50_ns),
			fmt(r.stats.p75_ns),
			fmt(r.stats.p90_ns),
			tail_cell(r.stats.p95_ns),
			tail_cell(r.stats.p99_ns),
			fmt(r.stats.min_ns),
			fmt(r.stats.max_ns),
			vs_cell,
		]);
	}

	const widths = rows[0].map((_, i) => Math.max(...rows.map((row) => row[i].length)));
	const lines: string[] = [];
	const render_row = (row: string[]) =>
		'| ' + row.map((c, i) => c.padEnd(widths[i])).join(' | ') + ' |';
	lines.push(render_row(rows[0]));
	lines.push('| ' + widths.map((w) => '-'.repeat(w)).join(' | ') + ' |');
	for (let i = 1; i < rows.length; i++) {
		lines.push(render_row(rows[i]));
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
function build_comparison_data(
	all_group_results: GroupResults[],
	languages: Language[],
	iterated_counts: Map<string, number> | undefined,
	task_tracking_by_group: Map<string, Map<string, string>> | undefined,
): ComparisonSection[] {
	function get_mean_ns(group_name: string, task_name: string): number | null {
		const group = all_group_results.find((g) => g.name === group_name);
		if (!group) return null;
		const result = group.results.find((r) => r.name === task_name);
		return result?.stats.mean_ns ?? null;
	}

	function ratio(tsv_ns: number, other_ns: number): number {
		return other_ns / tsv_ns;
	}

	const sections: ComparisonSection[] = [];

	// Native comparisons
	const native_rows: ComparisonRow[] = [];

	for (const lang of languages) {
		const group_name = `format/${lang}`;
		const tsv_ns = get_mean_ns(group_name, 'tsv');
		const prettier_ns = get_mean_ns(group_name, CANONICAL_FORMATTER);
		if (tsv_ns === null || prettier_ns === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'prettier', ratio: ratio(tsv_ns, prettier_ns) },
		];
		const oxfmt_ns = get_mean_ns(group_name, 'oxfmt');
		if (oxfmt_ns !== null) comparisons.push({ name: 'oxfmt', ratio: ratio(tsv_ns, oxfmt_ns) });

		native_rows.push({
			operation: 'format',
			language: lang,
			files: lookup_iterated(group_name, 'tsv', iterated_counts, task_tracking_by_group),
			comparisons,
		});
	}

	for (const lang of languages) {
		const group_name = `parse/${lang}`;
		const tsv_ns = get_mean_ns(group_name, 'tsv-json');
		const canonical_parse_name = CANONICAL_PARSERS[lang];
		const canonical_ns = get_mean_ns(group_name, canonical_parse_name);
		if (tsv_ns === null || canonical_ns === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'svelte', ratio: ratio(tsv_ns, canonical_ns) },
		];
		const oxc_ns = get_mean_ns(group_name, 'oxc-parser');
		if (oxc_ns !== null) comparisons.push({ name: 'oxc-parser', ratio: ratio(tsv_ns, oxc_ns) });

		native_rows.push({
			operation: 'parse',
			language: lang,
			files: lookup_iterated(group_name, 'tsv-json', iterated_counts, task_tracking_by_group),
			comparisons,
		});
	}

	if (native_rows.length > 0) {
		sections.push({ label: 'tsv', rows: native_rows });
	}

	// WASM comparisons
	const wasm_rows: ComparisonRow[] = [];

	for (const lang of languages) {
		const group_name = `format/${lang}`;
		const tsv_wasm_ns = get_mean_ns(group_name, 'tsv_wasm');
		const prettier_ns = get_mean_ns(group_name, CANONICAL_FORMATTER);
		if (tsv_wasm_ns === null || prettier_ns === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'prettier', ratio: ratio(tsv_wasm_ns, prettier_ns) },
		];
		const biome_ns = get_mean_ns(group_name, 'biome-wasm');
		if (biome_ns !== null) {
			comparisons.push({ name: 'biome-wasm', ratio: ratio(tsv_wasm_ns, biome_ns) });
		}

		wasm_rows.push({
			operation: 'format',
			language: lang,
			files: lookup_iterated(group_name, 'tsv_wasm', iterated_counts, task_tracking_by_group),
			comparisons,
		});
	}

	for (const lang of languages) {
		const group_name = `parse/${lang}`;
		const tsv_wasm_ns = get_mean_ns(group_name, 'tsv_wasm-json');
		const canonical_parse_name = CANONICAL_PARSERS[lang];
		const canonical_ns = get_mean_ns(group_name, canonical_parse_name);
		if (tsv_wasm_ns === null || canonical_ns === null) continue;

		const comparisons: ComparisonRow['comparisons'] = [
			{ name: 'svelte', ratio: ratio(tsv_wasm_ns, canonical_ns) },
		];
		const oxc_wasm_ns = get_mean_ns(group_name, 'oxc-parser-wasm');
		if (oxc_wasm_ns !== null) {
			comparisons.push({ name: 'oxc-parser-wasm', ratio: ratio(tsv_wasm_ns, oxc_wasm_ns) });
		}

		wasm_rows.push({
			operation: 'parse',
			language: lang,
			files: lookup_iterated(group_name, 'tsv_wasm-json', iterated_counts, task_tracking_by_group),
			comparisons,
		});
	}

	if (wasm_rows.length > 0) {
		sections.push({ label: 'tsv_wasm', rows: wasm_rows });
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
export function generate_comparison_summary(
	all_group_results: GroupResults[],
	languages: Language[],
	iterated_counts?: Map<string, number>,
	task_tracking_by_group?: Map<string, Map<string, string>>,
): string {
	const sections = build_comparison_data(
		all_group_results,
		languages,
		iterated_counts,
		task_tracking_by_group,
	);
	const lines: string[] = [];

	// (Nf) is uniform across cells in default intersection mode and describes
	// the self impl in union mode — either way it belongs on the row label,
	// not on each opponent cell. Pad to the widest label so ratios align.
	const build_label = (row: ComparisonRow): string => {
		const files_suffix = row.files !== undefined ? ` (${row.files}f)` : '';
		return `  ${row.operation.padEnd(7)}${row.language}${files_suffix}:`;
	};
	let label_width = 0;
	for (const section of sections) {
		for (const row of section.rows) {
			label_width = Math.max(label_width, build_label(row).length + 1);
		}
	}

	for (const section of sections) {
		lines.push('');
		lines.push('-'.repeat(80));
		lines.push(`COMPARISONS to ${section.label}:`);

		for (const row of section.rows) {
			const label = build_label(row).padEnd(label_width);
			const ratios = row.comparisons.map((c) => `${format_ratio(c.ratio)} ${c.name}`).join(', ');
			lines.push(label + ratios);
		}
	}

	// Fairness notes (only shown when oxc-parser data is present)
	const has_native_oxc = sections.some((s) =>
		s.label === 'tsv' &&
		s.rows.some((r) => r.comparisons.some((c) => c.name === 'oxc-parser'))
	);
	const has_wasm_oxc = sections.some((s) =>
		s.label === 'tsv_wasm' &&
		s.rows.some((r) => r.comparisons.some((c) => c.name === 'oxc-parser-wasm'))
	);

	lines.push('');
	lines.push('  (`Nx` = self is N× faster; `(Mf)` = files the timing reflects)');
	lines.push('  (parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts)');
	if (has_native_oxc || has_wasm_oxc) {
		lines.push(
			'  (oxc-parser — native and wasm — serializes the AST to JSON in Rust and',
		);
		lines.push(
			'   deserializes in JS, the same eager materialization as tsv-json — apples-to-apples)',
		);
		lines.push(
			'  (tsv-internal/tsv_wasm-internal are parse-only, no JS materialization;',
		);
		lines.push(
			'   oxc has no comparably cheap mode, so they have no oxc counterpart)',
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
export function generate_comparison_markdown(
	all_group_results: GroupResults[],
	languages: Language[],
	iterated_counts?: Map<string, number>,
	task_tracking_by_group?: Map<string, Map<string, string>>,
): string | null {
	const sections = build_comparison_data(
		all_group_results,
		languages,
		iterated_counts,
		task_tracking_by_group,
	);
	if (sections.length === 0) return null;

	const lines: string[] = [];

	for (const section of sections) {
		lines.push(`## Comparisons to ${section.label} (speedup)\n`);
		lines.push('| Benchmark | Comparisons |');
		lines.push('| --- | --- |');

		for (const row of section.rows) {
			const files_suffix = row.files !== undefined ? ` (${row.files}f)` : '';
			const label = `${row.operation} ${row.language}${files_suffix}`;
			const ratios = row.comparisons
				.map((c) => `**${format_ratio(c.ratio)}** ${c.name}`)
				.join(', ');
			lines.push(`| ${label} | ${ratios} |`);
		}

		lines.push('');
	}

	// Fairness notes (only shown when oxc-parser data is present)
	const has_native_oxc = sections.some((s) =>
		s.label === 'tsv' &&
		s.rows.some((r) => r.comparisons.some((c) => c.name === 'oxc-parser'))
	);
	const has_wasm_oxc = sections.some((s) =>
		s.label === 'tsv_wasm' &&
		s.rows.some((r) => r.comparisons.some((c) => c.name === 'oxc-parser-wasm'))
	);

	const notes: string[] = [
		'`Nx` is speedup — self is N× faster than the named opponent',
		"`(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`)",
		'Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts',
	];
	if (has_native_oxc || has_wasm_oxc) {
		notes.push(
			'oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples',
		);
		notes.push(
			'tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated)',
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
export function generate_group_throughput_markdown(
	results: BenchmarkResult[],
	tracking: Map<string, string> | undefined,
	effective_corpus_bytes: Map<string, number>,
): string | null {
	if (!tracking || results.length === 0) return null;
	const parts: string[] = [];
	for (const r of results) {
		const tracking_key = tracking.get(r.name);
		if (!tracking_key) continue;
		const effective_bytes = effective_corpus_bytes.get(tracking_key);
		if (effective_bytes === undefined || effective_bytes === 0) continue;
		const mb_per_sec = (r.stats.ops_per_second * effective_bytes) / 1_000_000;
		parts.push(`${r.name} ${mb_per_sec.toFixed(1)} MB/s`);
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
export function generate_group_files_markdown(
	iterated_counts: Map<string, number> | undefined,
): string | null {
	if (!iterated_counts || iterated_counts.size === 0) return null;
	const values = [...iterated_counts.values()];
	const uniform = values.every((v) => v === values[0]);
	if (!uniform) return null;
	return `**Files (intersection):** ${values[0]}`;
}

/**
 * One-line per-group coverage summary. Only emitted when implementations
 * diverge — if every participating impl processed 100% of files there's
 * nothing to disclose.
 */
export function generate_group_coverage_markdown(
	results: BenchmarkResult[],
	tracking: Map<string, string> | undefined,
	effective_corpus_size: Map<string, EffectiveCorpusEntry>,
): string | null {
	if (!tracking || results.length === 0) return null;
	const entries: { name: string; processed: number; total: number }[] = [];
	for (const r of results) {
		const tracking_key = tracking.get(r.name);
		if (!tracking_key) continue;
		const e = effective_corpus_size.get(tracking_key);
		if (!e) continue;
		entries.push({ name: r.name, processed: e.processed, total: e.total });
	}
	const all_full = entries.length > 0 && entries.every((e) => e.processed === e.total);
	if (all_full || entries.length === 0) return null;
	// Section presence already signals "some impl skipped"; per-row ⚠ added
	// no signal when every row was sub-100% (the common case).
	return format_coverage_line(entries);
}

/**
 * Coverage-only conformance report body: one `## group` + `**Coverage:**`
 * section per `language × operation`, rendered straight from pre-flight state
 * (a `BENCH_COVERAGE_ONLY=1` run skips the timed phase, so no result groups
 * exist). Unlike `generate_group_coverage_markdown` — the per-group perf
 * summary, which suppresses a line when every impl processed 100% (there it's a
 * "some impl skipped" warning) — every row is shown here including 100%, because
 * coverage IS the conformance headline. Returns the lines to splice into the
 * report (empty when no group has coverage data).
 */
export function generate_coverage_only_markdown(
	languages: readonly Language[],
	operations: readonly ('parse' | 'format')[],
	task_tracking: Map<string, Map<string, string>>,
	effective_corpus_size: Map<string, EffectiveCorpusEntry>,
): string[] {
	const lines: string[] = [];
	for (const language of languages) {
		for (const operation of operations) {
			const group_name = `${operation}/${language}`;
			const tracking = task_tracking.get(group_name);
			if (!tracking) continue;
			const rows: { name: string; processed: number; total: number }[] = [];
			for (const [name, tracking_key] of tracking) {
				const e = effective_corpus_size.get(tracking_key);
				if (!e) continue;
				rows.push({ name, processed: e.processed, total: e.total });
			}
			if (rows.length === 0) continue;
			lines.push(`## ${group_name}\n`);
			lines.push(format_coverage_line(rows), '');
		}
	}
	return lines;
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
export function generate_json_overhead_note(results: BenchmarkResult[]): string | null {
	// Each pair is [non-materializing parse, full-JS-tree parse]; the ratio is the
	// cost of materializing the AST into JS. tsv-only: oxc has no comparably cheap
	// parse-only mode (its JS API always serializes to cross into JS; experimentalLazy
	// is setup-dominated — see oxc.ts), so there's no oxc pair to show here.
	const pairs = [
		['tsv-internal', 'tsv-json'],
		['tsv_wasm-internal', 'tsv_wasm-json'],
	] as const;
	const notes: string[] = [];
	for (const [internal_name, json_name] of pairs) {
		const internal = results.find((r) => r.name === internal_name);
		const json = results.find((r) => r.name === json_name);
		if (!internal || !json) continue;
		const overhead = json.stats.mean_ns / internal.stats.mean_ns;
		notes.push(`${json_name} ${overhead.toFixed(1)}x ${internal_name}`);
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
 * The "universal tsv failure" pattern per language — the 6 tracking_keys
 * that fail together on unsupported-syntax fixtures (SCSS, JSX in .js,
 * stage-1 proposals, etc.). When a file's failure set matches this
 * exactly, the per-file `Failed in:` list collapses to one short label;
 * anything else is rendered explicitly because it might be an
 * impl-specific bug worth chasing.
 */
function tsv_universal_set(lang: Exclude<SkipLang, 'other'>): Set<string> {
	return new Set([
		`parse/${lang}/native`,
		`parse/${lang}/wasm`,
		`parse/${lang}/native-internal`,
		`parse/${lang}/wasm-internal`,
		`format/${lang}/native`,
		`format/${lang}/wasm`,
	]);
}

function is_universal_tsv_failure(lang: SkipLang, benchmarks: string[]): boolean {
	if (lang === 'other') return false;
	const universal = tsv_universal_set(lang);
	if (benchmarks.length !== universal.size) return false;
	for (const b of benchmarks) if (!universal.has(b)) return false;
	return true;
}

/**
 * Resolve a tracking_key (`parse/svelte/native`) to a display label
 * (`parse/svelte: tsv-json`). Falls back to the raw tracking_key when the
 * mapping isn't available — readers still see something useful.
 */
function tracking_key_display(
	tracking_key: string,
	task_tracking_by_group: Map<string, Map<string, string>> | undefined,
): string {
	if (!task_tracking_by_group) return tracking_key;
	const parts = tracking_key.split('/');
	if (parts.length < 3) return tracking_key;
	const group_name = `${parts[0]}/${parts[1]}`;
	const tracking = task_tracking_by_group.get(group_name);
	if (!tracking) return tracking_key;
	for (const [display_name, key] of tracking) {
		if (key === tracking_key) return `${group_name}: ${display_name}`;
	}
	return tracking_key;
}

export function generate_skipped_files_markdown(
	skipped_files: Map<string, Map<string, string>>,
	max_error_length = 200,
	verbose = false,
	task_tracking_by_group?: Map<string, Map<string, string>>,
): string | null {
	if (skipped_files.size === 0) return null;

	interface FileError {
		file_path: string;
		error: string;
		benchmarks: string[];
		lang: SkipLang;
	}

	const file_error_map = new Map<string, Map<string, string[]>>();
	for (const [bench_name, files_map] of skipped_files) {
		for (const [file_path, error] of files_map) {
			if (!file_error_map.has(file_path)) {
				file_error_map.set(file_path, new Map());
			}
			const error_map = file_error_map.get(file_path)!;
			if (!error_map.has(error)) {
				error_map.set(error, []);
			}
			error_map.get(error)!.push(bench_name);
		}
	}

	function classify_lang(path: string): SkipLang {
		if (path.endsWith('.svelte') || path.endsWith('.html')) return 'svelte';
		if (path.endsWith('.ts') || path.endsWith('.js')) return 'typescript';
		if (path.endsWith('.css')) return 'css';
		return 'other';
	}

	const all_errors: FileError[] = [];
	for (const [file_path, error_map] of file_error_map) {
		const lang = classify_lang(file_path);
		for (const [error, benchmarks] of error_map) {
			all_errors.push({ file_path, error, benchmarks, lang });
		}
	}
	// Sort ascending by failure-set size (rare/impl-specific first), then
	// alphabetical. Files that fail in every tsv variant are usually
	// unsupported-syntax fixtures — push them to the bottom so actionable
	// bugs surface at the top.
	const sort_fn = (a: FileError, b: FileError): number => {
		const bench_diff = a.benchmarks.length - b.benchmarks.length;
		return bench_diff !== 0 ? bench_diff : a.file_path.localeCompare(b.file_path);
	};

	const by_lang = {
		svelte: all_errors.filter((e) => e.lang === 'svelte').sort(sort_fn),
		typescript: all_errors.filter((e) => e.lang === 'typescript').sort(sort_fn),
		css: all_errors.filter((e) => e.lang === 'css').sort(sort_fn),
	};

	// Per-benchmark skip totals, sorted descending. Lets the reader see
	// "which implementation is the noisy one" at a glance.
	const per_bench: { name: string; skips: number }[] = [];
	for (const [bench_name, files_map] of skipped_files) {
		per_bench.push({ name: bench_name, skips: files_map.size });
	}
	per_bench.sort((a, b) => b.skips - a.skips);

	const lines: string[] = [];
	lines.push('## Skipped Files\n');
	lines.push(
		`${all_errors.length} unique file+error combinations — Svelte ${by_lang.svelte.length}, TypeScript ${by_lang.typescript.length}, CSS ${by_lang.css.length}.\n`,
	);

	if (per_bench.length > 0) {
		lines.push('**Per-benchmark skip counts:**');
		for (const { name, skips } of per_bench) {
			lines.push(`- ${tracking_key_display(name, task_tracking_by_group)}: ${skips}`);
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
	function render_entry(e: FileError): string[] {
		const truncated = e.error.length > max_error_length;
		const display_error = (truncated ? e.error.slice(0, max_error_length) + '…' : e.error)
			.replace(/`/g, '\\`')
			.replace(/\n/g, ' ');
		const failed_in = is_universal_tsv_failure(e.lang, e.benchmarks)
			? 'all tsv variants'
			: e.benchmarks.map((b) => tracking_key_display(b, task_tracking_by_group)).join(', ');
		return [
			`- \`${e.file_path}\``,
			`  - Error: ${display_error}`,
			`  - Failed in: ${failed_in}`,
		];
	}

	function render_bucket(label: string, entries: FileError[]): void {
		if (entries.length === 0) return;
		const more = entries.length > TOP_N_PER_LANG
			? ` (showing top ${TOP_N_PER_LANG} of ${entries.length}, sorted rarest failure-set first)`
			: '';
		lines.push(`### ${label}${more}\n`);
		for (const e of entries.slice(0, TOP_N_PER_LANG)) {
			lines.push(...render_entry(e));
		}
		lines.push('');
	}

	render_bucket('Svelte', by_lang.svelte);
	render_bucket('TypeScript', by_lang.typescript);
	render_bucket('CSS', by_lang.css);

	return lines.join('\n').trimEnd();
}

/**
 * Generate effective corpus report showing files actually processed per benchmark.
 *
 * `task_tracking_by_group` is the per-group `display_name → tracking_key` map
 * captured in `bench.ts`. We invert it here to render display names
 * (e.g. `svelte/compiler`, `tsv_wasm-internal`) instead of the tracking_key
 * suffix (e.g. `canonical`, `wasm-internal`) so the labels line up with
 * the bench tables.
 */
export function generate_effective_corpus_report(
	effective_corpus_size: Map<string, EffectiveCorpusEntry>,
	task_tracking_by_group?: Map<string, Map<string, string>>,
): string | null {
	// Check if any benchmarks had skipped files
	let has_skips = false;
	for (const { processed, total } of effective_corpus_size.values()) {
		if (processed < total) {
			has_skips = true;
			break;
		}
	}

	if (!has_skips) return null;

	// Build tracking_key → display_name lookup
	const tracking_to_display = new Map<string, string>();
	if (task_tracking_by_group) {
		for (const group_tracking of task_tracking_by_group.values()) {
			for (const [display_name, tracking_key] of group_tracking) {
				tracking_to_display.set(tracking_key, display_name);
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
	for (const [bench_name, entry] of effective_corpus_size) {
		// bench_name format: "parse/svelte/canonical" or "format/typescript/native"
		const parts = bench_name.split('/');
		const group_key = parts.slice(0, 2).join('/'); // "parse/svelte"
		// Prefer the display name when we have the tracking map; fall back
		// to the tracking_key suffix otherwise.
		const label = tracking_to_display.get(bench_name) ?? parts[2] ?? 'unknown';

		if (!grouped.has(group_key)) {
			grouped.set(group_key, new Map());
		}
		grouped.get(group_key)!.set(label, entry);
	}

	// Pad column widths consistently across all groups so impl names line up.
	let max_label_len = 0;
	for (const impls of grouped.values()) {
		for (const label of impls.keys()) {
			if (label.length > max_label_len) max_label_len = label.length;
		}
	}

	for (const [group_name, impls] of grouped) {
		const entries = Array.from(impls.entries());
		const any_skips = entries.some(([, e]) => e.processed < e.total);
		if (!any_skips) continue;

		lines.push(`  ${group_name}:`);
		for (const [label, entry] of entries) {
			const pct = coverage_pct(entry.processed, entry.total);
			lines.push(
				`    ${label.padEnd(max_label_len)} ${entry.processed}/${entry.total} files (${pct}%)`,
			);
		}
		lines.push('');
	}

	return lines.join('\n');
}
