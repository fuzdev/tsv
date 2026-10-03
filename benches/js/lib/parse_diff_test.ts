/**
 * Tests for the parse-comparison diff engine (`diff_asts`) and its classification seam
 * (`classify`).
 *
 * Hand-written trees, on purpose: the engine is a pure function over two JSON values, and a
 * test that parsed through the oracle would need the sidecar and stop gating in
 * `deno task check` (see `test:deno`). What the real oracle writes is graded by
 * `corpus:compare:parse`.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import { classify, diff_asts, type MatchContext, MAX_DIFFS_PER_FILE } from './parse_diff.ts';
import type { Language } from './types.ts';

const ctx = (canonical_root: unknown, language: Language = 'typescript'): MatchContext => ({
	source: '',
	canonical_root,
	language
});

/** Diff two trees with the canonical one as the match context's root. */
const diff = (ours: unknown, canonical: unknown, language: Language = 'typescript') =>
	diff_asts(ours, canonical, ctx(canonical, language));

/** The diffs as `kind path` lines, the shape most assertions read. */
const kinds = (ours: unknown, canonical: unknown): string[] =>
	diff(ours, canonical).diffs.map((d) => `${d.kind} ${d.path}`);

Deno.test('diff_asts: equal trees have no diffs', () => {
	const tree = { type: 'Program', start: 0, end: 3, body: [{ type: 'X', value: 'a' }] };
	const result = diff(tree, structuredClone(tree));
	deepStrictEqual(result.diffs, []);
	strictEqual(result.truncated, false);
	deepStrictEqual(result.loc_rows, {});
	deepStrictEqual(result.loc_superset, {});
	strictEqual(result.loc_span_skipped, 0);
});

Deno.test('diff_asts: a leaf value mismatch, with array indices erased in its signature', () => {
	const { diffs } = diff({ body: [{ end: 4 }] }, { body: [{ end: 5 }] });
	strictEqual(diffs.length, 1);
	const [d] = diffs;
	strictEqual(d!.kind, 'value_mismatch');
	strictEqual(d!.path, 'body[0].end');
	strictEqual(d!.signature, 'value_mismatch:body[].end');
	strictEqual(d!.ours, 4);
	strictEqual(d!.canonical, 5);
	strictEqual(d!.documented, null);
});

Deno.test('diff_asts: a type mismatch stops at the node and does not recurse', () => {
	deepStrictEqual(kinds({ a: 1 }, { a: '1' }), ['type_mismatch a']);
	deepStrictEqual(kinds({ a: { b: 1 } }, { a: [1] }), ['type_mismatch a']);
	deepStrictEqual(kinds({ a: null }, { a: {} }), ['type_mismatch a']);
});

Deno.test('diff_asts: a length mismatch still diffs the shared prefix', () => {
	deepStrictEqual(kinds({ xs: [1, 2] }, { xs: [1, 2, 3] }), ['length_mismatch xs']);
	deepStrictEqual(kinds({ xs: [9, 2, 3] }, { xs: [1, 2] }), [
		'length_mismatch xs',
		'value_mismatch xs[0]'
	]);
	const [d] = diff({ xs: [1] }, { xs: [1, 2] }).diffs;
	strictEqual(d!.ours, 1);
	strictEqual(d!.canonical, 2);
});

Deno.test('diff_asts: a key on one side only is missing on the other', () => {
	deepStrictEqual(kinds({ a: 1 }, { a: 1, b: 2 }), ['missing_ours b']);
	deepStrictEqual(kinds({ a: 1, b: 2 }, { a: 1 }), ['missing_canonical b']);
	const [d] = diff({}, { b: 2 }).diffs;
	strictEqual(d!.ours, undefined);
	strictEqual(d!.canonical, 2);
});

Deno.test('diff_asts: collection stops at the per-file cap and flags the file truncated', () => {
	const ours: Record<string, number> = {};
	const canonical: Record<string, number> = {};
	for (let i = 0; i < MAX_DIFFS_PER_FILE + 5; i++) {
		ours[`k${i}`] = 0;
		canonical[`k${i}`] = 1;
	}
	const result = diff(ours, canonical);
	strictEqual(result.diffs.length, MAX_DIFFS_PER_FILE);
	strictEqual(result.truncated, true);
	strictEqual(diff({ k: 0 }, { k: 1 }).truncated, false);
});

Deno.test('diff_asts: a documented divergence is classified by its matcher', () => {
	const [d] = diff({ value: '\u{FFFD}' }, { value: '\uD800' }).diffs;
	strictEqual(d!.documented, 'lone_surrogate_value');
	const [near] = diff({ value: 'x' }, { value: '\uD800' }).diffs;
	strictEqual(near!.documented, null);
});

Deno.test(
	'diff_asts: a TypeScript `loc` half follows its span, the other half still grades',
	() => {
		const at = (line: number, column: number) => ({ line, column });
		const ours = { start: 0, end: 5, loc: { start: at(1, 0), end: at(1, 5) } };
		// `end` differs: graded at the span, `loc.end` skipped
		const end_moved = { start: 0, end: 6, loc: { start: at(1, 0), end: at(1, 6) } };
		const skipped = diff(ours, end_moved);
		deepStrictEqual(
			skipped.diffs.map((d) => `${d.kind} ${d.path}`),
			['value_mismatch end']
		);
		strictEqual(skipped.loc_span_skipped, 1);
		// `end` differs AND `loc.start` does: the agreeing half is still graded, exactly
		const both = { start: 0, end: 6, loc: { start: at(2, 0), end: at(1, 6) } };
		deepStrictEqual(kinds(ours, both).sort(), [
			'value_mismatch end',
			'value_mismatch loc.start.line'
		]);
		// offsets agree: a TypeScript `loc` difference is never tolerated
		const line_moved = { start: 0, end: 5, loc: { start: at(1, 0), end: at(2, 5) } };
		const [d] = diff(ours, line_moved).diffs;
		strictEqual(d!.path, 'loc.end.line');
		strictEqual(d!.documented, null);
	}
);

Deno.test(
	'diff_asts: a tsv `loc` the TypeScript oracle lacks is a difference, never superset',
	() => {
		const ours = { start: 0, end: 1, loc: { start: { line: 1, column: 0 } } };
		const result = diff(ours, { start: 0, end: 1 });
		deepStrictEqual(
			result.diffs.map((d) => `${d.kind} ${d.path} ${d.documented}`),
			['missing_canonical loc null']
		);
		deepStrictEqual(result.loc_superset, {});
	}
);

Deno.test('classify: a declared divergence excuses span differences only', () => {
	const declared: MatchContext = { ...ctx({}), declared_divergence: true };
	const entry = (kind: 'value_mismatch' | 'missing_canonical', path: string) => ({
		kind,
		path,
		ours: 1,
		canonical: 2
	});
	strictEqual(
		classify(entry('value_mismatch', 'body[0].end'), {}, declared),
		'fixture_declared_divergence'
	);
	strictEqual(classify(entry('value_mismatch', 'body[0].end'), {}, ctx({})), null);
	// a `loc` / `name_loc` leaf is the tolerance rows' alone
	strictEqual(classify(entry('value_mismatch', 'body[0].loc.start.line'), {}, declared), null);
	strictEqual(classify(entry('value_mismatch', 'name_loc.end.column'), {}, declared), null);
	strictEqual(classify(entry('missing_canonical', 'body[0].loc'), {}, declared), null);
});

Deno.test('classify: no matcher may excuse a `loc` leaf or an unpinned superset `loc`', () => {
	// `static_member_ladder` claims EVERY difference under a ladder class body — the shape of
	// matcher the refusal exists for
	const member = {
		type: 'PropertyDefinition',
		computed: false,
		value: null,
		key: { name: 'static' }
	};
	const root = { body: [{ type: 'ClassBody', body: [member] }] };
	const entry = (kind: 'value_mismatch' | 'missing_canonical', path: string) => ({
		kind,
		path,
		ours: 1,
		canonical: 2
	});
	strictEqual(
		classify(entry('value_mismatch', 'body[0].body[0].start'), member, ctx(root)),
		'static_member_ladder'
	);
	strictEqual(
		classify(entry('value_mismatch', 'body[0].body[0].loc.start.line'), member, ctx(root)),
		null
	);
	strictEqual(classify(entry('missing_canonical', 'body[0].body[0].loc'), member, ctx(root)), null);
});
