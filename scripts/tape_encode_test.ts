import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { Worker } from 'node:worker_threads';
import { assert, onTestFinished, test } from 'vitest';

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

/**
 * The decoder's module text with one line added — a counter of the decodes whose fast pass threw
 * a `RangeError` and fell back — written to a temporary file (its own import made absolute) for a
 * worker to import, so a test asserts the fast pass overflowed rather than inferring it from a
 * probe recursion. Not a `data:` URL: Node decodes one recursively, past a small worker stack.
 * @returns the file's URL
 */
function counting_decoder(): string {
	const text = readFileSync(root('crates/tsv_wasm/npm/tape_decode.js'), 'utf8');
	const core = new URL('../crates/tsv_wasm/npm/locations_core.js', import.meta.url).href;
	const fallback = '\ttree = slow();'; // the catch's, not the interpreter's `const tree`
	const import_core = "from './locations_core.js'";
	assert.ok(
		text.split(fallback).length === 2 && text.split(import_core).length === 2,
		'the decoder has one fallback and one core import to instrument'
	);
	const counted = text
		.replace(import_core, `from '${core}'`)
		.replace(fallback, `globalThis.fallbacks = (globalThis.fallbacks ?? 0) + 1; ${fallback}`);
	const dir = mkdtempSync(join(tmpdir(), 'tape-decode-'));
	onTestFinished(() => rmSync(dir, { recursive: true, force: true }));
	const path = join(dir, 'tape_decode.js');
	writeFileSync(path, counted);
	return pathToFileURL(path).href;
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
	const fallbacks = () => globalThis.fallbacks ?? 0;
	const slow = decode_tape_iterative(tape, workerData.source);
	const decoded = decode_tape(tape, workerData.source);
	const fast_overflows = fallbacks() === 1;
	const again = decode_tape(tape, workerData.source);
	const next = decode_tape(new Uint8Array(workerData.small), 'a');
	// loc mode overflows the same way: the slow path builds the same \`loc\`, in the same place
	const slow_loc = decode_tape_iterative(tape, workerData.source, 'typescript');
	const before_loc = fallbacks();
	const decoded_loc = decode_tape(tape, workerData.source, 'typescript');
	const loc_fast_overflows = fallbacks() === before_loc + 1;
	const again_loc = decode_tape(tape, workerData.source, 'typescript');
	const next_after_loc = decode_tape(new Uint8Array(workerData.small), 'a');
	const deep = slow_loc.body[0].expression.left.left;
	parentPort.postMessage({
		overflows,
		fast_overflows,
		loc_fast_overflows,
		chain: slow.body[0].expression.left.left.operator === '+',
		equal: same(decoded, slow),
		again: same(again, slow),
		next: JSON.stringify(next) === workerData.small_expected,
		loc_chain:
			Object.keys(deep).slice(0, 4).join() === 'type,start,end,loc' &&
			deep.loc.end.column === deep.end &&
			deep.right.loc.start.column === deep.right.start,
		loc_equal: same(decoded_loc, slow_loc),
		loc_again: same(again_loc, slow_loc),
		next_after_loc: JSON.stringify(next_after_loc) === workerData.small_expected
	});
})();
`;

test('a tree too deep for the recursive pass decodes by the slow path, loc mode too, and the next decode is exact', async () => {
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
		{ ...built, decoder: counting_decoder(), depth: 2 * DEPTH },
		0.5
	);
	assert.deepEqual(outcome, {
		overflows: true,
		fast_overflows: true,
		loc_fast_overflows: true,
		chain: true,
		equal: true,
		again: true,
		next: true,
		loc_chain: true,
		loc_equal: true,
		loc_again: true,
		next_after_loc: true
	});
}, 60_000);

/**
 * A Svelte document that reaches every loc-mode rule the decoder applies once the tree is whole —
 * a shorthand attribute, an in-tag comment, a snippet, a const tag, an each and an await block —
 * with the const's init a chain at the spine limit, built (offsets and all) as the parser writes
 * it and encoded on a large stack.
 */
const BUILD_SVELTE = `
const { workerData, parentPort } = require('node:worker_threads');
(async () => {
	const { encode_tape } = await import(workerData.encoder);
	const { build_tape_schema } = await import(workerData.tape_schema);
	const { parse_ast_schema } = await import(workerData.ast_schema);
	const schema = build_tape_schema(parse_ast_schema(workerData.table), workerData.shapes);
	const head = '<div {x} /* c */>{#snippet s(a)}{@const y = ';
	const chain = Array(workerData.depth + 1).fill('a').join('+');
	const tail = '}{/snippet}{#each xs as z}{z}{/each}{#await p then v}{v}{/await}</div>';
	const source = head + chain + tail;
	// every offset past the chain's first \`a\` moves by what the rest of the chain adds
	const p = head.length;
	const at = (n) => (n > p ? n + chain.length - 1 : n);
	const id = (start, name) => ({
		type: 'Identifier', start: at(start), end: at(start) + name.length, name
	});
	const tag = (start, end, name) => ({
		type: 'ExpressionTag', start: at(start), end: at(end), expression: id(start + 1, name)
	});
	let init = { type: 'Identifier', start: p, end: p + 1, name: 'a' };
	for (let k = 1; k <= workerData.depth; k++) {
		const right = { type: 'Identifier', start: p + 2 * k, end: p + 2 * k + 1, name: 'a' };
		init = { type: 'BinaryExpression', start: p, end: right.end, left: init, operator: '+', right };
	}
	const declarator = { type: 'VariableDeclarator', start: 40, end: at(45), id: id(40, 'y'), init };
	const declaration = {
		type: 'VariableDeclaration', start: 34, end: at(45), declarations: [declarator], kind: 'const'
	};
	const snippet = {
		type: 'SnippetBlock', start: 17, end: at(56), expression: id(27, 's'),
		parameters: [id(29, 'a')],
		body: { type: 'Fragment', nodes: [{ type: 'ConstTag', start: 32, end: at(46), declaration }] }
	};
	const each = {
		type: 'EachBlock', start: at(56), end: at(81), expression: id(63, 'xs'),
		body: { type: 'Fragment', nodes: [tag(71, 74, 'z')] }, context: id(69, 'z')
	};
	const wait = {
		type: 'AwaitBlock', start: at(81), end: at(109), expression: id(89, 'p'), value: id(96, 'v'),
		error: null, pending: null, then: { type: 'Fragment', nodes: [tag(98, 101, 'v')] }, catch: null
	};
	const element = {
		type: 'RegularElement', start: 0, end: source.length, name: 'div',
		attributes: [{
			type: 'Attribute', start: 5, end: 8, name: 'x',
			value: { type: 'ExpressionTag', start: 6, end: 7, expression: id(6, 'x') }
		}],
		fragment: { type: 'Fragment', nodes: [snippet, each, wait] }
	};
	const tree = {
		css: null, js: [], start: 0, end: source.length, type: 'Root',
		fragment: { type: 'Fragment', nodes: [element] }, options: null,
		comments: [{ type: 'Block', value: ' c ', start: 9, end: 16 }]
	};
	const { bytes, stats } = encode_tape(tree, source, schema);
	parentPort.postMessage({ tape: bytes, source, all_fast: stats.generic + stats.escape === 0 });
})();
`;

/**
 * Decodes the Svelte document in loc mode on a stack too small for the fast pass: the fallback
 * must build what the interpreter alone builds, in its key order, and what `reconstruct_locations`
 * adds to the span-only decode, in any key order.
 */
const DECODE_SVELTE = `
const { workerData, parentPort } = require('node:worker_threads');
(async () => {
	const { decode_tape, decode_tape_iterative } = await import(workerData.decoder);
	const { reconstruct_locations } = await import(workerData.locations);
	// a comparison with no recursion, keys in order or as sets
	const same = (a, b, ordered) => {
		const stack = [[a, b]];
		while (stack.length > 0) {
			const [x, y] = stack.pop();
			if (x === null || y === null || typeof x !== 'object' || typeof y !== 'object') {
				if (!Object.is(x, y)) return false;
				continue;
			}
			const kx = Object.keys(x);
			const ky = Object.keys(y);
			if (kx.length !== ky.length) return false;
			const keys_differ = ordered
				? kx.some((k, i) => k !== ky[i])
				: kx.some((k) => !Object.hasOwn(y, k));
			if (keys_differ) return false;
			for (const k of kx) stack.push([x[k], y[k]]);
		}
		return true;
	};
	const tape = new Uint8Array(workerData.tape);
	const source = workerData.source;
	const slow_loc = decode_tape_iterative(tape, source, 'svelte');
	const decoded_loc = decode_tape(tape, source, 'svelte');
	const loc_fast_overflows = (globalThis.fallbacks ?? 0) === 1;
	const again_loc = decode_tape(tape, source, 'svelte');
	const reconstructed = reconstruct_locations(decode_tape_iterative(tape, source), source, {
		language: 'svelte'
	});
	const element = decoded_loc.fragment.nodes[0];
	const shorthand = element.attributes[0].value.expression;
	const snippet = element.fragment.nodes[0];
	parentPort.postMessage({
		loc_fast_overflows,
		loc_equal: same(decoded_loc, slow_loc, true),
		loc_again: same(again_loc, slow_loc, true),
		reconstructed: same(decoded_loc, reconstructed, false),
		key_order:
			Object.keys(element).join() === 'type,start,end,loc,name,name_loc,attributes,fragment',
		stamped:
			shorthand.loc.start.character === shorthand.start &&
			snippet.expression.loc.start.character === snippet.expression.start &&
			decoded_loc.comments[0].loc.start.character === decoded_loc.comments[0].start
	});
})();
`;

test('a Svelte tree too deep for the loc-mode fast pass decodes by the slow path with its stamps', async () => {
	const at = (path: string): string => new URL(path, import.meta.url).href;
	const built = await in_worker<{ tape: Uint8Array; source: string; all_fast: boolean }>(
		BUILD_SVELTE,
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
	assert.ok(built.all_fast, 'the document is written in fast shapes, so the fast pass overflows');
	const outcome = await in_worker<Record<string, boolean>>(
		DECODE_SVELTE,
		{
			...built,
			decoder: counting_decoder(),
			locations: at('../crates/tsv_wasm/npm/locations.js')
		},
		0.5
	);
	assert.deepEqual(outcome, {
		loc_fast_overflows: true,
		loc_equal: true,
		loc_again: true,
		reconstructed: true,
		key_order: true,
		stamped: true
	});
}, 60_000);
