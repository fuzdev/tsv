/**
 * Biome implementation wrapper (via WASM)
 *
 * Supports: TypeScript, JavaScript, CSS
 *
 * NOTE: Svelte is disabled because biome's WASM module crashes on Svelte 5
 * syntax ({@render} in {#if} blocks, etc.) and accumulates internal corruption
 * after repeated panics, eventually failing on all files.
 */

import { type Language, LANGUAGE_EXTENSIONS, type TsvImplementation } from './types.ts';
import type { BiomeVersions } from './versions.ts';

// Pre-import WASM package to ensure it's in the module graph
import '@biomejs/wasm-bundler';
import { Biome } from '@biomejs/js-api/bundler';

/** Module-level versions (set by constructor for external access) */
export let BIOME_VERSIONS: BiomeVersions = {
	jsApi: 'unknown',
	wasm: 'unknown',
};

/**
 * Biome implementation using WASM.
 *
 * Supports:
 * - Format: TypeScript, JavaScript, CSS (Svelte disabled due to WASM crashes)
 * - Parse: Not implemented in benchmarks
 */
export class BiomeImplementation implements TsvImplementation {
	name = 'biome' as const;
	private _biome: Biome | null = null;
	private _projectKey: number | null = null;

	/** Languages supported for parsing (none - not implemented) */
	static readonly PARSE_LANGUAGES: Language[] = [];

	/** Languages supported for formatting (Svelte disabled - causes WASM corruption) */
	static readonly FORMAT_LANGUAGES: Language[] = ['typescript', 'css'];

	constructor(versions: BiomeVersions) {
		BIOME_VERSIONS = versions;
	}

	// deno-lint-ignore require-await
	async init(): Promise<void> {
		this._biome = new Biome();
		const { projectKey } = this._biome.openProject('/tmp');
		this._projectKey = projectKey;

		// Configure to match prettier defaults (useTabs) and enable Svelte
		this._biome.applyConfiguration(projectKey, {
			formatter: {
				indentStyle: 'tab',
			},
			javascript: {
				formatter: {
					indentStyle: 'tab',
				},
			},
			css: {
				formatter: {
					indentStyle: 'tab',
				},
			},
			html: {
				experimentalFullSupportEnabled: true,
			},
		});
	}

	/** Check if parsing is supported for this language */
	supportsParseLanguage(language: Language): boolean {
		return BiomeImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supportsFormatLanguage(language: Language): boolean {
		return BiomeImplementation.FORMAT_LANGUAGES.includes(language);
	}

	parse(_source: string, _language: Language): unknown {
		throw new Error('Biome parse not implemented in benchmarks');
	}

	format(source: string, language: Language): string {
		if (!this._biome || !this._projectKey) {
			throw new Error('Biome not initialized');
		}
		if (!this.supportsFormatLanguage(language)) {
			throw new Error(`Biome does not support ${language}`);
		}

		try {
			const result = this._biome.formatContent(this._projectKey, source, {
				filePath: `file${LANGUAGE_EXTENSIONS[language]}`,
			});
			return result.content;
		} catch (e: unknown) {
			// Biome WASM panics have minimal info in the error - the full panic message
			// is printed to stderr by the WASM module (not capturable here).
			// Provide a cleaner error message for the benchmark output.
			if (e && typeof e === 'object' && 'stackTrace' in e) {
				const stackTrace = String((e as { stackTrace: unknown }).stackTrace);
				if (stackTrace.includes('unreachable')) {
					throw new Error('Biome internal error (WASM panic)');
				}
			}
			// For errors with actual messages, pass them through
			if (e instanceof Error && e.message) {
				throw e;
			}
			throw new Error('Biome format failed');
		}
	}

	// deno-lint-ignore require-await
	async formatAsync(source: string, language: Language): Promise<string> {
		return this.format(source, language);
	}

	dispose(): void {
		this._biome = null;
		this._projectKey = null;
	}
}
