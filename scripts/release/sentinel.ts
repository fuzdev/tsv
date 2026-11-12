/**
 * The resume sentinel: a gitignored file in the checkout naming the version a wetrun bumped to,
 * written right after the bump and removed only once the release commit, tag and push have
 * landed. While it names the checkout's version, a wetrun resumes that release instead of
 * starting a new one.
 *
 * @module
 */

import type { FsDir, FsReadDir } from '@fuzdev/gro/fs.ts';

/** The sentinel's path beneath the checkout. */
export const SENTINEL_PATH = '.publish-in-progress';

/** The version the sentinel names, or null when there is none. Only a missing file reads as
 * none; any other failure to read it throws. */
export const sentinel_read = async (checkout: FsReadDir): Promise<string | null> => {
	const text = await checkout.read_text_if_exists(SENTINEL_PATH);
	return text === null ? null : text.trim();
};

/** Write the sentinel naming `version`. */
export const sentinel_write = (checkout: FsDir, version: string): Promise<void> =>
	checkout.write(SENTINEL_PATH, `${version}\n`);

/** Remove the sentinel; a missing one is not an error. */
export const sentinel_remove = (checkout: FsDir): Promise<void> =>
	checkout.remove(SENTINEL_PATH, { missing_ok: true });
