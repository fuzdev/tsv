/**
 * The gzipped size of a file as `gzip -c` counts it, for the npm packaging scripts' size
 * lines (`patch_npm_package.ts`, `publish.ts`) and the branch report's size legs.
 *
 * @module
 */

/**
 * Return the gzipped size of a file. Shells out to `gzip -c` so the number
 * matches `gzip -c | wc -c` (Deno's CompressionStream uses a different
 * default level and reports ~2% high). Requires `--allow-run=gzip`.
 */
export async function gzip_size(path: string): Promise<number> {
	const output = await new Deno.Command('gzip', {
		args: ['-c', path],
		stdout: 'piped'
	}).output();
	return output.stdout.length;
}
