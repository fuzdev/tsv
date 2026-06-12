/**
 * OXC WASM implementation wrapper (oxc-parser via wasm32-wasi)
 *
 * Uses the browser entry point of @oxc-parser/binding-wasm32-wasi which
 * works in Deno (the default CJS entry uses node:wasi which Deno doesn't support).
 *
 * Supports: Parse only (TypeScript, JS). No formatting (oxfmt has no WASM variant).
 */

import { type Language, LANGUAGE_EXTENSIONS, type TsvImplementation } from './types.ts';
import type { OxcVersions } from './versions.ts';

/** oxc-parser WASM module types (same API as native) */
interface OxcParserWasmModule {
	parseSync: (filename: string, source: string) => { program: unknown; errors: unknown[] };
}

/**
 * OXC WASM implementation using @oxc-parser/binding-wasm32-wasi.
 *
 * Supports:
 * - Parse: TypeScript, JS (NOT Svelte, NOT CSS)
 * - Format: None (oxfmt has no WASM variant)
 */
export class OxcWasmImplementation implements TsvImplementation {
	name = 'oxc-wasm' as const;
	readonly versions: OxcVersions;
	private _parser: OxcParserWasmModule | null = null;

	constructor(versions: OxcVersions) {
		this.versions = versions;
	}

	async init(): Promise<void> {
		const mod = await import('@oxc-parser/binding-wasm32-wasi/parser.wasi-browser.js');
		this._parser = mod as OxcParserWasmModule;
	}

	/** Languages supported for parsing */
	static readonly PARSE_LANGUAGES: Language[] = ['typescript'];

	/** Languages supported for formatting (none) */
	static readonly FORMAT_LANGUAGES: Language[] = [];

	/** Check if parsing is supported for this language */
	supports_parse_language(language: Language): boolean {
		return OxcWasmImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supports_format_language(_language: Language): boolean {
		return false;
	}

	parse(source: string, language: Language): unknown {
		if (!this._parser) throw new Error('OXC WASM parser not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`OXC WASM parser does not support ${language}`);
		}

		const result = this._parser.parseSync(`file${LANGUAGE_EXTENSIONS[language]}`, source);

		if (result.errors && result.errors.length > 0) {
			throw new Error(`Parse errors: ${JSON.stringify(result.errors)}`);
		}

		// Unlike the native `oxc-parser` package (whose `index.js` `wrap()` runs
		// `JSON.parse` on `.program` access), the WASI binding hands back `program`
		// as the raw JSON string the Rust side serialized — it never deserializes.
		// Parse it so `oxc-parser-wasm` materializes a full JS AST, matching what
		// native `oxc-parser` and `tsv_wasm-json` both do (apples-to-apples timing).
		// The string is `{"node": <program>, "fixes": [...]}` (see oxc-parser
		// `src-js/wrap.js`); `.node` is the program.
		const program = result.program;
		return typeof program === 'string' ? JSON.parse(program).node : program;
	}

	format(_source: string, _language: Language): string {
		throw new Error('OXC WASM does not support formatting (oxfmt has no WASM variant)');
	}

	dispose(): void {
		this._parser = null;
	}
}
