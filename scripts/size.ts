/**
 * Size helpers shared by the npm packaging scripts — `patch_npm_package.ts`,
 * `build_napi_packages.ts`, `validate_artifacts.ts`, `validate_napi_artifact.ts`,
 * and `publish.ts`. Console output only: no size formatted here reaches a published
 * package, and no gate compares one (bounds are raw byte counts).
 */

/**
 * Format a byte count for humans (B / KB / MB), decimal (1000-based) — the same
 * convention as the benchmark harness's formatters (`benches/js/lib/corpus.ts`'s
 * `format_mb`, `benches/js/lib/binary_sizes.ts`'s `format_bytes`). The publish log
 * and the report's BINARY SIZES table size the SAME artifacts, so a `MB` here has
 * to mean what a `MB` there means; binary units under the same label made one
 * bundle read as 2.39 MB in the publish output and 2.5 MB in the published report.
 *
 * Display only. Every size BOUND in `validate_artifacts.ts` / `validate_napi_artifact.ts`
 * is a raw byte count and is compared as one, so this can never decide a gate.
 */
export function format_size(bytes: number): string {
	if (bytes < 1000) return `${bytes} B`;
	if (bytes < 1_000_000) return `${(bytes / 1000).toFixed(1)} KB`;
	return `${(bytes / 1_000_000).toFixed(2)} MB`;
}

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
