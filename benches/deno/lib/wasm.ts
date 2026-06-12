/**
 * WASM bindings to tsv
 *
 * Uses wasm-pack generated bindings for WebAssembly performance testing.
 */

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

	async init(): Promise<void> {
		const wasm_path = new URL(
			'../../../crates/tsv_wasm/pkg/all/deno/tsv_wasm.js',
			import.meta.url,
		).pathname;

		try {
			await Deno.stat(wasm_path);
		} catch {
			throw new Error(
				`WASM module not found at ${wasm_path}. ` +
					`Run 'deno task build:wasm:all:deno' first.`,
			);
		}

		// Dynamic import of wasm-pack generated module
		const module = await import(wasm_path);

		// wasm-pack for Deno generates a default export that initializes the module
		if (typeof module.default === 'function') {
			await module.default();
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
		};
	}

	parse(source: string, language: Language): unknown {
		return this.parse_fns[language](source);
	}

	parse_internal(source: string, language: Language): void {
		this.parse_internal_fns[language](source);
	}

	format(source: string, language: Language): string {
		return this.format_fns[language](source);
	}

	dispose(): void {
		this._module = null;
	}
}
