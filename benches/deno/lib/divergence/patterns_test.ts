/**
 * Synthetic tests for divergence detection patterns.
 *
 * Each pattern gets:
 * - Positive test: minimal synthetic diff that SHOULD trigger the pattern
 * - Negative test: similar-looking diff that should NOT trigger
 *
 * This catches overmatching — patterns incorrectly claiming hunks.
 */

import { assertEquals, assertNotEquals } from 'https://deno.land/std@0.224.0/assert/mod.ts';
import { diffLines, extractHunks } from '../diff.ts';
import {
	type DetectionContext,
	type DivergenceMatch,
	enrichDetectionContext,
	PATTERNS,
} from './patterns.ts';
import type { Language } from '../types.ts';

/**
 * Create a DetectionContext from ours/prettier strings.
 * Generates diff and hunks automatically.
 */
function makeContext(
	ours: string,
	prettier: string,
	language: Language = 'svelte',
): DetectionContext {
	const diff = diffLines(prettier, ours);
	const hunks = extractHunks(diff);
	const ctx: DetectionContext = {
		source: prettier, // simplification: source ≈ prettier for detection
		ours,
		prettier,
		diff,
		hunks,
		language,
	};
	enrichDetectionContext(ctx);
	return ctx;
}

/** Calculate visual width of a line (tabs = 2 spaces). */
function visualWidth(line: string): number {
	let width = 0;
	for (const char of line) {
		width += char === '\t' ? 2 : 1;
	}
	return width;
}

/** Run a single pattern's detect() on a context. */
function runPattern(patternId: string, ctx: DetectionContext): DivergenceMatch | null {
	const pattern = PATTERNS.find((p) => p.id === patternId);
	if (!pattern) throw new Error(`Unknown pattern: ${patternId}`);
	return pattern.detect(ctx);
}

// ─── template_literal_width ─────────────────────────────────────────────────

Deno.test('template_literal_width: positive - }` closing on own line', () => {
	// In real diffs, the template spans multiple lines. Prettier keeps ${expr} on one line,
	// we break ${...} to separate lines. The diff has added lines with ${ break
	// that don't appear in removed lines.
	// Simulate: ours adds a `}` backtick line that prettier doesn't have
	const prettier = 'line1\nline2 end';
	const ours = 'line1\n\t}`\nline2 end';
	const ctx = makeContext(ours, prettier, 'typescript');
	ctx.source = 'const x = `${expr}`;'; // source has ${
	const match = runPattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: positive - ${ at end of line in ours', () => {
	// We break after ${ (end of line), prettier keeps ${expr} inline
	const prettier = '\t\tconsole.error(\n\t\t\t`msg "${VALUE}", got "${fixture.prop}"`,\n\t\t);';
	const ours = '\t\tconsole.error(`msg "${VALUE}", got "${\n\t\t\t\tfixture.prop\n\t\t\t}"`);\n';
	const ctx = makeContext(ours, prettier, 'typescript');
	ctx.source = 'console.error(`msg "${VALUE}", got "${fixture.prop}"`);';
	const match = runPattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: negative - inline ${ on both sides', () => {
	// Both sides have ${expr} inline (not at end of line) — not a template break divergence
	const prettier = '\t\tconsole.log(`hello ${name}`);\n\t\tconsole.log(`bye ${name}`);';
	const ours = '\t\tconsole.log(`hello ${name}`);\n\t\tconsole.log(`goodbye ${name}`);';
	const ctx = makeContext(ours, prettier, 'typescript');
	ctx.source = 'console.log(`hello ${name}`);';
	const match = runPattern('template_literal_width', ctx);
	assertEquals(match, null);
});

Deno.test('template_literal_width: negative - no ${ in source', () => {
	const prettier = 'const x = someVeryLongVariableName.property.method();';
	const ours = 'const x =\n\tsomeVeryLongVariableName.property.method();';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('template_literal_width', ctx);
	assertEquals(match, null);
});

Deno.test('template_literal_width: negative - both sides have ${ breaks', () => {
	const prettier = 'const x = `hello ${\n\tname\n} world`;';
	const ours = 'const x = `hello ${\n\tname\n} world`;';
	const ctx = makeContext(ours, prettier, 'typescript');
	// No diff, no hunks
	assertEquals(ctx.hunks.length, 0);
});

Deno.test('template_literal_width: positive - atomization divergence (both sides break, different interpolation)', () => {
	// Both sides break at ${} boundaries, but at different interpolations.
	// Prettier atomizes simple expressions (keeps ${indent} inline) and breaks elsewhere.
	// We break the simple expression instead. Key signal: isolated simple expression in
	// our added lines that appears inline as ${expr} in prettier's removed lines.
	const prettier =
		'\t\t\treturn `<span style="--indent: ${indent}ch">${\n\t\t\t\tline ?? \'\'\n\t\t\t}</span>`;';
	const ours =
		'\t\t\treturn `<span style="--indent: ${\n\t\t\t\tindent\n\t\t\t}ch">${line ?? \'\'}</span>`;';
	const ctx = makeContext(ours, prettier, 'typescript');
	ctx.source = 'return `<span style="--indent: ${indent}ch">${line ?? \'\'}</span>`;';
	const match = runPattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: positive - atomization divergence (more breaks in ours)', () => {
	// Prettier keeps ${spec.method} inline (atomized), we break it.
	// Both sides end with ${ but ours has an additional break.
	const prettier = '\t\t\treturn `${spec.method}: (${';
	const ours = '\t\t\treturn `${\n\t\t\t\tspec.method\n\t\t\t}: (${';
	const ctx = makeContext(ours, prettier, 'typescript');
	ctx.source = 'return `${spec.method}: (${...';
	const match = runPattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: positive - atomization divergence (prettier breaks simple expr, ours keeps inline)', () => {
	// Reverse of the typical atomization case: prettier breaks the simple expression
	// (response.status) while ours keeps it inline and breaks a different expression.
	// From load_data.js: `...value: "${response.status}" type: ${typeof response.status}`
	const prettier =
		'\t\t\t\t\t\t\t`response.status is not a number. value: "${\n\t\t\t\t\t\t\t\tresponse.status\n\t\t\t\t\t\t\t}" type: ${typeof response.status}`,';
	const ours =
		'\t\t\t\t\t\t\t`response.status is not a number. value: "${response.status}" type: ${\n\t\t\t\t\t\t\t\ttypeof response.status\n\t\t\t\t\t\t\t}`,';
	const ctx = makeContext(ours, prettier, 'typescript');
	ctx.source =
		'`response.status is not a number. value: "${response.status}" type: ${typeof response.status}`,';
	const match = runPattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: negative - both sides break same complex expr', () => {
	// Both sides break at ${} but with complex expressions (not atomizable)
	// — not a template atomization divergence
	const prettier = '\t\tconst x = `${\n\t\t\tfoo() + bar()\n\t\t}`;';
	const ours = '\t\tconst x = `${\n\t\t\tfoo() +\n\t\t\tbar()\n\t\t}`;';
	const ctx = makeContext(ours, prettier, 'typescript');
	ctx.source = 'const x = `${foo() + bar()}`;';
	const match = runPattern('template_literal_width', ctx);
	assertEquals(match, null);
});

// ─── block_expression_logical ───────────────────────────────────────────────

Deno.test('block_expression_logical: positive - && at start of line in ours', () => {
	const prettier = '{#if someCondition && anotherCondition && thirdCondition}content{/if}';
	const ours = '{#if someCondition\n\t&& anotherCondition\n\t&& thirdCondition}content{/if}';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('block_expression_logical', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'block_expression_logical');
});

Deno.test('block_expression_logical: negative - not svelte', () => {
	const prettier = 'if (someCondition && anotherCondition) {}';
	const ours = 'if (someCondition\n\t&& anotherCondition) {}';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('block_expression_logical', ctx);
	assertEquals(match, null);
});

Deno.test('block_expression_logical: negative - both sides have operator breaks', () => {
	const prettier = '{#if a\n\t&& b}content{/if}';
	const ours = '{#if a\n\t&& b}content{/if}';
	const ctx = makeContext(ours, prettier, 'svelte');
	assertEquals(ctx.hunks.length, 0);
});

// ─── fill_101_boundary ──────────────────────────────────────────────────────

Deno.test('fill_101_boundary: positive - prettier > 100 chars, we break', () => {
	// Simulate a 105-char prettier line that we break
	const longLine = '\t' + 'x'.repeat(103); // visual width = 2 + 103 = 105
	const prettier = `before\n${longLine}\nafter`;
	const ours = `before\n\t${'x'.repeat(50)}\n\t${'x'.repeat(53)}\nafter`;
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('fill_101_boundary', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'fill_101_boundary');
});

Deno.test('fill_101_boundary: negative - prettier lines under 100 chars', () => {
	const prettier = 'before\n\t' + 'x'.repeat(90) + '\nafter';
	const ours = 'before\n\t' + 'x'.repeat(45) + '\n\t' + 'x'.repeat(45) + '\nafter';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('fill_101_boundary', ctx);
	assertEquals(match, null);
});

Deno.test('fill_101_boundary: positive - same line count, different wrapping at print width', () => {
	// Prettier has 105-char line, we rewrap to 3 lines all ≤ 100 chars (same total line count)
	const prettierLine = '\t' + 'x'.repeat(50) + ' ' + 'y'.repeat(52); // visual width = 2 + 50 + 1 + 52 = 105
	const prettier = `before\n${prettierLine}\nyy\nafter`;
	// Same 3 content lines, but we break differently (all ≤ 100)
	const ours = `before\n\t${'x'.repeat(50)}\n\t${'y'.repeat(52)} yy\nafter`;
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('fill_101_boundary', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'fill_101_boundary');
});

Deno.test('fill_101_boundary: negative - our lines also exceed 100 chars', () => {
	// Both sides have lines > 100 chars — not a print-width boundary divergence
	const longLine = '\t' + 'x'.repeat(103); // visual width = 105
	const prettier = `before\n${longLine}\nafter`;
	const ours = `before\n${longLine}\n\textra\nafter`;
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('fill_101_boundary', ctx);
	assertEquals(match, null);
});

Deno.test('fill_101_boundary: negative - we have fewer lines (not a break)', () => {
	const longLine = '\t' + 'x'.repeat(103);
	const prettier = `before\n${longLine}\n${'x'.repeat(50)}\nafter`;
	// We have FEWER lines, not more - not a break scenario
	const ours = `before\n${longLine}\nafter`;
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('fill_101_boundary', ctx);
	assertEquals(match, null);
});

// ─── menu_block ─────────────────────────────────────────────────────────────

Deno.test('menu_block: positive - prettier hugs menu content, we expand', () => {
	const prettier = '<menu\n\tdata-attr1="value1"\n\tdata-attr2="value2">{@render fn()}</menu\n>';
	const ours = '<menu\n\tdata-attr1="value1"\n\tdata-attr2="value2"\n>\n\t{@render fn()}\n</menu>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('menu_block', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'menu_block');
	assertEquals(match!.confidence, 'certain');
});

Deno.test('menu_block: negative - not svelte', () => {
	const prettier = '<menu\n\tclass="nav">{@render fn()}</menu\n>';
	const ours = '<menu\n\tclass="nav"\n>\n\t{@render fn()}\n</menu>';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('menu_block', ctx);
	assertEquals(match, null);
});

Deno.test('menu_block: negative - not a menu element', () => {
	const prettier = '<div\n\tclass="nav">content</div>';
	const ours = '<div\n\tclass="nav"\n>\n\tcontent\n</div>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('menu_block', ctx);
	assertEquals(match, null);
});

// ─── inline_content_hug ─────────────────────────────────────────────────────

Deno.test('inline_content_hug: positive - we hug >{, prettier breaks >', () => {
	const prettier = '<span\n\tclass="long"\n>\n\t{content}\n</span>';
	const ours = '<span\n\tclass="long"\n>{content}\n</span>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('inline_content_hug', ctx);
	assertNotEquals(match, null);
});

Deno.test('inline_content_hug: positive - prettier >content on same line, we break ternary', () => {
	const prettier =
		"<small\n\t>no action history{show ? '' : ', showing only write actions'}</small";
	const ours = "<small>no action history{show\n\t? ''\n\t: ', showing only write actions'}</small";
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('inline_content_hug', ctx);
	assertNotEquals(match, null);
});

Deno.test('inline_content_hug: negative - not svelte', () => {
	const prettier = '<span\n\tclass="long"\n>\n\t{content}\n</span>';
	const ours = '<span\n\tclass="long"\n>{content}\n</span>';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('inline_content_hug', ctx);
	assertEquals(match, null);
});

// ─── single_specifier_import ────────────────────────────────────────────────

Deno.test('single_specifier_import: positive - long import wraps', () => {
	const prettier =
		'import { someVeryLongExportedNameThatExceedsPrintWidthWhenCombinedWithTheModulePath } from "./some/very/long/module/path";';
	const ours =
		'import {\n\tsomeVeryLongExportedNameThatExceedsPrintWidthWhenCombinedWithTheModulePath,\n} from "./some/very/long/module/path";';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('single_specifier_import', ctx);
	assertNotEquals(match, null);
});

Deno.test('single_specifier_import: negative - import under 100 chars', () => {
	const prettier = 'import { foo } from "./bar";';
	const ours = 'import {\n\tfoo,\n} from "./bar";';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('single_specifier_import', ctx);
	assertEquals(match, null);
});

// ─── self_closing_nonvoid ───────────────────────────────────────────────────

Deno.test('self_closing_nonvoid: positive - self-closing component', () => {
	const prettier = '<Component></Component>';
	const ours = '<Component />';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('self_closing_nonvoid', ctx);
	assertNotEquals(match, null);
});

Deno.test('self_closing_nonvoid: negative - not svelte', () => {
	const prettier = '<Component></Component>';
	const ours = '<Component />';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('self_closing_nonvoid', ctx);
	assertEquals(match, null);
});

Deno.test('self_closing_nonvoid: positive - HTML element ours expands self-closing', () => {
	const prettier = '<div />';
	const ours = '<div></div>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('self_closing_nonvoid', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'self_closing_nonvoid');
});

Deno.test('self_closing_nonvoid: positive - split hunks from identical intervening line', () => {
	// When <div /> → <div></div> has an identical <div></div> between them,
	// the diff splits into two hunks (one remove-only, one add-only)
	const prettier = '<div />\n<div></div>';
	const ours = '<div></div>\n<div></div>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('self_closing_nonvoid', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.hunkIndices.length, 2);
});

Deno.test('self_closing_nonvoid: positive - multiline element /> vs ></div>', () => {
	const prettier = '  data-my-prop\n/>';
	const ours = '  data-my-prop\n></div>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('self_closing_nonvoid', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'self_closing_nonvoid');
});

Deno.test('self_closing_nonvoid: negative - different elements in wrapping diff', () => {
	// Self-closing <Glyph /> is same in both outputs (just rewrapped),
	// </ProviderLink> is an unrelated close tag also rewrapped
	const prettier = '><span><Glyph glyph={GLYPH} /> text</span\n> provider</ProviderLink';
	const ours = '><span><Glyph glyph={GLYPH} /> text</span> provider</ProviderLink';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('self_closing_nonvoid', ctx);
	assertEquals(match, null);
});

Deno.test('self_closing_nonvoid: negative - dotted component name regex escape', () => {
	// <X.Y /> (member expression component) self-closes in ours,
	// </X_Y> is a different component's close tag in prettier.
	// Without escaping `.` in the regex, `X.Y` matches `X_Y`
	// because `.` is a regex wildcard — false positive.
	const prettier = '</X_Y>';
	const ours = '<X.Y />';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('self_closing_nonvoid', ctx);
	assertEquals(match, null);
});

// ─── bom_strip ──────────────────────────────────────────────────────────────

Deno.test('bom_strip: positive - BOM in source, we strip', () => {
	const prettier = '\ufeffconst x = 1;';
	const ours = 'const x = 1;';
	const ctx = makeContext(ours, prettier, 'typescript');
	// BOM needs to be in source
	ctx.source = '\ufeffconst x = 1;';
	const match = runPattern('bom_strip', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.confidence, 'certain');
});

Deno.test('bom_strip: negative - no BOM in source', () => {
	const prettier = 'const x = 1;';
	const ours = 'const y = 1;';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('bom_strip', ctx);
	assertEquals(match, null);
});

// ─── fill_after_inline ──────────────────────────────────────────────────────

Deno.test('fill_after_inline: positive - long line with inline close tag', () => {
	const longContent = 'x'.repeat(90);
	const prettier = `\t<p>Some text <span>${longContent}</span> more text after the inline</p>`;
	const ours = `\t<p>Some text <span>${longContent}</span>\n\tmore text after the inline</p>`;
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('fill_after_inline', ctx);
	assertNotEquals(match, null);
});

Deno.test('fill_after_inline: negative - inline close tag under 100 chars', () => {
	const prettier = '\t<p>Some text <span>short</span> more text</p>';
	const ours = '\t<p>Some text <span>short</span>\n\tmore text</p>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('fill_after_inline', ctx);
	assertEquals(match, null);
});

Deno.test('fill_after_inline: negative - not svelte', () => {
	const longContent = 'x'.repeat(90);
	const prettier = `\t<p>Some text <span>${longContent}</span> more text</p>`;
	const ours = `\t<p>Some text <span>${longContent}</span>\n\tmore text</p>`;
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('fill_after_inline', ctx);
	assertEquals(match, null);
});

// ─── css_value_wrap ─────────────────────────────────────────────────────────

Deno.test('css_value_wrap: positive - long CSS property value', () => {
	const longValue = 'var(--a) ' + 'color-mix(in srgb, red, blue) '.repeat(3);
	const prettier = `\tbox-shadow: ${longValue};`;
	const ours = `\tbox-shadow:\n\t\t${longValue.split(' ').slice(0, 3).join(' ')}\n\t\t${
		longValue.split(' ').slice(3).join(' ')
	};`;
	const ctx = makeContext(ours, prettier, 'css');
	const match = runPattern('css_value_wrap', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_value_wrap: negative - short CSS property value', () => {
	const prettier = '\tcolor: red;';
	const ours = '\tcolor: blue;';
	const ctx = makeContext(ours, prettier, 'css');
	const match = runPattern('css_value_wrap', ctx);
	assertEquals(match, null);
});

// ─── member_expression_call ─────────────────────────────────────────────────

Deno.test('member_expression_call: positive - require.resolve in diff', () => {
	const prettier = 'const p = require.resolve.paths("some/very/long/module/path/that/exceeds");';
	const ours = 'const p = require.resolve.paths(\n\t"some/very/long/module/path/that/exceeds",\n);';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('member_expression_call', ctx);
	assertNotEquals(match, null);
});

Deno.test('member_expression_call: negative - no module pattern in source', () => {
	const prettier = 'const p = something.other("path");';
	const ours = 'const p = something.other(\n\t"path",\n);';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('member_expression_call', ctx);
	assertEquals(match, null);
});

// ─── comment_position ───────────────────────────────────────────────────────

Deno.test('comment_position: positive - comment moved to different line', () => {
	// Prettier puts comment after {, we put it on its own line
	// The comment text "todo" is the same, just on different lines
	const prettier = 'for (let i = 0; i < n; i++) // todo\n{\n\tx++;\n}';
	const ours = 'for (let i = 0; i < n; i++)\n\t// todo\n{\n\tx++;\n}';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('comment_position', ctx);
	assertNotEquals(match, null);
});

Deno.test('comment_position: negative - identical comments same position', () => {
	const prettier = 'const x = 1; // comment\nconst y = 2;';
	const ours = 'const x = 1; // comment\nconst y = 2;';
	const ctx = makeContext(ours, prettier, 'typescript');
	assertEquals(ctx.hunks.length, 0);
});

Deno.test('comment_position: negative - comment incidental to code layout change', () => {
	// Non-comment code differs (return vs return + paren wrapping) — comment is incidental
	const prettier = 'return (\n\tstr\n\t\t// replace\n\t\t.replace(/a/g, "-")\n);';
	const ours = 'return str\n\t// replace\n\t.replace(/a/g, "-");';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('comment_position', ctx);
	assertEquals(match, null);
});

Deno.test('comment_position: negative - Case 1 comment not in other side output', () => {
	// Comment on one side only, and the comment text doesn't exist in the other output
	const prettier = 'const x = 1;\nconst y = 2;';
	const ours = 'const x = 1;\n// added comment\nconst y = 2;';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('comment_position', ctx);
	assertEquals(match, null);
});

Deno.test('comment_position: positive - Case 1 comment moved out of hunk region', () => {
	// Comment on one side of hunk, but text exists in other side's full output (moved)
	const prettier = '// todo\nconst x = 1;\nconst y = 2;';
	const ours = 'const x = 1;\n// todo\nconst y = 2;';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('comment_position', ctx);
	assertNotEquals(match, null);
});

// ─── block_multiline_attrs_hug ──────────────────────────────────────────────

Deno.test('block_multiline_attrs_hug: positive - > on own line with pre context', () => {
	// prettier: <textarea with attr line ending in "> (removedHugsGt matches /['"]\s*>/)
	// ours: > on its own line (addedBreaksGt matches /^\t*>$/)
	// The <textarea tag must be in the hunk's ours/prettier line range or context lines
	// Putting the entire element in a single diff hunk by making all lines different
	const prettier = '<textarea class="code" id="x">content</textarea>';
	const ours = '<textarea\n\tclass="code"\n\tid="x"\n>\ncontent</textarea>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('block_multiline_attrs_hug', ctx);
	assertNotEquals(match, null);
});

Deno.test('block_multiline_attrs_hug: negative - not svelte', () => {
	const prettier = '<pre\n\tclass="code"\n\tdata-lang="js">content</pre>';
	const ours = '<pre\n\tclass="code"\n\tdata-lang="js"\n>content</pre>';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('block_multiline_attrs_hug', ctx);
	assertEquals(match, null);
});

Deno.test('block_multiline_attrs_hug: negative - not ws-sensitive element', () => {
	const prettier = '<div\n\tclass="long"\n\tdata-x="y">content</div>';
	const ours = '<div\n\tclass="long"\n\tdata-x="y"\n>content</div>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('block_multiline_attrs_hug', ctx);
	assertEquals(match, null);
});

// ─── short_expr_100 ─────────────────────────────────────────────────────────

Deno.test('short_expr_100: positive - block expr slightly over 100', () => {
	// Create a block expression that's 105 chars in prettier (within 100-110 range)
	const expr = 'a'.repeat(65);
	const prettier = `{#if typeof ${expr} === 'string'}content{/if}`;
	const ours = `{#if\n\ttypeof ${expr} === 'string'\n}content{/if}`;
	const ctx = makeContext(ours, prettier, 'svelte');
	const vw = visualWidth(prettier);
	assertEquals(vw > 100 && vw <= 110, true, `Expected visual width 101-110, got ${vw}`);
	const match = runPattern('short_expr_100', ctx);
	assertNotEquals(match, null);
});

Deno.test('short_expr_100: negative - not svelte', () => {
	const prettier = '{#if typeof x === "string"}content{/if}';
	const ours = '{#if\n\ttypeof x === "string"\n}content{/if}';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('short_expr_100', ctx);
	assertEquals(match, null);
});

// ─── css_atrule_spec_spacing ────────────────────────────────────────────────

Deno.test('css_atrule_spec_spacing: positive - missing space after and(', () => {
	const prettier =
		'<style>\n@container (min-width: 700px) and(min-height: 500px) {\n\tdiv { color: red; }\n}\n</style>';
	const ours =
		'<style>\n@container (min-width: 700px) and (min-height: 500px) {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_atrule_spec_spacing', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.confidence, 'certain');
});

Deno.test('css_atrule_spec_spacing: negative - correct spacing already', () => {
	const prettier =
		'<style>\n@media screen and (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ours =
		'<style>\n@media screen and (min-width: 768px) {\n\tdiv { color: blue; }\n}\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_atrule_spec_spacing', ctx);
	assertEquals(match, null);
});

// ─── css_atrule_long_wrap ───────────────────────────────────────────────────

Deno.test('css_atrule_long_wrap: positive - @media over 100 chars, we wrap', () => {
	const longQuery =
		'@media screen and (min-width: 768px) and (max-width: 1024px) and (orientation: landscape) and (color)';
	const prettier = `<style>\n${longQuery} {\n\tdiv { color: red; }\n}\n</style>`;
	const ours =
		`<style>\n@media screen and (min-width: 768px) and (max-width: 1024px)\n\tand (orientation: landscape) and (color) {\n\tdiv { color: red; }\n}\n</style>`;
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_atrule_long_wrap', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_atrule_long_wrap: negative - @media under 100 chars', () => {
	const prettier =
		'<style>\n@media screen and (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ours =
		'<style>\n@media screen\n\tand (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_atrule_long_wrap', ctx);
	assertEquals(match, null);
});

// ─── css_atrule_stable_quirk ────────────────────────────────────────────────

Deno.test('css_atrule_stable_quirk: positive - @layer with extra spaces', () => {
	const prettier = '<style>\n@layer base,  components;\n</style>';
	const ours = '<style>\n@layer base, components;\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_atrule_stable_quirk', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_atrule_stable_quirk: positive - @scope with spaces in parens', () => {
	const prettier =
		'<style>\n@scope ( .class1 )  to  ( .class2 ) {\n\timg { border: 1px; }\n}\n</style>';
	const ours = '<style>\n@scope (.class1) to (.class2) {\n\timg { border: 1px; }\n}\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_atrule_stable_quirk', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_atrule_stable_quirk: negative - at-rule wrapping (different pattern)', () => {
	const prettier =
		'<style>\n@media screen and (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ours =
		'<style>\n@media screen\n\tand (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_atrule_stable_quirk', ctx);
	assertEquals(match, null);
});

// ─── css_selector_divergence ────────────────────────────────────────────────

Deno.test('css_selector_divergence: positive - column combinator ||', () => {
	const prettier = '<style>\ncol.selected||td {\n\tcolor: red;\n}\n</style>';
	const ours = '<style>\ncol.selected || td {\n\tcolor: red;\n}\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_selector_divergence', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_selector_divergence: positive - nth-child normalization', () => {
	const prettier = '<style>\nli:nth-child(3n- 2) {\n\tcolor: yellow;\n}\n</style>';
	const ours = '<style>\nli:nth-child(3n - 2) {\n\tcolor: yellow;\n}\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_selector_divergence', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_selector_divergence: negative - || in TypeScript (logical OR)', () => {
	const prettier = 'const x = a || b;';
	const ours = 'const x = a ||\n\tb;';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('css_selector_divergence', ctx);
	assertEquals(match, null);
});

// ─── css_value_ratio ────────────────────────────────────────────────────────

Deno.test('css_value_ratio: positive - ratio spacing in media query', () => {
	const prettier = '<style>\n@media (aspect-ratio: 16  /  9) {\n\tdiv { color: red; }\n}\n</style>';
	const ours = '<style>\n@media (aspect-ratio: 16 / 9) {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_value_ratio', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_value_ratio: negative - division in TypeScript', () => {
	const prettier = 'const x = 16  /  9;';
	const ours = 'const x = 16 / 9;';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('css_value_ratio', ctx);
	assertEquals(match, null);
});

// ─── css_comment_stable_quirk ───────────────────────────────────────────────

Deno.test('css_comment_stable_quirk: positive - comment before { in at-rule', () => {
	const prettier = '<style>\n@media screen/* comment */ {\n\tdiv { color: red; }\n}\n</style>';
	const ours = '<style>\n@media screen /* comment */ {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('css_comment_stable_quirk', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_comment_stable_quirk: positive - CSS language', () => {
	const prettier = '@media screen/* comment */ {\n\tdiv { color: red; }\n}';
	const ours = '@media screen /* comment */ {\n\tdiv { color: red; }\n}';
	const ctx = makeContext(ours, prettier, 'css');
	const match = runPattern('css_comment_stable_quirk', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_comment_stable_quirk: negative - TypeScript comment (not CSS)', () => {
	const prettier = 'if (x) /* comment */ {\n\tconsole.log(1);\n}';
	const ours = 'if (x) /* comment */\n{\n\tconsole.log(1);\n}';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('css_comment_stable_quirk', ctx);
	assertEquals(match, null);
});

// ─── empty_statement_removal ────────────────────────────────────────────────

Deno.test('empty_statement_removal: positive - standalone ; removed', () => {
	const prettier = '<script>\n;\n;\nconst x = 1;\n</script>';
	const ours = '<script>\nconst x = 1;\n</script>';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('empty_statement_removal', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.confidence, 'certain');
});

Deno.test('empty_statement_removal: negative - semicolons in for(;;)', () => {
	const prettier = 'for (;;) {\n\tbreak;\n}';
	const ours = 'for (;;) {\n\tbreak;\n}';
	const ctx = makeContext(ours, prettier, 'typescript');
	assertEquals(ctx.hunks.length, 0);
});

Deno.test('empty_statement_removal: negative - normal statement semicolons', () => {
	const prettier = 'const x = 1;\nconst y = 2;';
	const ours = 'const x = 1;\nconst y = 3;';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('empty_statement_removal', ctx);
	assertEquals(match, null);
});

// ─── return_type_generic_union ──────────────────────────────────────────────

Deno.test('return_type_generic_union: positive - generic with | null wraps', () => {
	// Need prettier line > 100 chars with generic union
	const prettier =
		'function processWithVeryLongFunctionName(input: SomeVeryLongParameterType): Promise<VeryLongReturnTypeName | null> {';
	const ours =
		'function processWithVeryLongFunctionName(\n\tinput: SomeVeryLongParameterType,\n): Promise<\n\tVeryLongReturnTypeName | null\n> {';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('return_type_generic_union', ctx);
	assertNotEquals(match, null);
});

Deno.test('return_type_generic_union: negative - generic without union', () => {
	const prettier = 'function foo(): Promise<string> {';
	const ours = 'function foo(): Promise<\n\tstring\n> {';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('return_type_generic_union', ctx);
	assertEquals(match, null);
});

// ─── instantiation_parens ─────────────────────────────────────────────────

Deno.test('instantiation_parens: positive - ternary parens stripped', () => {
	const prettier = '\tlet c = x ? y : z<T>;';
	const ours = '\tlet c = (x ? y : z)<T>;';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('instantiation_parens', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'instantiation_parens');
	assertEquals(match!.confidence, 'certain');
});

Deno.test('instantiation_parens: positive - binary parens stripped', () => {
	const prettier = '\tlet d = a + b<T>;';
	const ours = '\tlet d = (a + b)<T>;';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('instantiation_parens', ctx);
	assertNotEquals(match, null);
});

Deno.test('instantiation_parens: negative - assignment parens (both agree)', () => {
	// Both formatters preserve parens for assignment — no diff
	const prettier = '\tlet a = (x = y)<T>;';
	const ours = '\tlet a = (x = y)<T>;';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('instantiation_parens', ctx);
	assertEquals(match, null);
});

Deno.test('instantiation_parens: positive - lowercase type param', () => {
	const prettier = '\tlet e = x ? y : z<string>;';
	const ours = '\tlet e = (x ? y : z)<string>;';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('instantiation_parens', ctx);
	assertNotEquals(match, null);
});

Deno.test('instantiation_parens: negative - normal generic usage', () => {
	const prettier = '\tconst x = foo<T>();';
	const ours = '\tconst x = foo<T>();';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('instantiation_parens', ctx);
	assertEquals(match, null);
});

// ─── block_comment_computed_member ────────────────────────────────────────

Deno.test('block_comment_computed_member: positive - JSDoc hoisted from brackets', () => {
	const prettier = '\t/** @type {string} */ obj.aaaa.bbbb.cccc?.[\n\t\td\n\t];';
	const ours = '\tobj.aaaa.bbbb.cccc?.[\n\t\t/** @type {string} */ d\n\t];';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('block_comment_computed_member', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'block_comment_computed_member');
	assertEquals(match!.confidence, 'certain');
});

Deno.test('block_comment_computed_member: negative - JSDoc in normal position', () => {
	const prettier = '\t/** @type {string} */ const x = 1;';
	const ours = '\t/** @type {string} */ const x = 1;';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('block_comment_computed_member', ctx);
	assertEquals(match, null);
});

Deno.test('block_comment_computed_member: positive - non-optional computed', () => {
	// Same pattern with non-optional computed member (still detects)
	const prettier = '\t/** @type {string} */ obj.aaaa.bbbb.cccc[\n\t\td\n\t];';
	const ours = '\tobj.aaaa.bbbb.cccc[\n\t\t/** @type {string} */ d\n\t];';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('block_comment_computed_member', ctx);
	assertNotEquals(match, null);
});

// ─── block_comment_chain ──────────────────────────────────────────────────

Deno.test('block_comment_chain: positive - comment spacing before dot differs', () => {
	// Prettier intermediate: `a/* inner */ .b` (space before dot)
	// Ours/stable: `a /* inner */.b` (no space before dot)
	const prettier = '\t/* outer */ a/* inner */ .b\n\t\t.c(a);';
	const ours = '\t/* outer */ a /* inner */.b\n\t\t.c(a);';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('block_comment_chain', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'block_comment_chain');
	assertEquals(match!.confidence, 'likely');
});

Deno.test('block_comment_chain: positive - deeper chain member', () => {
	const prettier = '\t/* outer */ a.b/* inner */ .c\n\t\t.d(a);';
	const ours = '\t/* outer */ a.b /* inner */.c\n\t\t.d(a);';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('block_comment_chain', ctx);
	assertNotEquals(match, null);
});

Deno.test('block_comment_chain: negative - identical comment spacing', () => {
	const prettier = '\t/* comment */ a.b\n\t\t.c(a);';
	const ours = '\t/* comment */ a.b\n\t\t.c(a);';
	const ctx = makeContext(ours, prettier, 'typescript');
	const match = runPattern('block_comment_chain', ctx);
	assertEquals(match, null);
});

Deno.test('block_comment_chain: negative - CSS (not applicable)', () => {
	const prettier = '\t/* comment */ .class { color: red; }';
	const ours = '\t/* comment */.class { color: red; }';
	const ctx = makeContext(ours, prettier, 'css');
	const match = runPattern('block_comment_chain', ctx);
	assertEquals(match, null);
});

// ─── fill_101_boundary covers multiline_value_inline_long ─────────────────

Deno.test('fill_101_boundary: positive - multiline attr inline long text', () => {
	// Prettier keeps trailing text on one line (102 chars), we break at word boundary
	const prettier =
		'\t> text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14 text15_ x';
	const ours =
		'\t> text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14 text15_\n\tx';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('fill_101_boundary', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'fill_101_boundary');
});

// ─── jsdoc_type_cast_parens covers arrow_jsdoc_cast_body_long ─────────────

Deno.test('jsdoc_type_cast_parens: positive - arrow callback body', () => {
	// prettier-plugin-svelte preserves parens, we strip them
	const prettier = '\tconst a = b.map((x) => /** @type {A} */ (fn(x)));';
	const ours = '\tconst a = b.map((x) => /** @type {A} */ fn(x));';
	const ctx = makeContext(ours, prettier, 'svelte');
	const match = runPattern('jsdoc_type_cast_parens', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'jsdoc_type_cast_parens');
});
