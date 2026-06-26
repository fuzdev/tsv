/**
 * Tests for expected-error classification (`check_expected_error`).
 *
 * Pins two contracts:
 *  - Non-standard CSS (SCSS/LESS/PostCSS/front-matter/IE hacks) classifies as an
 *    expected error (tsv correctly rejects it; prettier/oxfmt/biome only pass via
 *    lenient PostCSS-family parsers).
 *  - Genuine standard-CSS parser gaps (Bucket A — empty custom-property values,
 *    `url()` special chars, namespace `*|*`, …) do NOT classify, so they keep
 *    surfacing as `error`s.
 *
 * Plus language-scoping: a CSS marker must never fire on broken TS/Svelte.
 */

import { deepStrictEqual as assertEquals, ok as assert } from 'node:assert';
import { check_expected_error } from './expected_errors.ts';

// --- Bucket B: non-standard CSS → expected ---

const non_standard_css: Array<[string, string, string]> = [
	['front matter', '---\ntitle: x\n---\na { color: red; }\n', 'css_front_matter'],
	['scss var decl', '$myVar: 1px;\na { width: $myVar; }\n', 'scss_variable'],
	['scss interpolation', '.icon.is-#{$n} { color: red; }\n', 'scss_interpolation'],
	['scss dollar interpolation', '.icon.is-$(network) { color: red; }\n', 'scss_interpolation'],
	['scss @mixin', '@mixin foo { color: red; }\n', 'scss_less_at_rule'],
	['postcss @nest', 'a { @nest b & { order: 2; } }\n', 'scss_less_at_rule'],
	['css-modules @value', "@value c: './colors.css';\n", 'scss_less_at_rule'],
	['scss @extend', '.a { @extend .b; }\n', 'scss_less_at_rule'],
	['less import', '@import (multiple) "foo.less";\n', 'less_import'],
	[
		'scss nested props',
		'.funky {\n  font: {\n    family: fantasy;\n  }\n}\n',
		'css_scss_nested_props',
	],
	['ie hacks', '.class {\n*zoom: 1;_width: 200px;\n}\n', 'css_ie_hacks'],
	['range marker', '.x{}<<<PRETTIER_RANGE_END>>>\n', 'range_marker'],
];

for (const [label, content, expected_pattern] of non_standard_css) {
	Deno.test(`css expected: ${label}`, () => {
		const r = check_expected_error(content, 'css');
		assert(r.expected, `expected ${label} to classify`);
		assertEquals(r.pattern!.name, expected_pattern);
	});
}

// --- Bucket A: genuine standard-CSS gaps → NOT expected (must stay errors) ---

const genuine_gaps: Array<[string, string]> = [
	['empty custom property', ':root {\n  --empty:;\n}\n'],
	['one-space custom property', ':root {\n  --one-space: ;\n}\n'],
	['url query string', 'a { background: url(/path?query=1); }\n'],
	['unicode-range wildcard', '@font-face { unicode-range: U+4??; }\n'],
	['universal namespace', '*|* {}\n'],
	['attr function value', 'a[id=func("foo")] {}\n'],
];

for (const [label, content] of genuine_gaps) {
	Deno.test(`css genuine gap stays error: ${label}`, () => {
		const r = check_expected_error(content, 'css');
		assert(!r.expected, `${label} must NOT classify as expected (it is a real gap)`);
	});
}

// --- Language scoping: CSS markers must not fire on TS/Svelte ---

Deno.test('css markers do not fire on TypeScript', () => {
	// `prop: {` (the nested-props marker) is a normal TS object type.
	const ts = 'type T = { font: { weight: number } };\n';
	assertEquals(check_expected_error(ts, 'typescript').expected, false);
});

Deno.test('css markers do not fire on Svelte', () => {
	// `$foo:` is a Svelte reactive label, not an SCSS variable.
	const svelte = '<script>\n$foo: bar;\n</script>\n';
	assertEquals(check_expected_error(svelte, 'svelte').expected, false);
});

// --- Existing svelte/html patterns still apply (no language tag = all langs) ---

Deno.test('scss in <style> still classifies', () => {
	const r = check_expected_error('<style lang="scss">$x: 1;</style>', 'svelte');
	assert(r.expected);
	assertEquals(r.pattern!.name, 'scss_style');
});

Deno.test('clean CSS does not classify', () => {
	assertEquals(check_expected_error('a { color: red; }\n', 'css').expected, false);
});

// --- Non-standard JS/TS proposals → expected (scoped to the .js+.ts bucket) ---

const js_proposals: Array<[string, string, string]> = [
	['pipeline inline', 'const r = x |> f(%);\n', 'js_pipeline_operator'],
	['pipeline leading-pipe across lines', 'foo\n|> bar\n|> baz;\n', 'js_pipeline_operator'],
	['source-phase declaration', 'import source x from "x";\n', 'js_source_phase_import'],
	['source-phase expression', 'import.source("foo");\n', 'js_source_phase_import'],
	['source-phase namespace', 'import source * as x from "x";\n', 'js_source_phase_import'],
	[
		'source-phase binding-named-source',
		'import source source from "x";\n',
		'js_source_phase_import',
	],
];

for (const [label, content, expected_pattern] of js_proposals) {
	Deno.test(`js proposal expected: ${label}`, () => {
		const r = check_expected_error(content, 'typescript');
		assert(r.expected, `expected ${label} to classify`);
		assertEquals(r.pattern!.name, expected_pattern);
	});
}

// --- JS/TS markers must NOT fire on standard syntax (no hidden genuine gaps) ---

const js_must_stay_error: Array<[string, string]> = [
	// Bind operator `::` is intentionally unmatched — `a::b` strings are common
	// (embedded Rust/SQL/IPv6), so bind-expression files stay `error`s.
	['rust path in template', 'const c = `tokio::spawn`;\n'],
	['sql cast string', 'const q = "conrelid::regclass";\n'],
	// Standard default import whose binding happens to be named `source`.
	['standard default import named source', 'import source from "x";\n'],
	// `|>` inside a regex alternation (`>` literal after `|`) is not the operator.
	['gt-after-pipe in regex', 'const re = /a\\/>|>b/;\n'],
];

for (const [label, content] of js_must_stay_error) {
	Deno.test(`js stays error (no false-positive): ${label}`, () => {
		const r = check_expected_error(content, 'typescript');
		assert(!r.expected, `${label} must NOT classify (would hide a real gap)`);
	});
}

// --- range marker is language-agnostic (was CSS-scoped) ---

Deno.test('range marker classifies in TS too', () => {
	const r = check_expected_error('a = 1;<<<PRETTIER_RANGE_START>>>b = 2;\n', 'typescript');
	assert(r.expected);
	assertEquals(r.pattern!.name, 'range_marker');
});
