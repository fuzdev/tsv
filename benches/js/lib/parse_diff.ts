/**
 * The parse-comparison diff engine: a raw deep diff of tsv's wire against a canonical
 * parser's (`diff_asts`), with each difference classified against the documented
 * divergences (`parse_divergences.ts`) at the reporting layer only — no normalization is
 * applied before diffing, so a bug in the divergence reasoning surfaces as an undocumented
 * entry instead of being silently absorbed.
 *
 * `loc` is graded by its own rules (see `diff_asts`): per half where the offsets agree,
 * the pinned superset and the tolerance rows for Svelte (`loc_tolerance.ts`), exactly for
 * TypeScript.
 *
 * Shared by `corpus_compare_parse.ts` and the parse-conformance fixtures gates
 * (`fixtures_gate.ts`). Node-modules-free, so `parse_diff_test.ts` gates it in
 * `deno task test:deno`.
 *
 * @module
 */

import {
	classify_loc_difference,
	is_loc_leaf,
	type LocRow,
	superset_key,
	superset_kinds_of,
	type SupersetAnchor,
	wire_text
} from './loc_tolerance.ts';
import { DOCUMENTED_MATCHERS } from './parse_divergences.ts';
import type { Language } from './types.ts';

/** Per-file diff cap — collection stops here and the file is flagged truncated. */
export const MAX_DIFFS_PER_FILE = 50;

export type DiffKind =
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

/** Truncated single-line preview of a leaf value for reports. */
export function preview(value: unknown): string {
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

/** Per-file context available to matchers (some divergences are file-level, e.g. BOM). */
export interface MatchContext {
	source: string;
	/** Root of the canonical AST — lets matchers resolve ancestors from the entry path. */
	canonical_root: unknown;
	/**
	 * The document's language, which decides how `loc` is graded: exactly for TypeScript
	 * (acorn is the outside reference), with the pinned superset and the tolerance rows
	 * for Svelte (`lib/loc_tolerance.ts`), and as a pinned superset for CSS (`parseCss`
	 * emits no `loc`).
	 */
	language: Language;
	/**
	 * `corpus_compare_parse.ts --fixtures` only: this document is a `_svelte_divergence`
	 * fixture's input whose two parses equal its committed `expected_ours.json` /
	 * `expected_svelte.json` — so every span difference is the one the fixture declares (and
	 * `fixtures:validate` grades). Never excuses a `loc` or `name_loc` difference.
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

/** A path inside a `loc` or `name_loc` — what a fixture's span-only pins say nothing about. */
const LINE_COLUMN_PATH = /(^|\.)(name_)?loc(\.|$)/;

/**
 * The classification `--fixtures` gives a `_svelte_divergence` fixture's declared difference
 * — every span difference in a document whose parses still equal the committed pins (see
 * `MatchContext.declared_divergence` for what that reach hides).
 */
const FIXTURE_DECLARED_DIVERGENCE = 'fixture_declared_divergence';

/**
 * The documented divergence `entry` is, by name, or `null` = undocumented (actionable). A
 * `loc` line or column, and a tsv `loc` outside the pinned superset, are never classified
 * here — those are the tolerance rows' and the superset rule's alone (see `diff_asts`).
 */
export function classify(
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
	 * The `loc` differences the tolerance rows absorbed, per row — counted rather than
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
 * - a differing `loc` line or column is a finding unless one of the tolerance rows
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
