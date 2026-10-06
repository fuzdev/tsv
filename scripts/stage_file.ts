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
 * @module
 */

/** Copy `from` to `to` (mode preserved, as `Deno.copyFileSync` does) and set the copy's
 * access and modification times to now. */
export function stage_file(from: string, to: string): void {
	Deno.copyFileSync(from, to);
	const now = new Date();
	Deno.utimeSync(to, now, now);
}
