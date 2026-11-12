/**
 * What the harvests' roots share, not a root itself: `git` for the stamps' checkout ids, the
 * checkout's files with writes confined beneath `benches/js/.cache`, and read-only handles on
 * the sibling checkouts a harvest reads. `css_over_acceptance.ts`'s pin leg is stamped the
 * same way, so its root binds these too, and `audit:corpus`'s root reads `../prettier` through
 * `sibling_reads`.
 *
 * @module
 */

import { resolve } from 'node:path';

import { fs, type FsDir, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CHECKOUT_PATH } from '../../../scripts/roots/checkout.ts';
import type { StampGit, StampWrites } from '../lib/harvest_stamp.ts';

/** The one directory a harvest writes beneath, as every cache and stamp path spells it. */
export const HARVEST_CACHE_DIR = 'benches/js/.cache';

/**
 * `git` with git's local environment, run in the checkout, so a stamp's `-C ../<sibling>`
 * names the checkout's sibling. With no `git` on `PATH` every id reads as none, as for a
 * checkout that isn't a repo.
 */
export const stamp_git = (checkout: FsReadDir): StampGit => {
	const git = spawn.find('git', { pass_env: GIT_LOCAL_PASS_ENV });
	if (!git) return { out: () => Promise.reject(new Error('no `git` on PATH')) };
	return { out: (args) => git.out(args, { cwd: checkout }) };
};

/** The path beneath `HARVEST_CACHE_DIR` that `path` names, refusing any other. */
const cache_relative = (path: string): string => {
	if (!path.startsWith(`${HARVEST_CACHE_DIR}/`)) {
		throw new Error(`a harvest writes beneath ${HARVEST_CACHE_DIR} only, not ${path}`);
	}
	return path.slice(HARVEST_CACHE_DIR.length + 1);
};

/**
 * The checkout's files by project-root-relative path: any read, through a read-only handle,
 * and writes and directories beneath `HARVEST_CACHE_DIR` alone, through a handle on it, opened
 * on each call, since the directory may not exist until a harvest makes it (it is made, with
 * its parents, through the checkout).
 */
export const cache_files = (checkout: FsDir): StampWrites => {
	const readonly = checkout.readonly();
	return {
		exists: (path) => readonly.exists(path),
		read_text: (path) => readonly.read_text(path),
		write: async (path, data) => {
			const rel = cache_relative(path);
			await checkout.dir(HARVEST_CACHE_DIR).write(rel, data);
		},
		mkdir: async (path, options) => {
			const rel = path === HARVEST_CACHE_DIR ? null : cache_relative(path);
			await checkout.mkdir(HARVEST_CACHE_DIR, { recursive: true });
			if (rel !== null) await checkout.dir(HARVEST_CACHE_DIR).mkdir(rel, options);
		}
	};
};

/** A sibling checkout's files, read-only, by paths beneath it. */
export interface SiblingReads {
	/** Whether `path` exists beneath the checkout; `false` when the checkout itself is absent. */
	exists(path: string): Promise<boolean>;
	read_text(path: string): Promise<string>;
	list(path: string): AsyncIterable<{ name: string; kind: string }>;
}

/**
 * The checkout at `path` (relative to this checkout, as the tools spell it), read-only, opened
 * once, on the first read, so an absent checkout fails the read the tool reports. It opens by
 * its path, so a checkout that is a symlink, relative or absolute, to a directory elsewhere is
 * followed, and a dangling one reads as absent; paths beneath it resolve within it.
 */
export const sibling_reads = (path: string): SiblingReads => {
	let dir: Promise<FsReadDir> | undefined;
	const open = (): Promise<FsReadDir> =>
		(dir ??= fs.open_dir(resolve(CHECKOUT_PATH, path)).then((opened) => opened.readonly()));
	return {
		exists: async (beneath) => {
			let opened: FsReadDir;
			try {
				opened = await open();
			} catch {
				return false;
			}
			return opened.exists(beneath);
		},
		read_text: async (beneath) => (await open()).read_text(beneath),
		list: async function* (beneath) {
			yield* (await open()).list(beneath);
		}
	};
};
