/**
 * Synthetic tests for divergence detection patterns.
 *
 * Each pattern gets:
 * - Positive test: minimal synthetic diff that SHOULD trigger the pattern
 * - Negative test: similar-looking diff that should NOT trigger
 *
 * This catches overmatching — patterns incorrectly claiming hunks.
 */

import { deepStrictEqual as assertEquals, notDeepStrictEqual as assertNotEquals } from 'node:assert';
import { diff_lines, extract_hunks } from '../diff.ts';
import {
	type DetectionContext,
	type DivergenceMatch,
	detect_divergences,
	enrich_detection_context,
	PATTERNS,
} from './patterns.ts';
import type { Language } from '../types.ts';

/**
 * Create a DetectionContext from ours/prettier strings.
 * Generates diff and hunks automatically.
 */
function make_context(
	ours: string,
	prettier: string,
	language: Language = 'svelte',
): DetectionContext {
	const diff = diff_lines(prettier, ours);
	const hunks = extract_hunks(diff);
	const ctx: DetectionContext = {
		source: prettier, // simplification: source ≈ prettier for detection
		ours,
		prettier,
		diff,
		hunks,
		language,
	};
	enrich_detection_context(ctx);
	return ctx;
}

/** Calculate visual width of a line (tabs = 2 spaces). */
function visual_width(line: string): number {
	let width = 0;
	for (const char of line) {
		width += char === '\t' ? 2 : 1;
	}
	return width;
}

/** Run a single pattern's detect() on a context. */
function run_pattern(pattern_id: string, ctx: DetectionContext): DivergenceMatch | null {
	const pattern = PATTERNS.find((p) => p.id === pattern_id);
	if (!pattern) throw new Error(`Unknown pattern: ${pattern_id}`);
	return pattern.detect(ctx);
}

// ─── template_literal_width ─────────────────────────────────────────────────

Deno.test('template_literal_width: positive - }` closing on own line', () => {
	// Prettier keeps ${expr} on one line (> 80 chars), we break to separate lines.
	// The diff has a proper removed/added hunk with prettier's long line visible.
	const padding = 'x'.repeat(70);
	const prettier = `\tconst msg = \`${padding} \${expr}\`;`;
	const ours = `\tconst msg = \`${padding} \${\n\t\texpr\n\t}\`;`;
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = `const msg = \`${padding} \${expr}\`;`;
	const match = run_pattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: positive - ${ at end of line in ours', () => {
	// We break after ${ (end of line), prettier keeps ${expr} inline.
	// Prettier line must be > 80 chars to confirm width-motivated break.
	const prettier =
		'\t\tconsole.error(\n\t\t\t`message content is "${VALUE}" and the result value got "${fixture.prop}" with extra text`,\n\t\t);';
	const ours =
		'\t\tconsole.error(`message content is "${VALUE}" and the result value got "${\n\t\t\t\tfixture.prop\n\t\t\t}" with extra text`);\n';
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source =
		'console.error(`message content is "${VALUE}" and the result value got "${fixture.prop}" with extra text`);';
	const match = run_pattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: negative - inline ${ on both sides', () => {
	// Both sides have ${expr} inline (not at end of line) — not a template break divergence
	const prettier = '\t\tconsole.log(`hello ${name}`);\n\t\tconsole.log(`bye ${name}`);';
	const ours = '\t\tconsole.log(`hello ${name}`);\n\t\tconsole.log(`goodbye ${name}`);';
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = 'console.log(`hello ${name}`);';
	const match = run_pattern('template_literal_width', ctx);
	assertEquals(match, null);
});

Deno.test('template_literal_width: negative - no ${ in source', () => {
	const prettier = 'const x = someVeryLongVariableName.property.method();';
	const ours = 'const x =\n\tsomeVeryLongVariableName.property.method();';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('template_literal_width', ctx);
	assertEquals(match, null);
});

Deno.test('template_literal_width: negative - both sides have ${ breaks', () => {
	const prettier = 'const x = `hello ${\n\tname\n} world`;';
	const ours = 'const x = `hello ${\n\tname\n} world`;';
	const ctx = make_context(ours, prettier, 'typescript');
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
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = 'return `<span style="--indent: ${indent}ch">${line ?? \'\'}</span>`;';
	const match = run_pattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: positive - atomization divergence (more breaks in ours)', () => {
	// Prettier keeps ${spec.method} inline (atomized), we break it.
	// Both sides end with ${ but ours has an additional break.
	const prettier = '\t\t\treturn `${spec.method}: (${';
	const ours = '\t\t\treturn `${\n\t\t\t\tspec.method\n\t\t\t}: (${';
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = 'return `${spec.method}: (${...';
	const match = run_pattern('template_literal_width', ctx);
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
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source =
		'`response.status is not a number. value: "${response.status}" type: ${typeof response.status}`,';
	const match = run_pattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: positive - nested template/array in interpolation breaks', () => {
	// Prettier keeps the nested `${[`…`]}` construct inline past print width; ours
	// breaks the inner array bracket. The end-of-line `${` / `}`` markers never
	// appear — Case 3 (nested-template shape + re-wrap guard) claims it.
	const e = 'e'.repeat(64);
	const prettier = `\t\t\t\t\t\t\t\t\t\t\t\t\te: '\${[\`\${${e}}\`]}',`;
	const ours =
		`\t\t\t\t\t\t\t\t\t\t\t\t\te: '\${[\n\t\t\t\t\t\t\t\t\t\t\t\t\t\t\`\${${e}}\`,\n\t\t\t\t\t\t\t\t\t\t\t\t\t]}',`;
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = `e: '\${[\`\${${e}}\`]}',`;
	const vw = visual_width(prettier);
	assertEquals(vw > 100, true, `Expected prettier line > 100, got ${vw}`);
	const match = run_pattern('template_literal_width', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'template_literal_width');
});

Deno.test('template_literal_width: negative - nested template line, ours did NOT re-wrap', () => {
	// A wide nested-template interpolation line, but ours emitted the same long
	// line (a different in-place edit — no legitimate break). The re-wrap guard in
	// Case 3 (more added than removed lines) must reject — claiming would mask a
	// formatting bug purely from prettier's width.
	const e = 'e'.repeat(64);
	const prettier = `\t\t\t\t\t\t\t\t\t\t\t\t\te: '\${[\`\${${e}}\`]}',`;
	const ours = `\t\t\t\t\t\t\t\t\t\t\t\t\tE: '\${[\`\${${e}}\`]}',`;
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = `e: '\${[\`\${${e}}\`]}',`;
	const match = run_pattern('template_literal_width', ctx);
	assertEquals(match, null);
});

Deno.test('template_literal_width: negative - both sides break same complex expr', () => {
	// Both sides break at ${} but with complex expressions (not atomizable)
	// — not a template atomization divergence
	const prettier = '\t\tconst x = `${\n\t\t\tfoo() + bar()\n\t\t}`;';
	const ours = '\t\tconst x = `${\n\t\t\tfoo() +\n\t\t\tbar()\n\t\t}`;';
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = 'const x = `${foo() + bar()}`;';
	const match = run_pattern('template_literal_width', ctx);
	assertEquals(match, null);
});

// ─── block_expression_logical ───────────────────────────────────────────────

Deno.test('block_expression_logical: positive - && at start of line in ours', () => {
	const prettier = '{#if someCondition && anotherCondition && thirdCondition}content{/if}';
	const ours = '{#if someCondition\n\t&& anotherCondition\n\t&& thirdCondition}content{/if}';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('block_expression_logical', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'block_expression_logical');
});

Deno.test('block_expression_logical: negative - not svelte', () => {
	const prettier = 'if (someCondition && anotherCondition) {}';
	const ours = 'if (someCondition\n\t&& anotherCondition) {}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('block_expression_logical', ctx);
	assertEquals(match, null);
});

Deno.test('block_expression_logical: negative - both sides have operator breaks', () => {
	const prettier = '{#if a\n\t&& b}content{/if}';
	const ours = '{#if a\n\t&& b}content{/if}';
	const ctx = make_context(ours, prettier, 'svelte');
	assertEquals(ctx.hunks.length, 0);
});

// ─── fill_101_boundary ──────────────────────────────────────────────────────

Deno.test('fill_101_boundary: positive - prettier > 100 chars, we break', () => {
	// Simulate a 105-char prettier line that we break
	const longLine = '\t' + 'x'.repeat(103); // visual width = 2 + 103 = 105
	const prettier = `before\n${longLine}\nafter`;
	const ours = `before\n\t${'x'.repeat(50)}\n\t${'x'.repeat(53)}\nafter`;
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_101_boundary', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'fill_101_boundary');
});

Deno.test('fill_101_boundary: negative - prettier lines under 100 chars', () => {
	const prettier = 'before\n\t' + 'x'.repeat(90) + '\nafter';
	const ours = 'before\n\t' + 'x'.repeat(45) + '\n\t' + 'x'.repeat(45) + '\nafter';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_101_boundary', ctx);
	assertEquals(match, null);
});

Deno.test('fill_101_boundary: positive - same line count, different wrapping at print width', () => {
	// Prettier has 105-char line, we rewrap to 3 lines all ≤ 100 chars (same total line count)
	const prettierLine = '\t' + 'x'.repeat(50) + ' ' + 'y'.repeat(52); // visual width = 2 + 50 + 1 + 52 = 105
	const prettier = `before\n${prettierLine}\nyy\nafter`;
	// Same 3 content lines, but we break differently (all ≤ 100)
	const ours = `before\n\t${'x'.repeat(50)}\n\t${'y'.repeat(52)} yy\nafter`;
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_101_boundary', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'fill_101_boundary');
});

Deno.test('fill_101_boundary: negative - our lines also exceed 100 chars', () => {
	// Both sides have lines > 100 chars — not a print-width boundary divergence
	const longLine = '\t' + 'x'.repeat(103); // visual width = 105
	const prettier = `before\n${longLine}\nafter`;
	const ours = `before\n${longLine}\n\textra\nafter`;
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_101_boundary', ctx);
	assertEquals(match, null);
});

Deno.test('fill_101_boundary: negative - we have fewer lines (not a break)', () => {
	const longLine = '\t' + 'x'.repeat(103);
	const prettier = `before\n${longLine}\n${'x'.repeat(50)}\nafter`;
	// We have FEWER lines, not more - not a break scenario
	const ours = `before\n${longLine}\nafter`;
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_101_boundary', ctx);
	assertEquals(match, null);
});

// ─── menu_block ─────────────────────────────────────────────────────────────

Deno.test('menu_block: positive - prettier hugs menu content, we expand', () => {
	const prettier = '<menu\n\tdata-attr1="value1"\n\tdata-attr2="value2">{@render fn()}</menu\n>';
	const ours = '<menu\n\tdata-attr1="value1"\n\tdata-attr2="value2"\n>\n\t{@render fn()}\n</menu>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('menu_block', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'menu_block');
	assertEquals(match!.confidence, 'certain');
});

Deno.test('menu_block: negative - not svelte', () => {
	const prettier = '<menu\n\tclass="nav">{@render fn()}</menu\n>';
	const ours = '<menu\n\tclass="nav"\n>\n\t{@render fn()}\n</menu>';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('menu_block', ctx);
	assertEquals(match, null);
});

Deno.test('menu_block: negative - not a menu element', () => {
	const prettier = '<div\n\tclass="nav">content</div>';
	const ours = '<div\n\tclass="nav"\n>\n\tcontent\n</div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('menu_block', ctx);
	assertEquals(match, null);
});

// ─── inline_content_hug ─────────────────────────────────────────────────────

Deno.test('inline_content_hug: positive - we hug >{, prettier breaks >', () => {
	const prettier = '<span\n\tclass="long"\n>\n\t{content}\n</span>';
	const ours = '<span\n\tclass="long"\n>{content}\n</span>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('inline_content_hug', ctx);
	assertNotEquals(match, null);
});

Deno.test('inline_content_hug: positive - prettier >content on same line, we break ternary', () => {
	const prettier =
		"<small\n\t>no action history{show ? '' : ', showing only write actions'}</small";
	const ours = "<small>no action history{show\n\t? ''\n\t: ', showing only write actions'}</small";
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('inline_content_hug', ctx);
	assertNotEquals(match, null);
});

Deno.test('inline_content_hug: negative - not svelte', () => {
	const prettier = '<span\n\tclass="long"\n>\n\t{content}\n</span>';
	const ours = '<span\n\tclass="long"\n>{content}\n</span>';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('inline_content_hug', ctx);
	assertEquals(match, null);
});

// ─── inline_content_block_style ─────────────────────────────────────────────

Deno.test('inline_content_block_style: positive - ours block-style, prettier dangles closing >', () => {
	// prettier hugs content to the attr line and dangles the closing `>`; ours keeps
	// both tags intact with the content on its own indented line. Whitespace-only.
	const prettier = '<code\n\tclass="x">PUBLIC</code\n>';
	const ours = '<code\n\tclass="x"\n>\n\tPUBLIC\n</code>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('inline_content_block_style', ctx);
	assertNotEquals(match, null);
});

Deno.test('inline_content_block_style: positive - ours dangles the open > to its own line', () => {
	const prettier = '<span class="x">{content}</span>';
	const ours = '<span\n\tclass="x"\n>{content}</span\n>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('inline_content_block_style', ctx);
	assertNotEquals(match, null);
});

Deno.test('inline_content_block_style: negative - not svelte', () => {
	const prettier = '<code\n\tclass="x">PUBLIC</code\n>';
	const ours = '<code\n\tclass="x"\n>\n\tPUBLIC\n</code>';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('inline_content_block_style', ctx);
	assertEquals(match, null);
});

Deno.test('inline_content_block_style: negative - format-ignore verbatim (whitespace-only, no tag dangle)', () => {
	// tags stay intact and in place; only redundant internal spaces differ. No
	// dangled delimiter → must NOT be claimed (it is a format-ignore divergence).
	const prettier = '<div class="a">text</div>';
	const ours = '<div    class="a"    >text</div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('inline_content_block_style', ctx);
	assertEquals(match, null);
});

Deno.test('inline_content_block_style: negative - empty-destructure brace spacing (no tag dangle)', () => {
	const prettier = '{#each items as { }}<div>x</div>{/each}';
	const ours = '{#each items as {}}<div>x</div>{/each}';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('inline_content_block_style', ctx);
	assertEquals(match, null);
});

Deno.test('inline_content_block_style: positive - block body dropped to its own line', () => {
	// prettier hugs the block body onto the head line; tsv drops it, leaving the
	// `{#snippet …}` head alone on its own line. Whitespace-only.
	const prettier = '{#snippet foo()}{#if x}<A />{/if}{/snippet}';
	const ours = '{#snippet foo()}\n\t{#if x}<A />{/if}\n{/snippet}';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('inline_content_block_style', ctx);
	assertNotEquals(match, null);
});

Deno.test('inline_content_block_style: negative - block head not isolated (hugged in ours)', () => {
	// A `{#if}` head that stays hugged inside an element on ours side is not a
	// dropped-body signature (no head-alone line, no tag dangle).
	const prettier = '<div>\n\t{#if c}text{/if}\n</div>';
	const ours = '<div>{#if c}text{/if}</div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('inline_content_block_style', ctx);
	assertEquals(match, null);
});

Deno.test('inline_content_block_style: negative - content differs (safety gate blocks a real loss)', () => {
	// Same block-style/dangle shape, but the text content itself differs — the
	// whitespace-only gate fails so the detector can never absorb a content change.
	const prettier = '<code\n\tclass="x">GOODBYE</code\n>';
	const ours = '<code\n\tclass="x"\n>\n\tHELLO\n</code>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('inline_content_block_style', ctx);
	assertEquals(match, null);
});

// ─── single_specifier_import ────────────────────────────────────────────────

Deno.test('single_specifier_import: positive - long import wraps', () => {
	const prettier =
		'import { someVeryLongExportedNameThatExceedsPrintWidthWhenCombinedWithTheModulePath } from "./some/very/long/module/path";';
	const ours =
		'import {\n\tsomeVeryLongExportedNameThatExceedsPrintWidthWhenCombinedWithTheModulePath,\n} from "./some/very/long/module/path";';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('single_specifier_import', ctx);
	assertNotEquals(match, null);
});

Deno.test('single_specifier_import: negative - import under 100 chars', () => {
	const prettier = 'import { foo } from "./bar";';
	const ours = 'import {\n\tfoo,\n} from "./bar";';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('single_specifier_import', ctx);
	assertEquals(match, null);
});

Deno.test('single_specifier_import: positive - tab-indented import in <script> wraps', () => {
	// Imports inside a Svelte <script> are tab-indented; the leading-tab allowance
	// lets the detector see the broken opener and the long inline import.
	const longSpec = 'a'.repeat(80);
	const prettier = `\timport {${longSpec}} from './mod';`;
	const ours = `\timport {\n\t\t${longSpec},\n\t} from './mod';`;
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('single_specifier_import', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'single_specifier_import');
});

Deno.test('single_specifier_import: negative - long non-import line (no import opener)', () => {
	// A long line over 100 chars that is NOT an import, broken by ours. The
	// import-specific opener/inline regexes must not claim it.
	const longCall = 'a'.repeat(80);
	const prettier = `\tconst x = someFunction(${longCall});`;
	const ours = `\tconst x = someFunction(\n\t\t${longCall},\n\t);`;
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('single_specifier_import', ctx);
	assertEquals(match, null);
});

Deno.test('single_specifier_import: positive - cross-hunk split by a consecutive import', () => {
	// A following import makes the LCS split the long inline line and ours' broken
	// `import {` opener into separate hunks; path-keyed matching still claims it.
	const longSpec = 'describe_identity_parity_cross_tests_with_a_long_enough_name_to_overflow';
	const prettier =
		`import { ${longSpec} } from './cross_backend/identity_parity.ts';\n` +
		`import { create_specs, endpoints } from './spine.ts';`;
	const ours =
		`import {\n\t${longSpec}\n} from './cross_backend/identity_parity.ts';\n` +
		`import { create_specs, endpoints } from './spine.ts';`;
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('single_specifier_import', ctx);
	assertNotEquals(match, null);
});

Deno.test('single_specifier_import: negative - long import ours did NOT re-wrap', () => {
	// Prettier's import is long (> 100), but ours did NOT break it into the
	// multiline opener form — it edited the line in place (e.g. renamed the
	// source). Without an `import {`-on-its-own-line opener in our added lines,
	// the divergence (ours wraps) is not present → reject.
	const longSpec = 'a'.repeat(74);
	const prettier = `import {${longSpec}} from './mod';`;
	const ours = `import {${longSpec}} from './other';`;
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('single_specifier_import', ctx);
	assertEquals(match, null);
});

// ─── self_closing_nonvoid ───────────────────────────────────────────────────

Deno.test('self_closing_nonvoid: positive - self-closing component', () => {
	const prettier = '<Component></Component>';
	const ours = '<Component />';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('self_closing_nonvoid', ctx);
	assertNotEquals(match, null);
});

Deno.test('self_closing_nonvoid: negative - not svelte', () => {
	const prettier = '<Component></Component>';
	const ours = '<Component />';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('self_closing_nonvoid', ctx);
	assertEquals(match, null);
});

Deno.test('self_closing_nonvoid: positive - HTML element ours expands self-closing', () => {
	const prettier = '<div />';
	const ours = '<div></div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('self_closing_nonvoid', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'self_closing_nonvoid');
});

Deno.test('self_closing_nonvoid: positive - split hunks from identical intervening line', () => {
	// When <div /> → <div></div> has an identical <div></div> between them,
	// the diff splits into two hunks (one remove-only, one add-only)
	const prettier = '<div />\n<div></div>';
	const ours = '<div></div>\n<div></div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('self_closing_nonvoid', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.hunk_indices.length, 2);
});

Deno.test('self_closing_nonvoid: positive - multiline element /> vs ></div>', () => {
	const prettier = '  data-my-prop\n/>';
	const ours = '  data-my-prop\n></div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('self_closing_nonvoid', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'self_closing_nonvoid');
});

Deno.test('self_closing_nonvoid: negative - different elements in wrapping diff', () => {
	// Self-closing <Glyph /> is same in both outputs (just rewrapped),
	// </ProviderLink> is an unrelated close tag also rewrapped
	const prettier = '><span><Glyph glyph={GLYPH} /> text</span\n> provider</ProviderLink';
	const ours = '><span><Glyph glyph={GLYPH} /> text</span> provider</ProviderLink';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('self_closing_nonvoid', ctx);
	assertEquals(match, null);
});

Deno.test('self_closing_nonvoid: negative - dotted component name regex escape', () => {
	// <X.Y /> (member expression component) self-closes in ours,
	// </X_Y> is a different component's close tag in prettier.
	// Without escaping `.` in the regex, `X.Y` matches `X_Y`
	// because `.` is a regex wildcard — false positive.
	const prettier = '</X_Y>';
	const ours = '<X.Y />';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('self_closing_nonvoid', ctx);
	assertEquals(match, null);
});

// ─── bom_strip ──────────────────────────────────────────────────────────────

Deno.test('bom_strip: positive - BOM in source, we strip', () => {
	const prettier = '\ufeffconst x = 1;';
	const ours = 'const x = 1;';
	const ctx = make_context(ours, prettier, 'typescript');
	// BOM needs to be in source
	ctx.source = '\ufeffconst x = 1;';
	const match = run_pattern('bom_strip', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.confidence, 'certain');
});

Deno.test('bom_strip: negative - no BOM in source', () => {
	const prettier = 'const x = 1;';
	const ours = 'const y = 1;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('bom_strip', ctx);
	assertEquals(match, null);
});

// ─── fill_after_inline ──────────────────────────────────────────────────────

Deno.test('fill_after_inline: positive - long line with inline close tag', () => {
	const longContent = 'x'.repeat(90);
	const prettier = `\t<p>Some text <span>${longContent}</span> more text after the inline</p>`;
	const ours = `\t<p>Some text <span>${longContent}</span>\n\tmore text after the inline</p>`;
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_after_inline', ctx);
	assertNotEquals(match, null);
});

Deno.test('fill_after_inline: negative - inline close tag under 100 chars', () => {
	const prettier = '\t<p>Some text <span>short</span> more text</p>';
	const ours = '\t<p>Some text <span>short</span>\n\tmore text</p>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_after_inline', ctx);
	assertEquals(match, null);
});

Deno.test('fill_after_inline: negative - not svelte', () => {
	const longContent = 'x'.repeat(90);
	const prettier = `\t<p>Some text <span>${longContent}</span> more text</p>`;
	const ours = `\t<p>Some text <span>${longContent}</span>\n\tmore text</p>`;
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('fill_after_inline', ctx);
	assertEquals(match, null);
});

// ─── css_value_wrap ─────────────────────────────────────────────────────────

Deno.test('css_value_wrap: positive - long CSS property value', () => {
	const longValue = 'var(--a) ' + 'color-mix(in srgb, red, blue) '.repeat(3);
	const prettier = `\tbox-shadow: ${longValue};`;
	const ours = `\tbox-shadow:\n\t\t${longValue.split(' ').slice(0, 3).join(' ')}\n\t\t${
		longValue.split(' ').slice(3).join(' ')
	};`;
	const ctx = make_context(ours, prettier, 'css');
	const match = run_pattern('css_value_wrap', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_value_wrap: negative - short CSS property value', () => {
	const prettier = '\tcolor: red;';
	const ours = '\tcolor: blue;';
	const ctx = make_context(ours, prettier, 'css');
	const match = run_pattern('css_value_wrap', ctx);
	assertEquals(match, null);
});

// ─── member_expression_call ─────────────────────────────────────────────────

Deno.test('member_expression_call: positive - require.resolve in diff', () => {
	const prettier = 'const p = require.resolve.paths("some/very/long/module/path/that/exceeds");';
	const ours = 'const p = require.resolve.paths(\n\t"some/very/long/module/path/that/exceeds",\n);';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('member_expression_call', ctx);
	assertNotEquals(match, null);
});

Deno.test('member_expression_call: negative - no module pattern in source', () => {
	const prettier = 'const p = something.other("path");';
	const ours = 'const p = something.other(\n\t"path",\n);';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('member_expression_call', ctx);
	assertEquals(match, null);
});

// ─── comment_position ───────────────────────────────────────────────────────

Deno.test('comment_position: positive - comment moved to different line', () => {
	// Prettier puts comment after {, we put it on its own line
	// The comment text "todo" is the same, just on different lines
	const prettier = 'for (let i = 0; i < n; i++) // todo\n{\n\tx++;\n}';
	const ours = 'for (let i = 0; i < n; i++)\n\t// todo\n{\n\tx++;\n}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertNotEquals(match, null);
});

Deno.test('comment_position: negative - identical comments same position', () => {
	const prettier = 'const x = 1; // comment\nconst y = 2;';
	const ours = 'const x = 1; // comment\nconst y = 2;';
	const ctx = make_context(ours, prettier, 'typescript');
	assertEquals(ctx.hunks.length, 0);
});

Deno.test('comment_position: negative - comment incidental to code layout change', () => {
	// Non-comment code differs (return vs return + paren wrapping) — comment is incidental
	const prettier = 'return (\n\tstr\n\t\t// replace\n\t\t.replace(/a/g, "-")\n);';
	const ours = 'return str\n\t// replace\n\t.replace(/a/g, "-");';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertEquals(match, null);
});

Deno.test('comment_position: negative - Case 1 comment not in other side output', () => {
	// Comment on one side only, and the comment text doesn't exist in the other output
	const prettier = 'const x = 1;\nconst y = 2;';
	const ours = 'const x = 1;\n// added comment\nconst y = 2;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertEquals(match, null);
});

Deno.test('comment_position: positive - Case 1 comment moved out of hunk region', () => {
	// Comment on one side of hunk, but text exists in other side's full output (moved)
	const prettier = '// todo\nconst x = 1;\nconst y = 2;';
	const ours = 'const x = 1;\n// todo\nconst y = 2;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertNotEquals(match, null);
});

Deno.test('comment_position: positive - comment absorbed into block body', () => {
	// Prettier absorbs comment between ) and {} into the block body, reformatting the block
	const prettier = 'while (a) {\n\t/* comment */\n}';
	const ours = 'while (a) /* comment */ {}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertNotEquals(match, null);
});

Deno.test('comment_position: positive - line comment absorbed into try block', () => {
	// Prettier absorbs line comment between try and { into block body
	const prettier = 'try {\n\t// comment\n} catch (e) {}';
	const ours = 'try // comment\n{\n} catch (e) {}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertNotEquals(match, null);
});

Deno.test('comment_position: positive - comment absorbed into catch parens', () => {
	// Prettier absorbs line comment after catch (e) into parens, reformatting to multiline
	const prettier = '} catch (\n\te // comment\n) {}';
	const ours = '} catch (e) // comment\n{}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertNotEquals(match, null);
});

Deno.test('comment_position: positive - Case 3 structural relocation around bordering comment', () => {
	// Empty-switch shape: the `// note` comment is byte-identical in both outputs,
	// so the diff aligns it as a CONTEXT line; the hunk carries only the
	// discriminant-paren reshape (`switch (x) {` ↔ `switch (\n\tx\n) {`). The
	// comment borders the hunk in BOTH outputs and exists in both → Case 3 claims.
	const prettier = '\tswitch (\n\t\tx\n\t\t// note\n\t) {\n\t}';
	const ours = '\tswitch (x) {\n\t\t// note\n\t}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'comment_position');
});

Deno.test('comment_position: negative - Case 3 bordering comment DROPPED by prettier', () => {
	// Same structural reshape, but prettier DROPPED the `// note` comment entirely
	// (a content loss). The "exists as a whole comment line in BOTH outputs" guard
	// must reject — claiming would mask the drop as a known divergence and, in
	// corpus_compare_format, downgrade a real SAFETY data-loss to `known`.
	const prettier = '\tswitch (\n\t\tx\n\t) {\n\t}';
	const ours = '\tswitch (x) {\n\t\t// note\n\t}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertEquals(match, null);
});

Deno.test('comment_position: negative - Case 3 bordering comment CHANGED, not relocated', () => {
	// The comment bordering the hunk differs between the two outputs (`// note` vs
	// `// other`) — the comment text was altered, not merely repositioned. The
	// border-text-must-match-in-both guard must reject.
	const prettier = '\tswitch (\n\t\tx\n\t\t// other\n\t) {\n\t}';
	const ours = '\tswitch (x) {\n\t\t// note\n\t}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertEquals(match, null);
});

Deno.test('comment_position: negative - Case 3 preserved comment not bordering the hunk', () => {
	// A preserved comment exists in both outputs but is NOT the immediate border of
	// the structural hunk (a blank line and other code separate them). Case 3 must
	// not claim a structural reshape just because some preserved comment lives
	// elsewhere in the file.
	const prettier = '\t// faraway\n\tconst z = 1;\n\n\tswitch (\n\t\tx\n\t) {\n\t}';
	const ours = '\t// faraway\n\tconst z = 1;\n\n\tswitch (x) {\n\t}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertEquals(match, null);
});

// ─── block_multiline_attrs_hug ──────────────────────────────────────────────

Deno.test('block_multiline_attrs_hug: positive - > on own line with pre context', () => {
	// prettier: <textarea with attr line ending in "> (removed_hugs_gt matches /['"]\s*>/)
	// ours: > on its own line (added_breaks_gt matches /^\t*>$/)
	// The <textarea tag must be in the hunk's ours/prettier line range or context lines
	// Putting the entire element in a single diff hunk by making all lines different
	const prettier = '<textarea class="code" id="x">content</textarea>';
	const ours = '<textarea\n\tclass="code"\n\tid="x"\n>\ncontent</textarea>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('block_multiline_attrs_hug', ctx);
	assertNotEquals(match, null);
});

Deno.test('block_multiline_attrs_hug: negative - not svelte', () => {
	const prettier = '<pre\n\tclass="code"\n\tdata-lang="js">content</pre>';
	const ours = '<pre\n\tclass="code"\n\tdata-lang="js"\n>content</pre>';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('block_multiline_attrs_hug', ctx);
	assertEquals(match, null);
});

Deno.test('block_multiline_attrs_hug: negative - not ws-sensitive element', () => {
	const prettier = '<div\n\tclass="long"\n\tdata-x="y">content</div>';
	const ours = '<div\n\tclass="long"\n\tdata-x="y"\n>content</div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('block_multiline_attrs_hug', ctx);
	assertEquals(match, null);
});

// ─── short_expr_100 ─────────────────────────────────────────────────────────

Deno.test('short_expr_100: positive - block expr slightly over 100', () => {
	// Create a block expression that's 105 chars in prettier (within 100-110 range)
	const expr = 'a'.repeat(65);
	const prettier = `{#if typeof ${expr} === 'string'}content{/if}`;
	const ours = `{#if\n\ttypeof ${expr} === 'string'\n}content{/if}`;
	const ctx = make_context(ours, prettier, 'svelte');
	const vw = visual_width(prettier);
	assertEquals(vw > 100 && vw <= 110, true, `Expected visual width 101-110, got ${vw}`);
	const match = run_pattern('short_expr_100', ctx);
	assertNotEquals(match, null);
});

Deno.test('short_expr_100: negative - not svelte', () => {
	const prettier = '{#if typeof x === "string"}content{/if}';
	const ours = '{#if\n\ttypeof x === "string"\n}content{/if}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('short_expr_100', ctx);
	assertEquals(match, null);
});

// ─── css_unit_serialize_case ────────────────────────────────────────────────

Deno.test('css_unit_serialize_case: positive - Hz/kHz/Q lowercased in .css', () => {
	const prettier = 'a {\n\tpitch: 440Hz;\n\tpitch: 1kHz;\n\tleft: 10Q;\n}';
	const ours = 'a {\n\tpitch: 440hz;\n\tpitch: 1khz;\n\tleft: 10q;\n}';
	const ctx = make_context(ours, prettier, 'css');
	const match = run_pattern('css_unit_serialize_case', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.confidence, 'certain');
});

Deno.test('css_unit_serialize_case: positive - inside a Svelte <style> block', () => {
	const prettier = '<style>\n\ta {\n\t\tpitch: 440Hz;\n\t}\n</style>';
	const ours = '<style>\n\ta {\n\t\tpitch: 440hz;\n\t}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_unit_serialize_case', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_unit_serialize_case: negative - reverse direction (ours upcases) is NOT matched', () => {
	// A hypothetical bug where OURS upcases must not be excused — the pattern is
	// direction-specific (prettier-upcases / ours-lowercases only).
	const prettier = 'a {\n\tpitch: 440hz;\n}';
	const ours = 'a {\n\tpitch: 440Hz;\n}';
	const ctx = make_context(ours, prettier, 'css');
	const match = run_pattern('css_unit_serialize_case', ctx);
	assertEquals(match, null);
});

Deno.test('css_unit_serialize_case: negative - unrelated value change', () => {
	const prettier = 'a {\n\tcolor: red;\n}';
	const ours = 'a {\n\tcolor: blue;\n}';
	const ctx = make_context(ours, prettier, 'css');
	const match = run_pattern('css_unit_serialize_case', ctx);
	assertEquals(match, null);
});

// ─── css_atrule_spec_spacing ────────────────────────────────────────────────

Deno.test('css_atrule_spec_spacing: positive - missing space after and(', () => {
	const prettier =
		'<style>\n@container (min-width: 700px) and(min-height: 500px) {\n\tdiv { color: red; }\n}\n</style>';
	const ours =
		'<style>\n@container (min-width: 700px) and (min-height: 500px) {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_atrule_spec_spacing', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.confidence, 'certain');
});

Deno.test('css_atrule_spec_spacing: negative - correct spacing already', () => {
	const prettier =
		'<style>\n@media screen and (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ours =
		'<style>\n@media screen and (min-width: 768px) {\n\tdiv { color: blue; }\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_atrule_spec_spacing', ctx);
	assertEquals(match, null);
});

// ─── css_atrule_long_wrap ───────────────────────────────────────────────────

Deno.test('css_atrule_long_wrap: positive - @media over 100 chars, we wrap', () => {
	const longQuery =
		'@media screen and (min-width: 768px) and (max-width: 1024px) and (orientation: landscape) and (color)';
	const prettier = `<style>\n${longQuery} {\n\tdiv { color: red; }\n}\n</style>`;
	const ours =
		`<style>\n@media screen and (min-width: 768px) and (max-width: 1024px)\n\tand (orientation: landscape) and (color) {\n\tdiv { color: red; }\n}\n</style>`;
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_atrule_long_wrap', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_atrule_long_wrap: negative - @media under 100 chars', () => {
	const prettier =
		'<style>\n@media screen and (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ours =
		'<style>\n@media screen\n\tand (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_atrule_long_wrap', ctx);
	assertEquals(match, null);
});

// ─── css_atrule_stable_quirk ────────────────────────────────────────────────

Deno.test('css_atrule_stable_quirk: positive - @layer with extra spaces', () => {
	const prettier = '<style>\n@layer base,  components;\n</style>';
	const ours = '<style>\n@layer base, components;\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_atrule_stable_quirk', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_atrule_stable_quirk: positive - @scope with spaces in parens', () => {
	const prettier =
		'<style>\n@scope ( .class1 )  to  ( .class2 ) {\n\timg { border: 1px; }\n}\n</style>';
	const ours = '<style>\n@scope (.class1) to (.class2) {\n\timg { border: 1px; }\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_atrule_stable_quirk', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_atrule_stable_quirk: negative - at-rule wrapping (different pattern)', () => {
	const prettier =
		'<style>\n@media screen and (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ours =
		'<style>\n@media screen\n\tand (min-width: 768px) {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_atrule_stable_quirk', ctx);
	assertEquals(match, null);
});

// ─── css_selector_divergence ────────────────────────────────────────────────

Deno.test('css_selector_divergence: positive - column combinator ||', () => {
	const prettier = '<style>\ncol.selected||td {\n\tcolor: red;\n}\n</style>';
	const ours = '<style>\ncol.selected || td {\n\tcolor: red;\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_selector_divergence', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_selector_divergence: positive - nth-child normalization', () => {
	const prettier = '<style>\nli:nth-child(3n- 2) {\n\tcolor: yellow;\n}\n</style>';
	const ours = '<style>\nli:nth-child(3n - 2) {\n\tcolor: yellow;\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_selector_divergence', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_selector_divergence: negative - || in TypeScript (logical OR)', () => {
	const prettier = 'const x = a || b;';
	const ours = 'const x = a ||\n\tb;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('css_selector_divergence', ctx);
	assertEquals(match, null);
});

Deno.test('css_selector_divergence: positive - nested pseudo-args re-indent (:is in :where)', () => {
	// prettier indents the :is() arg list one extra level (`nodes.length > 2`); tsv
	// keys on a real combinator, so the same list sits one tab shallower. Pure re-indent.
	const prettier = ':where(\n\t:is(\n\t\t\tp,\n\t\t\tul\n\t\t):not(:last-child)\n) {\n\tmargin: 0;\n}';
	const ours = ':where(\n\t:is(\n\t\tp,\n\t\tul\n\t):not(:last-child)\n) {\n\tmargin: 0;\n}';
	const ctx = make_context(ours, prettier, 'css');
	const match = run_pattern('css_selector_divergence', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_selector_divergence: negative - selector content actually differs (not pure re-indent)', () => {
	// Same shape, but a selector token changed (`ul` → `ol`): NOT a pure re-indent,
	// so the pseudo-args clause must not claim it (a real content difference).
	const prettier = ':where(\n\t:is(\n\t\t\tp,\n\t\t\tul\n\t):not(:last-child)\n) {\n\tmargin: 0;\n}';
	const ours = ':where(\n\t:is(\n\t\tp,\n\t\tol\n\t):not(:last-child)\n) {\n\tmargin: 0;\n}';
	const ctx = make_context(ours, prettier, 'css');
	const match = run_pattern('css_selector_divergence', ctx);
	assertEquals(match, null);
});

// ─── annotation_continuation_indent ─────────────────────────────────────────

Deno.test('annotation_continuation_indent: positive - type after colon line comment indents one level', () => {
	const prettier = 'const e: // c\nFoo = x;';
	const ours = 'const e: // c\n\tFoo = x;';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('annotation_continuation_indent', ctx);
	assertNotEquals(match, null);
});

Deno.test('annotation_continuation_indent: negative - ternary branch (line-leading colon)', () => {
	// A `:` at the start of its line is a ternary branch, not an annotation target;
	// requiring a word/closer before the colon excludes it.
	const prettier = 'const x = cond\n\t? a\n\t: // c\nb;';
	const ours = 'const x = cond\n\t? a\n\t: // c\n\t\tb;';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('annotation_continuation_indent', ctx);
	assertEquals(match, null);
});

Deno.test('annotation_continuation_indent: negative - pure re-indent with no preceding colon comment', () => {
	const prettier = 'foo(\n\tbar\n);';
	const ours = 'foo(\n\t\tbar\n);';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('annotation_continuation_indent', ctx);
	assertEquals(match, null);
});

// ─── css_value_ratio ────────────────────────────────────────────────────────

Deno.test('css_value_ratio: positive - ratio spacing in media query', () => {
	const prettier = '<style>\n@media (aspect-ratio: 16  /  9) {\n\tdiv { color: red; }\n}\n</style>';
	const ours = '<style>\n@media (aspect-ratio: 16 / 9) {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_value_ratio', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_value_ratio: negative - division in TypeScript', () => {
	const prettier = 'const x = 16  /  9;';
	const ours = 'const x = 16 / 9;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('css_value_ratio', ctx);
	assertEquals(match, null);
});

// ─── css_comment_stable_quirk ───────────────────────────────────────────────

Deno.test('css_comment_stable_quirk: positive - comment before { in at-rule', () => {
	const prettier = '<style>\n@media screen/* comment */ {\n\tdiv { color: red; }\n}\n</style>';
	const ours = '<style>\n@media screen /* comment */ {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_comment_stable_quirk', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_comment_stable_quirk: positive - CSS language', () => {
	const prettier = '@media screen/* comment */ {\n\tdiv { color: red; }\n}';
	const ours = '@media screen /* comment */ {\n\tdiv { color: red; }\n}';
	const ctx = make_context(ours, prettier, 'css');
	const match = run_pattern('css_comment_stable_quirk', ctx);
	assertNotEquals(match, null);
});

Deno.test('css_comment_stable_quirk: negative - TypeScript comment (not CSS)', () => {
	const prettier = 'if (x) /* comment */ {\n\tconsole.log(1);\n}';
	const ours = 'if (x) /* comment */\n{\n\tconsole.log(1);\n}';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('css_comment_stable_quirk', ctx);
	assertEquals(match, null);
});

// ─── empty_statement_removal ────────────────────────────────────────────────

Deno.test('empty_statement_removal: positive - standalone ; removed', () => {
	const prettier = '<script>\n;\n;\nconst x = 1;\n</script>';
	const ours = '<script>\nconst x = 1;\n</script>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('empty_statement_removal', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.confidence, 'certain');
});

Deno.test('empty_statement_removal: negative - semicolons in for(;;)', () => {
	const prettier = 'for (;;) {\n\tbreak;\n}';
	const ours = 'for (;;) {\n\tbreak;\n}';
	const ctx = make_context(ours, prettier, 'typescript');
	assertEquals(ctx.hunks.length, 0);
});

Deno.test('empty_statement_removal: negative - normal statement semicolons', () => {
	const prettier = 'const x = 1;\nconst y = 2;';
	const ours = 'const x = 1;\nconst y = 3;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('empty_statement_removal', ctx);
	assertEquals(match, null);
});

// ─── return_type_generic_union ──────────────────────────────────────────────

Deno.test('return_type_generic_union: positive - generic with | null wraps', () => {
	// Need prettier line > 100 chars with generic union
	const prettier =
		'function processWithVeryLongFunctionName(input: SomeVeryLongParameterType): Promise<VeryLongReturnTypeName | null> {';
	const ours =
		'function processWithVeryLongFunctionName(\n\tinput: SomeVeryLongParameterType,\n): Promise<\n\tVeryLongReturnTypeName | null\n> {';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('return_type_generic_union', ctx);
	assertNotEquals(match, null);
});

Deno.test('return_type_generic_union: negative - generic without union', () => {
	const prettier = 'function foo(): Promise<string> {';
	const ours = 'function foo(): Promise<\n\tstring\n> {';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('return_type_generic_union', ctx);
	assertEquals(match, null);
});

// ─── non_null_paren_base ────────────────────────────────────────────────────

Deno.test('non_null_paren_base: positive - tsv hangs parens, prettier hugs ))!.', () => {
	// Prettier hugs the inner call: `))!.ok` collapses on one line.
	const prettier =
		'\tconst ok = (await call(\n\t\tapplicationObjectLong,\n\t\tspecificationObjectLong,\n\t\thh,\n\t))!.ok;';
	// tsv hangs the outer parens: `)` on its own line, then `)!.ok;`.
	const ours =
		'\tconst ok = (\n\t\tawait call(\n\t\t\tapplicationObjectLong,\n\t\t\tspecificationObjectLong,\n\t\t\thh,\n\t\t)\n\t)!.ok;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('non_null_paren_base', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'non_null_paren_base');
});

Deno.test('non_null_paren_base: negative - no `!` (plain trailing member matches prettier)', () => {
	// Without the non-null `!`, prettier has no `))!.` hug and tsv emits `).ok` —
	// the detector must not claim a plain trailing-member break.
	const prettier =
		'\tconst ok = (await call(\n\t\tapplicationObjectLong,\n\t\tspecificationObjectLong,\n\t\thh,\n\t)).ok;';
	const ours = '\tconst ok = (await call(\n\t\tapplicationObjectLong,\n\t\thh,\n\t))\n\t\t.ok;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('non_null_paren_base', ctx);
	assertEquals(match, null);
});

// ─── instantiation_parens ─────────────────────────────────────────────────

Deno.test('instantiation_parens: positive - ternary parens stripped', () => {
	const prettier = '\tlet c = x ? y : z<T>;';
	const ours = '\tlet c = (x ? y : z)<T>;';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('instantiation_parens', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'instantiation_parens');
	assertEquals(match!.confidence, 'certain');
});

Deno.test('instantiation_parens: positive - binary parens stripped', () => {
	const prettier = '\tlet d = a + b<T>;';
	const ours = '\tlet d = (a + b)<T>;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('instantiation_parens', ctx);
	assertNotEquals(match, null);
});

Deno.test('instantiation_parens: negative - assignment parens (both agree)', () => {
	// Both formatters preserve parens for assignment — no diff
	const prettier = '\tlet a = (x = y)<T>;';
	const ours = '\tlet a = (x = y)<T>;';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('instantiation_parens', ctx);
	assertEquals(match, null);
});

Deno.test('instantiation_parens: positive - lowercase type param', () => {
	const prettier = '\tlet e = x ? y : z<string>;';
	const ours = '\tlet e = (x ? y : z)<string>;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('instantiation_parens', ctx);
	assertNotEquals(match, null);
});

Deno.test('instantiation_parens: negative - normal generic usage', () => {
	const prettier = '\tconst x = foo<T>();';
	const ours = '\tconst x = foo<T>();';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('instantiation_parens', ctx);
	assertEquals(match, null);
});

// ─── single_type_param_comma ──────────────────────────────────────────────

Deno.test('single_type_param_comma: positive - bare <T> vs prettier <T,>', () => {
	const prettier = '\tconst identity = <T,>(x: T) => x;';
	const ours = '\tconst identity = <T>(x: T) => x;';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('single_type_param_comma', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'single_type_param_comma');
	assertEquals(match!.confidence, 'certain');
});

Deno.test('single_type_param_comma: positive - const-modified <const T>', () => {
	const prettier = '\tconst arrow = <const T,>(x: T) => x;';
	const ours = '\tconst arrow = <const T>(x: T) => x;';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('single_type_param_comma', ctx);
	assertNotEquals(match, null);
});

Deno.test('single_type_param_comma: positive - default-only <T = string>', () => {
	const prettier = '\tconst withDefault = <T = string,>(x: T) => x;';
	const ours = '\tconst withDefault = <T = string>(x: T) => x;';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('single_type_param_comma', ctx);
	assertNotEquals(match, null);
});

Deno.test('single_type_param_comma: negative - not svelte (pure .ts agrees)', () => {
	// On the .ts path prettier strips the comma, so there is no divergence and the
	// language guard rejects regardless. Synthesize the diff to prove the guard fires.
	const prettier = '\tconst identity = <T,>(x: T) => x;';
	const ours = '\tconst identity = <T>(x: T) => x;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('single_type_param_comma', ctx);
	assertEquals(match, null);
});

Deno.test('single_type_param_comma: negative - multi-param <T, U> (interior comma)', () => {
	// `<T, U>` has an interior comma, so the `<...,>` single-param shape never matches.
	const prettier = '\tconst pair = <T, U>(a: T, b: U) => [a, b];';
	const ours = '\tconst pair = <T,U>(a: T, b: U) => [a, b];';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('single_type_param_comma', ctx);
	assertEquals(match, null);
});

Deno.test('single_type_param_comma: negative - both sides keep <T,> (no divergence)', () => {
	const prettier = '\tconst identity = <T,>(x: T) => x;';
	const ours = '\tconst identity = <T,>(x: T) => x;';
	const ctx = make_context(ours, prettier, 'svelte');
	assertEquals(ctx.hunks.length, 0);
	const match = run_pattern('single_type_param_comma', ctx);
	assertEquals(match, null);
});

// ─── block_comment_computed_member ────────────────────────────────────────

Deno.test('block_comment_computed_member: positive - JSDoc hoisted from brackets', () => {
	const prettier = '\t/** @type {string} */ obj.aaaa.bbbb.cccc?.[\n\t\td\n\t];';
	const ours = '\tobj.aaaa.bbbb.cccc?.[\n\t\t/** @type {string} */ d\n\t];';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('block_comment_computed_member', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'block_comment_computed_member');
	assertEquals(match!.confidence, 'certain');
});

Deno.test('block_comment_computed_member: negative - JSDoc in normal position', () => {
	const prettier = '\t/** @type {string} */ const x = 1;';
	const ours = '\t/** @type {string} */ const x = 1;';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('block_comment_computed_member', ctx);
	assertEquals(match, null);
});

Deno.test('block_comment_computed_member: positive - non-optional computed', () => {
	// Same pattern with non-optional computed member (still detects)
	const prettier = '\t/** @type {string} */ obj.aaaa.bbbb.cccc[\n\t\td\n\t];';
	const ours = '\tobj.aaaa.bbbb.cccc[\n\t\t/** @type {string} */ d\n\t];';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('block_comment_computed_member', ctx);
	assertNotEquals(match, null);
});

// ─── block_comment_chain ──────────────────────────────────────────────────

Deno.test('block_comment_chain: positive - comment spacing before dot differs', () => {
	// Prettier intermediate: `a/* inner */ .b` (space before dot)
	// Ours/stable: `a /* inner */.b` (no space before dot)
	const prettier = '\t/* outer */ a/* inner */ .b\n\t\t.c(a);';
	const ours = '\t/* outer */ a /* inner */.b\n\t\t.c(a);';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('block_comment_chain', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'block_comment_chain');
	assertEquals(match!.confidence, 'likely');
});

Deno.test('block_comment_chain: positive - deeper chain member', () => {
	const prettier = '\t/* outer */ a.b/* inner */ .c\n\t\t.d(a);';
	const ours = '\t/* outer */ a.b /* inner */.c\n\t\t.d(a);';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('block_comment_chain', ctx);
	assertNotEquals(match, null);
});

Deno.test('block_comment_chain: negative - identical comment spacing', () => {
	const prettier = '\t/* comment */ a.b\n\t\t.c(a);';
	const ours = '\t/* comment */ a.b\n\t\t.c(a);';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('block_comment_chain', ctx);
	assertEquals(match, null);
});

Deno.test('block_comment_chain: negative - CSS (not applicable)', () => {
	const prettier = '\t/* comment */ .class { color: red; }';
	const ours = '\t/* comment */.class { color: red; }';
	const ctx = make_context(ours, prettier, 'css');
	const match = run_pattern('block_comment_chain', ctx);
	assertEquals(match, null);
});

// ─── block_comment_computed_member (additional) ──────────────────────────

Deno.test('block_comment_computed_member: positive - regular block comment (not JSDoc)', () => {
	// Same hoisting behavior with /* */ instead of /** */
	const prettier = '\t/* cast */ obj.aaaa.bbbb.cccc?.[\n\t\td\n\t];';
	const ours = '\tobj.aaaa.bbbb.cccc?.[\n\t\t/* cast */ d\n\t];';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('block_comment_computed_member', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'block_comment_computed_member');
});

// ─── fill_after_inline (additional) ───────────────────────────────────────

Deno.test('fill_after_inline: positive - <mark> element (expanded inline list)', () => {
	const longContent = 'x'.repeat(90);
	const prettier = `\t<p>Text <mark>${longContent}</mark> trailing text after inline element</p>`;
	const ours = `\t<p>Text <mark>${longContent}</mark>\n\ttrailing text after inline element</p>`;
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_after_inline', ctx);
	assertNotEquals(match, null);
});

// ─── template_literal_width (additional) ──────────────────────────────────

Deno.test('template_literal_width: negative - short prettier line (not width-motivated)', () => {
	// Our side breaks at ${ but prettier's line is only ~40 chars.
	// Not width-motivated — likely a bug in our formatter.
	const prettier = '\tconst x = `hello ${name}`;';
	const ours = '\tconst x = `hello ${\n\t\tname\n\t}`;';
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = 'const x = `hello ${name}`;';
	const match = run_pattern('template_literal_width', ctx);
	assertEquals(match, null);
});

// ─── jsdoc_type_cast_parens ────────────────────────────────────────────────

Deno.test('jsdoc_type_cast_parens: positive - typescript (ours keeps, prettier-TS strips)', () => {
	// tsv preserves the cast parens (required for the assertion); prettier's
	// oxc-ts backend strips them in TS contexts.
	const prettier = '\tconst a = /** @type {A} */ document.activeElement;';
	const ours = '\tconst a = /** @type {A} */ (document.activeElement);';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('jsdoc_type_cast_parens', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'jsdoc_type_cast_parens');
});

Deno.test('jsdoc_type_cast_parens: negative - @satisfies without paren difference', () => {
	// Both sides have the same parens — no divergence
	const prettier = '\tconst a = /** @satisfies {A} */ (fn(x));';
	const ours = '\tconst a = /** @satisfies {A} */ (fn(x));';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('jsdoc_type_cast_parens', ctx);
	assertEquals(match, null);
});

Deno.test('jsdoc_type_cast_parens: negative - reverse direction (ours strips) not claimed', () => {
	// If OURS dropped the parens and prettier kept them, that would be a bug — the
	// detector must NOT claim it (claiming an ours-side strip masks a real loss).
	const prettier = '\tconst a = /** @type {A} */ (fn(x));';
	const ours = '\tconst a = /** @type {A} */ fn(x);';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('jsdoc_type_cast_parens', ctx);
	assertEquals(match, null);
});

// ─── member_expression_call (additional) ──────────────────────────────────

Deno.test('member_expression_call: positive - import.meta.resolve in diff', () => {
	const prettier = 'const p = import.meta.resolve("some/very/long/module/path/that/exceeds");';
	const ours = 'const p = import.meta.resolve(\n\t"some/very/long/module/path/that/exceeds",\n);';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('member_expression_call', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'member_expression_call');
});

// ─── block_expression_logical (additional) ────────────────────────────────

Deno.test('block_expression_logical: positive - || at start of line in ours', () => {
	const prettier = '{#if someCondition || anotherCondition || thirdCondition}content{/if}';
	const ours = '{#if someCondition\n\t|| anotherCondition\n\t|| thirdCondition}content{/if}';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('block_expression_logical', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'block_expression_logical');
});

// ─── css_atrule_spec_spacing (additional) ─────────────────────────────────

Deno.test('css_atrule_spec_spacing: positive - not( missing space', () => {
	const prettier = '<style>\n@media not(print) {\n\tdiv { color: red; }\n}\n</style>';
	const ours = '<style>\n@media not (print) {\n\tdiv { color: red; }\n}\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_atrule_spec_spacing', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.confidence, 'certain');
});

// ─── comment_position (additional) ────────────────────────────────────────

Deno.test('comment_position: negative - short comment text matches incidentally', () => {
	// Comment text "x" is very short (< 3 chars) and would match incidentally
	// in prettier's output (e.g., inside variable names). Should NOT claim.
	const prettier = 'const max = 1;\nconst y = 2;';
	const ours = 'const max = 1;\n// x\nconst y = 2;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertEquals(match, null);
});

Deno.test('comment_position: negative - comment text in code but not as comment', () => {
	// Comment "map" (3+ chars) exists in prettier output but only as a method
	// name (arr.map), NOT as a comment. Should NOT claim as moved comment.
	const prettier = 'const result = arr.map(x => x * 2);\nconst y = 2;';
	const ours = '// map\nconst result = arr.map(x => x * 2);\nconst y = 2;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertEquals(match, null);
});

Deno.test('comment_position: positive - retained-paren-union line-comment expansion', () => {
	// A preserved line comment inside a parenthesized union forces tsv to expand
	// the member to its broken leading-`|` form; Prettier keeps it inline and
	// relocates the comment. Only separator/paren layout differs (same members,
	// same comment text) — claim it via the union-layout fallback.
	const prettier = 'type T =\n\t| (A | B) // comment 1\n\t| D;';
	const ours = 'type T =\n\t| (\n\t\t| A\n\t\t| B // comment 1\n\t)\n\t| D;';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertNotEquals(match, null);
});

Deno.test('comment_position: negative - dropped union separator not masked by layout strip', () => {
	// Same identifiers and same comment, but ours DROPS a union `|` (a real
	// content loss). The separator-count guard (ours `|` count < prettier's) must
	// reject so the union-layout fallback never masks a dropped separator.
	const prettier = 'type T = A | B; // comment';
	const ours = 'type T = A B; // comment';
	const ctx = make_context(ours, prettier, 'typescript');
	const match = run_pattern('comment_position', ctx);
	assertEquals(match, null);
});

// ─── css_comment_stable_quirk (additional) ────────────────────────────────

Deno.test('css_comment_stable_quirk: negative - one-sided comment not in other output', () => {
	// Comment on our side only, but text doesn't exist in prettier's output.
	// Should NOT claim as a comment position divergence.
	const prettier = '<style>\n.a { color: red; }\n</style>';
	const ours = '<style>\n.a { /* new-unique-text */ color: red; }\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_comment_stable_quirk', ctx);
	assertEquals(match, null);
});

// ─── fill_101_boundary covers multiline_value_inline_long ─────────────────

Deno.test('fill_101_boundary: positive - multiline attr inline long text', () => {
	// Prettier keeps trailing text on one line (102 chars), we break at word boundary
	const prettier =
		'\t> text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14 text15_ x';
	const ours =
		'\t> text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 text13 text14 text15_\n\tx';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_101_boundary', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'fill_101_boundary');
});

// ─── jsdoc_type_cast_parens in a svelte lang="ts" context ─────────────────

Deno.test('jsdoc_type_cast_parens: positive - svelte lang=ts (ours keeps, prettier strips)', () => {
	// In `<script lang="ts">` prettier routes to oxc-ts and strips; tsv preserves.
	const prettier = '\tconst a = b.map((x) => /** @type {A} */ fn(x));';
	const ours = '\tconst a = b.map((x) => /** @type {A} */ (fn(x)));';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('jsdoc_type_cast_parens', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'jsdoc_type_cast_parens');
});

// ─── Overmatch-rejection negative tests (ours-side evidence guards) ─────────
//
// Each of these is a real formatting BUG (or content loss) on a line that
// looks like a known divergence. The ours-side guard added to the detector
// must REJECT it — claiming would mask a bug, and in corpus_compare_format can mask a
// data-loss safety violation by reclassifying it as known_divergence.

Deno.test('member_expression_call: negative - bug collapses what prettier expanded (no re-wrap)', () => {
	// `require.resolve(` is present, but ours COLLAPSED the call onto one line
	// while prettier kept it expanded — the opposite of the documented divergence
	// (ours expands). added_lines.length is not > removed_lines.length → reject.
	const prettier = 'const p = require.resolve(\n\t"x",\n);';
	const ours = 'const p = require.resolve("x");';
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = 'const p = require.resolve("x");';
	const match = run_pattern('member_expression_call', ctx);
	assertEquals(match, null);
});

Deno.test('member_expression_call: negative - module pattern only on prettier side', () => {
	// The module pattern appears only in prettier's removed lines; ours rewrote
	// the expression to something else (e.g. dropped `require.resolve`). The
	// divergent break must be in OUR output to claim — reject.
	const prettier = 'const p = require.resolve(\n\t"x",\n);';
	const ours = 'const p = foo(\n\t"x",\n\t"y",\n);';
	const ctx = make_context(ours, prettier, 'typescript');
	ctx.source = 'const p = require.resolve("x");';
	const match = run_pattern('member_expression_call', ctx);
	assertEquals(match, null);
});

Deno.test('short_expr_100: negative - bug in 101-110 band without legitimate break', () => {
	// Prettier's `{#if` line is in the 101-110 band, but ours did NOT break the
	// block condition — it mangled the line in place (same line count). Claiming
	// purely from the prettier width would mask this bug → reject.
	const expr = 'a'.repeat(65);
	const prettier = `{#if typeof ${expr} === 'string'}content{/if}`;
	const ours = `{#if typeof ${expr} === "string"}content{/if}`;
	const ctx = make_context(ours, prettier, 'svelte');
	const vw = visual_width(prettier);
	assertEquals(vw > 100 && vw <= 110, true, `Expected width 101-110, got ${vw}`);
	const match = run_pattern('short_expr_100', ctx);
	assertEquals(match, null);
});

Deno.test('fill_after_inline: negative - long inline-close line, ours did not re-wrap', () => {
	// Prettier's line has a long inline close tag (> 100), but ours emitted the
	// same long line (no legitimate fill break — a different edit on it). Without
	// the re-wrap guard this would be claimed purely from prettier's width.
	const longContent = 'x'.repeat(90);
	const prettier = `\t<p>Some text <span>${longContent}</span> more</p>`;
	const ours = `\t<p>Some text <span>${longContent}</span> MORE</p>`;
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('fill_after_inline', ctx);
	assertEquals(match, null);
});

Deno.test('fill_101_boundary: negative - removal-only hunk (empty added_lines)', () => {
	// A prettier line >= 100 chars that we simply DELETED (no added lines). The
	// Case-2 `added_lines.every(...)` guard is vacuously true for empty arrays —
	// requiring added_lines.length > 0 keeps a deleted long line from being
	// claimed as a print-width rewrap.
	const longLine = '\t' + 'x'.repeat(103); // visual width 105
	const prettier = `before\n${longLine}\nafter`;
	const ours = 'before\nafter';
	const ctx = make_context(ours, prettier, 'svelte');
	// Confirm the hunk really is removal-only.
	assertEquals(ctx.hunks.length, 1);
	assertEquals(ctx.hunks[0].added_lines.length, 0);
	const match = run_pattern('fill_101_boundary', ctx);
	assertEquals(match, null);
});

// ─── css_scss_directive_number (positive + negative) ───────────────────────

Deno.test('css_scss_directive_number: positive - @include number normalization divergence', () => {
	// Prettier value-parses SCSS @include and number-normalizes (.5s → 0.5s,
	// 1.50 → 1.5); tsv preserves the prelude verbatim. Same numeric-token count,
	// identical non-numeric skeleton → claim.
	const prettier = '<style>\n\t@include foo(transform, 0.5s ease);\n\t@include baz(1.5);\n</style>';
	const ours = '<style>\n\t@include foo(transform, .5s ease);\n\t@include baz(1.50);\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_scss_directive_number', ctx);
	assertNotEquals(match, null);
	assertEquals(match!.pattern, 'css_scss_directive_number');
});

Deno.test('css_scss_directive_number: negative - dropped numeric value not claimed', () => {
	// Ours DROPS a numeric value (`100px` → `px`) inside an @include. The old
	// skeleton stripped digits/dots, so this compared skeleton-equal and was
	// masked. The numeric-token-count check (prettier has 1 number, ours has 0)
	// rejects this real content loss.
	const prettier = '<style>\n\t@include foo(width: 100px);\n</style>';
	const ours = '<style>\n\t@include foo(width: px);\n</style>';
	const ctx = make_context(ours, prettier, 'svelte');
	const match = run_pattern('css_scss_directive_number', ctx);
	assertEquals(match, null);
});

// ─── hunk-scoped SAFETY vouching ───────────────────────────────────────────
//
// `safety_vouched` is deliberately stricter than `classification === 'all_explained'`:
// a SAFETY differential may only be excused by a pattern that declared it can change
// semantic char counts, AND only for the hunks that actually carry such a change.
// Before this, any pattern covering any hunk propped up the downgrade — on
// `prettier/tests/format/html/tags/tags.html` two unrelated whitespace hunks were
// load-bearing for a delta caused entirely by a third.

Deno.test('safety_vouched: a char-risky hunk claimed only by a NON-vouching pattern is not vouched', () => {
	// `comment_position` does not declare `may_alter_char_frequency` (it relocates a
	// comment, it does not add or drop content), so it cannot excuse a char delta.
	const prettier = '<div>\n\t<!-- c -->\n\t<span>a</span>\n</div>';
	const ours = '<div>\n\t<span>a</span>\n\t<!-- c -->\n</div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const coverage = detect_divergences(ctx);
	// Whatever it claims, no vouching pattern exists here, so any char-risky hunk
	// leaves the coverage unvouched.
	if (coverage.char_risky_hunks.length > 0) {
		assertEquals(coverage.safety_vouched, false);
	}
});

Deno.test('safety_vouched: a whitespace-only hunk is never char-risky, so it need not vouch', () => {
	// Reflowing whitespace cannot move the semantic char count, so such a hunk is
	// excluded from the vouching requirement entirely — which is the whole point:
	// it can no longer prop up (or, by regressing, collapse) a SAFETY downgrade.
	const prettier = '<div>\n\t<span>a</span>\n</div>';
	const ours = '<div>\n  <span>a</span>\n</div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const coverage = detect_divergences(ctx);
	assertEquals(coverage.char_risky_hunks, []);
});

Deno.test('safety_vouched: self_closing_nonvoid vouches for the hunk carrying its own char delta', () => {
	// `<i />` → `<i></i>` adds `<`, `/`, `>` and the tag name — a real char delta, and
	// exactly the tags.html case. The pattern declares `may_alter_char_frequency`, so
	// it may vouch for the hunk it claims.
	const prettier = '<div>\n\t<i class="x" />\n</div>';
	const ours = '<div>\n\t<i class="x"></i>\n</div>';
	const ctx = make_context(ours, prettier, 'svelte');
	const coverage = detect_divergences(ctx);
	assertEquals(coverage.char_risky_hunks.length > 0, true);
	assertEquals(coverage.safety_vouched, true);
});

Deno.test('may_alter_char_frequency: only deliberately-declared patterns may vouch', () => {
	// The declaration is a promise that the pattern's detect carries a
	// content-preservation proof. Keep the set small and reviewed — a new pattern is
	// safe by omission (the field defaults to false, so the gate fails CLOSED).
	const vouching = PATTERNS.filter((p) => p.may_alter_char_frequency).map((p) => p.id).sort();
	assertEquals(vouching, [
		'bom_strip',
		'comment_preserved',
		'css_scss_directive_number',
		'self_closing_nonvoid',
	]);
});
