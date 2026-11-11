/**
 * The checkout the gro-hosted roots in `scripts/roots/` work in, found from this module's own
 * location, never the working directory, with its `target/` and the programs beneath it.
 *
 * A program named by a path (`target/corpus/tsv`) is resolved by gro's spawn host module
 * against its project root, the working directory, so `checkout_program` refuses to make one
 * unless the working directory is this checkout: the binary pinned is then the checkout's own.
 * The npm scripts and `deno task` both run from the checkout's root.
 *
 * @module
 */

import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { fs, type FsDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { spawn, type SpawnProgram, type SpawnProgramOptions } from '@fuzdev/gro/spawn.ts';

/** The checkout's absolute path, two directories above this module. */
export const CHECKOUT_PATH = fileURLToPath(new URL('../../', import.meta.url)).replace(/\/$/, '');

/** Opens the checkout. */
export const open_checkout = (): Promise<FsDir> => fs.open_dir(CHECKOUT_PATH);

/**
 * Opens the checkout's `target/`, made when missing. It opens by its path, so a `target` that
 * is a symlink to a directory elsewhere (as cargo allows, and as a relocated build directory
 * is) is followed, where a handle beneath the checkout refuses an absolute symlink.
 */
export const open_target = async (checkout: FsDir): Promise<FsDir> => {
	const path = join(checkout.path, 'target');
	try {
		return await fs.open_dir(path);
	} catch (error) {
		if (!is_fs_error(error, 'not_found')) throw error;
	}
	await checkout.mkdir('target');
	return fs.open_dir(path);
};

/**
 * The program at `path` beneath the checkout, pinned for the handle's life.
 * @throws `Error` - when the working directory isn't the checkout
 */
export const checkout_program = (
	checkout: FsDir,
	path: string,
	options?: SpawnProgramOptions
): SpawnProgram => {
	// `fs.project` is the working directory, the root gro's spawn resolves `path` against
	const cwd = fs.project.path;
	if (cwd !== checkout.path) {
		throw new Error(
			`run this from the checkout's root (${checkout.path}), not ${cwd}: ` +
				`${path} resolves against the working directory`
		);
	}
	return spawn.program(path, options);
};
