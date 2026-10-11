/**
 * The resume sentinel: a gitignored file naming the version a wetrun bumped to, written right
 * after the bump and removed only once the release commit, tag and push have landed. While it
 * names the checkout's version, a wetrun resumes that release instead of starting a new one.
 *
 * @module
 */

export const SENTINEL_PATH = '.publish-in-progress';

/** The version the sentinel names, or null when there is none. */
export const sentinel_read = async (): Promise<string | null> => {
	try {
		return (await Deno.readTextFile(SENTINEL_PATH)).trim();
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;
		return null;
	}
};

export const sentinel_write = (version: string): Promise<void> =>
	Deno.writeTextFile(SENTINEL_PATH, `${version}\n`);

/** Remove the sentinel; a missing one is not an error. */
export const sentinel_remove = async (): Promise<void> => {
	try {
		await Deno.remove(SENTINEL_PATH);
	} catch (error) {
		if (!(error instanceof Deno.errors.NotFound)) throw error;
	}
};
