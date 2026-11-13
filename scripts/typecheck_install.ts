/**
 * What `typecheck:packages` (`scripts/typecheck_packages.ts`) loads from the `benches/js`
 * install, checkout-relative: `scripts/publish.ts` asks for the same set in its preflight, so a
 * missing install fails a publish before the bump and the build.
 *
 * Imports nothing, so a script on any runtime can read it.
 *
 * @module
 */

/** The `benches/js` install (`npm run bench:install`). */
const BENCH_NODE_MODULES = 'benches/js/node_modules';

/** TypeScript's CommonJS entry in the install. */
export const TYPESCRIPT_MODULE = `${BENCH_NODE_MODULES}/typescript/lib/typescript.js`;

/** The install's `@types`, for a README block that grades with `@types/node`. */
export const TYPES_ROOT = `${BENCH_NODE_MODULES}/@types`;

/** Each package the check loads, by the file that shows it installed. */
export const TYPECHECK_INSTALL: ReadonlyArray<{ what: string; path: string }> = [
	{ what: 'typescript', path: TYPESCRIPT_MODULE },
	{ what: '@types/node', path: `${TYPES_ROOT}/node/package.json` }
];
