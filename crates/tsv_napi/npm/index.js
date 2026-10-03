/**
 * `@fuzdev/tsv` — native N-API bindings for tsv (Node.js / Bun).
 *
 * A thin loader over the per-platform prebuilt addons
 * (`@fuzdev/tsv-<triple>`, installed as optionalDependencies — the
 * package manager selects by their `os`/`cpu`/`libc` fields; this file only
 * resolves the matching name). The formatter and parser API is
 * `@fuzdev/tsv-wasm`'s — the same hand-written facade (`api.js` / `api_parse.js`,
 * staged in from the wasm crate), so the same names, `(source, options?)` bags, and
 * error strings — and the two are drop-in swaps: this one is the fast native path,
 * that one the universal fallback (browsers + unsupported platforms). What is
 * absent here is only what a WASM engine needs and this one doesn't: `init()`
 * and `init_sync()` (nothing to initialize), `wasm_module` (no compiled
 * module to hand a worker), `reinstantiate()` (no instance to poison — a
 * native stack overflow is a process-fatal SIGSEGV, not a recoverable trap),
 * and, one level down, `IgnoreStack`'s `free()` and its `[Symbol.dispose]`
 * alias, which a GC-managed native object has no handle to need. That list is
 * the whole delta — `scripts/test_napi_npm.ts` compares the two export sets
 * rather than trusting this sentence.
 *
 * `npm/cli.js` reads that absence three ways: as the signal to load this
 * loader in its workers rather than the wasm `./worker` entry, as the signal
 * to size its pool for an engine with no wasm tier-up competing for cores — a
 * wider pool, crossing over at fewer files — and as the signal that a trapped
 * engine cannot be recovered (`recover_engine_suffix`, unreachable here).
 *
 * ESM, the same module system as `@fuzdev/tsv-wasm` — one dialect across tsv's
 * whole npm surface, which is what lets the shared facade, `locations.js`, and
 * `cli.js` sources load unchanged in either package. `.node` binaries have no ESM
 * loader, so the platform addon itself is always `require`d, through
 * `createRequire`; that shim is the only CommonJS left here.
 *
 * A Rust panic — always a tsv bug — surfaces as a thrown JS error (the addon
 * builds with an unwinding profile and `catch_unwind` on every export); stack
 * overflow is the one crash that still aborts the host.
 */

import { createRequire } from 'node:module';

// The shared facade — staged beside this file by `scripts/build_napi_packages.ts` from
// `crates/tsv_wasm/npm/`, so these two resolve in the package, not in the source tree.
import { create_format_api } from './api.js';
import { create_parse_api } from './api_parse.js';
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
let addon;
try {
	addon = require(`@fuzdev/tsv-${triple}`);
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

// The published surface: the shared facade (`api.js` + `api_parse.js`, staged from
// `crates/tsv_wasm/npm/` — one options reader, one set of error texts, the
// `{locations: true}` sugar) over the addon's flat exports. The addon emits the
// span-only wire as a JSON string; the facade `JSON.parse`s it.
const engine_languages = ['svelte', 'typescript', 'css'];
const from_addon = (name) =>
	Object.fromEntries(
		engine_languages.map((language) => [language, addon[name.replace('<lang>', language)]])
	);

export const {
	parse_svelte,
	parse_svelte_json,
	parse_typescript,
	parse_typescript_json,
	parse_css,
	parse_css_json
} = create_parse_api({ parse_json: from_addon('parse_<lang>') });

export const { format_svelte, format_typescript, format_css } = create_format_api(
	from_addon('format_<lang>')
);

// The discovery matcher, re-exported straight off the addon — it takes no
// options bag, so there is nothing to wrap. Same class name and method surface
// as `@fuzdev/tsv-wasm`'s, including `undefined` (not `null`) from the three
// maybe-a-warning methods, so `cli.js` drives either package's copy unchanged.
export const IgnoreStack = addon.IgnoreStack;
