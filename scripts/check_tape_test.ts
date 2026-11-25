import { assert, test } from 'vitest';

import { tape_difference, wire_difference, wire_source } from './check_tape.ts';

test('wire_difference holds values, key order and -0 apart', () => {
	assert.strictEqual(wire_difference({ a: [1, { b: null }] }, { a: [1, { b: null }] }), null);
	assert.match(wire_difference({ a: 1, b: 2 }, { b: 2, a: 1 })!, /keys \[a, b\] vs \[b, a\]/);
	assert.match(wire_difference({ a: [1, 2] }, { a: [1, 3] })!, /^\$\.a\[1\]: 2 vs 3$/);
	assert.match(wire_difference([0], [-0])!, /^\$\[0\]/);
	assert.match(wire_difference([], {})!, /an array vs an object/);
});

test('tape_difference names the first differing word', () => {
	const a = new Uint8Array([1, 0, 0, 0, 2, 0, 0, 0]);
	assert.strictEqual(tape_difference(a, a.slice()), null);
	const b = new Uint8Array([1, 0, 0, 0, 3, 0, 0, 0]);
	assert.strictEqual(tape_difference(a, b), 'byte 4 (word 1): 2 vs 3, lengths 8 vs 8');
	assert.match(tape_difference(a, a.subarray(0, 4))!, /word 1\): 2 vs end/);
});

test('wire_source elides a leading BOM where the wire does', () => {
	assert.strictEqual(wire_source('svelte', '﻿a'), 'a');
	assert.strictEqual(wire_source('css', '﻿a'), 'a');
	assert.strictEqual(wire_source('typescript', '﻿a'), '﻿a');
});
