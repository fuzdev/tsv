/**
 * `@fuzdev/tsv` — native N-API bindings for tsv (Node.js / Bun).
 *
 * A thin loader over the per-platform prebuilt addons
 * (`@fuzdev/tsv-<triple>`, installed as optionalDependencies — the
 * package manager selects by their `os`/`cpu`/`libc` fields; this file only
 * resolves the matching name). The formatter and parser API mirrors
 * `@fuzdev/tsv_wasm` — same names, same `(source, options?)` bags, same error
 * strings — so the two are drop-in swaps: this one is the fast native path,
 * that one the universal fallback (browsers + unsupported platforms). What is
 * absent here is only what a WASM engine needs and this one doesn't: `init()`
 * and `init_sync()` (nothing to initialize), `wasm_module` (no compiled
 * module to hand a worker), and `reinstantiate()` (no instance to poison — a
 * native stack overflow is a process-fatal SIGSEGV, not a recoverable trap).
 * `npm/cli.js` reads that absence three ways: as the signal to load this
 * loader in its workers rather than the wasm `./worker` entry, as the signal
 * to size its pool for an engine with no wasm tier-up competing for cores — a
 * wider pool, crossing over at fewer files — and as the signal that a trapped
 * engine cannot be recovered (`recover_engine_suffix`, unreachable here).
 *
 * ESM, the same module system as `@fuzdev/tsv_wasm` — one dialect across tsv's
 * whole npm surface, which is what lets the shared `locations.js` and `cli.js`
 * sources load unchanged in either package. `.node` binaries have no ESM
 * loader, so the platform addon itself is always `require`d, through
 * `createRequire`; that shim is the only CommonJS left here.
 *
 * A Rust panic — always a tsv bug — surfaces as a thrown JS error (the addon
 * builds with an unwinding profile and `catch_unwind` on every export); stack
 * overflow is the one crash that still aborts the host.
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
			`@fuzdev/tsv_wasm (universal WASM, same API) needs no native package. `
		: `Prebuilt platforms: ${SUPPORTED.join(', ')}. ` +
			`On an unsupported platform use @fuzdev/tsv_wasm (universal WASM, same API). `;
	throw new Error(
		`@fuzdev/tsv: failed to load the native binding for ${triple} ` +
			`(@fuzdev/tsv-${triple}). ${remedy}Cause: ${cause?.message ?? cause}`,
		{ cause }
	);
}

/**
 * Mirror of `@fuzdev/tsv_wasm`'s `read_options`, key for key — same defaults,
 * same error strings — so swapping the WASM package for this one never changes
 * an error a caller matches on. Unknown keys error whatever their value (a
 * typo like `{locatons: false}` must not silently opt out); a supported key
 * explicitly set to `undefined` means its default, including the TS-only
 * `sourceType` on a language that rejects it, which lets one bag forward to
 * whichever parser or formatter.
 *
 * An unset `sourceType` stays `undefined` rather than becoming `'module'`,
 * because the addon's two families answer it differently: a parse reads it as
 * `module` (its wire's `Program.sourceType` is a claim one settled grammar has to
 * produce), a format as "none named" — the module grammar retried as a script, so
 * a legacy sloppy script formats without naming one.
 */
const read_options = (options, noun, has_locations, has_source_type) => {
	const parsed = { locations: true, source_type: undefined };
	if (options === undefined || options === null) return parsed;
	// An array is `typeof 'object'` and yields no keys — without this test a
	// positional-style call would read as all-defaults.
	if (typeof options !== 'object' || Array.isArray(options)) {
		throw new Error(`${noun} options must be an object`);
	}
	for (const name of Object.keys(options)) {
		const value = options[name];
		if (name === 'locations' && has_locations) {
			if (value === undefined) continue;
			if (typeof value !== 'boolean') {
				throw new Error(`${noun} option 'locations' must be a boolean`);
			}
			parsed.locations = value;
		} else if (name === 'sourceType') {
			if (value === undefined) continue;
			if (!has_source_type) {
				throw new Error(`${noun} option 'sourceType' is only supported for TypeScript`);
			}
			if (typeof value !== 'string') {
				throw new Error(`${noun} option 'sourceType' must be 'script' or 'module'`);
			}
			if (value !== 'script' && value !== 'module') {
				throw new Error(`invalid sourceType '${value}' (expected 'script' or 'module')`);
			}
			parsed.source_type = value;
		} else {
			const detail =
				has_locations && has_source_type
					? "expected 'locations' or 'sourceType'"
					: has_locations
						? "expected 'locations'"
						: has_source_type
							? "expected 'sourceType'"
							: 'this export takes no options';
			throw new Error(`unknown ${noun} option '${name}' (${detail})`);
		}
	}
	return parsed;
};

// The TS parse wire against the resolved options. The addon has one export per
// (language, operation), each taking the source type as a trailing optional
// argument — there is no goalless twin to pick between.
const ts_parse_json = (source, opts) =>
	opts.locations
		? addon.parse_typescript(source, opts.source_type)
		: addon.parse_typescript_no_locations(source, opts.source_type);

const svelte_parse_json = (source, opts) =>
	opts.locations ? addon.parse_svelte(source) : addon.parse_svelte_no_locations(source);

export const parse_svelte = (source, options) =>
	JSON.parse(svelte_parse_json(source, read_options(options, 'parse', true, false)));
export const parse_svelte_json = (source, options) =>
	svelte_parse_json(source, read_options(options, 'parse', true, false));

export const parse_typescript = (source, options) =>
	JSON.parse(ts_parse_json(source, read_options(options, 'parse', true, true)));
export const parse_typescript_json = (source, options) =>
	ts_parse_json(source, read_options(options, 'parse', true, true));

// `locations` is accepted and inert for CSS — its wire carries no `loc`
// (parity with the WASM package, whose `parse_css` reads the same bag).
export const parse_css = (source, options) => {
	read_options(options, 'parse', true, false);
	return JSON.parse(addon.parse_css(source));
};
export const parse_css_json = (source, options) => {
	read_options(options, 'parse', true, false);
	return addon.parse_css(source);
};

export const format_svelte = (source, options) => {
	read_options(options, 'format', false, false);
	return addon.format_svelte(source);
};
export const format_typescript = (source, options) =>
	addon.format_typescript(source, read_options(options, 'format', false, true).source_type);
export const format_css = (source, options) => {
	read_options(options, 'format', false, false);
	return addon.format_css(source);
};

// The discovery matcher, re-exported straight off the addon — it takes no
// options bag, so there is nothing to wrap. Same class name and method surface
// as `@fuzdev/tsv_wasm`'s, including `undefined` (not `null`) from the three
// maybe-a-warning methods, so `cli.js` drives either package's copy unchanged.
export const IgnoreStack = addon.IgnoreStack;
