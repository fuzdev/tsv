/**
 * The pre-flight process of a bench run: everything that needs the corpus and every
 * implementation loaded, done once, written down, and then gone.
 *
 * `bench.ts` starts this script before any row is timed:
 *
 * ```bash
 * <runtime> benches/js/bench_preflight.ts <path to a PreflightSpec JSON>
 * ```
 *
 * It loads the corpus and initializes every implementation, checks this surface's
 * row-composition claims against the registry, and runs every row once over every
 * file — untimed — to learn what each accepts. From that it grades the run's
 * standing claims (the perf surface's full coverage, the conformance pins, byte
 * parity between tsv's bindings), fixes each group's timed file set, and writes a
 * `PreflightSnapshot`: the plan for the timed phase plus every fact the report
 * states that only this process could establish. A failed grade exits non-zero
 * before a single row is timed.
 *
 * Then it exits, which is why it is a process of its own. The corpus and a dozen
 * loaded engines are a gigabyte or more of resident memory under some runtimes, and
 * the timed rows run after — each in a fresh process (`bench_row.ts`) on a machine
 * that no longer holds any of it.
 *
 * Nothing here is timed for publication. The one clock reading it keeps is each
 * row's cold sweep (`PreflightTimedRow.preflight_ms`), which prices the deadline of
 * that row's timed processes (`lib/bench_plan.ts` `row_deadline_ms`).
 *
 * @module
 */

import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { exit } from 'node:process';
import {
	ALLOW_MISSING,
	CORPUS_MODE,
	COVERAGE_ONLY,
	FILE_FILTER,
	IS_CONFORMANCE,
	IS_LIMITED,
	MAX_FILES_PER_LANGUAGE,
	OPERATIONS,
	RUNTIME,
	TASK_OPTIONS,
	USE_INTERSECTION
} from './lib/bench_config.ts';
import {
	file_set_digest,
	type PreflightGroup,
	type PreflightSnapshot,
	type PreflightSpec,
	read_child_spec,
	type VariantParityFinding,
	write_child_result
} from './lib/bench_protocol.ts';
import { create_logger, empty_output_error, suppress_stderr_noise } from './lib/bench_sweep.ts';
import { collect_binary_sizes } from './lib/binary_sizes.ts';
import {
	check_executed_artifacts,
	executed_artifact_identities,
	warn_stale_reported_artifacts
} from './lib/check_artifact_freshness.ts';
import { check_node_modules } from './lib/check_node_modules.ts';
import {
	corpus_missing_entries,
	CorpusLoader,
	type ExclusionCacheState,
	format_mb,
	group_by_language
} from './lib/corpus.ts';
import { detect_corpus_snapshot, enrich_source_repos } from './lib/corpus_repos.ts';
import { CSS_REJECTS_PIN } from './lib/gate_counts.ts';
import {
	get_alternative_versions,
	get_benchmark_tasks,
	get_defined_cells,
	get_defined_rows,
	init_implementations,
	unavailable_with_rows
} from './lib/implementations.ts';
import {
	type GroupOmissions,
	PERF_OMITS,
	type PerfOmit,
	perf_omit_matches,
	stale_perf_omits,
	summarize_group_omissions
} from './lib/perf_omit.ts';
import {
	type CoverageBySource,
	coverage_only_rows_missing_reason,
	rows_missing_from_comparisons,
	rows_missing_from_display_order,
	rows_missing_from_payload_tiers,
	type SourceCoverageCell
} from './lib/report.ts';
import { CANONICAL_PARSER_ROWS, type Language, LANGUAGES, type SourceFile } from './lib/types.ts';

const spec = read_child_spec<PreflightSpec>();
const log = create_logger(spec.log_to_stderr);

// before any engine loads — see `suppress_stderr_noise`
const suppressed_noise = suppress_stderr_noise();

//
// Setup
//

log('Loading corpus...\n');
const corpus_loader = new CorpusLoader(CORPUS_MODE, {
	// The bench loads EVERY language, so it has no `{ complete_for }` posture to
	// take here — `enforce_css_reject_pin` asks the per-language completeness
	// question separately, and degrades that one pin to "not graded" rather than
	// aborting a whole run over a corpus the other groups are fine with.
	missing: ALLOW_MISSING ? 'tolerate' : 'fail'
});
// Drain `stream()` directly instead of `load()` so we skip the loader's
// own corpus summary — the bench prints its own tighter one below that
// includes byte counts and (when applicable) limit annotations.
const files: SourceFile[] = [];
for await (const file of corpus_loader.stream(log)) {
	files.push(file);
}
// Reify each loaded source's GitHub origin (URL + commit + subpath) so the
// report links straight to the measured code — a few cheap `git` calls.
await enrich_source_repos(corpus_loader.sources);
enforce_exclusion_caches(corpus_loader.exclusion_caches);
const by_language = group_by_language(files);

// Preserve total counts before limiting
const total_file_counts = {
	svelte: by_language.svelte.length,
	typescript: by_language.typescript.length,
	css: by_language.css.length
};

// Apply file filter and limit (`!== undefined` so an explicit BENCH_LIMIT=0
// limits to zero files instead of silently meaning "no limit" — matching
// `IS_LIMITED`, which already treats 0 as a limited run)
function limit_files(files: SourceFile[]): SourceFile[] {
	const filter = FILE_FILTER;
	const filtered = filter ? files.filter((f) => f.path.includes(filter)) : files;
	return MAX_FILES_PER_LANGUAGE !== undefined
		? filtered.slice(0, MAX_FILES_PER_LANGUAGE)
		: filtered;
}

const svelte_files = limit_files(by_language.svelte);
const ts_files = limit_files(by_language.typescript);
const css_files = limit_files(by_language.css);

// Calculate total bytes per language for throughput metrics
const bytes_by_language: Record<Language, number> = {
	svelte: svelte_files.reduce((sum, f) => sum + f.bytes, 0),
	typescript: ts_files.reduce((sum, f) => sum + f.bytes, 0),
	css: css_files.reduce((sum, f) => sum + f.bytes, 0)
};

// Compact corpus summary: file counts + MB per language + total. When
// limited, each line reads `N of M files` so the subset is obvious.
const total_files = svelte_files.length + ts_files.length + css_files.length;
const total_bytes = bytes_by_language.svelte + bytes_by_language.typescript + bytes_by_language.css;
const fmt_count = (n: number, total: number) =>
	IS_LIMITED && n !== total ? `${n} of ${total}` : `${n}`;
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

// A run that measures NOTHING must not look like a run that measured everything.
// Without this, a mistyped `BENCH_FILTER` (the values are path substrings, so a
// typo matches nothing) loads every impl, "benchmarks" an empty corpus, writes a
// report with no entries and exits 0 — the same vacuity the byte check and the
// config probes each guard against, reached at the top level. Worse on the
// unfiltered path: `IS_LIMITED` is false there, so an empty corpus (every entry
// missing under BENCH_ALLOW_MISSING=1) would OVERWRITE the canonical report with an
// empty one. Refuse before init, and name whichever knob emptied it, since the
// corpus block above prints `0 of 773` without saying why.
if (total_files === 0) {
	const cause: string[] = [];
	if (FILE_FILTER !== undefined) cause.push(`BENCH_FILTER=${FILE_FILTER} matched no path`);
	if (MAX_FILES_PER_LANGUAGE !== undefined) cause.push(`BENCH_LIMIT=${MAX_FILES_PER_LANGUAGE}`);
	console.error(
		`Empty corpus — nothing to measure${cause.length > 0 ? `: ${cause.join(', ')}` : ''}.` +
			(cause.length > 0
				? '\n  The corpus loaded fine (see the counts above); the filter/limit removed every file.'
				: '\n  Every corpus entry loaded zero files — check the paths above and `deno task doctor`.')
	);
	exit(1);
}

// Refuse to measure stale binaries (the `:run` tasks skip the rebuild). Which
// artifacts this runtime executes — FFI or N-API, plus that runtime's WASM target
// — is `check_executed_artifacts`'s subject; override with BENCH_STALE_OK=1.
await check_executed_artifacts();
await warn_stale_reported_artifacts();
// What those artifacts ARE, read before anything loads them — the bytes every timed
// row's process must find again (`RowSpec.artifacts`).
const artifacts = executed_artifact_identities();

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
 * Mirrors the presence-gated fairness notes in `lib/report.ts`
 * (`comparison_notes`), which derive the same way — from what the run actually
 * produced rather than from a second hand-kept list.
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
	// The two `+locations` rows, one entry each because the claim is checked per row:
	// consumer-cost rows whose parse IS the default row's, so on a coverage surface
	// they would add nothing but a duplicate of that row's coverage.
	{
		row: 'tsv+locations',
		direction: 'excluded',
		prose:
			'**Excluded here:** tsv+locations — the default span-only parse plus `loc` ' +
			'reconstructed in JS (`{locations: true}`), a consumer-cost row measured on the perf ' +
			'corpus. The parse it runs is tsv’s, so its coverage is that row’s.\n'
	},
	{
		row: 'tsv-wasm+locations',
		direction: 'excluded',
		prose:
			'**Excluded here:** tsv-wasm+locations — the same consumer-cost row over the WASM ' +
			'binding; its coverage is tsv-wasm’s.\n'
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
 * Render `SURFACE_DISCLOSURES`, THROWING if a claim disagrees with this surface's
 * registry. A wrong disclosure is worse than none: it is a published sentence
 * asserting a policy the code no longer implements, and nothing else in the report
 * contradicts it (an absent row leaves no trace, which is the whole reason the
 * disclosure exists).
 *
 * An `added` row asks TWO questions, of two different sources, because `registered`
 * is availability-independent and so can only answer the first: does this surface
 * DEFINE the row (policy — a `no` is drift, and throws), and did its impl come up
 * on this MACHINE (a `no` is a shortfall, already in `unavailable` — the run warns
 * and drops the prose rather than explaining a row the report doesn't carry).
 * Collapsing them into one `!present` test loses whichever question the registry
 * isn't answering: against the live set a stale claim can never throw on a machine
 * missing the impl, and against the defined set the shortfall goes unnoticed.
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
					`surface, but this surface registers it. Update SURFACE_DISCLOSURES in bench_preflight.ts to ` +
					`match the registration in lib/implementations.ts.`
			);
		}
		if (d.direction === 'added') {
			// Two independent questions, each asked of the source that can answer it.
			// POLICY: does this surface define the row at all? `registered` is the
			// availability-independent set, so a `false` here is a decision in
			// `lib/implementations.ts` and nothing else — the stale-claim case.
			if (!present) {
				throw new Error(
					`report disclosure is stale: it says '${d.row}' is added on the conformance surface, ` +
						`but this surface does not register it. Update SURFACE_DISCLOSURES in bench_preflight.ts to ` +
						`match the registration in lib/implementations.ts.`
				);
			}
			// MACHINE: the surface defines the row, but did the impl behind it come
			// up here? If not, the prose would explain a row this report doesn't
			// carry. Asked separately because `registered` deliberately cannot see it.
			if (!d.initialized()) {
				warnings.push(
					`⚠ '${d.row}' did not initialize, so its "Added here" disclosure is omitted from ` +
						`this report (see \`unavailable\`).`
				);
				continue;
			}
		}
		lines.push(d.prose);
	}
	return { lines, warnings };
}

// Resolve the conformance report's row-composition disclosures HERE — before the
// pre-flight and timed phases — so a stale claim fails immediately, rather than at
// report time after a full run whose output the throw would then discard.
// The first two checks below ask the same registry, so it is built ONCE — two calls
// would invite them to answer from different sets after a future edit. (The payload-tier
// check further down asks a PARSE-scoped one of its own: a `DefinedRow` carries no
// operation to filter this one by.)
//
// The rows this surface DEFINES, not the rows this machine can run: both questions
// below are about policy, and answering them from the live set makes an `excluded`
// claim pass vacuously whenever the impl merely failed to load (see
// `get_defined_rows`). It is also why the answer is stable across machines — a row
// missing here is a decision, never a shortfall.
//
// The report's `unavailable` joins against this same list (`unavailable_with_rows`,
// at save time), so it is computed once here for all three readers.
const DEFINED_ROWS = get_defined_rows(impls, OPERATIONS, TASK_OPTIONS);
const REGISTERED_ROWS = new Set(DEFINED_ROWS.map((r) => r.name));
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

// The Comparisons tables' opponent list, asked the same way and at the same
// severity. Its drift is quieter than DISPLAY_ORDER's — an unlisted row doesn't
// sort oddly, it simply has no cell — and that is how `swc`, `postcss`,
// `rsvelte-parse` and `malva-wasm` each came to be registered, preflighted and
// timed at full coverage while appearing in no comparison at all. A row that
// genuinely belongs in none is listed in `COMPARISON_EXCLUSIONS` with its reason,
// so this can reach zero rather than being a warning nobody can clear.
const uncompared_rows = rows_missing_from_comparisons(REGISTERED_ROWS);
if (uncompared_rows.length > 0) {
	log(
		`⚠ ${uncompared_rows.join(', ')} — neither an opponent in report.ts ` +
			`COMPARISON_SECTIONS nor excused in COMPARISON_EXCLUSIONS, so ${
				uncompared_rows.length === 1 ? 'it appears' : 'they appear'
			} in no Comparisons table`
	);
}

// The parse rows' payload tiers (`report.ts` `PARSE_PAYLOAD_TIERS`), asked the same
// way: an unlisted parse row publishes `payload: null`, which a consumer can read
// only as "unknown" — and an `Nx` it renders from that row then carries no word on
// whether the two products match.
const untiered_rows = rows_missing_from_payload_tiers(
	get_defined_cells(impls, 'parse', TASK_OPTIONS)
);
if (untiered_rows.length > 0) {
	log(
		`⚠ ${untiered_rows.join(', ')} — no entry in report.ts PARSE_PAYLOAD_TIERS, so ${
			untiered_rows.length === 1 ? 'it publishes' : 'they publish'
		} \`payload: null\``
	);
}

// Each coverage-only row's published reason (`report.ts` `COVERAGE_ONLY_REASONS`),
// asked of every operation's cells. FATAL, unlike the three warnings above: the
// report refuses an unexplained untimed name, and asking here fails the run before
// anything is timed instead of after.
const unexplained_rows = coverage_only_rows_missing_reason(
	OPERATIONS.flatMap((operation) => get_defined_cells(impls, operation, TASK_OPTIONS))
);
if (unexplained_rows.length > 0) {
	throw new Error(
		`${unexplained_rows.join(', ')} — coverage-only with no entry in report.ts ` +
			`COVERAGE_ONLY_REASONS, so the report could not say why ${
				unexplained_rows.length === 1 ? 'it is' : 'they are'
			} not timed`
	);
}

//
// Pre-flight state
//

//
// Per-impl tracking maps (keyed by tracking_key, e.g. `parse/svelte/native`).
//
// Populated by the **untimed pre-flight pass**. It records each impl's
// success/skip set; the timed rows then sweep either the per-group all-N
// intersection (default) or each impl's preflight success set
// (`BENCH_MODE=union`).
//
// `successful_files` and `skipped_files` always reflect preflight results,
// independent of the iteration mode — they are the source of truth for
// coverage disclosure. `effective_corpus_bytes` is updated to reflect what a timed
// row will be timed on (intersection or per-impl).
//

/** Files an impl successfully processed during pre-flight, keyed by tracking_key. */
const successful_files: Map<string, Set<string>> = new Map();
/** Files an impl failed on during pre-flight, with the error message. */
const skipped_files: Map<string, Map<string, string>> = new Map();
/**
 * Of `successful_files`, the ones accepted only on the Script-goal retry a
 * `goal_fallback` file allows (`SourceFile.goal_fallback`), keyed by tracking_key —
 * surfaced per source as `SourceCoverageCell.script_only`.
 */
const script_only_files: Map<string, Set<string>> = new Map();
/** Effective corpus size per benchmark (processed / total files). */
const effective_corpus_size: Map<string, { processed: number; total: number }> = new Map();
/** Effective corpus bytes per benchmark — used for honest throughput math. */
const effective_corpus_bytes: Map<string, number> = new Map();
/**
 * Wall-clock ms for one preflight pass per task (iterating every file once) — what
 * prices the deadline of the task's timed processes (`PreflightTimedRow.preflight_ms`).
 */
const preflight_elapsed_ms: Map<string, number> = new Map();
/**
 * Map row name → tracking_key per group, for the checks below that hold a row name
 * and need its pre-flight state.
 */
const task_tracking_by_group: Map<string, Map<string, string>> = new Map();
/**
 * Every group pre-flight ran, as the snapshot carries it — the timed phase's plan
 * and the report's per-row coverage. Filled by `run_preflight_group`.
 */
const preflight_groups: PreflightGroup[] = [];

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
 * hard-fail, and the plan for the timed phase. See `BenchmarkTask.coverage_only`.
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
 *
 * The list is graded in BOTH directions, which is what makes it a ratchet rather
 * than an accumulator: an unlisted failure fails (above), and — on a full run — an
 * entry that excused nothing fails too (`stale_perf_omits`). One direction alone
 * lets a tolerance outlive the failure it was written for. Same posture as
 * `lib/fixtures_gate.ts`'s sanction / known-gap freshness check. (Neither
 * direction catches an entry written too BROADLY — that stays the author's job;
 * see `stale_perf_omits`.)
 *
 * Between the two sits the DISJOINTNESS check, which needs no full run: a failure
 * that more than one entry claims fails here, whatever the corpus scope, because
 * the overlap was observed rather than inferred. It is what makes the staleness
 * direction trustworthy — under a first-match reading an overlapping pair credits
 * only the earlier entry, and the shadowed one then reports as stale while the
 * failure it describes is live. Every match is credited (`perf_omit_matches`), so
 * that misreport is unreachable even if this check is somehow bypassed.
 *
 * Staleness is asked only of entries this run could have exercised, along two
 * independent axes, because "matched nothing" otherwise indicts the ledger for
 * something else's absence:
 *
 * - the TASK, passed down as the graded tracking keys. Every alternative impl is
 *   optional, and one that fails to load registers no task, so its entries can
 *   never fire — on that machine they are unasked, not stale. Coverage-only keys
 *   drop out for the mirror reason: they're exempt from the violation pass above,
 *   so nothing there can ever mark an entry used.
 * - the FILES, which is `full_corpus`: a filter or a partial checkout withholds
 *   the very files an entry is about, and reading that as staleness would fail
 *   every `BENCH_LIMIT` run.
 */
function enforce_perf_coverage(full_corpus: boolean): void {
	const violations: string[] = [];
	const used = new Set<PerfOmit>();
	/**
	 * Distinct overlapping CLAIM SETS, keyed by the entries that make them up — a
	 * broad entry reaching a narrow one's file can shadow it across dozens of
	 * files, and that is one ledger defect to fix, not dozens to read.
	 */
	const overlaps = new Map<string, { entries: PerfOmit[]; count: number; example: string }>();
	for (const [tracking_key, files] of skipped_files) {
		if (coverage_only_keys.has(tracking_key)) continue;
		for (const [path, error] of files) {
			const matches = perf_omit_matches(PERF_OMITS, tracking_key, path);
			if (matches.length === 0) {
				violations.push(`  ${tracking_key}  ${path}: ${error}`);
				continue;
			}
			for (const match of matches) used.add(match);
			if (matches.length === 1) continue;
			const key = matches.map((o) => `${o.task ?? '<any>'} @ ${o.path}`).join(' || ');
			const seen = overlaps.get(key);
			if (seen) seen.count += 1;
			else overlaps.set(key, { entries: matches, count: 1, example: `${tracking_key}  ${path}` });
		}
	}
	// Both are ledger failures and both are worth seeing in one pass: an unlisted
	// failure is a tool regression, an overlap is the list itself being ambiguous,
	// and fixing either in ignorance of the other invites a second round trip.
	let failed = false;
	if (violations.length > 0) {
		violations.sort();
		console.error(
			`Perf corpus: ${violations.length} unlisted pre-flight failure(s). Every in-scope tool must ` +
				`process every real-world file — fix the tool, or add a reviewed entry (with a reason) to ` +
				`PERF_OMITS in lib/perf_omit.ts:\n${violations.join('\n')}`
		);
		failed = true;
	}
	if (overlaps.size > 0) {
		console.error(
			`Perf corpus: ${overlaps.size} pre-flight failure shape(s) claimed by more than one ` +
				`PERF_OMITS entry. Entries must be DISJOINT — while two both match, neither is the entry ` +
				`that describes the failure, and one of them is redundant or reaches past what it was ` +
				`written for. Narrow or merge them in lib/perf_omit.ts:\n` +
				[...overlaps.values()]
					.map(
						(o) =>
							`  ${o.example}${o.count === 1 ? '' : ` (and ${o.count - 1} more file(s))`}\n` +
							o.entries
								.map((e) => `      claimed by  ${e.task ?? '<any task>'}  ${e.path}`)
								.join('\n')
					)
					.join('\n')
		);
		failed = true;
	}
	if (failed) exit(1);

	if (!full_corpus) return;
	// The tasks this run actually graded — every task that reached pre-flight,
	// minus the coverage-only ones the violation pass skips. An entry naming
	// anything else was never asked (see `stale_perf_omits`).
	const graded_keys = [...successful_files.keys()].filter((key) => !coverage_only_keys.has(key));
	const stale = stale_perf_omits(PERF_OMITS, used, graded_keys);
	if (stale.length === 0) return;
	console.error(
		`Perf corpus: ${stale.length} stale PERF_OMITS entr${stale.length === 1 ? 'y' : 'ies'} — ` +
			`excused no pre-flight failure in this full-corpus run, though the task each names ran:\n` +
			stale.map((o) => `  ${o.task ?? '<any task>'}  ${o.path}: ${o.reason}`).join('\n') +
			`\n  Delete the entry if the tool was fixed; update it if the corpus path was renamed.`
	);
	exit(1);
}

/**
 * The conformance run's exclusion caches, held to what the published coverage
 * presumes — refused before any impl loads, so a bad state costs seconds. The
 * loader applies them fail-open (most of its graders are untouched by them), but
 * the numbers this run writes are the committed report, and a cache that is absent
 * or predates its pin changes the Svelte or TypeScript denominator for every row.
 * The normal flow cannot reach either state — `bench:conformance` chains the
 * harvests first — so each means something broke:
 *
 * - absent: refused, naming the harvest, unless `BENCH_ALLOW_MISSING=1` — the same
 *   opt-in that tolerates a missing corpus entry and already marks the run as not
 *   comparable. The report records the absence either way (`exclusion_caches`);
 * - present at a size other than its exact pin: refused with no override. The
 *   harvest writes a cache only once its pin holds, so this is a cache from before
 *   a re-pin, and the fix is to re-harvest, never to tolerate it.
 *
 * Graded whatever `BENCH_FILTER` / `BENCH_LIMIT` say: the loader applies each cache
 * whole, so its size is a fact about the cache, not about the files this run kept.
 */
function enforce_exclusion_caches(caches: ExclusionCacheState[]): void {
	const refusals: string[] = [];
	for (const { label, task, pin, size } of caches) {
		if (size === null) {
			if (ALLOW_MISSING) {
				log(`  ⚠ ${label} cache absent — tolerated (BENCH_ALLOW_MISSING=1); not comparable`);
			} else {
				refusals.push(`the ${label} cache is absent — run \`deno task ${task}\``);
			}
		} else if (size !== pin) {
			refusals.push(
				`the ${label} cache holds ${size} paths ≠ its pin ${pin} — it predates a re-pin; ` +
					`re-run \`deno task ${task} --force\``
			);
		}
	}
	if (refusals.length === 0) return;
	console.error(
		`Conformance corpus: ${refusals.join('; ')}. The coverage this run publishes presumes ` +
			'every exclusion cache at its pin' +
			(ALLOW_MISSING ? '.' : ' (BENCH_ALLOW_MISSING=1 tolerates an ABSENT one).')
	);
	exit(1);
}

/**
 * Conformance mode's one exact pin. The files `svelte/compiler`'s `parseCss`
 * rejects are exactly the oracle row's pre-flight skips on `parse/css`, and their
 * count is `CSS_REJECTS_PIN` — the number `diagnostics/css_over_acceptance.ts`
 * grades (and stamps) from the same corpus. Graded here as well because this run
 * already holds it: the published `parse/css` reference row is built from these
 * skips, so a `parseCss` that changed what it accepts, or a corpus input that moved,
 * would otherwise reshape that row with nothing in this surface to catch it.
 *
 * Only a FULL CSS corpus can be graded: a filter, a limit, or a tolerated missing
 * entry withholds files, and a smaller reject set is then not a move. The loader
 * tolerates an absent OPTIONAL entry (the wpt-css cache) without
 * `BENCH_ALLOW_MISSING`, so that absence is asked separately — per language, since
 * an absent test262 cache withholds no CSS.
 */
async function enforce_css_reject_pin(full_corpus: boolean): Promise<void> {
	// Every not-graded path says so, this one included: a silent return reads
	// exactly like a pass, and a `BENCH_LIMIT` / `BENCH_FILTER` /
	// `BENCH_ALLOW_MISSING` run is the case where a reader is most likely to
	// assume the pin still held.
	if (!full_corpus) {
		log('\nCSS_REJECTS_PIN not graded — this run does not hold the full corpus.');
		return;
	}
	const tracking = task_tracking_by_group.get('parse/css');
	if (tracking === undefined) {
		log('\nCSS_REJECTS_PIN not graded — the parse/css group did not run.');
		return;
	}
	const { missing, optional_missing } = await corpus_missing_entries(CORPUS_MODE, 'css');
	const absent = [...missing, ...optional_missing];
	if (absent.length > 0) {
		log(`\nCSS_REJECTS_PIN not graded — the CSS corpus is partial: ${absent.join(', ')}`);
		return;
	}
	const oracle_key = tracking.get(CANONICAL_PARSER_ROWS.css);
	if (oracle_key === undefined) {
		// A `0` fallback here would report "rejects 0 ≠ 240" and read as a grammar
		// move — diagnosing a missing oracle ROW as a corpus change. It is neither:
		// the row that built the reject set is the measurement, so its absence fails
		// on its own terms.
		console.error(
			`Conformance corpus: the parse/css group ran without its oracle row ` +
				`(${CANONICAL_PARSER_ROWS.css}), so CSS_REJECTS_PIN has nothing to grade — the reject ` +
				`set IS that row's pre-flight skips. Check the impl registry and the row's name.`
		);
		exit(1);
	}
	const rejects = skipped_files.get(oracle_key)?.size ?? 0;
	if (rejects === CSS_REJECTS_PIN) {
		// Said aloud: a silent pass reads the same as a gate that never ran.
		log(
			`\nCSS_REJECTS_PIN: parseCss rejects ${rejects} of ${files_by_language.css.length} — matches.`
		);
		return;
	}
	console.error(
		`Conformance corpus: parseCss rejects ${rejects} of ${files_by_language.css.length} CSS files ` +
			`≠ pinned CSS_REJECTS_PIN ${CSS_REJECTS_PIN}. Either a pinned input moved (../prettier's, ` +
			`../svelte's or ../wpt's checkout) or svelte's parseCss changed what it accepts — ` +
			`re-pin in lib/gate_counts.ts deliberately, after checking which (\`deno task ` +
			`css:over-acceptance:pin\` grades the same count and stamps it).`
	);
	exit(1);
}

/**
 * A NATIVE tsv row — `tsv` and its variants, `tsv-<variant>` or `tsv+<option>` — as
 * opposed to the `tsv-wasm` family, which shares the `tsv-` prefix since the WASM
 * package took its kebab-case name. The wasm rows are the SIBLINGS these predicates
 * derive, never a base: the prefix test alone would pair `tsv-wasm-internal` with a
 * `tsv-wasm-wasm-internal` that no row defines.
 */
const is_native_tsv_row = (name: string): boolean =>
	name === 'tsv' ||
	name.startsWith('tsv+') ||
	(name.startsWith('tsv-') && !name.startsWith('tsv-wasm'));

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
	if (is_native_tsv_row(name)) return name.replace(/^tsv/, 'tsv-wasm');
	return null;
};

/**
 * The engine versions behind a native↔wasm pair, as the report labels them —
 * `[base, sibling]` — or `null` for a pair that has no second version to differ
 * (rsvelte's option pair, tsv's own rows: one package each).
 *
 * Read by the accept-set warning alone: its "same engine" claim holds only while
 * the two sides are installed at ONE version, and oxc's pair is pinned apart on
 * purpose (`package.json` `//oxc-wasi`), so the warning has to say which case it is
 * in rather than assert a binding bug over what may be an engine change.
 */
const same_engine_pair_versions = (
	name: string
): [string | undefined, string | undefined] | null => {
	const alt = get_alternative_versions(impls, TASK_OPTIONS);
	if (name === 'oxc-parser') return [alt.oxc_parser, alt.oxc_parser_wasm];
	if (name === 'yuku-parser') return [alt.yuku_parser, alt.yuku_parser_wasm];
	return null;
};

/**
 * Whether `name`'s same-engine sibling must produce byte-identical OUTPUT, not
 * merely accept the same files — the stronger half of the pair invariant, and the
 * only half graded FATALLY (`check_variant_parity`).
 *
 * True for tsv's own rows alone, and each exclusion is an argument rather than an
 * omission:
 *
 * - **tsv native↔wasm** is one Rust engine behind two bindings, so a byte
 *   difference is a marshalling or profile bug in tsv, with no reading under which
 *   it is tolerable — and under Node/Bun the native row IS the N-API addon, which
 *   makes this the cheapest standing correctness signal the shipped native path has.
 * - **rsvelte's option pair** is excluded on the strongest possible ground: its
 *   `skipExpressionLoc` variant removes payload BY DESIGN, so byte equality there
 *   would be the bug.
 * - **oxc's and yuku's native↔wasm pairs** should agree, but a divergence is a
 *   third-party binding's defect rather than something tsv's bench should hard-fail
 *   on — and oxc's WASI binding has a known one (lib/oxc_wasm.ts). They keep the
 *   accept-set warning, which is what surfaced that bug in the first place. oxc's
 *   pair is also pinned APART (the WASI binding is held back — `package.json`
 *   `//oxc-wasi`), so while the two pins differ a warning there can be an
 *   engine-version difference rather than a binding one.
 *
 * The `-internal` rows are excluded because they parse without serializing and
 * return nothing — there is no output to grade, and pretending otherwise would put
 * a pair in the graded set that can never carry digests, which is exactly the shape
 * the vacuity guard in `check_variant_parity` exists to catch. `tsv-forced-async` is
 * excluded for the neighbouring reason: it has no `tsv-wasm-forced-async` sibling to be
 * graded against (it is `tsv`'s own call behind an await, and `tsv` is graded), so
 * digesting it is work nothing compares.
 *
 * ⚠️ This names the BASE row of a pair. The pre-flight must digest BOTH sides, so it
 * derives its row set from this plus `same_engine_sibling_name` rather than from a
 * second hand-written predicate — see `run_preflight`.
 */
const sibling_outputs_must_match = (name: string): boolean =>
	is_native_tsv_row(name) && !name.endsWith('-internal') && name !== 'tsv-forced-async';

/**
 * Digest one pre-flight result for byte-parity comparison, or `null` when the
 * task produces nothing to compare.
 *
 * A format row returns its output string; a parse row returns the materialized
 * AST, whose serialization is stable to compare because both sides of a graded
 * pair emit it from the same Rust writer (the object is `JSON.parse`d from that
 * writer's bytes on the native side and materialized from the same bytes on the
 * wasm side, so key order is the writer's either way). The `-internal` rows parse
 * without serializing and return `undefined` — nothing to grade, hence `null`
 * rather than a digest of `"undefined"`, which would grade two blanks as equal
 * and read as coverage.
 *
 * TOTAL by construction, and that is load-bearing rather than defensive: V8's
 * `JSON.stringify` recurses once per AST level, so a pathologically deep tree
 * overflows the stack — tsc's `binderBinaryExpressionStress.ts` is 40 KB of nested
 * binary expressions and does exactly that. Only the WRITER recurses: the same
 * tree's `JSON.parse` is fine (V8's parser is iterative), which is why the row
 * PARSED that file and its output is fine — there is simply no digest for it.
 *
 * A throw escaping here would be caught by the pre-flight's skip handler and
 * recorded as the TOOL failing on a file it actually handled; and because the
 * success set is added to BEFORE this point, the file would count as processed AND
 * skipped at once — a phantom skip on all four byte-graded rows, and in the perf
 * corpus a `enforce_perf_coverage` hard-fail on a file nothing failed. No committed
 * report carries one, and the committed conformance report is the positive evidence
 * rather than the absence of a run: it was regenerated with the digest already
 * outside the try, so the four files that would have been phantom skips are recorded
 * as `output_digest_ungraded` — the disclosure — instead.
 */
function output_digest(result: unknown): string | null {
	if (result === undefined || result === null) return null;
	try {
		const text = typeof result === 'string' ? result : JSON.stringify(result);
		if (text === undefined) return null;
		return createHash('sha1').update(text).digest('hex');
	} catch {
		return null;
	}
}

/** Populated by `check_variant_parity()` after pre-flight; lands in the report as `variant_parity`. */
const variant_parity_findings: VariantParityFinding[] = [];

/**
 * Per-file output digests, keyed by tracking_key then path. Populated during
 * pre-flight for the rows `sibling_outputs_must_match` grades and no others.
 */
const output_digests: Map<string, Map<string, string>> = new Map();

/**
 * Files a byte-graded row ACCEPTED but whose output `output_digest` could not
 * digest, keyed by tracking_key — the byte check's own blind spot, counted rather
 * than assumed away.
 *
 * Not a failure of anything: the row parsed the file and its output is fine, so it
 * belongs in neither `skipped_files` (it isn't a skip) nor the digest map (there is
 * nothing to compare against). But an ungraded file IS a hole in the standing
 * correctness check on the shipped native artifact, and a hole nothing records is a
 * hole nobody finds — so it is reported both ways: a ⚠ at the pair, and
 * `output_digest_ungraded` in the committed JSON.
 */
const ungraded_digests: Map<string, { count: number; example: string }> = new Map();

/**
 * Grade every same-engine pair on the two things one engine behind two front-ends
 * owes: the same ACCEPT SET, and — where `sibling_outputs_must_match` says so —
 * the same OUTPUT BYTES.
 *
 * **Accept set, warning only.** Both rows run the identical engine (see
 * `same_engine_sibling_name`), so their accept sets should agree file-for-file; a
 * divergence means one binding's error surface is broken, or an option changed
 * more than it claims — the concrete case being the oxc WASI binding's
 * consume-once `errors` getter, which silently accepted every file and fabricated
 * a 100% coverage row while native oxc-parser correctly rejected 245 (see
 * lib/oxc_wasm.ts + CLAUDE.md §Known Issues). Never fatal: the coverage numbers
 * themselves are the product in conformance mode, and perf mode has its own
 * hard-fail.
 *
 * **Output bytes, FATAL.** An accept set can only see whether a file threw, so it
 * is blind to the failure that actually matters for a shipped binding — the same
 * engine returning *different content* through two front-ends. Under Node and Bun
 * the native row is the N-API addon built with the `napi` profile, so this pass is
 * the standing correctness check on the artifact the native npm packages ship,
 * over the whole bench corpus. There is no reading under which tsv's two bindings
 * may disagree byte-for-byte, so a mismatch exits non-zero rather than printing a
 * warning into a report nobody re-reads.
 *
 * ⚠️ The two halves are counted over different populations and must not be folded:
 * `impl_only`/`sibling_only` are files exactly one row accepted, `output_mismatch`
 * is files BOTH accepted. A file in the first population has no second output to
 * compare, so it can never appear in the second.
 *
 * Findings also land in the report JSON (`variant_parity`), so a surviving
 * accept-set divergence shows up in the committed diff at review time, not just
 * the terminal scroll.
 */
function check_variant_parity(): void {
	let fatal = false;
	for (const [group_name, task_tracking] of task_tracking_by_group) {
		for (const [name, tracking_key] of task_tracking) {
			const sibling_name = same_engine_sibling_name(name);
			if (sibling_name === null) continue;
			const sibling_key = task_tracking.get(sibling_name);
			if (sibling_key === undefined) continue;
			const impl_set = successful_files.get(tracking_key) ?? new Set<string>();
			const sibling_set = successful_files.get(sibling_key) ?? new Set<string>();
			let impl_only = 0;
			let shared = 0;
			for (const path of impl_set) {
				if (sibling_set.has(path)) shared++;
				else impl_only++;
			}
			let sibling_only = 0;
			for (const path of sibling_set) if (!impl_set.has(path)) sibling_only++;

			// Byte parity over the files both accepted. Absent digests mean this pair
			// isn't byte-graded (the `null` in pre-flight), not that it agreed.
			const impl_digests = output_digests.get(tracking_key);
			const sibling_digests = output_digests.get(sibling_key);
			let output_mismatch = 0;
			const output_mismatch_examples: string[] = [];
			let output_compared = 0;
			if (impl_digests && sibling_digests) {
				for (const [path, digest] of impl_digests) {
					const sibling_digest = sibling_digests.get(path);
					if (sibling_digest === undefined) continue;
					output_compared++;
					if (sibling_digest === digest) continue;
					output_mismatch++;
					if (output_mismatch_examples.length < 3) output_mismatch_examples.push(path);
				}
			}

			// ⚠️ Vacuity guard, and it is not decoration: a byte check that grades
			// NOTHING passes every run, so the failure mode of this pass is silence,
			// not a wrong answer. A pair the predicate declares byte-graded, with files
			// both rows accepted, must have digests on both sides — anything else means
			// the pre-flight's row set and the grading predicate have come apart, which
			// is precisely the bug the first cut of this shipped with. It exits on the
			// spot rather than joining `fatal` below: a mismatch is a finding worth
			// listing every pair of, but a harness that grades nothing makes every
			// other line of this pass meaningless, so there is nothing to collect.
			if (sibling_outputs_must_match(name) && shared > 0 && !(impl_digests && sibling_digests)) {
				console.error(
					`✗ variant parity (${group_name}): ${name}/${sibling_name} is byte-graded and shares ` +
						`${shared} accepted file(s), but ` +
						`${impl_digests ? sibling_name : name} carries no digests — the pre-flight row set ` +
						`and the grading predicate have drifted, so the byte check is a NO-OP. Fix the ` +
						`harness; a green run here proves nothing.`
				);
				exit(1);
			}

			// The byte check's blind spot, named where it applies. Not fatal — the row
			// accepted the file and its output is fine — but a pair that grades fewer
			// files than it accepted should say so rather than read as full coverage.
			//
			// BOTH sides, because `same_engine_sibling_name` names one direction only:
			// keying this on `tracking_key` alone warned about the base row and left the
			// sibling's hole to the JSON, so stderr and `output_digest_ungraded` reported
			// different totals for the same run (2 vs 4 on the conformance surface).
			//
			// The count is reported against what the pair actually COMPARED, not against
			// the hole alone: `output_digest` is total by construction, so "graded
			// nothing" is now reachable with both digest maps present — a state the
			// vacuity arm above cannot see, since it tests that the maps EXIST. Saying
			// `graded N of M` is what keeps `output_mismatch: 0` from quietly widening
			// from "agreed" to "never asked".
			for (const [row_name, key] of [
				[name, tracking_key],
				[sibling_name, sibling_key]
			] as const) {
				const ungraded = ungraded_digests.get(key);
				if (ungraded === undefined) continue;
				console.error(
					`⚠ variant parity (${group_name}): ${row_name} accepted ${ungraded.count} file(s) whose ` +
						`output could not be digested, so the byte check graded ${output_compared} of the ` +
						`${shared} file(s) the pair shares (first ungraded: ${ungraded.example}). Not a tool ` +
						`failure — see \`output_digest\`.`
				);
			}

			// …and when it graded NOTHING at all, that is the vacuity arm's own question
			// reached by the other road, so it takes the same posture minus the exit: a
			// pair whose every shared file went ungraded proves nothing, but the cause is
			// a runtime limit rather than harness drift (a lowered V8 stack would do it),
			// and hard-failing the run on it would accuse tsv of a defect it doesn't have.
			if (sibling_outputs_must_match(name) && shared > 0 && output_compared === 0) {
				console.error(
					`⚠ variant parity (${group_name}): ${name}/${sibling_name} is byte-graded and shares ` +
						`${shared} accepted file(s), but graded NONE of them — every output was ungraded, ` +
						`so this pair's byte check is a no-op this run. See \`output_digest\`.`
				);
			}

			if (impl_only === 0 && sibling_only === 0 && output_mismatch === 0) continue;
			variant_parity_findings.push({
				group: group_name,
				impl: name,
				sibling: sibling_name,
				impl_only,
				sibling_only,
				output_mismatch,
				output_mismatch_examples
			});
			if (impl_only > 0 || sibling_only > 0) {
				// The diagnosis depends on whether the pair really is ONE engine version.
				const pair = same_engine_pair_versions(name);
				const split =
					pair !== null && pair[0] !== undefined && pair[1] !== undefined && pair[0] !== pair[1];
				console.error(
					`⚠ variant parity (${group_name}): ${name} and ${sibling_name} accept different files ` +
						`(${impl_only} ${name}-only, ${sibling_only} ${sibling_name}-only). ` +
						(split
							? `The two sides are installed at DIFFERENT engine versions (${pair[0]} vs ` +
								`${pair[1]}), so this may be an engine change rather than a binding bug — ` +
								`re-read once the pins rejoin.`
							: `Same engine — a divergence means a broken binding or an option doing more ` +
								`than it claims, not an engine difference.`)
				);
			}
			if (output_mismatch > 0) {
				fatal = true;
				console.error(
					`✗ variant parity (${group_name}): ${name} and ${sibling_name} produced DIFFERENT ` +
						`OUTPUT on ${output_mismatch} file(s) both accepted. One engine, two bindings — this ` +
						`is a marshalling or build-profile bug, not an engine difference. First:\n` +
						output_mismatch_examples.map((path) => `    ${path}`).join('\n') +
						// Every other failure in this harness names the next action; this is the
						// most serious one, and the outputs themselves are gone by now (the check
						// keeps digests, not bytes), so the remedy is how to get them BACK.
						`\n  Isolate: BENCH_FILTER=${output_mismatch_examples[0]} deno task bench:${RUNTIME}:run` +
						`\n  Both sides are reachable per file from ${
							RUNTIME === 'deno' ? 'lib/ffi.ts' : 'lib/napi.ts'
						} and lib/wasm.ts; \`deno task smoke\` exercises the same two bindings.`
				);
			}
		}
	}
	if (fatal) exit(1);
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
			const script_only = script_only_files.get(tracking_key);
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
				if (script_only?.has(file.path)) cell.script_only = (cell.script_only ?? 0) + 1;
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
 * `ungraded_digests` as plain JSON, keyed `"<group>/<row>"` — the two identities a
 * report consumer already holds (`entries[].group` + `entries[].name`), rather than
 * the internal tracking key, which appears nowhere else in the emitted shape.
 */
function serialize_ungraded_digests(): Record<string, number> {
	const out: Record<string, number> = {};
	for (const [group_name, task_tracking] of task_tracking_by_group) {
		for (const [name, tracking_key] of task_tracking) {
			const entry = ungraded_digests.get(tracking_key);
			if (entry) out[`${group_name}/${name}`] = entry.count;
		}
	}
	return out;
}

/**
 * `compute_coverage_by_source` as plain JSON — `group → source → impl →
 * SourceCoverageCell` (`{processed, total}`, plus `script_only` where a goal
 * fallback added files) — for the committed report. Maps don't survive `JSON.stringify`, and the
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
 * to the timed phase (which sweeps the same files for seconds per row per pass).
 */
async function run_preflight(
	tasks: ReturnType<typeof get_benchmark_tasks>,
	files: SourceFile[],
	language: Language
): Promise<void> {
	// Rows on EITHER side of a byte-graded pair — the digest set for this group.
	// Derived from the pairing rather than spelled a second time, because the two
	// sides cannot be allowed to drift: digesting only the base row leaves the
	// sibling with no digests, and the byte comparison then silently comes up empty
	// and grades nothing. (That is not hypothetical — it is what the first cut of
	// this did, and it passed a clean run while checking nothing. The paired guard
	// is `check_variant_parity`'s vacuity arm.)
	const byte_graded_names = new Set<string>();
	for (const task of tasks) {
		if (!sibling_outputs_must_match(task.name)) continue;
		byte_graded_names.add(task.name);
		const sibling = same_engine_sibling_name(task.name);
		if (sibling !== null) byte_graded_names.add(sibling);
	}

	for (let i = 0; i < tasks.length; i++) {
		const task = tasks[i];
		const success = new Set<string>();
		const script_only = new Set<string>();
		// Digests are the byte half of `check_variant_parity`, and cost nothing on a
		// row it doesn't grade: `null` here means no hashing happens at all.
		const digests = byte_graded_names.has(task.name) ? new Map<string, string>() : null;
		let bytes = 0;
		const start_ms = performance.now();
		for (const file of files) {
			// ONLY the impl call belongs inside the skip-recording try. Anything else
			// in it — the digest below was — turns a HARNESS-side failure on a file the
			// tool handled into a recorded skip against the tool. See `output_digest`.
			let result: unknown;
			let at_script = false;
			try {
				// `file.goal` is set only on the conformance surface — test262's declared
				// goal, or a Prettier fixture's extension goal (`.mjs`, `.cts`, …); every
				// other file leaves it undefined → the default module parse.
				result = task.is_async
					? await task.run_async!(file.content, language, file.goal)
					: task.run(file.content, language, file.goal);
			} catch (e) {
				// A `goal_fallback` file (a TypeScript file with no goal of its own) is
				// retried at script (`SourceFile.goal_fallback`). The retry's own failure is
				// not the verdict — the module error is recorded, the primary reading — and
				// a tool the goal does not reach simply rejects twice.
				if (!file.goal_fallback) {
					record_skip(task.tracking_key, file.path, e);
					continue;
				}
				try {
					result = task.is_async
						? await task.run_async!(file.content, language, 'script')
						: task.run(file.content, language, 'script');
				} catch {
					record_skip(task.tracking_key, file.path, e);
					continue;
				}
				at_script = true;
			}
			// A tool-side verdict, not a harness failure: an empty output on a non-empty
			// input is the impl declining the file without saying so, recorded as a
			// skip against it — see `assert_output_present`.
			const empty = empty_output_error(task.name, file, result);
			if (empty !== null) {
				record_skip(task.tracking_key, file.path, empty);
				continue;
			}
			success.add(file.path);
			if (at_script) script_only.add(file.path);
			bytes += file.bytes;
			if (digests !== null) {
				const digest = output_digest(result);
				if (digest !== null) {
					digests.set(file.path, digest);
				} else if (result !== undefined && result !== null) {
					// An accepted output that produced no digest — see `ungraded_digests`.
					// A nullish result is the `-internal` shape, which has nothing to
					// grade by design and so is not a hole.
					const seen = ungraded_digests.get(task.tracking_key);
					if (seen) seen.count += 1;
					else ungraded_digests.set(task.tracking_key, { count: 1, example: file.path });
				}
			}
		}
		// digesting included (JSON.stringify + sha1, harness work): this prices a
		// deadline, where an over-estimate is the safe side
		const elapsed_ms = performance.now() - start_ms;
		successful_files.set(task.tracking_key, success);
		if (script_only.size > 0) script_only_files.set(task.tracking_key, script_only);
		if (digests !== null) output_digests.set(task.tracking_key, digests);
		effective_corpus_size.set(task.tracking_key, { processed: success.size, total: files.length });
		effective_corpus_bytes.set(task.tracking_key, bytes);
		preflight_elapsed_ms.set(task.tracking_key, elapsed_ms);
		log(`  [${i + 1}/${tasks.length}] ${task.name}: ${success.size}/${files.length} files`);
	}
}

//
// Run pre-flight
//

/**
 * The timed file sets written so far, by the list they hold. Keyed on the array's
 * identity: in `intersection` mode every timed row of a group is handed the SAME
 * list, and it is written once and shared.
 */
const written_file_sets: Map<SourceFile[], { path: string; digest: string }> = new Map();

/**
 * Write one timed file set into the run's scratch directory and return where, with
 * its digest (`file_set_digest`) — see `PreflightTimedRow.file_set`.
 *
 * The files are written whole, contents included, rather than as paths for the
 * timed process to read back: not every `SourceFile` is a file on disk (a harvested
 * stylesheet is cut out of a cache), and a row must be timed on the very strings
 * pre-flight ran it on. `JSON.stringify` escapes a lone surrogate rather than
 * replacing it, so every string reads back identical (and the row's process refuses
 * one its own UTF-8 re-decode would alter — `bench_row.ts`).
 */
function write_file_set(task_files: SourceFile[]): { path: string; digest: string } {
	const existing = written_file_sets.get(task_files);
	if (existing) return existing;
	const path = `${spec.run_dir}/file_set_${written_file_sets.size}.json`;
	writeFileSync(path, JSON.stringify(task_files));
	const written = { path, digest: file_set_digest(task_files) };
	written_file_sets.set(task_files, written);
	return written;
}

/**
 * Of a row's timed set, the paths it accepted only on the Script-goal retry
 * (`script_only_files`) — the ones its timed sweep must replay at `script`
 * (`PreflightTimedRow.script_only`). Empty off the conformance surface, where no
 * file allows the retry.
 */
function timed_script_only(tracking_key: string, task_files: SourceFile[]): string[] {
	const script_only = script_only_files.get(tracking_key);
	if (!script_only) return [];
	return task_files.filter((f) => script_only.has(f.path)).map((f) => f.path);
}

/**
 * Run pre-flight + iteration-set computation for one group. Populates
 * `successful_files`, `skipped_files`, `effective_corpus_size`,
 * `effective_corpus_bytes` and `task_tracking_by_group`, writes the group's timed
 * file sets, and records the group in `preflight_groups` — its plan for the timed
 * phase.
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

	const tasks = get_benchmark_tasks(impls, operation, language, TASK_OPTIONS);
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
		// Seeded EMPTY rather than null-until-first-task, so the no-timed-tasks case
		// (a group of nothing but coverage-only rows) falls out as the empty
		// intersection it is, without the membership test below having to re-answer
		// "was there a first task?" once per file.
		let intersection = new Set<string>();
		let seeded = false;
		for (const task of timed_tasks) {
			const success_set = successful_files.get(task.tracking_key) ?? new Set<string>();
			if (!seeded) {
				intersection = new Set(success_set);
				seeded = true;
			} else {
				for (const path of intersection) {
					if (!success_set.has(path)) intersection.delete(path);
				}
			}
		}
		const intersection_list = files.filter((f) => intersection.has(f.path));
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
	// actually measured.
	//
	// Coverage-only tasks are skipped: their `effective_corpus_bytes` keeps the
	// pre-flight value, which is the only bytes figure that means anything for them.
	for (const task of timed_tasks) {
		const task_files = filtered_files_by_task.get(task.tracking_key)!;
		effective_corpus_bytes.set(
			task.tracking_key,
			task_files.reduce((sum, f) => sum + f.bytes, 0)
		);
	}

	// The group as the snapshot carries it. A row is PLANNED for timing (`timed`) only
	// if something will time it: a coverage-only task was timed on nothing, so it
	// carries no set and its `files_iterated` reads `null` rather than borrowing the
	// intersection's count and implying a measurement that never happened; and a
	// coverage-only RUN times nothing either, so its every row reads the same.
	preflight_groups.push({
		name: group_name,
		operation,
		language,
		rows: tasks.map((task) => {
			const coverage = effective_corpus_size.get(task.tracking_key)!;
			const task_files = filtered_files_by_task.get(task.tracking_key);
			const file_set =
				task_files === undefined || COVERAGE_ONLY ? null : write_file_set(task_files);
			return {
				name: task.name,
				tracking_key: task.tracking_key,
				impl: task.impl,
				coverage_only: task.coverage_only === true,
				processed: coverage.processed,
				total: coverage.total,
				effective_bytes: effective_corpus_bytes.get(task.tracking_key) ?? 0,
				timed:
					file_set === null || task_files === undefined
						? null
						: {
								file_set: file_set.path,
								files: task_files.length,
								digest: file_set.digest,
								script_only: timed_script_only(task.tracking_key, task_files),
								preflight_ms: preflight_elapsed_ms.get(task.tracking_key) ?? 0
							}
			};
		})
	});
}

// Pre-flight every group up front, so the coverage picture — and any refusal below —
// lands before a single row is timed.
log('Pre-flight (discover coverage + exclude failing files before timing):');
for (const lang of LANGUAGES) {
	for (const operation of OPERATIONS) {
		await run_preflight_group(operation, lang);
	}
}

// Same-engine native/wasm variant pairs should accept identical file sets AND —
// for tsv's own pair — produce identical bytes. A divergence is a binding-boundary
// bug masquerading as coverage; the byte half is fatal (see the fn doc).
check_variant_parity();

// Perf corpus is real-world code every in-scope tool must fully process, so a
// per-file pre-flight failure that isn't an explicitly-reviewed `PERF_OMITS`
// entry is a hard error — not the silent skip that would quietly erode coverage.
// Conformance mode measures coverage (sub-100% is the metric), so this hard error is
// perf-only — but the other branch is not empty: conformance's own exact pin
// (`enforce_css_reject_pin`) is graded there, at the same point and for the same
// reason. Both run before the timed phase, so a regression fails in seconds and
// nothing is written.
//
// The staleness half of the same grade is asked only of a run that could actually
// reach every omitted file: a corpus filter, or a missing repo tolerated by
// BENCH_ALLOW_MISSING, withholds them, and "matched nothing" would then mean
// "wasn't there". The other absence — an optional impl that didn't load, so its
// task never ran — is handled inside, against the graded tracking keys (see
// `stale_perf_omits`).
if (CORPUS_MODE === 'perf') {
	enforce_perf_coverage(!IS_LIMITED && !ALLOW_MISSING);
} else {
	await enforce_css_reject_pin(!IS_LIMITED && !ALLOW_MISSING);
}

/**
 * `Baseline.omissions`: per timed group, the files its intersection left out and
 * the rows that left them (`summarize_group_omissions`). Perf surface, intersection
 * mode, timed run — the only place an omission exists: a coverage run times nothing,
 * and under `BENCH_MODE=union` a file one row fails leaves no other row's set.
 */
function build_omissions(): GroupOmissions[] | undefined {
	if (COVERAGE_ONLY || !USE_INTERSECTION || CORPUS_MODE !== 'perf') return undefined;
	const omissions: GroupOmissions[] = [];
	for (const language of LANGUAGES) {
		for (const operation of OPERATIONS) {
			const group_name = `${operation}/${language}`;
			const tracking = task_tracking_by_group.get(group_name);
			if (!tracking) continue;
			const rows = [...tracking]
				.filter(([, tracking_key]) => !coverage_only_keys.has(tracking_key))
				.map(([name, tracking_key]) => ({
					name,
					tracking_key,
					failed: [...(skipped_files.get(tracking_key)?.keys() ?? [])]
				}));
			// Nothing timed is "not measured", which the field spells as an absent group —
			// zeroes would claim an intersection that never existed.
			if (rows.length === 0) continue;
			omissions.push(
				summarize_group_omissions(group_name, files_by_language[language], rows, PERF_OMITS)
			);
		}
	}
	return omissions;
}

//
// Snapshot
//

// Everything above either passed or exited: what is left is to write down what this
// process learned, for the orchestrator to time from and report with.
const canonical_versions = impls.versions.canonical;
const snapshot: PreflightSnapshot = {
	corpus: {
		svelte: svelte_files.length,
		typescript: ts_files.length,
		css: css_files.length
	},
	bytes_by_language,
	corpus_sources: corpus_loader.sources,
	exclusion_caches: corpus_loader.exclusion_caches,
	corpus_snapshot: (await detect_corpus_snapshot(corpus_loader.sources)) ?? null,
	versions: {
		svelte: canonical_versions.svelte,
		acorn: canonical_versions.acorn,
		acorn_ts: canonical_versions['@sveltejs/acorn-typescript'],
		prettier: canonical_versions.prettier,
		prettier_svelte: canonical_versions['prettier-plugin-svelte'],
		...get_alternative_versions(impls, TASK_OPTIONS)
	},
	// Bindings live in node_modules (flat, no version dir), so no versions thread through.
	binary_sizes: await collect_binary_sizes(impls),
	unavailable: unavailable_with_rows(impls.unavailable, DEFINED_ROWS),
	surface_disclosure_prose: SURFACE_DISCLOSURE_PROSE,
	groups: preflight_groups,
	skipped_files: Object.fromEntries(
		[...skipped_files].map(([tracking_key, by_path]) => [tracking_key, Object.fromEntries(by_path)])
	),
	// Per-source coverage, the JSON half of the markdown tables. Coverage-only
	// runs only: on the perf surface every cell would read 100% by construction
	// (an unlisted per-file failure hard-fails the run instead).
	coverage_by_source: COVERAGE_ONLY ? serialize_coverage_by_source() : null,
	omissions: build_omissions() ?? null,
	output_digest_ungraded: serialize_ungraded_digests(),
	variant_parity: variant_parity_findings,
	suppressed_noise: Object.fromEntries(suppressed_noise),
	artifacts
};
write_child_result(spec, snapshot);

// Explicitly: a loaded binding may leave a handle or a thread alive, and the run is
// waiting on this process to end before it times anything.
exit(0);
