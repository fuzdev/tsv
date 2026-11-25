/**
 * The `.d.ts` generator: entries merging into one interface, member optionality, the layouts
 * `tsv format` gives a union, and the disagreements it refuses to paper over.
 */

import { ok, strictEqual } from 'node:assert';
import { test } from 'vitest';

import type { AstSchema, AstSchemaEntry } from './ast_schema.ts';
import { generate_ast_types } from './ast_types_gen.ts';

const one_section = (entries: AstSchema['sections'][number]['entries']): AstSchema => ({
	doc: ['Header.'],
	sections: [{ title: 'Nodes', entries }]
});

const generated = (schema: AstSchema): string => {
	const out = generate_ast_types(schema);
	if (typeof out !== 'string') throw new Error(out.errors.join('\n'));
	return out;
};

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
	const names = (n: number, stem: string) => Array.from({ length: n }, (_, i) => `${stem}${i}`);
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

test('an opaque entry generates nothing', () => {
	const text = generated(
		one_section([
			{ type: 'Rule', css: true, opaque: true, fields: [{ key: 'type', kind: 'type' }] }
		])
	);
	ok(!text.includes('Rule'));
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
