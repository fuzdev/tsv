/**
 * The shared "did the pinned layout config actually LAND" probe, for the two
 * format tools whose config surface reports nothing when it doesn't.
 *
 * Every formatter here is pinned to tsv's layout targets — width 100, tabs, single
 * quotes, no trailing commas — so each row wraps and rewrites the same amount of
 * code and a ratio measures the engine rather than the config (see
 * docs/benchmarks.md §Fairness caveats). Two of the four timed format opponents
 * have a channel that says so when a key stops being recognized, and two do not:
 * `lib/dprint.ts` and `lib/malva.ts` fail init on a non-empty
 * `getConfigDiagnostics()`, while biome's `applyConfiguration` and oxfmt's
 * per-call options bag each accept an unknown key SILENTLY and fall back to their
 * own defaults — measured: biome drops to width 80 + double quotes + trailing
 * commas, oxfmt to spaces + double quotes + trailing commas. A renamed key there
 * would leave the row wrapping a different amount of code with nothing anywhere in
 * the report to say so, which is exactly the config-vs-engine conflation the
 * pinning exists to prevent.
 *
 * So the check is BEHAVIORAL — format one source whose output differs under each
 * pinned option and read the answer back — in the spirit of `lib/swc.ts`'s
 * decorator/goal probes and `lib/yuku.ts`'s option probes. ONE probe set and one
 * grader for both tools rather than a copy per wrapper, on the rule the yuku wrapper
 * follows: a second spelling of the same question is free to drift into asking a
 * different one.
 *
 * ⚠️ **Per LANGUAGE, not per tool.** biome's config is a stack of per-language
 * sections (`javascript.formatter`, `css.formatter`, `html.formatter`), each
 * feeding a different row, so a probe that formats only TypeScript proves only the
 * `javascript` section and leaves the CSS and svelte rows free to un-pin silently —
 * the very failure this module exists to catch, on two of biome's three rows. Every
 * caller therefore probes each language it FORMATS, and this module carries a probe
 * source and a grading arm for each. (oxfmt drives all three languages from one
 * options bag, so its extra two probes are cheap corroboration rather than new
 * coverage — but they are also the standing proof that the pins reach its
 * bundled-prettier Svelte fallback, which docs/benchmarks.md asserts.)
 *
 * What each arm can and cannot see, since an option that matches the tool's own
 * default is unfalsifiable by a behavioral probe:
 *
 * - **biome** defaults to TAB indent, so no indent assertion can catch a dropped
 *   key there; width and quotes can, and do — dropping biome's whole `css` section
 *   is caught by the quote assertion (the top-level block still supplies the width).
 *   Losing `html.experimentalFullSupportEnabled` makes biome return EMPTY output for
 *   `.svelte`, which the svelte arm reads as "no probe line at all" — otherwise an
 *   empty string is a *successful* format as far as the timed row can tell.
 * - **oxfmt**'s own width default is already 100, so no width assertion can catch a
 *   dropped key there; tabs, quotes and the trailing comma can, and do (verified by
 *   renaming each key in turn).
 *
 * Those blind spots are why both options stay pinned rather than left implicit: the
 * assertion that can't see a dropped key still sees an upstream DEFAULT change.
 *
 * @module
 */

import type { Language } from './types.ts';

/**
 * One probe source per language, each written so its formatted output differs
 * under every pinned option that language can express.
 *
 * TypeScript carries all four:
 *
 * - the object fits in 100 columns and not in 80, so `printWidth`/`lineWidth`
 *   shows up as whether it stays on one line;
 * - its values are double-quoted in the input, so `singleQuote`/`quoteStyle`
 *   shows up as the quote character;
 * - the function body is indented, so `useTabs`/`indentStyle` shows up as the
 *   line's first byte;
 * - the array fits on no line at any plausible width, so
 *   `trailingComma`/`trailingCommas` shows up on its last element.
 *
 * CSS carries the three it has (there is no trailing comma in a declaration): the
 * `font-family` list formats to 96 columns, so it stays on one line at 100 and
 * breaks at 80; its family names are double-quoted; and the declaration sits inside
 * a rule, so it is indented.
 *
 * Svelte carries the two the markup pipeline governs (attribute quotes are not the
 * JS quote style): the `<div>` formats to 95 columns, so it stays whole at 100 and
 * breaks at 80, and the `<section>` breaks at any width, so its children are always
 * indented.
 */
export const FORMAT_CONFIG_PROBES: Readonly<Record<Language, string>> = {
	typescript:
		'function probe() { return { alpha: "aaaa", beta: "bbbb", gamma: "cccc", delta: "dddd", epsilon: "eeee" } }\n' +
		'const probe_list = ["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee", "ffffffffff", "gggggggggg", "hhhhhhhhhh"]\n',
	css: '.probe { font-family: "Alpha Sans", "Beta Serif", "Gamma Mono", "Delta Text", "Epsilon UI", sans-serif; }\n',
	svelte:
		'<div class="probe"><span>alpha</span><span>beta</span><span>gamma</span><span>delta</span></div>\n' +
		'<section><p>aaaaaaaaaa</p><p>bbbbbbbbbb</p><p>cccccccccc</p><p>dddddddddd</p></section>\n'
};

/**
 * Grade `tool`'s formatting of `FORMAT_CONFIG_PROBES[language]`, THROWING when a
 * pinned option is not visible in the output.
 *
 * Called from the wrapper's `init()`, so a failure lands inside
 * `init_implementations`' per-impl try/catch: the row goes ABSENT with this
 * message in the report's `unavailable`, rather than staying present and
 * publishing a number produced at some other tool's defaults. An absent row is a
 * disclosed shortfall; a misconfigured one is a wrong published ratio.
 *
 * Every assertion reads a LINE rather than an exact rendering, so a cosmetic
 * upstream change (brace spacing, statement order) can't fire it — only the layout
 * decisions the pins are about. Each arm also carries a vacuity guard where the
 * question it asks only exists on a BROKEN construct: an assertion that grades
 * nothing passes every run, so "the probe stopped discriminating" must fail loudly
 * rather than read as a pass (the same posture as `check_variant_parity`'s digest
 * guard in `bench.ts`).
 *
 * @param tool - the tool's row-facing name, so the throw names who failed
 * @param language - which probe `output` came from, and which arm grades it
 * @param output - what the tool made of `FORMAT_CONFIG_PROBES[language]`
 */
export function assert_format_config_landed(
	tool: string,
	language: Language,
	output: string
): void {
	const lines = output.split('\n');
	const fail = (option: string, detail: string): Error =>
		new Error(
			`${tool} (${language}): the pinned layout option '${option}' did not land — ${detail}. Every ` +
				`format row is pinned to the same targets so a ratio measures the engine and not the ` +
				`config, and ${tool} ignores an unrecognized option key silently, so this is an upstream ` +
				`rename or a changed default. See lib/format_config_probe.ts.`
		);

	switch (language) {
		case 'typescript': {
			const object_line = lines.find((line) => line.includes('alpha:'));
			if (object_line === undefined) {
				throw fail(
					'printWidth',
					'the probe produced no `alpha:` line at all — it no longer applies'
				);
			}
			if (!object_line.includes('epsilon:')) {
				throw fail(
					'printWidth/lineWidth',
					'the probe object broke across lines, so the width fell back below 100'
				);
			}

			if (!output.includes("'aaaa'")) {
				throw fail(
					'singleQuote/quoteStyle',
					'the probe strings kept the double quotes they were written with'
				);
			}

			const body_line = lines.find((line) => line.includes('return '));
			if (body_line === undefined) {
				throw fail('useTabs', 'the probe produced no `return` line at all — it no longer applies');
			}
			if (!body_line.startsWith('\t')) {
				throw fail('useTabs/indentStyle', 'the probe function body is not tab-indented');
			}

			const last_element_line = lines.find((line) => line.includes('hhhhhhhhhh'));
			if (last_element_line === undefined) {
				throw fail(
					'trailingComma',
					'the probe produced no array element line — it no longer applies'
				);
			}
			// Vacuity guard: the trailing-comma question only exists on a BROKEN array,
			// so an array that stayed inline means this assertion grades nothing.
			if (last_element_line.includes('aaaaaaaaaa')) {
				throw fail(
					'trailingComma',
					'the probe array stayed on one line, so nothing here can observe a trailing comma — the probe no longer discriminates'
				);
			}
			if (last_element_line.trimEnd().endsWith(',')) {
				throw fail(
					'trailingComma/trailingCommas',
					"the probe array's last element kept its trailing comma"
				);
			}
			return;
		}

		case 'css': {
			const declaration_line = lines.find((line) => line.includes('font-family:'));
			if (declaration_line === undefined) {
				throw fail(
					'lineWidth',
					'the probe produced no `font-family` line at all — the formatter declined the language, or the probe no longer applies'
				);
			}
			// Vacuity guard: the indent question only exists on a BROKEN rule, so a rule
			// that stayed inline means the indent assertion below grades nothing.
			if (declaration_line.includes('.probe')) {
				throw fail(
					'indentStyle',
					'the probe rule stayed on one line, so nothing here can observe the indent — the probe no longer discriminates'
				);
			}
			if (!declaration_line.includes('sans-serif')) {
				throw fail(
					'printWidth/lineWidth',
					'the probe declaration broke across lines, so the width fell back below 100'
				);
			}
			if (!output.includes("'Alpha Sans'")) {
				throw fail(
					'singleQuote/quoteStyle',
					'the probe family names kept the double quotes they were written with'
				);
			}
			if (!declaration_line.startsWith('\t')) {
				throw fail('useTabs/indentStyle', 'the probe declaration is not tab-indented');
			}
			return;
		}

		case 'svelte': {
			const element_line = lines.find((line) => line.includes('<div class="probe"'));
			if (element_line === undefined) {
				throw fail(
					'lineWidth',
					'the probe produced no `<div class="probe">` line at all — the formatter declined the language (biome returns EMPTY output for `.svelte` without `html.experimentalFullSupportEnabled`), or the probe no longer applies'
				);
			}
			if (!element_line.includes('</div>')) {
				throw fail(
					'printWidth/lineWidth',
					'the probe element broke across lines, so the width fell back below 100'
				);
			}

			const child_line = lines.find((line) => line.includes('<p>aaaaaaaaaa</p>'));
			if (child_line === undefined) {
				throw fail(
					'indentStyle',
					'the probe produced no `<p>` child line at all — it no longer applies'
				);
			}
			// Vacuity guard: the indent question only exists on a BROKEN element, so a
			// `<section>` that stayed inline means this assertion grades nothing.
			if (child_line.includes('<section')) {
				throw fail(
					'indentStyle',
					'the probe section stayed on one line, so nothing here can observe the indent — the probe no longer discriminates'
				);
			}
			if (!child_line.startsWith('\t')) {
				throw fail('useTabs/indentStyle', "the probe section's children are not tab-indented");
			}
			return;
		}
	}

	// Exhaustiveness, enforced at COMPILE time: `FORMAT_CONFIG_PROBES` is keyed by
	// `Language`, so a new language cannot be added without a probe source — but
	// nothing would force a grading ARM to go with it, and a `void` switch that
	// falls through would then pass every caller silently. That is the failure this
	// module exists to prevent, one level up. `deno task typecheck:js` fails here
	// until the arm exists.
	const unhandled: never = language;
	throw new Error(`format config probe: no grading arm for language '${String(unhandled)}'`);
}
