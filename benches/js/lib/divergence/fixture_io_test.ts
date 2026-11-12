/**
 * `fixture_io`: the fixture reads `divergence:audit` and the fixture-coverage suite share, over
 * a temp directory standing in for `tests/fixtures`. A missing path reads as absent, every other
 * failure throws, and a symlink is followed only where the handle follows one.
 */

import { deepStrictEqual, rejects, strictEqual, throws } from 'node:assert';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { FsError, is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { afterAll, beforeAll, describe, test } from 'vitest';

import type { FixtureIo } from './fixture_cases.ts';
import { fixture_io } from './fixture_io.ts';

let temp: FsDir & AsyncDisposable;
let io: FixtureIo;

beforeAll(async () => {
	temp = await fs.temp_dir('tsv_fixture_io_');
	await temp.mkdir('fixtures/a/nested', { recursive: true });
	await temp.write('fixtures/a/input.svelte', 'OURS');
	await temp.write('fixtures/a/output_prettier.svelte', 'PRETTIER');
	await temp.write('fixtures/a/b_variant.svelte', 'B');
	await temp.symlink('fixtures/a/prettier_variant_x.svelte', 'output_prettier.svelte');
	await temp.write('fixtures/plain_file', 'f');
	await temp.mkdir('fixtures/linked_input');
	await temp.write('fixtures/linked_input/real.ts', 'REAL');
	await temp.symlink('fixtures/linked_input/input.ts', 'real.ts');
	await temp.symlink('fixtures/link_to_a', 'a');
	await temp.symlink('fixtures/link_dangling', 'nowhere');
	await temp.symlink('fixtures/link_absolute', `${temp.path}/fixtures/a`);
	await temp.mkdir('outside');
	await temp.symlink('fixtures/link_up', '../outside');
	io = fixture_io(temp.dir('fixtures').readonly());
});

afterAll(async () => {
	await temp[Symbol.asyncDispose]();
});

const assert_fs_error = (kind: string) => (error: unknown) =>
	is_fs_error(error) && error.kind === kind;

describe('dir_exists', () => {
	test('a directory, and a relative symlink to one beneath the handle', () => {
		strictEqual(io.dir_exists('a'), true);
		strictEqual(io.dir_exists('a/nested'), true);
		strictEqual(io.dir_exists('link_to_a'), true);
	});

	test('a missing path, a dangling symlink, and a file all read as no directory', () => {
		strictEqual(io.dir_exists('missing'), false);
		strictEqual(io.dir_exists('a/missing'), false);
		strictEqual(io.dir_exists('link_dangling'), false);
		strictEqual(io.dir_exists('plain_file'), false);
	});

	test('a path through a file throws, never reading as missing', () => {
		throws(() => io.dir_exists('plain_file/x'), assert_fs_error('not_a_directory'));
	});

	test('a symlink the handle does not follow throws', () => {
		throws(() => io.dir_exists('link_absolute'), assert_fs_error('outside_root'));
		throws(() => io.dir_exists('link_up'), assert_fs_error('outside_root'));
		throws(() => io.dir_exists('../outside'), assert_fs_error('outside_root'));
	});
});

describe('list', () => {
	test('names the regular files only, sorted: no directory, no symlink', async () => {
		deepStrictEqual(await io.list('a'), [
			'b_variant.svelte',
			'input.svelte',
			'output_prettier.svelte'
		]);
	});

	test('a missing directory lists nothing', async () => {
		deepStrictEqual(await io.list('missing'), []);
	});

	test('a file where the directory should be throws', async () => {
		await rejects(io.list('plain_file'), assert_fs_error('not_a_directory'));
	});

	test('a failure other than a missing directory throws', async () => {
		const denied = fixture_io({
			list: () => ({
				[Symbol.asyncIterator]: () => ({
					next: () => Promise.reject(new FsError('permission_denied', 'x'))
				})
			})
		} as unknown as FsReadDir);
		await rejects(denied.list('a'), assert_fs_error('permission_denied'));
	});
});

describe('read', () => {
	test('a file reads as its text, a symlinked one through its target', async () => {
		strictEqual(await io.read('a', 'input.svelte'), 'OURS');
		strictEqual(await io.read('linked_input', 'input.ts'), 'REAL');
	});

	test('a missing file, or one in a missing directory, reads as null', async () => {
		strictEqual(await io.read('a', 'input.ts'), null);
		strictEqual(await io.read('missing', 'input.svelte'), null);
	});

	test('a directory where the file should be, or a path through a file, throws', async () => {
		await rejects(io.read('a', 'nested'), assert_fs_error('is_a_directory'));
		await rejects(io.read('plain_file', 'input.svelte'), assert_fs_error('not_a_directory'));
	});

	test('a fixture the handle cannot follow into throws', async () => {
		await rejects(io.read('link_absolute', 'input.svelte'), assert_fs_error('outside_root'));
	});
});
