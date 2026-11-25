/**
 * The platform addon `@fuzdev/tsv` loads — internal, in no `exports` entry: the triple
 * resolve, the `require` of `@fuzdev/tsv-<triple>` and its load error, in the one module
 * both entries import (`index.js` for format, strip, imports and `IgnoreStack`; `parse.js`
 * for the parse family), so a process importing both loads the addon once.
 *
 * `.node` binaries have no ESM loader, so the addon is `require`d through `createRequire`
 * (oxc-parser's shape) — the only CommonJS left in the package.
 */

import { createRequire } from 'node:module';

import { platform_triple } from './platform.js';

const require = createRequire(import.meta.url);

/** The platform packages that exist — what the load error reports. Keep in
 * sync with `scripts/build_napi_packages.ts`'s `SUPPORTED_TRIPLES`
 * (`scripts/test_napi_npm.ts` gates the agreement via the generated
 * optionalDependencies). */
const SUPPORTED = [
	'linux-x64-gnu',
	'linux-arm64-gnu',
	'linux-x64-musl',
	'darwin-arm64',
	'darwin-x64',
	'win32-x64'
];

const triple = platform_triple();
let loaded;
try {
	loaded = require(`@fuzdev/tsv-${triple}`);
} catch (cause) {
	// Two different failures share this catch: a platform with no prebuilt
	// package, and a supported platform whose package did not install (a
	// lockfile resolved on another OS, `--omit=optional`, npm's optional-dep
	// bugs). Only the second has a remedy short of switching engines, so name it.
	const remedy = SUPPORTED.includes(triple)
		? `This platform is prebuilt, so the package should have installed with @fuzdev/tsv — ` +
			`run \`npm i @fuzdev/tsv-${triple}\`, or delete node_modules and the lockfile and ` +
			`reinstall (a lockfile from another platform, or --omit=optional, drops it); ` +
			`@fuzdev/tsv-wasm (universal WASM, same API) needs no native package. `
		: `Prebuilt platforms: ${SUPPORTED.join(', ')}. ` +
			`On an unsupported platform use @fuzdev/tsv-wasm (universal WASM, same API). `;
	throw new Error(
		`@fuzdev/tsv: failed to load the native binding for ${triple} ` +
			`(@fuzdev/tsv-${triple}). ${remedy}Cause: ${cause?.message ?? cause}`,
		{ cause }
	);
}

/** The platform addon's flat exports. */
export const addon = loaded;
