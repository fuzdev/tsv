/**
 * The hand-written facade every tsv npm package publishes through, staged verbatim into
 * the wasm packages (`scripts/patch_npm_package.ts`) and the native `@fuzdev/tsv`
 * (`scripts/build_napi_packages.ts`). Named once so a new facade file reaches every
 * package's copy step, `files` list and staging-freshness check together, and so the two
 * staging scripts state the facade's export-map entry and its re-exported type names
 * from one place.
 *
 * The shared half — the options reader, the format family and the error type — rides every
 * package; the
 * parse half — the parse family, the `locations.js` helper it runs and the AST types its
 * declarations name — rides the parse-capable ones (the format-only package loads none of
 * it).
 *
 * Plain `node:` imports only: the package suites run this module under Node as well as
 * Deno.
 *
 * @module
 */

import { readFileSync } from 'node:fs';

/** Where the facade's JS and declarations live, relative to the repo root. */
export const FACADE_SOURCE_DIR = 'crates/tsv_wasm/npm';

/** One facade file: the name it ships under in a package, and its repo-relative source. */
export interface FacadeFile {
	published: string;
	source: string;
}

/** A facade file staged from `FACADE_SOURCE_DIR` under its own name. */
const from_source_dir = (published: string): FacadeFile => ({
	published,
	source: `${FACADE_SOURCE_DIR}/${published}`
});

/** The facade's shared half. */
const FACADE_SHARED_FILES: ReadonlyArray<FacadeFile> = [
	from_source_dir('api.js'),
	from_source_dir('api.d.ts'),
	from_source_dir('syntax_error.d.ts')
];

/** The facade's parse half — with the hand-maintained AST types `api_parse.d.ts` and
 * `locations.d.ts` import. */
const FACADE_PARSE_FILES: ReadonlyArray<FacadeFile> = [
	from_source_dir('api_parse.js'),
	from_source_dir('api_parse.d.ts'),
	from_source_dir('locations.js'),
	from_source_dir('locations.d.ts'),
	{ published: 'tsv_ast.d.ts', source: 'crates/tsv_wasm/types/tsv_ast.d.ts' }
];

/** The facade files a package ships — `parse` for a parse-capable one. */
export function facade_files(parse: boolean): Array<FacadeFile> {
	return [...FACADE_SHARED_FILES, ...(parse ? FACADE_PARSE_FILES : [])];
}

/** `facade_files`' repo-relative sources. */
export function facade_sources(parse: boolean): Array<string> {
	return facade_files(parse).map((file) => file.source);
}

/**
 * The `./locations` subpath's exports-map entry: the reconstruction helper alone — pure JS
 * that imports nothing, so a consumer holding a tree reaches it without loading an engine.
 * Every parse-capable package exports it.
 */
export const LOCATIONS_EXPORT = {
	types: './locations.d.ts',
	default: './locations.js'
} as const;

/** The facade declaration files whose types a package's entry re-exports by name. */
export type FacadeDeclarations = 'api.d.ts' | 'api_parse.d.ts' | 'syntax_error.d.ts';

/**
 * The types (interfaces and type aliases) a facade declaration file exports, in source
 * order — what each package entry's `.d.ts` re-exports by NAME, so the `tsv_ast` / locations
 * star exports beside them can never ambiguate one away (TS2308). Read off the source, so
 * a type added to the facade reaches every package with no list to keep.
 *
 * @throws Error when the file declares none, which a declaration form the scan does not
 *   read would otherwise turn into a type silently missing from every package
 */
export function facade_type_names(file: FacadeDeclarations): Array<string> {
	const source = readFileSync(new URL(`../${FACADE_SOURCE_DIR}/${file}`, import.meta.url), 'utf8');
	const names = [...source.matchAll(/^export (?:declare )?(?:interface|type) (\w+)/gm)].map(
		(m) => m[1]!
	);
	if (names.length === 0) throw new Error(`no exported types found in ${file}`);
	return names;
}
