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
 * JS string in and the returned `String` out. `parse_<lang>` returns a JSON
 * string (parity with FFI/WASM — the host `JSON.parse`s it), and engine errors
 * surface as thrown JS errors (napi-rs converts the `napi::Error`), so there is
 * no `{"error": …}` envelope to inspect — a throw just propagates. A Rust PANIC
 * surfaces the same way: every export carries `catch_unwind` and the `napi`
 * profile unwinds, so a panic throws instead of aborting the host (stack
 * overflow excepted — that still aborts).
 *
 * Only instantiated under Node/Bun (see `implementations.ts`); importing this
 * module under Deno is harmless because `process.dlopen` is only touched in
 * `init()`.
 */

import { stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { native_library_filename } from './runtime.ts';
import { BaseImplementation, type Language, LANGUAGES, type ParseGoal } from './types.ts';
import { assert_binding_reports_rejection } from './reject_probe.ts';

/** The N-API addon's exported functions (snake_case `js_name`s, matching WASM/FFI). */
export interface NapiAddon {
	parse_svelte: (source: string) => string;
	parse_internal_svelte: (source: string) => void;
	format_svelte: (source: string) => string;
	parse_typescript: (source: string) => string;
	parse_internal_typescript: (source: string) => void;
	format_typescript: (source: string) => string;
	parse_css: (source: string) => string;
	parse_internal_css: (source: string) => void;
	format_css: (source: string) => string;
	// span-only wire — svelte + typescript only (CSS emits no `loc`)
	parse_svelte_no_locations: (source: string) => string;
	parse_typescript_no_locations: (source: string) => string;
	// goal-aware TS (`'script'`/`'module'`) — parse for the conformance surface's
	// test262 files; format is the flat counterpart of tsv_wasm's `{goal}` bag
	parse_typescript_with_goal: (source: string, goal: string) => string;
	parse_typescript_no_locations_with_goal: (source: string, goal: string) => string;
	parse_internal_typescript_with_goal: (source: string, goal: string) => void;
	format_typescript_with_goal: (source: string, goal: string) => string;
	// test-only panic-contract probe — present only when built with the
	// `panic_probe` cargo feature (`deno task test:napi`); absent in published
	// builds, so `test_napi.ts` skips its contract test when undefined
	__panic_probe?: () => void;
}

/** Path to the built `tsv_napi` cdylib (loaded directly as an N-API addon).
 * `target/napi/` is the workspace `napi` profile's output — release + unwind,
 * the shipped panic contract. */
export function get_napi_library_path(): string {
	const project_root = fileURLToPath(new URL('../../../', import.meta.url));
	return `${project_root}target/napi/${native_library_filename('tsv_napi')}`;
}

/**
 * The per-language export tables, resolved ONCE in `init()`.
 *
 * The FFI sibling's `FfiTables` for the same reason: these were getters returning a
 * fresh object literal, so every timed call allocated one — harness-side allocation
 * charged to whichever row it sat under, which belongs to no impl.
 */
interface NapiTables {
	parse: Record<Language, (source: string) => string>;
	parse_internal: Record<Language, (source: string) => void>;
	/** Span-only wire — svelte + typescript only (CSS emits no `loc`). */
	parse_no_locations: Partial<Record<Language, (source: string) => string>>;
	format: Record<Language, (source: string) => string>;
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
				svelte: addon.parse_svelte,
				typescript: addon.parse_typescript,
				css: addon.parse_css
			},
			parse_internal: {
				svelte: addon.parse_internal_svelte,
				typescript: addon.parse_internal_typescript,
				css: addon.parse_internal_css
			},
			parse_no_locations: {
				svelte: addon.parse_svelte_no_locations,
				typescript: addon.parse_typescript_no_locations
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
	}

	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		// `parse_<lang>` returns a JSON string (the engine throws on parse error);
		// materialize it the same way ffi.ts / wasm.ts do for an apples-to-apples
		// `tsv-json`-style row. A test262 goal routes through the goal-aware TS export.
		if (goal && language === 'typescript') {
			return JSON.parse(this.addon.parse_typescript_with_goal(source, goal));
		}
		return JSON.parse(this.tables.parse[language](source));
	}

	parse_internal(source: string, language: Language, goal?: ParseGoal): void {
		if (goal && language === 'typescript') {
			this.addon.parse_internal_typescript_with_goal(source, goal);
			return;
		}
		this.tables.parse_internal[language](source);
	}

	parse_no_locations(source: string, language: Language, goal?: ParseGoal): unknown {
		if (goal && language === 'typescript') {
			return JSON.parse(this.addon.parse_typescript_no_locations_with_goal(source, goal));
		}
		const fn = this.tables.parse_no_locations[language];
		if (!fn) throw new Error(`no-locations parse unsupported for ${language}`);
		return JSON.parse(fn(source));
	}

	format(source: string, language: Language): string {
		return this.tables.format[language](source);
	}

	dispose(): void {
		this._addon = null;
		this._tables = null;
	}
}
