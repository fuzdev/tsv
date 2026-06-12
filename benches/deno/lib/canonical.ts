/**
 * Canonical implementation wrappers (prettier + svelte/compiler)
 *
 * Uses the same approach as tsv_debug's Deno sidecar for consistency.
 */

import {
	type Language,
	LANGUAGE_EXTENSIONS,
	LANGUAGE_PRETTIER_PARSERS,
	type TsvImplementation,
} from './types.ts';
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
async function load_prettier_config(): Promise<PrettierConfig> {
	const config_path = new URL('../../../.prettierrc.json', import.meta.url).pathname;
	try {
		const content = await Deno.readTextFile(config_path);
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
	#prettier_config: PrettierConfig = {};
	// deno-lint-ignore no-explicit-any
	#prettier_svelte: any = null;
	// deno-lint-ignore no-explicit-any
	#svelte_compiler: any = null;
	// deno-lint-ignore no-explicit-any
	#acorn_ts_parser: any = null;

	/** Languages supported for parsing */
	static readonly PARSE_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	/** Languages supported for formatting */
	static readonly FORMAT_LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

	constructor(versions: CanonicalVersions) {
		this.versions = versions;
	}

	/** Get initialized prettier or throw */
	get #prettier_checked(): PrettierModule {
		if (!this.#prettier) throw new Error('Prettier not initialized');
		return this.#prettier;
	}

	async init(): Promise<void> {
		// Load config and dependencies in parallel
		const [
			prettier_config,
			prettier_mod,
			prettier_svelte_mod,
			svelte_mod,
			acorn_mod,
			acorn_ts_mod,
		] = await Promise.all([
			load_prettier_config(),
			import('prettier'),
			import('prettier-plugin-svelte'),
			import('svelte/compiler'),
			import('acorn'),
			import('@sveltejs/acorn-typescript'),
		]);
		this.#prettier_config = prettier_config;
		this.#prettier = prettier_mod as PrettierModule;
		this.#prettier_svelte = prettier_svelte_mod;
		this.#svelte_compiler = svelte_mod;
		// Create TypeScript parser once (acorn.Parser.extend is expensive)
		// deno-lint-ignore no-explicit-any
		this.#acorn_ts_parser = acorn_mod.Parser.extend(acorn_ts_mod.tsPlugin() as any);
	}

	/** Check if parsing is supported for this language */
	supports_parse_language(language: Language): boolean {
		return CanonicalImplementation.PARSE_LANGUAGES.includes(language);
	}

	/** Check if formatting is supported for this language */
	supports_format_language(language: Language): boolean {
		return CanonicalImplementation.FORMAT_LANGUAGES.includes(language);
	}

	// Lookup table for parse functions by language
	get #parse_fns(): Record<Language, ParserFn> {
		return {
			svelte: (source) => {
				if (!this.#svelte_compiler) throw new Error('Svelte compiler not initialized');
				return this.#svelte_compiler.parse(source, { modern: true });
			},
			typescript: (source) => {
				if (!this.#acorn_ts_parser) throw new Error('Acorn not initialized');
				return this.#acorn_ts_parser.parse(source, {
					sourceType: 'module',
					ecmaVersion: 2025,
					locations: true,
				});
			},
			css: (source) => {
				if (!this.#svelte_compiler) throw new Error('Svelte compiler not initialized');
				return this.#svelte_compiler.parseCss(source);
			},
		};
	}

	parse(source: string, language: Language): unknown {
		return this.#parse_fns[language](source);
	}

	async format_async(source: string, language: Language): Promise<string> {
		if (!this.#prettier_svelte) throw new Error('Prettier Svelte plugin not initialized');

		const plugins = language === 'svelte' ? [this.#prettier_svelte] : [];

		// Pass a filepath so prettier applies extension-specific heuristics, matching how a
		// real on-disk file is formatted (and how the tsv_debug sidecar invokes prettier). Without
		// it, prettier can't tell a `.ts` file from `.tsx` and force-adds the JSX-disambiguating
		// trailing comma to single-type-param arrows (`<T,>`) that a real `.ts` run never emits —
		// see prettier's `shouldForceTrailingComma` in src/language-js/print/type-parameters.js.
		return await this.#prettier_checked.format(source, {
			parser: LANGUAGE_PRETTIER_PARSERS[language],
			filepath: `file${LANGUAGE_EXTENSIONS[language]}`,
			plugins,
			...this.#prettier_config,
		});
	}

	dispose(): void {
		this.#prettier = null;
		this.#prettier_svelte = null;
		this.#svelte_compiler = null;
		this.#acorn_ts_parser = null;
	}
}
