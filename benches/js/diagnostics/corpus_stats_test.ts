/**
 * `run_corpus_stats` over a temp directory as a candidate entry: the `--json` report's shape and
 * paths, the human report's stream, and each failure's exit code and message.
 */

import { deepStrictEqual, match, rejects, strictEqual } from 'node:assert';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { afterAll, afterEach, beforeAll, beforeEach, describe, test, vi } from 'vitest';

import { run_corpus_stats } from './corpus_stats.ts';

let temp: FsDir & AsyncDisposable;
let out: string[];
let err: string[];

beforeAll(async () => {
	temp = await fs.temp_dir('tsv_corpus_stats_');
	await temp.mkdir('src/tests', { recursive: true });
	await temp.mkdir('empty');
	await temp.write('src/a.ts', 'export const a = 1;\n'.repeat(20));
	await temp.write('src/b.svelte', '<p>b</p>\n');
	await temp.write('src/c.css', 'a { color: red; }\n'.repeat(400));
	await temp.write('src/tests/d.ts', 'export {};\n');
	await temp.write('src/notes.md', '# not in scope\n');
});

afterAll(async () => {
	await temp[Symbol.asyncDispose]();
});

beforeEach(() => {
	out = [];
	err = [];
	vi.spyOn(console, 'log').mockImplementation((...a: unknown[]) => void out.push(a.join(' ')));
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe('a candidate directory', () => {
	test('--json: the report on stdout, paths relative to the directory, exit 0', async () => {
		const dir = `${temp.path}/src`;
		strictEqual(await run_corpus_stats([dir, '--json', '--big', '4000']), 0);
		deepStrictEqual(err, []);
		strictEqual(out.length, 1);
		const report = JSON.parse(out[0]);
		strictEqual(report.title, `directory ${dir} (as a corpus entry)`);
		strictEqual(report.overall.files, 4);
		deepStrictEqual(report.overall.by_language, { svelte: 1, typescript: 2, css: 1 });
		deepStrictEqual(
			report.largest.map((f: { path: string }) => f.path),
			['c.css', 'a.ts', 'tests/d.ts', 'b.svelte']
		);
		deepStrictEqual(report.flags.big_files, [{ path: 'c.css', bytes: 7200 }]);
		deepStrictEqual(report.flags.watch_segments, [{ segment: 'tests', files: 1, bytes: 11 }]);
		deepStrictEqual(report.groups, []);
	});

	test('the human report goes to stderr, nothing to stdout', async () => {
		strictEqual(await run_corpus_stats([`${temp.path}/src`, '--largest', '2']), 0);
		deepStrictEqual(out, []);
		match(err.join('\n'), /Overall: 4 files/);
		match(err.join('\n'), /Largest 2 files:/);
		match(err.join('\n'), /⚠ 'tests\/' subtree present: 1 files/);
	});
});

describe('failures', () => {
	test('a directory with nothing in scope: its message on stderr, exit 1', async () => {
		const dir = `${temp.path}/empty`;
		strictEqual(await run_corpus_stats([dir]), 1);
		deepStrictEqual(err, [`No in-scope files found for directory ${dir} (as a corpus entry)`]);
	});

	test('a view that does not exist: its message on stderr, exit 1, nothing loaded', async () => {
		strictEqual(await run_corpus_stats(['--view', 'bogus']), 1);
		deepStrictEqual(err, ["--view must be perf|gates|conformance (got 'bogus')"]);
	});

	test('an unknown option throws, as parseArgs does', async () => {
		await rejects(run_corpus_stats(['--nope']), { code: 'ERR_PARSE_ARGS_UNKNOWN_OPTION' });
	});
});
