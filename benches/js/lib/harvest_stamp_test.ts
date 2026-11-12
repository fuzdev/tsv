/**
 * Tests for the harvest stamp logic over in-process fakes: the `git rev-parse` it sends and
 * what it makes of each result, the freshness gate, and the pinned cache writer, which writes
 * nothing on a pin mismatch.
 */

import { deepStrictEqual, strictEqual } from 'node:assert';
import { test } from 'vitest';

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import {
	checkout_hash,
	corpus_filter_fingerprint,
	git_head,
	harvest_up_to_date,
	type StampGit,
	type StampReads,
	type StampWrites,
	write_pinned_path_cache,
	write_stamp
} from './harvest_stamp.ts';

type GitResult = Awaited<ReturnType<StampGit['out']>>;

/** What a root hands in, checked by `tsc -p tsconfig.vitest.json`: gro's handles satisfy the
 * deps. */
const gro_handles_are_stamp_deps = (
	git: SpawnProgram,
	reads: FsReadDir,
	writes: FsDir
): [StampGit, StampReads, StampWrites] => [git, reads, writes];

/** A `git` answering every call with `result` (or throwing it), recording each argv. */
const fake_git = (
	result: GitResult | Error
): StampGit & { calls: Array<ReadonlyArray<string>> } => {
	const calls: Array<ReadonlyArray<string>> = [];
	return {
		calls,
		out: async (args) => {
			calls.push(args);
			if (result instanceof Error) throw result;
			return result;
		}
	};
};

/** The checkout's files as a map, with every write and mkdir recorded in order. */
const fake_fs = (
	files: Record<string, string> = {}
): StampWrites & { files: Map<string, string>; log: string[] } => {
	const map = new Map(Object.entries(files));
	const log: string[] = [];
	return {
		files: map,
		log,
		exists: async (path) => map.has(path),
		read_text: async (path) => {
			const text = map.get(path);
			if (text === undefined) throw new Error(`no ${path}`);
			return text;
		},
		write: async (path, data) => {
			log.push(`write ${path}`);
			map.set(path, data);
		},
		mkdir: async (path, options) => {
			log.push(`mkdir ${path} ${JSON.stringify(options)}`);
		}
	};
};

test('git_head asks rev-parse HEAD in the checkout and trims the id', async () => {
	const git = fake_git({ kind: 'exited', code: 0, stdout: 'abc123\n' });
	strictEqual(await git_head(git, '../svelte'), 'abc123');
	deepStrictEqual(git.calls, [['-C', '../svelte', 'rev-parse', 'HEAD']]);
});

test("checkout_hash asks for a tree input's subtree at HEAD", async () => {
	const git = fake_git({ kind: 'exited', code: 0, stdout: 'def456\n' });
	strictEqual(await checkout_hash(git, { path: '../corpora', tree: 'collections' }), 'def456');
	strictEqual(await checkout_hash(git, '../wpt/css'), 'def456');
	deepStrictEqual(git.calls, [
		['-C', '../corpora', 'rev-parse', 'HEAD:collections'],
		['-C', '../wpt/css', 'rev-parse', 'HEAD']
	]);
});

test('a failed, signaled, or unstartable git reads as no checkout', async () => {
	strictEqual(await git_head(fake_git({ kind: 'exited', code: 128, stdout: '' }), 'x'), null);
	strictEqual(await git_head(fake_git({ kind: 'signaled', stdout: 'abc\n' }), 'x'), null);
	strictEqual(await git_head(fake_git({ kind: 'timed_out', stdout: 'abc\n' }), 'x'), null);
	strictEqual(await git_head(fake_git(new Error('no git')), 'x'), null);
});

test('a harvest is up to date only with every cache present and the inputs exactly stamped', async () => {
	const stamp = 'benches/js/.cache/x.stamp.json';
	const inputs = { a: 'one', b: 2, c: null };
	const fs = fake_fs({ [stamp]: JSON.stringify(inputs), cache: '[]' });
	strictEqual(await harvest_up_to_date(fs, stamp, inputs, ['cache']), true);
	strictEqual(await harvest_up_to_date(fs, stamp, inputs, ['cache', 'gone']), false);
	strictEqual(await harvest_up_to_date(fs, stamp, { ...inputs, b: 3 }, []), false);
	strictEqual(await harvest_up_to_date(fs, stamp, { a: 'one', b: 2 }, []), false);
	strictEqual(await harvest_up_to_date(fs, stamp, { ...inputs, d: 'more' }, []), false);
	strictEqual(await harvest_up_to_date(fs, 'never_written.json', inputs, []), false);
	fs.files.set(stamp, '{not json');
	strictEqual(await harvest_up_to_date(fs, stamp, inputs, []), false);
});

test('write_stamp writes the inputs as tab-indented JSON with a final newline', async () => {
	const fs = fake_fs();
	await write_stamp(fs, 's.json', { a: 'one', b: 2 });
	strictEqual(fs.files.get('s.json'), '{\n\t"a": "one",\n\t"b": 2\n}\n');
});

test('a pin mismatch writes nothing, says so, and returns null', async () => {
	const fs = fake_fs();
	const printed: unknown[] = [];
	const print_error = console.error;
	console.error = (...line: unknown[]) => printed.push(line.join(' '));
	let out;
	try {
		out = await write_pinned_path_cache(fs, {
			cache: 'benches/js/.cache/list.json',
			paths: ['a', 'b'],
			pin: 3,
			what: 'files',
			stamp: { path: 'benches/js/.cache/list.stamp.json', inputs: { a: 'one' } }
		});
	} finally {
		console.error = print_error;
	}
	strictEqual(out, null);
	deepStrictEqual(fs.log, []);
	deepStrictEqual(printed, [
		'FAIL: pinned count mismatch — 2 files ≠ pinned 3; cache not written. If the move is ' +
			'deliberate (suite refresh), re-pin in lib/gate_counts.ts.'
	]);
});

test('a held pin writes the cache, then its stamp', async () => {
	const fs = fake_fs();
	const out = await write_pinned_path_cache(fs, {
		cache: 'benches/js/.cache/list.json',
		paths: ['a', 'b'],
		pin: 2,
		what: 'files',
		stamp: { path: 'benches/js/.cache/list.stamp.json', inputs: { a: 'one' } }
	});
	strictEqual(out, 'benches/js/.cache/list.json');
	deepStrictEqual(fs.log, [
		'mkdir benches/js/.cache {"recursive":true}',
		'write benches/js/.cache/list.json',
		'write benches/js/.cache/list.stamp.json'
	]);
	strictEqual(fs.files.get('benches/js/.cache/list.json'), '[\n\t"a",\n\t"b"\n]\n');
	const unstamped = fake_fs();
	await write_pinned_path_cache(unstamped, {
		cache: 'c.json',
		paths: [],
		pin: 0,
		what: 'files',
		stamp: null
	});
	deepStrictEqual(unstamped.log, ['mkdir . {"recursive":true}', 'write c.json']);
});

test("the filter fingerprint reads its view's modules and moves with their text", async () => {
	const fs = fake_fs({
		'benches/js/lib/corpus.ts': 'loader',
		'benches/js/lib/prettier_fixtures.ts': 'filter'
	});
	const perf = await corpus_filter_fingerprint(fs, 'perf');
	const conformance = await corpus_filter_fingerprint(fs, 'conformance');
	// each module's path, a NUL, its text, a NUL, through SHA-1, the first 16 hex digits: a
	// change to that recipe moves every stamp a harvest has written
	strictEqual(perf, '61a9c39372849058');
	strictEqual(perf === conformance, false);
	strictEqual(await corpus_filter_fingerprint(fs, 'perf'), perf);
	fs.files.set('benches/js/lib/corpus.ts', 'loader edited');
	strictEqual((await corpus_filter_fingerprint(fs, 'perf')) === perf, false);
	fs.files.set('benches/js/lib/corpus.ts', 'loader');
	fs.files.set('benches/js/lib/prettier_fixtures.ts', 'filter edited');
	strictEqual(await corpus_filter_fingerprint(fs, 'perf'), perf);
	strictEqual((await corpus_filter_fingerprint(fs, 'conformance')) === conformance, false);
});
