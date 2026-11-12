/**
 * What the harvests' roots share, not a root itself: `git` for the stamps' checkout ids, the
 * checkout's files with writes confined beneath `benches/js/.cache`, and read-only handles on
 * the sibling checkouts a harvest reads (`sibling_reads`, beside the checkout's own module).
 * `css_over_acceptance.ts`'s pin leg is stamped the same way, so its root binds these too.
 *
 * @module
 */

import { basename, dirname, join } from 'node:path';

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

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

/** A directory beneath `HARVEST_CACHE_DIR` that a harvest clears and rewrites, by paths beneath it. */
export interface CacheOutDir {
	/** The absolute path it resolves to. */
	path: string;
	/** Removes the directory and everything in it, if it exists; a symlink, the link itself. */
	remove(): Promise<void>;
	/** Makes `path` beneath the directory (`''`, the directory itself), with its parents. */
	mkdir(path: string): Promise<void>;
	write(path: string, data: string): Promise<void>;
}

/**
 * Opens the output directory at `path`, a normalized project-root-relative path strictly
 * beneath `HARVEST_CACHE_DIR`, through a handle on the cache, or says why it's refused: a
 * directory on its path beneath the cache is a symlink, which would lead the removal into
 * another harvest's files, or the path reaches out of the cache through its final symlink. It
 * reads nothing but those entries' kinds and removes nothing until `remove`.
 */
export const open_cache_out = async (
	checkout: FsDir,
	path: string
): Promise<CacheOutDir | { refused: string }> => {
	const rel = cache_relative(path);
	// opened on each call: the harvest makes the directory before its first write beneath it
	const cache = () => checkout.dir(HARVEST_CACHE_DIR);
	const segments = rel.split('/');
	for (let i = 1; i < segments.length; i++) {
		const beneath = segments.slice(0, i).join('/');
		const stat = await checkout.stat(`${HARVEST_CACHE_DIR}/${beneath}`, { follow: false });
		if (stat === null) break;
		if (stat.kind === 'symlink') {
			return { refused: `${HARVEST_CACHE_DIR}/${beneath} is a symlink` };
		}
	}
	// the cache's real path, or where it will be made
	let cache_path = join(checkout.path, HARVEST_CACHE_DIR);
	try {
		const dir = cache();
		cache_path = dir.path;
		// the cache's handle refuses a final symlink that leaves it
		await dir.stat(rel);
	} catch (error) {
		if (is_fs_error(error, 'outside_root')) {
			return { refused: `it reaches out of ${HARVEST_CACHE_DIR} through a symlink` };
		}
		if (!is_fs_error(error, 'not_found')) throw error;
	}
	const parent = dirname(rel);
	const name = basename(rel);
	return {
		path: join(cache_path, rel),
		remove: async () => {
			try {
				await (parent === '.' ? cache() : cache().dir(parent)).remove(name, {
					recursive: true,
					missing_ok: true
				});
			} catch (error) {
				// the parent itself absent: nothing to remove
				if (!is_fs_error(error, 'not_found')) throw error;
			}
		},
		mkdir: async (beneath) => {
			await checkout.mkdir(HARVEST_CACHE_DIR, { recursive: true });
			await cache().mkdir(beneath === '' ? rel : `${rel}/${beneath}`, { recursive: true });
		},
		write: (beneath, data) => cache().dir(rel).write(beneath, data)
	};
};
