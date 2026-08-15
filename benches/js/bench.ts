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
 * Latest results are also written to benches/js/results/report.<runtime>.{json,md} (committed
 * to git). Conformance runs (BENCH_CORPUS=conformance) tag both filenames with `conformance.`
 * before the runtime (report.conformance.<runtime>.{json,md}).
 *
 * Environment variables:
 *   BENCH_LIMIT         Limit files per language (default: all)
 *   BENCH_FILTER        Filter files by path pattern (default: none)
 *   BENCH_DURATION      Duration per benchmark in ms (default: 5000; 15000 in
 *                       conformance mode — full-corpus sweeps per iteration)
 *   BENCH_WARMUP        Warmup iterations (default: 3)
 *   BENCH_MODE          'intersection' (default) | 'union' — iteration corpus mode
 *   BENCH_CORPUS        'perf' (default) | 'conformance' — corpus + surface selector:
 *                       perf = real-world corpus, parse + format groups (every in-scope
 *                         tool must fully process it — unlisted pre-flight failures hard-fail;
 *                         see lib/perf_omit.ts);
 *                       conformance = fixtures-only view (the prettier + parse-conformance
 *                         suites, excluding the perf/real corpus; Svelte minus canonical-rejects),
 *                         parse groups only
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
import { Benchmark } from '@fuzdev/fuz_util/benchmark.ts';
import type { BenchmarkResult } from '@fuzdev/fuz_util/benchmark_types.ts';
import {
	benchmark_baseline_compare,
	benchmark_baseline_format,
	benchmark_baseline_save
} from '@fuzdev/fuz_util/benchmark_baseline.ts';
import { spawn_out } from '@fuzdev/fuz_util/process.ts';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { argv, env, exit } from 'node:process';
import { fileURLToPath } from 'node:url';
import { type CorpusSource, DevReposLoader, group_by_language } from './lib/corpus.ts';
import { enrich_source_repos } from './lib/corpus_repos.ts';
import { PERF_OMITS, perf_omit_reason } from './lib/perf_omit.ts';
import {
	canonical_parser_label,
	get_alternative_versions,
	get_benchmark_tasks,
	init_implementations,
	type UnavailableImpl
} from './lib/implementations.ts';
import {
	type CoverageBySource,
	type EffectiveCorpusEntry,
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
	generate_reconstruct_note,
	generate_skipped_files_markdown,
	generate_skipped_files_report,
	generate_summary_report,
	generate_versions_info,
	type GroupResults,
	rows_missing_from_display_order,
	type SourceCoverageCell,
	alternative_version_parts,
	type ReportVersions
} from './lib/report.ts';
import {
	type BinarySize,
	type CollectedBinarySizes,
	collect_binary_sizes,
	generate_binary_size_markdown,
	generate_binary_size_report
} from './lib/binary_sizes.ts';
import { type Language, LANGUAGES, type SourceFile } from './lib/types.ts';
import { check_executed_artifacts } from './lib/check_artifact_freshness.ts';
import { check_node_modules } from './lib/check_node_modules.ts';
import { current_machine, current_runtime, type Machine, type Runtime } from './lib/runtime.ts';

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

// Baseline statistics requested — raises the sample floors in the timed suite
// (see `min_iterations` in `run_benchmark_group`) so the Welch comparisons run
// on usable sample sizes. Costs wall clock only on baseline runs.
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
	'panicked at crates/biome_rowan'
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

/** Number of warmup iterations (default: 3; slow tasks tier down to 1 unless explicitly set) */
const BENCH_WARMUP_EXPLICIT = env_int('BENCH_WARMUP');
const BENCH_WARMUP = BENCH_WARMUP_EXPLICIT ?? 3;

/**
 * Enable the per-iteration forced-GC hook (default: off — measures realistic
 * throughput where GC happens opportunistically, matching real-world usage).
 * Set `BENCH_GC=1` to force a major GC between every iteration; useful for
 * stabilizing high-allocation workloads at the cost of penalizing efficient
 * low-allocation paths. See `docs/benchmarks.md` §Fairness caveats for the trade-off.
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

/** Which corpus/surface a report was produced from — see `BENCH_CORPUS`. */
type CorpusKind = 'perf' | 'conformance';

/**
 * Corpus + surface selector. Default `perf`: the real-world corpus view, parse
 * + format groups, writing `report.<runtime>.*` — the throughput headline.
 * `BENCH_CORPUS=conformance`: the fixtures-only corpus view (prettier suites +
 * the parse-conformance suites, disjoint from the perf/real corpus, minus the
 * Svelte files svelte/compiler rejects — see `lib/corpus.ts` `SVELTE_REJECT_CACHE`),
 * parse groups ONLY, writing `report.conformance.<runtime>.*` — the per-tool
 * parse coverage/throughput surface. Format impls are deliberately excluded there:
 * grading formatter behavior on the fixture suites is the correctness gates'
 * job (`corpus:compare:format`), and timing it would put prettier/oxfmt/biome
 * through tens of thousands of fixture files for numbers nothing consumes.
 */
const BENCH_CORPUS = env.BENCH_CORPUS;
if (BENCH_CORPUS !== undefined && BENCH_CORPUS !== 'perf' && BENCH_CORPUS !== 'conformance') {
	console.error(`Invalid BENCH_CORPUS: ${BENCH_CORPUS}. Expected 'perf' or 'conformance'.`);
	exit(1);
}
const CORPUS_MODE: CorpusKind = BENCH_CORPUS === 'conformance' ? 'conformance' : 'perf';
const IS_CONFORMANCE = CORPUS_MODE === 'conformance';

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

/** Operations measured this run — conformance is a parse-only surface. */
const OPERATIONS: ('parse' | 'format')[] = IS_CONFORMANCE ? ['parse'] : ['parse', 'format'];

/**
 * Report filename tag: `report.<tag>.{json,md}` and
 * `<timestamp>_<commit>.<tag>.{json,md}`. The conformance surface writes
 * sibling files rather than clobbering the perf reports (and stays invisible
 * to `compose_reports.ts`, which globs the exact perf filenames).
 */
const REPORT_TAG = IS_CONFORMANCE ? `conformance.${RUNTIME}` : RUNTIME;

/**
 * Duration per benchmark in ms. The default is surface-dependent: 5000 for
 * perf, 15000 in conformance mode — there each iteration is a full sweep of
 * the much larger conformance corpus, so the slow rows need the longer
 * window for a usable sample count. `BENCH_DURATION` overrides either.
 */
const BENCH_DURATION = env_int('BENCH_DURATION') ?? (IS_CONFORMANCE ? 15_000 : 5000);

/**
 * Coverage-only mode (`BENCH_COVERAGE_ONLY=1`): run pre-flight — which fully
 * determines per-tool parse coverage — and emit the report straight from it,
 * SKIPPING the timed benchmark phase entirely. That phase costs a fixed floor
 * of ≥8 full-corpus sweeps per row (3 warmup + ≥5 measured; slow tasks tier to
 * 1 warmup + ≥7 measured) no matter how low
 * `BENCH_DURATION` goes, yet the conformance surface's coverage consumers (the
 * site's per-engine table, `derive_conformance_groups`) read only the
 * pre-flight counts — so on a coverage refresh the whole timing cost is wasted.
 * Entries are emitted with null timing stats; the output stays the same
 * `report.<tag>.{json,md}` files (coverage is what a conformance report is
 * for). Orthogonal to `BENCH_CORPUS`, but only meaningful with `conformance` —
 * in perf mode the timing IS the headline.
 */
const COVERAGE_ONLY = env.BENCH_COVERAGE_ONLY === '1';
if (COVERAGE_ONLY && !IS_CONFORMANCE) {
	// Coverage-only is a conformance-surface mode. In perf mode it would skip the
	// timed phase and then overwrite the perf report (`report.<runtime>.json`) with
	// null-timing entries — corrupting the throughput headline. Reject the combo.
	console.error(
		'BENCH_COVERAGE_ONLY=1 requires BENCH_CORPUS=conformance (it is a conformance-only mode; ' +
			'running it in perf mode would overwrite the perf report with null-timing entries).'
	);
	exit(1);
}

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
const corpus_loader = new DevReposLoader(CORPUS_MODE, {
	allow_missing: env.BENCH_ALLOW_MISSING === '1'
});
// Drain `stream()` directly instead of `load()` so we skip the loader's
// own corpus summary — bench.ts prints its own tighter one below that
// includes byte counts and (when applicable) limit annotations.
const files: SourceFile[] = [];
for await (const file of corpus_loader.stream(log)) {
	files.push(file);
}
// Reify each loaded source's GitHub origin (URL + commit + subpath) so the
// report links straight to the measured code — a few cheap `git` calls.
await enrich_source_repos(corpus_loader.sources);
const by_language = group_by_language(files);

// Preserve total counts before limiting
const total_file_counts = {
	svelte: by_language.svelte.length,
	typescript: by_language.typescript.length,
	css: by_language.css.length
};

// Apply file filter and limit (`!== undefined` so an explicit BENCH_LIMIT=0
// limits to zero files instead of silently meaning "no limit" — matching
// `is_limited` below, which already treats 0 as a limited run)
function limit_files(files: SourceFile[]): SourceFile[] {
	const filtered = FILE_FILTER ? files.filter((f) => f.path.includes(FILE_FILTER)) : files;
	return MAX_FILES_PER_LANGUAGE !== undefined
		? filtered.slice(0, MAX_FILES_PER_LANGUAGE)
		: filtered;
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
	css: css_files.reduce((sum, f) => sum + f.bytes, 0)
};

/**
 * Format bytes/sec as MB/s. Always MB/s, even for sub-1-MB values
 * (renders as e.g. `0.4 MB/s`) so a column of throughput numbers scans
 * uniformly without unit-switching mid-table.
 */
function format_throughput(bytes_per_sec: number): string {
	return `${(bytes_per_sec / 1_000_000).toFixed(1)} MB/s`;
}

/**
 * Format bytes as MB with one decimal — corpus sizes, in the terminal summary and
 * in the markdown report's `**Corpus:**` line alike. Same always-MB convention as
 * `format_throughput` above, for the same reason, and ONE function because the two
 * surfaces report the same numbers: as a pair they were free to drift into
 * disagreeing about the corpus's size.
 */
function format_mb(bytes: number): string {
	return `${(bytes / 1_000_000).toFixed(1)} MB`;
}

// Compact corpus summary: file counts + MB per language + total. When
// limited, each line reads `N of M files` so the subset is obvious.
const total_files = svelte_files.length + ts_files.length + css_files.length;
const total_bytes = bytes_by_language.svelte + bytes_by_language.typescript + bytes_by_language.css;
const fmt_count = (n: number, total: number) =>
	is_limited && n !== total ? `${n} of ${total}` : `${n}`;
log(`Corpus (${CORPUS_MODE} view):`);
log(
	`  Svelte:      ${fmt_count(svelte_files.length, total_file_counts.svelte).padEnd(11)} files (${format_mb(
		bytes_by_language.svelte
	)})`
);
log(
	`  TypeScript:  ${fmt_count(ts_files.length, total_file_counts.typescript).padEnd(11)} files (${format_mb(
		bytes_by_language.typescript
	)})`
);
log(
	`  CSS:         ${fmt_count(css_files.length, total_file_counts.css).padEnd(11)} files (${format_mb(
		bytes_by_language.css
	)})`
);
log(`  Total:       ${String(total_files).padEnd(11)} files (${format_mb(total_bytes)})`);
log();

// Refuse to measure stale binaries (the `:run` tasks skip the rebuild). Which
// artifacts this runtime executes — FFI or N-API, plus that runtime's WASM target
// — is `check_executed_artifacts`'s subject; override with BENCH_STALE_OK=1.
await check_executed_artifacts();

// Friendly preflight: the canonical impls (prettier + svelte/compiler) resolve
// from the harness `node_modules`; without it, init fails with an opaque
// module-resolution error. Missing is fatal with the installer hint; stale (an
// exactly-pinned dep whose installed version isn't the pinned one) is fatal too,
// with BENCH_STALE_OK=1 as the escape — see lib/check_node_modules.ts.
await check_node_modules();

// Initialize implementations
const impls = await init_implementations({ logger: log });

/**
 * One row-composition claim about this surface, discriminated by which way it
 * points. `row` is the name `get_benchmark_tasks` registers, in both arms — that is
 * what makes the claim checkable rather than merely authored.
 */
type SurfaceDisclosure =
	/** Must NOT be registered on this surface. */
	| { row: string; direction: 'excluded'; prose: string }
	/**
	 * Must BE registered here — carrying `initialized`, which answers whether the
	 * impl behind the row came up on this machine. A row absent because its package
	 * didn't load is a machine shortfall (already recorded in `unavailable`), not a
	 * policy change, and this predicate is what draws that line. Only the `added`
	 * direction has that question to ask: an `excluded` row is absent by policy
	 * whether or not its impl loaded, so the shape doesn't carry a predicate nothing
	 * would read.
	 */
	| { row: string; direction: 'added'; initialized: () => boolean; prose: string };

/**
 * The conformance surface's row-composition disclosures: a row this surface drops,
 * or carries alone, with the reasoning a reader needs.
 *
 * The PROSE is authored (a rationale is not derivable), but the CLAIM is not: each
 * entry names the row it is about, and `surface_disclosure_lines` checks that claim
 * against the rows this surface's task REGISTRY produces before printing it. The
 * policy itself lives at the registration sites in `lib/implementations.ts`
 * (conditions on `corpus_kind`), so without that check this table is a second,
 * unlinked source of truth — and the one that gets stale, since re-enabling a row is
 * a change made there while a published report goes on claiming the row was
 * excluded. That is not hypothetical for the entry below: `benches/js/CLAUDE.md`
 * §Known Issues says to revisit yuku's exclusion on an upstream bump.
 *
 * The claim is checked against the REGISTRY rather than against the rows a run
 * measured, because those answer different questions: a corpus filter (`BENCH_LIMIT`
 * / `BENCH_FILTER`) can empty a whole group, and reading that as "the policy
 * changed" failed a partial run at report time — after its work, with nothing
 * written. A filter can only remove rows, never add one, so the registry is the
 * filter-proof form of the same question, and it can be asked before the run
 * measures anything.
 *
 * Mirrors the presence-gated disclosure notes in `lib/report.ts`
 * (`has('tsv', 'oxc-parser')`), which derive the same way.
 */
const SURFACE_DISCLOSURES: ReadonlyArray<SurfaceDisclosure> = [
	{
		row: 'yuku-parser',
		direction: 'excluded',
		prose:
			'**Excluded here:** yuku-parser (N-API) — its native binding faults the host process on ' +
			'this corpus (test262 escaped-identifier fixtures), so it cannot be measured against it. ' +
			'The WASM binding runs the same engine and carries the row; both are measured on the perf ' +
			'corpus.\n'
	},
	{
		// The mirror-image disclosure: a row present ONLY here needs saying as much
		// as one absent, and `tsc`'s reading changes by corpus source — it is the
		// oracle on the corpus it filtered, an independent parser everywhere else.
		row: 'tsc',
		direction: 'added',
		initialized: () => impls.tsc !== undefined,
		prose:
			'**Added here:** tsc — the TypeScript compiler’s own parser, a verdict rather than a ' +
			'speed, so it carries no row on the throughput surface. Its parser is error-recovering ' +
			'(`createSourceFile` never throws), so an accept means zero `parseDiagnostics`. On the ' +
			'tsc corpus it is the ORACLE that selected those files — 100% by construction, like ' +
			'svelte/compiler on the Svelte set — and an independent parser on every other source, ' +
			'which is what the per-source tables below are for. Coverage counts accepts and so ' +
			'cannot show over-acceptance; that axis is `deno task ts-repo:over-acceptance`.\n'
	}
];

/**
 * Every row name this surface's registry produces, over every operation+language
 * this run covers — the policy question, asked of `get_benchmark_tasks` directly
 * and independent of what the corpus happens to contain. Pure construction (each
 * task is a name plus a closure), so building the set costs nothing and runs before
 * a single file is read.
 */
function registered_row_names(): Set<string> {
	const names = new Set<string>();
	for (const operation of OPERATIONS) {
		for (const language of LANGUAGES) {
			for (const task of get_benchmark_tasks(impls, operation, language, {
				forced_async: BENCH_FORCED_ASYNC,
				corpus_kind: CORPUS_MODE
			})) {
				names.add(task.name);
			}
		}
	}
	return names;
}

/**
 * Render `SURFACE_DISCLOSURES`, THROWING if a claim disagrees with this surface's
 * registry. A wrong disclosure is worse than none: it is a published sentence
 * asserting a policy the code no longer implements, and nothing else in the report
 * contradicts it (an absent row leaves no trace, which is the whole reason the
 * disclosure exists).
 *
 * One absence is NOT drift and does not throw: an `added` row whose impl failed to
 * initialize. That is this machine coming up short, recorded as `unavailable` and
 * warned about here — while the disclosure's prose, which explains why the row is
 * on this surface, is dropped rather than printed over a row that isn't there.
 */
function surface_disclosure_lines(registered: Set<string>): {
	lines: string[];
	warnings: string[];
} {
	const lines: string[] = [];
	const warnings: string[] = [];
	for (const d of SURFACE_DISCLOSURES) {
		const present = registered.has(d.row);
		if (d.direction === 'excluded' && present) {
			throw new Error(
				`report disclosure is stale: it says '${d.row}' is excluded from the conformance ` +
					`surface, but this surface registers it. Update SURFACE_DISCLOSURES in bench.ts to ` +
					`match the registration in lib/implementations.ts.`
			);
		}
		if (d.direction === 'added' && !present) {
			if (!d.initialized()) {
				warnings.push(
					`⚠ '${d.row}' did not initialize, so its "Added here" disclosure is omitted from ` +
						`this report (see \`unavailable\`).`
				);
				continue;
			}
			throw new Error(
				`report disclosure is stale: it says '${d.row}' is added on the conformance surface, ` +
					`but this surface does not register it. Update SURFACE_DISCLOSURES in bench.ts to ` +
					`match the registration in lib/implementations.ts.`
			);
		}
		lines.push(d.prose);
	}
	return { lines, warnings };
}

// Resolve the conformance report's row-composition disclosures HERE — before the
// pre-flight and timed phases — so a stale claim fails immediately, rather than at
// report time after a full run whose output the throw would then discard.
// Both checks below ask the same registry, so it is built ONCE — two calls would
// invite them to answer from different sets after a future edit.
const REGISTERED_ROWS = registered_row_names();
const { lines: SURFACE_DISCLOSURE_PROSE, warnings: surface_disclosure_warnings } = IS_CONFORMANCE
	? surface_disclosure_lines(REGISTERED_ROWS)
	: { lines: [], warnings: [] };
for (const warning of surface_disclosure_warnings) log(warning);

// The report's row order is hand-maintained and UNCHECKED in the other direction:
// an unlisted name doesn't fail, it sorts silently to the end (`report.ts`
// `rows_missing_from_display_order`). Same drift shape as a stale disclosure, one
// severity down — a misordered table misleads nobody the way a false sentence does
// — so this warns where the disclosure check throws. Runs on both surfaces, since
// each registers rows the other doesn't.
const unordered_rows = rows_missing_from_display_order(REGISTERED_ROWS);
if (unordered_rows.length > 0) {
	log(
		`⚠ ${unordered_rows.join(', ')} — not in report.ts DISPLAY_ORDER, so ${
			unordered_rows.length === 1 ? 'it sorts' : 'they sort'
		} last in every table`
	);
}

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
 * Tracking keys of coverage-only tasks — measured in pre-flight, never timed.
 * Populated per group by `run_preflight_group`, and read wherever a task's
 * participation would otherwise be assumed: the intersection, the perf
 * hard-fail, and the timed loop. See `BenchmarkTask.coverage_only`.
 */
const coverage_only_keys: Set<string> = new Set();

/**
 * Fail the run if the perf pre-flight skipped any file for an in-scope task that
 * `PERF_OMITS` doesn't excuse. `skipped_files` is keyed by tracking_key and only
 * ever holds in-scope failures (a task exists only for the languages its impl
 * declares), so every unlisted entry is a real regression. Sorted, one line per
 * violation, so a reviewer can transcribe a genuine tolerance straight into
 * `PERF_OMITS`.
 *
 * Coverage-only tasks are exempt: the invariant says every tool whose THROUGHPUT
 * we publish must process every real-world file, and a coverage-only row
 * publishes no throughput — sub-100% there is the measurement, not an erosion of
 * one.
 */
function enforce_perf_coverage(): void {
	const violations: string[] = [];
	for (const [tracking_key, files] of skipped_files) {
		if (coverage_only_keys.has(tracking_key)) continue;
		for (const [path, error] of files) {
			if (perf_omit_reason(PERF_OMITS, tracking_key, path) === null) {
				violations.push(`  ${tracking_key}  ${path}: ${error}`);
			}
		}
	}
	if (violations.length === 0) return;
	violations.sort();
	console.error(
		`Perf corpus: ${violations.length} unlisted pre-flight failure(s). Every in-scope tool must ` +
			`process every real-world file — fix the tool, or add a reviewed entry (with a reason) to ` +
			`PERF_OMITS in lib/perf_omit.ts:\n${violations.join('\n')}`
	);
	exit(1);
}

/**
 * The task name that runs the SAME ENGINE as `name`, or `null` when it has no
 * such sibling. Two shapes qualify, and the invariant is identical for both: one
 * engine behind two BINDINGS (native/wasm), and one binding driven with two
 * OPTIONS (rsvelte's default wire vs its `skipExpressionLoc` one). Neither can
 * change which files parse, so a divergence is a broken binding or an option
 * that does more than it claims.
 */
const same_engine_sibling_name = (name: string): string | null => {
	if (name === 'oxc-parser') return 'oxc-parser-wasm';
	if (name === 'yuku-parser') return 'yuku-parser-wasm';
	if (name === 'rsvelte-parse') return 'rsvelte-parse-skip-expr-loc';
	if (name === 'tsv' || name.startsWith('tsv-')) return name.replace(/^tsv/, 'tsv_wasm');
	return null;
};

/** One same-engine pair whose pre-flight accept sets disagree. */
interface VariantParityFinding {
	group: string;
	/** The base row of the pair (the native binding, or the default-option wire). */
	impl: string;
	/** Its same-engine sibling (the wasm binding, or the reduced-option wire). */
	sibling: string;
	/** Files only `impl` accepted. */
	impl_only: number;
	/** Files only `sibling` accepted. */
	sibling_only: number;
}

/** Populated by `warn_variant_parity()` after pre-flight; lands in the report as `variant_parity`. */
const variant_parity_findings: VariantParityFinding[] = [];

/**
 * Warn when a same-engine pair diverges in its pre-flight accept set. Both rows
 * run the identical engine (see `same_engine_sibling_name`), so their accept sets
 * should agree file-for-file; a divergence means one binding's error surface is
 * broken, or an option changed more than it claims — the concrete case being the
 * oxc WASI binding's consume-once `errors` getter, which silently accepted every
 * file and fabricated a 100% coverage row while native oxc-parser correctly
 * rejected 245 (see lib/oxc_wasm.ts + CLAUDE.md §Known Issues). Warning only
 * (never fatal): the coverage numbers themselves are the product in conformance
 * mode, and perf mode has its own hard-fail. Findings also land in the report
 * JSON (`variant_parity`), so a divergence shows up in the committed diff at
 * review time, not just the terminal scroll.
 */
function warn_variant_parity(): void {
	for (const [group_name, task_tracking] of task_tracking_by_group) {
		for (const [name, tracking_key] of task_tracking) {
			const sibling_name = same_engine_sibling_name(name);
			if (sibling_name === null) continue;
			const sibling_key = task_tracking.get(sibling_name);
			if (sibling_key === undefined) continue;
			const impl_set = successful_files.get(tracking_key) ?? new Set<string>();
			const sibling_set = successful_files.get(sibling_key) ?? new Set<string>();
			let impl_only = 0;
			for (const path of impl_set) if (!sibling_set.has(path)) impl_only++;
			let sibling_only = 0;
			for (const path of sibling_set) if (!impl_set.has(path)) sibling_only++;
			if (impl_only === 0 && sibling_only === 0) continue;
			variant_parity_findings.push({
				group: group_name,
				impl: name,
				sibling: sibling_name,
				impl_only,
				sibling_only
			});
			console.error(
				`⚠ variant parity (${group_name}): ${name} and ${sibling_name} accept different files ` +
					`(${impl_only} ${name}-only, ${sibling_only} ${sibling_name}-only). Same engine — a ` +
					`divergence means a broken binding or an option doing more than it claims, not an ` +
					`engine difference.`
			);
		}
	}
}

/**
 * Split each group's pre-flight coverage by CORPUS SOURCE — the conformance
 * report's breakdown table.
 *
 * Pure post-processing over state pre-flight already produced (`successful_files`
 * + each file's `source` tag), so it adds no parse work. Only the conformance
 * surface renders it: the perf corpus is 100% by construction, where a per-source
 * split would be a table of `100%`.
 *
 * A file with no `source` (a `DirectoryLoader` run) is skipped rather than bucketed
 * under a placeholder — an unattributed row would read as a corpus entry that
 * doesn't exist.
 *
 * Computed ONCE (`coverage_by_source`, below) and shared by the JSON and markdown
 * halves of the report: two passes over the same live mutable state could report
 * two different numbers for one published figure.
 */
function compute_coverage_by_source(): CoverageBySource {
	const by_group: CoverageBySource = new Map();
	for (const [group_name, task_tracking] of task_tracking_by_group) {
		const [, language] = group_name.split('/') as ['parse' | 'format', Language];
		const files = files_by_language[language];
		const by_source = new Map<string, Map<string, SourceCoverageCell>>();
		for (const [name, tracking_key] of task_tracking) {
			const success = successful_files.get(tracking_key);
			if (!success) continue;
			for (const file of files) {
				if (file.source === undefined) continue;
				let cells = by_source.get(file.source);
				if (!cells) {
					cells = new Map();
					by_source.set(file.source, cells);
				}
				let cell = cells.get(name);
				if (!cell) {
					cell = { processed: 0, total: 0 };
					cells.set(name, cell);
				}
				cell.total++;
				if (success.has(file.path)) cell.processed++;
			}
		}
		if (by_source.size > 0) by_group.set(group_name, by_source);
	}
	return by_group;
}

/**
 * The per-source coverage both report halves render, computed once after pre-flight
 * and memoized — see `compute_coverage_by_source`.
 */
let coverage_by_source: CoverageBySource | null = null;
function get_coverage_by_source(): CoverageBySource {
	return (coverage_by_source ??= compute_coverage_by_source());
}

/**
 * `compute_coverage_by_source` as plain JSON — `group → source → impl → {processed,
 * total}` — for the committed report. Maps don't survive `JSON.stringify`, and the
 * markdown tables alone would leave a consumer (tsv.fuz.dev, a diff at review time)
 * reading percentages out of prose.
 */
function serialize_coverage_by_source(): Record<
	string,
	Record<string, Record<string, SourceCoverageCell>>
> {
	const out: Record<string, Record<string, Record<string, SourceCoverageCell>>> = {};
	for (const [group, by_source] of get_coverage_by_source()) {
		const sources: Record<string, Record<string, SourceCoverageCell>> = {};
		for (const [source, cells] of by_source) {
			sources[source] = Object.fromEntries(cells);
		}
		out[group] = sources;
	}
	return out;
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
	process_fn: (file: SourceFile) => Promise<void>
): Promise<void> {
	for (const file of files) {
		await process_fn(file);
	}
}

/** Files by language lookup */
const files_by_language: Record<Language, SourceFile[]> = {
	svelte: svelte_files,
	typescript: ts_files,
	css: css_files
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
	language: Language
): Promise<void> {
	for (let i = 0; i < tasks.length; i++) {
		const task = tasks[i];
		const success = new Set<string>();
		let bytes = 0;
		const start_ms = performance.now();
		for (const file of files) {
			try {
				// `file.goal` is set only for test262 (conformance surface); every
				// other corpus leaves it undefined → the default module parse.
				if (task.is_async) {
					await task.run_async!(file.content, language, file.goal);
				} else {
					task.run(file.content, language, file.goal);
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
	language: Language
): Promise<void> {
	const files = files_by_language[language];
	if (files.length === 0) return;

	const group_name = `${operation}/${language}`;
	log(`\n· ${group_name}`);

	const tasks = get_benchmark_tasks(impls, operation, language, {
		forced_async: BENCH_FORCED_ASYNC,
		corpus_kind: CORPUS_MODE
	});
	await run_preflight(tasks, files, language);

	const task_tracking = new Map<string, string>();
	for (const task of tasks) {
		task_tracking.set(task.name, task.tracking_key);
		if (task.coverage_only) coverage_only_keys.add(task.tracking_key);
	}
	task_tracking_by_group.set(group_name, task_tracking);

	// Build each task's iteration file list. In `intersection` mode (default)
	// every task in the group iterates the same all-N intersection, making
	// timing ratios within the group apples-to-apples. In `union` mode each
	// task iterates its own preflight success set — ratios then reflect
	// different file sets per impl, useful for auditing what intersection
	// mode hides.
	// A coverage-only task is never timed, so it must not narrow the intersection
	// either — otherwise a file it alone rejects would silently drop out of the
	// set every REAL row is measured on, letting a non-participant move the
	// published numbers.
	const timed_tasks = tasks.filter((task) => !task.coverage_only);

	const filtered_files_by_task = new Map<string, SourceFile[]>();
	if (USE_INTERSECTION) {
		let intersection: Set<string> | null = null;
		for (const task of timed_tasks) {
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
		for (const task of timed_tasks) {
			filtered_files_by_task.set(task.tracking_key, intersection_list);
		}
		log(`  Intersection: ${intersection_list.length}/${files.length} files`);
	} else {
		for (const task of timed_tasks) {
			const success_set = successful_files.get(task.tracking_key) ?? new Set<string>();
			filtered_files_by_task.set(
				task.tracking_key,
				files.filter((f) => success_set.has(f.path))
			);
		}
	}

	// Overwrite preflight-derived byte counts with iteration byte counts so
	// throughput math (`ops_per_sec × effective_corpus_bytes`) reflects what was
	// actually measured. Also record per-task iteration size for the
	// `Nx (Mf)` annotation in the bench-table `vs baseline` column.
	//
	// Coverage-only tasks are skipped, deliberately leaving them with NO
	// `iterated_file_count` entry: they were timed on nothing, so their
	// `files_iterated` must read `null` rather than borrow the intersection's
	// count and imply a measurement that never happened. Their
	// `effective_corpus_bytes` likewise keeps the pre-flight value, which is the
	// only bytes figure that means anything for them.
	for (const task of timed_tasks) {
		const task_files = filtered_files_by_task.get(task.tracking_key)!;
		effective_corpus_bytes.set(
			task.tracking_key,
			task_files.reduce((sum, f) => sum + f.bytes, 0)
		);
		iterated_file_count.set(task.tracking_key, task_files.length);
	}

	group_setups.set(group_name, { tasks: timed_tasks, filtered_files_by_task });
}

/** Run the timed measurement loop for one group using its stashed pre-flight setup. */
async function run_benchmark_group(
	operation: 'parse' | 'format',
	language: Language
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
		// When a baseline is being saved or compared, the floors rise (5→10,
		// slow-task 7→12 below): the Welch p-values feeding regression verdicts
		// need the samples (n≈4-7 sits in the unstable-DOF regime the timing
		// library's own n=30 floor exists to avoid), and the extra wall clock is
		// only paid on runs that asked for statistics. Plain runs keep the cheap
		// floors — their headline (per-sweep mean / MB/s) is intrinsically
		// low-variance.
		min_iterations: baselining ? 10 : 5,
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
			const effective_bytes = tracking_key ? (effective_corpus_bytes.get(tracking_key) ?? 0) : 0;
			// Mirror the report-path guard (`generate_group_throughput_markdown`):
			// with an empty intersection the MB/s figure is a misleading `0.0 MB/s`
			// while ops/sec is real, so print `—` instead of a fake throughput.
			const throughput =
				effective_bytes === 0
					? '—'
					: format_throughput(result.stats.ops_per_second * effective_bytes);
			log(`  [${index + 1}/${total}] ${result.name}: ${ops_per_sec} sweeps/sec (${throughput})`);
		}
	});

	for (const task of tasks) {
		const task_files = filtered_files_by_task.get(task.tracking_key)!;
		// Tier per-task `min_iterations` based on preflight pass time. The
		// suite floor (5; 10 when baselining) handles most cases; very slow
		// tasks (>5s/pass — prettier on the full TS corpus, oxfmt full passes)
		// get a bump (7; 12 when baselining) because at n=5 their p75/p90
		// still sit too close to max and the Welch DOF is on the edge. Above
		// that we don't keep climbing: each extra iteration on a 14s/pass task
		// costs another 14s of wall clock.
		const preflight_ms = preflight_elapsed_ms.get(task.tracking_key) ?? 0;
		const min_iter = preflight_ms > 5000 ? (baselining ? 12 : 7) : undefined;
		// Slow tasks also tier WARMUP down (3 → 1): a multi-second sweep over
		// thousands of files fully warms the JIT in one pass, so the 2nd and 3rd
		// warmups are pure wall clock (~25s/runtime on prettier's TS row alone).
		// An explicit `BENCH_WARMUP` wins — it's the knob for deliberately
		// studying warmup effects, so tiering must not silently override it.
		const warmup_iter = preflight_ms > 5000 && BENCH_WARMUP_EXPLICIT === undefined ? 1 : undefined;
		const base_task = { name: task.name, min_iterations: min_iter, warmup_iterations: warmup_iter };
		if (task.is_async) {
			bench.add({
				...base_task,
				fn: async () => {
					await process_corpus_async(task_files, async (f) => {
						await task.run_async!(f.content, language, f.goal);
					});
				},
				async: true
			});
		} else {
			bench.add({
				...base_task,
				fn: () => {
					process_corpus(task_files, (f) => task.run(f.content, language, f.goal));
				},
				async: false
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
	for (const operation of OPERATIONS) {
		await run_preflight_group(operation, lang);
	}
}

// Same-engine native/wasm variant pairs should accept identical file sets — a
// divergence is a binding-boundary bug masquerading as coverage (see the fn doc).
warn_variant_parity();

// Perf corpus is real-world code every in-scope tool must fully process, so a
// per-file pre-flight failure that isn't an explicitly-reviewed `PERF_OMITS`
// entry is a hard error — not the silent skip that would quietly erode coverage.
// Conformance mode measures coverage (sub-100% is the metric), so this is
// perf-only. Runs before the timed phase, so a regression fails in seconds.
if (CORPUS_MODE === 'perf') {
	enforce_perf_coverage();
}

if (COVERAGE_ONLY) {
	log(
		'\nCoverage-only mode: skipping the timed benchmark phase (coverage is a pre-flight product).'
	);
} else {
	log('\nRunning benchmarks:');
	for (const lang of LANGUAGES) {
		for (const operation of OPERATIONS) {
			await run_benchmark_group(operation, lang);
		}
	}
}

//
// Baseline Handling
//

interface BaselineEntry {
	name: string;
	group: string;
	// Timing stats — `null` in a coverage-only run (`BENCH_COVERAGE_ONLY=1`),
	// which skips the timed phase and emits coverage from pre-flight alone. A
	// timed run always fills them.
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

/**
 * Package versions used in the benchmark run — the report's `ReportVersions`
 * (canonical oracles + whichever alternatives loaded) plus tsv's own. Adding an
 * impl means extending `AlternativeVersionInfo` in `lib/report.ts`, one place,
 * rather than the three hand-kept field lists this used to be.
 */
interface BaselineVersions extends ReportVersions {
	/** tsv's own version, from `Cargo.toml` `[workspace.package]` (the binary under test). */
	tsv: string;
}

interface Baseline {
	version: number;
	/** The JS runtime that produced this report (`deno` | `node` | `bun`). Mirrors
	 * the per-row `runtime` and matches the `report.<runtime>.{json,md}` filename. */
	runtime: Runtime;
	/**
	 * Which corpus/surface produced this report: `perf` (real-world corpus,
	 * parse + format groups — `report.<runtime>.*`) or `conformance`
	 * (fixtures-only corpus, disjoint from perf, Svelte set minus canonical-rejects,
	 * parse groups only — `report.conformance.<runtime>.*`). See `BENCH_CORPUS`.
	 */
	corpus_kind: CorpusKind;
	timestamp: string;
	git_commit: string | null;
	/**
	 * The machine that produced this report — CPU model, OS/arch, and the
	 * runtime's own version. The throughput numbers are machine-relative, so
	 * without this a report copied to the site (or diffed against an older one)
	 * can't distinguish a code change from a different box. See `Machine`.
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
	 * Per-corpus-source coverage — `group → source → impl → {processed, total}`,
	 * the machine-readable half of the report's per-source tables. Present on
	 * COVERAGE-ONLY (conformance) runs only; `undefined` on the perf surface, where
	 * every cell would read 100% by construction. Read these rather than a group's
	 * aggregate: the aggregate blends corpora that answer different questions, and
	 * on a corpus filtered by its own canonical parser that parser's row is 100% by
	 * construction rather than by achievement.
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
	 */
	binary_sizes_absent: string[];
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
	 * Same-engine pairs — one engine behind two bindings, or one binding under two
	 * options — whose pre-flight accept sets
	 * disagreed (see `warn_variant_parity`). Empty `[]` when every pair agrees
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
	 */
	unavailable: UnavailableImpl[];
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
	sample_size: null
} as const;

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
		// Bumped 10 → 11 for `binary_sizes_absent` (the size table's composition
		// disclosure — which expected artifacts were not on disk). 9 → 10 for
		// `unavailable` (the impls that failed to init, so a row
		// missing from every table has a recorded cause instead of only a ⚠ line in
		// the run's output). 8 → 9 for `variant_parity`'s neutral pair keys
		// (`impl`/`sibling` + their `_only` counts, was `native`/`wasm`): the check
		// now also pairs one binding under two options, which those names described
		// wrongly. 7 → 8 added
		// `coverage_by_source` (coverage-only runs; the markdown's per-source tables in
		// machine-readable form). 6 → 7 added the top-level `machine` block (CPU model,
		// OS/arch, runtime version); 5 → 6 added `corpus_kind` + `corpus_sources`;
		// 4 → 5 added the `runtime` field, top-level and per row.
		version: 11,
		runtime: RUNTIME,
		corpus_kind: CORPUS_MODE,
		timestamp: new Date().toISOString(),
		git_commit: await get_git_commit(),
		machine: current_machine(),
		corpus,
		corpus_sources: corpus_loader.sources,
		// Per-source coverage, the JSON half of the markdown tables. Coverage-only
		// runs only: on the perf surface every cell would read 100% by construction
		// (an unlisted per-file failure hard-fails the run instead).
		coverage_by_source: COVERAGE_ONLY ? serialize_coverage_by_source() : undefined,
		versions,
		binary_sizes: collected_sizes.sizes,
		binary_sizes_absent: collected_sizes.absent,
		entries,
		suppressed_noise: Object.fromEntries(suppressed_noise),
		variant_parity: variant_parity_findings,
		unavailable: impls.unavailable
	};
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
	machine: Machine,
	task_tracking: Map<string, Map<string, string>>,
	effective_size: Map<string, EffectiveCorpusEntry>,
	effective_bytes: Map<string, number>,
	iterated_counts: Map<string, number>,
	skipped: Map<string, Map<string, string>>
): string {
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
		? 'conformance — fixtures-only corpus (disjoint from perf; Svelte set minus svelte/compiler-rejected files), parse groups only; per-tool Coverage lines only (coverage-only run — timed throughput skipped)'
		: 'conformance — fixtures-only corpus (disjoint from perf; Svelte set minus svelte/compiler-rejected files), parse groups only; the headline is the per-tool Coverage lines (parse success over the valid set), with throughput measured on the all-tools-pass intersection';
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
	if (corpus_loader.sources.length > 0) {
		lines.push(
			`**Sources:** ${corpus_loader.sources.map((s) => `${s.path} (${s.files})`).join(', ')}\n`
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
	// claim was CHECKED against this surface's registry back at init, before the
	// run's work — see `SURFACE_DISCLOSURES`.
	lines.push(...SURFACE_DISCLOSURE_PROSE);

	lines.push(
		'**Methodology:** Single-threaded — every implementation formats/parses one file at a time, ' +
			'measured sequentially with no cross-file parallelism. One timed iteration is one full sweep ' +
			'over the group\u2019s iterated file set, so the absolute columns (sweeps/sec, p50\u2013p99, min/max) ' +
			'are per-sweep, not per-file — divide by the group\u2019s file count (the Files lines / `(Mf)` ' +
			'annotations) for per-file figures; ratios and MB/s are denominated consistently either way. ' +
			'This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.\n'
	);

	// Coverage-only run: no timed groups exist, so render the per-tool coverage
	// tables straight from pre-flight state (the timed loop below no-ops).
	if (COVERAGE_ONLY) {
		lines.push(
			...generate_coverage_only_markdown(LANGUAGES, OPERATIONS, task_tracking, effective_size),
			...generate_coverage_by_source_markdown(LANGUAGES, OPERATIONS, get_coverage_by_source())
		);
	}

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
			generate_group_bench_table_markdown(group.results, baseline_exists ? baseline : undefined)
		);
		lines.push('');

		const files = generate_group_files_markdown(group_iterated_counts);
		if (files) lines.push(files, '');

		const throughput = generate_group_throughput_markdown(group.results, tracking, effective_bytes);
		if (throughput) lines.push(throughput, '');

		const coverage = generate_group_coverage_markdown(group.results, tracking, effective_size);
		if (coverage) lines.push(coverage, '');

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
		// Consumer-side reconstruct-vs-materialize note — a curated cross-ref to
		// `diagnostics/reconstruct_vs_materialize.ts` (not a bench row), sitting
		// with the parse comparison since it's about the `no-locations` wire.
		lines.push(generate_reconstruct_note(), '');
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
	binary_sizes: BinarySize[],
	write_report: boolean
): Promise<string> {
	await mkdir(RESULTS_DIR, { recursive: true });
	const timestamp = data.timestamp.replace(/[:.]/g, '-').slice(0, 19);
	const commit = data.git_commit ?? 'unknown';
	const base_path = `${RESULTS_DIR}/${timestamp}_${commit}.${REPORT_TAG}`;

	const markdown = generate_markdown_report(
		groups,
		binary_sizes,
		data.corpus,
		bytes_by_language,
		data.versions,
		data.timestamp,
		data.git_commit,
		data.machine,
		task_tracking_by_group,
		effective_corpus_size,
		effective_corpus_bytes,
		iterated_file_count,
		skipped_files
	);

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
 * context the old custom baseline used to carry).
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
 * high-cv or high-outlier-ratio rows. The flat ±5% ops/sec gate that lived
 * here previously is gone — see `benchmark_baseline_compare` and the
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

// Collect binary sizes once (used by all output paths). Versions no longer
// thread through — bindings live in node_modules (flat, no version dir).
const collected_sizes = await collect_binary_sizes(impls);
const binary_sizes = collected_sizes.sizes;

// Build results data (used by all output paths and always saved)
const corpus = {
	svelte: svelte_files.length,
	typescript: ts_files.length,
	css: css_files.length
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
	...alt_versions
};
const results_data = await build_results_data(all_group_results, corpus, versions, collected_sizes);

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
			results_data.machine,
			task_tracking_by_group,
			effective_corpus_size,
			effective_corpus_bytes,
			iterated_file_count,
			skipped_files
		)
	);
} else {
	// Standard text output. The timed summary is empty in coverage-only mode —
	// the effective-corpus report below carries the coverage picture instead.
	if (!COVERAGE_ONLY) {
		console.log(generate_summary_report(all_group_results, LANGUAGES));
	}

	console.log(generate_versions_info(versions));

	const effective_corpus_report = generate_effective_corpus_report(
		effective_corpus_size,
		task_tracking_by_group
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
const write_report = args.save_report || !is_limited;
const results_path = await save_results(
	results_data,
	all_group_results,
	binary_sizes,
	write_report
);
log(`\nResults saved to:`);
log(`  ${results_path}.json`);
log(`  ${results_path}.md`);
if (write_report) {
	log(`Canonical report updated:`);
	log(`  ${RESULTS_DIR}/report.${REPORT_TAG}.json`);
	log(`  ${RESULTS_DIR}/report.${REPORT_TAG}.md`);
	// A limited corpus withholds this file (above); a machine missing an impl does
	// NOT — and shouldn't, since `unavailable` is non-empty by design on some
	// runtimes (Bun loads neither biome nor oxc-parser-wasm). But the two are the
	// same KIND of diminished measurement, and this one leaves no trace in the
	// table it thins: the row is simply gone, and the ⚠ init lines are far up the
	// scroll. So name the shortfall at the moment the file is published.
	if (impls.unavailable.length > 0) {
		log(
			`  ⚠ published without ${impls.unavailable.length} impl(s) that failed to load: ` +
				`${impls.unavailable.map((u) => u.impl).join(', ')} (recorded in \`unavailable\`)`
		);
	}
	if (results_data.binary_sizes_absent.length > 0) {
		log(
			`  ⚠ size table missing ${results_data.binary_sizes_absent.length} artifact(s): ` +
				`${results_data.binary_sizes_absent.join(', ')} (recorded in \`binary_sizes_absent\`)`
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
