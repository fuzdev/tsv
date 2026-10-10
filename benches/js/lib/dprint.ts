/**
 * dprint implementation wrapper (via WASM)
 *
 * `dprint-plugin-typescript` is the engine **`deno fmt` runs** for TS/JS, loaded
 * here in-process as its Wasm plugin rather than by shelling out to the `deno`
 * CLI. That choice is deliberate on two axes:
 *
 * - **Runtime neutrality.** A `deno fmt` subprocess row would exist only under
 *   Deno, against a harness whose whole design is one body across three runtimes
 *   (see benches/js/CLAUDE.md §Cross-Runtime). The Wasm plugin loads under Deno,
 *   Node, AND Bun — verified — so this is a full three-runtime row.
 * - **It would measure the wrong thing.** The harness times one file at a time in
 *   a warm in-process loop; a fresh `deno` process per file would be dominated by
 *   spawn + IPC, not format work, and would be cold on every call against warm
 *   opponents.
 *
 * So the row is named for what it actually measures — the dprint engine — not
 * `deno fmt`, whose CLI wrapping (config discovery, file IO, its own CSS/HTML/
 * markdown plugins) is not in scope here.
 *
 * Supports: TypeScript/JS only. `@dprint/typescript` matches
 * `ts,tsx,js,jsx,mjs,cjs,mts,cts` and rejects CSS/Svelte outright (verified), so
 * unlike `oxfmt`/`biome` this contributes no css or svelte row. dprint's CSS plugin
 * is a separate Wasm plugin with a row of its own over this same formatter host —
 * see `lib/malva.ts` — and its markup plugin backs the Svelte row, composed with this
 * plugin and malva — see `lib/markup.ts`.
 */

import { readFile } from 'node:fs/promises';
import { BaseImplementation, type Language, LANGUAGE_EXTENSIONS } from './types.ts';
import { assert_format_config_landed, FORMAT_CONFIG_PROBES } from './format_config_probe.ts';
import { assert_tool_rejects_invalid } from './reject_probe.ts';
import type { DprintVersions } from './versions.ts';
// Type-only — `import type` is erased, so naming `Formatter` here does NOT load
// the Wasm plugin at this module's import. The value imports are deferred to
// `init()` (see there) so a load-time crash can't escape the registry's skip.
import type { Formatter } from '@dprint/formatter';

/**
 * dprint's GLOBAL config, shared by every dprint-hosted row (`dprint-wasm`,
 * `malva-wasm`, `markup-fmt-wasm`): the prettier/tsv layout targets — width 100,
 * tabs — so every format row does the same layout work. At dprint's defaults the
 * rows would wrap different amounts of code and the ratios would conflate config
 * with engine speed. See docs/benchmarks.md §Fairness caveats.
 */
export const DPRINT_GLOBAL_CONFIG = { lineWidth: 100, indentWidth: 2, useTabs: true } as const;

/**
 * `@dprint/typescript`'s plugin config — shared with `lib/markup.ts`, whose Svelte
 * row formats each embedded `<script>` and template expression through this same
 * plugin. `trailingCommas` fans out to the 12 per-construct keys
 * (`arguments.trailingCommas`, `arrayExpression.…`, …). `preferSingle` (not
 * `alwaysSingle`) is the faithful analogue of prettier's `singleQuote: true`,
 * which still switches quotes to avoid escaping.
 */
export const DPRINT_TYPESCRIPT_CONFIG = {
	quoteStyle: 'preferSingle',
	trailingCommas: 'never'
} as const;

/**
 * dprint implementation using the `dprint-plugin-typescript` Wasm plugin.
 *
 * Supports:
 * - Format: TypeScript, JS
 * - Parse: unsupported — dprint is a formatter; its Wasm plugin protocol exposes
 *   only `format_text` and config entry points, never an AST across the boundary.
 */
export class DprintImplementation extends BaseImplementation {
	readonly versions: DprintVersions;
	private _formatter: Formatter | null = null;

	/** The Wasm plugin exposes no parser. */
	readonly parse_languages: ReadonlyArray<Language> = [];
	readonly format_languages: ReadonlyArray<Language> = ['typescript'];

	constructor(versions: DprintVersions) {
		super();
		this.versions = versions;
	}

	async init(): Promise<void> {
		// Load the plugin + formatter host lazily (not as static top-level imports)
		// so a load-time failure throws HERE, inside init_implementations' per-impl
		// try/catch (and is skipped), instead of during this module's static import
		// graph and aborting the whole registry. Same posture as lib/biome.ts.
		const { getPath } = await import('@dprint/typescript');
		const { createFromBuffer } = await import('@dprint/formatter');
		this._formatter = createFromBuffer(await readFile(getPath()));

		// the pinned layout targets — see `DPRINT_GLOBAL_CONFIG` / `DPRINT_TYPESCRIPT_CONFIG`
		this._formatter.setConfig(DPRINT_GLOBAL_CONFIG, DPRINT_TYPESCRIPT_CONFIG);

		// Assert the config actually LANDED. dprint reports an unrecognized key as
		// a diagnostic rather than throwing (verified: a bogus key yields
		// `Unknown property in configuration`), so without this check a renamed key
		// in a future plugin version would silently leave that option at its default
		// and skew the row against every other formatter — exactly the config-vs-engine
		// conflation the fairness discipline exists to prevent.
		const diagnostics = this._formatter.getConfigDiagnostics();
		if (diagnostics.length > 0) {
			const detail = diagnostics.map((d) => `${d.propertyName}: ${d.message}`).join('; ');
			throw new Error(`dprint rejected the benchmark config (${detail})`);
		}
		// A clean diagnostic list proves every key was RECOGNIZED, not that its value
		// had the pinned effect (`trailingCommas: 'never'` fans out to twelve keys, any
		// of which a plugin bump could re-default). The behavioral proof is the shared
		// probe every other formatter row already runs.
		for (const language of this.format_languages) {
			assert_format_config_landed(
				'dprint',
				language,
				this.format(FORMAT_CONFIG_PROBES[language], language)
			);
			// And that a syntax error still THROWS: `format` has no other success test,
			// so a plugin that began handing its input back would publish free files
			// and a full coverage — see `lib/reject_probe.ts`.
			assert_tool_rejects_invalid('dprint', 'format', language, (source) =>
				this.format(source, language)
			);
		}
	}

	parse(_source: string, _language: Language): unknown {
		throw new Error('dprint has no parser: the Wasm plugin exposes no parse API');
	}

	format(source: string, language: Language): string {
		if (!this._formatter) {
			throw new Error('dprint not initialized');
		}
		if (!this.supports_format_language(language)) {
			throw new Error(`dprint does not support ${language}`);
		}

		// The corpus folds `.js` into the `typescript` Language (tsv formats both
		// through its TS path), so every file goes in as `file.ts` — the same
		// synthetic-filepath treatment lib/biome.ts gives the language.
		return this._formatter.formatText({
			filePath: `file${LANGUAGE_EXTENSIONS[language]}`,
			fileText: source
		});
	}

	// deno-lint-ignore require-await
	async format_async(source: string, language: Language): Promise<string> {
		return this.format(source, language);
	}

	dispose(): void {
		this._formatter = null;
	}
}
