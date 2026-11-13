/**
 * N-API bindings to native tsv (the Node/Bun native path).
 *
 * The runtime sibling of `ffi.ts` (Deno's `Deno.dlopen` C-FFI path): same engine
 * (`tsv_napi`, built from the same language crates), different binding boundary.
 * Loaded with `process.dlopen`, which accepts the built cdylib directly as an N-API
 * addon — no `.node` rename. By default the library is the profile-guided addon
 * `npm run build:bench:pgo` stages (`lib/pgo_bench.ts`): the shipped addon's own
 * pipeline (`scripts/pgo/pgo.ts`), on the workspace's unwinding `napi` profile, so the
 * bench measures what ships — its profile and its panic contract. A caller that grades
 * rather than times loads a cargo profile's plain build instead, by passing its path
 * (`napi_library_path` in `lib/tsv_artifacts.ts`).
 *
 * Unlike FFI there are no raw pointers and no manual free: napi-rs marshals the
 * JS string in and the returned `String` out. `parse_<lang>_json` returns the span-only
 * wire as a JSON string. A failure the source causes (a parse failure, a format's refusal)
 * is RETURNED in the product's place as `{ok: false, error}`; only an argument refusal, the
 * size cap and a caught panic surface as thrown JS errors (napi-rs converts the
 * `napi::Error`), so there is no status out-param to read (the FFI shape). A Rust PANIC
 * surfaces the same way: every export catches one and the `napi` profile unwinds, so a
 * panic throws an `Error` whose message is `panic: <payload>`, the C FFI's text, instead
 * of aborting the host (stack overflow excepted — that still aborts).
 *
 * The `parse` and `format` rows call the published API over these exports — the
 * packages' facade, wired as the `@fuzdev/tsv` loader wires it
 * (`crates/tsv_napi/npm/index.js`: `parse_<lang>_json` handed over as the engine, so
 * the facade runs the `JSON.parse`). See `lib/tsv_api.ts`.
 *
 * Only instantiated under Node/Bun (see `implementations.ts`); importing this
 * module under Deno is harmless because `process.dlopen` is only touched in
 * `init()`.
 */

import { stat } from 'node:fs/promises';
import { type EngineFailure, type EngineFn, TsvBinding } from './tsv_api.ts';
import { napi_loaded_path, napi_rebuild_task } from './tsv_artifacts.ts';
import type { InitScope } from './types.ts';

/**
 * The N-API addon's exported functions (snake_case `js_name`s, matching WASM/FFI).
 *
 * One export per (language, operation), each taking the goal as a trailing
 * OPTIONAL argument (`'script'`/`'module'`; omitted = module) — there is no
 * goalless twin to pick between. Svelte and CSS REJECT a set goal rather than
 * ignoring it (`tsv_napi`'s `napi_source_type`), so `TsvBinding` withholds it for them.
 */
export interface NapiAddon {
	parse_svelte_json: EngineFn<string | EngineFailure>;
	parse_internal_svelte: EngineFn<EngineFailure | undefined>;
	format_svelte: EngineFn<string | EngineFailure>;
	parse_typescript_json: EngineFn<string | EngineFailure>;
	parse_internal_typescript: EngineFn<EngineFailure | undefined>;
	format_typescript: EngineFn<string | EngineFailure>;
	parse_css_json: EngineFn<string | EngineFailure>;
	parse_internal_css: EngineFn<EngineFailure | undefined>;
	format_css: EngineFn<string | EngineFailure>;
	// test-only panic-contract probes — present only when built with the
	// `panic_probe` cargo feature (`npm run test:napi`); absent in published
	// builds, so `test_napi.ts` skips its contract test when undefined. The
	// second panics with a payload that is no string
	__panic_probe?: () => void;
	__panic_probe_any?: () => void;
}

export class NapiImplementation extends TsvBinding {
	/**
	 * @param lib_path - an addon to load in place of the profile-guided one
	 * (`napi_loaded_path(undefined)`), such as a cargo profile's plain build
	 * (`napi_library_path(profile)`)
	 */
	constructor(lib_path?: string) {
		super();
		this.lib_path = lib_path;
	}

	private readonly lib_path: string | undefined;

	async init(scope?: InitScope): Promise<void> {
		const path = this.lib_path ?? napi_loaded_path(undefined);
		const rebuild = this.lib_path === undefined ? napi_rebuild_task(undefined) : `rebuild ${path}`;
		try {
			await stat(path);
		} catch {
			throw new Error(`N-API addon not found at ${path}. Run '${rebuild}' first.`);
		}
		// `process.dlopen` loads a native addon from any path/extension into the
		// passed module's `exports` — the supported way to load a `.so`/`.dylib`
		// that isn't named `.node`.
		const mod: { exports: NapiAddon } = { exports: {} as NapiAddon };
		process.dlopen(mod, path);

		// The addon's flat exports, with `parse_<lang>_json` as the facade's parse
		// engine — the loader's own wiring.
		const addon = mod.exports;
		this.bind({
			parse_json: {
				svelte: addon.parse_svelte_json,
				typescript: addon.parse_typescript_json,
				css: addon.parse_css_json
			},
			format: {
				svelte: addon.format_svelte,
				typescript: addon.format_typescript,
				css: addon.format_css
			},
			parse_internal: {
				svelte: addon.parse_internal_svelte,
				typescript: addon.parse_internal_typescript,
				css: addon.parse_internal_css
			}
		});

		// The addon returns a parse failure as `{ok: false, error}`, which the wrapper rethrows
		// as the row's rejection (`rejected`, `lib/tsv_api.ts`); probed anyway so the three
		// bindings can't come to disagree about what surfacing a refusal MEANS. And the parse wire is
		// span-only, so a stale addon whose parse still emits `loc` can't be timed as the
		// span-only default.
		this.probe('tsv (N-API)', { path, rebuild }, scope);
	}
}
