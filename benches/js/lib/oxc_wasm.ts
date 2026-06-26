/**
 * OXC WASM implementation wrapper (oxc-parser via wasm32-wasi)
 *
 * Loads @oxc-parser/binding-wasm32-wasi via the entry each runtime can handle
 * (see `init`): Deno gets the fetch-based browser entry, Node/Bun get the
 * default `node:wasi` entry — so the oxc-parser-wasm row runs under all three.
 *
 * Supports: Parse only (TypeScript, JS). No formatting (oxfmt has no WASM variant).
 */

import { type Language, LANGUAGE_EXTENSIONS, type TsvImplementation } from './types.ts';
import type { OxcVersions } from './versions.ts';
import { current_runtime } from './runtime.ts';

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
		// The WASI binding ships two entries: the default CJS entry uses `node:wasi`
		// (Node/Bun support it, Deno doesn't), and an explicit fetch-based browser
		// entry (`parser.wasi-browser.js`, `@napi-rs/wasm-runtime` + `WebAssembly`)
		// that Deno can load but Node can't. Pick per runtime so the
		// oxc-parser-wasm comparison row is available on all three.
		const entry = current_runtime() === 'deno'
			? '@oxc-parser/binding-wasm32-wasi/parser.wasi-browser.js'
			: '@oxc-parser/binding-wasm32-wasi';
		const mod = await import(entry);
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
