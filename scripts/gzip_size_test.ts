/**
 * `gzip_size`: the byte length of what the `gzip` program writes for `gzip -c <path>`.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import { test } from 'vitest';

import { gzip_size, type GzipProgram } from './gzip_size.ts';

test('counts the bytes gzip -c writes, whatever its status', async () => {
	const calls: Array<ReadonlyArray<string>> = [];
	const gzip: GzipProgram = {
		out_bytes: async (args) => {
			calls.push(args);
			return { stdout: new Uint8Array(1234) };
		}
	};
	strictEqual(await gzip_size(gzip, '/pkg/tsv_wasm_bg.wasm'), 1234);
	deepStrictEqual(calls, [['-c', '/pkg/tsv_wasm_bg.wasm']]);
});

test('an empty output counts zero', async () => {
	const gzip: GzipProgram = { out_bytes: async () => ({ stdout: new Uint8Array(0) }) };
	strictEqual(await gzip_size(gzip, 'missing'), 0);
});
