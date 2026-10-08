/**
 * N-API bindings to native tsv (the Node/Bun native path).
 *
 * The runtime sibling of `ffi.ts` (Deno's `Deno.dlopen` C-FFI path): same engine
 * (`tsv_napi`, built from the same language crates), different binding boundary.
 * Loaded with `process.dlopen`, which accepts the built cdylib directly
 * (`target/napi/libtsv_napi.so`) as an N-API addon — no `.node` rename, so
 * `build:napi` is just `cargo build -p tsv_napi --profile napi` (the workspace's
 * unwinding release profile — the same artifact that ships, so the bench
 * measures the shipped panic contract).
 *
 * Unlike FFI there are no raw pointers and no manual free: napi-rs marshals the
 * JS string in and the returned `String` out. `parse_<lang>_json` returns the span-only
 * wire as a JSON string, and engine errors surface as thrown JS errors (napi-rs
 * converts the `napi::Error`), so there is no status out-param to read (the FFI
 * shape) — a throw just propagates. A Rust PANIC
 * surfaces the same way: every export carries `catch_unwind` and the `napi`
 * profile unwinds, so a panic throws instead of aborting the host (stack
 * overflow excepted — that still aborts).
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
import { napi_library_path } from './tsv_artifacts.ts';
import { TsvBinding } from './tsv_api.ts';
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
	parse_svelte_json: (source: string, goal?: string) => string;
	parse_internal_svelte: (source: string, goal?: string) => void;
	format_svelte: (source: string, goal?: string) => string;
	parse_typescript_json: (source: string, goal?: string) => string;
	parse_internal_typescript: (source: string, goal?: string) => void;
	format_typescript: (source: string, goal?: string) => string;
	parse_css_json: (source: string, goal?: string) => string;
	parse_internal_css: (source: string, goal?: string) => void;
	format_css: (source: string, goal?: string) => string;
	// test-only panic-contract probe — present only when built with the
	// `panic_probe` cargo feature (`deno task test:napi`); absent in published
	// builds, so `test_napi.ts` skips its contract test when undefined
	__panic_probe?: () => void;
}

/** Path to the built `tsv_napi` cdylib (loaded directly as an N-API addon).
 * `target/napi/` is the workspace `napi` profile's output — release + unwind,
 * the shipped panic contract. */
export function get_napi_library_path(): string {
	return napi_library_path();
}

export class NapiImplementation extends TsvBinding {
	async init(scope?: InitScope): Promise<void> {
		const path = get_napi_library_path();
		try {
			await stat(path);
		} catch {
			throw new Error(`N-API addon not found at ${path}. Run 'deno task build:napi' first.`);
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

		// The addon throws natively today; probed anyway so the three bindings can't
		// come to disagree about what surfacing a refusal MEANS. And the parse wire is
		// span-only, so a stale addon whose parse still emits `loc` can't be timed as the
		// span-only default.
		this.probe('tsv (N-API)', { path, rebuild: 'deno task build:napi' }, scope);
	}
}
