/**
 * WASM bindings to tsv
 *
 * Uses wasm-pack generated bindings for WebAssembly performance testing.
 */

import type { Language, TsvImplementation } from './types.ts';

// These will be dynamically imported from the wasm-pack output
let wasmModule: {
	parse_svelte: (source: string) => unknown;
	parse_internal_svelte: (source: string) => void;
	format_svelte: (source: string) => string;
	parse_typescript: (source: string) => unknown;
	parse_internal_typescript: (source: string) => void;
	format_typescript: (source: string) => string;
	parse_css: (source: string) => unknown;
	parse_internal_css: (source: string) => void;
	format_css: (source: string) => string;
} | null = null;

export class WasmImplementation implements TsvImplementation {
	name = 'wasm' as const;

	async init(): Promise<void> {
		const wasmPath = new URL(
			'../../../crates/tsv_wasm/pkg/tsv_wasm.js',
			import.meta.url,
		).pathname;

		try {
			await Deno.stat(wasmPath);
		} catch {
			throw new Error(
				`WASM module not found at ${wasmPath}. ` +
					`Run 'wasm-pack build crates/tsv_wasm --target deno --release' first.`,
			);
		}

		// Dynamic import of wasm-pack generated module
		const module = await import(wasmPath);

		// wasm-pack for Deno generates a default export that initializes the module
		if (typeof module.default === 'function') {
			await module.default();
		}

		wasmModule = {
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
		if (!wasmModule) throw new Error('WASM module not initialized');

		switch (language) {
			case 'svelte':
				return wasmModule.parse_svelte(source);
			case 'typescript':
				return wasmModule.parse_typescript(source);
			case 'css':
				return wasmModule.parse_css(source);
		}
	}

	parseInternal(source: string, language: Language): void {
		if (!wasmModule) throw new Error('WASM module not initialized');

		switch (language) {
			case 'svelte':
				wasmModule.parse_internal_svelte(source);
				break;
			case 'typescript':
				wasmModule.parse_internal_typescript(source);
				break;
			case 'css':
				wasmModule.parse_internal_css(source);
				break;
		}
	}

	format(source: string, language: Language): string {
		if (!wasmModule) throw new Error('WASM module not initialized');

		switch (language) {
			case 'svelte':
				return wasmModule.format_svelte(source);
			case 'typescript':
				return wasmModule.format_typescript(source);
			case 'css':
				return wasmModule.format_css(source);
		}
	}

	dispose(): void {
		wasmModule = null;
	}
}
