/**
 * The one file copy the npm staging scripts use (`scripts/patch_npm_package.ts`,
 * `scripts/build_napi_packages.ts`): copy, then date the copy at the staging.
 *
 * `scripts/check_staged_freshness.ts` reads a staged file's mtime as WHEN IT WAS
 * STAGED and compares it against the sources feeding it. A plain copy does not
 * promise that reading: Windows' `CopyFile` carries the source's last-write time
 * onto the copy, so a file staged verbatim is dated like its source — and on a
 * fresh checkout every source checked out after that one is newer, which grades
 * a staging made seconds ago as stale. Stamping the copy makes the mtime mean the
 * same thing on every platform.
 *
 * Imports nothing, so the Windows staging entry (`scripts/roots/build_napi_packages.ts`)
 * reaches it with no `node_modules`. Two bindings: that entry's, over `node:fs`, and the
 * wasm patcher's, over gro's fs handles (`scripts/stage_file_handle.ts`).
 *
 * @module
 */

/** The times a staging sets, in milliseconds since the epoch. */
export interface StageTimes {
	mtime_ms: number;
	atime_ms?: number;
}

/** What `stage_file` reaches: a copy that keeps the source's mode, and a file's dates. */
export interface StageFileDeps {
	copy: (from: string, to: string) => Promise<void>;
	set_times: (path: string, times: StageTimes) => Promise<void>;
}

/** Copy `from` to `to`, the copy keeping the source's mode, then set the copy's access and
 * modification times to now. */
export async function stage_file(deps: StageFileDeps, from: string, to: string): Promise<void> {
	await deps.copy(from, to);
	const now = Date.now();
	await deps.set_times(to, { mtime_ms: now, atime_ms: now });
}
