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
 * wire as a JSON string (parity with FFI/WASM — the host `JSON.parse`s it), and engine errors
 * surface as thrown JS errors (napi-rs converts the `napi::Error`), so there is
 * no status out-param to read (the FFI shape) — a throw just propagates. A Rust PANIC
 * surfaces the same way: every export carries `catch_unwind` and the `napi`
 * profile unwinds, so a panic throws instead of aborting the host (stack
 * overflow excepted — that still aborts).
 *
 * Only instantiated under Node/Bun (see `implementations.ts`); importing this
 * module under Deno is harmless because `process.dlopen` is only touched in
 * `init()`.
 */

import { stat } from 'node:fs/promises';
import { napi_library_path } from './tsv_artifacts.ts';
import { BaseImplementation, goal_for, type Language, LANGUAGES, type ParseGoal } from './types.ts';
import { assert_binding_reports_rejection } from './reject_probe.ts';
import { assert_binding_emits_span_only } from './locations_probe.ts';

/**
 * The N-API addon's exported functions (snake_case `js_name`s, matching WASM/FFI).
 *
 * One export per (language, operation), each taking the goal as a trailing
 * OPTIONAL argument (`'script'`/`'module'`; omitted = module) — there is no
 * goalless twin to pick between. Svelte and CSS REJECT a set goal rather than
 * ignoring it, so the wrappers below withhold it for them.
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

/**
 * The per-language export tables, resolved ONCE in `init()`.
 *
 * The FFI sibling's `FfiTables` for the same reason: these were getters returning a
 * fresh object literal, so every timed call allocated one — harness-side allocation
 * charged to whichever row it sat under, which belongs to no impl.
 */
interface NapiTables {
	/** The span-only wire — the one parse wire every tsv binding emits. */
	parse: Record<Language, (source: string, goal?: string) => string>;
	parse_internal: Record<Language, (source: string, goal?: string) => void>;
	format: Record<Language, (source: string, goal?: string) => string>;
}

export class NapiImplementation extends BaseImplementation {
	private _addon: NapiAddon | null = null;
	private _tables: NapiTables | null = null;

	readonly parse_languages = LANGUAGES;
	readonly format_languages = LANGUAGES;

	private get addon(): NapiAddon {
		if (!this._addon) throw new Error('N-API addon not initialized');
		return this._addon;
	}

	/** The per-language export tables, or throw if `init()` hasn't run. */
	private get tables(): NapiTables {
		if (!this._tables) throw new Error('N-API addon not initialized');
		return this._tables;
	}

	async init(): Promise<void> {
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
		this._addon = mod.exports;

		// Resolve every export table once — see `NapiTables`.
		const addon = this.addon;
		this._tables = {
			parse: {
				svelte: addon.parse_svelte_json,
				typescript: addon.parse_typescript_json,
				css: addon.parse_css_json
			},
			parse_internal: {
				svelte: addon.parse_internal_svelte,
				typescript: addon.parse_internal_typescript,
				css: addon.parse_internal_css
			},
			format: {
				svelte: addon.format_svelte,
				typescript: addon.format_typescript,
				css: addon.format_css
			}
		};

		// The addon throws natively today; probed anyway so the three bindings can't
		// come to disagree about what surfacing a refusal MEANS — see `lib/reject_probe.ts`.
		assert_binding_reports_rejection('tsv (N-API)', this);

		// The parse wire is span-only — prove this artifact emits it, so a stale addon
		// whose parse still emits `loc` can't be timed under the span rows' label.
		// See `lib/locations_probe.ts`.
		assert_binding_emits_span_only('tsv (N-API)', { path, rebuild: 'deno task build:napi' }, this);
	}

	// `goal_for` withholds the goal for svelte/css, which REJECT a set goal rather
	// than ignoring it (`tsv_napi`'s `napi_source_type`). One shared helper for all three
	// wrappers — see its doc in `lib/types.ts`.
	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		// `parse_<lang>_json` returns the span-only wire as a JSON string (the engine throws on
		// parse error); materialize it the same way ffi.ts / wasm.ts do, for an
		// apples-to-apples row.
		return JSON.parse(this.tables.parse[language](source, goal_for(language, goal)));
	}

	parse_internal(source: string, language: Language, goal?: ParseGoal): void {
		this.tables.parse_internal[language](source, goal_for(language, goal));
	}

	// No source type: the shipped default on every surface, and the one that reaches
	// the module-then-script fallback (`tsv_ts::parse_with_goal_or_fallback`).
	format(source: string, language: Language): string {
		return this.tables.format[language](source);
	}

	dispose(): void {
		this._addon = null;
		this._tables = null;
	}
}
