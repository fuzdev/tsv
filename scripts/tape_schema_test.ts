import { assert, test } from 'vitest';

import type { AstSchema, AstSchemaEntry } from './ast_schema.ts';
import {
	build_tape_schema,
	derive_tape_entries,
	Enc,
	field_encoding,
	generic_header,
	hot_file_from_counts,
	hot_shape_ids,
	layout_tape_hot,
	layout_tape_shapes,
	parse_tape_hot,
	parse_tape_shapes,
	read_generic_header,
	shapes_file_from_census,
	tape_hot_cut_changes,
	TAPE_MASK_BITS,
	TAPE_MAX_ENTRIES
} from './tape_schema.ts';

/** A table of two entries of one type (a base and a variant) and a type-less one. */
const TABLE: AstSchema = {
	doc: [],
	sections: [
		{
			title: 'test',
			entries: [
				{
					type: 'Lit',
					fields: [
						{ key: 'type', kind: 'type' },
						{ key: 'start', kind: 'offset' },
						{ key: 'end', kind: 'offset' },
						{ key: 'loc', kind: 'loc', entry: 'Loc', presence: 'locations' },
						{ key: 'raw', kind: 'string', string: 'own' },
						{ key: 'extra', kind: 'object', entry: 'Extra', presence: 'optional' }
					]
				},
				{
					type: 'Lit',
					variant: 'Built',
					fields: [
						{ key: 'type', kind: 'type' },
						{ key: 'raw', kind: 'string', string: 'pooled' }
					]
				},
				{
					type: 'Lit',
					variant: 'Twin',
					fields: [
						{ key: 'type', kind: 'type' },
						{ key: 'raw', kind: 'string', string: 'slice' }
					]
				},
				{ interface: 'Extra', fields: [{ key: 'n', kind: 'number', nullable: true }] },
				{ interface: 'Loc', fields: [{ key: 'line', kind: 'number' }] }
			]
		}
	]
};

test('each kind takes its encoding, nullability and class deciding', () => {
	assert.strictEqual(field_encoding({ key: 'a', kind: 'offset' }), Enc.offset);
	assert.strictEqual(field_encoding({ key: 'a', kind: 'offset', nullable: true }), Enc.value);
	assert.strictEqual(field_encoding({ key: 'a', kind: 'string', string: 'own' }), Enc.own);
	assert.strictEqual(
		field_encoding({ key: 'a', kind: 'string', string: 'cooked', nullable: true }),
		Enc.ref_nullable
	);
	assert.strictEqual(field_encoding({ key: 'a', kind: ['boolean', 'string'] }), Enc.value);
	assert.strictEqual(field_encoding({ key: 'a', kind: 'comments' }), Enc.list);
	assert.strictEqual(field_encoding({ key: 'a', kind: 'loc', presence: 'locations' }), null);
});

test('entries drop loc fields, fold identical twins, and resolve type-less positions', () => {
	const derived = derive_tape_entries(TABLE);
	if ('errors' in derived) throw new Error(derived.errors.join('\n'));
	// Built and Twin encode alike (both refs) and fold; the two type-less entries stay apart
	assert.deepEqual(
		derived.entries.map((e) => e.name),
		['Lit', 'Lit/Built', '~Extra', '~Loc']
	);
	assert.deepEqual(derived.by_type.get('Lit'), [0, 1]);
	const lit = derived.entries[0]!;
	assert.deepEqual(
		lit.fields.map((f) => [f.key, f.enc, f.bit]),
		[
			['type', Enc.type, -1],
			['start', Enc.offset, -1],
			['end', Enc.offset, -1],
			['raw', Enc.own, -1],
			['extra', Enc.record, 0]
		]
	);
	assert.strictEqual(lit.fields[4]!.entry, 2);
	// the loc wire's fields ride the entry as flags, outside the tape
	assert.deepEqual(
		derived.entries.map((e) => [e.loc, e.name_loc]),
		[
			[true, false],
			[false, false],
			[false, false],
			[false, false]
		]
	);
});

/** A one-entry table of `fields`. */
const one_entry = (fields: AstSchemaEntry['fields']): AstSchema =>
	({
		doc: [],
		sections: [{ title: 'test', entries: [{ type: 'El', fields }] }]
	}) as AstSchema;

test('the loc-wire fields sit where the decoder builds them, or the table is refused', () => {
	const loc = { key: 'loc', kind: 'loc', entry: 'Loc', presence: 'locations' } as const;
	const name_loc = { key: 'name_loc', kind: 'loc', entry: 'Loc', presence: 'locations' } as const;
	const head = [
		{ key: 'type', kind: 'type' },
		{ key: 'start', kind: 'offset' },
		{ key: 'end', kind: 'offset' }
	] as const;
	const name = { key: 'name', kind: 'string', string: 'slice' } as const;
	const ok = derive_tape_entries(one_entry([...head, loc, name, name_loc]));
	if ('errors' in ok) throw new Error(ok.errors.join('\n'));
	assert.deepEqual([ok.entries[0]!.loc, ok.entries[0]!.name_loc], [true, true]);
	const refusals: Array<[AstSchema, RegExp]> = [
		[one_entry([...head, name, loc]), /El\.loc: a loc-wire field/],
		[one_entry([...head, loc, name, { key: 'x', kind: 'boolean' }, name_loc]), /El\.name_loc/],
		[one_entry([...head]), /El: no `loc` despite/],
		[
			one_entry([{ key: 'type', kind: 'type' }, { key: 'end', kind: 'offset' }, loc]),
			/El: a `loc` without/
		],
		[one_entry([...head, loc, { key: 'other', kind: 'loc', presence: 'locations' }]), /El\.other/]
	];
	for (const [table, pattern] of refusals) {
		const derived = derive_tape_entries(table);
		assert.ok('errors' in derived && derived.errors.some((e) => pattern.test(e)), String(pattern));
	}
});

test('twins that fold must agree on their loc-wire fields', () => {
	const derived = derive_tape_entries({
		doc: [],
		sections: [
			{
				title: 'test',
				entries: [
					{
						type: 'El',
						fields: [
							{ key: 'type', kind: 'type' },
							{ key: 'name', kind: 'string', string: 'slice' }
						]
					},
					{
						type: 'El',
						variant: 'Named',
						fields: [
							{ key: 'type', kind: 'type' },
							{ key: 'name', kind: 'string', string: 'pooled' },
							{ key: 'name_loc', kind: 'loc', entry: 'Loc', presence: 'locations' }
						]
					}
				]
			}
		]
	} as AstSchema);
	assert.ok('errors' in derived && derived.errors.some((e) => /El\/Named: folds into El/.test(e)));
});

test('an own field ahead of its span is carried as a ref', () => {
	const derived = derive_tape_entries({
		doc: [],
		sections: [
			{
				title: 'test',
				entries: [
					{
						type: 'Sel',
						fields: [
							{ key: 'type', kind: 'type' },
							{ key: 'name', kind: 'string', string: 'own' },
							{ key: 'start', kind: 'offset' },
							{ key: 'end', kind: 'offset' },
							{ key: 'loc', kind: 'loc', entry: 'Loc', presence: 'locations' }
						]
					}
				]
			}
		]
	});
	if ('errors' in derived) throw new Error(derived.errors.join('\n'));
	assert.strictEqual(derived.entries[0]!.fields[1]!.enc, Enc.ref);
});

test('the fast shapes are every all-required mask plus the listed ones, numbered from one', () => {
	const schema = build_tape_schema(TABLE, { note: [], shapes: [['Lit', 1]] });
	if ('errors' in schema) throw new Error(schema.errors.join('\n'));
	assert.deepEqual(
		schema.shapes.map((s) => [s.id, s.entry, s.mask]),
		[
			[1, 0, 0],
			[2, 0, 1],
			[3, 1, 0],
			[4, 2, 0],
			[5, 3, 0]
		]
	);
	const refused = build_tape_schema(TABLE, {
		note: [],
		shapes: [
			['Lit', 2],
			['Nope', 0]
		]
	});
	assert.ok('errors' in refused && refused.errors.length === 2);
});

test('an entry-generic header round-trips its entry and mask', () => {
	for (const [entry, mask] of [
		[0, 0],
		[5, 3],
		[TAPE_MAX_ENTRIES - 1, 2 ** TAPE_MASK_BITS - 1]
	] as const) {
		const header = generic_header(entry, mask);
		assert.ok(header <= -2 && header >= -(2 ** 31));
		assert.deepEqual(read_generic_header(header), { entry, mask });
	}
});

test('the shape list lays out one shape per line and reads back', () => {
	const derived = derive_tape_entries(TABLE);
	if ('errors' in derived) throw new Error(derived.errors.join('\n'));
	const file = shapes_file_from_census(derived.entries, [
		[0, 1],
		[0, 0],
		[0, 1]
	]);
	assert.deepEqual(file.shapes, [['Lit', 1]]);
	const text = layout_tape_shapes(file);
	assert.deepEqual(parse_tape_shapes(text), file);
	assert.ok('errors' in parse_tape_shapes('{"note": [], "shapes": [["Lit"]]}'));
});

test('the hot-shape list lays out one shape per line, reads back, and names fast shapes only', () => {
	const schema = build_tape_schema(TABLE, { note: [], shapes: [['Lit', 1]] });
	if ('errors' in schema) throw new Error(schema.errors.join('\n'));
	const counts = {
		typescript: new Map([
			[1, 5],
			[2, 9],
			[3, 0]
		]),
		svelte: new Map([[1, 1]]),
		css: new Map<number, number>()
	};
	const file = hot_file_from_counts(schema, counts);
	// hottest first; a shape no record took is no hot shape
	assert.deepEqual(file.typescript, [
		['Lit', 1],
		['Lit', 0]
	]);
	assert.deepEqual(file.svelte, [['Lit', 0]]);
	assert.deepEqual(file.css, []);
	const text = layout_tape_hot(file);
	assert.deepEqual(parse_tape_hot(text), file);
	assert.ok('errors' in parse_tape_hot('{"note": [], "typescript": []}'));
	const ids = hot_shape_ids(schema, file);
	assert.ok(!('errors' in ids) && [...ids].sort().join() === '1,2');
	const stale = hot_shape_ids(schema, { ...file, css: [['Lit', 2]] });
	assert.ok('errors' in stale && /css names Lit mask 2/.test(stale.errors[0]!));
});

test('the hot-list staleness check compares each cut as a set and names what crossed it', () => {
	const file = (typescript: Array<[string, number]>, css: Array<[string, number]> = []) => ({
		note: [],
		typescript,
		svelte: [] as Array<[string, number]>,
		css
	});
	const committed = file([
		['A', 0],
		['B', 1]
	]);
	// a rank swap inside the cut is no change
	assert.deepEqual(
		tape_hot_cut_changes(
			committed,
			file([
				['B', 1],
				['A', 0]
			])
		),
		[]
	);
	// a shape crossing a cut names the language and both directions
	assert.deepEqual(
		tape_hot_cut_changes(
			committed,
			file(
				[
					['A', 0],
					['C', 2]
				],
				[['D', 0]]
			)
		),
		[
			'typescript: entered the cut: C mask 2',
			'typescript: left the cut: B mask 1',
			'css: entered the cut: D mask 0'
		]
	);
});
