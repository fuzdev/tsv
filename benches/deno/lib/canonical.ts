/**
 * Canonical implementation wrappers (prettier + svelte/compiler)
 *
 * Uses the same approach as tsv_debug's Deno sidecar for consistency.
 */

import { type Language, LANGUAGE_PRETTIER_PARSERS, type TsvImplementation } from './types.ts';
import type { CanonicalVersions } from './versions.ts';

/** Prettier module */
interface PrettierModule {
	format: (source: string, options: Record<string, unknown>) => Promise<string>;
}

/** Parser function type */
type ParserFn = (source: string) => unknown;

export class CanonicalImplementation implements TsvImplementation {
	name = 'canonical' as const;
	readonly versions: CanonicalVersions;
	private _prettier: PrettierModule | null = null;

	/** Languages supported for parsing */
	static readonly PARSE_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	/** Languages supported for formatting */
	static readonly FORMAT_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];
	// deno-lint-ignore no-explicit-any
	private _prettierSvelte: any = null;
	// deno-lint-ignore no-explicit-any
	private _svelteCompiler: any = null;
	// deno-lint-ignore no-explicit-any
	private _acornTsParser: any = null;

	constructor(versions: CanonicalVersions) {
		this.versions = versions;
	}

	/** Get initialized prettier or throw */
	private get prettier(): PrettierModule {
		if (!this._prettier) throw new Error('Prettier not initialized');
		return this._prettier;
	}

	async init(): Promise<void> {
		// Import all dependencies via deno.json import map
		const [prettierMod, prettierSvelteMod, svelteMod, acornMod, acornTsMod] = await Promise.all([
			import('prettier'),
			import('prettier-plugin-svelte'),
			import('svelte/compiler'),
			import('acorn'),
			import('@sveltejs/acorn-typescript'),
		]);
		this._prettier = prettierMod as PrettierModule;
		this._prettierSvelte = prettierSvelteMod;
		this._svelteCompiler = svelteMod;
		// Create TypeScript parser once (acorn.Parser.extend is expensive)
		// deno-lint-ignore no-explicit-any
		this._acornTsParser = acornMod.Parser.extend(acornTsMod.tsPlugin() as any);
	}

	/** Check if parsing is supported for this language */
	supportsParseLanguage(language: Language): boolean {
		return CanonicalImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supportsFormatLanguage(language: Language): boolean {
		return CanonicalImplementation.FORMAT_LANGUAGES.includes(language);
	}

	// Lookup table for parse functions by language
	private get parseFns(): Record<Language, ParserFn> {
		return {
			svelte: (source) => {
				if (!this._svelteCompiler) throw new Error('Svelte compiler not initialized');
				return this._svelteCompiler.parse(source, { modern: true });
			},
			typescript: (source) => {
				if (!this._acornTsParser) throw new Error('Acorn not initialized');
				return this._acornTsParser.parse(source, {
					sourceType: 'module',
					ecmaVersion: 2025,
					locations: true,
				});
			},
			css: (source) => {
				// Wrap CSS in <style> tags and parse as Svelte to get CSS AST
				if (!this._svelteCompiler) throw new Error('Svelte compiler not initialized');
				return this._svelteCompiler.parse(`<style>${source}</style>`, { modern: true });
			},
		};
	}

	parse(source: string, language: Language): unknown {
		return this.parseFns[language](source);
	}

	async formatAsync(source: string, language: Language): Promise<string> {
		if (!this._prettierSvelte) throw new Error('Prettier Svelte plugin not initialized');

		const plugins = language === 'svelte' ? [this._prettierSvelte] : [];

		return await this.prettier.format(source, {
			parser: LANGUAGE_PRETTIER_PARSERS[language],
			plugins,
			useTabs: true,
		});
	}

	dispose(): void {
		this._prettier = null;
		this._prettierSvelte = null;
		this._svelteCompiler = null;
		this._acornTsParser = null;
	}
}
