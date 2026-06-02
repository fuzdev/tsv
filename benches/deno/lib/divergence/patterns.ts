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
	/** Pre-computed by enrich_detection_context — patterns use these instead of splitting */
	ours_lines?: string[];
	prettier_lines?: string[];
	/** Pre-computed <style> block line ranges for Svelte files */
	ours_style_boundaries?: Array<{ start: number; end: number }>;
	prettier_style_boundaries?: Array<{ start: number; end: number }>;
}

export interface DivergenceMatch {
	/** Pattern ID (matches conformance_prettier.md) */
	pattern: string;
	/** Detection confidence */
	confidence: 'certain' | 'likely' | 'possible';
	/** Indices of hunks this pattern explains */
	hunk_indices: number[];
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
	conformance_sections: string[];
	/** Fixture paths (relative to tests/fixtures/) this pattern should detect */
	fixtures: string[];
	/** Detection function */
	detect: (ctx: DetectionContext) => DivergenceMatch | null;
}

/**
 * Calculate visual width of a line (tabs = 2 spaces).
 */
function visual_width(line: string): number {
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
	explained_hunks: Set<number>;
	/** Hunk indices not explained by any pattern */
	unexplained_hunks: number[];
	/** Overall classification */
	classification: 'all_explained' | 'partial' | 'none_explained';
}

/**
 * Find hunk indices where the predicate matches.
 */
function find_matching_hunks(hunks: DiffHunk[], predicate: (h: DiffHunk) => boolean): number[] {
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
function prettier_lines_in_hunk(prettier_lines: string[], hunk: DiffHunk): string[] {
	if (!hunk.prettier_range) return [];
	return prettier_lines.slice(hunk.prettier_range.start, hunk.prettier_range.end + 1);
}

/**
 * Get ours lines within a hunk's ours range.
 */
function ours_lines_in_hunk(ours_lines: string[], hunk: DiffHunk): string[] {
	if (!hunk.ours_range) return [];
	return ours_lines.slice(hunk.ours_range.start, hunk.ours_range.end + 1);
}

/**
 * Compute <style> block line ranges from an array of lines.
 * Returns an array of { start, end } (inclusive line indices).
 */
function compute_style_boundaries(lines: string[]): Array<{ start: number; end: number }> {
	const boundaries: Array<{ start: number; end: number }> = [];
	let style_start = -1;

	for (let i = 0; i < lines.length; i++) {
		if (/<style[\s>]/.test(lines[i]) && style_start === -1) {
			style_start = i;
		} else if (/<\/style>/.test(lines[i]) && style_start !== -1) {
			boundaries.push({ start: style_start, end: i });
			style_start = -1;
		}
	}

	return boundaries;
}

/**
 * Check if a line index falls within any style block boundary.
 */
function is_line_in_style_block(
	line: number,
	boundaries: Array<{ start: number; end: number }>,
): boolean {
	for (const b of boundaries) {
		if (line >= b.start && line <= b.end) return true;
	}
	return false;
}

/**
 * Pre-compute cached fields on a DetectionContext.
 * Called by detect_divergences before running patterns.
 */
export function enrich_detection_context(ctx: DetectionContext): void {
	ctx.ours_lines = ctx.ours.split('\n');
	ctx.prettier_lines = ctx.prettier.split('\n');
	if (ctx.language === 'svelte') {
		ctx.ours_style_boundaries = compute_style_boundaries(ctx.ours_lines);
		ctx.prettier_style_boundaries = compute_style_boundaries(ctx.prettier_lines);
	} else {
		ctx.ours_style_boundaries = [];
		ctx.prettier_style_boundaries = [];
	}
}

/**
 * Check if a hunk's context is within a CSS context.
 * For Svelte files, uses pre-computed style boundaries.
 * For removal-only hunks, checks prettier's boundaries (not ours).
 */
function is_in_css_context(hunk: DiffHunk, ctx: DetectionContext): boolean {
	if (ctx.language === 'css') return true;
	if (ctx.language !== 'svelte') return false;

	// Use ours range when available; for removal-only hunks, use prettier range
	// against prettier's style boundaries (fixes line index mismatch)
	if (hunk.ours_range) {
		return is_line_in_style_block(hunk.ours_range.start, ctx.ours_style_boundaries ?? []);
	}
	if (hunk.prettier_range) {
		return is_line_in_style_block(hunk.prettier_range.start, ctx.prettier_style_boundaries ?? []);
	}
	return false;
}

/**
 * Extract comment text content from a line (strip delimiters and whitespace).
 * Returns the text inside the comment, ignoring surrounding code.
 */
function extract_comment_content(line: string): string {
	// Line comment: extract text after //
	const line_comment = line.match(/\/\/\s*(.*)/);
	if (line_comment) return line_comment[1].trim();
	// Block comment: extract text inside /* */
	const block_comment = line.match(/\/\*\s*(.*?)\s*\*\//);
	if (block_comment) return block_comment[1].trim();
	// Partial block comment (opening or closing only)
	const block_open = line.match(/\/\*\s*(.*)/);
	if (block_open) return block_open[1].trim();
	const block_close = line.match(/(.*?)\s*\*\//);
	if (block_close) return block_close[1].trim();
	return line.trim();
}

/**
 * Check if a comment with the given text content exists in the output.
 * Searches for the text preceded by comment delimiters rather than matching
 * the bare text anywhere — prevents "map" from matching `arr.map(...)`.
 */
function comment_exists_in_output(output: string, text: string): boolean {
	return output.includes(`// ${text}`) ||
		output.includes(`/* ${text}`) ||
		output.includes(` * ${text}`);
}

// ─── Pattern Detectors ──────────────────────────────────────────────────────
//
// Ordered from most specific/narrow to most broad.
// Specific patterns run first so hunks get the most precise explanation.

// ─── Language-specific narrow patterns ──────────────────────────────────────

const bom_strip: DivergencePattern = {
	id: 'bom_strip',
	description: 'BOM (byte order mark) removed',
	languages: ['svelte', 'typescript', 'css'],
	conformance_sections: ['Whitespace: BOM Handling'],
	fixtures: [
		'svelte/syntax/whitespace/bom_prettier_divergence',
		'css/tokens/whitespace/bom_prettier_divergence',
		'typescript/syntax/whitespace/bom_prettier_divergence',
	],
	detect(ctx) {
		// Source starts with BOM, our output doesn't
		if (ctx.source.startsWith('﻿') && !ctx.ours.startsWith('﻿')) {
			// Verify prettier keeps BOM
			if (ctx.prettier.startsWith('﻿')) {
				// BOM difference is always in hunk 0 (first line)
				const hunk_indices = ctx.hunks.length > 0 ? [0] : [];
				return {
					pattern: 'bom_strip',
					confidence: 'certain',
					hunk_indices,
					reason: 'BOM (byte order mark) removed',
				};
			}
		}
		return null;
	},
};

const self_closing_nonvoid: DivergencePattern = {
	id: 'self_closing_nonvoid',
	description: 'Non-void HTML element self-closing normalization',
	languages: ['svelte'],
	conformance_sections: ['Svelte/HTML'],
	fixtures: ['svelte/elements/self_closing_nonvoid_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Two directions:
		// 1. Components: ours normalizes <Component></Component> → <Component />
		//    (ours adds self-closing, prettier has explicit close)
		// 2. HTML elements: ours normalizes <div /> → <div></div>
		//    (prettier has self-closing, ours has explicit close)
		//
		// Tag name matching required: a self-closing <Foo /> in one side must
		// have a matching </Foo> in the other side. Without this, wrapping diffs
		// that incidentally contain self-closing components (e.g. <Glyph />) and
		// unrelated close tags (e.g. </ProviderLink>) would false-positive.

		// Multiline elements: /> on its own line, ></tag> on the other
		const self_closing_end = /^\s*\/>\s*$/;
		const explicit_close_end = />\s*<\/[a-zA-Z][\w.-]*>\s*$/;

		// Orphaned hunk patterns: when <div /> → <div></div> has an identical
		// <div></div> between them, the diff algorithm splits the change into
		// two hunks (one remove-only, one add-only). Match these individually.
		const self_closing_nonvoid_tag = /<([a-z][\w.-]*)\s*\/>/; // lowercase = HTML element
		const empty_explicit_close = /<([a-z][\w.-]*)(\s[^>]*)?>(\s*)<\/\1>/; // <tag></tag>

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Full-tag: require self-closing <Tag /> on one side and </Tag> on other
			// Covers both directions (components and HTML elements)
			for (
				const [self_lines, close_lines] of [
					[hunk.added_lines, hunk.removed_lines],
					[hunk.removed_lines, hunk.added_lines],
				]
			) {
				for (const line of self_lines) {
					const re = /<([a-zA-Z][\w.-]*)[^>]*\/>/g;
					let m;
					while ((m = re.exec(line)) !== null) {
						const tag_name = m[1].replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
						if (close_lines.some((l) => new RegExp(`</${tag_name}\\b`).test(l))) {
							return true;
						}
					}
				}
			}
			// Multiline: /> on its own line ↔ ></tag> (inherently paired by position)
			if (
				hunk.removed_lines.some((l) => self_closing_end.test(l)) &&
				hunk.added_lines.some((l) => explicit_close_end.test(l))
			) return true;
			if (
				hunk.added_lines.some((l) => self_closing_end.test(l)) &&
				hunk.removed_lines.some((l) => explicit_close_end.test(l))
			) return true;
			// Orphaned remove-only: prettier has self-closing non-void HTML that we removed
			if (
				hunk.added_lines.length === 0 &&
				hunk.removed_lines.every((l) => self_closing_nonvoid_tag.test(l))
			) return true;
			// Orphaned add-only: we added empty explicit-close HTML that prettier didn't have
			if (
				hunk.removed_lines.length === 0 &&
				hunk.added_lines.every((l) => empty_explicit_close.test(l))
			) return true;
			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'self_closing_nonvoid',
				confidence: 'likely',
				hunk_indices,
				reason: 'Non-void HTML element self-closing normalization',
			};
		}
		return null;
	},
};

const empty_statement_removal: DivergencePattern = {
	id: 'empty_statement_removal',
	description: 'Standalone empty statement (;) removed',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: ['typescript/statements/empty_standalone_prettier_divergence'],
	detect(ctx) {
		// Look for hunks where removed lines contain standalone semicolons
		// (not part of for(;;) or other syntax)
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Removed lines should have standalone ; that we remove
			const removed_standalone = hunk.removed_lines.some((l) => /^\t*;$/.test(l));
			// Added lines should NOT have standalone ;
			const added_standalone = hunk.added_lines.some((l) => /^\t*;$/.test(l));
			return removed_standalone && !added_standalone;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'empty_statement_removal',
				confidence: 'certain',
				hunk_indices,
				reason: 'Standalone empty statement (;) removed',
			};
		}
		return null;
	},
};

const css_value_ratio: DivergencePattern = {
	id: 'css_value_ratio',
	description: 'Ratio spacing normalized in CSS',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: Values'],
	fixtures: ['css/values/ratio/ratio_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		// Look for ratio patterns (digit / digit) with spacing differences
		const ratio_pattern = /\d+\s*\/\s*\d+/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;

			const removed_has_ratio = hunk.removed_lines.some((l) => ratio_pattern.test(l));
			const added_has_ratio = hunk.added_lines.some((l) => ratio_pattern.test(l));
			if (!removed_has_ratio || !added_has_ratio) return false;

			// Check for spacing differences around /
			const removed_spacing = hunk.removed_lines.some((l) => /\d+\s{2,}\/|\/ {2,}\d+/.test(l));
			const added_normalized = hunk.added_lines.some((l) => /\d+ \/ \d+/.test(l));
			return removed_spacing && added_normalized;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_value_ratio',
				confidence: 'likely',
				hunk_indices,
				reason: 'Ratio spacing normalized in CSS',
			};
		}
		return null;
	},
};

// ─── CSS-specific patterns ──────────────────────────────────────────────────

const css_atrule_spec_spacing: DivergencePattern = {
	id: 'css_atrule_spec_spacing',
	description: 'CSS at-rule keyword spacing normalized per spec',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: At-Rules'],
	fixtures: [
		'css/at_rules/container_spacing_prettier_divergence',
		'css/at_rules/media_boolean_spacing_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		// Detect missing space before ( after boolean keywords: and(, or(, not(
		// Also detect style( vs style ( in container queries
		const missing_space = /(?:and|or|not|style)\(/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;

			// Removed lines (prettier) have and( or or( without space
			const removed_missing_space = hunk.removed_lines.some((l) => missing_space.test(l));
			// Added lines (ours) have and ( or or ( with space
			const added_has_space = hunk.added_lines.some((l) => /(?:and|or|not|style) \(/.test(l));

			// Also check the reverse: we normalize spacing where prettier doesn't
			const removed_has_atrule = hunk.removed_lines.some((l) =>
				/@(?:container|media|supports)/.test(l)
			);
			const added_has_atrule = hunk.added_lines.some((l) =>
				/@(?:container|media|supports)/.test(l)
			);

			return (removed_missing_space && added_has_space) ||
				(removed_has_atrule && added_has_atrule && removed_missing_space);
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_atrule_spec_spacing',
				confidence: 'certain',
				hunk_indices,
				reason: 'CSS at-rule keyword spacing normalized per spec (CSS Syntax 3 §4.3.4)',
			};
		}
		return null;
	},
};

const css_atrule_long_wrap: DivergencePattern = {
	id: 'css_atrule_long_wrap',
	description: 'CSS at-rule wraps at print width',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: At-Rules'],
	fixtures: [
		'css/at_rules/container_long_prettier_divergence',
		'css/at_rules/media_long_prettier_divergence',
		'css/at_rules/import_media_query_long_prettier_divergence',
		'css/at_rules/supports_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const prettier_lines = ctx.prettier_lines!;
		const atrule_pattern = /@(?:container|media|import|supports)/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;

			// Prettier's removed lines have a long at-rule that exceeds 100 chars
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			const has_long_atrule = p_lines.some(
				(l) => atrule_pattern.test(l) && visual_width(l) > 100,
			);
			if (!has_long_atrule) return false;

			// We have more lines (we wrapped)
			return hunk.added_lines.length > hunk.removed_lines.length;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_atrule_long_wrap',
				confidence: 'likely',
				hunk_indices,
				reason: 'CSS at-rule wraps at print width',
			};
		}
		return null;
	},
};

const css_atrule_stable_quirk: DivergencePattern = {
	id: 'css_atrule_stable_quirk',
	description: 'CSS at-rule stable quirk (Prettier preserves multiple forms)',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: At-Rules'],
	fixtures: [
		'css/at_rules/layer_list_prettier_divergence',
		'css/at_rules/scope_complex_prettier_divergence',
		'css/at_rules/scope_selector_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;

			const removed_joined = hunk.removed_lines.join('\n');
			const added_joined = hunk.added_lines.join('\n');

			// @layer with spacing quirks (extra spaces after commas)
			if (/@layer/.test(removed_joined) || /@layer/.test(added_joined)) {
				const removed_extra_spaces = hunk.removed_lines.some((l) =>
					/@layer/.test(l) && /,\s{2,}/.test(l)
				);
				const added_normalized = hunk.added_lines.some((l) =>
					/@layer/.test(l) && /, [^\s]/.test(l)
				);
				if (removed_extra_spaces && added_normalized) return true;
			}

			// @scope with spacing quirks (spaces inside parens, double spaces around to)
			if (/@scope/.test(removed_joined) || /@scope/.test(added_joined)) {
				// Prettier adds spaces inside scope parens: ( .class ) vs (.class)
				const removed_has_quirk = hunk.removed_lines.some((l) =>
					/@scope/.test(l) && (/\( /.test(l) || / \)/.test(l) || /\s{2,}to\s{2,}/.test(l))
				);
				const added_is_normal = hunk.added_lines.some((l) => /@scope/.test(l));
				if (removed_has_quirk && added_is_normal) return true;
			}

			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_atrule_stable_quirk',
				confidence: 'likely',
				hunk_indices,
				reason: 'CSS at-rule stable quirk (Prettier preserves multiple forms, we normalize)',
			};
		}
		return null;
	},
};

const css_selector_divergence: DivergencePattern = {
	id: 'css_selector_divergence',
	description: 'CSS selector formatting divergence',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: Selectors'],
	fixtures: [
		'css/selectors/combinators/column_prettier_divergence',
		'css/selectors/pseudo_class/nth_child_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;

			// Column combinator: || with/without spaces in CSS selectors
			const removed_has_compact = hunk.removed_lines.some((l) => /\w\|\|\w/.test(l) && /{/.test(l));
			const added_has_spaced = hunk.added_lines.some((l) => /\w \|\| \w/.test(l) && /{/.test(l));
			if (removed_has_compact && added_has_spaced) return true;

			// nth-child An+B normalization: spacing differences around operators
			const nth_pattern = /:nth-(?:child|last-child|of-type|last-of-type)\(/;
			const removed_has_nth = hunk.removed_lines.some((l) => nth_pattern.test(l));
			const added_has_nth = hunk.added_lines.some((l) => nth_pattern.test(l));
			if (removed_has_nth && added_has_nth) {
				// Check for spacing difference in the An+B expression
				const removed_nth_content = hunk.removed_lines.filter((l) => nth_pattern.test(l));
				const added_nth_content = hunk.added_lines.filter((l) => nth_pattern.test(l));
				if (
					removed_nth_content.length > 0 && added_nth_content.length > 0 &&
					removed_nth_content.some((l, i) =>
						added_nth_content[i] &&
						l.replace(/\s+/g, '') === added_nth_content[i].replace(/\s+/g, '')
					)
				) {
					return true;
				}
			}

			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_selector_divergence',
				confidence: 'likely',
				hunk_indices,
				reason: 'CSS selector formatting divergence',
			};
		}
		return null;
	},
};

const css_comment_stable_quirk: DivergencePattern = {
	id: 'css_comment_stable_quirk',
	description: 'CSS comment position stable quirk (Prettier preserves multiple forms)',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: Comments'],
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

		const comment_pattern = /\/\*.*?\*\/|\/\*|\*\//;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;

			// Both sides have CSS comments, but position/spacing differs
			const added_has_comment = hunk.added_lines.some((l) => comment_pattern.test(l));
			const removed_has_comment = hunk.removed_lines.some((l) => comment_pattern.test(l));

			if (!added_has_comment && !removed_has_comment) return false;

			// Extract comment text from both sides and verify content is the same
			// (only position/spacing should differ, not content)
			const single_line_comment = /\/\*(.*?)\*\//;
			const added_comment_texts = hunk.added_lines
				.filter((l) => comment_pattern.test(l))
				.map((l) => {
					const m = l.match(single_line_comment);
					return m ? m[1].trim() : '';
				});
			const removed_comment_texts = hunk.removed_lines
				.filter((l) => comment_pattern.test(l))
				.map((l) => {
					const m = l.match(single_line_comment);
					return m ? m[1].trim() : '';
				});

			// Comment content should be the same - only position differs
			if (added_comment_texts.length === 0 && removed_comment_texts.length === 0) return false;

			// If one side has comment and other doesn't, verify the comment text
			// exists in the other side's full output (it was moved, not incidentally included).
			// Require minimum text length to avoid short strings matching accidentally.
			if (added_has_comment && !removed_has_comment) {
				const texts = added_comment_texts.filter((t) => t.length >= 2);
				return texts.length > 0 && texts.some((t) => comment_exists_in_output(ctx.prettier, t));
			}
			if (removed_has_comment && !added_has_comment) {
				const texts = removed_comment_texts.filter((t) => t.length >= 2);
				return texts.length > 0 && texts.some((t) => comment_exists_in_output(ctx.ours, t));
			}

			// Both have comments - verify same content, different position
			if (added_comment_texts.length > 0 && removed_comment_texts.length > 0) {
				const added_set = new Set(added_comment_texts);
				const removed_set = new Set(removed_comment_texts);
				// At least some comment content overlaps
				const has_overlap = [...added_set].some((t) => removed_set.has(t));
				if (has_overlap) {
					// Lines differ (position change)
					const added_comment_lines = hunk.added_lines.filter((l) => comment_pattern.test(l));
					const removed_comment_lines = hunk.removed_lines.filter((l) => comment_pattern.test(l));
					return added_comment_lines.some((l, i) => l !== removed_comment_lines[i]);
				}
			}

			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_comment_stable_quirk',
				confidence: 'likely',
				hunk_indices,
				reason: 'CSS comment position stable quirk (we normalize)',
			};
		}
		return null;
	},
};

// ─── Feature-specific patterns ──────────────────────────────────────────────

const template_literal_width: DivergencePattern = {
	id: 'template_literal_width',
	description: 'Template literal interpolation breaks to respect print width',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript: Template Literals'],
	fixtures: [
		'typescript/expressions/literals/template/long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_expression_long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_multiline_indent_long_prettier_divergence',
		'typescript/expressions/literals/template/interpolation_nested_template_prettier_divergence',
		'typescript/types/template_literal_type_long_prettier_divergence',
		'typescript/types/template_literal_type_conditional_long_prettier_divergence',
		'typescript/expressions/ternary/template_consequent_long_prettier_divergence',
		'typescript/expressions/logical/template_operand_long_prettier_divergence',
	],
	detect(ctx) {
		// Template literal break patterns — we break inside ${...} to respect print width.
		// Detect by looking for lines that END with ${ (the break point) or start with }`
		// (closing after break). Must use end-of-line anchor to avoid matching inline ${expr}
		// which appears in both our output and prettier's output.
		const break_after_dollar_brace = /\$\{\s*$/;
		const closing_brace_backtick = /^\t+\}\`/;

		// Simple expression on its own line: identifier or member chain (a.b.c, a?.b)
		// These are expressions Prettier atomizes (pre-renders at infinite width).
		const simple_expr_line = /^\t+(\w+(?:[.?]+\w+)*)\s*$/;

		const prettier_lines = ctx.prettier_lines!;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const added_has_break = hunk.added_lines.some(
				(l) => break_after_dollar_brace.test(l) || closing_brace_backtick.test(l),
			);
			const removed_has_break = hunk.removed_lines.some(
				(l) => break_after_dollar_brace.test(l) || closing_brace_backtick.test(l),
			);

			// Case 1: Only our side has template breaks — verify the break is
			// plausibly width-motivated by checking that prettier's corresponding
			// line is near print width (>80 chars). Without this, a bug that
			// incorrectly breaks a short template literal would be claimed.
			if (added_has_break && !removed_has_break) {
				const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
				return p_lines.some((l) => visual_width(l) > 80);
			}

			// Case 2: Both sides break at ${} boundaries, but at different interpolations.
			// Prettier atomizes simple expressions (Identifier, MemberExpression) so they
			// stay inline, then breaks at a different ${} if needed. We break the simple
			// expression instead (or vice versa — either side can have the simple expression
			// broken). Detect by finding isolated simple expressions on one side that appear
			// inline as ${expr} on the other side.
			if (added_has_break && removed_has_break) {
				// Check ours→prettier: simple expr in added, inline in removed
				for (const line of hunk.added_lines) {
					const m = simple_expr_line.exec(line);
					if (m) {
						const expr = m[1];
						if (hunk.removed_lines.some((l) => l.includes(`\${${expr}}`))) {
							return true;
						}
					}
				}
				// Check prettier→ours: simple expr in removed, inline in added
				for (const line of hunk.removed_lines) {
					const m = simple_expr_line.exec(line);
					if (m) {
						const expr = m[1];
						if (hunk.added_lines.some((l) => l.includes(`\${${expr}}`))) {
							return true;
						}
					}
				}
			}

			return false;
		});

		if (hunk_indices.length > 0 && ctx.source.includes('${')) {
			return {
				pattern: 'template_literal_width',
				confidence: 'likely',
				hunk_indices,
				reason: 'Template interpolation breaks to respect print width',
			};
		}
		return null;
	},
};

const block_expression_logical: DivergencePattern = {
	id: 'block_expression_logical',
	description: 'Block expression logical operators wrap to respect print width',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Blocks'],
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
		const block_operator_break = /^\t+(?:&&|\|\|)/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			return hunk.added_lines.some((l) => block_operator_break.test(l)) &&
				!hunk.removed_lines.some((l) => block_operator_break.test(l));
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'block_expression_logical',
				confidence: 'likely',
				hunk_indices,
				reason: 'Logical expression in block condition broken to respect print width',
			};
		}
		return null;
	},
};

const single_specifier_import: DivergencePattern = {
	id: 'single_specifier_import',
	description: 'Single-specifier import wraps at print width',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: ['typescript/modules/imports/single_specifier_long_prettier_divergence'],
	detect(ctx) {
		// Check hunks for import statement differences
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Added lines show multiline import (we break)
			const added_has_import = hunk.added_lines.some((l) => /^import \{/.test(l));
			// Removed lines show single-line import (prettier keeps inline)
			const removed_has_long_import = hunk.removed_lines.some(
				(l) => /^import \{/.test(l) && visual_width(l) > 100,
			);
			return added_has_import && removed_has_long_import;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'single_specifier_import',
				confidence: 'likely',
				hunk_indices,
				reason: 'Single specifier import wraps at print width',
			};
		}
		return null;
	},
};

const member_expression_call: DivergencePattern = {
	id: 'member_expression_call',
	description: 'Member expression in call args breaks differently',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: ['typescript/modules/imports/path_calls_long_prettier_divergence'],
	detect(ctx) {
		const module_patterns = /(?:require\.resolve(?:\.paths)?|import\.meta\.resolve)\(/;

		if (!module_patterns.test(ctx.source)) return null;

		// Map to specific hunks that contain the module pattern
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			return hunk.added_lines.some((l) => module_patterns.test(l)) ||
				hunk.removed_lines.some((l) => module_patterns.test(l));
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'member_expression_call',
				confidence: 'possible',
				hunk_indices,
				reason: 'Member expression in call args breaks differently',
			};
		}
		return null;
	},
};

const return_type_generic_union: DivergencePattern = {
	id: 'return_type_generic_union',
	description: 'Return type generic with union wraps at print width',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: [
		'typescript/declarations/function/return_type_generic_union_long_prettier_divergence',
	],
	detect(ctx) {
		const prettier_lines = ctx.prettier_lines!;

		// Look for generic types with union (| null, | void, | undefined) in hunks
		// where prettier's line exceeds 100 chars
		const union_in_generic = /[<>].*\|\s*(?:null|void|undefined)/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			// Prettier has a long line with generic union
			const has_long_generic_union = p_lines.some(
				(l) => union_in_generic.test(l) && visual_width(l) > 100,
			);
			if (!has_long_generic_union) return false;

			// We break (more lines in our version)
			return hunk.added_lines.length > hunk.removed_lines.length;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'return_type_generic_union',
				confidence: 'likely',
				hunk_indices,
				reason: 'Return type generic with union wraps at print width',
			};
		}
		return null;
	},
};

// ─── Svelte-specific patterns ───────────────────────────────────────────────

const menu_block: DivergencePattern = {
	id: 'menu_block',
	description: '<menu> treated as block element (spec-compliant)',
	languages: ['svelte'],
	conformance_sections: ['Svelte/HTML'],
	fixtures: ['svelte/elements/menu_block_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Look for hunks involving <menu> elements where prettier hugs content
		// (inline formatting) and we expand it (block formatting)
		const ours_lines = ctx.ours_lines!;
		const prettier_lines = ctx.prettier_lines!;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Check for </menu in removed lines (prettier hugs: content</menu on same line,
			// with > possibly on next line)
			const removed_has_menu_close = hunk.removed_lines.some((l) => /<\/menu/.test(l));
			// Check for </menu> on added lines on its own line (we expand: block formatting)
			const added_has_menu_close = hunk.added_lines.some((l) => /^\s*<\/menu>/.test(l));

			if (removed_has_menu_close || added_has_menu_close) return true;

			// Also check context: <menu in surrounding lines
			const o_lines = ours_lines_in_hunk(ours_lines, hunk);
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			const context_lines = hunk.lines.filter((l) => l.type === 'same').map((l) => l.line);
			const all_lines = [...o_lines, ...p_lines, ...context_lines];
			const has_menu_element = all_lines.some((l) => /<menu[\s>]/.test(l));

			if (!has_menu_element) return false;

			// Prettier hugs: >{content} on same line as attribute
			const removed_hugs = hunk.removed_lines.some((l) => />[^<\n]*<\/menu/.test(l));
			// We expand: > on own line
			const added_breaks_gt = hunk.added_lines.some((l) => /^\t*>$/.test(l));

			return removed_hugs || added_breaks_gt;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'menu_block',
				confidence: 'certain',
				hunk_indices,
				reason: '<menu> treated as block element (prettier treats as inline)',
			};
		}
		return null;
	},
};

const inline_content_hug: DivergencePattern = {
	id: 'inline_content_hug',
	description: 'Expression breaks internally vs bracket breaks',
	languages: ['svelte'],
	conformance_sections: ['Svelte/HTML'],
	fixtures: ['svelte/elements/inline_content_hug_long_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// For each hunk, check if removed lines show tag breaks (prettier breaks tag)
		// while added lines show >{ hugging (we hug content)
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const added_joined = hunk.added_lines.join('\n');
			const removed_joined = hunk.removed_lines.join('\n');

			// Our added lines hug: >{ or > followed by content
			const ours_hugs = />\{/.test(added_joined) || />[^<\n]+\{/.test(added_joined);
			// Prettier removed lines show tag break:
			//   - > alone on a line (tag break with content on next line)
			//   - >content on a line (tag break with content on same line, e.g. <small\n\t>text{expr})
			//   - removed content ending with > (tag with > at end of line)
			// Exclude closing tags (>/) to avoid matching </tag>
			const prettier_breaks = hunk.removed_lines.some((l) => /^\s*>(?!\/)/.test(l)) ||
				/>\s*$/.test(removed_joined);

			return ours_hugs && prettier_breaks;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'inline_content_hug',
				confidence: 'likely',
				hunk_indices,
				reason: 'Expression breaks internally vs bracket breaks',
			};
		}
		return null;
	},
};

const fill_after_inline: DivergencePattern = {
	id: 'fill_after_inline',
	description: 'Text after inline element breaks at print width',
	languages: ['svelte'],
	conformance_sections: ['Svelte/HTML'],
	fixtures: [
		'svelte/elements/fill_after_inline_prettier_divergence',
		'svelte/elements/fill_multiple_expr_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		const prettier_lines = ctx.prettier_lines!;
		const inline_close_tag =
			/<\/(?:span|a|strong|em|code|b|i|small|abbr|sub|sup|mark|cite|q|time|data|kbd|samp|var|dfn|ins|del|u|s)>/;

		// Check each hunk for prettier lines with long inline element lines
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			return p_lines.some((l) => inline_close_tag.test(l) && visual_width(l) > 100);
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'fill_after_inline',
				confidence: 'likely',
				hunk_indices,
				reason: 'Text after inline element breaks at print width',
			};
		}
		return null;
	},
};

const block_multiline_attrs_hug: DivergencePattern = {
	id: 'block_multiline_attrs_hug',
	description: 'Block element with multiline attrs, we break >',
	languages: ['svelte'],
	conformance_sections: ['Svelte/HTML'],
	fixtures: ['svelte/elements/block_multiline_attrs_content_hug_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// For each hunk, check if it involves a whitespace-sensitive element
		// AND shows > placement differences
		const ours_lines = ctx.ours_lines!;
		const prettier_lines = ctx.prettier_lines!;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Check for > on its own line in added lines (we break)
			const added_breaks_gt = hunk.added_lines.some((l) => /^\t*>$/.test(l));
			// Check for attr followed by > on same line in removed lines (prettier hugs)
			const removed_hugs_gt = hunk.removed_lines.some((l) => /['"]\s*>/.test(l));

			if (!added_breaks_gt && !removed_hugs_gt) return false;

			// Verify context involves a whitespace-sensitive element
			// Check ours and prettier lines in hunk range for <pre or <textarea
			const ws_element = /<(?:pre|textarea)/i;
			const o_lines = ours_lines_in_hunk(ours_lines, hunk);
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			const context_lines = hunk.lines.filter((l) => l.type === 'same').map((l) => l.line);

			return o_lines.some((l) => ws_element.test(l)) ||
				p_lines.some((l) => ws_element.test(l)) ||
				context_lines.some((l) => ws_element.test(l));
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'block_multiline_attrs_hug',
				confidence: 'likely',
				hunk_indices,
				reason: 'Block element with multiline attrs, we break > to new line',
			};
		}
		return null;
	},
};

const short_expr_100: DivergencePattern = {
	id: 'short_expr_100',
	description: 'Short expression in block exceeds 100 chars, we break',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Blocks'],
	fixtures: ['svelte/blocks/if/in_inline_element_long_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		const prettier_lines = ctx.prettier_lines!;
		const block_expr_pattern = /\{#(?:if|each|await|key)/;

		// Check each hunk for block expressions that exceed 100 chars in prettier range
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			return p_lines.some(
				(l) => block_expr_pattern.test(l) && visual_width(l) > 100 && visual_width(l) <= 110,
			);
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'short_expr_100',
				confidence: 'likely',
				hunk_indices,
				reason: 'Short expression in block condition exceeds 100 chars, we break',
			};
		}
		return null;
	},
};

// ─── Broad patterns (run last) ──────────────────────────────────────────────

const css_value_wrap: DivergencePattern = {
	id: 'css_value_wrap',
	description: 'CSS property value wraps at print width',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: Values'],
	fixtures: [
		'css/values/functions/transform_long_prettier_divergence',
		'css/values/lists/space_separated_long_wrap_prettier_divergence',
	],
	detect(ctx) {
		const prettier_lines = ctx.prettier_lines!;

		// Check each hunk for long CSS property values in prettier's range
		// AND verify we actually wrapped (more lines than prettier)
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			const has_long_property = p_lines.some(
				(l) => /^\t+[\w-]+:\s*.+/.test(l) && visual_width(l) > 100,
			);
			if (!has_long_property) return false;
			// We must have more lines (we wrapped the long value)
			return hunk.added_lines.length > hunk.removed_lines.length;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_value_wrap',
				confidence: 'likely',
				hunk_indices,
				reason: 'CSS property value wraps at print width',
			};
		}
		return null;
	},
};

const fill_101_boundary: DivergencePattern = {
	id: 'fill_101_boundary',
	description: 'Prettier allows lines to exceed print width, we break',
	languages: ['svelte', 'typescript', 'css'],
	conformance_sections: ['CSS: Layout', 'CSS: Values', 'Svelte/HTML', 'TypeScript'],
	fixtures: [
		'css/comma_separated_greedy_fill_prettier_divergence',
		'css/values/lists/comma_space_separated_long_prettier_divergence',
		'svelte/elements/inline_element_fill_long_prettier_divergence',
		'svelte/elements/inline_component_fill_long_prettier_divergence',
		'svelte/elements/fill_expr_break_boundary_long_prettier_divergence',
		'svelte/attributes/multiline_value_inline_long_prettier_divergence',
	],
	detect(ctx) {
		const prettier_lines = ctx.prettier_lines!;
		let longest_prettier_overflow = 0;

		// For each hunk, check if prettier lines in that hunk's range exceed 100 chars
		// AND the difference looks like a print-width boundary divergence.
		// Two cases: (1) we produce more lines (broke the long line), or
		// (2) same/fewer lines but all our lines fit within 100 chars (rewrapped at print width).
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			// >= 100: includes lines at exactly print width, since the divergence is
			// that prettier fills right up to the limit while we break earlier.
			const has_long_line = p_lines.some((l) => visual_width(l) >= 100);
			if (!has_long_line) return false;

			// Case 1: We have more lines (we broke prettier's long line)
			const we_break_more = hunk.added_lines.length > hunk.removed_lines.length;
			// Case 2: Same or fewer lines, but all our lines fit within print width
			const ours_all_fit = hunk.added_lines.every((l) => visual_width(l) <= 100);
			if (!we_break_more && !ours_all_fit) return false;

			for (const l of p_lines) {
				const w = visual_width(l);
				if (w >= 100) longest_prettier_overflow = Math.max(longest_prettier_overflow, w);
			}
			return true;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'fill_101_boundary',
				confidence: 'likely',
				hunk_indices,
				reason: `Prettier allows ${longest_prettier_overflow} chars, we break at print width`,
			};
		}
		return null;
	},
};

const comment_position: DivergencePattern = {
	id: 'comment_position',
	description: 'Comment preserved where user placed it (Prettier relocates)',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript: Comments', 'Svelte: Attributes'],
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
		'typescript/statements/try/line_comment_absorbed_prettier_divergence',
		'typescript/statements/labeled/comment_prettier_divergence',
		'typescript/statements/if/else_block_own_line_comment_prettier_divergence',
		'typescript/statements/while/line_before_body_comment_prettier_divergence',
		'typescript/statements/while/absorbed_body_comment_prettier_divergence',
		'typescript/statements/do_while/line_before_while_comment_prettier_divergence',
		// TypeScript chain comments
		'typescript/expressions/calls/chained/trailing_member_comment_prettier_divergence',
		'typescript/expressions/calls/chained/trailing_member_computed_comment_prettier_divergence',
		// Call open paren `(` trailing comment kept on the `(` line
		'typescript/expressions/calls/open_paren_comment_prettier_divergence',
		'typescript/expressions/calls/chain_open_paren_comment_prettier_divergence',
		'typescript/expressions/calls/new_open_paren_comment_prettier_divergence',
		// Object/array literal + block body open-delimiter trailing comment kept on the delimiter line
		'typescript/expressions/objects/open_brace_comment_prettier_divergence',
		'typescript/expressions/arrays/open_bracket_comment_prettier_divergence',
		'typescript/statements/block_open_brace_comment_prettier_divergence',
		// Type-parameter `<` + function/constructor-type `(` open-delimiter trailing comment kept on the delimiter line
		'typescript/types/type_params/open_angle_comment_prettier_divergence',
		'typescript/types/function_type/open_paren_comment_prettier_divergence',
		// Object/array destructuring pattern open-delimiter trailing comment kept on the delimiter line
		'typescript/expressions/destructuring/object_open_brace_comment_prettier_divergence',
		'typescript/expressions/destructuring/array_open_bracket_comment_prettier_divergence',
		// Import/export keyword-to-braces comments
		'typescript/modules/imports/empty_keyword_comment_prettier_divergence',
		'typescript/modules/exports/empty_keyword_comment_prettier_divergence',
		// Svelte comments
		'svelte/syntax/comments/expr_trailing_prettier_divergence',
		'svelte/tags/debug/debug_comment_prettier_divergence',
	],
	detect(ctx) {
		const js_comment_pattern = /\/\/|\/\*|\*\//;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const added_comment_lines = hunk.added_lines.filter((l) => js_comment_pattern.test(l));
			const removed_comment_lines = hunk.removed_lines.filter((l) => js_comment_pattern.test(l));

			// At least one side must have comments
			if (added_comment_lines.length === 0 && removed_comment_lines.length === 0) return false;

			// Case 1: Comment on one side only — verify it was MOVED (exists in
			// other side's full output), not incidentally included by reformatting.
			if (added_comment_lines.length > 0 && removed_comment_lines.length === 0) {
				return added_comment_lines.some((l) => {
					const text = extract_comment_content(l);
					// Require minimum length and search with comment delimiters
					// to avoid matching bare text in code (e.g., "map" in arr.map())
					return text.length >= 3 && comment_exists_in_output(ctx.prettier, text);
				});
			}
			if (removed_comment_lines.length > 0 && added_comment_lines.length === 0) {
				return removed_comment_lines.some((l) => {
					const text = extract_comment_content(l);
					return text.length >= 3 && comment_exists_in_output(ctx.ours, text);
				});
			}

			// Case 2: Both sides have comments — verify the comment TEXT overlaps
			// AND the hunk is primarily about comment repositioning (non-comment
			// content should be similar). This prevents claiming hunks where the
			// real diff is code layout and comments are incidentally present.
			const added_texts = added_comment_lines.map(extract_comment_content).sort();
			const removed_texts = removed_comment_lines.map(extract_comment_content).sort();

			// Comment content must overlap (at least some comments have same text)
			const added_set = new Set(added_texts);
			const has_overlap = removed_texts.some((t) => added_set.has(t));
			if (!has_overlap) return false;

			// Lines must differ (the comment moved positions)
			const lines_differ = added_comment_lines.length !== removed_comment_lines.length ||
				added_comment_lines.some((l, i) => l !== removed_comment_lines[i]);
			if (!lines_differ) return false;

			// Non-comment content must be similar — strip comments from both sides
			// and compare the trimmed non-empty lines. If the code itself changed
			// significantly, this is a formatting bug, not a comment position divergence.
			const strip_comments = (line: string) =>
				line.replace(/\/\/.*$/, '').replace(/\/\*.*?\*\//g, '').trim();
			const added_code = hunk.added_lines.map(strip_comments).filter((l) => l.length > 0).sort();
			const removed_code = hunk.removed_lines.map(strip_comments).filter((l) => l.length > 0)
				.sort();

			// If non-comment content is identical (same set of trimmed lines),
			// the hunk is purely about comment positioning — claim it.
			if (
				added_code.length === removed_code.length &&
				added_code.every((l, i) => l === removed_code[i])
			) {
				return true;
			}

			// Fallback: when comment relocation also reformats the surrounding
			// code structure (e.g., Prettier absorbs `while (a) /* c */ {}` into
			// `while (a) {\n  /* c */\n}`, splitting one line into three), the
			// line-by-line check fails. Join non-comment code in document order
			// and compare whitespace-normalized to handle these cases.
			// Cap at 100 chars to avoid masking real formatting bugs in longer code.
			const added_code_unsorted = hunk.added_lines.map(strip_comments).filter((l) => l.length > 0);
			const removed_code_unsorted = hunk.removed_lines.map(strip_comments).filter(
				(l) => l.length > 0,
			);
			const normalize = (lines: string[]) => lines.join('').replace(/\s+/g, '');
			const normalized_added = normalize(added_code_unsorted);
			const normalized_removed = normalize(removed_code_unsorted);
			if (
				normalized_added.length <= 100 &&
				normalized_added === normalized_removed
			) {
				return true;
			}

			// If non-comment content differs, this is likely a code layout change
			// with incidental comments. Don't claim.
			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'comment_position',
				confidence: 'likely',
				hunk_indices,
				reason: 'Comment preserved where user placed it (Prettier relocates)',
			};
		}
		return null;
	},
};

const instantiation_parens: DivergencePattern = {
	id: 'instantiation_parens',
	description: 'Parens preserved in ternary/binary instantiation expressions',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: [
		'typescript/typescript_specific/assertions/instantiation_parens_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;

		// Ours preserves: (x ? y : z)<T> or (a + b)<T> — has )<
		// Prettier strips:  x ? y : z<T>  or  a + b<T>  — no )<
		const paren_before_type_args = /\)<[a-zA-Z]/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const ours_has_parens = hunk.added_lines.some((l) => paren_before_type_args.test(l));
			const prettier_missing = hunk.removed_lines.some(
				(l) => !paren_before_type_args.test(l) && /[?+\-]\s.*<[a-zA-Z]/.test(l),
			);
			return ours_has_parens && prettier_missing;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'instantiation_parens',
				confidence: 'certain',
				hunk_indices,
				reason:
					'Parens preserved around ternary/binary in instantiation expression (changes semantics)',
			};
		}
		return null;
	},
};

const block_comment_computed_member: DivergencePattern = {
	id: 'block_comment_computed_member',
	description: 'Block comment preserved inside computed member brackets',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript: Comments'],
	fixtures: [
		'typescript/syntax/comments/block_comment_computed_member_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;

		// Prettier hoists block comments from inside brackets to before the chain:
		//   removed: /* @type {T} */ obj.aaa.bbb?.[
		//   added:   obj.aaa.bbb?.[
		//            /* @type {T} */ d
		// Matches both /* */ and /** */ (JSDoc) comments.
		const block_comment_before_chain = /\/\*.*?\*\/\s+\w+\.\w+/;
		const block_comment_before_ident = /\/\*.*?\*\/\s+\w+\s*$/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const prettier_hoisted = hunk.removed_lines.some((l) => block_comment_before_chain.test(l));
			const ours_preserved = hunk.added_lines.some((l) => block_comment_before_ident.test(l));
			return prettier_hoisted && ours_preserved;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'block_comment_computed_member',
				confidence: 'certain',
				hunk_indices,
				reason:
					'Block comment preserved inside computed member brackets (Prettier hoists, changing association)',
			};
		}
		return null;
	},
};

const block_comment_chain: DivergencePattern = {
	id: 'block_comment_chain',
	description: 'Block comment spacing in member chain normalization',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript: Comments'],
	fixtures: [
		'typescript/expressions/calls/chained/block_comment_chain_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;

		// Prettier intermediate: `a/* comment */ .b` (space before dot)
		// Ours/stable:           `a /* comment */.b` (no space before dot)
		// One side has `*/ .` and the other has `*/.` — different comment-dot spacing
		const comment_space_dot = /\*\/\s+\./;
		const comment_dot = /\*\/\./;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const prettier_spaced = hunk.removed_lines.some((l) => comment_space_dot.test(l));
			const ours_compact = hunk.added_lines.some((l) => comment_dot.test(l));
			if (prettier_spaced && ours_compact) return true;
			// Reverse direction (ours spaced, prettier compact)
			const ours_spaced = hunk.added_lines.some((l) => comment_space_dot.test(l));
			const prettier_compact = hunk.removed_lines.some((l) => comment_dot.test(l));
			return ours_spaced && prettier_compact;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'block_comment_chain',
				confidence: 'likely',
				hunk_indices,
				reason:
					'Block comment spacing in member chain differs (normalization-only, both reach same stable output)',
			};
		}
		return null;
	},
};

const jsdoc_type_cast_parens: DivergencePattern = {
	id: 'jsdoc_type_cast_parens',
	description: 'JSDoc type cast parens stripped',
	languages: ['svelte'],
	conformance_sections: ['TypeScript: Comments'],
	fixtures: [
		'typescript/syntax/comments/jsdoc_type_cast_prettier_divergence',
		'typescript/calls/arrow_jsdoc_cast_body_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Prettier keeps parens: /** @type {T} */ (expr)
		// We strip them: /** @type {T} */ expr
		const jsdoc_cast_with_parens = /@(?:type|satisfies)\s*\{[^}]*\}\s*\*\/\s*\(/;
		const jsdoc_cast_without_parens = /@(?:type|satisfies)\s*\{[^}]*\}\s*\*\/\s*[^(]/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const prettier_has_parens = hunk.removed_lines.some((l) => jsdoc_cast_with_parens.test(l));
			const ours_without_parens = hunk.added_lines.some((l) => jsdoc_cast_without_parens.test(l));
			return prettier_has_parens && ours_without_parens;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'jsdoc_type_cast_parens',
				confidence: 'certain',
				hunk_indices,
				reason: 'JSDoc type cast parens stripped (semantically meaningless)',
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
	bom_strip,
	self_closing_nonvoid,
	empty_statement_removal,
	css_value_ratio,

	// 2. CSS-specific patterns
	css_atrule_spec_spacing,
	css_atrule_long_wrap,
	css_atrule_stable_quirk,
	css_selector_divergence,
	css_comment_stable_quirk,

	// 3. Feature-specific patterns
	template_literal_width,
	block_expression_logical,
	single_specifier_import,
	member_expression_call,
	return_type_generic_union,

	// 4. Svelte-specific patterns
	menu_block,
	inline_content_hug,
	fill_after_inline,
	block_multiline_attrs_hug,
	short_expr_100,

	// 5. Semantic preservation patterns
	instantiation_parens,
	block_comment_computed_member,
	block_comment_chain,
	jsdoc_type_cast_parens,

	// 6. Broad patterns (run last)
	css_value_wrap,
	fill_101_boundary,
	comment_position,
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
export function detect_divergences(ctx: DetectionContext): HunkCoverageResult {
	// Pre-compute cached fields (line arrays, style boundaries)
	if (!ctx.ours_lines) enrich_detection_context(ctx);

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
	const explained_hunks = new Set<number>();
	for (const match of matches) {
		for (const idx of match.hunk_indices) {
			explained_hunks.add(idx);
		}
	}

	const all_hunk_indices = hunks.map((h) => h.index);
	const unexplained_hunks = all_hunk_indices.filter((idx) => !explained_hunks.has(idx));

	let classification: HunkCoverageResult['classification'];
	if (matches.length === 0 || explained_hunks.size === 0) {
		classification = 'none_explained';
	} else if (unexplained_hunks.length === 0) {
		classification = 'all_explained';
	} else {
		classification = 'partial';
	}

	return {
		hunks,
		matches,
		explained_hunks,
		unexplained_hunks,
		classification,
	};
}
