/**
 * Tests for the pinned-layout-config grader (`assert_format_config_landed`) and the
 * embedded-region grader (`assert_embedded_regions_landed`).
 *
 * Pins the two directions that matter, since a wrong verdict either publishes a
 * number produced at some other tool's defaults or removes a healthy row:
 *  - pinned-shaped output PASSES for every language, and
 *  - output produced at a tool's own defaults FAILS, one arm per pinned option —
 *    the check's whole purpose, and the half that goes vacuous silently.
 *
 * Hand-written rather than tool-produced on purpose: the graders' job is to read a
 * layout decision out of text, and a unit test that formats through the real tools
 * would test the tools instead, need their node_modules, and stop gating in
 * `npm run check` (see `test:node`). The claims about what each TOOL actually
 * emits at its defaults live in the module's own docs, verified by running them.
 *
 * The vacuity arms are here too — an assertion that grades nothing passes every
 * run, so "the probe stopped discriminating" must be a failure, and that is
 * exactly the shape no live run can show you.
 */

import { throws, doesNotThrow } from 'node:assert';
import { test } from 'vitest';
import {
	assert_embedded_regions_landed,
	assert_format_config_landed
} from './format_config_probe.ts';
import type { Language } from './types.ts';

/** What each language's probe looks like once the pins LANDED — the passing case. */
const pinned: Record<Language, string> = {
	typescript: [
		'function probe() {',
		"\treturn { alpha: 'aaaa', beta: 'bbbb', gamma: 'cccc', delta: 'dddd', epsilon: 'eeee' };",
		'}',
		'const probe_list = [',
		"\t'aaaaaaaaaa',",
		"\t'bbbbbbbbbb',",
		"\t'hhhhhhhhhh'",
		'];',
		'const probe_wide = {',
		"\talpha: 'aaaaaaaaa',",
		"\teps: 'eeee'",
		'};',
		''
	].join('\n'),
	css: [
		'.probe {',
		"\tfont-family: 'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI', sans-serif;",
		'}',
		'.wide {',
		'\tfont-family:',
		"\t\t'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI', 'Zeta Sans', serif;",
		'}',
		''
	].join('\n'),
	svelte: [
		'<div class="probe"><span>alpha</span><span>beta</span><span>gamma</span><span>delta</span></div>',
		'<section>',
		'\t<p>aaaaaaaaaa</p>',
		'\t<p>dddddddddd</p>',
		'</section>',
		'<div class="wide">',
		'\t<span>alpha</span><span>beta</span><span>gamma</span><span>delta</span><b>eps</b>',
		'</div>',
		''
	].join('\n')
};

for (const language of Object.keys(pinned) as Array<Language>) {
	test(`config landed: ${language} pinned output passes`, () => {
		doesNotThrow(() => assert_format_config_landed('probe-tool', language, pinned[language]));
	});
}

/**
 * One dropped pin per case, spelled as the output the tool produces instead.
 *
 * `option` is the substring the throw must name, so a case can't pass by firing the
 * WRONG arm — which is how a grader silently stops covering the option it claims.
 */
const dropped: Array<{ label: string; language: Language; output: string; option: string }> = [
	{
		label: 'ts width fell back below 100',
		language: 'typescript',
		output: pinned.typescript.replace(
			"\treturn { alpha: 'aaaa', beta: 'bbbb', gamma: 'cccc', delta: 'dddd', epsilon: 'eeee' };",
			"\treturn {\n\t\talpha: 'aaaa',\n\t\tepsilon: 'eeee'\n\t};"
		),
		option: 'printWidth'
	},
	{
		label: 'ts kept double quotes',
		language: 'typescript',
		output: pinned.typescript.replaceAll("'", '"'),
		option: 'singleQuote'
	},
	{
		label: 'ts indented with spaces',
		language: 'typescript',
		output: pinned.typescript.replaceAll('\t', '  '),
		option: 'useTabs'
	},
	{
		label: 'ts kept the trailing comma',
		language: 'typescript',
		output: pinned.typescript.replace("\t'hhhhhhhhhh'", "\t'hhhhhhhhhh',"),
		option: 'trailingComma'
	},
	{
		label: 'ts array stayed inline (vacuous trailing-comma arm)',
		language: 'typescript',
		output: pinned.typescript.replace(
			"const probe_list = [\n\t'aaaaaaaaaa',\n\t'bbbbbbbbbb',\n\t'hhhhhhhhhh'\n];",
			"const probe_list = ['aaaaaaaaaa', 'bbbbbbbbbb', 'hhhhhhhhhh'];"
		),
		option: 'trailingComma'
	},
	{
		label: 'css width fell back below 100',
		language: 'css',
		output: pinned.css.replace(
			"\tfont-family: 'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI', sans-serif;",
			"\tfont-family:\n\t\t'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI',\n\t\tsans-serif;"
		),
		option: 'printWidth'
	},
	{
		label: 'css kept double quotes',
		language: 'css',
		output: pinned.css.replaceAll("'", '"'),
		option: 'singleQuote'
	},
	{
		label: 'css indented with spaces',
		language: 'css',
		output: pinned.css.replaceAll('\t', '  '),
		option: 'useTabs'
	},
	{
		label: 'css rule stayed inline (vacuous indent arm)',
		language: 'css',
		output: pinned.css.replace(
			".probe {\n\tfont-family: 'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI', sans-serif;\n}",
			".probe { font-family: 'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI', sans-serif; }"
		),
		option: 'indentStyle'
	},
	{
		label: 'svelte width fell back below 100',
		language: 'svelte',
		output: pinned.svelte.replace(
			'<div class="probe"><span>alpha</span><span>beta</span><span>gamma</span><span>delta</span></div>',
			'<div class="probe">\n\t<span>alpha</span><span>delta</span>\n</div>'
		),
		option: 'printWidth'
	},
	{
		label: 'svelte indented with spaces',
		language: 'svelte',
		output: pinned.svelte.replaceAll('\t', '  '),
		option: 'useTabs'
	},
	{
		label: 'svelte section stayed inline (vacuous indent arm)',
		language: 'svelte',
		output: pinned.svelte.replace(
			'<section>\n\t<p>aaaaaaaaaa</p>\n\t<p>dddddddddd</p>\n</section>',
			'<section><p>aaaaaaaaaa</p><p>dddddddddd</p></section>'
		),
		option: 'indentStyle'
	},
	{
		// The fallback the one-sided check missed: dprint's 120 for a misspelled global
		// `lineWidth` — every line under 100 still fits, so only an over-100 line shows it.
		label: 'ts width rose above 100',
		language: 'typescript',
		output: pinned.typescript.replace(
			"const probe_wide = {\n\talpha: 'aaaaaaaaa',\n\teps: 'eeee'\n};",
			"const probe_wide = { alpha: 'aaaaaaaaa', eps: 'eeee' };"
		),
		option: 'printWidth'
	},
	{
		label: 'css width rose above 100',
		language: 'css',
		output: pinned.css.replace(
			"\tfont-family:\n\t\t'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI', 'Zeta Sans', serif;",
			"\tfont-family: 'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI', 'Zeta Sans', serif;"
		),
		option: 'printWidth'
	},
	{
		label: 'svelte width rose above 100',
		language: 'svelte',
		output: pinned.svelte.replace(
			'<div class="wide">\n\t<span>alpha</span><span>beta</span><span>gamma</span><span>delta</span><b>eps</b>\n</div>',
			'<div class="wide"><span>alpha</span><span>beta</span><span>gamma</span><span>delta</span><b>eps</b></div>'
		),
		option: 'printWidth'
	},
	{
		// The shape biome produces for `.svelte` when `experimentalFullSupportEnabled`
		// is lost: an EMPTY string, which every other check reads as a clean format.
		label: 'svelte formatter declined the language (empty output)',
		language: 'svelte',
		output: '',
		option: 'lineWidth'
	}
];

for (const { label, language, output, option } of dropped) {
	test(`config did NOT land: ${label}`, () => {
		throws(
			() => assert_format_config_landed('probe-tool', language, output),
			(e: unknown) => e instanceof Error && e.message.includes(option),
			`expected the throw to name '${option}'`
		);
	});
}

const embedded_pinned = [
	'<script>',
	"\tconst probe = 'embedded';",
	'</script>',
	'',
	'<p>{a + b}</p>',
	'',
	'<style>',
	'\t.probe {',
	'\t\tcolor: red;',
	'\t}',
	'</style>',
	''
].join('\n');

test('embedded regions landed: pinned output passes', () => {
	doesNotThrow(() => assert_embedded_regions_landed('probe-tool', embedded_pinned));
});

/** Each dropped hop, as the output a tool produces without it — `hop` is what the throw must name. */
const dropped_embeds: Array<{ label: string; output: string; hop: string }> = [
	{
		// biome before `indentScriptAndStyle`: both bodies flush
		label: 'bodies left flush',
		output: embedded_pinned
			.replace("\tconst probe = 'embedded';", "const probe = 'embedded';")
			.replace('\t.probe {\n\t\tcolor: red;\n\t}', '.probe {\n\tcolor: red;\n}'),
		hop: '<script>'
	},
	{
		label: '<style> body left flush',
		output: embedded_pinned.replace(
			'\t.probe {\n\t\tcolor: red;\n\t}',
			'.probe {\n\tcolor: red;\n}'
		),
		hop: '<style>'
	},
	{
		label: '<script> handed back unformatted',
		output: embedded_pinned.replace("\tconst probe = 'embedded';", '\tconst probe = "embedded"'),
		hop: '<script>'
	},
	{
		label: 'template expression handed back unformatted',
		output: embedded_pinned.replace('{a + b}', '{a+b}'),
		hop: 'template expression'
	}
];

for (const { label, output, hop } of dropped_embeds) {
	test(`embedded regions did NOT land: ${label}`, () => {
		throws(
			() => assert_embedded_regions_landed('probe-tool', output),
			(e: unknown) => e instanceof Error && e.message.includes(hop),
			`expected the throw to name '${hop}'`
		);
	});
}
