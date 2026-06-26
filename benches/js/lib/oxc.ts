/**
 * OXC implementation wrappers (oxc-parser + oxfmt)
 *
 * oxc-parser: Fast TypeScript/JS parser
 * oxfmt: Fast TypeScript/JS/CSS/Svelte formatter (Svelte is experimental as of 0.49)
 */

import { type Language, LANGUAGE_EXTENSIONS, type TsvImplementation } from './types.ts';
import type { OxcVersions } from './versions.ts';

/** oxc-parser module types */
interface OxcParserModule {
	parseSync: (filename: string, source: string) => { program: unknown; errors: unknown[] };
}

/** oxfmt module types */
interface OxfmtFormatOptions {
	useTabs?: boolean;
	/** Enable experimental Svelte support — `{}` accepts defaults. */
	svelte?: boolean | Record<string, unknown>;
}

interface OxfmtModule {
	format: (
		filename: string,
		source: string,
		options?: OxfmtFormatOptions,
	) => Promise<{ code: string; errors: unknown[] }>;
}

/**
 * OXC implementation using oxc-parser and oxfmt.
 *
 * Supports:
 * - Parse: TypeScript, JS (NOT Svelte, NOT CSS)
 * - Format: TypeScript, JS, CSS, Svelte (Svelte is experimental, expect partial coverage)
 */
export class OxcImplementation implements TsvImplementation {
	name = 'oxc' as const;
	readonly versions: OxcVersions;
	private _parser: OxcParserModule | null = null;
	private _formatter: OxfmtModule | null = null;

	constructor(versions: OxcVersions) {
		this.versions = versions;
	}

	async init(): Promise<void> {
		const [parser_mod, formatter_mod] = await Promise.all([import('oxc-parser'), import('oxfmt')]);

		this._parser = parser_mod as OxcParserModule;
		this._formatter = formatter_mod as OxfmtModule;
	}

	/** Languages supported for parsing */
	static readonly PARSE_LANGUAGES: Language[] = ['typescript'];

	/** Languages supported for formatting */
	static readonly FORMAT_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	/** Check if parsing is supported for this language */
	supports_parse_language(language: Language): boolean {
		return OxcImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supports_format_language(language: Language): boolean {
		return OxcImplementation.FORMAT_LANGUAGES.includes(language);
	}

	parse(source: string, language: Language): unknown {
		if (!this._parser) throw new Error('OXC parser not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`OXC parser does not support ${language}`);
		}

		const result = this._parser.parseSync(`file${LANGUAGE_EXTENSIONS[language]}`, source);

		if (result.errors && result.errors.length > 0) {
			throw new Error(`Parse errors: ${JSON.stringify(result.errors)}`);
		}

		// Accessing `.program` runs the package's `wrap()` getter, which `JSON.parse`s
		// the Rust-serialized AST — a full eager materialization (matching `tsv-json`,
		// so the `oxc-parser` row is apples-to-apples with it). There is deliberately no
		// lazy variant: oxc's `experimentalLazy` raw transfer is setup-dominated
		// (~1.7ms/call on Node, ~2.1ms on Deno, vs ~0.7ms eager + ~0.16ms parse-only) —
		// it eagerly copies the whole AST transfer buffer, so it measures buffer setup,
		// not parse speed, in any runtime. See `benches/js/CLAUDE.md` → Fairness Caveats.
		return result.program;
	}

	format(_source: string, _language: Language): string {
		// oxfmt is async, so we can't implement sync format
		throw new Error('OXC formatter is async-only, use format_async');
	}

	async format_async(source: string, language: Language): Promise<string> {
		if (!this._formatter) throw new Error('OXC formatter not initialized');
		if (!this.supports_format_language(language)) {
			throw new Error(`OXC formatter does not support ${language}`);
		}

		const options: OxfmtFormatOptions = { useTabs: true };
		// oxfmt gates .svelte handling behind the `svelte` config key (experimental as of 0.49).
		if (language === 'svelte') options.svelte = {};

		const result = await this._formatter.format(
			`file${LANGUAGE_EXTENSIONS[language]}`,
			source,
			options,
		);

		if (result.errors && result.errors.length > 0) {
			throw new Error(`Format errors: ${JSON.stringify(result.errors)}`);
		}

		return result.code;
	}

	dispose(): void {
		this._parser = null;
		this._formatter = null;
	}
}
