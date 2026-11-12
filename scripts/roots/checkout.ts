/**
 * The checkout the gro-hosted roots in `scripts/roots/` work in, found from this module's own
 * location, never the working directory, with its `target/`, the programs beneath it, and the
 * sibling checkouts beside it.
 *
 * A program named by a path (`target/corpus/tsv`) is resolved by gro's spawn host module
 * against its project root, the working directory, so `checkout_program` refuses to make one
 * unless the working directory is this checkout: the binary pinned is then the checkout's own.
 * The npm scripts and `deno task` both run from the checkout's root.
 *
 * @module
 */

import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
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

/**
 * The sibling checkout at `path` (relative to this checkout, as the tools spell it, or
 * absolute), read-only, or `null` when it can't be opened: absent, a dangling symlink, or not
 * a directory. It opens by its path, as a path from outside the checkout, so a checkout that
 * is a symlink, relative or absolute or a chain of them, to a directory anywhere is followed;
 * paths beneath it resolve within it.
 */
export const open_sibling = async (path: string): Promise<FsReadDir | null> => {
	try {
		return (await fs.open_dir(resolve(CHECKOUT_PATH, path))).readonly();
	} catch {
		return null;
	}
};

/** A sibling checkout's files, read-only, by paths beneath it. */
export interface SiblingReads {
	/** The checkout's handle, or `null` when it is absent (`open_sibling`). */
	open(): Promise<FsReadDir | null>;
	/**
	 * Whether `path` exists beneath the checkout: `false` when the checkout is absent, and when
	 * `path` is missing or passes through a file.
	 * @throws `Error` - when `path` reaches through a symlink that is absolute or climbs out of
	 * the checkout, wherever it lands, which is never followed and never read as missing, since a
	 * tool hands the path itself to a program that would follow it
	 */
	exists(path: string): Promise<boolean>;
	read_text(path: string): Promise<string>;
	list(path: string): AsyncIterable<{ name: string; kind: string }>;
}

/**
 * The checkout at `path` (`open_sibling`), opened once, on the first read, so an absent
 * checkout fails the read the tool reports.
 */
export const sibling_reads = (path: string): SiblingReads => {
	let dir: Promise<FsReadDir | null> | undefined;
	const open = (): Promise<FsReadDir | null> => (dir ??= open_sibling(path));
	const opened = async (): Promise<FsReadDir> => {
		const handle = await open();
		if (handle === null) throw new Error(`no sibling checkout at ${path}`);
		return handle;
	};
	return {
		open,
		exists: async (beneath) => {
			const handle = await open();
			if (handle === null) return false;
			try {
				return await handle.exists(beneath);
			} catch (error) {
				if (is_fs_error(error, 'not_a_directory')) return false;
				if (is_fs_error(error, 'outside_root')) {
					throw new Error(
						`${path}/${beneath} reaches through a symlink that is absolute or climbs out of ${path}, which is never followed; make it relative within the checkout, or link the checkout itself instead`,
						{ cause: error }
					);
				}
				throw error;
			}
		},
		read_text: async (beneath) => (await opened()).read_text(beneath),
		list: async function* (beneath) {
			yield* (await opened()).list(beneath);
		}
	};
};
