/**
 * `bench:harvest:wpt`'s root (`benches/js/diagnostics/wpt_css_harvest.ts`). Its reach: the
 * source tree (`../wpt/css`, or the path given), read-only; the checkout's files, read-only,
 * and the stamp it writes beneath `benches/js/.cache`; the output directory
 * (`benches/js/.cache/wpt_css`, or the `--out` given, which `to_wpt_out` accepts only strictly
 * beneath `benches/js/.cache`), reached through a handle on that directory (`open_cache_out`,
 * which refuses one with a symlink on its path), which it clears and rewrites; and `git` with git's local environment, for the source's commit. It runs no other
 * program. Run it from the checkout's root, where the default paths and the sibling checkouts
 * resolve.
 *
 * @module
 */

import { resolve } from 'node:path';

import { fs } from '@fuzdev/gro/fs.ts';
import { is_fs_error } from '@fuzdev/gro/fs_error.ts';
import { process } from '@fuzdev/gro/process.ts';

import { CHECKOUT_PATH, open_checkout } from '../../../scripts/roots/checkout.ts';
import { run_wpt_css_harvest } from '../diagnostics/wpt_css_harvest.ts';
import { cache_files, open_cache_out, stamp_git } from './harvest_deps.ts';

const checkout = await open_checkout();

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
		open_out: (path) => open_cache_out(checkout, path),
		stdout: process.stdout
	},
	process.args
);
process.exit(code);
