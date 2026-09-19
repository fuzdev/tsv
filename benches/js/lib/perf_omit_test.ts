/**
 * Tests for the omissions summary the perf report publishes
 * (`summarize_group_omissions`) and the ledger's own shape. Dependency-free, so it
 * gates in `deno task test:deno`.
 */

import assert from 'node:assert';
import {
	generate_group_omissions_markdown,
	OMISSION_CATEGORIES,
	PERF_OMITS,
	type PerfOmit,
	perf_omit_matches,
	stale_perf_omits,
	summarize_group_omissions
} from './perf_omit.ts';

const OMITS: PerfOmit[] = [
	{
		task: 'format/css/biome',
		path: 'big.css',
		category: 'harvest_artifact',
		reason: 'test'
	},
	{
		task: 'format/css/oxfmt',
		path: 'a.css',
		category: 'tool_limit',
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
			{ name: 'oxfmt', files: 2, bytes: 80, categories: { tool_limit: 1, unlisted: 1 } }
		]
	});
});

Deno.test('categories are keyed in published order, not encounter order', () => {
	// `deepStrictEqual` ignores key order, and the committed JSON does not: the corpus
	// lists big.css (unlisted for oxfmt) before a.css here, and the record must not.
	const [tool] = summarize_group_omissions(
		'format/css',
		FILES,
		[{ name: 'oxfmt', tracking_key: 'format/css/oxfmt', failed: ['/c/big.css', '/c/a.css'] }],
		OMITS
	).by_tool;
	const published: readonly string[] = OMISSION_CATEGORIES;
	const keys = Object.keys(tool.categories);
	assert.deepStrictEqual(
		keys,
		[...keys].sort((a, b) => published.indexOf(a) - published.indexOf(b))
	);
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

Deno.test('no PERF_OMITS entry is scoped to a tsv row under another category', () => {
	// The pin above grades a LABEL. This grades the fact behind it: an entry whose task
	// names one of tsv's own rows (tracking keys end in `native…` / `wasm…`; the rivals'
	// wasm rows end in `<tool>-wasm`), or names no task at all and so reaches every row,
	// excuses a tsv failure whatever its category says.
	const reaches_tsv = (o: PerfOmit): boolean =>
		o.task === undefined || /^(native|wasm)(-|$)/.test(o.task.split('/').at(-1)!);
	assert.deepStrictEqual(
		PERF_OMITS.filter((o) => reaches_tsv(o) && o.category !== 'tsv_failure'),
		[]
	);
});

Deno.test('every PERF_OMITS entry names one path and says why', () => {
	for (const o of PERF_OMITS) {
		assert.ok(o.path.length > 0, 'empty path matches every file');
		assert.ok(o.reason.length > 0, `${o.path}: no reason`);
	}
});

Deno.test(
	'perf_omit_matches: task and path are both substring tests, and every match is returned',
	() => {
		const wildcard: PerfOmit = { ...OMITS[0], task: undefined, path: '.css' };
		const omits = [...OMITS, wildcard];
		assert.deepStrictEqual(perf_omit_matches(omits, 'format/css/biome', '/c/big.css'), [
			OMITS[0],
			wildcard
		]);
		// the right file under another row's key is not the entry's failure
		assert.deepStrictEqual(perf_omit_matches(OMITS, 'format/css/oxfmt', '/c/big.css'), []);
		assert.deepStrictEqual(perf_omit_matches(OMITS, 'format/css/biome', '/c/b.css'), []);
	}
);

Deno.test('stale_perf_omits: unused AND reachable, never one alone', () => {
	const keys = ['format/css/biome', 'format/css/canonical'];
	// used → not stale
	assert.deepStrictEqual(stale_perf_omits(OMITS, new Set([OMITS[0]]), keys), []);
	// unused, and its task ran → stale
	assert.deepStrictEqual(stale_perf_omits(OMITS, new Set(), keys), [OMITS[0]]);
	// unused, but its task never registered on this machine → unasked, not stale
	assert.deepStrictEqual(stale_perf_omits(OMITS, new Set(), ['format/css/canonical']), []);
});

Deno.test(
	'the markdown line leads with bytes, names each row, and is absent when nothing is',
	() => {
		const summary = summarize_group_omissions(
			'format/css',
			FILES,
			[
				{ name: 'prettier', tracking_key: 'format/css/canonical', failed: [] },
				{ name: 'biome-wasm', tracking_key: 'format/css/biome', failed: ['/c/big.css'] }
			],
			OMITS
		);
		assert.strictEqual(
			generate_group_omissions_markdown(summary),
			"**Omitted from every row's timed set:** 1 of 3 files, 70.0% of the group's bytes " +
				'(33.3% of its files) — by row: biome-wasm 1 (1 harvest_artifact). Each is a reviewed ' +
				'entry in `lib/perf_omit.ts`.'
		);
		assert.strictEqual(generate_group_omissions_markdown(undefined), null);
		assert.strictEqual(
			generate_group_omissions_markdown(summarize_group_omissions('format/css', FILES, [], OMITS)),
			null
		);
	}
);
