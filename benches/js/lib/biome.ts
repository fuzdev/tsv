/**
 * Biome implementation wrapper (via WASM)
 *
 * Supports: TypeScript, JS, CSS, Svelte
 */

import { type Language, LANGUAGE_EXTENSIONS, type TsvImplementation } from './types.ts';
import type { BiomeVersions } from './versions.ts';
// Type-only — `import type` is erased, so referencing `Biome` here does NOT load
// the WASM package at this module's import. The value import is deferred to
// `init()` (see there) so a load-time crash can't escape the registry's skip.
import type { Biome } from '@biomejs/js-api/bundler';

/**
 * Biome implementation using WASM.
 *
 * Supports:
 * - Format: Svelte, TypeScript, JS, CSS
 * - Parse: unsupported — the `@biomejs/js-api` package exposes no parse entry
 *   point (only `formatContent`/`lintContent`/`fixFile`); Biome parses
 *   internally but never surfaces the AST across the JS boundary.
 */
export class BiomeImplementation implements TsvImplementation {
	name = 'biome-wasm' as const;
	readonly versions: BiomeVersions;
	private _biome: Biome | null = null;
	private _project_key: number | null = null;

	/** Languages supported for parsing (none — the js-api exposes no parser) */
	static readonly PARSE_LANGUAGES: Language[] = [];

	/** Languages supported for formatting */
	static readonly FORMAT_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	constructor(versions: BiomeVersions) {
		this.versions = versions;
	}

	async init(): Promise<void> {
		// Load the WASM package + js-api lazily (not as static top-level imports)
		// so a load-time failure — e.g. Bun's wasm-bindgen-`start` incompatibility
		// with `@biomejs/wasm-bundler` — throws HERE, inside init_implementations'
		// per-impl try/catch (and is skipped), instead of throwing during this
		// module's static import graph and aborting the whole registry. The
		// wasm-bundler import runs first so it's registered before js-api loads.
		await import('@biomejs/wasm-bundler');
		const { Biome } = await import('@biomejs/js-api/bundler');
		this._biome = new Biome();
		const { projectKey } = this._biome.openProject('/tmp');
		this._project_key = projectKey;

		// Configure to match prettier defaults (useTabs) and enable Svelte/HTML
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
	supports_parse_language(language: Language): boolean {
		return BiomeImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supports_format_language(language: Language): boolean {
		return BiomeImplementation.FORMAT_LANGUAGES.includes(language);
	}

	parse(_source: string, _language: Language): unknown {
		throw new Error('Biome has no parser: the @biomejs/js-api package exposes no parse API');
	}

	format(source: string, language: Language): string {
		if (!this._biome || !this._project_key) {
			throw new Error('Biome not initialized');
		}
		if (!this.supports_format_language(language)) {
			throw new Error(`Biome does not support ${language}`);
		}

		try {
			const result = this._biome.formatContent(this._project_key, source, {
				filePath: `file${LANGUAGE_EXTENSIONS[language]}`,
			});
			return result.content;
		} catch (e: unknown) {
			// Biome WASM panics have minimal info in the error - the full panic message
			// is printed to stderr by the WASM module (not capturable here).
			// Provide a cleaner error message for the benchmark output.
			if (e && typeof e === 'object' && 'stackTrace' in e) {
				const stack_trace = String((e as { stackTrace: unknown }).stackTrace);
				if (stack_trace.includes('unreachable')) {
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
	async format_async(source: string, language: Language): Promise<string> {
		return this.format(source, language);
	}

	dispose(): void {
		this._biome = null;
		this._project_key = null;
	}
}
