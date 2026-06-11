/**
 * Size helpers shared by the npm packaging scripts
 * (`patch_npm_package.ts`, `validate_artifacts.ts`, `publish.ts`).
 */

/** Format a byte count for humans (B / KB / MB). */
export function format_size(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

/**
 * Return the gzipped size of a file. Shells out to `gzip -c` so the number
 * matches `gzip -c | wc -c` (Deno's CompressionStream uses a different
 * default level and reports ~2% high). Requires `--allow-run=gzip`.
 */
export async function gzip_size(path: string): Promise<number> {
	const output = await new Deno.Command('gzip', {
		args: ['-c', path],
		stdout: 'piped',
	}).output();
	return output.stdout.length;
}
