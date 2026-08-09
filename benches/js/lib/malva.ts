/**
 * malva implementation wrapper (via WASM)
 *
 * `dprint-plugin-malva` is dprint's CSS formatter, loaded in-process as its Wasm
 * plugin through the same `@dprint/formatter` host `lib/dprint.ts` uses — so it
 * costs one more plugin wasm and no new machinery. It is one of two wasm-tier
 * engines on `format/css`, the other being `biome-wasm`. dprint's HTML plugin
 * stays unwired: it does not format Svelte.
 *
 * **CSS only, and enforced by the plugin itself.** malva matches CSS/SCSS/Less
 * extensions and rejects anything else outright with "unknown file extension"
 * (verified for `.svelte` and `.ts`), exactly as `@dprint/typescript` rejects CSS
 * and Svelte. So the language list here is not a policy choice the wrapper could
 * get wrong — the plugin would refuse.
 */

import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { BaseImplementation, type Language } from './types.ts';
import type { MalvaVersions } from './versions.ts';
// Type-only, so naming `Formatter` here does not load the plugin at import time;
// the value imports are deferred to `init()`. Same posture as lib/dprint.ts.
import type { Formatter } from '@dprint/formatter';

/**
 * malva CSS formatter.
 *
 * Supports:
 * - Format: CSS only
 * - Parse: unsupported — dprint's Wasm plugin protocol exposes `format_text` and
 *   config entry points, never an AST across the boundary.
 */
export class MalvaImplementation extends BaseImplementation {
	readonly name = 'malva-wasm' as const;
	readonly versions: MalvaVersions;
	private _formatter: Formatter | null = null;

	/** The Wasm plugin exposes no parser. */
	readonly parse_languages: ReadonlyArray<Language> = [];
	readonly format_languages: ReadonlyArray<Language> = ['css'];

	constructor(versions: MalvaVersions) {
		super();
		this.versions = versions;
	}

	async init(): Promise<void> {
		const { createFromBuffer } = await import('@dprint/formatter');
		// The package ships only `*.wasm` (no JS entry, so no `getPath()` helper like
		// `@dprint/typescript` has) — resolve the wasm file directly.
		const require = createRequire(import.meta.url);
		const wasm_path = require.resolve('dprint-plugin-malva/plugin.wasm');
		this._formatter = createFromBuffer(await readFile(wasm_path));

		// Match the prettier/tsv layout targets so every format row does the same
		// work — see docs/benchmarks.md §Fairness caveats. `lineWidth`/`indentWidth`/
		// `useTabs` are dprint GLOBAL config; `quotes` is malva's plugin config, and
		// `preferSingle` is the faithful analogue of prettier's `singleQuote: true`
		// (it still switches quotes to avoid escaping).
		this._formatter.setConfig(
			{ lineWidth: 100, indentWidth: 2, useTabs: true },
			{ quotes: 'preferSingle' }
		);

		// Assert the config LANDED, for the same reason lib/dprint.ts does: dprint
		// reports an unrecognized key as a diagnostic rather than throwing (verified
		// — a bogus key yields `Unknown property in configuration`), so a renamed key
		// would silently leave an option at its default and skew this row against
		// every other formatter.
		const diagnostics = this._formatter.getConfigDiagnostics();
		if (diagnostics.length > 0) {
			const detail = diagnostics.map((d) => `${d.propertyName}: ${d.message}`).join('; ');
			throw new Error(`malva rejected the benchmark config (${detail})`);
		}
	}

	parse(_source: string, _language: Language): unknown {
		throw new Error('malva has no parser: the Wasm plugin exposes no parse API');
	}

	format(source: string, language: Language): string {
		if (!this._formatter) throw new Error('malva not initialized');
		if (!this.supports_format_language(language)) {
			throw new Error(`malva does not support ${language}`);
		}
		return this._formatter.formatText({ filePath: 'file.css', fileText: source });
	}

	// deno-lint-ignore require-await
	async format_async(source: string, language: Language): Promise<string> {
		return this.format(source, language);
	}

	dispose(): void {
		this._formatter = null;
	}
}
