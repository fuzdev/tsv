/**
 * `run_wasm_format_probe` and `run_wasm_memory_probe` over fake glue builds and a fake corpus:
 * the byte-identity gate a `--baseline` must pass, the memory report each mode builds from the
 * instance's linear memory, and a load that yields no memory. Package-free, so it gates in
 * `npm run test:node`.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { afterEach, beforeEach, test, vi } from 'vitest';

import type { Language, SourceFile } from '../lib/types.ts';
import { run_wasm_format_probe, type WasmGlueExports } from './wasm_format_probe.ts';
import { type FreshGlue, run_wasm_memory_probe } from './wasm_memory_probe.ts';

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

const FILES: SourceFile[] = (['a.css', 'b.svelte', 'c.ts'] as const).map(
	(path, i) =>
		({
			path,
			language: (['css', 'svelte', 'typescript'] as Language[])[i],
			content: 'x'.repeat(i + 1),
			bytes: i + 1
		}) as SourceFile
);

/** A glue build whose formatters append `suffix`, growing `memory` by a page per call. */
const glue = (suffix: string, memory?: WebAssembly.Memory): WasmGlueExports => {
	const format = (source: string) => {
		memory?.grow(1);
		return source + suffix;
	};
	return { format_css: format, format_svelte: format, format_typescript: format };
};

test('the format probe aborts when a baseline formats differently', async () => {
	const loaded: string[] = [];
	const code = await run_wasm_format_probe(
		{
			load_glue: async (path) => {
				loaded.push(path);
				return glue(path === 'base.js' ? '!' : '');
			},
			load_files: async () => FILES
		},
		['corpus', '--baseline', 'base.js', '--pairs', '1', '--warmup', '0']
	);
	strictEqual(code, 1);
	deepStrictEqual(loaded, ['crates/tsv_wasm/pkg/all/nodejs/tsv_wasm.js', 'base.js']);
	ok(err.includes('\nbyte-identity: 0/3 identical  — 3 MISMATCH'));
});

test('the format probe times identical builds and reports each language', async () => {
	const code = await run_wasm_format_probe(
		{ load_glue: async () => glue(''), load_files: async () => FILES },
		['corpus', '--baseline', 'base.js', '--pairs', '2', '--warmup', '0']
	);
	strictEqual(code, 0);
	ok(err.includes('\nbyte-identity: 3/3 identical'));
	for (const label of ['svelte', 'typescript', 'css', 'total']) {
		ok(err.some((l) => l.trimStart().startsWith(`${label} `) && l.includes(' A/B ')));
	}
});

test('the memory probe reports each cold format’s reserve and growth from a fresh memory', async () => {
	let out = '';
	let loads = 0;
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async (): Promise<FreshGlue> => {
				loads++;
				const memory = new WebAssembly.Memory({ initial: 2 });
				return { exports: glue('', memory), memory };
			},
			load_files: async () => FILES,
			stdout: { write: async (data) => void (out += data) }
		},
		['corpus', '--cold', '--json']
	);
	strictEqual(code, 0);
	strictEqual(loads, 3);
	const current = JSON.parse(out).builds.current;
	strictEqual(current.reserve_bytes, 2 * 65536);
	strictEqual(current.peak_max, 3 * 65536);
	strictEqual(current.growth_max, 65536);
	strictEqual(current.n, 3);
});

test('the memory probe’s steady mode reuses one instance across the corpus', async () => {
	let out = '';
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async () => {
				const memory = new WebAssembly.Memory({ initial: 1 });
				return { exports: glue('', memory), memory };
			},
			load_files: async () => FILES,
			stdout: { write: async (data) => void (out += data) }
		},
		['corpus', '--json']
	);
	strictEqual(code, 0);
	deepStrictEqual(JSON.parse(out).builds.current, {
		reserve_bytes: 65536,
		high_water_bytes: 4 * 65536,
		total_growth_bytes: 3 * 65536,
		processed: 3
	});
});

test('the memory probe aborts an A/B whose builds format differently', async () => {
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async (path) => {
				const memory = new WebAssembly.Memory({ initial: 1 });
				return { exports: glue(path === 'base.js' ? '!' : '', memory), memory };
			},
			load_files: async () => FILES,
			stdout: { write: async () => {} }
		},
		['corpus', '--baseline', 'base.js']
	);
	strictEqual(code, 1);
	ok(err.some((l) => l.startsWith('ABORT: builds format differently on 3/3 files')));
});

test('a glue load that yields no memory is refused', async () => {
	await rejects(
		run_wasm_memory_probe(
			{
				load_fresh_glue: async () => ({ exports: glue(''), memory: null }),
				load_files: async () => FILES,
				stdout: { write: async () => {} }
			},
			['corpus', '--baseline', 'base.js']
		),
		/no memory captured for crates\/tsv_wasm\/pkg\/all\/nodejs\/tsv_wasm\.js/
	);
});
