/**
 * `run_test262_compare` over a fake manifest, checkout, and parser: the goal and strict-mode
 * source each row hands oxc, each bucket of the differential, the pass rates, and an unreadable
 * test counted rather than graded. Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, beforeEach, test, vi } from 'vitest';

import {
	type OxcParseSync,
	run_test262_compare,
	type Test262CompareDeps
} from './test262_compare.ts';

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

const row = (
	relative_path: string,
	expected: string,
	tsv: string,
	module = false,
	strict = false
) => ({
	relative_path,
	module,
	strict,
	expected,
	tsv
});

const TESTS = [
	row('pos/both.js', 'accept', 'accept'),
	row('pos/tsv_bug.js', 'accept', 'reject'),
	row('pos/oxc_gap.js', 'accept', 'accept', true),
	row('pos/strict.js', 'accept', 'reject', false, true),
	row('neg/both.js', 'reject', 'reject'),
	row('neg/tsv_gap.js', 'reject', 'accept'),
	row('neg/both_accept.js', 'reject', 'accept'),
	row('neg/tsv_stricter.js', 'reject', 'reject'),
	row('missing.js', 'accept', 'accept')
];

/** oxc refuses a source holding `OXC_REJECTS`, or a module-goal parse, and throws on `THROW`. */
const parse_sync: OxcParseSync = (_filename, source, options) => {
	if (source.includes('THROW')) throw new Error('boom');
	return { errors: source.includes('OXC_REJECTS') || options.sourceType === 'module' ? [{}] : [] };
};

const SOURCES: Record<string, string> = {
	'pos/both.js': 'ok',
	'pos/tsv_bug.js': 'ok',
	'pos/oxc_gap.js': 'ok',
	'pos/strict.js': 'ok',
	'neg/both.js': 'THROW',
	'neg/tsv_gap.js': 'OXC_REJECTS',
	'neg/both_accept.js': 'ok',
	'neg/tsv_stricter.js': 'ok'
};

const fake_deps = () => {
	const state = { manifest_path: '', root: '', parsed: [] as string[], out: '' };
	const deps: Test262CompareDeps = {
		read_manifest: async (path) => {
			state.manifest_path = path;
			return JSON.stringify({ test262_root: '../test262', count: TESTS.length, tests: TESTS });
		},
		open_root: async (path) => {
			state.root = path;
			return {
				read_text: async (file) => {
					const source = SOURCES[file];
					if (source === undefined) throw new Error('not_found');
					return source;
				}
			};
		},
		parse_sync: (filename, source, options) => {
			state.parsed.push(`${filename} ${options.sourceType} ${JSON.stringify(source)}`);
			return parse_sync(filename, source, options);
		},
		stdout: { write: async (data) => void (state.out += data) }
	};
	return { deps, state };
};

test('each row is graded at its goal, strict rows behind the harness prefix, into its bucket', async () => {
	const { deps, state } = fake_deps();
	await run_test262_compare(deps, ['--manifest', '/m.json', '--examples', '1']);
	strictEqual(state.manifest_path, '/m.json');
	strictEqual(state.root, '../test262');
	ok(state.parsed.includes('pos/oxc_gap.js module "ok"'));
	ok(state.parsed.includes('pos/strict.js script "\\"use strict\\";\\nok"'));
	ok(state.parsed.includes('pos/both.js script "ok"'));
	const report = JSON.parse(state.out);
	strictEqual(report.graded, 8);
	strictEqual(report.read_errors, 1);
	deepStrictEqual(report.counts, {
		pos_both_accept: 1,
		pos_tsv_bug: 2,
		pos_oxc_gap: 1,
		pos_both_reject: 0,
		neg_both_reject: 1,
		neg_tsv_gap: 1,
		neg_both_accept: 1,
		neg_tsv_stricter: 1
	});
	deepStrictEqual(report.pass_rate, { tsv: 4, oxc: 5, total: 8 });
	strictEqual(report.buckets.pos_both_accept, undefined);
	deepStrictEqual(
		report.buckets.pos_tsv_bug.map((r: { path: string }) => r.path),
		['pos/tsv_bug.js', 'pos/strict.js']
	);
	ok(err.includes('(1 files unreadable, skipped)'));
	// --examples caps each starred list
	ok(err.includes('  pos/tsv_bug.js'));
	ok(!err.includes('  pos/strict.js'));
});

test('the manifest defaults to the path the Rust runner documents', async () => {
	const { deps, state } = fake_deps();
	await run_test262_compare(deps, []);
	strictEqual(state.manifest_path, '/tmp/tsv-test262-manifest.json');
});
