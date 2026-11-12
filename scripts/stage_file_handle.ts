/**
 * `stage_file`'s binding over gro's fs handles (`scripts/stage_file.ts`), for a staging that
 * reads its sources through one handle and writes beneath another: the wasm package patcher
 * reads the checkout and writes beneath `crates/tsv_wasm/pkg/` (`scripts/patch_npm_package.ts`).
 * The N-API staging, which CI runs on Windows with no `node_modules`, binds `node:fs` instead
 * (`scripts/roots/build_napi_packages.ts`).
 *
 * @module
 */

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';

import type { StageFileDeps } from './stage_file.ts';

/**
 * The deps for staging a file from `sources` into `staging`, each path relative to its own
 * handle. A handle copies only beneath itself, so the copy is a read through one and a write
 * through the other, with the source's mode set on the copy after, as a file copy keeps it.
 */
export const stage_file_between = (sources: FsReadDir, staging: FsDir): StageFileDeps => ({
	copy: async (from, to) => {
		const bytes = await sources.read(from);
		const info = await sources.stat(from);
		if (info === null) throw new Error(`${from} was removed while it was staged`);
		await staging.write(to, bytes);
		await staging.set_mode(to, info.mode);
	},
	set_times: (path, times) => staging.set_times(path, times)
});
