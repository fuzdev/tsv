/**
 * markup_fmt implementation wrapper (via WASM)
 *
 * `dprint-plugin-markup` is markup_fmt — by malva's author — as a dprint Wasm
 * plugin: dprint's Svelte formatter. It formats the markup itself (elements,
 * attributes, `{#…}` blocks) and hands every embedded `<script>`, `<style>` and
 * template expression back to the host, so on its own it would leave all three as
 * written — a fraction of the work every other `format/svelte` row does.
 *
 * **So the row is a composition, the one the dprint CLI runs.** One
 * `createContext` holds three plugins — markup_fmt, `@dprint/typescript` and
 * malva — and the context routes each embedded region to the plugin that matches
 * it. The two embedded plugins carry the exact configs of the `dprint-wasm` and
 * `malva-wasm` rows (`DPRINT_TYPESCRIPT_CONFIG`, `MALVA_CONFIG`), so this row's
 * script and style formatting IS those rows' engines; what it adds is the markup
 * layer and the host round-trips between plugins.
 *
 * **Svelte only.** markup_fmt also formats HTML, Vue, Astro and template
 * dialects, none of which tsv handles, and the context's TypeScript and CSS are
 * the `dprint-wasm` and `malva-wasm` rows already.
 *
 * **An accept is a whole-component format.** A syntax error in markup, in a
 * block, or in any embedded region throws (verified for each: a broken `<script>`,
 * `<style>`, text expression, attribute expression, `{#if}` and `{#each}` head,
 * and an unclosed tag), so the row cannot count a file whose script it silently
 * left unformatted.
 */

import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { BaseImplementation, type Language } from './types.ts';
import { assert_format_config_landed, FORMAT_CONFIG_PROBES } from './format_config_probe.ts';
import { assert_tool_rejects_invalid } from './reject_probe.ts';
import type { MarkupVersions } from './versions.ts';
import { DPRINT_GLOBAL_CONFIG, DPRINT_TYPESCRIPT_CONFIG } from './dprint.ts';
import { MALVA_CONFIG } from './malva.ts';
// Type-only, so naming `FormatterContext` here does not load a plugin at import
// time; the value imports are deferred to `init()`. Same posture as lib/dprint.ts.
import type { FormatterContext } from '@dprint/formatter';

/**
 * markup_fmt's plugin config. `scriptIndent` / `styleIndent` indent the bodies of
 * `<script>` and `<style>` one level, as prettier-plugin-svelte does by default
 * (`svelteIndentScriptAndStyle: true`) — markup_fmt's default leaves them flush,
 * and the indent changes the width every embedded line has to fit in. Attribute
 * quotes stay at markup_fmt's default (double), which is prettier's too:
 * `singleQuote` governs JS strings, never HTML attributes.
 */
const MARKUP_CONFIG = { scriptIndent: true, styleIndent: true } as const;

/**
 * Proves the context routes each embedded region to its configured plugin. With a
 * plugin missing, markup_fmt hands the region back as written, so a broken region
 * would format "successfully" and the row would count a file it never formatted
 * (measured: with no CSS plugin, an unclosed `<style>` rule is accepted). One
 * region per hop, each graded by a line only its plugin produces:
 * - `<script>` → TypeScript under `scriptIndent`: the double-quoted string comes
 *   back single-quoted on a line indented by a tab;
 * - template expression → TypeScript (a separate hop, as `.tsx`): spaces appear
 *   around the operator;
 * - `<style>` → malva under `styleIndent`: the rule expands and its declaration
 *   comes back terminated on a line indented by two tabs.
 *
 * `FORMAT_CONFIG_PROBES.svelte` grades only the markup layer, and the
 * `typescript` / `css` probes reach those plugins standalone — neither sees these
 * hops.
 */
const EMBED_PROBE_SOURCE =
	'<script>\nconst probe = "embedded"\n</script>\n\n<p>{a+b}</p>\n\n<style>\n.probe{color:red}\n</style>\n';

/** The line each `EMBED_PROBE_SOURCE` hop must produce, keyed by the hop. */
const EMBED_PROBE_LINES: ReadonlyArray<readonly [hop: string, line: string]> = [
	['<script> → TypeScript (scriptIndent)', "\tconst probe = 'embedded';"],
	['template expression → TypeScript', '<p>{a + b}</p>'],
	['<style> → malva (styleIndent)', '\t\tcolor: red;']
];

/**
 * markup_fmt Svelte formatter, composed with dprint's TypeScript and CSS plugins.
 *
 * Supports:
 * - Format: Svelte only
 * - Parse: unsupported — dprint's Wasm plugin protocol exposes `format_text` and
 *   config entry points, never an AST across the boundary.
 */
export class MarkupImplementation extends BaseImplementation {
	readonly versions: MarkupVersions;
	private _context: FormatterContext | null = null;

	/** The Wasm plugin exposes no parser. */
	readonly parse_languages: ReadonlyArray<Language> = [];
	readonly format_languages: ReadonlyArray<Language> = ['svelte'];

	constructor(versions: MarkupVersions) {
		super();
		this.versions = versions;
	}

	async init(): Promise<void> {
		const { getPath } = await import('@dprint/typescript');
		const { createContext } = await import('@dprint/formatter');
		// markup and malva ship only `*.wasm` (no JS entry, so no `getPath()` helper
		// like `@dprint/typescript` has) — resolve the wasm files directly.
		const require = createRequire(import.meta.url);
		const context = createContext(DPRINT_GLOBAL_CONFIG);
		context.addPlugin(await readFile(getPath()), DPRINT_TYPESCRIPT_CONFIG);
		context.addPlugin(
			await readFile(require.resolve('dprint-plugin-malva/plugin.wasm')),
			MALVA_CONFIG
		);
		context.addPlugin(
			await readFile(require.resolve('dprint-plugin-markup/plugin.wasm')),
			MARKUP_CONFIG
		);
		this._context = context;

		// Assert the config LANDED, for the same reason lib/dprint.ts does: dprint
		// reports an unrecognized key as a diagnostic rather than throwing, so a
		// renamed key would silently leave an option at its default. The context
		// reports every plugin's diagnostics at once.
		const diagnostics = context.getConfigDiagnostics();
		if (diagnostics.length > 0) {
			const detail = diagnostics.map((d) => `${d.propertyName}: ${d.message}`).join('; ');
			throw new Error(`markup_fmt rejected the benchmark config (${detail})`);
		}
		// Recognized is not landed — the behavioral proof every formatter row runs, on
		// the markup layer, then the embed hop (see `EMBED_PROBE_SOURCE`).
		for (const language of this.format_languages) {
			assert_format_config_landed(
				'markup-fmt',
				language,
				this.format(FORMAT_CONFIG_PROBES[language], language)
			);
			// And that a syntax error still THROWS: `format` has no other success test,
			// so a plugin that began handing its input back would publish free files
			// and a full coverage — see `lib/reject_probe.ts`.
			assert_tool_rejects_invalid('markup-fmt', 'format', language, (source) =>
				this.format(source, language)
			);
		}
		const embedded = this.format(EMBED_PROBE_SOURCE, 'svelte');
		const embedded_lines = embedded.split('\n');
		const missed = EMBED_PROBE_LINES.filter(([, line]) => !embedded_lines.includes(line));
		if (missed.length > 0) {
			throw new Error(
				`markup-fmt (svelte): an embedded region came back unformatted, so the context no ` +
					`longer routes it to its configured plugin or an indent option did not land ` +
					`(${missed.map(([hop, line]) => `${hop}: no line ${JSON.stringify(line)}`).join('; ')}) ` +
					`— got ${JSON.stringify(embedded)}. See lib/markup.ts.`
			);
		}
	}

	parse(_source: string, _language: Language): unknown {
		throw new Error('markup_fmt has no parser: the Wasm plugin exposes no parse API');
	}

	format(source: string, language: Language): string {
		if (!this._context) throw new Error('markup_fmt not initialized');
		if (!this.supports_format_language(language)) {
			throw new Error(`markup_fmt does not support ${language}`);
		}
		return this._context.formatText({ filePath: 'file.svelte', fileText: source });
	}

	// deno-lint-ignore require-await
	async format_async(source: string, language: Language): Promise<string> {
		return this.format(source, language);
	}

	dispose(): void {
		this._context = null;
	}
}
