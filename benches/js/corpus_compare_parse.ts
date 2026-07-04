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
 *
 * Multibyte files are the high-value slice: byte→UTF-16 offset translation vs
 * the canonical parsers' native offsets is the riskiest machinery. Use
 * --multibyte-only for fast iteration on it.
 *
 * Usage:
 *   deno task corpus:compare:parse ~/dev/some-project
 *   deno task corpus:compare:parse --all
 *   deno task corpus:compare:parse --all --multibyte-only
 *   deno task corpus:compare:parse --all --filter typescript --limit 100
 *   deno task corpus:compare:parse --all --json 2>/dev/null > report.json
 *
 * Parse FAILURES (one side throws) are counted but not the focus —
 * diagnostics/skip_triage.ts is the dedicated tool for parse-gap triage.
 */

import process from 'node:process';

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import { z } from 'zod';

import {
	COMPARE_BASE_ARG_FIELDS,
	create_compare_loader,
	emit_json_stdout,
	init_compare_implementations,
	parse_language_filter,
	redirect_logs_to_stderr,
	rel_path,
	resolve_compare_base_path,
	run_compare_main,
} from './lib/compare_cli.ts';
import { type Language, LANGUAGES } from './lib/types.ts';

const CorpusCompareParseArgs = z.object({
	...COMPARE_BASE_ARG_FIELDS,
	'multibyte-only': z.boolean().default(false).meta({ aliases: ['m'] }),
});

/** Per-file diff cap — collection stops here and the file is flagged truncated. */
const MAX_DIFFS_PER_FILE = 50;

/** Max sample entries shown per diff group in the report. */
const MAX_GROUP_SAMPLES = 3;

type DiffKind =
	| 'value_mismatch'
	| 'type_mismatch'
	| 'missing_ours'
	| 'missing_canonical'
	| 'length_mismatch';

interface DiffEntry {
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
		both_errors: 0,
	};
}

/** Whether the source contains any non-ASCII character (the multibyte slice). */
function has_non_ascii(s: string): boolean {
	for (let i = 0; i < s.length; i++) {
		if (s.charCodeAt(i) > 0x7f) return true;
	}
	return false;
}

// Matches the fixture sidecar's jsonReplacer (crates/tsv_debug/src/deno/sidecar.ts)
// so corpus comparison and expected.json generation serialize the canonical AST
// identically (BigInt literal values can't be serialized natively).
function bigint_replacer(_key: string, value: unknown): unknown {
	return typeof value === 'bigint' ? value.toString() : value;
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
interface MatchContext {
	source: string;
	/** Root of the canonical AST — lets matchers resolve ancestors from the entry path. */
	canonical_root: unknown;
}

/** Resolve a node by concrete diff path (`fragment.nodes[3].expression`). */
function get_at_path(root: unknown, path: string): unknown {
	let node = root;
	for (const seg of path.split('.')) {
		if (node == null) return null;
		const m = seg.match(/^([^[]+)((?:\[\d+\])*)$/);
		if (!m) return null;
		node = (node as Record<string, unknown>)[m[1]];
		for (const idx of m[2].matchAll(/\[(\d+)\]/g)) {
			if (!Array.isArray(node)) return null;
			node = node[Number(idx[1])];
		}
	}
	return node;
}

/**
 * True if `node`'s subtree contains an `Nth` whose value carries Svelte's ` of `
 * (a normal Nth value never does) — the tell of a `:nth-child(An+B of S)` argument.
 */
function subtree_has_nth_of(node: unknown): boolean {
	if (node == null || typeof node !== 'object') return false;
	const n = node as Record<string, unknown>;
	if (n.type === 'Nth' && typeof n.value === 'string' && n.value.includes(' of ')) return true;
	for (const v of Object.values(n)) {
		if (Array.isArray(v)) {
			if (v.some(subtree_has_nth_of)) return true;
		} else if (v && typeof v === 'object' && subtree_has_nth_of(v)) {
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
		ctx: MatchContext,
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
		matches: (entry, _canonical_parent, ctx) => {
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
			return false;
		},
	},
	{
		// Svelte parses `<script module>` and the instance `<script>` against one
		// shared root.comments queue (acorn.js get_comment_handlers/add_comments),
		// so a module-region comment — a module-script comment, or a leading
		// fragment HTML comment (`<!-- @component -->`) — is also attached to the
		// instance script's Program or its first statement. tsv attaches each
		// comment once, in its source region. The comment is never lost (it stays
		// on its module/fragment home), so this is a pure cross-script duplication.
		name: 'svelte_instance_comment_duplication',
		conformance_section: 'Comment Attachment Differences',
		matches: (entry) =>
			entry.kind === 'missing_ours' &&
			/^instance\.content(\.body\[0\])?\.(leadingComments|trailingComments)$/.test(entry.path),
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
			/(^|\.)(leadingComments|trailingComments)$/.test(entry.path),
	},
	{
		// acorn-typescript drops ALL params from async arrows with type params
		// (`async <T,>(x: T) => x` → params: []) — documented semantic
		// corruption tsv corrects.
		name: 'async_generic_arrow_params',
		conformance_section: 'TypeScript Corrections — Async generic arrow params',
		matches: (entry, canonical_parent) => {
			if (!/(^|\.)params$/.test(entry.path)) return false;
			const parent = canonical_parent as
				| { async?: unknown; typeParameters?: unknown }
				| null
				| undefined;
			return parent?.async === true && parent?.typeParameters != null;
		},
	},
	{
		// Svelte's parseCss/parse call remove_bom before parsing, so every canonical
		// offset in a BOM-prefixed file is 1 (UTF-16 unit) lower than the real file
		// position; tsv deliberately keeps file-true offsets (its lexer skips the BOM
		// but never shifts positions — acorn agrees on the TS side).
		name: 'bom_offset',
		conformance_section: 'CSS Parser Corrections (corpus-enforced) — BOM offset shift',
		matches: (entry, _canonical_parent, ctx) =>
			ctx.source.charCodeAt(0) === 0xfeff &&
			entry.kind === 'value_mismatch' &&
			typeof entry.ours === 'number' &&
			typeof entry.canonical === 'number' &&
			entry.ours === entry.canonical + 1,
	},
	{
		// Under lang="ts", Svelte parses `{#each expr as binding}` by letting the TS
		// parser read `expr as binding` as an as-expression, then unwraps it — patching
		// the expression's `end` OFFSET back to the real expression but leaving
		// `loc.end` at the as-expression's end (after the binding). tsv's loc agrees
		// with the corrected offset. Scoped to EachBlock expressions; the offsets and
		// loc.start are not absorbed, so a real loc bug still surfaces undocumented.
		name: 'each_as_stale_loc',
		conformance_section: 'Svelte Template Corrections (corpus-enforced) — each-as stale loc.end',
		matches: (entry, _canonical_parent, ctx) => {
			const m = entry.path.match(/^(.*)\.expression\.loc\.end\.(line|column)$/);
			if (!m) return false;
			const owner = get_at_path(ctx.canonical_root, m[1]) as { type?: unknown } | null;
			return owner?.type === 'EachBlock';
		},
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
		},
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
						member.key?.name === 'static',
				);
			}
			return false;
		},
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
		},
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
				'\u{FFFD}',
			);
			return replaced !== entry.canonical && replaced === entry.ours;
		},
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
				/^(.*\.decorators\[\d+\]\.expression)(?:\.(?:callee|object|expression))*\.(?:start|loc\.start\.(?:line|column))$/,
			);
			if (!m) return false;
			const expr = get_at_path(ctx.canonical_root, m[1]) as { type?: unknown } | null;
			return expr?.type === 'CallExpression' || expr?.type === 'MemberExpression';
		},
	},
	{
		// Svelte's read_declaration tokenizes garbage when a stray `;` or an adjacent
		// comment touches the property name: `border-box;;` yields a declaration with
		// property ";" that swallows the next declaration into its value, and
		// `color/* c */:` yields property "color/*" with the comment tail leaking into
		// the value (read_until stops at the first whitespace, which sits INSIDE the
		// comment). tsv parses per spec (skips empty declarations, tokenizes comments).
		name: 'css_declaration_tokenization',
		conformance_section:
			'CSS Parser Corrections (corpus-enforced) — Declaration tokenization garbage',
		matches: (_entry, canonical_parent) => {
			const parent = canonical_parent as
				| { type?: unknown; property?: unknown }
				| null
				| undefined;
			return (
				parent?.type === 'Declaration' &&
				typeof parent.property === 'string' &&
				(parent.property.startsWith(';') || parent.property.includes('/*'))
			);
		},
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
					typeParameters?: {end?: unknown} | null;
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
		},
	},
	{
		// Svelte's parseCss reads `:nth-child(An+B of S)` as `Nth.value = "2n of "`
		// (the ` of ` leaks in from its REGEX_NTH_OF terminator) and flattens `S` as
		// sibling simple selectors of the Nth. Per Selectors 4 the `S` is a nested
		// <complex-selector-list> scoped to the nth term, so tsv keeps `Nth.value =
		// "2n"` with `S` nested under a tsv-only `Nth.selector` field. The whole args
		// subtree reshapes: the `Nth.value`/`.selector`, the sibling-count lengths, and
		// the container span ends all differ. Anchor on Svelte's unambiguous tell — a
		// canonical `Nth` whose value contains " of " — and absorb only the
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
		},
	},
];

function classify(
	entry: Omit<DiffEntry, 'documented' | 'signature'>,
	canonical_parent: unknown,
	ctx: MatchContext,
): string | null {
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

/**
 * Recursively diff two JSON-shaped values, collecting up to MAX_DIFFS_PER_FILE
 * entries. Arrays with differing lengths report one length_mismatch and still
 * recurse the shared prefix so positional drift inside is visible.
 */
function diff_asts(
	ours: unknown,
	canonical: unknown,
	ctx: MatchContext,
): { diffs: DiffEntry[]; truncated: boolean } {
	const diffs: DiffEntry[] = [];
	let truncated = false;

	const push = (
		kind: DiffKind,
		path: string,
		o: unknown,
		c: unknown,
		canonical_parent: unknown,
	): void => {
		if (diffs.length >= MAX_DIFFS_PER_FILE) {
			truncated = true;
			return;
		}
		const base = { path, kind, ours: o, canonical: c };
		diffs.push({
			...base,
			signature: `${kind}:${path_signature(path)}`,
			documented: classify(base, canonical_parent, ctx),
		});
	};

	const walk = (o: unknown, c: unknown, path: string, canonical_parent: unknown): void => {
		if (truncated || diffs.length >= MAX_DIFFS_PER_FILE) {
			truncated = true;
			return;
		}
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
					walk(o_arr[i], c_arr[i], `${path}[${i}]`, c);
				}
				break;
			}
			case 'object': {
				const o_obj = o as Record<string, unknown>;
				const c_obj = c as Record<string, unknown>;
				const keys = new Set([...Object.keys(o_obj), ...Object.keys(c_obj)]);
				for (const key of keys) {
					const child_path = path === '' ? key : `${path}.${key}`;
					const in_ours = key in o_obj;
					const in_canonical = key in c_obj;
					if (!in_ours) {
						push('missing_ours', child_path, undefined, c_obj[key], c);
					} else if (!in_canonical) {
						push('missing_canonical', child_path, o_obj[key], undefined, c);
					} else {
						walk(o_obj[key], c_obj[key], child_path, c);
					}
				}
				break;
			}
			default:
				push('value_mismatch', path, o, c, canonical_parent);
		}
	};

	walk(ours, canonical, '', null);
	return { diffs, truncated };
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

/** Group stored diff entries across files by (language, signature). */
function build_groups(results: Map<Language, FileResult[]>): DiffGroup[] {
	const groups = new Map<string, DiffGroup>();
	for (const lang of LANGUAGES) {
		for (const r of results.get(lang)!) {
			for (const entry of r.diffs) {
				const key = `${lang}:${entry.signature}`;
				let group = groups.get(key);
				if (!group) {
					group = {
						language: lang,
						signature: entry.signature,
						documented: entry.documented,
						files: new Set(),
						entry_count: 0,
						samples: [],
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
  --verbose          Show each file as it's processed + per-file diff detail
  --json             Emit a single JSON report to stdout; human output → stderr
  --help             Show this help message

Examples:
  deno task corpus:compare:parse --all
  deno task corpus:compare:parse --all --multibyte-only
  deno task corpus:compare:parse ~/dev/zzz --filter typescript --limit 100
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
		both_errors: s.both_errors,
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

/** Build the single buffered JSON report. */
function build_json_report(
	results: Map<Language, FileResult[]>,
	stats: Map<Language, LanguageStats>,
	groups: DiffGroup[],
	base_path: string,
): Record<string, unknown> {
	return {
		stats: build_stats_block(stats),
		groups: groups.map((g) => ({
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
				canonical: preview(s.entry.canonical),
			})),
		})),
		errors: LANGUAGES.flatMap((lang) =>
			results.get(lang)!
				.filter((r) => r.status.endsWith('_error'))
				.map((r) => ({
					path: rel_path(r.path, base_path),
					language: lang,
					status: r.status,
					error: r.error,
				}))
		),
		truncated_files: LANGUAGES.flatMap((lang) =>
			results.get(lang)!.filter((r) => r.truncated).map((r) => rel_path(r.path, base_path))
		),
	};
}

async function main(): Promise<void> {
	const parsed = args_parse(argv_parse(process.argv.slice(2)), CorpusCompareParseArgs);
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
		use_all_repos ? 'Parse-comparing: All default corpus repos' : `Parse-comparing: ${base_path}`,
	);
	if (filter_lang) console.log(`Filter: ${filter_lang} only`);
	if (limit) console.log(`Limit: ${limit} files per language`);
	if (multibyte_only) console.log('Mode: multibyte-only (offset-translation slice)');
	console.log();

	const loader = create_compare_loader(use_all_repos, base_path);
	const { canonical, native } = await init_compare_implementations();

	const results: Map<Language, FileResult[]> = new Map();
	const stats: Map<Language, LanguageStats> = new Map();
	for (const lang of LANGUAGES) {
		results.set(lang, []);
		stats.set(lang, empty_stats());
	}

	const lang_counts: Record<Language, number> = { svelte: 0, typescript: 0, css: 0 };

	for await (const file of loader.stream(verbose ? console.log : () => {})) {
		const lang = file.language;
		if (filter_lang && lang !== filter_lang) continue;
		const multibyte = has_non_ascii(file.content);
		if (multibyte_only && !multibyte) continue;
		if (limit && lang_counts[lang] >= limit) continue;
		lang_counts[lang]++;

		const lang_stats = stats.get(lang)!;
		const lang_results = results.get(lang)!;
		lang_stats.total++;

		if (verbose) console.log(`  ${file.path}`);

		// Parse both sides; a throw on either side is an error bucket, not a diff.
		let ours: unknown;
		let tsv_error: string | null = null;
		try {
			ours = native.parse(file.content, lang);
		} catch (e) {
			tsv_error = String(e instanceof Error ? e.message : e).split('\n')[0];
		}
		let canonical_ast: unknown;
		let canonical_error: string | null = null;
		try {
			// Serialize exactly like the fixture sidecar does (BigInt → string;
			// RegExp values collapse to {}), so corpus and fixture semantics match.
			canonical_ast = JSON.parse(
				JSON.stringify(canonical.parse(file.content, lang), bigint_replacer),
			);
		} catch (e) {
			canonical_error = String(e instanceof Error ? e.message : e).split('\n')[0];
		}

		if (tsv_error || canonical_error) {
			const status = tsv_error && canonical_error
				? 'both_error'
				: tsv_error
				? 'tsv_error'
				: 'canonical_error';
			lang_stats[`${status}s` as 'both_errors' | 'tsv_errors' | 'canonical_errors']++;
			lang_results.push({
				path: file.path,
				bytes: file.bytes,
				multibyte,
				status,
				diffs: [],
				truncated: false,
				error: tsv_error ?? canonical_error ?? undefined,
			});
			continue;
		}

		lang_stats.compared++;
		if (multibyte) lang_stats.multibyte++;

		const { diffs, truncated } = diff_asts(ours, canonical_ast, {
			source: file.content,
			canonical_root: canonical_ast,
		});
		if (diffs.length === 0) {
			lang_stats.match++;
			continue; // exact matches are counted, not stored
		}

		const all_documented = diffs.every((d) => d.documented !== null);
		if (all_documented) {
			lang_stats.documented++;
		} else {
			lang_stats.undocumented++;
		}
		lang_results.push({
			path: file.path,
			bytes: file.bytes,
			multibyte,
			status: all_documented ? 'documented' : 'undocumented',
			diffs,
			truncated,
		});

		if (verbose) {
			for (const d of diffs) {
				const tag = d.documented ? `documented:${d.documented}` : 'UNDOCUMENTED';
				console.log(`    ${d.kind} at ${d.path} (${tag})`);
				console.log(`      ours: ${preview(d.ours)}  canonical: ${preview(d.canonical)}`);
			}
		}
	}

	const total_processed = Object.values(lang_counts).reduce((a, b) => a + b, 0);
	if (total_processed === 0) {
		console.log('No files found.');
		if (json_mode) emit_json_stdout(build_json_report(results, stats, [], base_path));
		canonical.dispose();
		native.dispose();
		return;
	}

	const counts = LANGUAGES.map((lang) => `${lang_counts[lang]} ${lang}`).join(', ');
	console.log(`\nProcessed: ${total_processed} files (${counts})\n`);

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
			`\n\x1b[2mParse failures skipped (tsv ${totals.tsv_errors} / canonical ${totals.canonical_errors} / both ${totals.both_errors}) — triage with diagnostics/skip_triage.ts\x1b[0m`,
		);
	}

	const groups = build_groups(results);
	const undocumented_groups = groups.filter((g) => g.documented === null);
	const documented_groups = groups.filter((g) => g.documented !== null);

	if (json_mode) emit_json_stdout(build_json_report(results, stats, groups, base_path));

	// Undocumented groups — the actionable output
	if (undocumented_groups.length > 0) {
		console.log(`\n\x1b[31mUNDOCUMENTED diff groups (${undocumented_groups.length}):\x1b[0m`);
		for (const g of undocumented_groups) {
			console.log(
				`\n  [${g.language}] ${g.signature}  (${g.files.size} files, ${g.entry_count} sites)`,
			);
			for (const s of g.samples) {
				console.log(`    ${rel_path(s.path, base_path)}`);
				console.log(`      at ${s.entry.path}`);
				console.log(
					`      ours: ${preview(s.entry.ours)}  canonical: ${preview(s.entry.canonical)}`,
				);
			}
			if (g.files.size > g.samples.length) {
				console.log(`    ... and ${g.files.size - g.samples.length} more files`);
			}
		}
	}

	// Documented groups — compact summary only
	if (documented_groups.length > 0) {
		console.log(`\nDocumented divergence groups (${documented_groups.length}):`);
		for (const g of documented_groups) {
			console.log(
				`  [${g.language}] ${g.documented}: ${g.signature} (${g.files.size} files)`,
			);
		}
	}

	const truncated_files = LANGUAGES.flatMap((lang) =>
		results.get(lang)!.filter((r) => r.truncated)
	);
	if (truncated_files.length > 0) {
		console.log(
			`\n\x1b[33mNote: ${truncated_files.length} file(s) hit the ${MAX_DIFFS_PER_FILE}-diff cap — diff lists are partial:\x1b[0m`,
		);
		for (const r of truncated_files.slice(0, 5)) {
			console.log(`  ${rel_path(r.path, base_path)}`);
		}
		if (truncated_files.length > 5) {
			console.log(`  ... and ${truncated_files.length - 5} more`);
		}
	}

	console.log();
	if (total_undocumented > 0) {
		console.log(
			`\x1b[31mFAIL: ${total_undocumented} file(s) with undocumented AST diffs vs canonical\x1b[0m`,
		);
		canonical.dispose();
		native.dispose();
		Deno.exit(1);
	} else {
		console.log('\x1b[32mPASS: no undocumented AST diffs vs canonical\x1b[0m');
	}

	canonical.dispose();
	native.dispose();
}

/**
 * Minimal but valid JSON report for the failure path, keeping the contract
 * that `--json` always writes a parseable document to stdout.
 */
function build_error_json_report(message: string): Record<string, unknown> {
	const empty: Map<Language, LanguageStats> = new Map(
		LANGUAGES.map((lang) => [lang, empty_stats()]),
	);
	return {
		stats: build_stats_block(empty),
		groups: [],
		errors: [],
		truncated_files: [],
		error: message,
	};
}

run_compare_main(main, CorpusCompareParseArgs, build_error_json_report);
