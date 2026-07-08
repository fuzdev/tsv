/**
 * N-API bindings to native tsv (the Node/Bun native path).
 *
 * The runtime sibling of `ffi.ts` (Deno's `Deno.dlopen` C-FFI path): same engine
 * (`tsv_napi`, built from the same language crates), different binding boundary.
 * Loaded with `process.dlopen`, which accepts the built cdylib directly
 * (`target/release/libtsv_napi.so`) as an N-API addon — no `.node` rename, so
 * `build:napi` is just `cargo build -p tsv_napi --release`.
 *
 * Unlike FFI there are no raw pointers and no manual free: napi-rs marshals the
 * JS string in and the returned `String` out. `parse_<lang>` returns a JSON
 * string (parity with FFI/WASM — the host `JSON.parse`s it), and engine errors
 * surface as thrown JS errors (napi-rs converts the `napi::Error`), so there is
 * no `{"error": …}` envelope to inspect — a throw just propagates.
 *
 * Only instantiated under Node/Bun (see `implementations.ts`); importing this
 * module under Deno is harmless because `process.dlopen` is only touched in
 * `init()`.
 */

import { stat } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { native_library_filename } from './runtime.ts';
import type { Language, ParseGoal, TsvImplementation } from './types.ts';

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
	// goal-aware TS parse (`'script'`/`'module'`) — the conformance surface's test262 files
	parse_typescript_with_goal: (source: string, goal: string) => string;
	parse_typescript_no_locations_with_goal: (source: string, goal: string) => string;
	parse_internal_typescript_with_goal: (source: string, goal: string) => void;
}

/** Path to the built `tsv_napi` cdylib (loaded directly as an N-API addon). */
export function get_napi_library_path(): string {
	const project_root = fileURLToPath(new URL('../../../', import.meta.url));
	return `${project_root}target/release/${native_library_filename('tsv_napi')}`;
}

export class NapiImplementation implements TsvImplementation {
	// Distinct from FFI's `'native'` so the two native bindings are
	// self-describing (one is instantiated per runtime — FFI under Deno, N-API
	// under Node/Bun). Nothing branches on this tag; rows key on `tracking_key`.
	name = 'napi' as const;
	private _addon: NapiAddon | null = null;

	/** Languages supported for parsing */
	static readonly PARSE_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	/** Languages supported for formatting */
	static readonly FORMAT_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	private get addon(): NapiAddon {
		if (!this._addon) throw new Error('N-API addon not initialized');
		return this._addon;
	}

	async init(): Promise<void> {
		const path = get_napi_library_path();
		try {
			await stat(path);
		} catch {
			throw new Error(
				`N-API addon not found at ${path}. Run 'deno task build:napi' first.`,
			);
		}
		// `process.dlopen` loads a native addon from any path/extension into the
		// passed module's `exports` — the supported way to load a `.so`/`.dylib`
		// that isn't named `.node`.
		const mod: { exports: NapiAddon } = { exports: {} as NapiAddon };
		process.dlopen(mod, path);
		this._addon = mod.exports;
	}

	supports_parse_language(language: Language): boolean {
		return NapiImplementation.PARSE_LANGUAGES.includes(language);
	}

	supports_format_language(language: Language): boolean {
		return NapiImplementation.FORMAT_LANGUAGES.includes(language);
	}

	private get parse_fns(): Record<Language, (source: string) => string> {
		return {
			svelte: this.addon.parse_svelte,
			typescript: this.addon.parse_typescript,
			css: this.addon.parse_css,
		};
	}

	private get parse_internal_fns(): Record<Language, (source: string) => void> {
		return {
			svelte: this.addon.parse_internal_svelte,
			typescript: this.addon.parse_internal_typescript,
			css: this.addon.parse_internal_css,
		};
	}

	private get format_fns(): Record<Language, (source: string) => string> {
		return {
			svelte: this.addon.format_svelte,
			typescript: this.addon.format_typescript,
			css: this.addon.format_css,
		};
	}

	// Span-only wire — svelte + typescript only (CSS has no `loc`).
	private get parse_no_locations_fns(): Partial<Record<Language, (source: string) => string>> {
		return {
			svelte: this.addon.parse_svelte_no_locations,
			typescript: this.addon.parse_typescript_no_locations,
		};
	}

	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		// `parse_<lang>` returns a JSON string (the engine throws on parse error);
		// materialize it the same way ffi.ts / wasm.ts do for an apples-to-apples
		// `tsv-json`-style row. A test262 goal routes through the goal-aware TS export.
		if (goal && language === 'typescript') {
			return JSON.parse(this.addon.parse_typescript_with_goal(source, goal));
		}
		return JSON.parse(this.parse_fns[language](source));
	}

	parse_internal(source: string, language: Language, goal?: ParseGoal): void {
		if (goal && language === 'typescript') {
			this.addon.parse_internal_typescript_with_goal(source, goal);
			return;
		}
		this.parse_internal_fns[language](source);
	}

	parse_no_locations(source: string, language: Language, goal?: ParseGoal): unknown {
		if (goal && language === 'typescript') {
			return JSON.parse(this.addon.parse_typescript_no_locations_with_goal(source, goal));
		}
		const fn = this.parse_no_locations_fns[language];
		if (!fn) throw new Error(`no-locations parse unsupported for ${language}`);
		return JSON.parse(fn(source));
	}

	format(source: string, language: Language): string {
		return this.format_fns[language](source);
	}

	dispose(): void {
		this._addon = null;
	}
}
