/**
 * The AST schema table's reader and layout: the vocabulary it refuses, and a layout that is a
 * fixed point — the shape `check:ast-schema` holds the committed table to.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { test } from 'vitest';

import {
	type AstSchema,
	duplicate_keys,
	layout_ast_schema,
	parse_ast_schema
} from './ast_schema.ts';

const table = (): AstSchema => ({
	doc: ['A header.'],
	sections: [
		{
			title: 'One',
			entries: [
				{ alias: 'Node', union: ['Leaf', 'Pair'] },
				{
					type: 'Leaf',
					interface: 'Leaf',
					doc: ['A leaf.', '', 'Two paragraphs.'],
					fields: [
						{ key: 'type', kind: 'type' },
						{ key: 'start', kind: 'offset', ts: 'number' },
						{ key: 'name', kind: 'string', ts: 'string', string: 'own', doc: ['Its text.'] },
						{
							key: 'value',
							kind: ['string', 'number'],
							ts: 'string | number',
							string: 'cooked',
							nullable: true
						}
					]
				},
				{
					interface: 'Extra',
					fields: [{ key: 'raw', kind: 'string', ts: 'string', string: 'slice' }]
				},
				{
					type: 'Pair',
					interface: 'Pair',
					fields: [
						{ key: 'type', kind: 'type' },
						{ key: 'extra', kind: 'object', entry: 'Extra', presence: 'optional' }
					]
				}
			]
		}
	]
});

test('the layout is a fixed point of reading it back', () => {
	const text = layout_ast_schema(table());
	const read = parse_ast_schema(text);
	ok(!('errors' in read), JSON.stringify(read));
	strictEqual(layout_ast_schema(read), text);
	deepStrictEqual(read, table());
});

test('a field fits one line; a multi-line doc takes a string per line', () => {
	const text = layout_ast_schema(table());
	ok(text.includes('\t\t\t\t\t\t{"key": "start", "kind": "offset", "ts": "number"},\n'));
	ok(text.includes('\t\t\t\t\t"doc": [\n\t\t\t\t\t\t"A leaf.",\n\t\t\t\t\t\t"",\n'));
	// a one-line doc stays inline, but a field carrying one is never folded onto a line
	ok(text.includes('\t\t\t\t\t\t\t"doc": ["Its text."]\n'));
});

test('the layout orders keys by object kind, whatever order they were written in', () => {
	const scrambled = table();
	const leaf = scrambled.sections[0]!.entries[1]!;
	const reordered = JSON.parse(
		JSON.stringify(leaf, [
			'fields',
			'doc',
			'interface',
			'type',
			'key',
			'kind',
			'ts',
			'string',
			'nullable'
		])
	);
	scrambled.sections[0]!.entries[1] = reordered;
	strictEqual(layout_ast_schema(scrambled), layout_ast_schema(table()));
});

const errors_of = (mutate: (raw: any) => void): string[] => {
	const raw = JSON.parse(JSON.stringify(table()));
	mutate(raw);
	const read = parse_ast_schema(JSON.stringify(raw));
	return 'errors' in read ? read.errors : [];
};

test('the reader refuses a key, a kind, a presence or a string class outside the vocabulary', () => {
	ok(
		errors_of((t) => (t.sections[0].entries[1].colour = 'red')).some((e) =>
			e.includes('unknown key "colour"')
		)
	);
	ok(
		errors_of((t) => (t.sections[0].entries[1].fields[1].kind = 'integer')).some((e) =>
			e.includes('outside the vocabulary')
		)
	);
	ok(
		errors_of((t) => (t.sections[0].entries[3].fields[1].presence = 'sometimes')).some((e) =>
			e.includes('outside the vocabulary')
		)
	);
	ok(
		errors_of((t) => (t.sections[0].entries[1].fields[2].string = 'borrowed')).some((e) =>
			e.includes('outside the vocabulary')
		)
	);
});

test('the reader refuses what it cannot mean: a default spelled out, a class with no string, a duplicate', () => {
	ok(
		errors_of((t) => (t.sections[0].entries[1].fields[1].presence = 'always')).some((e) =>
			e.includes('is the default')
		)
	);
	ok(
		errors_of((t) => delete t.sections[0].entries[1].fields[2].string).some((e) =>
			e.includes('come together')
		)
	);
	ok(
		errors_of((t) =>
			t.sections[0].entries[1].fields.push({ key: 'name', kind: 'number', ts: 'number' })
		).some((e) => e.includes('key name twice'))
	);
	ok(
		errors_of((t) =>
			t.sections[0].entries.push(JSON.parse(JSON.stringify(t.sections[0].entries[1])))
		).some((e) => e.includes('Leaf has two entries'))
	);
	ok(
		errors_of((t) => (t.sections[0].entries[3].fields[1].entry = 'Missing')).some((e) =>
			e.includes('names no entry')
		)
	);
});

test('a key written twice in one object is refused, never collapsed to the last', () => {
	const text = layout_ast_schema(table()).replace(
		'"title": "One",',
		'"title": "One",\n\t\t\t"title": "Two",'
	);
	const read = parse_ast_schema(text);
	ok('errors' in read && read.errors.some((e) => e.includes('key "title" twice in one object')));
	deepStrictEqual(duplicate_keys('{"a": {"b": 1, "c": [{"b": 2}]}, "d": "\\"b\\""}'), []);
	deepStrictEqual(duplicate_keys('{"a": 1,\n"a": 2}'), [{ key: 'a', line: 2 }]);
});

test('a keyword field and its values come together', () => {
	ok(
		errors_of((t) => {
			t.sections[0].entries[1].fields[2].string = 'keyword';
		}).some((e) => e.includes('its `values` come together'))
	);
	ok(
		errors_of((t) => {
			t.sections[0].entries[1].fields[2].values = ['a'];
		}).some((e) => e.includes('its `values` come together'))
	);
});

test('the committed table reads and is in its own layout', async () => {
	const { readFileSync } = await import('node:fs');
	const text = readFileSync(
		new URL('../crates/tsv_wasm/types/tsv_ast_schema.json', import.meta.url),
		'utf8'
	);
	const read = parse_ast_schema(text);
	ok(!('errors' in read), 'errors' in read ? read.errors.join('\n') : '');
	strictEqual(layout_ast_schema(read), text);
});
