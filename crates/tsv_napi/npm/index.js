/**
 * `@fuzdev/tsv` — native N-API bindings for tsv (Node.js / Bun).
 *
 * A thin loader over the per-platform prebuilt addons (`@fuzdev/tsv-<triple>`, installed as
 * optionalDependencies — the package manager selects by their `os`/`cpu`/`libc` fields;
 * `addon.js` only resolves the matching name). The formatter, parser, type-strip and
 * import-records API is `@fuzdev/tsv-wasm`'s — the same hand-written facade (`api.js` /
 * `api_parse.js` / `api_strip.js` / `api_imports.js`, staged in from the wasm crate), so the same
 * names, `(source, options?)` bags, and error strings — and the two are drop-in swaps: this one is
 * the fast native path, that one the universal fallback (browsers + unsupported platforms).
 *
 * Two entries, as in `@fuzdev/tsv-wasm`: this root carries format, the type strip, the import
 * records and `IgnoreStack`; the parse family — `parse_<lang>`, the `locations.js` helpers and
 * the AST types — is the `./parse` entry's (`parse.js`), whose static graph holds the tape
 * decoder, so a process that never parses never loads either.
 * Both import the addon from `addon.js`, so it loads once.
 *
 * What is absent here is only what a WASM engine needs and this one doesn't: `init()` and
 * `init_sync()` (nothing to initialize), `wasm_module` (no compiled module to hand a worker),
 * `reinstantiate()` (no instance to poison — a native stack overflow is a process-fatal SIGSEGV,
 * not a recoverable trap), and, one level down, `IgnoreStack`'s `free()` and its
 * `[Symbol.dispose]` alias, which a GC-managed native object has no handle to need. That list is
 * the whole delta — `scripts/test_napi_npm.ts` compares the two export sets, entry by entry,
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
 * A Rust panic — always a tsv bug — surfaces as a thrown JS `Error` whose message
 * is `panic: <payload>` (the addon builds with an unwinding profile and catches a
 * panic in every export); stack overflow is the one crash that still aborts the host.
 */

// The shared facade — staged beside this file by `scripts/build_napi_packages.ts` from
// `crates/tsv_wasm/npm/`, so these resolve in the package, not in the source tree.
import { addon } from './addon.js';
import { create_format_api } from './api.js';
import { create_imports_api } from './api_imports.js';
import { create_strip_api } from './api_strip.js';

// The published surface: the shared facade (`api.js` + `api_strip.js` + `api_imports.js`,
// staged from `crates/tsv_wasm/npm/` — one options reader, one set of error texts) over the
// addon's flat exports. The parse family is the `./parse` entry's (`parse.js`), so a
// process that never parses never loads it or the tape decoder.
const from_addon = (name) =>
	Object.fromEntries(
		['svelte', 'typescript', 'css'].map((language) => [
			language,
			addon[name.replace('<lang>', language)]
		])
	);

export const { format_svelte, format_typescript, format_css } = create_format_api(
	from_addon('format_<lang>')
);

// The type strip: TypeScript's alone, so its one engine function is named directly.
export const { strip_typescript } = create_strip_api({ typescript: addon.strip_typescript });

// The import records: TypeScript's and Svelte's, the two languages that hold a module request.
export const { parse_imports_typescript, parse_imports_svelte } = create_imports_api({
	typescript: addon.parse_imports_typescript,
	svelte: addon.parse_imports_svelte
});

// The discovery matcher, re-exported straight off the addon — it takes no
// options bag, so there is nothing to wrap. Same class name and method surface
// as `@fuzdev/tsv-wasm`'s, including `undefined` (not `null`) from the three
// maybe-a-warning methods, so `cli.js` drives either package's copy unchanged.
export const IgnoreStack = addon.IgnoreStack;
