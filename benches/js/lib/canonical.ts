/**
 * Canonical implementation wrappers (prettier + svelte/compiler)
 *
 * Uses the same approach as tsv_debug's Deno sidecar for consistency.
 */

import { extname } from 'node:path';
import { PrettierCache, prettier_cache_enabled } from './prettier_cache.ts';
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

/** Parser function type */
type ParserFn = (source: string) => unknown;

/**
 * Prettier formatting options, fixed to tsv's settings and passed explicitly on
 * every call. Mirrors the inline options in the fixture oracle
 * (`crates/tsv_debug/src/deno/sidecar.ts`), the source of truth for fixture
 * correctness — tsv ships no prettier config file, so the corpus oracle reads
 * none either. Keep these two option sets identical.
 */
/** Shared empty plugins array — hoisted so the timed loop doesn't allocate one per call. */
const NO_PLUGINS: unknown[] = [];

const PRETTIER_OPTIONS = {
	useTabs: true,
	printWidth: 100,
	singleQuote: true,
	trailingComma: 'none',
} as const;

export class CanonicalImplementation implements TsvImplementation {
	name = 'canonical' as const;
	readonly versions: CanonicalVersions;

	#prettier: PrettierModule | null = null;
	#format_cache: PrettierCache | null = null;
	// deno-lint-ignore no-explicit-any
	#prettier_svelte: any = null;
	/** The svelte plugin wrapped in its plugins array, hoisted out of the per-call path. */
	#svelte_plugins: unknown[] = [];
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
		// Load dependencies in parallel
		const [
			prettier_mod,
			prettier_svelte_mod,
			svelte_mod,
			acorn_mod,
			acorn_ts_mod,
		] = await Promise.all([
			import('prettier'),
			import('prettier-plugin-svelte'),
			import('svelte/compiler'),
			import('acorn'),
			import('@sveltejs/acorn-typescript'),
		]);
		this.#prettier = prettier_mod as PrettierModule;
		this.#prettier_svelte = prettier_svelte_mod;
		// Hoisted once so the bench's timed loop doesn't allocate a fresh plugins
		// array per format call (the other option fields vary per call and stay inline).
		this.#svelte_plugins = [prettier_svelte_mod];
		this.#svelte_compiler = svelte_mod;
		// Create TypeScript parser once (acorn.Parser.extend is expensive)
		// deno-lint-ignore no-explicit-any
		this.#acorn_ts_parser = acorn_mod.Parser.extend(acorn_ts_mod.tsPlugin() as any);
	}

	/**
	 * Opt into the content-addressed prettier-output cache (`lib/prettier_cache.ts`)
	 * for `format_async` — used by `corpus:compare:format` and the conformance
	 * driver ONLY (never the bench, which times prettier; never the fixture
	 * validator, which live-verifies by design). Returns the cache so the caller
	 * can report hit/miss stats, or null when `TSV_PRETTIER_CACHE=0` disables it.
	 */
	enable_format_cache(): PrettierCache | null {
		if (!prettier_cache_enabled()) return null;
		this.#format_cache = new PrettierCache(this.versions, JSON.stringify(PRETTIER_OPTIONS));
		return this.#format_cache;
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

	async format_async(
		source: string,
		language: Language,
		source_path?: string,
	): Promise<string> {
		if (!this.#prettier_svelte) throw new Error('Prettier Svelte plugin not initialized');

		const plugins = language === 'svelte' ? this.#svelte_plugins : NO_PLUGINS;

		// Real prettier keys its parser off the file extension. The corpus collapses `.js`
		// and `.ts` into one `typescript` Language (tsv formats both through its TS path), but
		// real prettier-on-`.js` uses **babel** (which preserves JSDoc `@type` casts) where
		// prettier-on-`.ts` uses **typescript** (which strips them). When the caller hands us
		// the real path, route a `.js` file through `babel` so the oracle matches what a real
		// on-disk `.js` run produces — otherwise every `.js` file carrying a JSDoc cast reads
		// as the `jsdoc_type_cast_parens` divergence against tsv's (correct) uniform
		// preservation. `.ts`/`.svelte`/`.css` keep the synthetic `file.<ext>`.
		const is_js =
			language === 'typescript' &&
			source_path !== undefined &&
			extname(source_path).toLowerCase() === '.js';
		const parser = is_js ? 'babel' : LANGUAGE_PRETTIER_PARSERS[language];
		const ext = is_js ? '.js' : LANGUAGE_EXTENSIONS[language];

		// Pass a filepath so prettier applies extension-specific heuristics, matching how a
		// real on-disk file is formatted (and how the tsv_debug sidecar invokes prettier). Without
		// it, prettier can't tell a `.ts` file from `.tsx` and force-adds the JSX-disambiguating
		// trailing comma to single-type-param arrows (`<T,>`) that a real `.ts` run never emits —
		// see prettier's `shouldForceTrailingComma` in src/language-js/print/type-parameters.js.
		const filepath = `file${ext}`;

		if (this.#format_cache) {
			const cached = await this.#format_cache.get(source, parser, filepath);
			if (cached !== null) return cached;
		}
		const output = await this.#prettier_checked.format(source, {
			parser,
			filepath,
			plugins,
			...PRETTIER_OPTIONS,
		});
		// Success-only put (a throw above skips it; `put` itself rejects '').
		if (this.#format_cache) await this.#format_cache.put(source, parser, filepath, output);
		return output;
	}

	dispose(): void {
		this.#prettier = null;
		this.#prettier_svelte = null;
		this.#svelte_compiler = null;
		this.#acorn_ts_parser = null;
	}
}
