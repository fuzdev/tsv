/**
 * Corpus parse comparison - deep-diffs tsv's shipped parse output against the
 * canonical parsers (acorn-typescript / svelte / parseCss) on real codebases.
 *
 * Why this exists: the fixture suite's expected.json files are
 * canonical-derived but cover only curated cases, and the wire-JSON writer is
 * the sole emission path — so a writer bug on an uncurated shape (e.g. an
 * untranslated position field) has no internal gate to trip. This script is
 * the external oracle at corpus scale: the parser-side sibling of
 * corpus_compare_format.ts (which diffs formatting against prettier).
 *
 * Method: ASTs are raw-diffed with NO normalization applied before diffing
 * (`lib/parse_diff.ts`); diffs are classified against the documented divergences
 * (docs/conformance_svelte.md, matched by `lib/parse_divergences.ts`) at the REPORTING
 * layer only — so a bug in our own divergence reasoning surfaces as an undocumented group
 * instead of being silently absorbed. The canonical AST is serialized exactly like the fixture
 * sidecar serializes it (JSON round-trip with BigInt → string), so fixture and
 * corpus semantics match. tsv has two wires and each arm grades one. The span-only
 * arm diffs the shipped wire — the FFI's, which every binding shares (each language
 * crate's `convert_ast_json_*`) — against that same oracle output with every `loc` /
 * `name_loc` removed: the wire's own contract (it omits exactly those keys), not a
 * tolerance; everything else is raw-diffed as above. The loc arm diffs the Rust `loc`
 * emitter's wire (`convert_ast_json_bytes_with_locations`), which no binding ships —
 * the one `tsv parse --locations` writes — asked of one `tsv_debug loc_wires --stdin`
 * process (`lib/loc_wire_client.ts`).
 *
 * `loc` is graded by its own rules on the loc arm (see `diff_asts`): tsv's `loc` must
 * first be the definition — the shipped `locations.js` reconstruction of its own
 * span-only wire (`lib/loc_cross_grade.ts`) — and only then is it compared with the
 * oracle's: exactly for TypeScript, and for Svelte with the pinned superset and the
 * named tolerance rows (`lib/loc_tolerance.ts`), where any other difference fails.
 *
 * Multibyte files are the high-value slice: byte→UTF-16 offset translation vs
 * the canonical parsers' native offsets is the riskiest machinery. Use
 * --multibyte-only for fast iteration on it.
 *
 * Usage:
 *   deno task corpus:compare:parse ../some-project
 *   deno task corpus:compare:parse --all
 *   deno task corpus:compare:parse --all --multibyte-only
 *   deno task corpus:compare:parse --all --filter typescript --limit 100
 *   deno task corpus:compare:parse --all --json 2>/dev/null > report.json
 *
 * Parse FAILURES (one side throws) are counted but not the focus —
 * diagnostics/skip_triage.ts is the dedicated tool for parse-gap triage.
 */

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import { z } from 'zod';

import {
	COMPARE_BASE_ARG_FIELDS,
	type CompareFailure,
	create_compare_loader,
	dispose_compare,
	emit_json_stdout,
	exit_compare_failure,
	gate_on_panics,
	init_compare_implementations,
	parse_language_filter,
	redirect_logs_to_stderr,
	rel_path,
	resolve_compare_base_path,
	run_compare_main
} from './lib/compare_cli.ts';
import { is_native_panic_error } from './lib/divergence/panic_errors.ts';
import { first_line } from './lib/error_text.ts';
import { fixture_document } from './lib/fixture_documents.ts';
import { CORPUS_PARSE_COMPARED_PIN, CORPUS_PARSE_TSV_ERRORS_PIN } from './lib/gate_counts.ts';
import { first_difference, loc_definition_violation } from './lib/loc_cross_grade.ts';
import { LocWireClient } from './lib/loc_wire_client.ts';
import { LOC_ROWS, type LocRow } from './lib/loc_tolerance.ts';
import {
	type DiffEntry,
	type DiffResult,
	diff_asts,
	type LocRowCounts,
	MAX_DIFFS_PER_FILE,
	preview
} from './lib/parse_diff.ts';
import { bigint_replacer, span_only_replacer } from './lib/span_only.ts';
import { type Language, LANGUAGES } from './lib/types.ts';
import {
	type InjectKind,
	subtract_baseline_diffs,
	VARIANT_MARKER,
	with_injected_variants
} from './lib/wire_inject.ts';

/**
 * Injected variants per file. The cap is a blast-radius bound, not a coverage target: a
 * head-dense document would otherwise dominate a run, and every variant costs a canonical
 * parse. Raise it with `--inject-limit` when narrowing to a subtree, or pass
 * `--inject-limit 0` for a CENSUS — every site, which is what `wire:audit` runs and what
 * makes its finding set stable enough to grade (see `lib/wire_inject.ts`).
 */
const DEFAULT_INJECT_LIMIT = 12;

const CorpusCompareParseArgs = z.object({
	...COMPARE_BASE_ARG_FIELDS,
	'multibyte-only': z
		.boolean()
		.default(false)
		.meta({ aliases: ['m'] }),
	inject: z.boolean().default(false),
	'inject-terminators': z.boolean().default(false),
	'inject-limit': z.coerce.number().int().nonnegative().default(DEFAULT_INJECT_LIMIT),
	fixtures: z.boolean().default(false)
});

/** Max sample entries shown per diff group in the report. */
const MAX_GROUP_SAMPLES = 3;

interface FileResult {
	path: string;
	bytes: number;
	multibyte: boolean;
	status: 'documented' | 'undocumented' | 'tsv_error' | 'canonical_error' | 'both_error';
	diffs: DiffEntry[];
	truncated: boolean;
	error?: string;
}

interface LanguageStats {
	total: number;
	compared: number;
	multibyte: number;
	match: number;
	documented: number;
	undocumented: number;
	tsv_errors: number;
	canonical_errors: number;
	both_errors: number;
}

/** A zeroed per-language stats accumulator. */
function empty_stats(): LanguageStats {
	return {
		total: 0,
		compared: 0,
		multibyte: 0,
		match: 0,
		documented: 0,
		undocumented: 0,
		tsv_errors: 0,
		canonical_errors: 0,
		both_errors: 0
	};
}

/** Whether the source contains any non-ASCII character (the multibyte slice). */
function has_non_ascii(s: string): boolean {
	for (let i = 0; i < s.length; i++) {
		if (s.charCodeAt(i) > 0x7f) return true;
	}
	return false;
}

// --- The span-only arm -----------------------------------------------------------
//
// tsv's span-only wire — the one every binding emits, here the FFI's — is graded against
// the SAME oracle output with its line/column objects removed (`lib/span_only.ts`'s
// `span_only_replacer`): every `loc` key and every Svelte `name_loc` key, anywhere in the
// tree. Nothing else differs between the two tsv wires, so the arm reuses the diff engine
// and the documented matchers unchanged: a span difference the loc arm excuses is excused
// identically here, and the `loc` rules have no `loc` to grade. Every language has the
// arm, CSS included — `parseCss` emits no `loc`, so for CSS the stripped oracle is the
// oracle itself.
//
// The same span-only wire is what the loc arm's definition check reconstructs from: before
// tsv's loc wire is graded against the oracle, `loc_definition_violation` requires it to
// equal the shipped reconstruction of this wire, so a tolerance row can only excuse the
// oracle's departure from the definition, never tsv's.

/** Per-language counts for the span-only arm. */
interface SpanStats {
	/** Files both tsv wires and the oracle parsed, so the span-only tree was diffed. */
	compared: number;
	match: number;
	documented: number;
	undocumented: number;
	/**
	 * Files the two tsv wires gave different VERDICTS on — one parsed, the other threw.
	 * One parser behind two writers, so any count here is a binding bug, and fails.
	 */
	verdict_mismatch: number;
}

function empty_span_stats(): SpanStats {
	return { compared: 0, match: 0, documented: 0, undocumented: 0, verdict_mismatch: 0 };
}

/** A file the two tsv wires disagreed on parsing — each side's first error line, or `parsed`. */
interface SpanVerdictMismatch {
	path: string;
	language: Language;
	loc_wire: string;
	span_wire: string;
}

/** A file whose loc wire is not the reconstruction of its span-only wire — a tsv bug. */
interface LocDefinitionViolation {
	path: string;
	language: Language;
	/** The first difference, as `path: wire vs reconstruct`. */
	difference: string;
}

/** The loc arm's books beside its diff results: the definition check and the tolerances. */
interface LocBooks {
	/** Files whose loc wire was checked against the definition, per language. */
	checked: Record<Language, number>;
	violations: LocDefinitionViolation[];
	/** Per tolerance row: the files it absorbed a difference in, and how many differences. */
	rows: Record<LocRow, { files: number; sites: number }>;
	/** Objects carrying a tsv `loc` the oracle gives none (Svelte and CSS). */
	superset: Record<Language, number>;
	/** The superset objects by kind (`superset_key`), per language. */
	superset_kinds: Record<Language, Record<string, number>>;
	/** `loc` halves left to the span grading because the two sides' offsets there disagree. */
	span_skipped: Record<Language, number>;
}

function empty_loc_books(): LocBooks {
	const zero = (): Record<Language, number> => ({ svelte: 0, typescript: 0, css: 0 });
	return {
		checked: zero(),
		violations: [],
		rows: Object.fromEntries(LOC_ROWS.map((row) => [row, { files: 0, sites: 0 }])) as Record<
			LocRow,
			{ files: number; sites: number }
		>,
		superset: zero(),
		superset_kinds: { svelte: {}, typescript: {}, css: {} },
		span_skipped: zero()
	};
}

/** Fold one file's `diff_asts` loc tallies into the books. */
function record_loc_tolerances(
	books: LocBooks,
	lang: Language,
	rows: LocRowCounts,
	superset: Record<string, number>,
	span_skipped: number
): void {
	for (const [row, sites] of Object.entries(rows) as [LocRow, number][]) {
		books.rows[row].files++;
		books.rows[row].sites += sites;
	}
	const kinds = books.superset_kinds[lang];
	for (const [kind, count] of Object.entries(superset)) {
		books.superset[lang] += count;
		kinds[kind] = (kinds[kind] ?? 0) + count;
	}
	books.span_skipped[lang] += span_skipped;
}

// --- The two arms' books -----------------------------------------------------------

type FileStatus = FileResult['status'];

/** The counts both arms keep per language — what `ArmBooks` records and re-derives. */
interface ArmCounts {
	compared: number;
	match: number;
	documented: number;
	undocumented: number;
}

/**
 * Sum per-language counts key by key, over every key of `zero` — which also sets the
 * result's key order, the `--json` shape's.
 */
function sum_counts<T extends Record<keyof T, number>>(zero: T, all: Iterable<T>): T {
	const total: Record<keyof T, number> = zero;
	for (const s of all) {
		for (const key of Object.keys(total) as (keyof T)[]) total[key] += s[key];
	}
	return total as T;
}

/** What an arm's books need to know about the file being recorded. */
interface ArmFile {
	path: string;
	bytes: number;
	multibyte: boolean;
	/**
	 * In inject mode a base file is a CONTROL: it is parsed so its own divergences can be
	 * subtracted from its variants, and counted in neither the tables nor the stats.
	 * Letting it into a table would blend two populations behind one percentage — the
	 * reading this tool's per-source disclosures exist to prevent. Its results are still
	 * stored, which is what `ArmBooks.rederive` subtracts.
	 */
	is_control: boolean;
}

/**
 * One arm's books: its stored per-file results (diverging and failed files — exact matches
 * are counted, not stored) and its per-language counts, kept apart per arm so neither table
 * blends the two wires.
 */
class ArmBooks<S extends ArmCounts & Record<keyof S, number>> {
	readonly results: Map<Language, FileResult[]> = new Map(LANGUAGES.map((lang) => [lang, []]));
	readonly stats: Map<Language, S>;
	readonly #empty: () => S;

	/** @param empty - a zeroed per-language counts object */
	constructor(empty: () => S) {
		this.#empty = empty;
		this.stats = new Map(LANGUAGES.map((lang) => [lang, empty()]));
	}

	/**
	 * Record one file the arm compared: counted as compared, then as an exact match, a
	 * documented divergence, or an undocumented one — stored unless it matched. A file whose
	 * diff is empty but `tolerated` (the loc arm's tolerance rows absorbed a difference) is
	 * documented rather than exact.
	 */
	record(
		lang: Language,
		file: ArmFile,
		diff: Pick<DiffResult, 'diffs' | 'truncated'>,
		tolerated = false
	): void {
		const s = this.stats.get(lang)!;
		if (!file.is_control) s.compared++;
		if (diff.diffs.length === 0) {
			if (!file.is_control) {
				if (tolerated) s.documented++;
				else s.match++;
			}
			return;
		}
		const all_documented = diff.diffs.every((d) => d.documented !== null);
		if (!file.is_control) {
			if (all_documented) s.documented++;
			else s.undocumented++;
		}
		this.results.get(lang)!.push({
			path: file.path,
			bytes: file.bytes,
			multibyte: file.multibyte,
			status: all_documented ? 'documented' : 'undocumented',
			diffs: diff.diffs,
			truncated: diff.truncated
		});
	}

	/**
	 * Re-grade every manufactured input against its own base (`subtract_baseline_diffs`): a
	 * divergence — or a parse failure — the base file already had is not the injection's
	 * doing. A variant whose every diff its base already had is no longer a finding, so it
	 * is an exact match for this run's question: `match` has to be re-derived rather than
	 * left as the raw count, or the table reports those variants as neither matched nor
	 * diverging and the exact-% reads far below the truth.
	 */
	rederive(): void {
		for (const lang of LANGUAGES) {
			const kept = subtract_baseline_diffs(this.results.get(lang)!);
			this.results.set(lang, kept);
			const s = this.stats.get(lang)!;
			const count = (status: FileStatus): number => kept.filter((r) => r.status === status).length;
			s.undocumented = count('undocumented');
			s.documented = count('documented');
			s.match = s.compared - s.documented - s.undocumented;
			this.rederive_more(s, count);
		}
	}

	/** Re-derive the counts beyond `ArmCounts` after injection — none by default. */
	protected rederive_more(_s: S, _count: (status: FileStatus) => number): void {}

	/** The counts summed over every language. */
	totals(): S {
		return sum_counts(this.#empty(), this.stats.values());
	}
}

/**
 * The loc arm's books: the loc wire against the oracle, plus the counts only it keeps — every
 * file seen, the multibyte share of those compared, and the parse-failure buckets (the span
 * arm's failures are graded against these, never counted twice).
 */
class LocArmBooks extends ArmBooks<LanguageStats> {
	constructor() {
		super(empty_stats);
	}

	/** Count a file the run reached, before either side parses it. */
	record_seen(lang: Language, file: ArmFile): void {
		if (!file.is_control) this.stats.get(lang)!.total++;
	}

	/** As `ArmBooks.record`, also counting a compared file's multibyte-ness. */
	override record(
		lang: Language,
		file: ArmFile,
		diff: Pick<DiffResult, 'diffs' | 'truncated'>,
		tolerated = false
	): void {
		if (!file.is_control && file.multibyte) this.stats.get(lang)!.multibyte++;
		super.record(lang, file, diff, tolerated);
	}

	/** Record a file one side failed to parse — an error bucket, never a diff. */
	record_failure(
		lang: Language,
		file: ArmFile,
		status: 'tsv_error' | 'canonical_error' | 'both_error',
		error: string | undefined
	): void {
		if (!file.is_control) this.stats.get(lang)![FAILURE_COUNT[status]]++;
		this.results.get(lang)!.push({
			path: file.path,
			bytes: file.bytes,
			multibyte: file.multibyte,
			status,
			diffs: [],
			truncated: false,
			error
		});
	}

	protected override rederive_more(s: LanguageStats, count: (status: FileStatus) => number): void {
		// The tsv-side rejections, which the raw count gets backwards on this corpus:
		// `tests/fixtures` holds hundreds of `input_invalid_*` files plus the `tsv_rejects` set,
		// and each fails in every variant derived from it while its own base is excluded as a
		// control — so the raw number reports inherited failures as injected ones. What
		// survives here is an injection that turned an accepted document into a rejected one.
		// The oracle-side rejections are re-derived for the mirror-image reason — raw, they
		// re-report each `_svelte_divergence` fixture's own sanctioned over-acceptance once per
		// variant derived from it — and what survives there is an injection that turned a
		// document the ORACLE accepted into one only tsv does. `both_error` stays raw: no side
		// accepted it, so it is a finding about neither, and "parse-fail skipped" is the right
		// home for it.
		s.tsv_errors = count('tsv_error');
		s.canonical_errors = count('canonical_error');
	}
}

/** Each parse-failure status's count in `LanguageStats`. */
const FAILURE_COUNT = {
	tsv_error: 'tsv_errors',
	canonical_error: 'canonical_errors',
	both_error: 'both_errors'
} as const;

/**
 * The span-only arm's books. Its rows hold only diverging files (its parse failures are the
 * loc arm's to count), so its re-derivation is the signature subtraction alone; what it adds
 * is the files whose two wires disagree on whether they parse at all.
 */
class SpanArmBooks extends ArmBooks<SpanStats> {
	readonly verdict_mismatches: SpanVerdictMismatch[] = [];

	constructor() {
		super(empty_span_stats);
	}

	/** Record a file the loc and span-only wires parse differently. */
	record_verdict_mismatch(mismatch: SpanVerdictMismatch): void {
		this.stats.get(mismatch.language)!.verdict_mismatch++;
		this.verdict_mismatches.push(mismatch);
	}
}

// --- Reporting -------------------------------------------------------------------

interface DiffGroup {
	language: Language;
	signature: string;
	documented: string | null;
	files: Set<string>;
	entry_count: number;
	samples: { path: string; entry: DiffEntry }[];
}

/**
 * Group stored diff entries across files by (language, signature, documented-as).
 *
 * The matcher is part of the key, not only the signature: a signature erases array
 * indices, and a matcher can accept one index and refuse its sibling (the
 * instance-comment matcher admits the indices a module-region prefix shifts and
 * refuses any other), so keyed on the signature alone an undocumented
 * `trailingComments[1]` entry folded into the documented `trailingComments[]` group
 * — the run failed with "1 UNDOCUMENTED", the group listing showed only documented
 * groups, the JSON `groups` carried no undocumented entry, and only `--verbose`
 * named the file. Undocumented entries always form their own group now.
 */
function build_groups(results: Map<Language, FileResult[]>): DiffGroup[] {
	const groups = new Map<string, DiffGroup>();
	for (const lang of LANGUAGES) {
		for (const r of results.get(lang)!) {
			for (const entry of r.diffs) {
				const key = `${lang}:${entry.signature}:${entry.documented ?? ''}`;
				let group = groups.get(key);
				if (!group) {
					group = {
						language: lang,
						signature: entry.signature,
						documented: entry.documented,
						files: new Set(),
						entry_count: 0,
						samples: []
					};
					groups.set(key, group);
				}
				group.entry_count++;
				if (!group.files.has(r.path) && group.samples.length < MAX_GROUP_SAMPLES) {
					group.samples.push({ path: r.path, entry });
				}
				group.files.add(r.path);
			}
		}
	}
	return [...groups.values()].sort((a, b) => b.files.size - a.files.size);
}

function print_usage(): void {
	console.log(`
Usage: deno task corpus:compare:parse <path> [options]
       deno task corpus:compare:parse --all [options]

Deep-diffs tsv's parse output against the canonical parsers (acorn-typescript /
svelte / parseCss) on two arms: the shipped span-only wire (FFI) and the Rust \`loc\`
emitter's wire (\`tsv_debug loc_wires --stdin\`). Raw diff first, documented-divergence
classification at the reporting layer only.

Arguments:
  path               Directory to scan for source files
  --all              Compare all default corpus repos

Options:
  --filter <lang>    Only compare files of this language (svelte, typescript, css)
  --limit <n>        Limit to first n files per language
  --multibyte-only   Only compare files with non-ASCII source (the offset-translation slice)
  --inject           Also compare MANUFACTURED inputs: whitespace injected inside every
                     Svelte tag/block head (see lib/wire_inject.ts). Off by default —
                     it multiplies canonical parses, and its findings are about spellings
                     no corpus contains rather than about the corpus
  --inject-terminators  The same, injecting a lone CR / U+2028 / U+2029 anywhere in the
                     document — where ECMAScript's terminators and \\n disagree, so its
                     \`loc\` differences are all one tolerance row and it grades the span
                     arm. Composes with --inject; the per-file budget is split between them
  --fixtures         Treat <path> as a fixture tree: grade each fixture's parse-pinned
                     documents (its input.* at its \`goal\` marker's goal, plus the variants an
                     expected_<stem>.json pins) and nothing else. A _svelte_divergence input
                     whose two parses equal its expected_ours.json / expected_svelte.json is
                     classified \`fixture_declared_divergence\` on its span differences
                     only. What \`deno task conformance\` runs over tests/fixtures
  --inject-limit <n> Injected variants per file, across all kinds (default 12).
                     0 = CENSUS: every site, no cap — the only mode whose finding set is
                     stable across fixture edits, and what wire:audit runs
  --verbose          Show each file as it's processed + per-file diff detail
  --json             Emit a single JSON report to stdout; human output → stderr
  --help             Show this help message

Examples:
  deno task corpus:compare:parse --all
  deno task corpus:compare:parse --all --multibyte-only
  deno task corpus:compare:parse ../corpora/collections/zzz --filter typescript --limit 100
  deno task corpus:compare:parse tests/fixtures --filter svelte --inject
  deno task corpus:compare:parse tests/fixtures --filter svelte --inject-terminators
  deno task corpus:compare:parse --all --json 2>/dev/null | jq '.stats.total'
`);
}

/** The `stats` block: per-language counts (languages the run saw) plus a summed total. */
function build_stats_block(books: ArmBooks<LanguageStats>) {
	const languages: Record<string, LanguageStats> = {};
	for (const lang of LANGUAGES) {
		const s = books.stats.get(lang)!;
		if (s.total > 0) languages[lang] = { ...s };
	}
	return { languages, total: books.totals() };
}

/** One diff group as the JSON report carries it — samples previewed, paths repo-relative. */
function group_to_json(g: DiffGroup, base_path: string) {
	return {
		language: g.language,
		signature: g.signature,
		documented: g.documented,
		file_count: g.files.size,
		entry_count: g.entry_count,
		files: [...g.files].slice(0, 20).map((p) => rel_path(p, base_path)),
		samples: g.samples.map((s) => ({
			file: rel_path(s.path, base_path),
			path: s.entry.path,
			kind: s.entry.kind,
			ours: preview(s.entry.ours),
			canonical: preview(s.entry.canonical)
		}))
	};
}

/** The span-only arm's books, as the run hands them to the JSON report. */
interface SpanReport {
	books: ArmBooks<SpanStats>;
	groups: DiffGroup[];
	verdict_mismatches: SpanVerdictMismatch[];
}

/** The `span_only` block: per-language counts plus a summed total, its groups and verdict mismatches. */
function build_span_json(span: SpanReport, base_path: string) {
	const languages: Record<string, SpanStats> = {};
	for (const [lang, s] of span.books.stats) languages[lang] = { ...s };
	return {
		stats: { languages, total: span.books.totals() },
		groups: span.groups.map((g) => group_to_json(g, base_path)),
		verdict_mismatches: span.verdict_mismatches.map((m) => ({
			...m,
			path: rel_path(m.path, base_path)
		}))
	};
}

/** Build the single buffered JSON report. */
function build_json_report(
	loc_arm: ArmBooks<LanguageStats>,
	groups: DiffGroup[],
	span: SpanReport,
	loc: LocBooks,
	base_path: string
): Record<string, unknown> {
	return {
		stats: build_stats_block(loc_arm),
		groups: groups.map((g) => group_to_json(g, base_path)),
		span_only: build_span_json(span, base_path),
		loc_arm: {
			checked: loc.checked,
			violations: loc.violations.map((v) => ({ ...v, path: rel_path(v.path, base_path) })),
			rows: loc.rows,
			superset: loc.superset,
			superset_kinds: loc.superset_kinds,
			span_skipped: loc.span_skipped
		},
		errors: LANGUAGES.flatMap((lang) =>
			loc_arm.results
				.get(lang)!
				.filter((r) => r.status.endsWith('_error'))
				.map((r) => ({
					path: rel_path(r.path, base_path),
					language: lang,
					status: r.status,
					error: r.error
				}))
		),
		truncated_files: LANGUAGES.flatMap((lang) =>
			loc_arm.results
				.get(lang)!
				.filter((r) => r.truncated)
				.map((r) => rel_path(r.path, base_path))
		)
	};
}

/** The rule above an arm table's total row. */
const TABLE_RULE = '  ' + '─'.repeat(72);

/**
 * Print one arm-table row: the exact-match share, then `lead`, the documented and
 * undocumented counts, and `tail` — or `all exact` when there is nothing to list.
 */
function print_arm_row(label: string, s: ArmCounts, lead: string[], tail: string[]): void {
	const pct = s.compared > 0 ? ((s.match / s.compared) * 100).toFixed(1) : '100.0';
	const match_str = `${s.match}/${s.compared} exact (${pct}%)`.padEnd(26);
	const parts = [...lead];
	if (s.documented > 0) parts.push(`${s.documented} documented`);
	if (s.undocumented > 0) parts.push(`\x1b[31m${s.undocumented} UNDOCUMENTED\x1b[0m`);
	parts.push(...tail);
	console.log(`  ${label.padEnd(12)} ${match_str} | ${parts.join(' | ') || 'all exact'}`);
}

/** A loc-arm row: multibyte files ahead of the counts, the parse-fail skips behind them. */
function print_loc_row(label: string, s: LanguageStats): void {
	const skipped = s.tsv_errors + s.canonical_errors + s.both_errors;
	print_arm_row(
		label,
		s,
		s.multibyte > 0 ? [`${s.multibyte} multibyte`] : [],
		skipped > 0 ? [`\x1b[2m${skipped} parse-fail skipped\x1b[0m`] : []
	);
}

/** A span-only-arm row: a verdict mismatch behind the counts. */
function print_span_row(label: string, s: SpanStats): void {
	print_arm_row(
		label,
		s,
		[],
		s.verdict_mismatch > 0
			? [`\x1b[31m${s.verdict_mismatch} VERDICT MISMATCH (loc vs span wire)\x1b[0m`]
			: []
	);
}

/**
 * The tool's entry, importable by the `conformance.ts` driver (which passes
 * `['--all']`); the CLI wrapper at the bottom feeds it the real argv. Failure
 * semantics are process-level (`Deno.exit(1)` at each gate), matching the old
 * `&&`-chain aggregate exactly.
 */
export async function run_corpus_compare_parse(argv: string[] = Deno.args): Promise<void> {
	const parsed = args_parse(argv_parse(argv), CorpusCompareParseArgs);
	if (!parsed.success) {
		console.error(z.prettifyError(parsed.error));
		print_usage();
		Deno.exit(1);
	}
	const args = parsed.data;

	if (args.help) {
		print_usage();
		return;
	}

	const use_all_repos = args.all;
	const path = args._[0]?.toString();

	if (!path && !use_all_repos) {
		console.error('Error: No path provided (use --all for all repos)\n');
		print_usage();
		Deno.exit(1);
	}

	const base_path = resolve_compare_base_path(path, use_all_repos);
	const filter_lang = parse_language_filter(args.filter);

	const limit = args.limit;
	const multibyte_only = args['multibyte-only'];
	const verbose = args.verbose;
	const json_mode = args.json;

	if (json_mode) redirect_logs_to_stderr();

	console.log(
		use_all_repos ? 'Parse-comparing: All default corpus repos' : `Parse-comparing: ${base_path}`
	);
	if (filter_lang) console.log(`Filter: ${filter_lang} only`);
	if (limit) console.log(`Limit: ${limit} files per language`);
	if (multibyte_only) console.log('Mode: multibyte-only (offset-translation slice)');
	const inject_kinds: InjectKind[] = [
		...(args.inject ? (['ws'] as const) : []),
		...(args['inject-terminators'] ? (['terminators'] as const) : [])
	];
	const injecting = inject_kinds.length > 0;
	const fixtures_mode = args.fixtures;
	if (fixtures_mode && (injecting || use_all_repos)) {
		console.error(
			'Error: --fixtures grades a fixture tree as authored; it takes neither --all nor --inject*'
		);
		Deno.exit(1);
	}
	if (fixtures_mode) {
		console.log(
			"Mode: fixture tree — each fixture's parse-pinned documents, at its `goal` marker's goal"
		);
	}
	// `--fixtures`: documents whose differences were all their fixture's declared divergence.
	let declared_divergences = 0;
	if (injecting) {
		console.log(
			`Mode: +injection into Svelte inputs [${inject_kinds.join(', ')}] ` +
				(args['inject-limit'] > 0 ? `(<=${args['inject-limit']}/file)` : '(CENSUS: every site)')
		);
	}
	console.log();

	const loader = create_compare_loader(use_all_repos, base_path);
	const impls = await init_compare_implementations();
	const { canonical, native } = impls;
	// The loc wire ships through no binding, so the loc arm asks `tsv_debug` for it
	// (`lib/loc_wire_client.ts`); the span-only wire is the FFI's.
	const loc_wires = await LocWireClient.start();

	const loc_arm = new LocArmBooks();
	const span_arm = new SpanArmBooks();
	// Span-wire panics, held for `gate_on_panics` beside the loc arm's failures: a file
	// whose loc wire throws a plain rejection while the span wire panics reads as
	// both-errored (no verdict mismatch) and is skipped, so nothing else would see it.
	const span_panics: CompareFailure[] = [];
	// The loc arm's own books: tsv's loc wire against the definition (the reconstruction of
	// its span-only wire — any violation fails), and the oracle departures it tolerated.
	const loc_books = empty_loc_books();

	const lang_counts: Record<Language, number> = { svelte: 0, typescript: 0, css: 0 };
	// Inject-mode split of `lang_counts`: the base files parsed only to seed the
	// subtraction, and the manufactured inputs the run is actually about.
	let controls = 0;
	let variants = 0;

	const stream = injecting
		? with_injected_variants(
				loader.stream(verbose ? console.log : () => {}),
				args['inject-limit'],
				inject_kinds
			)
		: loader.stream(verbose ? console.log : () => {});

	for await (const file of stream) {
		const lang = file.language;
		if (filter_lang && lang !== filter_lang) continue;
		const multibyte = has_non_ascii(file.content);
		if (multibyte_only && !multibyte) continue;
		const fixture = fixtures_mode ? fixture_document(file.path) : null;
		if (fixtures_mode && fixture === null) continue;
		if (limit && lang_counts[lang] >= limit) continue;
		lang_counts[lang]++;
		const goal = fixture?.goal ?? file.goal;

		// See `ArmFile.is_control`.
		const is_control = injecting && !file.path.includes(VARIANT_MARKER);
		if (is_control) controls++;
		else variants++;
		const arm_file: ArmFile = { path: file.path, bytes: file.bytes, multibyte, is_control };

		loc_arm.record_seen(lang, arm_file);

		if (verbose) console.log(`  ${file.path}`);

		// Parse both sides; a throw on either side is an error bucket, not a diff.
		let ours: unknown;
		let tsv_error: string | null = null;
		try {
			ours = await loc_wires.parse(file.content, lang, goal);
		} catch (e) {
			tsv_error = first_line(e);
		}
		let canonical_ast: unknown;
		let canonical_error: string | null = null;
		let canonical_raw: unknown;
		try {
			canonical_raw = canonical.parse(file.content, lang, goal);
			// Serialize exactly like the fixture sidecar does (BigInt → string;
			// RegExp values collapse to {}), so corpus and fixture semantics match.
			canonical_ast = JSON.parse(JSON.stringify(canonical_raw, bigint_replacer));
		} catch (e) {
			canonical_error = first_line(e);
		}

		// The span-only arm (see `lib/span_only.ts`). Its controls are recorded but not counted,
		// like the loc arm's, so `subtract_baseline_diffs` can grade a variant against them.
		let declared_divergence = false;
		let ours_span: unknown;
		let span_error: string | null = null;
		try {
			ours_span = native.parse(file.content, lang, goal);
		} catch (e) {
			span_error = first_line(e);
			if (is_native_panic_error(span_error)) {
				span_panics.push({ path: file.path, error: span_error });
			}
		}
		if ((span_error === null) !== (tsv_error === null)) {
			span_arm.record_verdict_mismatch({
				path: file.path,
				language: lang,
				loc_wire: tsv_error ?? 'parsed',
				span_wire: span_error ?? 'parsed'
			});
		} else if (span_error === null && canonical_error === null) {
			const canonical_span = JSON.parse(JSON.stringify(canonical_raw, span_only_replacer));
			// A `_svelte_divergence` fixture's difference is declared only while both parses
			// still ARE its committed pins — the moment either moves, every difference grades.
			declared_divergence =
				fixture?.declared != null &&
				first_difference(ours_span, fixture.declared.ours) === null &&
				first_difference(canonical_span, fixture.declared.svelte) === null;
			if (declared_divergence) declared_divergences++;
			const span = diff_asts(ours_span, canonical_span, {
				source: file.content,
				canonical_root: canonical_span,
				language: lang,
				declared_divergence
			});
			span_arm.record(lang, arm_file, span);
		}

		// The definition check, ahead of the oracle: tsv's loc wire must equal the shipped
		// reconstruction of its span-only wire. A violation is a tsv bug on every run —
		// controls included, since a control's loc wire is no less tsv's — and is never
		// tolerated. The reconstruction runs on a clone, so the span arm's stored samples
		// above keep the span-only wire they graded.
		if (tsv_error === null && span_error === null) {
			const violation = loc_definition_violation(ours, ours_span, file.content, lang);
			loc_books.checked[lang]++;
			if (violation !== null) {
				loc_books.violations.push({ path: file.path, language: lang, difference: violation });
			}
		}

		if (tsv_error || canonical_error) {
			const status =
				tsv_error && canonical_error ? 'both_error' : tsv_error ? 'tsv_error' : 'canonical_error';
			loc_arm.record_failure(lang, arm_file, status, tsv_error ?? canonical_error ?? undefined);
			continue;
		}

		const loc = diff_asts(ours, canonical_ast, {
			source: file.content,
			canonical_root: canonical_ast,
			language: lang,
			declared_divergence
		});
		if (!is_control) {
			record_loc_tolerances(loc_books, lang, loc.loc_rows, loc.loc_superset, loc.loc_span_skipped);
		}
		// exact matches are counted, not stored — and so is a file whose only differences
		// are tolerated `loc` rows, which is documented rather than exact
		loc_arm.record(lang, arm_file, loc, Object.keys(loc.loc_rows).length > 0);

		if (verbose) {
			for (const d of loc.diffs) {
				const tag = d.documented ? `documented:${d.documented}` : 'UNDOCUMENTED';
				console.log(`    ${d.kind} at ${d.path} (${tag})`);
				console.log(`      ours: ${preview(d.ours)}  canonical: ${preview(d.canonical)}`);
			}
		}
	}

	await loc_wires.close();

	if (injecting) {
		loc_arm.rederive();
		span_arm.rederive();
	}

	const total_processed = Object.values(lang_counts).reduce((a, b) => a + b, 0);
	if (total_processed === 0) {
		// An empty scope is a failed comparison run, not a pass — an existing-but-
		// source-empty path (typo, moved src/) must not read as green.
		console.log('No files found — nothing was compared.');
		if (json_mode) {
			emit_json_stdout(
				build_json_report(
					loc_arm,
					[],
					{ books: span_arm, groups: [], verdict_mismatches: [] },
					loc_books,
					base_path
				)
			);
		}
		exit_compare_failure(impls);
	}

	const counts = LANGUAGES.map((lang) => `${lang_counts[lang]} ${lang}`).join(', ');
	const processed_detail = injecting
		? `${counts} — ${variants} injected variants over ${controls} base controls`
		: counts;
	console.log(`\nProcessed: ${total_processed} files (${processed_detail})\n`);

	// Per-language results table
	console.log('Results (AST deep-diff vs canonical):');
	const totals = loc_arm.totals();
	for (const lang of LANGUAGES) {
		const s = loc_arm.stats.get(lang)!;
		if (s.total > 0) print_loc_row(lang, s);
	}
	if (totals.compared > 0) {
		console.log(TABLE_RULE);
		print_loc_row('total', totals);
	}
	const total_skipped = totals.tsv_errors + totals.canonical_errors + totals.both_errors;
	if (total_skipped > 0) {
		console.log(
			`\n\x1b[2mParse failures skipped (tsv ${totals.tsv_errors} / canonical ${totals.canonical_errors} / both ${totals.both_errors}) — triage with diagnostics/skip_triage.ts\x1b[0m`
		);
		// In inject mode the first two numbers are baseline-subtracted FINDINGS, not skips:
		// each is an injection that moved a verdict, one per direction of the drop-in claim.
		// Both are listed per file in `--json`'s `errors`, which is the only place the
		// `#inj:` label that reproduces them survives.
		if (injecting && totals.tsv_errors + totals.canonical_errors > 0) {
			console.log(
				`\x1b[2m  of those, injected: ${totals.tsv_errors} over-rejection(s) (tsv refused what it accepted before) and ${totals.canonical_errors} over-acceptance(s) (canonical refused, tsv did not) — per-file in --json\x1b[0m`
			);
		}
	}

	// The span-only arm's table: the same deep diff over the span-only wire, against
	// the oracle with its line/column objects stripped (see `lib/span_only.ts`).
	const span_totals = span_arm.totals();
	if (span_totals.compared + span_totals.verdict_mismatch > 0) {
		console.log('\nSpan-only arm (span-only wire vs canonical with `loc`/`name_loc` stripped):');
		for (const [lang, s] of span_arm.stats) {
			if (s.compared + s.verdict_mismatch > 0) print_span_row(lang, s);
		}
		console.log(TABLE_RULE);
		print_span_row('total', span_totals);
	}

	// The loc arm's own lines: the definition check (tsv's loc wire against the
	// reconstruction of its span-only wire), then what the oracle comparison tolerated.
	const loc_checked = LANGUAGES.reduce((n, lang) => n + loc_books.checked[lang], 0);
	console.log(
		`\nLoc arm: definition check over ${loc_checked} files — ` +
			(loc_books.violations.length === 0
				? "tsv's loc wire is the reconstruction of its span-only wire on every one"
				: `\x1b[31m${loc_books.violations.length} VIOLATION(S)\x1b[0m`)
	);
	const superset_total = LANGUAGES.reduce((n, lang) => n + loc_books.superset[lang], 0);
	const span_skipped_total = LANGUAGES.reduce((n, lang) => n + loc_books.span_skipped[lang], 0);
	console.log(
		`  superset (tsv \`loc\`, oracle none): ${superset_total} objects` +
			` (${LANGUAGES.filter((l) => loc_books.superset[l] > 0)
				.map((l) => {
					const kinds = Object.keys(loc_books.superset_kinds[l]).length;
					return `${l} ${loc_books.superset[l]} over ${kinds} pinned kinds`;
				})
				.join(', ')})` +
			` | halves left to the span grading (offsets differ): ${span_skipped_total}`
	);
	if (fixtures_mode) {
		console.log(
			`  _svelte_divergence inputs at their committed pins (their span difference is the declared one): ${declared_divergences}`
		);
	}
	console.log('  tolerance rows (oracle departs from the definition; Svelte only):');
	LOC_ROWS.forEach((row, i) => {
		const r = loc_books.rows[row];
		console.log(
			`    ${i + 1}. ${row.padEnd(24)} ${String(r.files).padStart(5)} files ${String(r.sites).padStart(7)} sites`
		);
	});

	const groups = build_groups(loc_arm.results);
	const undocumented_groups = groups.filter((g) => g.documented === null);
	const documented_groups = groups.filter((g) => g.documented !== null);
	const span_groups = build_groups(span_arm.results);
	const span_undocumented_groups = span_groups.filter((g) => g.documented === null);

	if (json_mode) {
		emit_json_stdout(
			build_json_report(
				loc_arm,
				groups,
				{ books: span_arm, groups: span_groups, verdict_mismatches: span_arm.verdict_mismatches },
				loc_books,
				base_path
			)
		);
	}

	// A caught panic hard-fails ahead of the "compared nothing" floor and the
	// count pins — a tsv-side parse failure is otherwise dimmed as "skipped", and
	// only `--all`'s exact `CORPUS_PARSE_TSV_ERRORS_PIN` would notice, reporting a
	// crash as one more over-rejection. `canonical_error` files are excluded: their
	// recorded message is the JS oracle's, which can't be a panic. The span-only arm's
	// panics ride along — no other check of that arm reaches a file both wires reject.
	gate_on_panics(
		[
			...LANGUAGES.flatMap((lang) =>
				loc_arm.results.get(lang)!.filter((r) => r.status !== 'canonical_error')
			),
			...span_panics
		],
		impls,
		base_path
	);

	// Floor: a run where NOTHING was compared (every file parse-fail-skipped on
	// one side) is a systemic failure, not a pass — without this, an FFI or
	// sidecar breakage would zero out `compared` and sail through green.
	if (totals.compared === 0) {
		console.log(
			`\x1b[31mFAIL: 0 of ${total_processed} files compared (all parse-fail-skipped) — systemic failure or wrong corpus?\x1b[0m`
		);
		exit_compare_failure(impls);
	}

	// The span-only arm's vacuity guard, structural rather than pinned: it diffs exactly the
	// files the loc arm compares (both tsv wires and the oracle parsed — a verdict mismatch
	// fails on its own), so per language its `compared` must EQUAL the loc arm's — controls
	// excluded from both under injection. An arm that silently stopped running would
	// otherwise read as zero findings, not a failure. Unlike a count pin this holds on a
	// narrowed root too, and a corpus refresh costs no re-pin. Skipped for a language with a
	// verdict mismatch, which the loc arm counts and the span arm cannot — that failure
	// reports itself below, with its paths. The definition check rides the same guard: it
	// runs on every file both tsv wires parsed, so it can never have checked fewer than the
	// loc arm compared.
	const population_failures = LANGUAGES.flatMap((lang) => {
		const span = span_arm.stats.get(lang)!;
		const compared = loc_arm.stats.get(lang)!.compared;
		const out: string[] = [];
		if (span.verdict_mismatch === 0 && span.compared !== compared) {
			out.push(`${lang} span-only compared ${span.compared} ≠ loc-arm compared ${compared}`);
		}
		if (span.verdict_mismatch === 0 && loc_books.checked[lang] < compared) {
			out.push(
				`${lang} loc definition checked ${loc_books.checked[lang]} < loc-arm compared ${compared}`
			);
		}
		return out;
	});
	if (population_failures.length > 0) {
		console.log(`\x1b[31mFAIL: arm population — ${population_failures.join('; ')}\x1b[0m`);
		exit_compare_failure(impls);
	}

	// Pinned counts (--all only — see lib/gate_counts.ts): EXACT per-language
	// `compared` (the corpus is the pinned `../corpora` snapshot + the prettier
	// suites, so any move is a corpus refresh or a one-language parse collapse that
	// can't hide under the cross-language total), and EXACT per-language tsv-side
	// parse-failure counts (up = a new over-rejection on real code — triage with
	// diagnostics/skip_triage.ts; down = a gap closed, re-pin to record the win).
	if (use_all_repos) {
		const pin_failures = [
			...LANGUAGES.filter(
				(lang) => loc_arm.stats.get(lang)!.compared !== CORPUS_PARSE_COMPARED_PIN[lang]
			).map(
				(lang) =>
					`${lang} compared ${loc_arm.stats.get(lang)!.compared} ≠ pinned ${CORPUS_PARSE_COMPARED_PIN[lang]}`
			),
			...LANGUAGES.filter(
				(lang) => loc_arm.stats.get(lang)!.tsv_errors !== CORPUS_PARSE_TSV_ERRORS_PIN[lang]
			).map(
				(lang) =>
					`${lang} tsv-parse-failures ${loc_arm.stats.get(lang)!.tsv_errors} ≠ pinned ${CORPUS_PARSE_TSV_ERRORS_PIN[lang]}`
			)
		];
		if (pin_failures.length > 0) {
			console.log(
				`\x1b[31mFAIL: pinned counts — ${pin_failures.join('; ')}. If deliberate, re-pin in lib/gate_counts.ts.\x1b[0m`
			);
			exit_compare_failure(impls);
		}
	}

	// Undocumented groups — the actionable output, one listing per arm
	const print_undocumented = (label: string, undocumented: DiffGroup[]): void => {
		if (undocumented.length === 0) return;
		console.log(`\n\x1b[31mUNDOCUMENTED ${label} (${undocumented.length}):\x1b[0m`);
		for (const g of undocumented) {
			console.log(
				`\n  [${g.language}] ${g.signature}  (${g.files.size} files, ${g.entry_count} sites)`
			);
			for (const s of g.samples) {
				console.log(`    ${rel_path(s.path, base_path)}`);
				console.log(`      at ${s.entry.path}`);
				console.log(
					`      ours: ${preview(s.entry.ours)}  canonical: ${preview(s.entry.canonical)}`
				);
			}
			if (g.files.size > g.samples.length) {
				console.log(`    ... and ${g.files.size - g.samples.length} more files`);
			}
		}
	};
	print_undocumented('diff groups', undocumented_groups);
	print_undocumented('span-only diff groups', span_undocumented_groups);
	if (span_arm.verdict_mismatches.length > 0) {
		console.log(
			`\n\x1b[31mVERDICT MISMATCH between the loc and span-only wires (${span_arm.verdict_mismatches.length}):\x1b[0m`
		);
		for (const m of span_arm.verdict_mismatches.slice(0, 10)) {
			console.log(`  [${m.language}] ${rel_path(m.path, base_path)}`);
			console.log(`    loc wire: ${m.loc_wire}  span wire: ${m.span_wire}`);
		}
	}

	if (loc_books.violations.length > 0) {
		console.log(
			`\n\x1b[31mLOC DEFINITION VIOLATIONS — tsv's loc wire is not the reconstruction of its span-only wire (${loc_books.violations.length}):\x1b[0m`
		);
		for (const v of loc_books.violations.slice(0, 10)) {
			console.log(`  [${v.language}] ${rel_path(v.path, base_path)}`);
			console.log(`    wire vs reconstruct at ${v.difference}`);
		}
	}

	// Documented groups — compact summary only
	if (documented_groups.length > 0) {
		console.log(`\nDocumented divergence groups (${documented_groups.length}):`);
		for (const g of documented_groups) {
			console.log(`  [${g.language}] ${g.documented}: ${g.signature} (${g.files.size} files)`);
		}
	}

	const truncated_files = LANGUAGES.flatMap((lang) =>
		loc_arm.results.get(lang)!.filter((r) => r.truncated)
	);
	if (truncated_files.length > 0) {
		console.log(
			`\n\x1b[33mNote: ${truncated_files.length} file(s) hit the ${MAX_DIFFS_PER_FILE}-diff cap — diff lists are partial:\x1b[0m`
		);
		for (const r of truncated_files.slice(0, 5)) {
			console.log(`  ${rel_path(r.path, base_path)}`);
		}
		if (truncated_files.length > 5) {
			console.log(`  ... and ${truncated_files.length - 5} more`);
		}
	}

	console.log();
	const failures = [
		...(totals.undocumented > 0
			? [`${totals.undocumented} file(s) with undocumented AST diffs vs canonical`]
			: []),
		...(span_totals.undocumented > 0
			? [`${span_totals.undocumented} file(s) with undocumented span-only AST diffs vs canonical`]
			: []),
		...(span_totals.verdict_mismatch > 0
			? [`${span_totals.verdict_mismatch} file(s) the loc and span-only wires parse differently`]
			: []),
		...(loc_books.violations.length > 0
			? [`${loc_books.violations.length} file(s) whose loc wire breaks the loc definition`]
			: [])
	];
	if (failures.length > 0) {
		console.log(`\x1b[31mFAIL: ${failures.join('; ')}\x1b[0m`);
		exit_compare_failure(impls);
	} else {
		console.log(
			'\x1b[32mPASS: no undocumented AST diffs vs canonical, on either wire, and the loc wire is the definition\x1b[0m'
		);
	}

	dispose_compare(impls);
}

/**
 * Minimal but valid JSON report for the failure path, keeping the contract
 * that `--json` always writes a parseable document to stdout.
 */
function build_error_json_report(message: string): Record<string, unknown> {
	return {
		...build_json_report(
			new LocArmBooks(),
			[],
			{ books: new SpanArmBooks(), groups: [], verdict_mismatches: [] },
			empty_loc_books(),
			''
		),
		error: message
	};
}

// Guarded so the conformance driver can import `run_corpus_compare_parse` without
// running the CLI on import.
if (import.meta.main) {
	run_compare_main(run_corpus_compare_parse, CorpusCompareParseArgs, build_error_json_report);
}
