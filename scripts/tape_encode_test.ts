import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { Worker } from 'node:worker_threads';
import { assert, test } from 'vitest';

import { decode_tape, decode_tape_iterative } from '../crates/tsv_wasm/npm/tape_decode.js';
import { parse_ast_schema } from './ast_schema.ts';
import { wire_difference } from './check_tape.ts';
import { encode_tape } from './tape_encode.ts';
import {
	build_tape_schema,
	parse_tape_shapes,
	TAPE_HEADER_WORDS,
	type TapeSchema
} from './tape_schema.ts';

const root = (path: string): string => fileURLToPath(new URL(`../${path}`, import.meta.url));

function committed_schema(): TapeSchema {
	const table = parse_ast_schema(
		readFileSync(root('crates/tsv_wasm/types/tsv_ast_schema.json'), 'utf8')
	);
	if ('errors' in table) throw new Error(table.errors.join('\n'));
	const shapes = parse_tape_shapes(
		readFileSync(root('crates/tsv_wasm/types/tsv_tape_shapes.json'), 'utf8')
	);
	if ('errors' in shapes) throw new Error(shapes.errors.join('\n'));
	const schema = build_tape_schema(table, shapes);
	if ('errors' in schema) throw new Error(schema.errors.join('\n'));
	return schema;
}

const schema = committed_schema();

/** The tape's body words, signed. */
const body = (bytes: Uint8Array): number[] => {
	const view = new DataView(bytes.buffer, bytes.byteOffset);
	const words = view.getUint32(12, true);
	return Array.from({ length: words }, (_, k) => view.getInt32((TAPE_HEADER_WORDS + k) * 4, true));
};

/** Encodes, then holds every decoder to the tree; returns the encode. */
function round_trip(tree: unknown, source: string): ReturnType<typeof encode_tape> {
	const encoded = encode_tape(tree, source, schema);
	assert.strictEqual(wire_difference(decode_tape(encoded.bytes, source), tree), null);
	assert.strictEqual(wire_difference(decode_tape_iterative(encoded.bytes, source), tree), null);
	const generic = encode_tape(tree, source, schema, { no_fast_shapes: true });
	assert.strictEqual(wire_difference(decode_tape(generic.bytes, source), tree), null);
	return encoded;
}

const identifier = (start: number, name: string): Record<string, unknown> => ({
	type: 'Identifier',
	start,
	end: start + name.length,
	name
});

const program = (source: string, expression: unknown): unknown => ({
	type: 'Program',
	start: 0,
	end: source.length,
	body: [{ type: 'ExpressionStatement', start: 0, end: source.length, expression }],
	sourceType: 'module'
});

test('a wire tree writes as fast shapes and reads back in key order', () => {
	const source = 'abc';
	const encoded = round_trip(program(source, identifier(0, 'abc')), source);
	assert.deepEqual(encoded.stats, { fast: 3, generic: 0, escape: 0 });
});

test('a string found in its scope is a source slice, one that is not is pooled', () => {
	// the cooked value of an escape the source does not hold is pooled
	const source = '`\\x41`';
	const tree = program(source, {
		type: 'TemplateLiteral',
		start: 0,
		end: 6,
		expressions: [],
		quasis: [
			{
				type: 'TemplateElement',
				start: 1,
				end: 5,
				value: { raw: '\\x41', cooked: 'A' },
				tail: true
			}
		]
	});
	const words = body(round_trip(tree, source).bytes);
	// raw sliced at [1, 5), cooked the pool's first entry: [-1 - 0, 1)
	const at = words.findIndex((w, k) => w === 1 && words[k + 1] === 5 && words[k + 2] === -1);
	assert.notStrictEqual(at, -1);
	assert.strictEqual(words[at + 3], 1);
});

test('a nullable string field carries null as the null ref', () => {
	const source = 'x`\\u`';
	const tree = program(source, {
		type: 'TaggedTemplateExpression',
		start: 0,
		end: 5,
		tag: identifier(0, 'x'),
		quasi: {
			type: 'TemplateLiteral',
			start: 1,
			end: 5,
			expressions: [],
			quasis: [
				{
					type: 'TemplateElement',
					start: 2,
					end: 4,
					value: { raw: '\\u', cooked: null },
					tail: true
				}
			]
		}
	});
	assert.include(body(encode_tape(tree, source, schema).bytes), -0x80000000);
	round_trip(tree, source);
});

test('numbers keep their bits, a non-integral one and the extremes included', () => {
	for (const value of [1.5, -0.25, 1e308, 5e-324, 0.1 + 0.2, 2 ** 53 + 2]) {
		const raw = String(value);
		const tree = program(raw, { type: 'Literal', start: 0, end: raw.length, value, raw });
		assert.strictEqual(round_trip(tree, raw).stats.escape, 0);
	}
});

test('an object no entry holds is self-describing, a __proto__ key included', () => {
	const tree = JSON.parse(
		'{"type":"Program","start":0,"end":1,"body":[{"__proto__":{"x":[1,"y",null,true]},"0":false}],"sourceType":"module"}'
	) as unknown;
	const encoded = round_trip(tree, 'y');
	assert.strictEqual(encoded.stats.escape, 2);
	const decoded = decode_tape(encoded.bytes, 'y') as { body: Array<Record<string, unknown>> };
	assert.ok(Object.hasOwn(decoded.body[0]!, '__proto__'));
	assert.strictEqual(Object.getPrototypeOf(decoded.body[0]), Object.prototype);
});

test('a value its entry refuses writes the object self-describing', () => {
	// an own raw that is not the source between its offsets, and a keyword out of its vocabulary
	const source = '1 + 2';
	for (const expression of [
		{ type: 'Literal', start: 0, end: 1, value: 1, raw: '2' },
		{
			type: 'BinaryExpression',
			start: 0,
			end: 5,
			left: identifier(0, '1'),
			operator: '<=>',
			right: identifier(4, '2')
		}
	]) {
		assert.ok(round_trip(program(source, expression), source).stats.escape >= 1);
	}
});

test('a mask outside the shape list writes the entry-generic record', () => {
	const source = 'a';
	const tree = program(source, {
		...identifier(0, 'a'),
		optional: true,
		decorators: [],
		leadingComments: [],
		trailingComments: []
	});
	assert.strictEqual(round_trip(tree, source).stats.generic, 1);
});

test('the encoder and the tree comparison need no stack for a deep tree', () => {
	const depth = 20_000;
	const source = Array(depth + 1)
		.fill('a')
		.join('+');
	let node: Record<string, unknown> = identifier(0, 'a');
	for (let k = 1; k <= depth; k++) {
		node = {
			type: 'BinaryExpression',
			start: 0,
			end: 2 * k + 1,
			left: node,
			operator: '+',
			right: identifier(2 * k, 'a')
		};
	}
	const tree = program(source, node);
	const encoded = encode_tape(tree, source, schema);
	assert.strictEqual(encoded.stats.escape + encoded.stats.generic, 0);
	assert.strictEqual(wire_difference(decode_tape_iterative(encoded.bytes, source), tree), null);
});

test('a string holding a lone surrogate cannot be written', () => {
	const tree = program('a', { ...identifier(0, 'a'), name: '\uD800' });
	assert.throws(() => encode_tape(tree, 'a', schema), /lone surrogate/);
});

test('a tape decoded against another source is refused, a BOM the wire elides included', () => {
	// a Svelte or CSS wire's offsets index the source less its BOM; handed the BOM, the decoder
	// would read every slice one unit early
	const source = '<p>a</p>';
	const encoded = encode_tape(program(source, identifier(3, 'a')), source, schema).bytes;
	assert.throws(() => decode_tape(encoded, `\uFEFF${source}`), /another source/);
	assert.throws(() => decode_tape_iterative(encoded, `\uFEFF${source}`), /another source/);
	assert.doesNotThrow(() => decode_tape(encoded, source));
});

test('a tape from another schema is refused', () => {
	const copy = encode_tape(program('a', identifier(0, 'a')), 'a', schema).bytes.slice();
	copy[8] = (copy[8]! + 1) & 0xff;
	assert.throws(() => decode_tape(copy, 'a'), /SHAPES_HASH/);
});

/** Runs `code` in a worker with a stack of `stack_mb` and resolves its one message. */
function in_worker<T>(code: string, data: unknown, stack_mb: number): Promise<T> {
	return new Promise((resolve, reject) => {
		const worker = new Worker(code, {
			eval: true,
			workerData: data,
			resourceLimits: { stackSizeMb: stack_mb }
		});
		worker.once('message', (message: T) => {
			void worker.terminate();
			resolve(message);
		});
		worker.once('error', reject);
	});
}

/** The deepest wire the parsers accept: a left-deep chain of the spine limit's binary operators. */
const DEPTH = 5520;

/** Builds the chain's tree and tape on a large stack (the encoder recurses). */
const BUILD = `
const { workerData, parentPort } = require('node:worker_threads');
(async () => {
	const { encode_tape } = await import(workerData.encoder);
	const { build_tape_schema } = await import(workerData.tape_schema);
	const { parse_ast_schema } = await import(workerData.ast_schema);
	const schema = build_tape_schema(parse_ast_schema(workerData.table), workerData.shapes);
	const source = Array(workerData.depth + 1).fill('a').join('+');
	let node = { type: 'Identifier', start: 0, end: 1, name: 'a' };
	for (let k = 1; k <= workerData.depth; k++) {
		const right = { type: 'Identifier', start: 2 * k, end: 2 * k + 1, name: 'a' };
		node = { type: 'BinaryExpression', start: 0, end: 2 * k + 1, left: node, operator: '+', right };
	}
	const statement = { type: 'ExpressionStatement', start: 0, end: source.length, expression: node };
	const tree = { type: 'Program', start: 0, end: source.length, body: [statement], sourceType: 'module' };
	const { bytes, stats } = encode_tape(tree, source, schema);
	const small_tree = { type: 'Program', start: 0, end: 1, body: [], sourceType: 'module' };
	parentPort.postMessage({
		tape: bytes,
		source,
		all_fast: stats.generic + stats.escape === 0,
		small: encode_tape(small_tree, 'a', schema).bytes,
		small_expected: JSON.stringify(small_tree)
	});
})();
`;

/**
 * Decodes the chain on a stack too small for the recursive pass, against the slow path's result,
 * then decodes again: the proof that a `RangeError` out of the fast path leaves nothing behind.
 */
const DECODE = `
const { workerData, parentPort } = require('node:worker_threads');
(async () => {
	const { decode_tape, decode_tape_iterative } = await import(workerData.decoder);
	// one frame per level overflows here, so the fast pass (two per level) cannot finish
	const recurse = (n) => (n === 0 ? 0 : 1 + recurse(n - 1));
	let overflows = false;
	try {
		recurse(workerData.depth);
	} catch (error) {
		overflows = error instanceof RangeError;
	}
	// an ordered comparison with no recursion either
	const same = (a, b) => {
		const stack = [[a, b]];
		while (stack.length > 0) {
			const [x, y] = stack.pop();
			if (x === null || y === null || typeof x !== 'object' || typeof y !== 'object') {
				if (!Object.is(x, y)) return false;
				continue;
			}
			const kx = Object.keys(x);
			const ky = Object.keys(y);
			if (kx.length !== ky.length || kx.some((k, i) => k !== ky[i])) return false;
			for (const k of kx) stack.push([x[k], y[k]]);
		}
		return true;
	};
	const tape = new Uint8Array(workerData.tape);
	const slow = decode_tape_iterative(tape, workerData.source);
	const decoded = decode_tape(tape, workerData.source);
	const again = decode_tape(tape, workerData.source);
	const next = decode_tape(new Uint8Array(workerData.small), 'a');
	parentPort.postMessage({
		overflows,
		chain: slow.body[0].expression.left.left.operator === '+',
		equal: same(decoded, slow),
		again: same(again, slow),
		next: JSON.stringify(next) === workerData.small_expected
	});
})();
`;

test('a tree too deep for the recursive pass decodes by the slow path, and the next decode is exact', async () => {
	const at = (path: string): string => new URL(path, import.meta.url).href;
	const built = await in_worker<{
		tape: Uint8Array;
		source: string;
		all_fast: boolean;
		small: Uint8Array;
		small_expected: string;
	}>(
		BUILD,
		{
			encoder: at('./tape_encode.ts'),
			tape_schema: at('./tape_schema.ts'),
			ast_schema: at('./ast_schema.ts'),
			table: readFileSync(root('crates/tsv_wasm/types/tsv_ast_schema.json'), 'utf8'),
			shapes: JSON.parse(readFileSync(root('crates/tsv_wasm/types/tsv_tape_shapes.json'), 'utf8')),
			depth: DEPTH
		},
		256
	);
	assert.ok(
		built.all_fast,
		'the chain is written in fast shapes, so the fast pass is what overflows'
	);
	const outcome = await in_worker<Record<string, boolean>>(
		DECODE,
		{ ...built, decoder: at('../crates/tsv_wasm/npm/tape_decode.js'), depth: 2 * DEPTH },
		0.5
	);
	assert.deepEqual(outcome, { overflows: true, chain: true, equal: true, again: true, next: true });
}, 60_000);
