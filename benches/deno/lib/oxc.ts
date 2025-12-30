/**
 * OXC implementation wrappers (oxc-parser + oxfmt)
 *
 * oxc-parser: Fast TypeScript/JavaScript parser
 * oxfmt: Fast TypeScript/JavaScript/CSS formatter
 *
 * Note: Neither supports Svelte files.
 */

import { type Language, LANGUAGE_EXTENSIONS, type TsvImplementation } from './types.ts';
import type { OxcVersions } from './versions.ts';

/** oxc-parser module types */
interface OxcParserModule {
	parseSync: (filename: string, source: string) => { program: unknown; errors: unknown[] };
}

/** oxfmt module types */
interface OxfmtModule {
	format: (
		filename: string,
		source: string,
		options?: { useTabs?: boolean },
	) => Promise<{ code: string; errors: unknown[] }>;
}

/**
 * OXC implementation using oxc-parser and oxfmt.
 *
 * Supports:
 * - Parse: TypeScript, JavaScript (NOT Svelte, NOT CSS)
 * - Format: TypeScript, JavaScript, CSS (NOT Svelte)
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
		const [parserMod, formatterMod] = await Promise.all([import('oxc-parser'), import('oxfmt')]);

		this._parser = parserMod as OxcParserModule;
		this._formatter = formatterMod as OxfmtModule;
	}

	/** Languages supported for parsing */
	static readonly PARSE_LANGUAGES: Language[] = ['typescript'];

	/** Languages supported for formatting */
	static readonly FORMAT_LANGUAGES: Language[] = ['typescript', 'css'];

	/** Check if parsing is supported for this language */
	supportsParseLanguage(language: Language): boolean {
		return OxcImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supportsFormatLanguage(language: Language): boolean {
		return OxcImplementation.FORMAT_LANGUAGES.includes(language);
	}

	parse(source: string, language: Language): unknown {
		if (!this._parser) throw new Error('OXC parser not initialized');
		if (!this.supportsParseLanguage(language)) {
			throw new Error(`OXC parser does not support ${language}`);
		}

		const result = this._parser.parseSync(`file${LANGUAGE_EXTENSIONS[language]}`, source);

		if (result.errors && result.errors.length > 0) {
			throw new Error(`Parse errors: ${JSON.stringify(result.errors)}`);
		}

		return result.program;
	}

	format(_source: string, _language: Language): string {
		// oxfmt is async, so we can't implement sync format
		throw new Error('OXC formatter is async-only, use formatAsync');
	}

	async formatAsync(source: string, language: Language): Promise<string> {
		if (!this._formatter) throw new Error('OXC formatter not initialized');
		if (!this.supportsFormatLanguage(language)) {
			throw new Error(`OXC formatter does not support ${language}`);
		}

		const result = await this._formatter.format(`file${LANGUAGE_EXTENSIONS[language]}`, source, {
			useTabs: true,
		});

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
