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
 * Method: ASTs are raw-diffed with NO normalization applied before diffing;
 * diffs are classified against the documented divergences
 * (docs/conformance_svelte.md) at the REPORTING layer only — so a bug in our
 * own divergence reasoning surfaces as an undocumented group instead of being
 * silently absorbed. The canonical AST is serialized exactly like the fixture
 * sidecar serializes it (JSON round-trip with BigInt → string), so fixture and
 * corpus semantics match; the tsv side is the shipped FFI wire
 * (`convert_ast_json_string`), which the WASM artifact shares post-Win-1.
 * The span-only arm diffs the FFI `no-locations` wire against that same oracle
 * output with every `loc` / `name_loc` removed — the wire's own contract (it
 * omits exactly those keys), not a tolerance; everything else is raw-diffed as
 * above.
 *
 * `loc` is graded by its own rules on the loc arm (see `diff_asts`): tsv's `loc` must
 * first be the definition — the shipped `locations.js` reconstruction of its own
 * span-only wire (`lib/loc_cross_grade.ts`) — and only then is it compared with the
 * oracle's: exactly for TypeScript, and for Svelte with the pinned superset and the six
 * named tolerance rows (`lib/loc_tolerance.ts`), where a seventh difference fails.
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
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
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
import { CORPUS_PARSE_COMPARED_PIN, CORPUS_PARSE_TSV_ERRORS_PIN } from './lib/gate_counts.ts';
import { first_difference, loc_definition_violation } from './lib/loc_cross_grade.ts';
import {
	classify_loc_difference,
	get_at_path,
	is_loc_leaf,
	LOC_ROWS,
	type LocRow,
	superset_key,
	superset_kinds_of,
	type SupersetAnchor,
	wire_text
} from './lib/loc_tolerance.ts';
import { type Language, LANGUAGES, type ParseGoal } from './lib/types.ts';
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

/** Per-file diff cap — collection stops here and the file is flagged truncated. */
const MAX_DIFFS_PER_FILE = 50;

/** Max sample entries shown per diff group in the report. */
const MAX_GROUP_SAMPLES = 3;

type DiffKind =
	'value_mismatch' | 'type_mismatch' | 'missing_ours' | 'missing_canonical' | 'length_mismatch';

export interface DiffEntry {
	/** Concrete path into the AST, e.g. `body[3].declarations[0].init.start` */
	path: string;
	/** Grouping key: kind + path with array indices normalized to `[]` */
	signature: string;
	kind: DiffKind;
	ours: unknown;
	canonical: unknown;
	/** Matched documented-divergence name, or null = undocumented (actionable) */
	documented: string | null;
}

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

// The BigInt half of the fixture sidecar's jsonReplacer
// (crates/tsv_debug/src/deno/sidecar.ts), so corpus comparison and expected.json
// generation agree on the values neither can serialize natively. Exported so
// diagnostics/svelte_fixtures_compare.ts serializes its canonical AST the same way.
//
// ⚠️ Deliberately NOT the sidecar's whole replacer: the sidecar also substitutes
// U+FFFD for lone surrogates, because its response crosses a Rust boundary where
// serde_json rejects the document outright. Nothing crosses a boundary here, so the
// canonical AST keeps acorn's TRUE lone-surrogate value — which is the only reason
// the `lone_surrogate_value` divergence detector below can still fire. Substituting
// here would make the canonical side agree with ours by construction and silently
// retire that detector.
export function bigint_replacer(_key: string, value: unknown): unknown {
	return typeof value === 'bigint' ? value.toString() : value;
}

// --- The span-only arm -----------------------------------------------------------
//
// tsv's `no-locations` wire is graded against the SAME oracle output with its line/column
// objects removed — the definition `tests/no_locations.rs` (`strip_locations`) encodes:
// every `loc` key and every Svelte `name_loc` key, anywhere in the tree (the `character`
// field Svelte puts on a name-shaped or in-tag-comment position lives inside one of those,
// so it goes too). Nothing else differs between the two wires, so the arm reuses the diff
// engine and the documented matchers unchanged: a span difference the loc arm excuses is
// excused identically here, and the `loc` rules have no `loc` to grade. Every language has
// the arm, CSS included — `parseCss` emits no `loc`, so for CSS the stripped oracle is the
// oracle itself.
//
// The same span-only wire is what the loc arm's definition check reconstructs from: before
// tsv's loc wire is graded against the oracle, `loc_definition_violation` requires it to
// equal the shipped reconstruction of this wire, so a tolerance row can only excuse the
// oracle's departure from the definition, never tsv's.

/** The keys the span-only wire drops — see the section comment above. */
const LOCATION_KEYS: ReadonlySet<string> = new Set(['loc', 'name_loc']);

/** `bigint_replacer` that also drops the span-only wire's dropped keys. */
function span_only_replacer(key: string, value: unknown): unknown {
	return LOCATION_KEYS.has(key) ? undefined : bigint_replacer(key, value);
}

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

/** Truncated single-line preview of a leaf value for reports. */
function preview(value: unknown): string {
	if (value === undefined) return '(absent)';
	let s: string;
	try {
		s = JSON.stringify(value) ?? 'undefined';
	} catch {
		s = String(value);
	}
	return s.length > 60 ? s.slice(0, 57) + '...' : s;
}

type ValueType = 'null' | 'array' | 'object' | 'string' | 'number' | 'boolean' | 'undefined';

function value_type(v: unknown): ValueType {
	if (v === null) return 'null';
	if (Array.isArray(v)) return 'array';
	return typeof v as ValueType;
}

// --- Documented divergence classification -------------------------------------
//
// Classification happens at the REPORTING layer, after the raw diff — never as
// pre-diff normalization. Matchers cover the documented AST-content divergences
// in docs/conformance_svelte.md that parse successfully on both sides (the
// parser-FEATURE corrections there — `using`, v-flag regex, CSS namespaces —
// make the canonical parser throw, so they land in the error buckets instead).
// When triage confirms a new group is a documented divergence, add a matcher
// here AND ensure the divergence is cataloged in conformance_svelte.md.

/** Per-file context available to matchers (some divergences are file-level, e.g. BOM). */
export interface MatchContext {
	source: string;
	/** Root of the canonical AST — lets matchers resolve ancestors from the entry path. */
	canonical_root: unknown;
	/**
	 * The document's language, which decides how `loc` is graded: exactly for TypeScript
	 * (acorn is the outside reference), with the pinned superset and the six tolerance rows
	 * for Svelte (`lib/loc_tolerance.ts`), and as a pinned superset for CSS (`parseCss`
	 * emits no `loc`).
	 */
	language: Language;
	/**
	 * `--fixtures` only: this document is a `_svelte_divergence` fixture's input whose two
	 * parses equal its committed `expected_ours.json` / `expected_svelte.json` — so every
	 * span difference is the one the fixture declares (and `fixtures:validate` grades).
	 * Never excuses a `loc` or `name_loc` difference.
	 *
	 * Its reach is the whole document, not the declared difference: while both parses
	 * equal their pins, EVERY span difference in it reads as declared — the same as
	 * excluding those fixtures from span grading while still grading their `loc`. So a
	 * divergence baked into `expected_ours.json` stays invisible here, exactly as it does
	 * to `fixtures:validate`, which compares tsv against that same pin; the pins' review
	 * is what grades it.
	 */
	declared_divergence?: boolean;
}

/** Svelte's `Nth.value` tell for an `of S` argument — see `subtree_has_nth_of`. */
const NTH_OF_TELL = /\sof\s*$/;

/**
 * True if `node`'s subtree contains an `Nth` whose value ends in Svelte's `of` word
 * (a normal Nth value never does) — the tell of a `:nth-child(An+B of S)` argument.
 * The word is followed by a space when the source spaces it (`2n of `) and by nothing
 * when the selector is glued to it (`2n of.a` → `2n of`).
 */
function subtree_has_nth_of(node: unknown): boolean {
	if (node == null || typeof node !== 'object') return false;
	const n = node as Record<string, unknown>;
	if (n.type === 'Nth' && typeof n.value === 'string' && NTH_OF_TELL.test(n.value)) return true;
	for (const v of Object.values(n)) {
		if (Array.isArray(v)) {
			if (v.some(subtree_has_nth_of)) return true;
		} else if (v && typeof v === 'object' && subtree_has_nth_of(v)) {
			return true;
		}
	}
	return false;
}

/** True if `property` carries a tell of Svelte's `read_declaration` tokenization garbage. */
const is_garbage_property = (property: unknown): boolean =>
	typeof property === 'string' && (property.startsWith(';') || property.includes('/*'));

/**
 * True if `node`'s subtree contains a canonical `Declaration` whose `property`
 * shows that garbage — the document-level precondition for the comment-array
 * consequence below.
 */
function subtree_has_garbage_declaration(node: unknown): boolean {
	if (node == null || typeof node !== 'object') return false;
	const n = node as Record<string, unknown>;
	if (n.type === 'Declaration' && is_garbage_property(n.property)) return true;
	for (const v of Object.values(n)) {
		if (Array.isArray(v)) {
			if (v.some(subtree_has_garbage_declaration)) return true;
		} else if (v && typeof v === 'object' && subtree_has_garbage_declaration(v)) {
			return true;
		}
	}
	return false;
}

interface DocumentedMatcher {
	name: string;
	/** docs/conformance_svelte.md section the divergence is cataloged under */
	conformance_section: string;
	matches: (
		entry: Omit<DiffEntry, 'documented' | 'signature'>,
		canonical_parent: unknown,
		ctx: MatchContext
	) => boolean;
}

const DOCUMENTED_MATCHERS: DocumentedMatcher[] = [
	{
		// Acorn-typescript's backtrack-reparse duplicates a comment inside any
		// re-parsed construct, emitting it twice — into a node's
		// leading/trailingComments AND into the root `comments` array (which
		// shifts every later index). tsv emits each comment once, so the canonical
		// side always has MORE entries. Two precise signatures (NOT "any path with
		// a comment field" — that masked genuine attachment divergences):
		// (1) a comment array that is strictly longer on the canonical side, and
		// (2) a root `comments[i]` field drift, gated on canonical actually
		// carrying a duplicated comment span (so a real per-comment value/offset
		// bug, with no duplicate, still surfaces as undocumented).
		name: 'comment_dedup',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry, canonical_parent, ctx) => {
			if (
				entry.kind === 'length_mismatch' &&
				/(^|\.)(comments|leadingComments|trailingComments)$/.test(entry.path) &&
				Number(entry.canonical) > Number(entry.ours)
			) {
				return true;
			}
			if (entry.kind === 'value_mismatch' && /^comments\[\d+\]\./.test(entry.path)) {
				const root = ctx.canonical_root as { comments?: { start?: number }[] } | null;
				const starts = root?.comments?.map((c) => c?.start);
				return Array.isArray(starts) && new Set(starts).size !== starts.length;
			}
			// (3) An ATTACHED copy whose `value` disagrees with its own twin. The throwaway
			// parse is entered over a DIFFERENT synthetic source than the surviving one, and
			// the comment `value` acorn writes is dedented by the indentation of the comment's
			// line IN that source — so the two copies of one multi-line block comment can carry
			// two different values, and index [0] is the discarded parse's. Gated hard on the
			// twin actually existing and on OUR value being one of the twins', so a genuine
			// dedent bug — a value neither copy holds — stays undocumented.
			const attached = /^(.*(?:leadingComments|trailingComments))\[\d+\]\.value$/.exec(entry.path);
			if (entry.kind === 'value_mismatch' && attached) {
				const arr = get_at_path(ctx.canonical_root, attached[1]) as
					{ start?: number; end?: number; value?: unknown }[] | null;
				const copy = canonical_parent as { start?: number; end?: number } | null;
				if (!Array.isArray(arr) || copy == null || typeof copy.start !== 'number') {
					return false;
				}
				const twins = arr.filter((c) => c?.start === copy.start && c?.end === copy.end);
				return twins.length > 1 && twins.some((c) => c?.value === entry.ours);
			}
			return false;
		}
	},
	{
		// Svelte parses `<script module>` and the instance `<script>` against one
		// shared root.comments queue (acorn.js get_comment_handlers/add_comments),
		// so a module-region comment — a module-script comment, or a leading
		// fragment HTML comment (`<!-- @component -->`) — is also attached to the
		// instance script: ahead of its first statement's own leading comments, or,
		// when the script holds no statement, ahead of the Program's own trailing
		// comments (`add_comments`'s root special case). tsv attaches each comment
		// once, in its source region. The comment is never lost (it stays on its
		// module/fragment home), so this is a pure cross-script duplication.
		// Two shapes: (1) tsv has no comment array at that slot, so the shifted
		// comment is a plain `missing_ours` on the array; (2) tsv DOES have its own
		// comments there, and the copies form a PREFIX of the canonical array — the
		// module-region comments in source order, then the instance's own — so
		// every index reads one whole prefix later: canonical `[i]` is tsv's
		// `[i − shift]`, a `value_mismatch` on each of the comment's fields at EVERY
		// index, not only `[0]`. The arm counts that prefix (the canonical comments
		// whose span lies BEFORE the instance script) and admits an entry only when
		// OUR value is the same field of canonical's comment at `[i + shift]` — so a
		// genuine instance-internal attachment difference (a comment tsv lists at
		// `[i]` that canonical never lists after the prefix) stays undocumented.
		name: 'svelte_instance_comment_duplication',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry, _canonical_parent, ctx) => {
			if (
				entry.kind === 'missing_ours' &&
				/^instance\.content(\.body\[0\])?\.(leadingComments|trailingComments)$/.test(entry.path)
			) {
				return true;
			}
			const shifted =
				/^(instance\.content(?:\.body\[0\])?\.(?:leadingComments|trailingComments))\[(\d+)\]\.(type|value|start|end)$/.exec(
					entry.path
				);
			if (entry.kind === 'value_mismatch' && shifted) {
				const [, array_path, index, field] = shifted;
				const arr = get_at_path(ctx.canonical_root, array_path) as Record<string, unknown>[] | null;
				const instance = get_at_path(ctx.canonical_root, 'instance.content') as {
					start?: number;
				} | null;
				if (!Array.isArray(arr) || instance == null || typeof instance.start !== 'number') {
					return false;
				}
				const instance_start = instance.start;
				const in_module_region = (comment: Record<string, unknown> | undefined): boolean =>
					typeof comment?.start === 'number' && comment.start < instance_start;
				let shift = 0;
				while (in_module_region(arr[shift])) shift++;
				if (shift === 0) return false;
				const twin = arr[Number(index) + shift];
				return twin != null && twin[field] === entry.ours;
			}
			return false;
		}
	},
	{
		// Svelte's parse_expression_at sets acorn `preserveParens: true`; a leading
		// comment before a parenthesized subexpression attaches to the synthetic
		// ParenthesizedExpression, which Svelte's remove_parens then strips —
		// dropping the attachment (the comment survives only in root `comments`).
		// tsv has no ParenthesizedExpression node, so it keeps the comment on the
		// inner expression — a template-expression attachment Svelte lacks
		// (`missing_canonical` under a `fragment.` path). Template-only; a plain
		// `<script>` parse does not set preserveParens.
		name: 'svelte_template_paren_comment',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry) =>
			entry.kind === 'missing_canonical' &&
			/^fragment\./.test(entry.path) &&
			/(^|\.)(leadingComments|trailingComments)$/.test(entry.path)
	},
	{
		// acorn-typescript ends a typed RestElement at the binding, excluding the
		// type annotation (`(...args: Array<any>)` → end after `args`) — inconsistent
		// with its own Identifier params, and with babel/TS-ESLint, which include
		// the annotation like tsv does.
		name: 'rest_param_type_end',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — Rest param type-annotation end',
		matches: (entry, _canonical_parent, ctx) => {
			const m = entry.path.match(/^(.*)\.(?:end|loc\.end\.(?:line|column))$/);
			if (!m) return false;
			const owner = get_at_path(ctx.canonical_root, m[1]) as {
				type?: unknown;
				typeAnnotation?: unknown;
			} | null;
			return owner?.type === 'RestElement' && owner.typeAnnotation != null;
		}
	},
	{
		// `static` newline `static` in a class body: tsc reads modifier + member (a
		// static field named `static`); acorn ASI-splits every bare `static` into its
		// own value-less field. tsv follows tsc. Scoped to class bodies whose
		// canonical AST contains a value-less, non-computed property literally named
		// `static` — the ladder pattern.
		name: 'static_member_ladder',
		conformance_section: 'TypeScript Parser Corrections (corpus-enforced) — static member ladder',
		matches: (entry, _canonical_parent, ctx) => {
			const segments = entry.path.split('.');
			for (let i = segments.length - 1; i > 0; i--) {
				const node = get_at_path(ctx.canonical_root, segments.slice(0, i).join('.')) as {
					type?: unknown;
					body?: unknown;
				} | null;
				if (node?.type !== 'ClassBody' || !Array.isArray(node.body)) continue;
				return node.body.some(
					(member: {
						type?: unknown;
						computed?: unknown;
						value?: unknown;
						key?: { name?: unknown };
					}) =>
						member?.type === 'PropertyDefinition' &&
						member.computed === false &&
						member.value === null &&
						member.key?.name === 'static'
				);
			}
			return false;
		}
	},
	{
		// acorn-typescript leaves a class heritage with type args as a
		// `TSInstantiationExpression` superClass when a line break precedes the next
		// clause (`extends Base<T>` newline `implements I` — its instantiation bail
		// checks hasPrecedingLineBreak); the same-line form yields
		// `superClass: Identifier` + `superTypeParameters`. tsv emits the same-line
		// shape uniformly.
		name: 'extends_instantiation_linebreak',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — extends instantiation line-break shape',
		matches: (entry, _canonical_parent, ctx) => {
			const m = entry.path.match(/^(.*)\.(?:superClass(?:\..*)?|superTypeParameters)$/);
			if (!m) return false;
			const cls = get_at_path(ctx.canonical_root, m[1]) as {
				superClass?: { type?: unknown } | null;
			} | null;
			return cls?.superClass?.type === 'TSInstantiationExpression';
		}
	},
	{
		// A lone UTF-16 surrogate in a string value (`"\ud800"`) is unrepresentable
		// in tsv's Rust strings — the decoded value carries U+FFFD where acorn keeps
		// the WTF-16 lone surrogate. Matches when replacing canonical's lone
		// surrogates with U+FFFD yields ours.
		name: 'lone_surrogate_value',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — Lone surrogates in string values',
		matches: (entry) => {
			if (entry.kind !== 'value_mismatch') return false;
			if (typeof entry.ours !== 'string' || typeof entry.canonical !== 'string') return false;
			const replaced = entry.canonical.replace(
				/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/gu,
				'\u{FFFD}'
			);
			return replaced !== entry.canonical && replaced === entry.ours;
		}
	},
	{
		// acorn-typescript starts the call/member nodes built on a parenthesized
		// decorator expression after the opening paren (`@(a)() p` → start at the
		// inner `a`) — inconsistent with its own non-decorator parse of `(a)()`
		// and with babel, which start at the `(` like tsv does.
		name: 'decorator_paren_subscript_start',
		conformance_section:
			'TypeScript Parser Corrections (corpus-enforced) — Parenthesized decorator subscript start',
		matches: (entry, _canonical_parent, ctx) => {
			const m = entry.path.match(
				/^(.*\.decorators\[\d+\]\.expression)(?:\.(?:callee|object|expression))*\.(?:start|loc\.start\.(?:line|column))$/
			);
			if (!m) return false;
			const expr = get_at_path(ctx.canonical_root, m[1]) as { type?: unknown } | null;
			return expr?.type === 'CallExpression' || expr?.type === 'MemberExpression';
		}
	},
	{
		// Svelte's read_declaration tokenizes garbage when a stray `;` or an adjacent
		// comment touches the property name: `border-box;;` yields a declaration with
		// property ";" that swallows the next declaration into its value, and
		// `color/* c */:` yields property "color/*" with the comment tail leaking into
		// the value (read_until stops at the first whitespace, which sits INSIDE the
		// comment). tsv parses per spec (skips empty declarations, tokenizes comments).
		//
		// Two shapes, because the same garbage lands in two places. (1) The
		// declaration's own `property`/`value`, gated on the canonical PARENT being
		// the garbage declaration. (2) The stylesheet's flat `comments` array: a
		// comment Svelte swallowed into a property token is never captured, so tsv
		// emits it plus every later comment at a shifted index, with `value` and
		// `position` following the shift. One insertion renumbers the whole tail, so
		// that arm cannot be scoped per index — it is scoped to the root `comments`
		// array of a document that actually contains the garbage. It is scoped by
		// DIRECTION too (the `comment_dedup` discipline, mirrored): the divergence
		// is an INSERTION, so the array must be longer on OUR side. A comment tsv
		// *loses* — the failure that matters — is the opposite direction, and its
		// `length_mismatch` still surfaces undocumented, as does a `missing_ours`
		// on the array ITSELF (tsv emitting no comments array at all) and any
		// comment-array divergence in a file with no garbage declaration.
		//
		// ⚠️ The direction lives on the ARRAY, never on the entry kind. Inside an
		// element the shift can only reach the FIELDS — an element always pairs
		// with an element — and `position` is the one OPTIONAL field (Svelte sets
		// it on a `read_value` comment only), so a value comment paired with a
		// non-value one reports it missing on whichever side lacks it. That is why
		// all three of `value_mismatch` / `missing_canonical` / `missing_ours` land
		// on `comments[i].position`, and why the refusal below is keyed on the PATH
		// rather than the kind — keyed on the kind it claims one renumbering and
		// refuses another that is the same fact told from the other side.
		name: 'css_declaration_tokenization',
		conformance_section:
			'CSS Parser Corrections (corpus-enforced) — Declaration tokenization garbage',
		matches: (entry, canonical_parent, ctx) => {
			const parent = canonical_parent as { type?: unknown; property?: unknown } | null | undefined;
			if (parent?.type === 'Declaration' && is_garbage_property(parent.property)) return true;
			if (!/^(css\.)?comments(\[\d+\])?(\.|$)/.test(entry.path)) return false;
			if (entry.kind === 'length_mismatch' && Number(entry.ours) <= Number(entry.canonical)) {
				return false;
			}
			if (entry.kind === 'missing_ours' && !/\[\d+\]\.position$/.test(entry.path)) return false;
			return subtree_has_garbage_declaration(ctx.canonical_root);
		}
	},
	{
		// acorn-typescript reads `<T>(<parenthesized arrow>)` as a generic arrow —
		// its Babel-ported abort on parenthesized arrows never fires (acorn sets no
		// `extra.parenthesized`) — where TypeScript reads a type assertion over the
		// arrow. tsv follows TypeScript (`TSTypeAssertion` wrapping the arrow), so
		// every field of that expression diffs. Gate: some ancestor canonical node
		// is an arrow with `typeParameters` whose source continues `( (` after the
		// type-params `>` — a doubled paren cannot open a real generic arrow's
		// param list, so genuine generic-arrow divergences stay unmasked.
		name: 'type_assertion_paren_arrow',
		conformance_section: 'TypeScript Corrections — Type assertion vs. generic arrow',
		matches: (entry, _canonical_parent, ctx) => {
			const parts = entry.path.split('.');
			for (let i = parts.length - 1; i >= 1; i--) {
				const node = get_at_path(ctx.canonical_root, parts.slice(0, i).join('.')) as {
					type?: unknown;
					typeParameters?: { end?: unknown } | null;
				} | null;
				if (
					node?.type === 'ArrowFunctionExpression' &&
					typeof node.typeParameters?.end === 'number' &&
					/^\s*\(\s*\(/.test(ctx.source.slice(node.typeParameters.end))
				) {
					return true;
				}
			}
			return false;
		}
	},
	{
		// Svelte's parseCss reads `:nth-child(An+B of S)` as `Nth.value = "2n of "`
		// (the ` of ` leaks in from its REGEX_NTH_OF terminator) and flattens `S` as
		// sibling simple selectors of the Nth. Per Selectors 4 the `S` is a nested
		// <complex-selector-list> scoped to the nth term, so tsv keeps `Nth.value =
		// "2n"` with `S` nested under a tsv-only `Nth.selector` field. The whole args
		// subtree reshapes: the `Nth.value`/`.selector`, the sibling-count lengths, and
		// the container span ends all differ. Anchor on Svelte's unambiguous tell — a
		// canonical `Nth` whose value ends in the `of` word — and absorb only the
		// reshape-shaped fields, so a genuine content bug inside `S` (a wrong
		// `.name`/`.type`) still surfaces as undocumented.
		name: 'nth_of_structure',
		conformance_section: 'CSS Corrections — :nth-child(An+B of S)',
		matches: (entry, _canonical_parent, ctx) => {
			const args_match = entry.path.match(/^(.*\.args)\b/);
			if (!args_match) return false;
			if (!subtree_has_nth_of(get_at_path(ctx.canonical_root, args_match[1]))) return false;
			return (
				(entry.kind === 'value_mismatch' && /\.(value|start|end)$/.test(entry.path)) ||
				(entry.kind === 'missing_canonical' && /\.selector$/.test(entry.path)) ||
				(entry.kind === 'length_mismatch' && /\.(children|selectors)$/.test(entry.path))
			);
		}
	}
];

/** A path inside a `loc` or `name_loc` — what a fixture's span-only pins say nothing about. */
const LINE_COLUMN_PATH = /(^|\.)(name_)?loc(\.|$)/;

/**
 * The classification `--fixtures` gives a `_svelte_divergence` fixture's declared difference
 * — every span difference in a document whose parses still equal the committed pins (see
 * `MatchContext.declared_divergence` for what that reach hides).
 */
const FIXTURE_DECLARED_DIVERGENCE = 'fixture_declared_divergence';

// --- `--fixtures`: a fixture tree's parse-pinned documents ---------------------------

/** A fixture document's parse claim, as `--fixtures` reads it off the fixture directory. */
interface FixtureDocument {
	/** The fixture's `goal` marker (`script`), or `undefined` for the module default. */
	goal: ParseGoal | undefined;
	/** A `_svelte_divergence` input's committed pins: tsv's wire and the oracle's. */
	declared: { ours: unknown; svelte: unknown } | null;
}

/** Each fixture directory's documents, keyed by basename — read once per directory. */
const fixture_dirs = new Map<string, Map<string, FixtureDocument> | null>();

/** The fixture input filenames, in `find_input_file`'s precedence order. */
const FIXTURE_INPUTS = ['input.svelte', 'input.svelte.ts', 'input.ts', 'input.css'];

/**
 * The parse claim a fixture tree makes about the document at `path`, or `null` when it
 * makes none: a fixture's `input.*` (at its `goal` marker's goal) and every variant an
 * `expected_<stem>.json` pins are parse-pinned; every other file — the `unformatted_*` and
 * prettier-side variants, `input_invalid_*` — carries no parse claim, and a `tsv_rejects.txt`
 * fixture has no tsv wire. The parse-pinned subset of `tsv_debug loc_wires`' documents: that
 * leg also grades the format variants, which need no oracle, where this one grades only the
 * documents a committed oracle verdict exists for.
 */
function fixture_document(path: string): FixtureDocument | null {
	const dir = dirname(path);
	let docs = fixture_dirs.get(dir);
	if (docs === undefined) {
		docs = read_fixture_dir(dir);
		fixture_dirs.set(dir, docs);
	}
	return docs?.get(basename(path)) ?? null;
}

function read_fixture_dir(dir: string): Map<string, FixtureDocument> | null {
	const entries = new Set(readdirSync(dir));
	const input = FIXTURE_INPUTS.find((name) => entries.has(name));
	if (input === undefined || entries.has('tsv_rejects.txt')) return null;
	const goal: ParseGoal | undefined =
		entries.has('goal') && readFileSync(join(dir, 'goal'), 'utf8').trim() === 'script'
			? 'script'
			: undefined;
	const read_json = (name: string): unknown => JSON.parse(readFileSync(join(dir, name), 'utf8'));
	const declared =
		/_svelte(_prettier)?_divergence$/.test(basename(dir)) &&
		entries.has('expected_ours.json') &&
		entries.has('expected_svelte.json')
			? { ours: read_json('expected_ours.json'), svelte: read_json('expected_svelte.json') }
			: null;
	const docs = new Map<string, FixtureDocument>([[input, { goal, declared }]]);
	const ext = input.slice('input'.length);
	for (const name of entries) {
		const stem = /^expected_(.+)\.json$/.exec(name)?.[1];
		if (stem === undefined || stem === 'ours' || stem === 'svelte') continue;
		if (existsSync(join(dir, `${stem}${ext}`))) {
			docs.set(`${stem}${ext}`, { goal, declared: null });
		}
	}
	return docs;
}

function classify(
	entry: Omit<DiffEntry, 'documented' | 'signature'>,
	canonical_parent: unknown,
	ctx: MatchContext
): string | null {
	// A `loc` line or column is classified by the tolerance rows alone (see `diff_asts`) —
	// a whole-subtree matcher written for a span divergence must not excuse one.
	if (is_loc_leaf(entry.path)) return null;
	// A tsv `loc` the oracle lacks is graded by the superset rule alone (see `diff_asts`):
	// one that reaches here is outside `LOC_SUPERSET_KEYS`, which no matcher may excuse.
	if (entry.kind === 'missing_canonical' && /(^|\.)loc$/.test(entry.path)) return null;
	if (ctx.declared_divergence && !LINE_COLUMN_PATH.test(entry.path)) {
		return FIXTURE_DECLARED_DIVERGENCE;
	}
	for (const matcher of DOCUMENTED_MATCHERS) {
		if (matcher.matches(entry, canonical_parent, ctx)) return matcher.name;
	}
	return null;
}

// --- Diff engine ---------------------------------------------------------------

/** Normalize a concrete path to its grouping signature (array indices erased). */
function path_signature(path: string): string {
	return path.replace(/\[\d+\]/g, '[]');
}

/** Per-row counts of the `loc` differences a diff tolerated (`lib/loc_tolerance.ts`). */
export type LocRowCounts = Partial<Record<LocRow, number>>;

/** What one deep diff found. */
export interface DiffResult {
	diffs: DiffEntry[];
	truncated: boolean;
	/**
	 * The `loc` differences the six tolerance rows absorbed, per row — counted rather than
	 * stored, so a tolerated row neither fills the per-file cap nor hides a finding behind it.
	 */
	loc_rows: LocRowCounts;
	/**
	 * Objects carrying a tsv `loc` the oracle gives none, by `superset_key` — the accepted
	 * superset (Svelte and CSS, kinds in `LOC_SUPERSET_KEYS` only).
	 */
	loc_superset: Record<string, number>;
	/** `loc` halves left ungraded because the two sides' offsets at that half disagree. */
	loc_span_skipped: number;
}

/** Whether an object carries numeric `start` and `end`. */
const has_span = (o: Record<string, unknown>): boolean =>
	typeof o.start === 'number' && typeof o.end === 'number';

/**
 * Recursively diff two JSON-shaped values, collecting up to MAX_DIFFS_PER_FILE
 * entries. Arrays with differing lengths report one length_mismatch and still
 * recurse the shared prefix so positional drift inside is visible.
 *
 * **`loc` is graded by its own rules**, keyed on `ctx.language`:
 * - per half, only where both sides' offset at that half agrees — a span difference is
 *   graded (and classified) at the span itself, and a `loc.start` / `loc.end` that follows
 *   a differing `start` / `end` says nothing more, while the other half is still graded;
 * - a tsv `loc` on an object the oracle gives none is accepted for Svelte and CSS — the
 *   definition's superset (template, style and option objects, every CSS node) — when its
 *   kind is one `LOC_SUPERSET_KEYS` pins for the language, and is a difference otherwise,
 *   and always for TypeScript, where acorn gives every node one;
 * - a differing `loc` line or column is a finding unless one of the six tolerance rows
 *   claims it, which applies to Svelte only. A TypeScript `loc` is exact.
 */
export function diff_asts(ours: unknown, canonical: unknown, ctx: MatchContext): DiffResult {
	const diffs: DiffEntry[] = [];
	let truncated = false;
	const loc_rows: LocRowCounts = {};
	const loc_superset: Record<string, number> = {};
	let loc_span_skipped = 0;
	const superset_kinds = superset_kinds_of(ctx.language);
	const superset_text = superset_kinds === null ? '' : wire_text(ctx.source);

	const push = (
		kind: DiffKind,
		path: string,
		o: unknown,
		c: unknown,
		canonical_parent: unknown
	): void => {
		if (kind === 'value_mismatch' && is_loc_leaf(path)) {
			const row = classify_loc_difference(path, o, c, ctx);
			if (row !== null) {
				loc_rows[row] = (loc_rows[row] ?? 0) + 1;
				return;
			}
		}
		if (diffs.length >= MAX_DIFFS_PER_FILE) {
			truncated = true;
			return;
		}
		const base = { path, kind, ours: o, canonical: c };
		diffs.push({
			...base,
			signature: `${kind}:${path_signature(path)}`,
			documented: classify(base, canonical_parent, ctx)
		});
	};

	// `anchor` is the nearest enclosing object of a pinned superset type — the context a
	// superset object outside those types is keyed by (`superset_key`)
	const walk = (
		o: unknown,
		c: unknown,
		path: string,
		canonical_parent: unknown,
		anchor: SupersetAnchor | null
	): void => {
		if (truncated) return;
		if (o === c) return;
		const o_type = value_type(o);
		const c_type = value_type(c);
		if (o_type !== c_type) {
			push('type_mismatch', path, o, c, canonical_parent);
			return;
		}
		switch (o_type) {
			case 'array': {
				const o_arr = o as unknown[];
				const c_arr = c as unknown[];
				if (o_arr.length !== c_arr.length) {
					push('length_mismatch', path, o_arr.length, c_arr.length, canonical_parent);
				}
				const shared = Math.min(o_arr.length, c_arr.length);
				for (let i = 0; i < shared; i++) {
					walk(o_arr[i], c_arr[i], `${path}[${i}]`, c, anchor);
				}
				break;
			}
			case 'object': {
				const o_obj = o as Record<string, unknown>;
				const c_obj = c as Record<string, unknown>;
				const keys = new Set([...Object.keys(o_obj), ...Object.keys(c_obj)]);
				const child_anchor =
					superset_kinds !== null &&
					typeof o_obj.type === 'string' &&
					superset_kinds.has(o_obj.type)
						? { type: o_obj.type, path }
						: anchor;
				for (const key of keys) {
					const child_path = path === '' ? key : `${path}.${key}`;
					const in_ours = key in o_obj;
					const in_canonical = key in c_obj;
					if (key === 'loc') {
						if (in_ours && !in_canonical && superset_kinds !== null && has_span(o_obj)) {
							const kind = superset_key(o_obj, path, anchor, superset_kinds, superset_text);
							if (superset_kinds.has(kind)) {
								loc_superset[kind] = (loc_superset[kind] ?? 0) + 1;
								continue;
							}
						}
						if (
							in_ours &&
							in_canonical &&
							has_span(o_obj) &&
							has_span(c_obj) &&
							(o_obj.start !== c_obj.start || o_obj.end !== c_obj.end)
						) {
							// grade the half whose offset agrees; the other follows a span
							// difference already graded at the span
							const o_loc = o_obj.loc as Record<string, unknown> | null;
							const c_loc = c_obj.loc as Record<string, unknown> | null;
							for (const side of ['start', 'end'] as const) {
								if (o_obj[side] === c_obj[side]) {
									walk(o_loc?.[side], c_loc?.[side], `${child_path}.${side}`, c_loc, null);
								} else {
									loc_span_skipped++;
								}
							}
							continue;
						}
					}
					if (!in_ours) {
						push('missing_ours', child_path, undefined, c_obj[key], c);
					} else if (!in_canonical) {
						push('missing_canonical', child_path, o_obj[key], undefined, c);
					} else {
						walk(o_obj[key], c_obj[key], child_path, c, child_anchor);
					}
				}
				break;
			}
			default:
				push('value_mismatch', path, o, c, canonical_parent);
		}
	};

	walk(ours, canonical, '', null, null);
	return { diffs, truncated, loc_rows, loc_superset, loc_span_skipped };
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

Deep-diffs tsv's shipped parse output (FFI wire) against the canonical parsers
(acorn-typescript / svelte / parseCss). Raw diff first, documented-divergence
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

/** Flatten a {@link LanguageStats} to the count shape used in JSON. */
function stats_to_counts(s: LanguageStats) {
	return {
		total: s.total,
		compared: s.compared,
		multibyte: s.multibyte,
		match: s.match,
		documented: s.documented,
		undocumented: s.undocumented,
		tsv_errors: s.tsv_errors,
		canonical_errors: s.canonical_errors,
		both_errors: s.both_errors
	};
}

/** Build the `stats` block: per-language counts plus a summed total. */
function build_stats_block(stats: Map<Language, LanguageStats>) {
	const languages: Record<string, ReturnType<typeof stats_to_counts>> = {};
	const totals = empty_stats();
	for (const lang of LANGUAGES) {
		const s = stats.get(lang)!;
		if (s.total === 0) continue;
		languages[lang] = stats_to_counts(s);
		totals.total += s.total;
		totals.compared += s.compared;
		totals.multibyte += s.multibyte;
		totals.match += s.match;
		totals.documented += s.documented;
		totals.undocumented += s.undocumented;
		totals.tsv_errors += s.tsv_errors;
		totals.canonical_errors += s.canonical_errors;
		totals.both_errors += s.both_errors;
	}
	return { languages, total: stats_to_counts(totals) };
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
	stats: Map<Language, SpanStats>;
	groups: DiffGroup[];
	verdict_mismatches: SpanVerdictMismatch[];
}

/** The `span_only` block: per-language counts plus a summed total, its groups and verdict mismatches. */
function build_span_json(span: SpanReport, base_path: string) {
	const languages: Record<string, SpanStats> = {};
	const total = empty_span_stats();
	for (const [lang, s] of span.stats) {
		languages[lang] = { ...s };
		total.compared += s.compared;
		total.match += s.match;
		total.documented += s.documented;
		total.undocumented += s.undocumented;
		total.verdict_mismatch += s.verdict_mismatch;
	}
	return {
		stats: { languages, total },
		groups: span.groups.map((g) => group_to_json(g, base_path)),
		verdict_mismatches: span.verdict_mismatches.map((m) => ({
			...m,
			path: rel_path(m.path, base_path)
		}))
	};
}

/** Build the single buffered JSON report. */
function build_json_report(
	results: Map<Language, FileResult[]>,
	stats: Map<Language, LanguageStats>,
	groups: DiffGroup[],
	span: SpanReport,
	loc: LocBooks,
	base_path: string
): Record<string, unknown> {
	return {
		stats: build_stats_block(stats),
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
			results
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
			results
				.get(lang)!
				.filter((r) => r.truncated)
				.map((r) => rel_path(r.path, base_path))
		)
	};
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

	const results: Map<Language, FileResult[]> = new Map();
	const stats: Map<Language, LanguageStats> = new Map();
	for (const lang of LANGUAGES) {
		results.set(lang, []);
		stats.set(lang, empty_stats());
	}

	// The span-only arm's own books, kept apart from the loc arm's so neither table blends
	// the two wires.
	const span_results: Map<Language, FileResult[]> = new Map(LANGUAGES.map((lang) => [lang, []]));
	const span_stats_by_lang: Map<Language, SpanStats> = new Map(
		LANGUAGES.map((lang) => [lang, empty_span_stats()])
	);
	const span_verdict_mismatches: SpanVerdictMismatch[] = [];
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

		// In inject mode a base file is a CONTROL: it is parsed so its own divergences can
		// be subtracted from its variants, and counted in neither the results nor the
		// stats. Letting it into the table would blend two populations behind one
		// percentage — the reading this tool's per-source disclosures exist to prevent.
		const is_control = injecting && !file.path.includes(VARIANT_MARKER);
		if (is_control) controls++;
		else variants++;

		const lang_stats = stats.get(lang)!;
		const lang_results = results.get(lang)!;
		if (!is_control) lang_stats.total++;

		if (verbose) console.log(`  ${file.path}`);

		// Parse both sides; a throw on either side is an error bucket, not a diff.
		let ours: unknown;
		let tsv_error: string | null = null;
		try {
			ours = native.parse(file.content, lang, goal);
		} catch (e) {
			tsv_error = String(e instanceof Error ? e.message : e).split('\n')[0];
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
			canonical_error = String(e instanceof Error ? e.message : e).split('\n')[0];
		}

		// The span-only arm (see `LOCATION_KEYS`). Its controls are recorded but not counted,
		// like the loc arm's, so `subtract_baseline_diffs` can grade a variant against them.
		const span_stats = span_stats_by_lang.get(lang)!;
		let declared_divergence = false;
		let ours_span: unknown;
		let span_error: string | null = null;
		try {
			ours_span = native.parse_no_locations(file.content, lang, goal);
		} catch (e) {
			span_error = String(e instanceof Error ? e.message : e).split('\n')[0];
			if (is_native_panic_error(span_error)) {
				span_panics.push({ path: file.path, error: span_error });
			}
		}
		if ((span_error === null) !== (tsv_error === null)) {
			span_stats.verdict_mismatch++;
			span_verdict_mismatches.push({
				path: file.path,
				language: lang,
				loc_wire: tsv_error ?? 'parsed',
				span_wire: span_error ?? 'parsed'
			});
		} else if (span_error === null && canonical_error === null) {
			if (!is_control) span_stats.compared++;
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
			if (span.diffs.length === 0) {
				if (!is_control) span_stats.match++;
			} else {
				const all_documented = span.diffs.every((d) => d.documented !== null);
				if (!is_control) {
					if (all_documented) span_stats.documented++;
					else span_stats.undocumented++;
				}
				span_results.get(lang)!.push({
					path: file.path,
					bytes: file.bytes,
					multibyte,
					status: all_documented ? 'documented' : 'undocumented',
					diffs: span.diffs,
					truncated: span.truncated
				});
			}
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
			if (!is_control) {
				lang_stats[`${status}s` as 'both_errors' | 'tsv_errors' | 'canonical_errors']++;
			}
			lang_results.push({
				path: file.path,
				bytes: file.bytes,
				multibyte,
				status,
				diffs: [],
				truncated: false,
				error: tsv_error ?? canonical_error ?? undefined
			});
			continue;
		}

		if (!is_control) {
			lang_stats.compared++;
			if (multibyte) lang_stats.multibyte++;
		}

		const { diffs, truncated, loc_rows, loc_superset, loc_span_skipped } = diff_asts(
			ours,
			canonical_ast,
			{ source: file.content, canonical_root: canonical_ast, language: lang, declared_divergence }
		);
		if (!is_control)
			record_loc_tolerances(loc_books, lang, loc_rows, loc_superset, loc_span_skipped);
		if (diffs.length === 0) {
			// exact matches are counted, not stored — and so is a file whose only differences
			// are tolerated `loc` rows, which is documented rather than exact
			if (!is_control) {
				if (Object.keys(loc_rows).length > 0) lang_stats.documented++;
				else lang_stats.match++;
			}
			continue;
		}

		const all_documented = diffs.every((d) => d.documented !== null);
		if (!is_control) {
			if (all_documented) {
				lang_stats.documented++;
			} else {
				lang_stats.undocumented++;
			}
		}
		lang_results.push({
			path: file.path,
			bytes: file.bytes,
			multibyte,
			status: all_documented ? 'documented' : 'undocumented',
			diffs,
			truncated
		});

		if (verbose) {
			for (const d of diffs) {
				const tag = d.documented ? `documented:${d.documented}` : 'UNDOCUMENTED';
				console.log(`    ${d.kind} at ${d.path} (${tag})`);
				console.log(`      ours: ${preview(d.ours)}  canonical: ${preview(d.canonical)}`);
			}
		}
	}

	if (injecting) {
		// Re-grade every manufactured input against its own base: a divergence — or a parse
		// failure — the base file already had is not the injection's doing. See
		// `subtract_baseline_diffs`.
		for (const lang of LANGUAGES) {
			const kept = subtract_baseline_diffs(results.get(lang)!);
			results.set(lang, kept);
			const s = stats.get(lang)!;
			const count = (status: string) => kept.filter((r) => r.status === status).length;
			s.undocumented = count('undocumented');
			s.documented = count('documented');
			// A variant whose every diff its base already had is no longer a finding, so it
			// is an exact match for this run's question. `match` has to be re-derived rather
			// than left as the raw count, or the table reports those variants as neither
			// matched nor diverging and the exact-% reads far below the truth.
			s.match = s.compared - s.documented - s.undocumented;
			// The tsv-side rejections likewise, which the raw count gets backwards on this
			// corpus: `tests/fixtures` holds ~593 `input_invalid_*` files plus the
			// `tsv_rejects` set, and each fails in every variant derived from it while its
			// own base is excluded as a control — so the raw number reports inherited
			// failures as injected ones. What survives here is an injection that turned an
			// accepted document into a rejected one. The oracle-side rejections are
			// re-derived for the mirror-image reason — raw, they re-report each
			// `_svelte_divergence` fixture's own sanctioned over-acceptance once per variant
			// derived from it — and what survives there is an injection that turned a
			// document the ORACLE accepted into one only tsv does. `both_error` stays raw: no
			// side accepted it, so it is a finding about neither, and "parse-fail skipped" is
			// the right home for it.
			s.tsv_errors = count('tsv_error');
			s.canonical_errors = count('canonical_error');

			// The span arm, the same way. Its rows hold only diverging files (its parse
			// failures are the loc arm's to count), so the pass reduces to the signature
			// subtraction, and `match` is re-derived for the same reason as above.
			const span_kept = subtract_baseline_diffs(span_results.get(lang)!);
			span_results.set(lang, span_kept);
			const ss = span_stats_by_lang.get(lang)!;
			ss.undocumented = span_kept.filter((r) => r.status === 'undocumented').length;
			ss.documented = span_kept.filter((r) => r.status === 'documented').length;
			ss.match = ss.compared - ss.documented - ss.undocumented;
		}
	}

	const total_processed = Object.values(lang_counts).reduce((a, b) => a + b, 0);
	if (total_processed === 0) {
		// An empty scope is a failed comparison run, not a pass — an existing-but-
		// source-empty path (typo, moved src/) must not read as green.
		console.log('No files found — nothing was compared.');
		if (json_mode) {
			emit_json_stdout(
				build_json_report(
					results,
					stats,
					[],
					{ stats: span_stats_by_lang, groups: [], verdict_mismatches: [] },
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
	let total_undocumented = 0;
	const totals = empty_stats();
	for (const lang of LANGUAGES) {
		const s = stats.get(lang)!;
		if (s.total === 0) continue;
		totals.compared += s.compared;
		totals.multibyte += s.multibyte;
		totals.match += s.match;
		totals.documented += s.documented;
		totals.undocumented += s.undocumented;
		totals.tsv_errors += s.tsv_errors;
		totals.canonical_errors += s.canonical_errors;
		totals.both_errors += s.both_errors;
		total_undocumented += s.undocumented;

		const pct = s.compared > 0 ? ((s.match / s.compared) * 100).toFixed(1) : '100.0';
		const match_str = `${s.match}/${s.compared} exact (${pct}%)`.padEnd(26);
		const parts: string[] = [];
		if (s.multibyte > 0) parts.push(`${s.multibyte} multibyte`);
		if (s.documented > 0) parts.push(`${s.documented} documented`);
		if (s.undocumented > 0) parts.push(`\x1b[31m${s.undocumented} UNDOCUMENTED\x1b[0m`);
		const skipped = s.tsv_errors + s.canonical_errors + s.both_errors;
		if (skipped > 0) parts.push(`\x1b[2m${skipped} parse-fail skipped\x1b[0m`);
		console.log(`  ${lang.padEnd(12)} ${match_str} | ${parts.join(' | ') || 'all exact'}`);
	}
	if (totals.compared > 0) {
		console.log('  ' + '─'.repeat(72));
		const pct = ((totals.match / totals.compared) * 100).toFixed(1);
		const match_str = `${totals.match}/${totals.compared} exact (${pct}%)`.padEnd(26);
		const parts: string[] = [];
		if (totals.multibyte > 0) parts.push(`${totals.multibyte} multibyte`);
		if (totals.documented > 0) parts.push(`${totals.documented} documented`);
		if (totals.undocumented > 0) {
			parts.push(`\x1b[31m${totals.undocumented} UNDOCUMENTED\x1b[0m`);
		}
		const skipped = totals.tsv_errors + totals.canonical_errors + totals.both_errors;
		if (skipped > 0) parts.push(`\x1b[2m${skipped} parse-fail skipped\x1b[0m`);
		console.log(`  ${'total'.padEnd(12)} ${match_str} | ${parts.join(' | ') || 'all exact'}`);
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

	// The span-only arm's table: the same deep diff over the `no-locations` wire, against
	// the oracle with its line/column objects stripped (see `LOCATION_KEYS`).
	const span_totals = empty_span_stats();
	for (const s of span_stats_by_lang.values()) {
		span_totals.compared += s.compared;
		span_totals.match += s.match;
		span_totals.documented += s.documented;
		span_totals.undocumented += s.undocumented;
		span_totals.verdict_mismatch += s.verdict_mismatch;
	}
	if (span_totals.compared + span_totals.verdict_mismatch > 0) {
		console.log('\nSpan-only arm (no-locations wire vs canonical with `loc`/`name_loc` stripped):');
		const rows: [string, SpanStats][] = [
			...[...span_stats_by_lang].filter(([, s]) => s.compared + s.verdict_mismatch > 0),
			['total', span_totals]
		];
		for (const [label, s] of rows) {
			if (label === 'total') console.log('  ' + '─'.repeat(72));
			const pct = s.compared > 0 ? ((s.match / s.compared) * 100).toFixed(1) : '100.0';
			const match_str = `${s.match}/${s.compared} exact (${pct}%)`.padEnd(26);
			const parts: string[] = [];
			if (s.documented > 0) parts.push(`${s.documented} documented`);
			if (s.undocumented > 0) parts.push(`\x1b[31m${s.undocumented} UNDOCUMENTED\x1b[0m`);
			if (s.verdict_mismatch > 0) {
				parts.push(`\x1b[31m${s.verdict_mismatch} VERDICT MISMATCH (loc vs span wire)\x1b[0m`);
			}
			console.log(`  ${label.padEnd(12)} ${match_str} | ${parts.join(' | ') || 'all exact'}`);
		}
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

	const groups = build_groups(results);
	const undocumented_groups = groups.filter((g) => g.documented === null);
	const documented_groups = groups.filter((g) => g.documented !== null);
	const span_groups = build_groups(span_results);
	const span_undocumented_groups = span_groups.filter((g) => g.documented === null);

	if (json_mode) {
		emit_json_stdout(
			build_json_report(
				results,
				stats,
				groups,
				{
					stats: span_stats_by_lang,
					groups: span_groups,
					verdict_mismatches: span_verdict_mismatches
				},
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
				results.get(lang)!.filter((r) => r.status !== 'canonical_error')
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
		const span = span_stats_by_lang.get(lang)!;
		const compared = stats.get(lang)!.compared;
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
				(lang) => stats.get(lang)!.compared !== CORPUS_PARSE_COMPARED_PIN[lang]
			).map(
				(lang) =>
					`${lang} compared ${stats.get(lang)!.compared} ≠ pinned ${CORPUS_PARSE_COMPARED_PIN[lang]}`
			),
			...LANGUAGES.filter(
				(lang) => stats.get(lang)!.tsv_errors !== CORPUS_PARSE_TSV_ERRORS_PIN[lang]
			).map(
				(lang) =>
					`${lang} tsv-parse-failures ${stats.get(lang)!.tsv_errors} ≠ pinned ${CORPUS_PARSE_TSV_ERRORS_PIN[lang]}`
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
	if (span_verdict_mismatches.length > 0) {
		console.log(
			`\n\x1b[31mVERDICT MISMATCH between the loc and span-only wires (${span_verdict_mismatches.length}):\x1b[0m`
		);
		for (const m of span_verdict_mismatches.slice(0, 10)) {
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
		results.get(lang)!.filter((r) => r.truncated)
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
		...(total_undocumented > 0
			? [`${total_undocumented} file(s) with undocumented AST diffs vs canonical`]
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
	const empty: Map<Language, LanguageStats> = new Map(
		LANGUAGES.map((lang) => [lang, empty_stats()])
	);
	const empty_loc = empty_loc_books();
	return {
		stats: build_stats_block(empty),
		groups: [],
		span_only: build_span_json(
			{
				stats: new Map(LANGUAGES.map((lang) => [lang, empty_span_stats()])),
				groups: [],
				verdict_mismatches: []
			},
			''
		),
		loc_arm: {
			checked: empty_loc.checked,
			violations: [],
			rows: empty_loc.rows,
			superset: empty_loc.superset,
			superset_kinds: empty_loc.superset_kinds,
			span_skipped: empty_loc.span_skipped
		},
		errors: [],
		truncated_files: [],
		error: message
	};
}

// Guarded so this module can be imported for its diff engine (`diff_asts`,
// `DiffEntry`, `MatchContext`) — e.g. by diagnostics/svelte_fixtures_compare.ts —
// without running the CLI on import.
if (import.meta.main) {
	run_compare_main(run_corpus_compare_parse, CorpusCompareParseArgs, build_error_json_report);
}
