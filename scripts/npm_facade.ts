/**
 * The hand-written facade every tsv npm package publishes through, staged verbatim from
 * `crates/tsv_wasm/npm/` into the wasm packages (`scripts/patch_npm_package.ts`) and the
 * native `@fuzdev/tsv` (`scripts/build_napi_packages.ts`). Named once so a new facade file
 * reaches every package's copy step, `files` list and staging-freshness check together.
 *
 * The shared half — the options reader and the format family — rides every package; the
 * parse half — the parse family and the `locations.js` helper it runs — rides the
 * parse-capable ones (the format-only package loads neither).
 *
 * @module
 */

/** Where the facade sources live, relative to the repo root. */
export const FACADE_SOURCE_DIR = 'crates/tsv_wasm/npm';

/** The facade's shared half, by file name. */
const FACADE_SHARED_FILES: readonly string[] = ['api.js', 'api.d.ts'];

/** The facade's parse half, by file name. */
const FACADE_PARSE_FILES: readonly string[] = [
	'api_parse.js',
	'api_parse.d.ts',
	'locations.js',
	'locations.d.ts'
];

/** The facade files a package ships, by file name — `parse` for a parse-capable one. */
export function facade_files(parse: boolean): string[] {
	return [...FACADE_SHARED_FILES, ...(parse ? FACADE_PARSE_FILES : [])];
}

/** `facade_files`, as repo-relative source paths. */
export function facade_sources(parse: boolean): string[] {
	return facade_files(parse).map((file) => `${FACADE_SOURCE_DIR}/${file}`);
}
