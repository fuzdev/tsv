/**
 * Biome implementation wrapper (via WASM)
 *
 * Supports: TypeScript, JS, CSS, Svelte
 */

import { BaseImplementation, type Language, LANGUAGE_EXTENSIONS, LANGUAGES } from './types.ts';
import type { BiomeVersions } from './versions.ts';
// Type-only — `import type` is erased, so referencing `Biome` here does NOT load
// the WASM package at this module's import. The value import is deferred to
// `init()` (see there) so a load-time crash can't escape the registry's skip.
import type { Biome } from '@biomejs/js-api/bundler';
import { assert_format_config_landed, FORMAT_CONFIG_PROBES } from './format_config_probe.ts';

/**
 * Biome implementation using WASM.
 *
 * Supports:
 * - Format: Svelte, TypeScript, JS, CSS
 * - Parse: unsupported — the `@biomejs/js-api` package exposes no parse entry
 *   point (only `formatContent`/`lintContent`/`fixFile`); Biome parses
 *   internally but never surfaces the AST across the JS boundary.
 */
export class BiomeImplementation extends BaseImplementation {
	readonly versions: BiomeVersions;
	private _biome: Biome | null = null;
	private _project_key: number | null = null;

	/** The js-api exposes no parser. */
	readonly parse_languages: ReadonlyArray<Language> = [];
	readonly format_languages = LANGUAGES;

	constructor(versions: BiomeVersions) {
		super();
		this.versions = versions;
	}

	async init(): Promise<void> {
		// Load the WASM package + js-api lazily (not as static top-level imports)
		// so a load-time failure — e.g. Bun's wasm-bindgen-`start` incompatibility
		// with `@biomejs/wasm-bundler` — throws HERE, inside init_implementations'
		// per-impl try/catch (and is skipped), instead of throwing during this
		// module's static import graph and aborting the whole registry. The
		// wasm-bundler import runs first so it's registered before js-api loads.
		await import('@biomejs/wasm-bundler');
		const { Biome } = await import('@biomejs/js-api/bundler');
		this._biome = new Biome();
		const { projectKey } = this._biome.openProject('/tmp');
		this._project_key = projectKey;

		// Match the prettier/tsv config — tabs, line width 100, single quotes, no
		// trailing commas — so every format row does the same layout work (at
		// biome's defaults, width 80 + double quotes, the rows wrap different
		// amounts of code and the ratios conflate config with engine speed). The
		// per-language `formatter.*` sections DO inherit the top-level ones and
		// override them where they set a key (measured in both directions); each
		// repeats the shared values anyway, so a rename that reached only the
		// top-level block can't silently un-pin every language at once. Biome has no
		// dedicated Svelte formatter — `html.experimentalFullSupportEnabled` is what lets
		// it format `.svelte` at all, via its experimental HTML-superset pipeline; that
		// path formats the embedded `<script>`/`<style>` too (verified), so the svelte row
		// is comparable work to prettier-plugin-svelte / tsv. Without the flag biome skips it.
		this._biome.applyConfiguration(projectKey, {
			formatter: {
				indentStyle: 'tab',
				lineWidth: 100
			},
			javascript: {
				formatter: {
					indentStyle: 'tab',
					lineWidth: 100,
					quoteStyle: 'single',
					trailingCommas: 'none'
				}
			},
			css: {
				formatter: {
					indentStyle: 'tab',
					lineWidth: 100,
					quoteStyle: 'single'
				}
			},
			html: {
				experimentalFullSupportEnabled: true,
				formatter: {
					indentStyle: 'tab',
					lineWidth: 100
				}
			}
		});

		// Assert the configuration actually LANDED. `applyConfiguration` accepts an
		// unrecognized key SILENTLY — no throw, no diagnostic (verified) — so a
		// renamed key in a future biome major would leave this row formatting at
		// biome's own defaults (measured: width 80, double quotes, trailing commas)
		// and wrapping a different amount of code than every other format row, with
		// nothing in the report to say so. dprint and malva have a diagnostic channel
		// for this (`getConfigDiagnostics`); biome has none, so the check is
		// behavioral. Routed through `format` rather than `formatContent` so the probe
		// exercises the exact call the timed row makes.
		//
		// ONE probe PER LANGUAGE, because the config above is one section per language
		// and each feeds a different row: a TypeScript-only probe proves the
		// `javascript` section and leaves `css` and `html` — the CSS and svelte rows —
		// free to un-pin silently, which is the failure this check exists to catch. The
		// svelte pass doubles as the only guard on `experimentalFullSupportEnabled`:
		// without it biome returns an EMPTY string for `.svelte`, which the timed row
		// would otherwise score as a successful format.
		//
		// ⚠️ Per-language coverage, impl-wide COST — the same shape `lib/oxc.ts` carries
		// for its two tools: the registry's unit of absence is the impl, so a probe
		// failing on ONE language takes biome's other rows down with it. Sharper here
		// than there, because the likeliest failure is the language whose support is
		// itself experimental: a biome release that changes `.svelte` handling removes
		// the TypeScript and CSS rows too. Deliberate — the alternative is a row
		// publishing a number produced at biome's own defaults — and disclosed rather
		// than silent: `unavailable[].rows` names every row the failure removed.
		for (const language of this.format_languages) {
			assert_format_config_landed(
				'biome',
				language,
				this.format(FORMAT_CONFIG_PROBES[language], language)
			);
		}
	}

	parse(_source: string, _language: Language): unknown {
		throw new Error('Biome has no parser: the @biomejs/js-api package exposes no parse API');
	}

	format(source: string, language: Language): string {
		// `=== null` on the key, not a truthiness test: `openProject` hands back a
		// numeric handle, and a legitimate `0` would read as uninitialized.
		if (!this._biome || this._project_key === null) {
			throw new Error('Biome not initialized');
		}
		if (!this.supports_format_language(language)) {
			throw new Error(`Biome does not support ${language}`);
		}

		try {
			const result = this._biome.formatContent(this._project_key, source, {
				filePath: `file${LANGUAGE_EXTENSIONS[language]}`
			});
			return result.content;
		} catch (e: unknown) {
			// Biome WASM panics have minimal info in the error - the full panic message
			// is printed to stderr by the WASM module (not capturable here).
			// Provide a cleaner error message for the benchmark output.
			if (e && typeof e === 'object' && 'stackTrace' in e) {
				const stack_trace = String((e as { stackTrace: unknown }).stackTrace);
				if (stack_trace.includes('unreachable')) {
					throw new Error('Biome internal error (WASM panic)');
				}
			}
			// For errors with actual messages, pass them through
			if (e instanceof Error && e.message) {
				throw e;
			}
			throw new Error('Biome format failed');
		}
	}

	// deno-lint-ignore require-await
	async format_async(source: string, language: Language): Promise<string> {
		return this.format(source, language);
	}

	dispose(): void {
		this._biome = null;
		this._project_key = null;
	}
}
