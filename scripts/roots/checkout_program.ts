/**
 * The programs a root makes beneath the checkout (`checkout.ts`), pinned for the handle's life,
 * whatever the working directory. Apart from `checkout.ts` so a root that only reads the
 * checkout links without `spawn`.
 *
 * @module
 */

import { fs, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { spawn, type SpawnProgram, type SpawnProgramOptions } from '@fuzdev/gro/spawn.ts';

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
			linked.set(name, (await fs.open_dir(`${checkout.path}/${name}`)).readonly());
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
