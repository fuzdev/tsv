/**
 * Tests for the pinned-layout-config grader (`assert_format_config_landed`).
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
 * `deno task check` (see `test:deno`). The claims about what each TOOL actually
 * emits at its defaults live in the module's own docs, verified by running them.
 *
 * The vacuity arms are here too — an assertion that grades nothing passes every
 * run, so "the probe stopped discriminating" must be a failure, and that is
 * exactly the shape no live run can show you.
 */

import { throws, doesNotThrow } from 'node:assert';
import { assert_format_config_landed } from './format_config_probe.ts';
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
		''
	].join('\n'),
	css: [
		'.probe {',
		"\tfont-family: 'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI', sans-serif;",
		'}',
		''
	].join('\n'),
	svelte: [
		'<div class="probe"><span>alpha</span><span>beta</span><span>gamma</span><span>delta</span></div>',
		'<section>',
		'\t<p>aaaaaaaaaa</p>',
		'\t<p>dddddddddd</p>',
		'</section>',
		''
	].join('\n')
};

for (const language of Object.keys(pinned) as Array<Language>) {
	Deno.test(`config landed: ${language} pinned output passes`, () => {
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
		output: [
			'.probe {',
			'\tfont-family:',
			"\t\t'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI',",
			'\t\tsans-serif;',
			'}',
			''
		].join('\n'),
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
		output:
			".probe { font-family: 'Alpha Sans', 'Beta Serif', 'Gamma Mono', 'Delta Text', 'Epsilon UI', sans-serif; }\n",
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
		// The shape biome produces for `.svelte` when `experimentalFullSupportEnabled`
		// is lost: an EMPTY string, which every other check reads as a clean format.
		label: 'svelte formatter declined the language (empty output)',
		language: 'svelte',
		output: '',
		option: 'lineWidth'
	}
];

for (const { label, language, output, option } of dropped) {
	Deno.test(`config did NOT land: ${label}`, () => {
		throws(
			() => assert_format_config_landed('probe-tool', language, output),
			(e: unknown) => e instanceof Error && e.message.includes(option),
			`expected the throw to name '${option}'`
		);
	});
}
