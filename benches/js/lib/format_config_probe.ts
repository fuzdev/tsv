/**
 * The shared "did the pinned layout config actually LAND" probe, for the two
 * format rows whose tools report nothing when it doesn't.
 *
 * Every formatter here is pinned to tsv's layout targets — width 100, tabs, single
 * quotes, no trailing commas — so each row wraps and rewrites the same amount of
 * code and a ratio measures the engine rather than the config (see
 * docs/benchmarks.md §Fairness caveats). Two of the four format opponents
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
 * decorator/goal probes and `lib/yuku.ts`'s option probes. ONE probe and one grader
 * for both tools rather than a copy per wrapper, on the rule the yuku wrapper
 * follows: a second spelling of the same question is free to drift into asking a
 * different one.
 *
 * Two of the four options match the tool's own default on one side each
 * (`indentStyle: tab` is biome's, `printWidth: 100` is oxfmt's), so those
 * assertions cannot catch a DROPPED key there. They catch an upstream DEFAULT
 * change, which is the reason both are pinned rather than left implicit.
 *
 * @module
 */

/**
 * One source whose formatted output differs under every pinned option:
 *
 * - the object fits in 100 columns and not in 80, so `printWidth`/`lineWidth`
 *   shows up as whether it stays on one line;
 * - its values are double-quoted in the input, so `singleQuote`/`quoteStyle`
 *   shows up as the quote character;
 * - the function body is indented, so `useTabs`/`indentStyle` shows up as the
 *   line's first byte;
 * - the array fits on no line at any plausible width, so
 *   `trailingComma`/`trailingCommas` shows up on its last element.
 */
export const FORMAT_CONFIG_PROBE =
	'function probe() { return { alpha: "aaaa", beta: "bbbb", gamma: "cccc", delta: "dddd", epsilon: "eeee" } }\n' +
	'const probe_list = ["aaaaaaaaaa", "bbbbbbbbbb", "cccccccccc", "dddddddddd", "eeeeeeeeee", "ffffffffff", "gggggggggg", "hhhhhhhhhh"]\n';

/**
 * Grade `tool`'s formatting of `FORMAT_CONFIG_PROBE`, THROWING when a pinned
 * option is not visible in the output.
 *
 * Called from the wrapper's `init()`, so a failure lands inside
 * `init_implementations`' per-impl try/catch: the row goes ABSENT with this
 * message in the report's `unavailable`, rather than staying present and
 * publishing a number produced at some other tool's defaults. An absent row is a
 * disclosed shortfall; a misconfigured one is a wrong published ratio.
 *
 * Every assertion reads a LINE rather than an exact rendering, so a cosmetic
 * upstream change (brace spacing, statement order) can't fire it — only the four
 * layout decisions the pins are about.
 *
 * @param tool - the tool's row-facing name, so the throw names who failed
 * @param output - what the tool made of `FORMAT_CONFIG_PROBE`
 */
export function assert_format_config_landed(tool: string, output: string): void {
	const lines = output.split('\n');
	const fail = (option: string, detail: string): Error =>
		new Error(
			`${tool}: the pinned layout option '${option}' did not land — ${detail}. Every format row ` +
				`is pinned to the same targets so a ratio measures the engine and not the config, and ` +
				`${tool} ignores an unrecognized option key silently, so this is an upstream rename or a ` +
				`changed default. See lib/format_config_probe.ts.`
		);

	const object_line = lines.find((line) => line.includes('alpha:'));
	if (object_line === undefined) {
		throw fail('printWidth', 'the probe produced no `alpha:` line at all — it no longer applies');
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
		throw fail('trailingComma', 'the probe produced no array element line — it no longer applies');
	}
	// Vacuity guard: the trailing-comma question only exists on a BROKEN array, so
	// an array that stayed inline means this assertion grades nothing and would
	// pass every run. Same posture as `check_variant_parity`'s digest guard.
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
}
