/**
 * The gzipped size of a file as `gzip -c` counts it, for the npm packaging scripts' size
 * lines (`patch_npm_package.ts`, `publish.ts`) and the branch report's size legs.
 *
 * Imports nothing: each caller hands it the `gzip` program its runtime runs.
 *
 * @module
 */

/** The `gzip` program, its output captured as bytes — a gro `SpawnProgram` satisfies it. */
export interface GzipProgram {
	out_bytes: (args: ReadonlyArray<string>) => Promise<{ stdout: Uint8Array }>;
}

/**
 * Return the gzipped size of a file: the byte length of `gzip -c`'s output, so the number
 * matches `gzip -c | wc -c` (a JS deflate, `node:zlib` or `CompressionStream`, writes a
 * different header and stream, and its count differs). A `gzip` that fails counts what it
 * wrote.
 * @param path - the file, absolute or relative to the program's working directory
 */
export async function gzip_size(gzip: GzipProgram, path: string): Promise<number> {
	const output = await gzip.out_bytes(['-c', path]);
	return output.stdout.length;
}
