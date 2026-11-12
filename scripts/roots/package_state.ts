/**
 * The two directories `publish:crates`' packaging cleanup writes beneath
 * (`clear_package_state` in `scripts/release/crates.ts`), granted at exactly that reach:
 * `target/package` in the checkout, and `$HOME/.cargo/registry/src`.
 *
 * The directories the cleanup is asked to clear come from cargo: the target directory is
 * `cargo metadata`'s `target_directory` (which `CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR`,
 * and a config's `build.target-dir` move) and the cargo home is `CARGO_HOME`. One outside its
 * grant is refused, not skipped, so a cleanup that can't run fails the publish rather than
 * leaving an earlier run's sources for cargo to verify. Each refusal names the variable that
 * moved the directory, and comes from the removal itself: a target directory elsewhere fails
 * every run, since the cleanup always removes its `package/`, and a cargo home elsewhere only
 * when it holds an unpack to remove, its listing read through a read-only handle. Whether a
 * directory is inside its grant is lexical, on the paths as cargo and the environment name
 * them, and a symlink beneath a grant is followed.
 *
 * @module
 */

import { basename, dirname, isAbsolute, join, relative, resolve } from 'node:path';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';

import type { PackageStateDeps, RegistrySrcDir } from '../release/crates.ts';

/** The variables that say where the target directory and the cargo home are. */
export interface PackageStateEnv {
	HOME?: string | undefined;
	CARGO_HOME?: string | undefined;
	CARGO_TARGET_DIR?: string | undefined;
	CARGO_BUILD_TARGET_DIR?: string | undefined;
}

/** The variable names `PackageStateEnv` reads. */
export const PACKAGE_STATE_ENV_NAMES = [
	'HOME',
	'CARGO_HOME',
	'CARGO_TARGET_DIR',
	'CARGO_BUILD_TARGET_DIR'
] as const;

/** `$CARGO_HOME/registry/src`, or `$HOME/.cargo/registry/src` with no `CARGO_HOME`, as the
 * cleanup names it. */
export const to_registry_src_path = (env: PackageStateEnv): string =>
	join(env.CARGO_HOME ?? join(env.HOME ?? '', '.cargo'), 'registry', 'src');

/** Whether `path` is `dir` or beneath it, lexically. */
const is_within = (dir: string, path: string): boolean => {
	const rel = relative(dir, path);
	return rel === '' || (rel !== '..' && !rel.startsWith('../') && !isAbsolute(rel));
};

/** What moved the target directory, as cargo ranks them. */
const target_dir_cause = (env: PackageStateEnv): string =>
	env.CARGO_TARGET_DIR !== undefined
		? `CARGO_TARGET_DIR (${env.CARGO_TARGET_DIR})`
		: env.CARGO_BUILD_TARGET_DIR !== undefined
			? `CARGO_BUILD_TARGET_DIR (${env.CARGO_BUILD_TARGET_DIR})`
			: "a cargo config's build.target-dir";

/** What moved the cargo home: `CARGO_HOME`, or with none an unset `HOME`. */
const cargo_home_cause = (env: PackageStateEnv): string =>
	env.CARGO_HOME !== undefined ? `CARGO_HOME (${env.CARGO_HOME})` : `HOME (${env.HOME ?? 'unset'})`;

/**
 * Opens the cleanup's two directories for one run of it.
 * @param options - `checkout` is the checkout's absolute path, which the relative paths
 *   resolve against; `target_directory` is `cargo metadata`'s
 * @returns the cleanup's deps: the removal of `package/` beneath the target directory, and
 *   `registry/src`, `null` when it doesn't exist
 * @throws `FsError` - when `registry/src` is something other than a directory
 */
export const open_package_state = async (options: {
	checkout: string;
	target_directory: string;
	env: PackageStateEnv;
}): Promise<PackageStateDeps> => {
	const { checkout, env } = options;
	const package_grant = resolve(checkout, 'target/package');
	// the shell form `$HOME/.cargo/registry/src`, so an unset `HOME` grants `/.cargo/registry/src`
	const registry_grant = resolve(checkout, `${env.HOME ?? ''}/.cargo/registry/src`);

	const package_dir = resolve(checkout, options.target_directory, 'package');
	const remove_package_dir = is_within(package_grant, package_dir)
		? async (): Promise<void> => {
				const parent = await fs.open_dir(dirname(package_dir));
				await parent.remove(basename(package_dir), { recursive: true });
			}
		: (): Promise<void> =>
				Promise.reject(
					new Error(
						`${package_dir} is outside ${package_grant}, the one package directory ` +
							`publish:crates may clear: ${target_dir_cause(env)} moved the target directory ` +
							`to ${options.target_directory}`
					)
				);

	const registry_src = resolve(checkout, to_registry_src_path(env));
	let dir: FsDir;
	try {
		dir = await fs.open_dir(registry_src);
	} catch (error) {
		if (!is_fs_error(error, 'not_found')) throw error;
		return { remove_package_dir, registry_src: null };
	}
	if (is_within(registry_grant, registry_src)) return { remove_package_dir, registry_src: dir };
	const read = dir.readonly();
	const refused: RegistrySrcDir = {
		list: (path) => read.list(path),
		stat: (path, stat_options) => read.stat(path, stat_options),
		remove: () =>
			Promise.reject(
				new Error(
					`${registry_src} is outside ${registry_grant}, the one registry publish:crates may ` +
						`clear: ${cargo_home_cause(env)} moved the cargo home`
				)
			)
	};
	return { remove_package_dir, registry_src: refused };
};
