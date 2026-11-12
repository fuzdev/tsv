/**
 * `bench:harvest:wpt`'s root (`benches/js/diagnostics/wpt_css_harvest.ts`). Its reach: the
 * source tree (`../wpt/css`, or the path given), read-only; the checkout's files, read-only,
 * and the stamp it writes beneath `benches/js/.cache`; the output directory
 * (`benches/js/.cache/wpt_css`, or the `--out` given, which `to_wpt_out` accepts only strictly
 * beneath `benches/js/.cache`), reached through a handle on that directory, which it clears and
 * rewrites; and `git` with git's local environment, for the source's commit. It runs no other
 * program. Run it from the checkout's root, where the default paths and the sibling checkouts
 * resolve.
 *
 * @module
 */

import { basename, dirname, resolve } from 'node:path';

import { fs } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { process } from '@fuzdev/gro/process.ts';

import { CHECKOUT_PATH, open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_wpt_css_harvest, type WptOut } from '../diagnostics/wpt_css_harvest.ts';
import { cache_files, HARVEST_CACHE_DIR, stamp_git } from './harvest_deps.ts';

const checkout = await open_checkout();
// opened on each call: the harvest makes the directory before its first write beneath it
const cache = () => checkout.dir(HARVEST_CACHE_DIR);

const code = await run_wpt_css_harvest(
	{
		git: stamp_git(checkout),
		files: cache_files(checkout),
		open_source: async (path) => {
			try {
				return (await fs.open_dir(resolve(CHECKOUT_PATH, path))).readonly();
			} catch (error) {
				if (is_fs_error(error, 'not_found')) return null;
				throw error;
			}
		},
		open_out: async (path): Promise<WptOut | null> => {
			// `to_wpt_out` accepted only a path strictly beneath the cache
			const rel = path.slice(HARVEST_CACHE_DIR.length + 1);
			try {
				// the cache's handle refuses a path through a symlink that leaves it
				await cache().stat(rel);
			} catch (error) {
				if (is_fs_error(error, 'outside_root')) return null;
				if (!is_fs_error(error, 'not_found')) throw error;
			}
			const parent = dirname(rel);
			const name = basename(rel);
			return {
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
		},
		stdout: process.stdout
	},
	process.args
);
process.exit(code);
