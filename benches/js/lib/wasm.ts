/**
 * WASM bindings to tsv
 *
 * Uses wasm-pack generated bindings for WebAssembly performance testing.
 * Runtime-aware: each runtime loads its own wasm-pack *target* bundle (same
 * `tsv_wasm_bg.wasm`, different JS glue), both carrying the full export set
 * including the benchmark-only `parse_internal_*`:
 *  - Deno: the `deno` target (ESM; explicit `default()` init)
 *  - Node/Bun: the `nodejs` target (CommonJS; self-initializing on require)
 * The shipped `@fuzdev/tsv_wasm` (web) bundle is deliberately NOT used here — it
 * curates out `parse_internal_*`, which the `tsv_wasm-internal` row needs.
 */

import { stat } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { current_runtime } from './runtime.ts';
import { BaseImplementation, type Language, LANGUAGES, type ParseGoal } from './types.ts';

/** The `{locations?, goal?}` options bag the parse exports take (`goal` is
 * TypeScript-only — the other languages reject the key). */
interface WasmParseOptions {
	locations?: boolean;
	goal?: ParseGoal;
}

/** WASM module function signatures */
interface WasmModule {
	parse_svelte: (source: string, options?: WasmParseOptions) => unknown;
	parse_internal_svelte: (source: string, options?: WasmParseOptions) => void;
	format_svelte: (source: string) => string;
	parse_typescript: (source: string, options?: WasmParseOptions) => unknown;
	parse_internal_typescript: (source: string, options?: WasmParseOptions) => void;
	format_typescript: (source: string) => string;
	parse_css: (source: string, options?: WasmParseOptions) => unknown;
	parse_internal_css: (source: string, options?: WasmParseOptions) => void;
	format_css: (source: string) => string;
}

export class WasmImplementation extends BaseImplementation {
	readonly name = 'wasm' as const;
	private _module: WasmModule | null = null;

	readonly parse_languages = LANGUAGES;
	readonly format_languages = LANGUAGES;

	/** Get initialized module or throw */
	private get module(): WasmModule {
		if (!this._module) throw new Error('WASM module not initialized');
		return this._module;
	}

	// Lookup tables for WASM functions by language
	private get parse_fns(): Record<
		Language,
		(source: string, options?: WasmParseOptions) => unknown
	> {
		return {
			svelte: this.module.parse_svelte,
			typescript: this.module.parse_typescript,
			css: this.module.parse_css
		};
	}

	private get parse_internal_fns(): Record<
		Language,
		(source: string, options?: WasmParseOptions) => void
	> {
		return {
			svelte: this.module.parse_internal_svelte,
			typescript: this.module.parse_internal_typescript,
			css: this.module.parse_internal_css
		};
	}

	private get format_fns(): Record<Language, (source: string) => string> {
		return {
			svelte: this.module.format_svelte,
			typescript: this.module.format_typescript,
			css: this.module.format_css
		};
	}

	async init(): Promise<void> {
		const target = current_runtime() === 'deno' ? 'deno' : 'nodejs';
		const wasm_path = fileURLToPath(
			new URL(`../../../crates/tsv_wasm/pkg/all/${target}/tsv_wasm.js`, import.meta.url)
		);

		try {
			await stat(wasm_path);
		} catch {
			throw new Error(
				`WASM module not found at ${wasm_path}. ` +
					`Run 'deno task build:wasm:all:${target}' first.`
			);
		}

		// The deno target is ESM with an explicit `default()` initializer; the
		// nodejs target is CommonJS and self-initializes on require. Load each in
		// its native module system (both resolve to `any`), then read the same
		// function names off both into the typed `WasmModule` shape below.
		let module: WasmModule;
		if (target === 'deno') {
			const esm = await import(wasm_path);
			if (typeof esm.default === 'function') {
				await esm.default();
			}
			module = esm;
		} else {
			module = createRequire(import.meta.url)(wasm_path);
		}

		this._module = {
			parse_svelte: module.parse_svelte,
			parse_internal_svelte: module.parse_internal_svelte,
			format_svelte: module.format_svelte,
			parse_typescript: module.parse_typescript,
			parse_internal_typescript: module.parse_internal_typescript,
			format_typescript: module.format_typescript,
			parse_css: module.parse_css,
			parse_internal_css: module.parse_internal_css,
			format_css: module.format_css
		};

		// Fairness guard for the parse rows: the wasm parse fns must return a
		// js_sys-materialized OBJECT (the engine runs the host's JSON.parse from
		// Rust). If a glue/build regression ever handed back the raw JSON string
		// instead, the timed `tsv_wasm-json` rows would silently skip
		// materialization and read artificially fast vs `tsv-json`. Probe once
		// here, outside any timed loop.
		const probe = this._module.parse_typescript('const x = 1;');
		if (typeof probe !== 'object' || probe === null) {
			throw new Error(
				`tsv_wasm parse returned a ${typeof probe} — expected a materialized AST object`
			);
		}
	}

	// The `goal` option is TypeScript-only (the other languages reject the key),
	// so it's withheld unless the language is typescript.
	parse(source: string, language: Language, goal?: ParseGoal): unknown {
		return this.parse_fns[language](
			source,
			goal && language === 'typescript' ? { goal } : undefined
		);
	}

	parse_internal(source: string, language: Language, goal?: ParseGoal): void {
		this.parse_internal_fns[language](
			source,
			goal && language === 'typescript' ? { goal } : undefined
		);
	}

	parse_no_locations(source: string, language: Language, goal?: ParseGoal): unknown {
		return this.parse_fns[language](source, {
			locations: false,
			goal: goal && language === 'typescript' ? goal : undefined
		});
	}

	format(source: string, language: Language): string {
		return this.format_fns[language](source);
	}

	dispose(): void {
		this._module = null;
	}
}
