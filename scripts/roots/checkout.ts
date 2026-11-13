/**
 * The checkout the gro-hosted roots in `scripts/roots/` work in, found from this module's own
 * location, never the working directory, with its `target/`, the programs beneath it, and the
 * sibling checkouts beside it.
 *
 * @module
 */

import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { process } from '@fuzdev/gro/process.ts';
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
 * Makes the program at `path` beneath the checkout (`target/corpus/tsv`), pinned for the
 * handle's life, whatever the working directory.
 * @throws `SpawnError` - `not_found` when nothing is there, naming the name it looked for and
 *   the directory beneath which; `not_executable`, or `not_granted` for a path that leads out
 *   of the checkout, as `spawn.program_in`
 */
export type CheckoutProgram = (path: string, options?: SpawnProgramOptions) => SpawnProgram;

/**
 * The checkout's top-level directories a setup may make a symlink to a directory elsewhere,
 * absolute or relative: cargo's `target` (a relocated build directory) and npm's
 * `node_modules` (an install a second tree shares).
 */
const LINKABLE_DIRS: ReadonlyArray<string> = ['target', 'node_modules'];

/**
 * Opens the checkout for making programs beneath it with gro's `spawn.program_in`. A path whose
 * first segment is one of `LINKABLE_DIRS` resolves beneath that directory, opened now by its
 * path, so one that is a symlink to a directory elsewhere is followed, as `open_target` follows
 * `target`; one that is absent or a file is never made, and its programs are then not found
 * beneath the checkout. Every other path, and the rest of the path beneath one of those, follows
 * `program_in`'s rules: a relative symlink that stays inside is followed
 * (`node_modules/.bin/tsc`), and an absolute one, a binary that is itself an absolute symlink
 * included, is refused as `not_granted`.
 */
export const open_checkout_program = async (checkout: FsReadDir): Promise<CheckoutProgram> => {
	const linked = new Map<string, FsReadDir>();
	for (const name of LINKABLE_DIRS) {
		try {
			linked.set(name, (await fs.open_dir(join(checkout.path, name))).readonly());
		} catch (error) {
			if (!is_fs_error(error, 'not_found') && !is_fs_error(error, 'not_a_directory')) throw error;
		}
	}
	return (path, options) => {
		const slash = path.indexOf('/');
		const dir = slash === -1 ? undefined : linked.get(path.slice(0, slash));
		return dir === undefined
			? spawn.program_in(checkout, path, options)
			: spawn.program_in(dir, path.slice(slash + 1), options);
	};
};

/**
 * What a root whose reads resolve against the working directory (the corpus loader's paths, a
 * path argument) says when it is started anywhere but the checkout's root, or `null` from there.
 * @param checkout_path - the checkout's path, as `open_checkout` opened it
 * @param cwd - the working directory, as gro's `fs.project` names it
 */
export const to_cwd_refusal = (checkout_path: string, cwd: string): string | null =>
	cwd === checkout_path
		? null
		: `✗ run this from the checkout's root, ${checkout_path}, not ${cwd}: ` +
			'the corpus and the paths it is given resolve against the working directory';

/**
 * Ends the process with exit 1, printing `to_cwd_refusal`'s message, unless the working
 * directory is the checkout's root. A root whose reads resolve against the working directory
 * calls it first, before any guard or loader runs.
 */
export const exit_unless_checkout_cwd = (checkout: FsReadDir): void => {
	const refusal = to_cwd_refusal(checkout.path, fs.project.path);
	if (refusal === null) return;
	console.error(refusal);
	process.exit(1);
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
