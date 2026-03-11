/**
 * Divergence pattern detection - identify known intentional differences from Prettier.
 *
 * Each pattern corresponds to a documented divergence in conformance_prettier.md.
 * These are NOT bugs - they are design choices.
 *
 * Patterns are ordered from most specific to most broad. This ensures hunks get
 * the most precise explanation possible. Multiple patterns CAN claim the same hunk.
 */

import type { DiffHunk, DiffLine } from '../diff.ts';
import type { Language } from '../types.ts';

export interface DetectionContext {
	/** Original source code */
	source: string;
	/** Our formatter output */
	ours: string;
	/** Prettier's output */
	prettier: string;
	/** Line-based diff between prettier and ours */
	diff: DiffLine[];
	/** Diff hunks extracted from diff (contiguous change groups) */
	hunks: DiffHunk[];
	/** Source language */
	language: Language;
}

export interface DivergenceMatch {
	/** Pattern ID (matches conformance_prettier.md) */
	pattern: string;
	/** Detection confidence */
	confidence: 'certain' | 'likely' | 'possible';
	/** Indices of hunks this pattern explains */
	hunkIndices: number[];
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

export interface HunkCoverageResult {
	/** All hunks in the diff */
	hunks: DiffHunk[];
	/** Pattern matches with hunk associations */
	matches: DivergenceMatch[];
	/** Set of hunk indices explained by at least one pattern */
	explainedHunks: Set<number>;
	/** Hunk indices not explained by any pattern */
	unexplainedHunks: number[];
	/** Overall classification */
	classification: 'all_explained' | 'partial' | 'none_explained';
}

/**
 * Find hunk indices where the predicate matches.
 */
function findMatchingHunks(hunks: DiffHunk[], predicate: (h: DiffHunk) => boolean): number[] {
	const indices: number[] = [];
	for (const hunk of hunks) {
		if (predicate(hunk)) {
			indices.push(hunk.index);
		}
	}
	return indices;
}

/**
 * Get prettier lines within a hunk's prettier range.
 */
function prettierLinesInHunk(prettierLines: string[], hunk: DiffHunk): string[] {
	if (!hunk.prettierRange) return [];
	return prettierLines.slice(hunk.prettierRange.start, hunk.prettierRange.end + 1);
}

/**
 * Get ours lines within a hunk's ours range.
 */
function oursLinesInHunk(oursLines: string[], hunk: DiffHunk): string[] {
	if (!hunk.oursRange) return [];
	return oursLines.slice(hunk.oursRange.start, hunk.oursRange.end + 1);
}

/**
 * Check if a hunk's context (surrounding content, added/removed lines) is within
 * a CSS context. For Svelte files, looks for <style> context. For CSS files, always true.
 */
function isInCssContext(hunk: DiffHunk, ctx: DetectionContext): boolean {
	if (ctx.language === 'css') return true;
	if (ctx.language !== 'svelte') return false;

	// Check if the hunk lines are inside a <style> block
	// Look at the full source for <style> boundaries
	const oursLines = ctx.ours.split('\n');
	let inStyle = false;
	const startLine = hunk.oursRange?.start ?? hunk.prettierRange?.start ?? 0;

	// Scan from beginning up to hunk start to determine if we're in <style>
	for (let i = 0; i < startLine && i < oursLines.length; i++) {
		if (/<style[\s>]/.test(oursLines[i])) inStyle = true;
		if (/<\/style>/.test(oursLines[i])) inStyle = false;
	}

	return inStyle;
}

/**
 * Extract comment text content from a line (strip delimiters and whitespace).
 * Returns the text inside the comment, ignoring surrounding code.
 */
function extractCommentContent(line: string): string {
	// Line comment: extract text after //
	const lineComment = line.match(/\/\/\s*(.*)/);
	if (lineComment) return lineComment[1].trim();
	// Block comment: extract text inside /* */
	const blockComment = line.match(/\/\*\s*(.*?)\s*\*\//);
	if (blockComment) return blockComment[1].trim();
	// Partial block comment (opening or closing only)
	const blockOpen = line.match(/\/\*\s*(.*)/);
	if (blockOpen) return blockOpen[1].trim();
	const blockClose = line.match(/(.*?)\s*\*\//);
	if (blockClose) return blockClose[1].trim();
	return line.trim();
}

// ─── Pattern Detectors ──────────────────────────────────────────────────────
//
// Ordered from most specific/narrow to most broad.
// Specific patterns run first so hunks get the most precise explanation.

// ─── Language-specific narrow patterns ──────────────────────────────────────

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
				// BOM difference is always in hunk 0 (first line)
				const hunkIndices = ctx.hunks.length > 0 ? [0] : [];
				return {
					pattern: 'bom_strip',
					confidence: 'certain',
					hunkIndices,
					reason: 'BOM (byte order mark) removed',
				};
			}
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

		const oursSelfClosing = /<[A-Z]\w*[^>]*\/>/;
		const prettierExplicitClose = /<\/[A-Z]\w*>/;

		// Check hunks for self-closing vs explicit close differences
		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			const addedHasSelfClose = hunk.addedLines.some((l) => oursSelfClosing.test(l));
			const removedHasExplicitClose = hunk.removedLines.some((l) => prettierExplicitClose.test(l));
			return addedHasSelfClose && removedHasExplicitClose;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'self_closing_nonvoid',
				confidence: 'likely',
				hunkIndices,
				reason: 'Empty component normalized to self-closing',
			};
		}
		return null;
	},
};

const emptyStatementRemoval: DivergencePattern = {
	id: 'empty_statement_removal',
	description: 'Standalone empty statement (;) removed',
	languages: ['typescript', 'svelte'],
	conformanceSections: ['TypeScript'],
	fixtures: ['typescript/statements/empty_standalone_prettier_divergence'],
	detect(ctx) {
		// Look for hunks where removed lines contain standalone semicolons
		// (not part of for(;;) or other syntax)
		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			// Removed lines should have standalone ; that we remove
			const removedStandalone = hunk.removedLines.some((l) => /^\t*;$/.test(l));
			// Added lines should NOT have standalone ;
			const addedStandalone = hunk.addedLines.some((l) => /^\t*;$/.test(l));
			return removedStandalone && !addedStandalone;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'empty_statement_removal',
				confidence: 'certain',
				hunkIndices,
				reason: 'Standalone empty statement (;) removed',
			};
		}
		return null;
	},
};

const cssValueRatio: DivergencePattern = {
	id: 'css_value_ratio',
	description: 'Ratio spacing normalized in CSS',
	languages: ['css', 'svelte'],
	conformanceSections: ['CSS: Values'],
	fixtures: ['css/values/ratio/ratio_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		// Look for ratio patterns (digit / digit) with spacing differences
		const ratioPattern = /\d+\s*\/\s*\d+/;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			if (!isInCssContext(hunk, ctx)) return false;

			const removedHasRatio = hunk.removedLines.some((l) => ratioPattern.test(l));
			const addedHasRatio = hunk.addedLines.some((l) => ratioPattern.test(l));
			if (!removedHasRatio || !addedHasRatio) return false;

			// Check for spacing differences around /
			const removedSpacing = hunk.removedLines.some((l) => /\d+\s{2,}\/|\/ {2,}\d+/.test(l));
			const addedNormalized = hunk.addedLines.some((l) => /\d+ \/ \d+/.test(l));
			return removedSpacing && addedNormalized;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'css_value_ratio',
				confidence: 'likely',
				hunkIndices,
				reason: 'Ratio spacing normalized in CSS',
			};
		}
		return null;
	},
};

// ─── CSS-specific patterns ──────────────────────────────────────────────────

const cssAtruleSpecSpacing: DivergencePattern = {
	id: 'css_atrule_spec_spacing',
	description: 'CSS at-rule keyword spacing normalized per spec',
	languages: ['css', 'svelte'],
	conformanceSections: ['CSS: At-Rules'],
	fixtures: [
		'css/at_rules/container_spacing_prettier_divergence',
		'css/at_rules/media_boolean_spacing_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		// Detect missing space before ( after boolean keywords: and(, or(, not(
		// Also detect style( vs style ( in container queries
		const missingSpace = /(?:and|or|not|style)\(/;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			if (!isInCssContext(hunk, ctx)) return false;

			// Removed lines (prettier) have and( or or( without space
			const removedMissingSpace = hunk.removedLines.some((l) => missingSpace.test(l));
			// Added lines (ours) have and ( or or ( with space
			const addedHasSpace = hunk.addedLines.some((l) => /(?:and|or|not|style) \(/.test(l));

			// Also check the reverse: we normalize spacing where prettier doesn't
			const removedHasAtRule = hunk.removedLines.some((l) =>
				/@(?:container|media|supports)/.test(l)
			);
			const addedHasAtRule = hunk.addedLines.some((l) => /@(?:container|media|supports)/.test(l));

			return (removedMissingSpace && addedHasSpace) ||
				(removedHasAtRule && addedHasAtRule && removedMissingSpace);
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'css_atrule_spec_spacing',
				confidence: 'certain',
				hunkIndices,
				reason: 'CSS at-rule keyword spacing normalized per spec (CSS Syntax 3 §4.3.4)',
			};
		}
		return null;
	},
};

const cssAtruleLongWrap: DivergencePattern = {
	id: 'css_atrule_long_wrap',
	description: 'CSS at-rule wraps at print width',
	languages: ['css', 'svelte'],
	conformanceSections: ['CSS: At-Rules'],
	fixtures: [
		'css/at_rules/container_long_prettier_divergence',
		'css/at_rules/media_long_prettier_divergence',
		'css/at_rules/import_media_query_long_prettier_divergence',
		'css/at_rules/supports_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const prettierLines = ctx.prettier.split('\n');
		const atRulePattern = /@(?:container|media|import|supports)/;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			if (!isInCssContext(hunk, ctx)) return false;

			// Prettier's removed lines have a long at-rule that exceeds 100 chars
			const pLines = prettierLinesInHunk(prettierLines, hunk);
			const hasLongAtRule = pLines.some(
				(l) => atRulePattern.test(l) && visualWidth(l) > 100,
			);
			if (!hasLongAtRule) return false;

			// We have more lines (we wrapped)
			return hunk.addedLines.length > hunk.removedLines.length;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'css_atrule_long_wrap',
				confidence: 'likely',
				hunkIndices,
				reason: 'CSS at-rule wraps at print width',
			};
		}
		return null;
	},
};

const cssAtruleStableQuirk: DivergencePattern = {
	id: 'css_atrule_stable_quirk',
	description: 'CSS at-rule stable quirk (Prettier preserves multiple forms)',
	languages: ['css', 'svelte'],
	conformanceSections: ['CSS: At-Rules'],
	fixtures: [
		'css/at_rules/layer_list_prettier_divergence',
		'css/at_rules/scope_complex_prettier_divergence',
		'css/at_rules/scope_selector_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			if (!isInCssContext(hunk, ctx)) return false;

			const removedJoined = hunk.removedLines.join('\n');
			const addedJoined = hunk.addedLines.join('\n');

			// @layer with spacing quirks (extra spaces after commas)
			if (/@layer/.test(removedJoined) || /@layer/.test(addedJoined)) {
				const removedExtraSpaces = hunk.removedLines.some((l) =>
					/@layer/.test(l) && /,\s{2,}/.test(l)
				);
				const addedNormalized = hunk.addedLines.some((l) => /@layer/.test(l) && /, [^\s]/.test(l));
				if (removedExtraSpaces && addedNormalized) return true;
			}

			// @scope with spacing quirks (spaces inside parens, double spaces around to)
			if (/@scope/.test(removedJoined) || /@scope/.test(addedJoined)) {
				// Prettier adds spaces inside scope parens: ( .class ) vs (.class)
				const removedHasQuirk = hunk.removedLines.some((l) =>
					/@scope/.test(l) && (/\( /.test(l) || / \)/.test(l) || /\s{2,}to\s{2,}/.test(l))
				);
				const addedIsNormal = hunk.addedLines.some((l) => /@scope/.test(l));
				if (removedHasQuirk && addedIsNormal) return true;
			}

			return false;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'css_atrule_stable_quirk',
				confidence: 'likely',
				hunkIndices,
				reason: 'CSS at-rule stable quirk (Prettier preserves multiple forms, we normalize)',
			};
		}
		return null;
	},
};

const cssSelectorDivergence: DivergencePattern = {
	id: 'css_selector_divergence',
	description: 'CSS selector formatting divergence',
	languages: ['css', 'svelte'],
	conformanceSections: ['CSS: Selectors'],
	fixtures: [
		'css/selectors/combinators/column_prettier_divergence',
		'css/selectors/pseudo_class/nth_child_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			if (!isInCssContext(hunk, ctx)) return false;

			// Column combinator: || with/without spaces in CSS selectors
			const removedHasCompact = hunk.removedLines.some((l) => /\w\|\|\w/.test(l) && /{/.test(l));
			const addedHasSpaced = hunk.addedLines.some((l) => /\w \|\| \w/.test(l) && /{/.test(l));
			if (removedHasCompact && addedHasSpaced) return true;

			// nth-child An+B normalization: spacing differences around operators
			const nthPattern = /:nth-(?:child|last-child|of-type|last-of-type)\(/;
			const removedHasNth = hunk.removedLines.some((l) => nthPattern.test(l));
			const addedHasNth = hunk.addedLines.some((l) => nthPattern.test(l));
			if (removedHasNth && addedHasNth) {
				// Check for spacing difference in the An+B expression
				const removedNthContent = hunk.removedLines.filter((l) => nthPattern.test(l));
				const addedNthContent = hunk.addedLines.filter((l) => nthPattern.test(l));
				if (
					removedNthContent.length > 0 && addedNthContent.length > 0 &&
					removedNthContent.some((l, i) =>
						addedNthContent[i] && l.replace(/\s+/g, '') === addedNthContent[i].replace(/\s+/g, '')
					)
				) {
					return true;
				}
			}

			return false;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'css_selector_divergence',
				confidence: 'likely',
				hunkIndices,
				reason: 'CSS selector formatting divergence',
			};
		}
		return null;
	},
};

const cssCommentStableQuirk: DivergencePattern = {
	id: 'css_comment_stable_quirk',
	description: 'CSS comment position stable quirk (Prettier preserves multiple forms)',
	languages: ['css', 'svelte'],
	conformanceSections: ['CSS: Comments'],
	fixtures: [
		'css/tokens/comments/atrule_before_opening_brace_prettier_divergence',
		'css/tokens/comments/atrule_in_prelude_prettier_divergence',
		'css/tokens/comments/keyframes_before_opening_brace_prettier_divergence',
		'css/tokens/comments/in_property_value_after_colon_prettier_divergence',
		'css/tokens/comments/in_property_value_before_colon_prettier_divergence',
		'css/tokens/comments/media_list_prettier_divergence',
		'css/tokens/comments/media_long_prettier_divergence',
		'css/tokens/comments/selector_before_opening_brace_prettier_divergence',
		'css/tokens/comments/selector_list_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const commentPattern = /\/\*.*?\*\/|\/\*|\*\//;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			if (!isInCssContext(hunk, ctx)) return false;

			// Both sides have CSS comments, but position/spacing differs
			const addedHasComment = hunk.addedLines.some((l) => commentPattern.test(l));
			const removedHasComment = hunk.removedLines.some((l) => commentPattern.test(l));

			if (!addedHasComment && !removedHasComment) return false;

			// Extract comment text from both sides and verify content is the same
			// (only position/spacing should differ, not content)
			const singleLineComment = /\/\*(.*?)\*\//;
			const addedCommentTexts = hunk.addedLines
				.filter((l) => commentPattern.test(l))
				.map((l) => {
					const m = l.match(singleLineComment);
					return m ? m[1].trim() : '';
				});
			const removedCommentTexts = hunk.removedLines
				.filter((l) => commentPattern.test(l))
				.map((l) => {
					const m = l.match(singleLineComment);
					return m ? m[1].trim() : '';
				});

			// Comment content should be the same - only position differs
			if (addedCommentTexts.length === 0 && removedCommentTexts.length === 0) return false;

			// If one side has comment and other doesn't, it moved
			if (addedHasComment !== removedHasComment) return true;

			// Both have comments - verify same content, different position
			if (addedCommentTexts.length > 0 && removedCommentTexts.length > 0) {
				const addedSet = new Set(addedCommentTexts);
				const removedSet = new Set(removedCommentTexts);
				// At least some comment content overlaps
				const hasOverlap = [...addedSet].some((t) => removedSet.has(t));
				if (hasOverlap) {
					// Lines differ (position change)
					const addedCommentLines = hunk.addedLines.filter((l) => commentPattern.test(l));
					const removedCommentLines = hunk.removedLines.filter((l) => commentPattern.test(l));
					return addedCommentLines.some((l, i) => l !== removedCommentLines[i]);
				}
			}

			return false;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'css_comment_stable_quirk',
				confidence: 'likely',
				hunkIndices,
				reason: 'CSS comment position stable quirk (we normalize)',
			};
		}
		return null;
	},
};

// ─── Feature-specific patterns ──────────────────────────────────────────────

const templateLiteralWidth: DivergencePattern = {
	id: 'template_literal_width',
	description: 'Template literal interpolation breaks to respect print width',
	languages: ['typescript', 'svelte'],
	conformanceSections: ['TypeScript: Template Literals'],
	fixtures: [
		'typescript/expressions/literals/template/long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_expression_long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_method_chain_long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_multiline_indent_long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_nested_template_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_short_multiline_prettier_divergence',
		'typescript/types/template_literal_type_long_prettier_divergence',
		'typescript/types/template_literal_type_conditional_long_prettier_divergence',
		'typescript/expressions/ternary/template_consequent_long_prettier_divergence',
		'typescript/expressions/logical/template_operand_long_prettier_divergence',
		'typescript/expressions/calls/template_interpolation_call_break_prettier_divergence',
	],
	detect(ctx) {
		// Template literal break patterns — we break inside ${...} to respect print width.
		// Detect by looking for lines that END with ${ (the break point) or start with }`
		// (closing after break). Must use end-of-line anchor to avoid matching inline ${expr}
		// which appears in both our output and prettier's output.
		const breakAfterDollarBrace = /\$\{\s*$/;
		const closingBraceBacktick = /^\t+\}\`/;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			const addedHasBreak = hunk.addedLines.some(
				(l) => breakAfterDollarBrace.test(l) || closingBraceBacktick.test(l),
			);
			const removedHasBreak = hunk.removedLines.some(
				(l) => breakAfterDollarBrace.test(l) || closingBraceBacktick.test(l),
			);
			return addedHasBreak && !removedHasBreak;
		});

		if (hunkIndices.length > 0 && ctx.source.includes('${')) {
			return {
				pattern: 'template_literal_width',
				confidence: 'likely',
				hunkIndices,
				reason: 'Template interpolation breaks to respect print width',
			};
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

		// Look for && or || at start of line in added hunk lines (we break)
		// but not in removed lines (prettier keeps inline)
		const blockOperatorBreak = /^\t+(?:&&|\|\|)/;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			return hunk.addedLines.some((l) => blockOperatorBreak.test(l)) &&
				!hunk.removedLines.some((l) => blockOperatorBreak.test(l));
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'block_expression_logical',
				confidence: 'likely',
				hunkIndices,
				reason: 'Logical expression in block condition broken to respect print width',
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
		// Check hunks for import statement differences
		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			// Added lines show multiline import (we break)
			const addedHasImport = hunk.addedLines.some((l) => /^import \{/.test(l));
			// Removed lines show single-line import (prettier keeps inline)
			const removedHasLongImport = hunk.removedLines.some(
				(l) => /^import \{/.test(l) && visualWidth(l) > 100,
			);
			return addedHasImport && removedHasLongImport;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'single_specifier_import',
				confidence: 'likely',
				hunkIndices,
				reason: 'Single specifier import wraps at print width',
			};
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
		const modulePatterns = /(?:require\.resolve(?:\.paths)?|import\.meta\.resolve)\(/;

		if (!modulePatterns.test(ctx.source)) return null;

		// Map to specific hunks that contain the module pattern
		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			return hunk.addedLines.some((l) => modulePatterns.test(l)) ||
				hunk.removedLines.some((l) => modulePatterns.test(l));
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'member_expression_call',
				confidence: 'possible',
				hunkIndices,
				reason: 'Member expression in call args breaks differently',
			};
		}
		return null;
	},
};

const returnTypeGenericUnion: DivergencePattern = {
	id: 'return_type_generic_union',
	description: 'Return type generic with union wraps at print width',
	languages: ['typescript', 'svelte'],
	conformanceSections: ['TypeScript'],
	fixtures: [
		'typescript/declarations/function/return_type_generic_union_long_prettier_divergence',
	],
	detect(ctx) {
		const prettierLines = ctx.prettier.split('\n');

		// Look for generic types with union (| null, | void, | undefined) in hunks
		// where prettier's line exceeds 100 chars
		const unionInGeneric = /[<>].*\|\s*(?:null|void|undefined)/;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			const pLines = prettierLinesInHunk(prettierLines, hunk);
			// Prettier has a long line with generic union
			const hasLongGenericUnion = pLines.some(
				(l) => unionInGeneric.test(l) && visualWidth(l) > 100,
			);
			if (!hasLongGenericUnion) return false;

			// We break (more lines in our version)
			return hunk.addedLines.length > hunk.removedLines.length;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'return_type_generic_union',
				confidence: 'likely',
				hunkIndices,
				reason: 'Return type generic with union wraps at print width',
			};
		}
		return null;
	},
};

// ─── Svelte-specific patterns ───────────────────────────────────────────────

const menuBlock: DivergencePattern = {
	id: 'menu_block',
	description: '<menu> treated as block element (spec-compliant)',
	languages: ['svelte'],
	conformanceSections: ['Svelte/HTML'],
	fixtures: ['svelte/elements/menu_block_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Look for hunks involving <menu> elements where prettier hugs content
		// (inline formatting) and we expand it (block formatting)
		const oursLines = ctx.ours.split('\n');
		const prettierLines = ctx.prettier.split('\n');

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			// Check for </menu in removed lines (prettier hugs: content</menu on same line,
			// with > possibly on next line)
			const removedHasMenuClose = hunk.removedLines.some((l) => /<\/menu/.test(l));
			// Check for </menu> on added lines on its own line (we expand: block formatting)
			const addedHasMenuClose = hunk.addedLines.some((l) => /^\s*<\/menu>/.test(l));

			if (removedHasMenuClose || addedHasMenuClose) return true;

			// Also check context: <menu in surrounding lines
			const oLines = oursLinesInHunk(oursLines, hunk);
			const pLines = prettierLinesInHunk(prettierLines, hunk);
			const contextLines = hunk.lines.filter((l) => l.type === 'same').map((l) => l.line);
			const allLines = [...oLines, ...pLines, ...contextLines];
			const hasMenuElement = allLines.some((l) => /<menu[\s>]/.test(l));

			if (!hasMenuElement) return false;

			// Prettier hugs: >{content} on same line as attribute
			const removedHugs = hunk.removedLines.some((l) => />[^<\n]*<\/menu/.test(l));
			// We expand: > on own line
			const addedBreaksGt = hunk.addedLines.some((l) => /^\t*>$/.test(l));

			return removedHugs || addedBreaksGt;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'menu_block',
				confidence: 'certain',
				hunkIndices,
				reason: '<menu> treated as block element (prettier treats as inline)',
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

		// For each hunk, check if removed lines show tag breaks (prettier breaks tag)
		// while added lines show >{ hugging (we hug content)
		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			const addedJoined = hunk.addedLines.join('\n');
			const removedJoined = hunk.removedLines.join('\n');

			// Our added lines hug: >{ or > followed by content
			const oursHugs = />\{/.test(addedJoined) || />[^<\n]+\{/.test(addedJoined);
			// Prettier removed lines show tag break:
			//   - > alone on a line (tag break with content on next line)
			//   - >content on a line (tag break with content on same line, e.g. <small\n\t>text{expr})
			//   - removed content ending with > (tag with > at end of line)
			// Exclude closing tags (>/) to avoid matching </tag>
			const prettierBreaks = hunk.removedLines.some((l) => /^\s*>(?!\/)/.test(l)) ||
				/>\s*$/.test(removedJoined);

			return oursHugs && prettierBreaks;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'inline_content_hug',
				confidence: 'likely',
				hunkIndices,
				reason: 'Expression breaks internally vs bracket breaks',
			};
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

		const prettierLines = ctx.prettier.split('\n');
		const inlineCloseTag = /<\/(?:span|a|strong|em|code|b|i|small|abbr|sub|sup)>/;

		// Check each hunk for prettier lines with long inline element lines
		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			const pLines = prettierLinesInHunk(prettierLines, hunk);
			return pLines.some((l) => inlineCloseTag.test(l) && visualWidth(l) > 100);
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'fill_after_inline',
				confidence: 'likely',
				hunkIndices,
				reason: 'Text after inline element breaks at print width',
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

		// For each hunk, check if it involves a whitespace-sensitive element
		// AND shows > placement differences
		const oursLines = ctx.ours.split('\n');
		const prettierLines = ctx.prettier.split('\n');

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			// Check for > on its own line in added lines (we break)
			const addedBreaksGt = hunk.addedLines.some((l) => /^\t*>$/.test(l));
			// Check for attr followed by > on same line in removed lines (prettier hugs)
			const removedHugsGt = hunk.removedLines.some((l) => /['"]\s*>/.test(l));

			if (!addedBreaksGt && !removedHugsGt) return false;

			// Verify context involves a whitespace-sensitive element
			// Check ours and prettier lines in hunk range for <pre or <textarea
			const wsElement = /<(?:pre|textarea)/i;
			const oLines = oursLinesInHunk(oursLines, hunk);
			const pLines = prettierLinesInHunk(prettierLines, hunk);
			const contextLines = hunk.lines.filter((l) => l.type === 'same').map((l) => l.line);

			return oLines.some((l) => wsElement.test(l)) ||
				pLines.some((l) => wsElement.test(l)) ||
				contextLines.some((l) => wsElement.test(l));
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'block_multiline_attrs_hug',
				confidence: 'likely',
				hunkIndices,
				reason: 'Block element with multiline attrs, we break > to new line',
			};
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

		const prettierLines = ctx.prettier.split('\n');
		const blockExprPattern = /\{#(?:if|each|await|key)/;

		// Check each hunk for block expressions that exceed 100 chars in prettier range
		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			const pLines = prettierLinesInHunk(prettierLines, hunk);
			return pLines.some(
				(l) => blockExprPattern.test(l) && visualWidth(l) > 100 && visualWidth(l) <= 110,
			);
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'short_expr_100',
				confidence: 'likely',
				hunkIndices,
				reason: 'Short expression in block condition exceeds 100 chars, we break',
			};
		}
		return null;
	},
};

// ─── Broad patterns (run last) ──────────────────────────────────────────────

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
		const prettierLines = ctx.prettier.split('\n');

		// Check each hunk for long CSS property values in prettier's range
		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			if (!isInCssContext(hunk, ctx)) return false;
			const pLines = prettierLinesInHunk(prettierLines, hunk);
			return pLines.some((l) => /^\t+[\w-]+:\s*.+/.test(l) && visualWidth(l) > 100);
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'css_value_wrap',
				confidence: 'likely',
				hunkIndices,
				reason: 'CSS property value wraps at print width',
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
		'css/values/lists/comma_space_separated_long_prettier_divergence',
		'svelte/elements/inline_element_fill_long_prettier_divergence',
		'svelte/elements/inline_component_fill_long_prettier_divergence',
	],
	detect(ctx) {
		const prettierLines = ctx.prettier.split('\n');
		let longestPrettierOverflow = 0;

		// For each hunk, check if prettier lines in that hunk's range exceed 100 chars
		// AND the difference looks like a print-width boundary divergence.
		// Two cases: (1) we produce more lines (broke the long line), or
		// (2) same/fewer lines but all our lines fit within 100 chars (rewrapped at print width).
		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			const pLines = prettierLinesInHunk(prettierLines, hunk);
			// >= 100: includes lines at exactly print width, since the divergence is
			// that prettier fills right up to the limit while we break earlier.
			const hasLongLine = pLines.some((l) => visualWidth(l) >= 100);
			if (!hasLongLine) return false;

			// Case 1: We have more lines (we broke prettier's long line)
			const weBreakMore = hunk.addedLines.length > hunk.removedLines.length;
			// Case 2: Same or fewer lines, but all our lines fit within print width
			const oursAllFit = hunk.addedLines.every((l) => visualWidth(l) <= 100);
			if (!weBreakMore && !oursAllFit) return false;

			for (const l of pLines) {
				const w = visualWidth(l);
				if (w >= 100) longestPrettierOverflow = Math.max(longestPrettierOverflow, w);
			}
			return true;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'fill_101_boundary',
				confidence: 'likely',
				hunkIndices,
				reason: `Prettier allows ${longestPrettierOverflow} chars, we break at print width`,
			};
		}
		return null;
	},
};

const commentPosition: DivergencePattern = {
	id: 'comment_position',
	description: 'Comment preserved where user placed it (Prettier relocates)',
	languages: ['typescript', 'svelte'],
	conformanceSections: ['TypeScript: Comments', 'Svelte: Attributes'],
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
		const jsCommentPattern = /\/\/|\/\*|\*\//;

		const hunkIndices = findMatchingHunks(ctx.hunks, (hunk) => {
			const addedCommentLines = hunk.addedLines.filter((l) => jsCommentPattern.test(l));
			const removedCommentLines = hunk.removedLines.filter((l) => jsCommentPattern.test(l));

			// At least one side must have comments
			if (addedCommentLines.length === 0 && removedCommentLines.length === 0) return false;

			// Case 1: Comment on one side only — verify it was MOVED (exists in
			// other side's full output), not incidentally included by reformatting.
			if (addedCommentLines.length > 0 && removedCommentLines.length === 0) {
				return addedCommentLines.some((l) => {
					const text = extractCommentContent(l);
					return text.length > 0 && ctx.prettier.includes(text);
				});
			}
			if (removedCommentLines.length > 0 && addedCommentLines.length === 0) {
				return removedCommentLines.some((l) => {
					const text = extractCommentContent(l);
					return text.length > 0 && ctx.ours.includes(text);
				});
			}

			// Case 2: Both sides have comments — verify the comment TEXT overlaps
			// AND the hunk is primarily about comment repositioning (non-comment
			// content should be similar). This prevents claiming hunks where the
			// real diff is code layout and comments are incidentally present.
			const addedTexts = addedCommentLines.map(extractCommentContent).sort();
			const removedTexts = removedCommentLines.map(extractCommentContent).sort();

			// Comment content must overlap (at least some comments have same text)
			const addedSet = new Set(addedTexts);
			const hasOverlap = removedTexts.some((t) => addedSet.has(t));
			if (!hasOverlap) return false;

			// Lines must differ (the comment moved positions)
			const linesDiffer = addedCommentLines.length !== removedCommentLines.length ||
				addedCommentLines.some((l, i) => l !== removedCommentLines[i]);
			if (!linesDiffer) return false;

			// Non-comment content must be similar — strip comments from both sides
			// and compare the trimmed non-empty lines. If the code itself changed
			// significantly, this is a formatting bug, not a comment position divergence.
			const stripComments = (line: string) =>
				line.replace(/\/\/.*$/, '').replace(/\/\*.*?\*\//g, '').trim();
			const addedCode = hunk.addedLines.map(stripComments).filter((l) => l.length > 0).sort();
			const removedCode = hunk.removedLines.map(stripComments).filter((l) => l.length > 0).sort();

			// If non-comment content is identical (same set of trimmed lines),
			// the hunk is purely about comment positioning — claim it.
			if (
				addedCode.length === removedCode.length &&
				addedCode.every((l, i) => l === removedCode[i])
			) {
				return true;
			}

			// If non-comment content differs, this is likely a code layout change
			// with incidental comments. Don't claim.
			return false;
		});

		if (hunkIndices.length > 0) {
			return {
				pattern: 'comment_position',
				confidence: 'likely',
				hunkIndices,
				reason: 'Comment preserved where user placed it (Prettier relocates)',
			};
		}
		return null;
	},
};

// ─── Pattern Registry ───────────────────────────────────────────────────────
//
// Ordered: specific → broad. Specific patterns run first for best explanations.
// Multiple patterns CAN claim the same hunk (by design).

export const PATTERNS: DivergencePattern[] = [
	// 1. Language-specific narrow patterns (certain or rare)
	bomStrip,
	selfClosingNonvoid,
	emptyStatementRemoval,
	cssValueRatio,

	// 2. CSS-specific patterns
	cssAtruleSpecSpacing,
	cssAtruleLongWrap,
	cssAtruleStableQuirk,
	cssSelectorDivergence,
	cssCommentStableQuirk,

	// 3. Feature-specific patterns
	templateLiteralWidth,
	blockExpressionLogical,
	singleSpecifierImport,
	memberExpressionCall,
	returnTypeGenericUnion,

	// 4. Svelte-specific patterns
	menuBlock,
	inlineContentHug,
	fillAfterInline,
	blockMultilineAttrsHug,
	shortExpr100,

	// 5. Broad patterns (run last)
	cssValueWrap,
	fill101Boundary,
	commentPosition,
];

/**
 * Detect which known divergence patterns explain the difference between
 * our formatter output and Prettier's output.
 *
 * Returns hunk-level coverage: which hunks are explained by patterns, which are not.
 *
 * @param ctx - Detection context (source, ours, prettier, diff, hunks, language)
 * @returns Hunk coverage result with classification
 */
export function detectDivergences(ctx: DetectionContext): HunkCoverageResult {
	const matches: DivergenceMatch[] = [];
	const { hunks } = ctx;

	for (const pattern of PATTERNS) {
		if (!pattern.languages.includes(ctx.language)) continue;

		const match = pattern.detect(ctx);
		if (match) {
			matches.push(match);
		}
	}

	// Compute hunk coverage
	const explainedHunks = new Set<number>();
	for (const match of matches) {
		for (const idx of match.hunkIndices) {
			explainedHunks.add(idx);
		}
	}

	const allHunkIndices = hunks.map((h) => h.index);
	const unexplainedHunks = allHunkIndices.filter((idx) => !explainedHunks.has(idx));

	let classification: HunkCoverageResult['classification'];
	if (matches.length === 0 || explainedHunks.size === 0) {
		classification = 'none_explained';
	} else if (unexplainedHunks.length === 0) {
		classification = 'all_explained';
	} else {
		classification = 'partial';
	}

	return {
		hunks,
		matches,
		explainedHunks,
		unexplainedHunks,
		classification,
	};
}
