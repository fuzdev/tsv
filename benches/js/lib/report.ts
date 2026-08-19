/**
 * Summary report generation for benchmark results
 */

import type { BenchmarkResult } from '@fuzdev/fuz_util/benchmark_types.ts';
import { benchmark_format_number } from '@fuzdev/fuz_util/benchmark_format.ts';
import { time_format, time_unit_detect_best, TIME_UNIT_DISPLAY } from '@fuzdev/fuz_util/time.ts';

import { CANONICAL_FORMATTER_ROW, CANONICAL_PARSER_ROWS, type Language } from './types.ts';

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
	rows: ReadonlyArray<{ name: string; processed: number; total: number }>
): string {
	const parts = rows.map(
		(e) => `${e.name} ${e.processed}/${e.total} (${coverage_pct(e.processed, e.total)}%)`
	);
	return `**Coverage:** ${parts.join(', ')}`;
}

/** Create a visual bar for comparison (based on time - slower = longer bar) */
function create_bar(value: number, max: number, width = 40): string {
	const filled = Math.round((value / max) * width);
	return '█'.repeat(filled) + '░'.repeat(width - filled);
}

/**
 * The parse-only row beside the row that materializes the same AST into JS, as
 * `[internal, json]` — one pair per tsv tier. The ratio between the two IS the
 * JSON-materialization cost, which is the only thing either reader below renders.
 *
 * ONE table because this is one fact with three readers, and it used to be three
 * separate statements: which rows are internal, which json row each internal row
 * pairs with (terminal summary), and the same pairing again (markdown note). The
 * partner lookup was the fragile one — it chose the json row by testing the
 * internal row's NAME for `wasm`, so the correctness of every rendered overhead
 * ratio rested on no other internal row ever containing that substring. A third
 * tier would have paired silently to `tsv-json` and rendered a wrong number, with
 * nothing in the output looking off.
 *
 * tsv-only by ARGUMENT, not for lack of looking: no alternative parser has a
 * comparably cheap parse-only mode — oxc's JS API always serializes to cross into
 * JS (`experimentalLazy` is setup-dominated, see `oxc.ts`), and yuku's lazy
 * `parse()` has already serialized the AST to a binary buffer by the time it
 * returns (see `yuku.ts`) — so neither yields a pair that belongs here.
 */
const INTERNAL_PARSE_PAIRS: ReadonlyArray<readonly [internal: string, json: string]> = [
	['tsv-internal', 'tsv-json'],
	['tsv_wasm-internal', 'tsv_wasm-json']
];

/** The `-internal` half of `INTERNAL_PARSE_PAIRS` — the rows a group table shows separately. */
const INTERNAL_PARSE_VARIANTS: readonly string[] = INTERNAL_PARSE_PAIRS.map(
	([internal]) => internal
);

/**
 * The row materializing the same AST as `internal_name`, or `undefined` when it
 * names no pair — which a caller must treat as "render no ratio", never as a
 * default partner.
 */
function internal_parse_partner(internal_name: string): string | undefined {
	return INTERNAL_PARSE_PAIRS.find(([internal]) => internal === internal_name)?.[1];
}

/**
 * Stable display order for implementations.
 * Order: canonical → tsv variants → third-party alternatives (alphabetical)
 */
const DISPLAY_ORDER = [
	// Canonical (shown separately, but included for completeness). Taken from the
	// shared constants rather than respelled — a literal here is the same drift
	// `CANONICAL_PARSER_ROWS` exists to close, and the cost of getting it wrong is
	// the quietest kind: the row sorts last instead of first, and nothing says so.
	// Deduped because CSS and Svelte share an oracle.
	...new Set(Object.values(CANONICAL_PARSER_ROWS)),
	CANONICAL_FORMATTER_ROW,
	// tsv variants
	'tsv-json',
	'tsv-json-no-locations',
	'tsv_wasm-json',
	'tsv_wasm-json-no-locations',
	'tsv',
	'tsv_wasm',
	// Opt-in diagnostic (`BENCH_FORCED_ASYNC=1`), so it reaches no committed report
	// — but the completeness guard asks about every row the surface DEFINES, not
	// every row a default run renders, so leaving it out fired a ⚠ on each
	// forced-async run about a row that is deliberately unpublished. Listed for the
	// same reason `tsc` below is: the order is kept COMPLETE.
	'tsv-forced-async',
	// Internal variants (shown separately)
	'tsv-internal',
	'tsv_wasm-internal',
	// Third-party alternatives (alphabetical)
	'biome-wasm',
	'dprint-wasm',
	'malva-wasm',
	'oxc-parser',
	'oxc-parser-wasm',
	'oxfmt',
	'postcss',
	'rsvelte-fmt',
	'rsvelte-parse',
	'rsvelte-parse-skip-expr-loc',
	'swc',
	// Conformance-only, so it reaches no table this order sorts (the timed
	// summaries are perf-surface). Listed for the same reason the canonical rows
	// above are: an unlisted name sorts silently to the end, so the list is kept
	// COMPLETE rather than kept to what today's surfaces happen to render.
	'tsc',
	'yuku-parser',
	'yuku-parser-wasm'
];

/**
 * The names in `names` this order doesn't list.
 *
 * An unlisted name is not an error — `sort_by_display_order` below sends it to the
 * END, silently — which is exactly how a row joins a surface and sorts last
 * indefinitely, since nothing about the output looks broken. Asking the task
 * REGISTRY this question turns a hand-maintained list into a checked one, the same
 * way `SURFACE_DISCLOSURES` (bench.ts) checks its claims against the registry.
 *
 * One direction only. A LISTED name absent from `names` is NOT drift: each surface
 * registers its own subset (`tsc` rides the conformance surface alone), so the
 * reverse check would fire on every run of the other surface — while this direction
 * is answerable per surface and self-completes across the two.
 *
 * The caller asks it of the rows a surface DEFINES (`get_defined_rows`), not of the
 * rows a machine could run, so the answer is a property of the code rather than of
 * the box — and an opt-in row (`tsv-forced-async`, `BENCH_FORCED_ASYNC=1`) is asked
 * about too, which is why the order lists it.
 */
export function rows_missing_from_display_order(names: Iterable<string>): string[] {
	return [...names].filter((name) => !DISPLAY_ORDER.includes(name));
}

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
	languages: Language[]
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

	// Parse performance comparison. The `*-json-no-locations` rows render as
	// ordinary bars; a curated "no-locations vs oxc" line is appended per language
	// (see below) — that's the payload-matched comparison (same span-only shape),
	// where plain `tsv-json` carries the richer loc-bearing drop-in AST.
	lines.push('');
	lines.push('Parse Performance:');
	for (const lang of languages) {
		const results = get_group_results(`parse/${lang}`);
		if (results.length === 0) continue;

		const canonical_name = CANONICAL_PARSER_ROWS[lang];
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
				`    ${canonical_result.name.padEnd(max_name_len)} ${create_bar(
					canonical_result.stats.mean_ns,
					max_time
				)} ${fmt(canonical_result.stats.mean_ns)}`
			);
		}

		// Show alternatives in stable display order (tsv variants, then third-party)
		const alternatives = sort_by_display_order(
			main_results.filter((r) => r.name !== canonical_name)
		);

		for (const result of alternatives) {
			const comparison = format_comparison(baseline, result.stats.mean_ns);
			lines.push(
				`    ${result.name.padEnd(max_name_len)} ${create_bar(result.stats.mean_ns, max_time)} ${fmt(
					result.stats.mean_ns
				)} ${comparison}`
			);
		}

		// Curated apples-to-apples lines: each pair is a tsv wire beside the ONE
		// opponent whose product it actually matches, with the note naming what makes
		// the match. Emitted only where both rows exist, so a pair costs nothing on a
		// surface neither runs on.
		//
		// ⚠ The THIRD hand-maintained row list in this module, and the one no
		// registry guard covers — deliberately, because unlike `DISPLAY_ORDER` and
		// `COMPARISON_SECTIONS` its membership is an ARGUMENT rather than a
		// completeness claim: most rows have no payload-matched partner and never
		// will, so "every registered row appears or is excused" is not the
		// invariant. An impl added to the harness still has to be considered here;
		// `swc` and `postcss` were, and are absent on purpose (docs/benchmarks.md
		// §Fairness caveats states why for each).
		//
		// - The span-only pairs: tsv's `no-locations` wire against the span-only
		//   default ASTs oxc and yuku emit (plain `tsv-json` carries the richer
		//   loc-bearing drop-in AST both omit; yuku pads
		//   `decorators`/`typeAnnotation`/`optional` exactly as oxc does, so the two
		//   opponents are payload-matched to each other too). TS/JS only — neither
		//   parses svelte or css.
		// - The Svelte pair: rsvelte's parser is the only third-party engine there,
		//   and it matches `tsv-json` on BOTH axes — mechanism (each returns a compact
		//   JSON string the caller parses, so both pay the identical serialize +
		//   boundary + JSON.parse cost) and payload (within ~1.5% of tsv's bytes on a
		//   real component). Deliberately NOT paired with the no-locations rows:
		//   `skipExpressionLoc` is a different reduction from tsv's span-only wire
		//   (see lib/rsvelte_parse.ts), which is also why that row is named for its
		//   option rather than for tsv's.
		for (const [ours, opponent, note] of [
			['tsv-json-no-locations', 'oxc-parser', 'payload-matched, span-only'],
			['tsv-json-no-locations', 'yuku-parser', 'payload-matched, span-only'],
			['tsv_wasm-json-no-locations', 'oxc-parser-wasm', 'payload-matched, span-only'],
			['tsv_wasm-json-no-locations', 'yuku-parser-wasm', 'payload-matched, span-only'],
			['tsv-json', 'rsvelte-parse', 'mechanism- and payload-matched, full AST']
		] as const) {
			const ours_result = results.find((r) => r.name === ours);
			const opponent_result = results.find((r) => r.name === opponent);
			if (ours_result && opponent_result) {
				lines.push(
					`      ↳ ${ours} vs ${opponent}: ${format_comparison(
						opponent_result.stats.mean_ns,
						ours_result.stats.mean_ns
					)} (${note})`
				);
			}
		}

		// Show internal variants (JSON overhead measurement)
		for (const internal_result of sort_by_display_order(internal_results)) {
			// The json row materializing the same AST, by table rather than by name shape.
			const json_name = internal_parse_partner(internal_result.name);
			const json_result =
				json_name === undefined ? undefined : results.find((r) => r.name === json_name);

			if (json_result) {
				const json_overhead = json_result.stats.mean_ns / internal_result.stats.mean_ns;
				lines.push(
					`    ${internal_result.name.padEnd(max_name_len)} ${create_bar(
						internal_result.stats.mean_ns,
						max_time
					)} ${fmt(internal_result.stats.mean_ns)} (${json_overhead.toFixed(1)}x JSON overhead)`
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

		const canonical_result = results.find((r) => r.name === CANONICAL_FORMATTER_ROW);
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
			`    ${canonical_result.name.padEnd(max_name_len)} ${create_bar(
				canonical_result.stats.mean_ns,
				max_time
			)} ${fmt(canonical_result.stats.mean_ns)}`
		);

		// Show alternatives in stable display order (tsv variants, then third-party)
		const alternatives = sort_by_display_order(
			results.filter((r) => r.name !== CANONICAL_FORMATTER_ROW)
		);

		for (const result of alternatives) {
			const comparison = format_comparison(baseline, result.stats.mean_ns);
			lines.push(
				`    ${result.name.padEnd(max_name_len)} ${create_bar(result.stats.mean_ns, max_time)} ${fmt(
					result.stats.mean_ns
				)} ${comparison}`
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
	task_tracking_by_group?: Map<string, Map<string, string>>
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
		const prefix =
			benchmarks.length === 1 ? 'Failed in' : `Failed in ${benchmarks.length} benchmarks`;
		lines.push(`  ${prefix}: ${failed_in}`);
		lines.push('');
	}

	if (sorted_errors.length > 10) {
		lines.push(`  ... and ${sorted_errors.length - 10} more (sorted rarest failure-set first)`);
	}

	return lines.join('\n');
}

/** The five canonical-oracle versions every report carries. */
export interface CanonicalVersionInfo {
	svelte: string;
	acorn: string;
	acorn_ts: string;
	prettier: string;
	prettier_svelte: string;
}

/**
 * The alternative-impl versions a report carries — each optional, since an impl
 * that didn't load contributes nothing. Mirrors `get_alternative_versions`.
 */
export interface AlternativeVersionInfo {
	oxc_parser?: string;
	oxfmt?: string;
	yuku_parser?: string;
	yuku_parser_wasm?: string;
	biome?: string;
	dprint?: string;
	malva?: string;
	postcss?: string;
	rsvelte_fmt?: string;
	rsvelte_parse?: string;
	/**
	 * The upstream Svelte version rsvelte's parse addon targets (its own `VERSION`
	 * export), which is NOT its package version. Rendered alongside it because a
	 * drift from the harness's `svelte` pin means that row parses to a different
	 * Svelte than the `svelte/compiler` oracle it sits next to.
	 */
	rsvelte_parse_svelte_target?: string;
	swc?: string;
}

/** Everything the versions blocks render. */
export type ReportVersions = CanonicalVersionInfo & AlternativeVersionInfo;

/**
 * Version parts for yuku's two packages. They ship one engine behind two
 * bindings and version in lockstep upstream, so the matched case collapses to a
 * single `yuku-parser@X`; a skewed local install prints both, making the skew
 * legible in the report rather than silently comparing two engines.
 */
function yuku_version_parts(versions: AlternativeVersionInfo): string[] {
	const { yuku_parser, yuku_parser_wasm } = versions;
	if (yuku_parser && yuku_parser_wasm && yuku_parser === yuku_parser_wasm) {
		return [`yuku-parser@${yuku_parser}`];
	}
	const parts: string[] = [];
	if (yuku_parser) parts.push(`yuku-parser@${yuku_parser}`);
	if (yuku_parser_wasm) parts.push(`@yuku-parser/wasm@${yuku_parser_wasm}`);
	return parts;
}

/**
 * Every alternative impl's version label, in report order — the ONE list both
 * versions blocks render (the terminal run's and the committed markdown's).
 *
 * They were two hand-maintained copies and had already drifted: `@rsvelte/fmt`
 * reached the markdown but never the terminal block, and each impl added since
 * widened the seam. An impl absent from the run contributes nothing, so callers
 * can append the result unconditionally.
 */
export function alternative_version_parts(versions: AlternativeVersionInfo): string[] {
	const parts: string[] = [];
	if (versions.oxc_parser) parts.push(`oxc-parser@${versions.oxc_parser}`);
	if (versions.oxfmt) parts.push(`oxfmt@${versions.oxfmt}`);
	parts.push(...yuku_version_parts(versions));
	if (versions.biome) parts.push(`@biomejs/wasm-bundler@${versions.biome}`);
	if (versions.dprint) parts.push(`@dprint/typescript@${versions.dprint}`);
	if (versions.malva) parts.push(`dprint-plugin-malva@${versions.malva}`);
	if (versions.postcss) parts.push(`postcss@${versions.postcss}`);
	if (versions.rsvelte_fmt) parts.push(`@rsvelte/fmt@${versions.rsvelte_fmt}`);
	if (versions.rsvelte_parse) {
		// The addon's version plus the upstream Svelte it targets — see
		// `AlternativeVersionInfo.rsvelte_parse_svelte_target`.
		const target = versions.rsvelte_parse_svelte_target;
		parts.push(
			`@rsvelte/vite-plugin-svelte-native@${versions.rsvelte_parse}` +
				(target ? ` (targets svelte@${target})` : '')
		);
	}
	if (versions.swc) parts.push(`@swc/core@${versions.swc}`);
	return parts;
}

/**
 * Versions block for the terminal run. (Corpus counts already print at the
 * top of the run, so this used to duplicate them — now versions only.)
 */
export function generate_versions_info(versions: ReportVersions): string {
	const lines: string[] = [];
	lines.push('');
	lines.push('-'.repeat(80));
	lines.push('Versions:');
	lines.push(
		`  svelte@${versions.svelte}, acorn@${versions.acorn}, @sveltejs/acorn-typescript@${versions.acorn_ts}`
	);
	lines.push(`  prettier@${versions.prettier}, prettier-plugin-svelte@${versions.prettier_svelte}`);

	const alt_versions = alternative_version_parts(versions);
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

/**
 * A fairness note, in the two wordings the two surfaces take: the terminal's
 * hand-wrapped lines and the markdown's single sentence (joined into the trailing
 * `_…._` paragraph). Rendered only when its opponent produced at least one cell,
 * and deduped by identity — one note covers a native/wasm opponent pair, and must
 * not print twice when both ran.
 */
interface FairnessNote {
	terminal: readonly string[];
	markdown: string;
}

const OXFMT_NOTE: FairnessNote = {
	terminal: [
		'  (oxfmt formats JS/TS natively; its css/svelte rows route through its BUNDLED',
		'   prettier — native-vs-native reads apply to the typescript group only)'
	],
	markdown:
		'oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only'
};

const OXC_NOTE: FairnessNote = {
	terminal: [
		'  (oxc-parser — native and wasm — serializes the AST to JSON in Rust and',
		'   deserializes in JS, the same eager materialization as tsv-json — apples-to-apples)'
	],
	markdown:
		'oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples'
};

const YUKU_NOTE: FairnessNote = {
	terminal: [
		'  (yuku-parser — native and wasm — decodes a binary AST buffer into JS objects,',
		'   also full eager materialization; its parse() is lazy, so the bench forces it —',
		'   verified: no lazy accessors survive, and its tree serializes to oxc-parser’s size)'
	],
	markdown:
		'yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built'
};

const SWC_NOTE: FairnessNote = {
	terminal: [
		'  (swc parses to its own AST dialect — root `Module`, `span` rather than `loc` —',
		'   so like oxc it emits neither tsv’s loc-bearing drop-in shape nor its span-only wire)'
	],
	markdown:
		'swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire'
};

const RSVELTE_PARSE_NOTE: FairnessNote = {
	terminal: [
		'  (rsvelte-parse returns a JSON string the caller parses — the identical mechanism',
		'   tsv-json measures, and within ~1.5% of its payload, so this pair matches on both axes)'
	],
	markdown:
		'rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload on a real component, so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire'
};

const POSTCSS_NOTE: FairnessNote = {
	terminal: [
		'  (postcss is the JS parser behind prettier’s CSS printer — i.e. behind the',
		'   format/css baseline; no Rust CSS parser exposes an AST to JS, so it is the only peer)'
	],
	markdown:
		'postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS'
};

const MALVA_NOTE: FairnessNote = {
	terminal: [
		'  (malva-wasm is dprint’s CSS plugin over the same @dprint/formatter wasm host as',
		'   dprint-wasm — a same-tier wasm-vs-wasm read on format/css)'
	],
	markdown:
		'malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`'
};

/**
 * One opponent a section compares against: the row to look up, plus the fairness
 * note its presence earns.
 *
 * `row` is a per-language record where the opponent's row name varies by language —
 * only the canonical parser does (`CANONICAL_PARSER_ROWS`), which is why the type
 * admits exactly that shape rather than an arbitrary resolver.
 */
interface ComparisonOpponent {
	row: string | Record<Language, string>;
	note?: FairnessNote;
}

/** The row `opponent` names in `lang`'s group. */
function opponent_row(opponent: ComparisonOpponent, lang: Language): string {
	return typeof opponent.row === 'string' ? opponent.row : opponent.row[lang];
}

/**
 * One section of the Comparisons tables: a tsv row, and every opponent it is
 * measured against per operation.
 *
 * DECLARED rather than open-coded, and this is the whole point of the shape. The
 * four loops this replaced spelled their opponent lists inline, so an impl added to
 * the harness had to be remembered in one of four places to reach the table — and
 * four weren't: `swc`, `postcss`, `rsvelte-parse` and `malva-wasm` were each
 * registered, preflighted and timed at full coverage, then dropped from every
 * comparison with nothing saying so. `rows_missing_from_comparisons` is the
 * guard that keeps it from happening again; this table is what that guard can read.
 *
 * Sections are TIERS — tsv's native binding and its wasm bundle — so a wasm engine
 * belongs under `tsv_wasm` and a native one under `tsv`. The two JS opponents
 * (`prettier`/the canonical parsers, and `postcss`) appear under BOTH: a JS
 * reference is equally meaningful against either build, which is the reading
 * prettier has always had here.
 */
interface ComparisonSectionSpec {
	/** Section heading, and the tsv row every ratio in it is against. */
	label: string;
	/** The tsv row this section measures, per operation. */
	self: Record<'format' | 'parse', string>;
	/** Opponents in render order; one absent from a run contributes no cell. */
	opponents: Record<'format' | 'parse', readonly ComparisonOpponent[]>;
}

const COMPARISON_SECTIONS: readonly ComparisonSectionSpec[] = [
	{
		label: 'tsv',
		self: { format: 'tsv', parse: 'tsv-json' },
		opponents: {
			format: [{ row: CANONICAL_FORMATTER_ROW }, { row: 'oxfmt', note: OXFMT_NOTE }],
			parse: [
				{ row: CANONICAL_PARSER_ROWS },
				{ row: 'oxc-parser', note: OXC_NOTE },
				{ row: 'yuku-parser', note: YUKU_NOTE },
				{ row: 'swc', note: SWC_NOTE },
				{ row: 'rsvelte-parse', note: RSVELTE_PARSE_NOTE },
				{ row: 'postcss', note: POSTCSS_NOTE }
			]
		}
	},
	{
		label: 'tsv_wasm',
		self: { format: 'tsv_wasm', parse: 'tsv_wasm-json' },
		opponents: {
			format: [
				{ row: CANONICAL_FORMATTER_ROW },
				{ row: 'biome-wasm' },
				// dprint is TypeScript/JS-only, so it contributes a cell on that language
				// alone — the same-tier WASM-vs-WASM read (docs/benchmarks.md §Fairness caveats).
				{ row: 'dprint-wasm' },
				{ row: 'malva-wasm', note: MALVA_NOTE }
			],
			parse: [
				{ row: CANONICAL_PARSER_ROWS },
				{ row: 'oxc-parser-wasm', note: OXC_NOTE },
				{ row: 'yuku-parser-wasm', note: YUKU_NOTE },
				{ row: 'postcss', note: POSTCSS_NOTE }
			]
		}
	}
];

/**
 * Rows that carry no comparison cell BY DECISION, each with the decision.
 *
 * The other half of `rows_missing_from_comparisons`: a registered row is either an
 * opponent, a section's own `self`, or listed here. Without this list the guard
 * could only be a warning nobody could ever clear, since several rows legitimately
 * belong in no comparison — and "legitimately absent" and "forgotten" would stay
 * indistinguishable, which is the state that let four impls go missing.
 *
 * UNCHECKED in the reverse direction, and that is a decision rather than an
 * oversight: a renamed or deleted row leaves a dead entry here and nothing fires.
 * The check that would catch it cannot be written from inside a run, because the
 * row universe is SURFACE-scoped — `tsc` is registered only on the conformance
 * surface, `tsv-forced-async` only under `BENCH_FORCED_ASYNC=1` — so asking "does
 * every key here name a registered row?" of one surface's registry would indict the
 * entries the other surface owns, on every run. Same one-directional shape as
 * `DISPLAY_ORDER`, for the same reason.
 *
 * This is also where the list differs from `PERF_OMITS`'s two-direction ratchet,
 * which grades its own entries for staleness: an omit names a TASK, and a run can
 * observe whether that task was reachable (`graded_keys`), so "matched nothing"
 * separates cleanly from "was never asked". A row's existence on the OTHER surface
 * has no such per-run signal here — the two surfaces never share a process.
 */
const COMPARISON_EXCLUSIONS: Readonly<Record<string, string>> = {
	'tsv-json-no-locations': "tsv's own wire variant — a row of the group table, not an opponent",
	'tsv_wasm-json-no-locations':
		"tsv's own wire variant — a row of the group table, not an opponent",
	'tsv-internal': "tsv's own parse-only variant; no third-party row is the same tier",
	'tsv_wasm-internal': "tsv's own parse-only variant; no third-party row is the same tier",
	'tsv-forced-async': 'opt-in async-tax control (`BENCH_FORCED_ASYNC=1`), deliberately unpublished',
	'rsvelte-fmt': 'coverage-only — never timed, so there is no ratio to take',
	tsc: 'conformance surface only — a verdict row, never timed',
	'rsvelte-parse-skip-expr-loc':
		'its reduction is not tsv’s span-only wire, so it is not payload-matched to any tsv row; the plain `rsvelte-parse` row carries the engine'
};

/**
 * The rows in `names` this module neither compares nor excuses.
 *
 * Asked of the rows a surface DEFINES (`get_defined_rows`), so the answer is a
 * property of the code rather than of the machine — the same question, and the same
 * one-directional shape, as `rows_missing_from_display_order`: a LISTED opponent
 * absent from `names` is not drift, since each surface registers its own subset.
 *
 * Warns rather than throws at the call site, matching `DISPLAY_ORDER`'s severity: a
 * missing cell understates a comparison, where a stale `SURFACE_DISCLOSURES` claim
 * would assert something false.
 */
export function rows_missing_from_comparisons(names: Iterable<string>): string[] {
	const covered = new Set<string>(Object.keys(COMPARISON_EXCLUSIONS));
	for (const section of COMPARISON_SECTIONS) {
		for (const operation of ['format', 'parse'] as const) {
			covered.add(section.self[operation]);
			for (const opponent of section.opponents[operation]) {
				if (typeof opponent.row === 'string') covered.add(opponent.row);
				else for (const row of Object.values(opponent.row)) covered.add(row);
			}
		}
	}
	return [...names].filter((name) => !covered.has(name));
}

/** Resolve the iterated file count for a (group, display_name) pair via task_tracking. */
function lookup_iterated(
	group_name: string,
	display_name: string,
	iterated_counts: Map<string, number> | undefined,
	task_tracking_by_group: Map<string, Map<string, string>> | undefined
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
	baseline: string | undefined
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
	// "sweeps/sec", not "ops/sec": one timed iteration is one full pass over the
	// group's iterated file set, so every absolute column here (rate, percentiles,
	// min/max) is per-SWEEP — a reader wanting per-file figures divides by the
	// group's file count. Ratios and MB/s are denominated consistently either way.
	rows.push([
		'Task Name',
		'sweeps/sec',
		'n',
		`p50 (${unit_str})`,
		`p75 (${unit_str})`,
		`p90 (${unit_str})`,
		`p95 (${unit_str})`,
		`p99 (${unit_str})`,
		`min (${unit_str})`,
		`max (${unit_str})`,
		vs_header
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
			vs_cell
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
 * Build comparison data from benchmark results, one pass over
 * `COMPARISON_SECTIONS`.
 *
 * Ratios are computed from timed ops/sec — in default `intersection` mode the
 * comparison is apples-to-apples within each group (every impl ran on the
 * same files). The `(Mf)` annotation is the self impl's iterated file count
 * for that group (the per-group intersection size in default mode; the
 * impl's preflight success set size in `BENCH_MODE=union`).
 *
 * A row is emitted only when the section's own tsv row AND the section's FIRST
 * opponent both timed — the first opponent is the canonical reference in every
 * section, so a group without it has no baseline to compare against. Every later
 * opponent contributes a cell iff it timed, which is how a language-scoped tool
 * (dprint on TS, malva on CSS) appears on its language alone with no special case.
 */
function build_comparison_data(
	all_group_results: GroupResults[],
	languages: Language[],
	iterated_counts: Map<string, number> | undefined,
	task_tracking_by_group: Map<string, Map<string, string>> | undefined
): ComparisonSection[] {
	function get_mean_ns(group_name: string, task_name: string): number | null {
		const group = all_group_results.find((g) => g.name === group_name);
		if (!group) return null;
		const result = group.results.find((r) => r.name === task_name);
		return result?.stats.mean_ns ?? null;
	}

	function ratio(self_ns: number, other_ns: number): number {
		return other_ns / self_ns;
	}

	const sections: ComparisonSection[] = [];

	for (const spec of COMPARISON_SECTIONS) {
		const rows: ComparisonRow[] = [];
		for (const operation of ['format', 'parse'] as const) {
			const self_row = spec.self[operation];
			for (const lang of languages) {
				const group_name = `${operation}/${lang}`;
				const self_ns = get_mean_ns(group_name, self_row);
				if (self_ns === null) continue;

				const opponents = spec.opponents[operation];
				// A section with no opponents is a COMPARISON_SECTIONS mistake rather
				// than a run condition — there is no reference to gate on below.
				if (opponents.length === 0) continue;

				const comparisons: ComparisonRow['comparisons'] = [];
				for (const opponent of opponents) {
					const name = opponent_row(opponent, lang);
					const opponent_ns = get_mean_ns(group_name, name);
					if (opponent_ns === null) continue;
					comparisons.push({ name, ratio: ratio(self_ns, opponent_ns) });
				}
				// Gated on the FIRST opponent specifically, not on "any cell survived":
				// the first is the canonical reference in every section, and without it
				// there is nothing to be N× faster THAN — an alternatives-only row
				// would read as a comparison while naming no baseline. Cells are pushed
				// in opponent order, so the reference timed iff it leads the list.
				if (comparisons[0]?.name !== opponent_row(opponents[0], lang)) continue;

				rows.push({
					operation,
					language: lang,
					files: lookup_iterated(group_name, self_row, iterated_counts, task_tracking_by_group),
					comparisons
				});
			}
		}
		if (rows.length > 0) sections.push({ label: spec.label, rows });
	}

	return sections;
}

/**
 * The fairness notes this run earned, in `surface`'s wording.
 *
 * DERIVED from the built sections rather than from a second list of row names: a
 * note explains a cell, so it must appear exactly when that cell does. As two
 * hand-kept predicates the presence tests had already drifted in shape (the oxc
 * test pinned the section label, the yuku test didn't), and a note silently
 * missing from one surface is the exact failure the notes exist to prevent.
 *
 * Presence is asked of ALL sections at once, with no like-tier check of its own —
 * which the oxc predicate used to carry, so that a cross-tier oxc cell couldn't
 * earn the apples-to-apples note. That check is now structural: `COMPARISON_SECTIONS`
 * assigns `oxc-parser` to the native section and `oxc-parser-wasm` to the wasm one,
 * so a cross-tier cell cannot be built. Move a row between tiers and the guarantee
 * moves with it — re-tier the note's opponent entry too.
 *
 * Deduped by note identity, since one note covers a native/wasm opponent pair and
 * must print once when both ran.
 */
function comparison_notes(
	sections: ComparisonSection[],
	surface: 'terminal' | 'markdown'
): string[] {
	const present = new Set<string>();
	for (const section of sections) {
		for (const row of section.rows) for (const c of row.comparisons) present.add(c.name);
	}

	const seen = new Set<FairnessNote>();
	const notes: string[] = [];
	for (const spec of COMPARISON_SECTIONS) {
		for (const operation of ['format', 'parse'] as const) {
			for (const opponent of spec.opponents[operation]) {
				if (!opponent.note || seen.has(opponent.note)) continue;
				const rows =
					typeof opponent.row === 'string' ? [opponent.row] : Object.values(opponent.row);
				if (!rows.some((row) => present.has(row))) continue;
				seen.add(opponent.note);
				if (surface === 'markdown') notes.push(opponent.note.markdown);
				else notes.push(...opponent.note.terminal);
			}
		}
	}

	// The one note that is about an ABSENCE rather than about a row, so it hangs
	// off no opponent: tsv's parse-only variants have no counterpart among the
	// eager-materializing third-party parsers, which is only worth saying once one
	// of them is on the page.
	//
	// Gated on the NOTES those two rows earned, not on a re-spelled list of their
	// four row names: this note's text is about oxc and yuku specifically, so the
	// question it needs answered is the one `seen` already holds — did their notes
	// print? A parallel row list here would be a fifth spelling of the same
	// membership, free to drift from the opponents that define it.
	if (seen.has(OXC_NOTE) || seen.has(YUKU_NOTE)) {
		if (surface === 'markdown') {
			notes.push(
				'tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier'
			);
		} else {
			notes.push(
				'  (tsv-internal/tsv_wasm-internal are parse-only, no JS materialization; neither',
				'   oxc nor yuku has a matching mode, so they have no counterpart row)'
			);
		}
	}

	return notes;
}

/**
 * Which canonical parser backs which language's baseline —
 * `svelte/compiler for svelte + css, acorn-typescript for typescript` — inverted
 * out of `CANONICAL_PARSER_ROWS` rather than spelled out.
 *
 * The note it feeds used to name FILE EXTENSIONS (`.svelte`/`.css` vs `.ts`), which
 * read naturally but left this module's last hand-written canonical-parser fact
 * sitting one line under the tables the record now names — the same drift
 * `CANONICAL_PARSER_ROWS` exists to close, reachable here because the extensions
 * are not in it and so nothing could check the sentence. Languages are what the
 * record holds AND what the tables key on (`parse svelte`, `parse css`), so the
 * note now joins against the rows beside it instead of describing them from
 * memory.
 *
 * Takes the run's languages, so a filtered run (`BENCH_FILTER`) describes the
 * baselines it actually rendered rather than the full set.
 */
function canonical_parser_note(languages: readonly Language[]): string {
	const by_row: Map<string, Language[]> = new Map();
	for (const lang of languages) {
		const row = CANONICAL_PARSER_ROWS[lang];
		const langs = by_row.get(row);
		if (langs) langs.push(lang);
		else by_row.set(row, [lang]);
	}
	return [...by_row].map(([row, langs]) => `${row} for ${langs.join(' + ')}`).join(', ');
}

/**
 * Generate compact comparison summary (plain text).
 *
 * Ratios are speedup form (other_time / self_time): >1 means tsv is faster.
 * Each opponent is named by its ROW, the canonical parser included — it is
 * `acorn-typescript` on the TS group and `svelte/compiler` on the other two, and
 * a single hardcoded label there named the wrong tool on one group of three.
 * Each cell carries an `(Mf)` annotation — the iterated file count timing
 * reflects.
 */
export function generate_comparison_summary(
	all_group_results: GroupResults[],
	languages: Language[],
	iterated_counts?: Map<string, number>,
	task_tracking_by_group?: Map<string, Map<string, string>>
): string {
	const sections = build_comparison_data(
		all_group_results,
		languages,
		iterated_counts,
		task_tracking_by_group
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

	// The notes that hold whatever ran, then the ones derived from the cells this
	// run actually produced — general before specific, which is also why the
	// per-opponent notes come last rather than being interleaved.
	lines.push('');
	lines.push('  (`Nx` = self is N× faster; `(Mf)` = files the timing reflects)');
	lines.push(`  (parse canonical: ${canonical_parser_note(languages)})`);
	lines.push('  (format groups include parse time — each formatter parses internally)');
	lines.push(...comparison_notes(sections, 'terminal'));

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
	task_tracking_by_group?: Map<string, Map<string, string>>
): string | null {
	const sections = build_comparison_data(
		all_group_results,
		languages,
		iterated_counts,
		task_tracking_by_group
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

	// The notes that hold whatever ran, then the ones derived from the cells this
	// run actually produced — same order as the terminal surface, general before
	// specific.
	const notes: string[] = [
		'`Nx` is speedup — self is N× faster than the named opponent',
		"`(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`)",
		`Parse canonical: ${canonical_parser_note(languages)} — each named by its own row`,
		'Format groups include parse time — each formatter parses internally',
		...comparison_notes(sections, 'markdown')
	];

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
	effective_corpus_bytes: Map<string, number>
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
	iterated_counts: Map<string, number> | undefined
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
	effective_corpus_size: Map<string, EffectiveCorpusEntry>
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
 * Per-group line for the impls that were measured for coverage but never timed
 * (`BenchmarkTask.coverage_only`) — an impl with no in-process API, whose timed
 * row would rank process spawn rather than format work.
 *
 * Always emitted when such an impl ran, including at 100%: unlike
 * `generate_group_coverage_markdown` (where a line means "some impl skipped
 * files"), coverage IS the entire measurement here, so suppressing it at 100%
 * would erase the row. Carries its own inline reason so a reader meeting an
 * untimed name in a throughput report never has to hunt for why.
 */
export function generate_group_coverage_only_markdown(
	names: readonly string[],
	tracking: Map<string, string> | undefined,
	effective_corpus_size: Map<string, EffectiveCorpusEntry>
): string | null {
	if (!tracking || names.length === 0) return null;
	const rows: { name: string; processed: number; total: number }[] = [];
	for (const name of names) {
		const tracking_key = tracking.get(name);
		if (!tracking_key) continue;
		const e = effective_corpus_size.get(tracking_key);
		if (!e) continue;
		rows.push({ name, processed: e.processed, total: e.total });
	}
	if (rows.length === 0) return null;
	const parts = rows.map(
		(e) => `${e.name} ${e.processed}/${e.total} (${coverage_pct(e.processed, e.total)}%)`
	);
	return (
		`**Coverage-only (not timed):** ${parts.join(', ')} — no in-process API, so a timed row ` +
		`would measure process spawn rather than format work; these are accept rates, not speeds.`
	);
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
	effective_corpus_size: Map<string, EffectiveCorpusEntry>
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

/** One impl's coverage over one corpus source, for the conformance breakdown. */
export interface SourceCoverageCell {
	processed: number;
	total: number;
}

/** Per-group, per-source, per-impl coverage: `group → source → impl name → cell`. */
export type CoverageBySource = Map<string, Map<string, Map<string, SourceCoverageCell>>>;

/**
 * Per-SOURCE coverage tables for the conformance report — the aggregate
 * `**Coverage:**` line split by corpus entry.
 *
 * Why it exists: a group's headline blends sources that answer different
 * questions. `parse/typescript` is ~83% test262 (ECMAScript), so a TypeScript
 * parse gap moves the aggregate by tenths of a point; and on the tsc corpus the
 * `tsc` row is the ORACLE (100% by construction — the harvest keeps exactly what
 * that parser accepts), which is only honest to read source by source. Same reason
 * the report separates coverage from throughput rather than averaging them.
 *
 * A source is omitted for an impl that never saw it (a language it doesn't
 * support), and a whole source is omitted when no impl has data. Rows are the
 * sources in corpus order; columns are the impls in display order.
 *
 * The same-engine variant columns (`tsv-json` / `tsv-internal` /
 * `tsv_wasm-*`, `oxc-parser` / `oxc-parser-wasm`) look redundant and are
 * deliberately kept: they read identically only while the *bindings and payloads*
 * agree, which is a claim, not a given — a wire-writer failure would show as
 * `-json` trailing `-internal`, and a broken binding error surface as a native/wasm
 * split (the oxc WASI consume-once bug, `bench.ts` `check_variant_parity`). Folding
 * the columns per engine would erase exactly that signal, per source, where it is
 * most legible.
 */
export function generate_coverage_by_source_markdown(
	languages: readonly Language[],
	operations: readonly ('parse' | 'format')[],
	coverage_by_source: CoverageBySource
): string[] {
	const lines: string[] = [];
	for (const language of languages) {
		for (const operation of operations) {
			const group_name = `${operation}/${language}`;
			const by_source = coverage_by_source.get(group_name);
			if (!by_source || by_source.size === 0) continue;
			const impl_names: string[] = [];
			for (const cells of by_source.values()) {
				for (const name of cells.keys()) if (!impl_names.includes(name)) impl_names.push(name);
			}
			if (impl_names.length === 0) continue;
			lines.push(`### ${group_name} by corpus source\n`);
			lines.push(`| Source | Files | ${impl_names.join(' | ')} |`);
			lines.push(`| --- | ---: | ${impl_names.map(() => '---:').join(' | ')} |`);
			for (const [source, cells] of by_source) {
				const total = [...cells.values()][0]?.total ?? 0;
				const columns = impl_names.map((name) => {
					const cell = cells.get(name);
					if (!cell) return '—';
					return `${cell.processed} (${coverage_pct(cell.processed, cell.total)}%)`;
				});
				lines.push(`| \`${source}\` | ${total} | ${columns.join(' | ')} |`);
			}
			lines.push('');
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
	// The pairs, and the argument for why they are tsv-only, live on
	// `INTERNAL_PARSE_PAIRS` — this note is one of its three readers.
	const notes: string[] = [];
	for (const [internal_name, json_name] of INTERNAL_PARSE_PAIRS) {
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
 * Measured ratios for the reconstruct-vs-materialize note below. NOT computed
 * from bench rows (there is no reconstruct benchmark row) — the source of truth
 * is `benches/js/diagnostics/reconstruct_vs_materialize.ts`. Refresh these when
 * that diagnostic's number moves materially. `full` = reconstruct ALL `loc` in
 * JS; `loc_free` = the loc-sparse/free ceiling. TypeScript (exact), perf corpus.
 */
const RECONSTRUCT_VS_MATERIALIZE = { full: '~1.7x', loc_free: '~2.2x' } as const;

/**
 * One-line consumer-side note: for a JS consumer that needs full `loc`, fetching
 * the span-only `no-locations` wire and reconstructing `loc` in JS (via the
 * shipped `reconstruct_locations` helper) beats fetching the full loc-bearing
 * `tsv-json` wire end-to-end — the full wire's `loc` bytes cost real `JSON.parse`
 * tokenization, while a line-start table + binary search is cheaper. So
 * pre-materializing `loc` in Rust is not optimal for JS consumers.
 *
 * Not a tsv impl row (there's no reconstruct benchmark) — a curated cross-ref to
 * the diagnostic that measures it; see `RECONSTRUCT_VS_MATERIALIZE`.
 */
export function generate_reconstruct_note(): string {
	const { full, loc_free } = RECONSTRUCT_VS_MATERIALIZE;
	return `_Consumer-side: for full \`loc\`, fetching the span-only \`no-locations\` wire and reconstructing \`loc\` in JS (\`reconstruct_locations\`, shipped in every parse-capable package) beats the full loc-bearing \`tsv-json\` wire end-to-end — ${full} faster reconstructing every node, ${loc_free} loc-free (TypeScript, exact; measured by \`diagnostics/reconstruct_vs_materialize.ts\`). Pre-materializing \`loc\` in Rust is not optimal for JS consumers._`;
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
 * early-stage proposals, etc.). When a file's failure set matches this
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
		`format/${lang}/wasm`
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
	task_tracking_by_group: Map<string, Map<string, string>> | undefined
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
	task_tracking_by_group?: Map<string, Map<string, string>>
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
		css: all_errors.filter((e) => e.lang === 'css').sort(sort_fn)
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
		`${all_errors.length} unique file+error combinations — Svelte ${by_lang.svelte.length}, TypeScript ${by_lang.typescript.length}, CSS ${by_lang.css.length}.\n`
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
			'_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._'
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
		return [`- \`${e.file_path}\``, `  - Error: ${display_error}`, `  - Failed in: ${failed_in}`];
	}

	function render_bucket(label: string, entries: FileError[]): void {
		if (entries.length === 0) return;
		const more =
			entries.length > TOP_N_PER_LANG
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
	task_tracking_by_group?: Map<string, Map<string, string>>
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
				`    ${label.padEnd(max_label_len)} ${entry.processed}/${entry.total} files (${pct}%)`
			);
		}
		lines.push('');
	}

	return lines.join('\n');
}
