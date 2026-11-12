/**
 * The mtime walk's `SourceTree` (`benches/js/lib/source_mtime.ts`) over a gro directory handle,
 * for the gro-hosted scripts that date an artifact against its sources: the build-side skip
 * (`scripts/run_if_stale.ts`) and artifact validation (`scripts/validate_artifacts.ts`). The
 * scripts on Node's own modules read through `node_source_tree` instead.
 *
 * @module
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import { fs_search } from '@fuzdev/gro/fs_search.ts';

import type { SourceTree } from '../benches/js/lib/source_mtime.ts';

/** A `SourceTree` over a directory handle: a symlink is listed as a file and followed by
 * `stat`, never descended into, and one the handle can't follow (an absolute target, or one
 * climbing out of the checkout) fails the walk with `outside_root`. */
export const handle_source_tree = (dir: FsReadDir): SourceTree => ({
	stat: async (path) => {
		const info = await dir.stat(path);
		return info === null ? null : { mtime_ms: info.mtime_ms, is_file: info.kind === 'file' };
	},
	list: async (path) => (await fs_search(dir, { path, sort: false })).map((found) => found.path)
});
