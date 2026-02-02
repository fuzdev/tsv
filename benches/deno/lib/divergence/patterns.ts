/**
 * Divergence pattern detection - identify known intentional differences from Prettier.
 *
 * Each pattern corresponds to a documented divergence in conformance_prettier.md.
 * These are NOT bugs - they are design choices.
 */

import type {DiffLine} from '../diff.ts';
import type {Language} from '../types.ts';

export interface DetectionContext {
	/** Original source code */
	source: string;
	/** Our formatter output */
	ours: string;
	/** Prettier's output */
	prettier: string;
	/** Line-based diff between prettier and ours */
	diff: DiffLine[];
	/** Source language */
	language: Language;
}

export interface DivergenceMatch {
	/** Pattern ID (matches conformance_prettier.md) */
	pattern: string;
	/** Detection confidence */
	confidence: 'certain' | 'likely' | 'possible';
	/** Affected line numbers (0-indexed) */
	lines: number[];
	/** Human-readable explanation */
	reason: string;
}

export interface DivergencePattern {
	/** Pattern ID (matches fixture naming convention) */
	id: string;
	/** Human-readable description */
	description: string;
	/** Languages this pattern applies to */
	languages: Language[];
	/** Section names from conformance_prettier.md this pattern covers */
	conformanceSections: string[];
	/** Fixture paths (relative to tests/fixtures/) this pattern should detect */
	fixtures: string[];
	/** Detection function */
	detect: (ctx: DetectionContext) => DivergenceMatch | null;
}

/**
 * Calculate visual width of a line (tabs = 2 spaces).
 */
function visualWidth(line: string): number {
	let width = 0;
	for (const char of line) {
		width += char === '\t' ? 2 : 1;
	}
	return width;
}

/**
 * Find line numbers matching a pattern.
 */
function findLinesMatching(text: string, pattern: RegExp): number[] {
	const lines = text.split('\n');
	const matches: number[] = [];
	for (let i = 0; i < lines.length; i++) {
		if (pattern.test(lines[i])) {
			matches.push(i);
		}
	}
	return matches;
}

// Pattern detectors

const templateLiteralWidth: DivergencePattern = {
	id: 'template_literal_width',
	description: 'Template literal interpolation breaks to respect print width',
	languages: ['typescript', 'svelte'],
	conformanceSections: ['TypeScript: Template Literals'],
	fixtures: [
		'typescript/expressions/literals/template/long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_expression_long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_method_chain_long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_chain_blank_lines_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_multiline_indent_long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_nested_template_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_short_multiline_prettier_divergence',
		'typescript/types/template_literal_type_long_prettier_divergence',
		'typescript/types/template_literal_type_conditional_long_prettier_divergence',
		'typescript/expressions/literals/template/arrow_template_body_nested_long_prettier_divergence',
		'typescript/expressions/ternary/template_consequent_long_prettier_divergence',
		'typescript/expressions/logical/template_operand_long_prettier_divergence',
	],
	detect(ctx) {
		// Template literal break patterns we use:
		// 1. ${ at end of line, content on next line: `prefix ${
		//    content
		// }`
		// 2. ${ at start of indented line: `
		//    ${content}
		// `
		// 3. }` alone on a line (closing after break)

		// Pattern 1: ${ followed by newline (we break after ${)
		const oursBreakAfterDollar = /\$\{\s*\n/m.test(ctx.ours);
		const prettierBreakAfterDollar = /\$\{\s*\n/m.test(ctx.prettier);

		// Pattern 2: ${ at start of indented line
		const oursBreakAtStart = /^\t+\$\{/m.test(ctx.ours);
		const prettierBreakAtStart = /^\t+\$\{/m.test(ctx.prettier);

		// Pattern 3: }` alone on indented line (template ends after content break)
		const oursClosingAlone = /^\t+\}\`/m.test(ctx.ours);
		const prettierClosingAlone = /^\t+\}\`/m.test(ctx.prettier);

		const oursHasBreak = oursBreakAfterDollar || oursBreakAtStart || oursClosingAlone;
		const prettierHasBreak =
			prettierBreakAfterDollar || prettierBreakAtStart || prettierClosingAlone;

		if (oursHasBreak && !prettierHasBreak) {
			// Verify source has template literal
			if (ctx.source.includes('${')) {
				return {
					pattern: 'template_literal_width',
					confidence: 'likely',
					lines: findLinesMatching(ctx.ours, /\$\{\s*\n|^\t+\$\{|^\t+\}\`/),
					reason: 'Template interpolation breaks to respect print width',
				};
			}
		}
		return null;
	},
};

const blockExpressionLogical: DivergencePattern = {
	id: 'block_expression_logical',
	description: 'Block expression logical operators wrap to respect print width',
	languages: ['svelte'],
	conformanceSections: ['Svelte: Blocks'],
	fixtures: [
		'svelte/blocks/each/long_prettier_divergence',
		'svelte/blocks/await/long_prettier_divergence',
		'svelte/blocks/key/long_prettier_divergence',
		'svelte/blocks/if/long_prettier_divergence',
		'svelte/blocks/if/last_block_prettier_divergence',
		'svelte/blocks/if/in_inline_element_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Look for && or || at start of line after indentation
		const blockOperatorBreak = /^\t+(?:&&|\|\|)/m;

		if (blockOperatorBreak.test(ctx.ours) && !blockOperatorBreak.test(ctx.prettier)) {
			return {
				pattern: 'block_expression_logical',
				confidence: 'likely',
				lines: findLinesMatching(ctx.ours, blockOperatorBreak),
				reason: 'Logical expression in block condition broken to respect print width',
			};
		}
		return null;
	},
};

const fill101Boundary: DivergencePattern = {
	id: 'fill_101_boundary',
	description: 'Prettier allows lines to exceed print width, we break',
	languages: ['svelte', 'typescript', 'css'],
	conformanceSections: ['CSS: Layout', 'CSS: Values', 'Svelte/HTML'],
	fixtures: [
		'css/comma_separated_greedy_fill_prettier_divergence',
		'css/values/lists/comma_space_separated_101_prettier_divergence',
		'svelte/elements/inline_element_fill_101_prettier_divergence',
	],
	detect(ctx) {
		const prettierLines = ctx.prettier.split('\n');
		const oursLines = ctx.ours.split('\n');
		const matches: number[] = [];

		// Find lines where prettier exceeds print width but we don't
		// This is a line-by-line comparison since line counts may differ
		let longestPrettierOverflow = 0;

		for (let i = 0; i < prettierLines.length; i++) {
			const pLen = visualWidth(prettierLines[i]);

			// Prettier exceeds print width
			if (pLen > 100) {
				longestPrettierOverflow = Math.max(longestPrettierOverflow, pLen);
				matches.push(i);
			}
		}

		// If prettier has overflowing lines, this is a fill/width divergence
		if (matches.length > 0) {
			return {
				pattern: 'fill_101_boundary',
				confidence: 'likely',
				lines: matches,
				reason: `Prettier allows ${longestPrettierOverflow} chars, we break at print width`,
			};
		}
		return null;
	},
};

const inlineContentHug: DivergencePattern = {
	id: 'inline_content_hug',
	description: 'Expression breaks internally vs bracket breaks',
	languages: ['svelte'],
	conformanceSections: ['Svelte/HTML'],
	fixtures: ['svelte/elements/inline_content_hug_long_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// We keep > hugging content, break expression internally
		// Pattern 1: >{ followed by content then newline (expression breaks inside)
		const oursHugPattern1 = />\{[^}]*\n/;
		// Pattern 2: > followed by text then { with newline (ternary breaks)
		const oursHugPattern2 = />[^<\n]+\{[^\n]*\n\t*\?/;
		// Prettier breaks the opening tag: newline before >
		const prettierBreakPattern = /\n\t*>/;
		// Or breaks the opening tag itself: <tag\n>
		const prettierTagBreak = /<\w+\s*\n\t*>/;

		const oursHugs = oursHugPattern1.test(ctx.ours) || oursHugPattern2.test(ctx.ours);
		const prettierBreaks =
			prettierBreakPattern.test(ctx.prettier) || prettierTagBreak.test(ctx.prettier);

		if (oursHugs && prettierBreaks) {
			return {
				pattern: 'inline_content_hug',
				confidence: 'likely',
				lines: findLinesMatching(ctx.ours, />\{|>\w/),
				reason: 'Expression breaks internally vs bracket breaks',
			};
		}
		return null;
	},
};

const singleSpecifierImport: DivergencePattern = {
	id: 'single_specifier_import',
	description: 'Single-specifier import wraps at print width',
	languages: ['typescript', 'svelte'],
	conformanceSections: ['TypeScript'],
	fixtures: ['typescript/modules/imports/single_specifier_long_prettier_divergence'],
	detect(ctx) {
		// Look for import statements that span multiple lines in ours but not prettier
		const oursMultilineImport = /^import \{[\s\S]*?\n\t/m.test(ctx.ours);
		const prettierSingleLine = /^import \{ \w+ \} from/m.test(ctx.prettier);

		// Check if prettier's single-line import exceeds 100 chars
		const prettierLines = ctx.prettier.split('\n');
		const longImport = prettierLines.some(
			(line) => /^import \{/.test(line) && visualWidth(line) > 100,
		);

		if (oursMultilineImport && prettierSingleLine && longImport) {
			return {
				pattern: 'single_specifier_import',
				confidence: 'likely',
				lines: findLinesMatching(ctx.ours, /^import \{/),
				reason: 'Single specifier import wraps at print width',
			};
		}
		return null;
	},
};

const selfClosingNonvoid: DivergencePattern = {
	id: 'self_closing_nonvoid',
	description: 'Empty component normalized to self-closing',
	languages: ['svelte'],
	conformanceSections: ['Svelte/HTML'],
	fixtures: ['svelte/elements/self_closing_nonvoid_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Our output has /> for components, prettier has ></Component>
		const oursSelfClosing = /<[A-Z]\w*[^>]*\/>/;
		const prettierExplicitClose = /<[A-Z]\w*[^>]*><\/[A-Z]\w*>/;

		if (oursSelfClosing.test(ctx.ours) && prettierExplicitClose.test(ctx.prettier)) {
			return {
				pattern: 'self_closing_nonvoid',
				confidence: 'likely',
				lines: findLinesMatching(ctx.ours, oursSelfClosing),
				reason: 'Empty component normalized to self-closing',
			};
		}
		return null;
	},
};

const bomStrip: DivergencePattern = {
	id: 'bom_strip',
	description: 'BOM (byte order mark) removed',
	languages: ['svelte', 'typescript', 'css'],
	conformanceSections: ['Whitespace: BOM Handling'],
	fixtures: [
		'svelte/syntax/whitespace/bom_prettier_divergence',
		'css/tokens/whitespace/bom_prettier_divergence',
		'typescript/syntax/whitespace/bom_prettier_divergence',
	],
	detect(ctx) {
		// Source starts with BOM, our output doesn't
		if (ctx.source.startsWith('\ufeff') && !ctx.ours.startsWith('\ufeff')) {
			// Verify prettier keeps BOM
			if (ctx.prettier.startsWith('\ufeff')) {
				return {
					pattern: 'bom_strip',
					confidence: 'certain',
					lines: [0],
					reason: 'BOM (byte order mark) removed',
				};
			}
		}
		return null;
	},
};

const fillAfterInline: DivergencePattern = {
	id: 'fill_after_inline',
	description: 'Text after inline element breaks at print width',
	languages: ['svelte'],
	conformanceSections: ['Svelte/HTML'],
	fixtures: [
		'svelte/elements/fill_after_inline_prettier_divergence',
		'svelte/elements/fill_multiple_expr_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Prettier allows text after </inline> to exceed print width
		// We break at 100
		const prettierLines = ctx.prettier.split('\n');
		const hasLongLineAfterInline = prettierLines.some((line) => {
			return (
				/<\/(?:span|a|strong|em|code|b|i|small|abbr|sub|sup)>/.test(line) && visualWidth(line) > 100
			);
		});

		if (hasLongLineAfterInline) {
			const oursLines = ctx.ours.split('\n');
			const ourMaxWidth = Math.max(...oursLines.map(visualWidth));

			if (ourMaxWidth <= 100) {
				return {
					pattern: 'fill_after_inline',
					confidence: 'likely',
					lines: [],
					reason: 'Text after inline element breaks at print width',
				};
			}
		}
		return null;
	},
};

const cssValueWrap: DivergencePattern = {
	id: 'css_value_wrap',
	description: 'CSS property value wraps at print width',
	languages: ['css', 'svelte'],
	conformanceSections: ['CSS: Values'],
	fixtures: [
		'css/values/functions/transform_long_prettier_divergence',
		'css/values/lists/space_separated_long_wrap_prettier_divergence',
	],
	detect(ctx) {
		// Check if prettier has long CSS property values that we break
		const prettierLines = ctx.prettier.split('\n');
		const hasLongCssValue = prettierLines.some((line) => {
			// Match CSS property: value pattern with long value
			return /^\t+[\w-]+:\s*.+/.test(line) && visualWidth(line) > 100;
		});

		if (hasLongCssValue) {
			const oursLines = ctx.ours.split('\n');
			const ourMaxWidth = Math.max(...oursLines.map(visualWidth));

			if (ourMaxWidth <= 100) {
				return {
					pattern: 'css_value_wrap',
					confidence: 'likely',
					lines: [],
					reason: 'CSS property value wraps at print width',
				};
			}
		}
		return null;
	},
};

const memberExpressionCall: DivergencePattern = {
	id: 'member_expression_call',
	description: 'Member expression in call args breaks differently',
	languages: ['typescript', 'svelte'],
	conformanceSections: ['TypeScript'],
	fixtures: ['typescript/modules/imports/path_calls_long_prettier_divergence'],
	detect(ctx) {
		// Patterns like require.resolve.paths() or import.meta.resolve()
		// Prettier breaks at the chain, we expand call arguments
		const modulePatterns = /(?:require\.resolve(?:\.paths)?|import\.meta\.resolve)\(/;

		if (modulePatterns.test(ctx.source)) {
			// Check if diff shows different breaking patterns
			const additions = ctx.diff.filter((d) => d.type === 'add');
			const removals = ctx.diff.filter((d) => d.type === 'remove');

			// We have additions/removals around these patterns
			const hasRelatedChanges =
				additions.some((d) => modulePatterns.test(d.line)) ||
				removals.some((d) => modulePatterns.test(d.line));

			if (hasRelatedChanges) {
				return {
					pattern: 'member_expression_call',
					confidence: 'possible',
					lines: [],
					reason: 'Member expression in call args breaks differently',
				};
			}
		}
		return null;
	},
};

const commentPosition: DivergencePattern = {
	id: 'comment_position',
	description: 'Comment preserved where user placed it (Prettier relocates)',
	languages: ['typescript', 'svelte'],
	conformanceSections: ['TypeScript: Comments', 'CSS: Comments', 'Svelte: Attributes'],
	fixtures: [
		// TypeScript comments
		'typescript/statements/switch/empty_comment_prettier_divergence',
		'typescript/statements/switch/case_block_comment_prettier_divergence',
		'typescript/statements/switch/discriminant_trailing_comment_prettier_divergence',
		'typescript/statements/for/trailing_comment_prettier_divergence',
		'typescript/statements/for/empty_clauses_comment_prettier_divergence',
		'typescript/statements/for/of_line_comment_prettier_divergence',
		'typescript/statements/do_while/open_paren_comment_prettier_divergence',
		'typescript/statements/try/catch_between_comment_prettier_divergence',
		'typescript/statements/labeled/comment_prettier_divergence',
		'typescript/statements/if/else_block_own_line_comment_prettier_divergence',
		'typescript/statements/while/line_before_body_comment_prettier_divergence',
		'typescript/statements/do_while/line_before_while_comment_prettier_divergence',
		// TypeScript chain comments
		'typescript/expressions/calls/chained/trailing_member_comment_prettier_divergence',
		'typescript/expressions/calls/chained/trailing_member_computed_comment_prettier_divergence',
		// Svelte comments
		'svelte/syntax/comments/expr_trailing_prettier_divergence',
		'svelte/tags/debug/debug_comment_prettier_divergence',
	],
	detect(ctx) {
		// Check if source has comments that appear at different positions in output
		// This is a broad pattern - specific cases are in conformance_prettier.md

		// Look for line comments in diff that appear in different contexts
		const sourceCommentLines = findLinesMatching(ctx.source, /\/\/|\/\*|\*\//);
		const oursCommentLines = findLinesMatching(ctx.ours, /\/\/|\/\*|\*\//);
		const prettierCommentLines = findLinesMatching(ctx.prettier, /\/\/|\/\*|\*\//);

		// If comment line counts differ significantly, Prettier may have moved comments
		if (
			sourceCommentLines.length > 0 &&
			Math.abs(oursCommentLines.length - prettierCommentLines.length) > 0
		) {
			return {
				pattern: 'comment_position',
				confidence: 'possible',
				lines: oursCommentLines,
				reason: 'Comment preserved where user placed it (Prettier relocates)',
			};
		}
		return null;
	},
};

const blockMultilineAttrsHug: DivergencePattern = {
	id: 'block_multiline_attrs_hug',
	description: 'Block element with multiline attrs, we break >',
	languages: ['svelte'],
	conformanceSections: ['Svelte/HTML'],
	fixtures: ['svelte/elements/block_multiline_attrs_content_hug_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Pattern: <pre with multiline attrs, we put > on its own line
		// Prettier keeps >{content}</pre> on same line as last attr

		// Check for <pre or <textarea in source with multiline attrs
		const hasWhitespaceSensitive = /<(?:pre|textarea)[^>]*\n/i.test(ctx.source);

		if (hasWhitespaceSensitive) {
			// We have \n> pattern (break before >)
			const oursBreaksGt = /\n\t*>/m.test(ctx.ours);
			// Prettier has attr followed by > on same line
			const prettierHugsGt = /['"]\s*>/m.test(ctx.prettier);

			if (oursBreaksGt && prettierHugsGt) {
				return {
					pattern: 'block_multiline_attrs_hug',
					confidence: 'likely',
					lines: findLinesMatching(ctx.ours, /^\t*>$/),
					reason: 'Block element with multiline attrs, we break > to new line',
				};
			}
		}
		return null;
	},
};

const shortExpr100: DivergencePattern = {
	id: 'short_expr_100',
	description: 'Short expression in block exceeds 100 chars, we break',
	languages: ['svelte'],
	conformanceSections: ['Svelte: Blocks'],
	fixtures: ['svelte/blocks/if/in_inline_element_long_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Prettier allows {#if short_expr} to exceed 100 chars
		// We break these
		const prettierLines = ctx.prettier.split('\n');
		const longBlockExpr = prettierLines.some((line) => {
			return (
				/\{#(?:if|each|await|key)/.test(line) && visualWidth(line) > 100 && visualWidth(line) <= 110
			);
		});

		if (longBlockExpr) {
			const oursLines = ctx.ours.split('\n');
			const ourBlockLines = oursLines.filter((line) => /\{#(?:if|each|await|key)/.test(line));
			const ourMaxBlockWidth = Math.max(...ourBlockLines.map(visualWidth), 0);

			if (ourMaxBlockWidth <= 100) {
				return {
					pattern: 'short_expr_100',
					confidence: 'likely',
					lines: [],
					reason: 'Short expression in block condition exceeds 100 chars, we break',
				};
			}
		}
		return null;
	},
};

// All patterns in detection order (most common first for performance)
export const PATTERNS: DivergencePattern[] = [
	templateLiteralWidth, // Most common (~22 files in corpus)
	fill101Boundary, // Common edge case
	blockExpressionLogical, // Common in Svelte
	inlineContentHug, // Svelte-specific
	fillAfterInline, // Svelte-specific
	blockMultilineAttrsHug, // Svelte-specific
	shortExpr100, // Svelte-specific
	singleSpecifierImport, // TypeScript/Svelte
	selfClosingNonvoid, // Svelte-specific
	cssValueWrap, // CSS-specific
	memberExpressionCall, // TypeScript/Svelte
	commentPosition, // Broad pattern
	bomStrip, // Rare but certain
];

/**
 * Detect which known divergence patterns explain the difference between
 * our formatter output and Prettier's output.
 *
 * @param ctx - Detection context (source, ours, prettier, diff, language)
 * @returns Array of matching divergence patterns
 */
export function detectDivergences(ctx: DetectionContext): DivergenceMatch[] {
	const matches: DivergenceMatch[] = [];

	for (const pattern of PATTERNS) {
		if (!pattern.languages.includes(ctx.language)) continue;

		const match = pattern.detect(ctx);
		if (match) {
			matches.push(match);
		}
	}

	return matches;
}
