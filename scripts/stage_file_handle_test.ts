/**
 * `stage_file_between`: `stage_file` across two handles — the source's bytes and mode on the
 * copy, the copy dated at the staging, and nothing written outside the staging handle.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { test } from 'vitest';

import { stage_file } from './stage_file.ts';
import { stage_file_between } from './stage_file_handle.ts';

/** A temp tree holding `src/` and `pkg/`, with handles on each. */
const make_tree = async (): Promise<{
	root: FsDir & AsyncDisposable;
	src: FsDir;
	pkg: FsDir;
}> => {
	const root = await fs.temp_dir('tsv_stage_file_handle_test_');
	await root.mkdir('src');
	await root.mkdir('pkg');
	return { root, src: root.dir('src'), pkg: root.dir('pkg') };
};

/** A source dated well before the staging, so a copy that kept its date would show. */
const PAST_MS = Date.UTC(2020, 0, 1);

test("the copy holds the source's bytes and mode, dated at the staging", async () => {
	const { root, src, pkg } = await make_tree();
	await using _ = root;
	await src.write('a.js', 'export const a = 1;\n', { mode: 0o640 });
	await src.set_mode('a.js', 0o640);
	await src.set_times('a.js', { mtime_ms: PAST_MS });
	const before = Date.now();
	await stage_file(stage_file_between(src.readonly(), pkg), 'a.js', 'a.js');
	const after = Date.now();
	strictEqual(await pkg.read_text('a.js'), 'export const a = 1;\n');
	const staged = (await pkg.stat('a.js'))!;
	strictEqual(staged.mode, 0o640);
	// the Node implementation passes seconds as a double, so allow a millisecond either way
	ok(staged.mtime_ms >= before - 1 && staged.mtime_ms <= after + 1, `mtime ${staged.mtime_ms}`);
	strictEqual((await src.stat('a.js'))!.mtime_ms, PAST_MS);
});

test("an existing copy is replaced, its mode the source's", async () => {
	const { root, src, pkg } = await make_tree();
	await using _ = root;
	await src.write('cli.js', 'new');
	await src.set_mode('cli.js', 0o755);
	await pkg.write('cli.js', 'old old old');
	await pkg.set_mode('cli.js', 0o600);
	await stage_file(stage_file_between(src.readonly(), pkg), 'cli.js', 'cli.js');
	strictEqual(await pkg.read_text('cli.js'), 'new');
	strictEqual((await pkg.stat('cli.js'))!.mode, 0o755);
});

test('a missing source stages nothing', async () => {
	const { root, src, pkg } = await make_tree();
	await using _ = root;
	await rejects(stage_file(stage_file_between(src.readonly(), pkg), 'absent', 'absent'), (error) =>
		is_fs_error(error, 'not_found')
	);
	deepStrictEqual(await Array.fromAsync(pkg.list()), []);
});

test('the copy is written beneath the staging handle alone', async () => {
	const { root, src, pkg } = await make_tree();
	await using _ = root;
	await src.write('a.js', 'a');
	await rejects(
		stage_file(stage_file_between(src.readonly(), pkg), 'a.js', '../escape.js'),
		(error) => is_fs_error(error, 'outside_root')
	);
	strictEqual(await root.exists('escape.js'), false);
});
