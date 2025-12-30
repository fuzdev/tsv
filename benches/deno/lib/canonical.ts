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

/** Prettier config options we care about */
interface PrettierConfig {
	useTabs?: boolean;
	printWidth?: number;
	singleQuote?: boolean;
	bracketSpacing?: boolean;
}

/** Parser function type */
type ParserFn = (source: string) => unknown;

/**
 * Load prettier config from .prettierrc.json at project root.
 * Falls back to defaults if file not found.
 */
async function loadPrettierConfig(): Promise<PrettierConfig> {
	const configPath = new URL('../../../.prettierrc.json', import.meta.url).pathname;
	try {
		const content = await Deno.readTextFile(configPath);
		const config = JSON.parse(content);
		// Extract only the options we care about (not plugins - we handle those separately)
		return {
			useTabs: config.useTabs,
			printWidth: config.printWidth,
			singleQuote: config.singleQuote,
			bracketSpacing: config.bracketSpacing,
		};
	} catch {
		// Fall back to defaults matching our .prettierrc.json
		return {
			useTabs: true,
			printWidth: 100,
			singleQuote: true,
			bracketSpacing: false,
		};
	}
}

export class CanonicalImplementation implements TsvImplementation {
	name = 'canonical' as const;
	readonly versions: CanonicalVersions;

	#prettier: PrettierModule | null = null;
	#prettierConfig: PrettierConfig = {};
	// deno-lint-ignore no-explicit-any
	#prettierSvelte: any = null;
	// deno-lint-ignore no-explicit-any
	#svelteCompiler: any = null;
	// deno-lint-ignore no-explicit-any
	#acornTsParser: any = null;

	/** Languages supported for parsing */
	static readonly PARSE_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	/** Languages supported for formatting */
	static readonly FORMAT_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	constructor(versions: CanonicalVersions) {
		this.versions = versions;
	}

	/** Get initialized prettier or throw */
	get #prettierChecked(): PrettierModule {
		if (!this.#prettier) throw new Error('Prettier not initialized');
		return this.#prettier;
	}

	async init(): Promise<void> {
		// Load config and dependencies in parallel
		const [prettierConfig, prettierMod, prettierSvelteMod, svelteMod, acornMod, acornTsMod] =
			await Promise.all([
				loadPrettierConfig(),
				import('prettier'),
				import('prettier-plugin-svelte'),
				import('svelte/compiler'),
				import('acorn'),
				import('@sveltejs/acorn-typescript'),
			]);
		this.#prettierConfig = prettierConfig;
		this.#prettier = prettierMod as PrettierModule;
		this.#prettierSvelte = prettierSvelteMod;
		this.#svelteCompiler = svelteMod;
		// Create TypeScript parser once (acorn.Parser.extend is expensive)
		// deno-lint-ignore no-explicit-any
		this.#acornTsParser = acornMod.Parser.extend(acornTsMod.tsPlugin() as any);
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
	get #parseFns(): Record<Language, ParserFn> {
		return {
			svelte: (source) => {
				if (!this.#svelteCompiler) throw new Error('Svelte compiler not initialized');
				return this.#svelteCompiler.parse(source, { modern: true });
			},
			typescript: (source) => {
				if (!this.#acornTsParser) throw new Error('Acorn not initialized');
				return this.#acornTsParser.parse(source, {
					sourceType: 'module',
					ecmaVersion: 2025,
					locations: true,
				});
			},
			css: (source) => {
				// Wrap CSS in <style> tags and parse as Svelte to get CSS AST
				if (!this.#svelteCompiler) throw new Error('Svelte compiler not initialized');
				return this.#svelteCompiler.parse(`<style>${source}</style>`, { modern: true });
			},
		};
	}

	parse(source: string, language: Language): unknown {
		return this.#parseFns[language](source);
	}

	async formatAsync(source: string, language: Language): Promise<string> {
		if (!this.#prettierSvelte) throw new Error('Prettier Svelte plugin not initialized');

		const plugins = language === 'svelte' ? [this.#prettierSvelte] : [];

		return await this.#prettierChecked.format(source, {
			parser: LANGUAGE_PRETTIER_PARSERS[language],
			plugins,
			...this.#prettierConfig,
		});
	}

	dispose(): void {
		this.#prettier = null;
		this.#prettierSvelte = null;
		this.#svelteCompiler = null;
		this.#acornTsParser = null;
	}
}
