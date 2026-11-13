/**
 * The WASM-memory probe over fake instances whose linear memory grows by the size of what each
 * call is given: the warm high-water over the whole list, the cold reading per language's
 * largest file on a fresh instance, a throw counted and named, and the arguments it refuses.
 */

import { deepStrictEqual, strictEqual, throws } from 'node:assert';

import { test } from 'vitest';

import {
	measure_wasm_memory,
	parse_wasm_memory_args,
	type WasmInstance,
	type WasmMemoryDeps
} from './wasm_memory.ts';

/** Instances whose memory grows to the largest source any call sees, from a 1000-byte floor;
 * `throws_on` makes the call throw. */
const fake_deps = (
	files: Readonly<Record<string, string>>,
	throws_on: ReadonlySet<string> = new Set()
): WasmMemoryDeps & { instances: Array<Array<string>> } => {
	const instances: Array<Array<string>> = [];
	return {
		instances,
		read_text: async (path) => {
			if (path === 'list.txt') return `${Object.keys(files).join('\n')}\n\n`;
			return files[path]!;
		},
		instantiate: async (): Promise<WasmInstance> => {
			const seen: Array<string> = [];
			instances.push(seen);
			const memory = { buffer: { byteLength: 1000 } };
			const call = (name: string) => (source: string) => {
				seen.push(name);
				if (throws_on.has(source)) throw new Error(`refused ${source.length}`);
				memory.buffer.byteLength = Math.max(memory.buffer.byteLength, 1000 + source.length);
			};
			const glue = Object.fromEntries(
				[
					'format_typescript',
					'format_svelte',
					'format_css',
					'parse_typescript_json',
					'parse_svelte_json',
					'parse_css_json'
				].map((name) => [name, call(name)])
			);
			return { memory, glue };
		}
	};
};

const FILES = {
	'a.ts': 'x'.repeat(50),
	'b.ts': 'x'.repeat(300),
	'c.svelte': 'x'.repeat(20),
	'd.css': 'x'.repeat(7)
};

test('the warm high-water spans the list; each cold reading is a fresh instance', async () => {
	const deps = fake_deps(FILES);
	const result = await measure_wasm_memory(deps, 'list.txt', 'format');
	deepStrictEqual(result, {
		surface: 'format',
		files: 4,
		errors: 0,
		first_error: null,
		warm_high_water_bytes: 1300,
		cold_largest_bytes: { typescript: 1300, svelte: 1020, css: 1007 }
	});
	deepStrictEqual(deps.instances, [
		['format_typescript', 'format_typescript', 'format_svelte', 'format_css'],
		['format_typescript'],
		['format_svelte'],
		['format_css']
	]);
	const parsed = fake_deps(FILES);
	await measure_wasm_memory(parsed, 'list.txt', 'parse');
	deepStrictEqual(parsed.instances[0], [
		'parse_typescript_json',
		'parse_typescript_json',
		'parse_svelte_json',
		'parse_css_json'
	]);
});

test('a throw is counted and the first one named, the sweep going on', async () => {
	const result = await measure_wasm_memory(
		fake_deps(FILES, new Set([FILES['a.ts']])),
		'list.txt',
		'format'
	);
	strictEqual(result.errors, 1);
	strictEqual(result.first_error, 'a.ts: refused 50');
	strictEqual(result.warm_high_water_bytes, 1300);
});

test('the probe takes exactly a glue, a list and a surface', () => {
	deepStrictEqual(parse_wasm_memory_args(['g.js', 'l.txt', 'parse']), {
		glue: 'g.js',
		list: 'l.txt',
		surface: 'parse'
	});
	for (const args of [
		[],
		['g.js', 'l.txt'],
		['g.js', 'l.txt', 'lint'],
		['g.js', 'l.txt', 'format', 'x']
	]) {
		throws(() => parse_wasm_memory_args(args), /usage: wasm_memory\.ts/);
	}
});
