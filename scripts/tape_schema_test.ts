import { assert, test } from 'vitest';

import type { AstSchema } from './ast_schema.ts';
import {
	build_tape_schema,
	derive_tape_entries,
	Enc,
	field_encoding,
	generic_header,
	layout_tape_shapes,
	parse_tape_shapes,
	read_generic_header,
	shapes_file_from_census,
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
						{ key: 'raw', kind: 'string', string: 'own' },
						{ key: 'loc', kind: 'loc', entry: 'Loc', presence: 'locations' },
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
							{ key: 'end', kind: 'offset' }
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
