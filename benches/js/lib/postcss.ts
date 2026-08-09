/**
 * postcss implementation wrapper (JS)
 *
 * The only third-party engine on the `parse/css` surface — the rest of that group
 * is `svelte/compiler`'s `parseCss` (the reference row) and tsv's own variants.
 *
 * **It earns the row on one argument**: postcss is the parser behind prettier's
 * CSS printer, and prettier is the `format/css` baseline — so this is the parse
 * surface's counterpart to that baseline. (A standalone install, not the copy
 * prettier bundles, so read it as "the postcss engine at the pinned version",
 * not as prettier's internals.)
 *
 * **Why there is no native peer on this surface.** No Rust CSS parser exposes an
 * AST to JS at all: lightningcss ships `transform`/`bundle` only (its `./ast`
 * export is types for the `visitor` callback option — a napi round-trip per
 * visited node, not a parse product), biome's `js-api` exposes
 * `formatContent`/`lintContent`/`openProject`, malva is a formatter, and oxc's CSS
 * is `oxc_formatter_css` with no JS parse binding. So `parse/css` having a JS-only
 * alternative is an availability fact rather than an omission.
 *
 * **Not payload-matched.** postcss's `Root` is a CSSOM-ish tree with `nodes` and
 * `raws`, not the `parseCss` shape tsv is a drop-in for, so a ratio against
 * `tsv-json` compares two different products. Same disclosure class as the oxc and
 * swc rows.
 *
 * `css-tree` was evaluated for this slot and rejected: it parses an unclosed block
 * without error even with `onParseError` supplied, so its accept rate would read
 * ~100% vacuously. postcss throws on the same input, so its coverage number
 * carries information.
 */

import { createRequire } from 'node:module';
import { BaseImplementation, type Language } from './types.ts';
import type { PostcssVersions } from './versions.ts';

interface PostcssModule {
	parse: (source: string, options?: { from?: string }) => unknown;
}

/**
 * postcss's CSS parser.
 *
 * Supports:
 * - Parse: CSS only
 * - Format: unsupported — postcss is a parser plus a stringifier, not a
 *   layout-deciding formatter (prettier's printer is what makes layout decisions,
 *   and prettier already has the baseline row).
 */
export class PostcssImplementation extends BaseImplementation {
	readonly name = 'postcss' as const;
	readonly versions: PostcssVersions;
	private _postcss: PostcssModule | null = null;

	readonly parse_languages: ReadonlyArray<Language> = ['css'];
	readonly format_languages: ReadonlyArray<Language> = [];

	constructor(versions: PostcssVersions) {
		super();
		this.versions = versions;
	}

	// deno-lint-ignore require-await
	async init(): Promise<void> {
		const require = createRequire(import.meta.url);
		const postcss = require('postcss') as PostcssModule;
		if (typeof postcss.parse !== 'function') {
			throw new Error('postcss exposes no parse()');
		}
		postcss.parse('a{color:red}');
		this._postcss = postcss;
	}

	parse(source: string, language: Language): unknown {
		if (!this._postcss) throw new Error('postcss not initialized');
		if (!this.supports_parse_language(language)) {
			throw new Error(`postcss does not support ${language}`);
		}
		// No `from`: postcss only uses it for source maps and error messages, and
		// passing a synthetic path would add a per-call string for no measured work.
		return this._postcss.parse(source);
	}

	dispose(): void {
		this._postcss = null;
	}
}
