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
import type { Language, TsvImplementation } from './types.ts';

/** WASM module function signatures */
interface WasmModule {
	parse_svelte: (source: string) => unknown;
	parse_internal_svelte: (source: string) => void;
	format_svelte: (source: string) => string;
	parse_typescript: (source: string) => unknown;
	parse_internal_typescript: (source: string) => void;
	format_typescript: (source: string) => string;
	parse_css: (source: string) => unknown;
	parse_internal_css: (source: string) => void;
	format_css: (source: string) => string;
	// span-only wire, materialized in Rust (mechanism-matched with parse_*) —
	// svelte + typescript only (CSS emits no `loc`)
	parse_svelte_no_locations: (source: string) => unknown;
	parse_typescript_no_locations: (source: string) => unknown;
}

export class WasmImplementation implements TsvImplementation {
	name = 'wasm' as const;
	private _module: WasmModule | null = null;

	/** Languages supported for parsing */
	static readonly PARSE_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	/** Languages supported for formatting */
	static readonly FORMAT_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	/** Get initialized module or throw */
	private get module(): WasmModule {
		if (!this._module) throw new Error('WASM module not initialized');
		return this._module;
	}

	/** Check if parsing is supported for this language */
	supports_parse_language(language: Language): boolean {
		return WasmImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supports_format_language(language: Language): boolean {
		return WasmImplementation.FORMAT_LANGUAGES.includes(language);
	}

	// Lookup tables for WASM functions by language
	private get parse_fns(): Record<Language, (source: string) => unknown> {
		return {
			svelte: this.module.parse_svelte,
			typescript: this.module.parse_typescript,
			css: this.module.parse_css,
		};
	}

	private get parse_internal_fns(): Record<Language, (source: string) => void> {
		return {
			svelte: this.module.parse_internal_svelte,
			typescript: this.module.parse_internal_typescript,
			css: this.module.parse_internal_css,
		};
	}

	private get format_fns(): Record<Language, (source: string) => string> {
		return {
			svelte: this.module.format_svelte,
			typescript: this.module.format_typescript,
			css: this.module.format_css,
		};
	}

	// Span-only wire — svelte + typescript only (CSS has no `loc`).
	private get parse_no_locations_fns(): Partial<Record<Language, (source: string) => unknown>> {
		return {
			svelte: this.module.parse_svelte_no_locations,
			typescript: this.module.parse_typescript_no_locations,
		};
	}

	async init(): Promise<void> {
		const target = current_runtime() === 'deno' ? 'deno' : 'nodejs';
		const wasm_path = fileURLToPath(
			new URL(`../../../crates/tsv_wasm/pkg/all/${target}/tsv_wasm.js`, import.meta.url),
		);

		try {
			await stat(wasm_path);
		} catch {
			throw new Error(
				`WASM module not found at ${wasm_path}. ` +
					`Run 'deno task build:wasm:all:${target}' first.`,
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
			format_css: module.format_css,
			parse_svelte_no_locations: module.parse_svelte_no_locations,
			parse_typescript_no_locations: module.parse_typescript_no_locations,
		};
	}

	parse(source: string, language: Language): unknown {
		return this.parse_fns[language](source);
	}

	parse_internal(source: string, language: Language): void {
		this.parse_internal_fns[language](source);
	}

	parse_no_locations(source: string, language: Language): unknown {
		const fn = this.parse_no_locations_fns[language];
		if (!fn) throw new Error(`no-locations parse unsupported for ${language}`);
		return fn(source);
	}

	format(source: string, language: Language): string {
		return this.format_fns[language](source);
	}

	dispose(): void {
		this._module = null;
	}
}
