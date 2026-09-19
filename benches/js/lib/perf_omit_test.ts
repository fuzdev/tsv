/**
 * Tests for the omissions summary the perf report publishes
 * (`summarize_group_omissions`) and the ledger's own shape. Dependency-free, so it
 * gates in `deno task test:deno`.
 */

import assert from 'node:assert';
import { PERF_OMITS, type PerfOmit, summarize_group_omissions } from './perf_omit.ts';

const OMITS: PerfOmit[] = [
	{
		task: 'format/css/biome',
		path: 'big.css',
		category: 'harvest_artifact',
		failure: 'no_op',
		reason: 'test'
	},
	{
		task: 'format/css/oxfmt',
		path: 'a.css',
		category: 'tool_limit',
		failure: 'no_op',
		reason: 'test'
	}
];

const FILES = [
	{ path: '/c/a.css', bytes: 10 },
	{ path: '/c/b.css', bytes: 20 },
	{ path: '/c/big.css', bytes: 70 }
];

Deno.test('omitted_* is the union over rows, by_tool is per row', () => {
	const summary = summarize_group_omissions(
		'format/css',
		FILES,
		[
			{ name: 'prettier', tracking_key: 'format/css/canonical', failed: [] },
			{ name: 'biome-wasm', tracking_key: 'format/css/biome', failed: ['/c/big.css'] },
			{ name: 'oxfmt', tracking_key: 'format/css/oxfmt', failed: ['/c/big.css', '/c/a.css'] }
		],
		OMITS
	);
	assert.deepStrictEqual(summary, {
		group: 'format/css',
		files_total: 3,
		bytes_total: 100,
		omitted_files: 2,
		omitted_bytes: 80,
		by_tool: [
			{ name: 'biome-wasm', files: 1, bytes: 70, categories: { harvest_artifact: 1 } },
			// big.css matches no oxfmt entry, so it is named rather than dropped
			{ name: 'oxfmt', files: 2, bytes: 80, categories: { unlisted: 1, tool_limit: 1 } }
		]
	});
});

Deno.test('a group nothing failed reports zeroes and no rows', () => {
	const summary = summarize_group_omissions(
		'format/css',
		FILES,
		[{ name: 'prettier', tracking_key: 'format/css/canonical', failed: [] }],
		OMITS
	);
	assert.strictEqual(summary.omitted_files, 0);
	assert.strictEqual(summary.omitted_bytes, 0);
	assert.deepStrictEqual(summary.by_tool, []);
});

Deno.test('a failure outside the group’s files is not reported', () => {
	const summary = summarize_group_omissions(
		'format/css',
		FILES,
		[{ name: 'biome-wasm', tracking_key: 'format/css/biome', failed: ['/elsewhere/x.css'] }],
		OMITS
	);
	assert.strictEqual(summary.omitted_files, 0);
	assert.deepStrictEqual(summary.by_tool, []);
});

Deno.test('no PERF_OMITS entry tolerates a tsv failure', () => {
	// The one category that would hide tsv's own bug behind a rival's ledger. An
	// entry here is a deliberate decision to make in review, not to land silently.
	assert.deepStrictEqual(
		PERF_OMITS.filter((o) => o.category === 'tsv_failure'),
		[]
	);
});
