/**
 * `run_compose_reports` over an in-memory results directory: what it reads, what it writes and
 * where it says it wrote it, and each sibling it treats as absent.
 */

import { deepStrictEqual, match, ok as assert, strictEqual } from 'node:assert';

import { afterEach, beforeEach, describe, test, vi } from 'vitest';

import { type ComposeResultsDir, run_compose_reports } from './compose_reports.ts';

const RESULTS_PATH = '/checkout/benches/js/results';

interface Results extends ComposeResultsDir {
	reads: string[];
	writes: Map<string, string>;
}

/** A results directory over `{name: text}`; a missing name rejects like any failed read. */
const results_of = (files: Record<string, string | Error>): Results => {
	const reads: string[] = [];
	const writes = new Map<string, string>();
	return {
		path: RESULTS_PATH,
		reads,
		writes,
		read_text: (name) => {
			reads.push(name);
			const value = files[name];
			if (value === undefined) return Promise.reject(new Error(`ENOENT ${name}`));
			if (value instanceof Error) return Promise.reject(value);
			return Promise.resolve(value);
		},
		write: (name, data) => {
			writes.set(name, data);
			return Promise.resolve();
		}
	};
};

/** A sibling report with one timed row. */
const sibling = (
	runtime: string,
	git_commit = 'abc12345',
	timestamp = '2026-01-01T00:00:00.000Z'
) =>
	JSON.stringify({
		version: 23,
		runtime,
		timestamp,
		git_commit,
		versions: { tsv: '0.1.0' },
		entries: [
			{
				name: 'tsv',
				group: 'format/css',
				mean_ns: 1000,
				ops_per_second: runtime === 'deno' ? 100 : 200,
				files_iterated: 10,
				runtime
			}
		],
		unavailable: []
	});

let out: string[];
let err: string[];

beforeEach(() => {
	out = [];
	err = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void out.push(a.join(' ')));
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe('run_compose_reports', () => {
	test('no sibling: exits 1, says so on stderr, writes nothing', async () => {
		const results = results_of({});
		strictEqual(await run_compose_reports({ results }), 1);
		strictEqual(results.writes.size, 0);
		strictEqual(err.length, 1);
		match(err[0], /^No per-runtime reports found in results\//);
		deepStrictEqual(results.reads, ['report.deno.json', 'report.node.json', 'report.bun.json']);
	});

	test('folds the siblings present, writes report.json and report.md, and names them by the directory', async () => {
		const results = results_of({
			'report.deno.json': sibling('deno'),
			'report.node.json': sibling('node')
		});
		strictEqual(await run_compose_reports({ results }), 0);
		deepStrictEqual([...results.writes.keys()], ['report.json', 'report.md']);
		const combined = JSON.parse(results.writes.get('report.json')!);
		strictEqual(combined.version, 19);
		deepStrictEqual(combined.runtimes, ['deno', 'node']);
		deepStrictEqual(combined.rows, [
			{
				group: 'format/css',
				name: 'tsv',
				ops_per_second: { deno: 100, node: 200 },
				mean_ns: { deno: 1000, node: 1000 },
				files_iterated: { deno: 10, node: 10 }
			}
		]);
		strictEqual(combined.conformance_vintage, null);
		strictEqual(results.writes.get('report.json'), JSON.stringify(combined, null, '\t'));
		const md = results.writes.get('report.md')!.split('\n');
		strictEqual(md[0], '# tsv benchmark results — cross-runtime');
		const table = md.indexOf('## format/css');
		deepStrictEqual(md.slice(table, table + 6), [
			'## format/css',
			'',
			'| Impl | deno sweeps/sec | node sweeps/sec | node/deno |',
			'| --- | ---: | ---: | ---: |',
			'| tsv | 100.0 | 200.0 | 2.00x |',
			''
		]);
		deepStrictEqual(out, [
			'Composed cross-runtime report from: deno, node',
			`  ${RESULTS_PATH}/report.json`,
			`  ${RESULTS_PATH}/report.md`
		]);
		deepStrictEqual(results.reads, [
			'report.deno.json',
			'report.node.json',
			'report.bun.json',
			'report.conformance.node.json'
		]);
	});

	test('a sibling that is not JSON is absent, with a warning naming its path', async () => {
		const results = results_of({ 'report.deno.json': sibling('deno'), 'report.bun.json': '{' });
		strictEqual(await run_compose_reports({ results }), 0);
		deepStrictEqual(JSON.parse(results.writes.get('report.json')!).runtimes, ['deno']);
		assert(
			err.some((line) =>
				line.startsWith(`⚠ compose: ${RESULTS_PATH}/report.bun.json exists but is not valid JSON (`)
			),
			err.join('\n')
		);
	});

	test('a sibling whose read fails for any reason is absent, silently', async () => {
		const results = results_of({
			'report.deno.json': sibling('deno'),
			'report.node.json': new Error('EISDIR: illegal operation on a directory')
		});
		strictEqual(await run_compose_reports({ results }), 0);
		deepStrictEqual(JSON.parse(results.writes.get('report.json')!).runtimes, ['deno']);
		deepStrictEqual(err, []);
	});

	test('a conformance report from another commit is a stale vintage, warned with its remedy', async () => {
		const results = results_of({
			'report.deno.json': sibling('deno', 'abc12345', '2026-01-02T00:00:00.000Z'),
			'report.conformance.node.json': sibling('node', 'def67890', '2026-01-01T00:00:00.000Z')
		});
		strictEqual(await run_compose_reports({ results }), 0);
		const vintage = JSON.parse(results.writes.get('report.json')!).conformance_vintage;
		deepStrictEqual(vintage, {
			git_commit: 'def67890',
			timestamp: '2026-01-01T00:00:00.000Z',
			tsv: '0.1.0',
			stale: true
		});
		deepStrictEqual(err, [
			'⚠ compose: report.conformance.node.json is at def67890, behind the perf siblings — the site ' +
				'publishes both from one directory; re-run `deno task bench:conformance` (or `deno task ' +
				'bench` for everything).'
		]);
	});

	test('a conformance report that is not JSON drops the vintage, with a warning naming its path', async () => {
		const results = results_of({
			'report.deno.json': sibling('deno'),
			'report.conformance.node.json': '{"version":'
		});
		strictEqual(await run_compose_reports({ results }), 0);
		strictEqual(JSON.parse(results.writes.get('report.json')!).conformance_vintage, null);
		assert(
			err.some((line) =>
				line.startsWith(
					`⚠ compose: ${RESULTS_PATH}/report.conformance.node.json exists but is not valid JSON (`
				)
			),
			err.join('\n')
		);
	});
});
