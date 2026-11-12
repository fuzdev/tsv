/**
 * The bump's writes over copies of the checkout's own `Cargo.toml` and `CHANGELOG.md` in a temp
 * directory: byte for byte what the version setter and the stamp make, each file's mode kept, no
 * write when nothing changes; the lockfile update's call over a fake `cargo`; and the sentinel's
 * read, write, and removal.
 *
 * @module
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';
import { test } from 'vitest';

import { bump_cargo_version, bump_changelog, bump_lockfile, RELEASE_PATHS } from './bump.ts';
import { changelog_stamp } from './changelog.ts';
import { sentinel_read, sentinel_remove, sentinel_write, SENTINEL_PATH } from './sentinel.ts';
import { cargo_workspace_version, version_bump } from './version.ts';

const CHECKOUT = new URL('../../', import.meta.url).pathname.replace(/\/$/, '');

/** A temp checkout holding copies of the real manifests the bump edits. */
const copy_checkout = async (): Promise<{
	dir: FsDir & AsyncDisposable;
	cargo: string;
	changelog: string;
}> => {
	const real = await fs.open_dir(CHECKOUT);
	const cargo = await real.read_text('Cargo.toml');
	const changelog = await real.read_text('CHANGELOG.md');
	const dir = await fs.temp_dir('tsv_bump_test_');
	await dir.write('Cargo.toml', cargo);
	await dir.write('CHANGELOG.md', changelog);
	return { dir, cargo, changelog };
};

test('the release files are the manifest, the lockfile, and the changelog', () => {
	deepStrictEqual(RELEASE_PATHS, ['Cargo.toml', 'Cargo.lock', 'CHANGELOG.md']);
});

test("the version bump rewrites the real manifest's version and every sibling pin, nothing else", async () => {
	const { dir, cargo } = await copy_checkout();
	await using _ = dir;
	const version = cargo_workspace_version(cargo)!;
	const next = version_bump(version, 'patch')!;
	await dir.set_mode('Cargo.toml', 0o640);
	deepStrictEqual(await bump_cargo_version(dir, next), { ok: true });
	// the oracle, spelled apart from the setter: the one workspace version line and each pin
	const pins = cargo.split(`"=${version}"`).length - 1;
	ok(pins > 0);
	const expected = cargo
		.replace(`\nversion = "${version}"\n`, `\nversion = "${next}"\n`)
		.replaceAll(`"=${version}"`, `"=${next}"`);
	strictEqual(await dir.read_text('Cargo.toml'), expected);
	const changed = cargo.split('\n').filter((line, i) => line !== expected.split('\n')[i]);
	strictEqual(changed.length, pins + 1);
	strictEqual((await dir.stat('Cargo.toml'))!.mode & 0o777, 0o640);
});

test('a version bump the setter refuses writes nothing', async () => {
	await using dir = await fs.temp_dir('tsv_bump_test_');
	const text =
		'[workspace.package]\nversion = "0.1.0"\n\n[workspace.dependencies.tsv_x]\npath = "x"\n';
	await dir.write('Cargo.toml', text);
	await dir.set_times('Cargo.toml', { mtime_ms: 1_000_000 });
	const result = await bump_cargo_version(dir, '0.1.1');
	strictEqual(result.ok, false);
	strictEqual(await dir.read_text('Cargo.toml'), text);
	strictEqual((await dir.stat('Cargo.toml'))!.mtime_ms, 1_000_000);
});

test("the changelog stamp writes the real changelog's stamped text once, then nothing", async () => {
	const { dir, changelog } = await copy_checkout();
	await using _ = dir;
	const next = version_bump(cargo_workspace_version(await dir.read_text('Cargo.toml'))!, 'patch')!;
	const stamped = changelog_stamp(changelog, next);
	ok(stamped.outcome === 'stamped' || stamped.outcome === 'stamped_without_marker');
	strictEqual(await bump_changelog(dir, next), stamped.outcome);
	strictEqual(await dir.read_text('CHANGELOG.md'), stamped.text);
	ok(stamped.text.includes(`\n## ${next}\n`));
	await dir.set_times('CHANGELOG.md', { mtime_ms: 2_000_000 });
	strictEqual(await bump_changelog(dir, next), 'already_stamped');
	strictEqual((await dir.stat('CHANGELOG.md'))!.mtime_ms, 2_000_000);
	strictEqual(await dir.read_text('CHANGELOG.md'), stamped.text);
});

test('a changelog with no Unreleased section is not written', async () => {
	await using dir = await fs.temp_dir('tsv_bump_test_');
	await dir.write('CHANGELOG.md', '# log\n\n## 0.1.0\n');
	await dir.set_times('CHANGELOG.md', { mtime_ms: 3_000_000 });
	strictEqual(await bump_changelog(dir, '0.1.1'), 'no_unreleased');
	strictEqual((await dir.stat('CHANGELOG.md'))!.mtime_ms, 3_000_000);
});

test('the lockfile update runs cargo offline in the checkout, and a failure carries stderr', async () => {
	await using dir = await fs.temp_dir('tsv_bump_test_');
	const calls: Array<unknown> = [];
	const cargo = (code: number) =>
		({
			out: async (args: ReadonlyArray<string>, options: unknown) => {
				calls.push([args, options]);
				return {
					kind: 'exited',
					code,
					stdout: '',
					stderr: ' error: no network \n',
					truncated: false
				};
			}
		}) as unknown as SpawnProgram;
	deepStrictEqual(await bump_lockfile(cargo(0), dir), { ok: true });
	deepStrictEqual(calls, [[['update', '--workspace', '--offline'], { cwd: dir }]]);
	deepStrictEqual(await bump_lockfile(cargo(101), dir), {
		ok: false,
		error: 'cargo update --workspace for Cargo.lock failed:\nerror: no network'
	});
	deepStrictEqual(await bump_lockfile(null, dir), {
		ok: false,
		error: 'cargo update --workspace for Cargo.lock failed:\ncargo: command not found'
	});
});

test('the sentinel: none, written with a newline, read trimmed, removed, removed again', async () => {
	await using dir = await fs.temp_dir('tsv_sentinel_test_');
	strictEqual(SENTINEL_PATH, '.publish-in-progress');
	strictEqual(await sentinel_read(dir), null);
	await sentinel_write(dir, '0.8.2');
	strictEqual(await dir.read_text(SENTINEL_PATH), '0.8.2\n');
	strictEqual(await sentinel_read(dir), '0.8.2');
	await sentinel_remove(dir);
	strictEqual(await dir.exists(SENTINEL_PATH), false);
	await sentinel_remove(dir);
});

test('a sentinel that cannot be read is an error, not a missing sentinel', async () => {
	await using dir = await fs.temp_dir('tsv_sentinel_test_');
	await dir.mkdir(SENTINEL_PATH);
	await rejects(sentinel_read(dir), (error) => is_fs_error(error) && error.kind !== 'not_found');
});
