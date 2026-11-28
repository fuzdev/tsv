/**
 * `wasm_simd_census` over handcrafted modules, each checked valid by the engine first so the
 * reader is graded against real framing: bodies with and without a `v128` local, a custom
 * section, a multi-byte LEB128, a module without code, and the malformed inputs it must refuse.
 */

import { deepStrictEqual, strictEqual, throws } from 'node:assert';

import { test } from 'vitest';

import { wasm_simd_census } from './wasm_simd.ts';

const HEADER = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
/** One type, `() -> ()`. */
const TYPES = [0x01, 0x60, 0x00, 0x00];

/** A section: its id, its size as a one-byte LEB128 (every section here is short), its bytes. */
const section = (id: number, content: Array<number>): Array<number> => {
	if (content.length > 0x7f) throw Error('test sections are short');
	return [id, content.length, ...content];
};

/** A body declaring one local of each valtype in `locals`, then `end`. */
const body = (locals: Array<number>): Array<number> => {
	const content = [locals.length, ...locals.flatMap((valtype) => [0x01, valtype]), 0x0b];
	return [content.length, ...content];
};

/** A module of one function per body, all of type 0, with `extra` sections ahead of the code. */
const module = (bodies: Array<Array<number>>, extra: Array<number> = []): Uint8Array<ArrayBuffer> =>
	new Uint8Array([
		...HEADER,
		...section(1, TYPES),
		...section(3, [bodies.length, ...bodies.map(() => 0x00)]),
		...extra,
		...section(10, [bodies.length, ...bodies.flat()])
	]);

const I32 = 0x7f;
const V128 = 0x7b;

const valid = (bytes: Uint8Array<ArrayBuffer>): Uint8Array<ArrayBuffer> => {
	strictEqual(WebAssembly.validate(bytes), true, 'the handcrafted module must be valid');
	return bytes;
};

test('a body declaring a v128 local is counted, one without is not', () => {
	deepStrictEqual(wasm_simd_census(valid(module([body([I32]), body([I32, V128])]))), {
		functions: 2,
		v128_functions: 1
	});
	deepStrictEqual(wasm_simd_census(valid(module([body([I32]), body([])]))), {
		functions: 2,
		v128_functions: 0
	});
});

test('a custom section is skipped, and a multi-byte LEB128 count reads whole', () => {
	const custom = section(0, [0x04, ...new TextEncoder().encode('name'), 0xff, 0x7b]);
	// 200 v128 locals in one declaration: the count is `c8 01`
	const wide = [0x05, 0x01, 0xc8, 0x01, V128, 0x0b];
	deepStrictEqual(wasm_simd_census(valid(module([body([I32]), wide], custom))), {
		functions: 2,
		v128_functions: 1
	});
});

test('a module without a code section has no functions', () => {
	deepStrictEqual(wasm_simd_census(valid(new Uint8Array([...HEADER, ...section(1, TYPES)]))), {
		functions: 0,
		v128_functions: 0
	});
});

test('malformed input throws rather than reading as no SIMD', () => {
	throws(() => wasm_simd_census(new TextEncoder().encode('wasm')), /not a version-1/);
	const good = module([body([V128])]);
	throws(() => wasm_simd_census(good.subarray(0, good.length - 1)), /past the end/);
	// a body whose size claims more than its section holds
	const long = new Uint8Array([...HEADER, ...section(10, [0x01, 0x09, 0x00, 0x0b])]);
	throws(() => wasm_simd_census(long), /runs past its section/);
	// a LEB128 that never ends inside the module
	throws(() => wasm_simd_census(new Uint8Array([...HEADER, 0x0a, 0x80])), /unexpected end/);
	const overflow = new Uint8Array([...HEADER, 0x0a, 0xff, 0xff, 0xff, 0xff, 0x1f]);
	throws(() => wasm_simd_census(overflow), /overflows/);
	// a code section whose count ends before its bytes do
	const trailing = new Uint8Array([...HEADER, ...section(10, [0x00, 0x00])]);
	throws(() => wasm_simd_census(trailing), /trailing bytes/);
});
