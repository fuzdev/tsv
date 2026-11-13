import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { test } from 'vitest';

import { clear_package_state } from '../release/crates.ts';
import { open_package_state, type PackageStateEnv, to_registry_src_path } from './package_state.ts';

const OVERLAY = '-088a2fcb40cb7e05';

/** A temp tree with a checkout and a home beside it; `dirs` are made beneath it. */
const make_tree = async (dirs: Array<string>): Promise<FsDir & AsyncDisposable> => {
	const root = await fs.temp_dir('tsv_package_state_test_');
	for (const dir of ['checkout', 'home', ...dirs]) await root.mkdir(dir, { recursive: true });
	return root;
};

/** One cleanup over `root`'s checkout, with `env` relative to `root`. */
const clear = async (root: FsDir, env: PackageStateEnv, target_directory = 'checkout/target') => {
	const at = (path: string | undefined) =>
		path === undefined ? undefined : `${root.path}/${path}`;
	const absolute: PackageStateEnv = {
		HOME: at(env.HOME),
		CARGO_HOME: at(env.CARGO_HOME),
		CARGO_TARGET_DIR: at(env.CARGO_TARGET_DIR),
		CARGO_BUILD_TARGET_DIR: at(env.CARGO_BUILD_TARGET_DIR)
	};
	const target = `${root.path}/${target_directory}`;
	const registry_src = to_registry_src_path(absolute);
	return clear_package_state(
		await open_package_state({
			checkout: `${root.path}/checkout`,
			target_directory: target,
			env: absolute
		}),
		{ package_dir: `${target}/package`, registry_src, names: ['tsv_lang'], version: '0.7.1' }
	);
};

const HOME: PackageStateEnv = { HOME: 'home' };

test('a cargo home with no registry/src and no target directory is no error', async () => {
	await using root = await make_tree([]);
	deepStrictEqual(await clear(root, HOME), []);
	await root.mkdir('home/.cargo');
	deepStrictEqual(await clear(root, HOME), []);
});

test('the cleanup removes the package dir, the unpacks, and the emptied overlay dir', async () => {
	await using root = await make_tree([
		'checkout/target/package/x',
		`home/.cargo/registry/src/${OVERLAY}/tsv_lang-0.7.1/src`,
		`home/.cargo/registry/src/${OVERLAY}/tsv_lang-0.7.0`,
		'home/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tsv_lang-0.7.1'
	]);
	const src = `${root.path}/home/.cargo/registry/src`;
	deepStrictEqual(await clear(root, HOME), [
		`${root.path}/checkout/target/package`,
		`${src}/${OVERLAY}/tsv_lang-0.7.1`
	]);
	strictEqual(await root.stat('checkout/target/package'), null);
	ok(await root.exists('checkout/target'));
	ok(await root.exists(`home/.cargo/registry/src/${OVERLAY}/tsv_lang-0.7.0`));
	ok(await root.exists('home/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tsv_lang-0.7.1'));
	await root.remove(`home/.cargo/registry/src/${OVERLAY}/tsv_lang-0.7.0`);
	await root.mkdir(`home/.cargo/registry/src/${OVERLAY}/tsv_lang-0.7.1`);
	deepStrictEqual(await clear(root, HOME), [
		`${src}/${OVERLAY}/tsv_lang-0.7.1`,
		`${src}/${OVERLAY}`
	]);
	deepStrictEqual(await Array.fromAsync(root.list('home/.cargo/registry/src')), [
		{ name: 'index.crates.io-1949cf8c6b5b557f', kind: 'directory' }
	]);
});

test('a symlinked overlay dir is skipped, never followed', async () => {
	await using root = await make_tree(['elsewhere/tsv_lang-0.7.1', 'home/.cargo/registry/src']);
	await root.symlink(`home/.cargo/registry/src/${OVERLAY}`, `${root.path}/elsewhere`);
	deepStrictEqual(await clear(root, HOME), []);
	ok(await root.exists('elsewhere/tsv_lang-0.7.1'));
});

test('a target directory outside the checkout is refused, present or not, naming what moved it', async () => {
	await using root = await make_tree(['checkout/target/package']);
	for (const [env, cause] of [
		[{ ...HOME, CARGO_TARGET_DIR: 'elsewhere' }, 'CARGO_TARGET_DIR'],
		[{ ...HOME, CARGO_BUILD_TARGET_DIR: 'elsewhere' }, 'CARGO_BUILD_TARGET_DIR'],
		[{ ...HOME, CARGO_TARGET_DIR: 'elsewhere', CARGO_BUILD_TARGET_DIR: 'x' }, 'CARGO_TARGET_DIR'],
		[HOME, "a cargo config's build.target-dir"]
	] as const) {
		await rejects(clear(root, env, 'elsewhere'), (error) => {
			ok(error instanceof Error && !is_fs_error(error), String(error));
			ok(error.message.includes(`${root.path}/elsewhere/package is outside`), error.message);
			ok(error.message.includes(`: ${cause}`), error.message);
			return true;
		});
		await root.mkdir('elsewhere/package/x', { recursive: true });
	}
	ok(await root.exists('elsewhere/package/x'));
	ok(await root.exists('checkout/target/package'));
	// lexical: a path that climbs out and back in is outside too
	await rejects(clear(root, HOME, 'checkout/../elsewhere'), /is outside/);
});

test('a target directory symlinked out of the checkout is followed, as cargo names it', async () => {
	await using root = await make_tree(['elsewhere/package/x']);
	await root.symlink('checkout/target', `${root.path}/elsewhere`);
	deepStrictEqual(await clear(root, HOME), [`${root.path}/checkout/target/package`]);
	strictEqual(await root.stat('elsewhere/package'), null);
	ok(await root.exists('elsewhere'));
});

test('a cargo home elsewhere is read, and refused only when there is an unpack to remove', async () => {
	await using root = await make_tree([`cargo/registry/src/${OVERLAY}/tsv_lang-0.7.0`]);
	const env = { ...HOME, CARGO_HOME: 'cargo' };
	deepStrictEqual(await clear(root, env), []);
	await root.mkdir(`cargo/registry/src/${OVERLAY}/tsv_lang-0.7.1`);
	await rejects(clear(root, env), (error) => {
		ok(error instanceof Error && !is_fs_error(error), String(error));
		ok(error.message.includes(`${root.path}/cargo/registry/src is outside`), error.message);
		ok(error.message.includes(`: CARGO_HOME (${root.path}/cargo)`), error.message);
		return true;
	});
	ok(await root.exists(`cargo/registry/src/${OVERLAY}/tsv_lang-0.7.1`));
	// `$HOME/.cargo` named by CARGO_HOME is the grant itself
	await root.mkdir(`home/.cargo/registry/src/${OVERLAY}/tsv_lang-0.7.1`, { recursive: true });
	deepStrictEqual((await clear(root, { ...HOME, CARGO_HOME: 'home/.cargo' })).length, 2);
});

test('an empty CARGO_HOME is none, as cargo reads it: the cleanup clears $HOME/.cargo', async () => {
	await using root = await make_tree([`home/.cargo/registry/src/${OVERLAY}/tsv_lang-0.7.1`]);
	const env: PackageStateEnv = { HOME: `${root.path}/home`, CARGO_HOME: '' };
	const registry_src = to_registry_src_path(env);
	strictEqual(registry_src, `${root.path}/home/.cargo/registry/src`);
	const target = `${root.path}/checkout/target`;
	deepStrictEqual(
		await clear_package_state(
			await open_package_state({
				checkout: `${root.path}/checkout`,
				target_directory: target,
				env
			}),
			{ package_dir: `${target}/package`, registry_src, names: ['tsv_lang'], version: '0.7.1' }
		),
		[`${registry_src}/${OVERLAY}/tsv_lang-0.7.1`, `${registry_src}/${OVERLAY}`]
	);
});

test('registry/src that is a file fails the cleanup', async () => {
	await using root = await make_tree(['home/.cargo/registry']);
	await root.write('home/.cargo/registry/src', '');
	await rejects(clear(root, HOME), (error) => is_fs_error(error, 'not_a_directory'));
});

test('a name that climbs out of registry/src is refused, whether or not it exists', async () => {
	await using root = await make_tree(['home/.cargo/registry/src', 'home/.cargo/registry/x']);
	const state = await open_package_state({
		checkout: `${root.path}/checkout`,
		target_directory: `${root.path}/checkout/target`,
		env: { HOME: `${root.path}/home` }
	});
	ok(state.registry_src !== null);
	for (const name of ['../x', '../absent']) {
		await rejects(state.registry_src.remove(name, { recursive: true }), (error) =>
			is_fs_error(error, 'outside_root')
		);
	}
	ok(await root.exists('home/.cargo/registry/x'));
});

test('a target directory elsewhere beneath the checkout target/ is refused too', async () => {
	await using root = await make_tree(['checkout/target/x/package/y']);
	await rejects(
		clear(root, { ...HOME, CARGO_TARGET_DIR: 'checkout/target/x' }, 'checkout/target/x'),
		(error) => {
			ok(error instanceof Error && !is_fs_error(error), String(error));
			ok(
				error.message.includes(`${root.path}/checkout/target/x/package is outside`),
				error.message
			);
			return true;
		}
	);
	ok(await root.exists('checkout/target/x/package/y'));
});
