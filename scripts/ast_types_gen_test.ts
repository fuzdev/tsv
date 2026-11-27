/**
 * The `.d.ts` generator: entries merging into one interface, member optionality, the layouts
 * `tsv format` gives a union, and the disagreements it refuses to paper over.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { test } from 'vitest';

import type { AstSchema, AstSchemaEntry } from './ast_schema.ts';
import { generate_ast_types, split_top_level_union } from './ast_types_gen.ts';

const one_section = (entries: AstSchema['sections'][number]['entries']): AstSchema => ({
	doc: ['Header.'],
	sections: [{ title: 'Nodes', entries }]
});

const generated = (schema: AstSchema): string => {
	const out = generate_ast_types(schema);
	if (typeof out !== 'string') throw new Error(out.errors.join('\n'));
	return out;
};

const names = (n: number, stem: string) => Array.from({ length: n }, (_, i) => `${stem}${i}`);

const comment = (type: string, extra: Partial<AstSchemaEntry> = {}): AstSchemaEntry => ({
	type,
	interface: 'Comment',
	fields: [
		{ key: 'type', kind: 'type' },
		{ key: 'value', kind: 'string', ts: 'string', string: 'slice' },
		{ key: 'start', kind: 'offset', ts: 'number', doc: ['Where it starts.'] },
		{ key: 'loc', kind: 'loc', entry: 'SourceLocation', presence: 'locations' }
	],
	...extra
});

test('entries sharing an interface merge: the type a union, a member one lacks optional', () => {
	const bare: AstSchemaEntry = {
		type: 'Line',
		variant: 'Bare',
		interface: 'Comment',
		fields: [
			{ key: 'type', kind: 'type' },
			{ key: 'value', kind: 'string', ts: 'string', string: 'slice' }
		]
	};
	const text = generated(
		one_section([comment('Line', { doc: ['A comment.'] }), comment('Block'), bare])
	);
	strictEqual(
		text,
		[
			'/** Header. */',
			'',
			'//',
			'// Nodes',
			'//',
			'',
			'/** A comment. */',
			'export interface Comment {',
			"\ttype: 'Line' | 'Block';",
			'\tvalue: string;',
			'\t/** Where it starts. */',
			'\tstart?: number;',
			'\tloc?: SourceLocation;',
			'}',
			''
		].join('\n')
	);
});

test('a dialect-dependent field is optional; a comment list with no ts is declared elsewhere', () => {
	const text = generated(
		one_section([
			{
				type: 'Import',
				interface: 'Import',
				extends: ['Mixin'],
				fields: [
					{ key: 'type', kind: 'type' },
					{
						key: 'kind',
						kind: 'string',
						ts: 'string',
						string: 'keyword',
						vanilla_acorn: 'optional'
					},
					{ key: 'leadingComments', kind: 'comments', presence: 'optional' }
				]
			}
		])
	);
	ok(
		text.includes("export interface Import extends Mixin {\n\ttype: 'Import';\n\tkind?: string;\n}")
	);
});

test('a union alias takes the layout tsv format gives it at each width', () => {
	const text = generated(
		one_section([
			{ alias: 'Short', union: ['A', 'B'] },
			{ alias: 'Hung', union: names(5, 'LongerMemberName') },
			{ alias: 'Broken', union: names(12, 'LongerMemberName') },
			{ alias: 'Mixed', union: names(12, 'LongerMemberName'), intersect: 'Mixin' }
		])
	);
	ok(text.includes('export type Short = A | B;\n'));
	ok(text.includes('export type Hung =\n\tLongerMemberName0 | LongerMemberName1'));
	ok(text.includes('export type Broken =\n\t| LongerMemberName0\n\t| LongerMemberName1\n'));
	ok(text.includes('export type Mixed = (\n\t| LongerMemberName0\n'));
	ok(text.includes('\t| LongerMemberName11\n) &\n\tMixin;\n'));
});

const member = (key: string, ts: string): AstSchemaEntry => ({
	type: 'Node',
	interface: 'Node',
	fields: [
		{ key: 'type', kind: 'type' },
		{ key, kind: 'list', ts }
	]
});

test('a union splits at its top level alone, never inside brackets or a quoted literal', () => {
	deepStrictEqual(
		split_top_level_union("A | (B | C)[] | Array<D | E> | '|' | '| x' | { a: F | G }"),
		['A', '(B | C)[]', 'Array<D | E>', "'|'", "'| x'", '{ a: F | G }']
	);
	deepStrictEqual(split_top_level_union('(a: A) => B | C'), ['(a: A) => B | C']);
	deepStrictEqual(split_top_level_union('A'), ['A']);
});

test('a too-wide member union breaks one alternative a line, a bracketed one kept whole', () => {
	const ts = [...names(5, 'LongerMemberName'), '(A | B)[]', "'| x'"].join(' | ');
	const text = generated(one_section([member('items', ts)]));
	ok(
		text.includes(
			[
				'\titems:',
				...names(5, 'LongerMemberName').map((n) => `\t\t| ${n}`),
				'\t\t| (A | B)[]',
				"\t\t| '| x';"
			].join('\n')
		)
	);
});

test('a too-wide parenthesized array breaks inside its parentheses', () => {
	const ts = `(${names(9, 'LongerMemberName').join(' | ')})[]`;
	const text = generated(one_section([member('items', ts)]));
	ok(
		text.includes(
			['\titems: (', ...names(9, 'LongerMemberName').map((n) => `\t\t| ${n}`), '\t)[];'].join('\n')
		)
	);
	const fits = generated(one_section([member('items', '(A | B)[]')]));
	ok(fits.includes('\titems: (A | B)[];\n'));
});

test('a too-wide member with no layout is refused, not emitted unformatted', () => {
	const wide = `(${names(9, 'LongerMemberName').join(' | ')})[]`;
	for (const ts of [`${wide} | null`, `Array<${names(9, 'LongerMemberName').join(' | ')}>`]) {
		const out = generate_ast_types(one_section([member('items', ts)]));
		ok(typeof out !== 'string' && out.errors.some((e) => e.startsWith('Node.items: no layout')));
	}
	const unbreakable = 'A'.repeat(100);
	ok(
		generated(one_section([member('items', `${unbreakable}[]`)])).includes(
			`\titems: ${unbreakable}[];`
		)
	);
});

test('entries that disagree on a member are refused, not merged', () => {
	const disagree = comment('Block');
	disagree.fields[1] = { key: 'value', kind: 'number', ts: 'number' };
	const out = generate_ast_types(one_section([comment('Line'), disagree]));
	ok(typeof out !== 'string' && out.errors.some((e) => e.includes('disagree on `ts`')));
	const reordered = comment('Block');
	reordered.fields = [reordered.fields[0]!, reordered.fields[2]!, reordered.fields[1]!];
	const out2 = generate_ast_types(one_section([comment('Line'), reordered]));
	ok(typeof out2 !== 'string' && out2.errors.some((e) => e.includes('order')));
});
