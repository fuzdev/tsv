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
 * The recurring long-line divergence shape: a print-width-driven re-wrap.
 *
 * A hunk matches when prettier has a line satisfying `line_predicate` that
 * exceeds `min_width`, AND ours genuinely re-wrapped it — more added lines than
 * removed. The re-wrap evidence is the load-bearing guard: matching solely on a
 * wide prettier line (with no proof OURS broke it into the benign divergent
 * form) is exactly the over-match class that lets a real bug — or worse, a
 * data-loss reclassified as `known_divergence` — slip through. Centralizing the
 * shape here makes the missing-guard mistake structurally hard to reintroduce.
 */
function long_line_rewrapped(
	hunk: DiffHunk,
	prettier_lines: string[],
	options: { min_width?: number; line_predicate: (line: string) => boolean },
): boolean {
	const min_width = options.min_width ?? 100;
	const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
	const has_long_match = p_lines.some(
		(l) => options.line_predicate(l) && visual_width(l) > min_width,
	);
	if (!has_long_match) return false;
	return hunk.added_lines.length > hunk.removed_lines.length;
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
 * Whether a hunk is a pure re-indent: ours and prettier carry the same lines in the
 * same order, each differing only by leading whitespace (so no token can be lost),
 * with at least one line's indentation actually changing. Indentation-only by
 * construction, so claiming such a hunk can never mask a content change.
 */
function is_pure_reindent(hunk: DiffHunk): boolean {
	const rem = hunk.removed_lines;
	const add = hunk.added_lines;
	if (rem.length === 0 || rem.length !== add.length) return false;
	let any_change = false;
	for (let i = 0; i < rem.length; i++) {
		if (rem[i].replace(/^[ \t]*/, '') !== add[i].replace(/^[ \t]*/, '')) return false;
		if (rem[i] !== add[i]) any_change = true;
	}
	return any_change;
}

/**
 * Whether a pure-re-indent hunk is CSS *selector* content — at least one line is a
 * list item (`…,`), a post-pseudo continuation (`):not(…)`), or a pseudo-class
 * function (`:is(`/`:where(`/`:not(`/`:has(`). This is the §CSS: Selectors indent
 * divergence (`compound_args_indent` / `nested_where_is`): tsv keys the extra indent
 * on a real combinator while prettier keys it on a flat `nodes.length > 2` count, so
 * a nested pseudo's argument list sits one level shallower under tsv.
 */
function is_pure_selector_reindent(hunk: DiffHunk): boolean {
	if (!is_pure_reindent(hunk)) return false;
	return hunk.removed_lines.some((l) => {
		const t = l.replace(/^[ \t]*/, '');
		return /,$/.test(t) || /^\)[:.\w]/.test(t) || /:(?:is|where|not|has|matches|any)\(/.test(t);
	});
}

/**
 * Extract comment text content from a line (strip delimiters and whitespace).
 * Returns only the comment token's text, stripping any code that precedes the
 * comment delimiter. Returns `''` when the line has no comment delimiter — the
 * whole code line must never be treated as comment content (that would let
 * comment-position matching key off arbitrary code text).
 */
function extract_comment_content(line: string): string {
	// Line comment: take everything after the FIRST `//`, dropping the code
	// before it (e.g. `foo(); // bar` → `bar`).
	const line_comment_at = line.indexOf('//');
	if (line_comment_at !== -1) {
		return line.slice(line_comment_at + 2).trim();
	}
	// Block comment: take the text after the FIRST `/*`, then strip a trailing
	// `*/` and anything after it (e.g. `foo(); /* a */ bar()` → `a`). When the
	// `*/` is absent (opening-only fragment) keep the remainder of the line.
	const block_open_at = line.indexOf('/*');
	if (block_open_at !== -1) {
		let inner = line.slice(block_open_at + 2);
		const close_at = inner.indexOf('*/');
		if (close_at !== -1) inner = inner.slice(0, close_at);
		return inner.trim();
	}
	// Closing-only fragment: text before the `*/` is the comment continuation.
	const block_close_at = line.indexOf('*/');
	if (block_close_at !== -1) {
		return line.slice(0, block_close_at).trim();
	}
	// No comment delimiter on this line — not comment content.
	return '';
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

/**
 * Check if `text` appears as a WHOLE comment line in `output` — i.e. some line
 * whose only comment content (after delimiter stripping) is exactly `text`.
 * Stricter than `comment_exists_in_output`: the prefix-substring form there can
 * match `// ${text}` embedded in a string literal, a longer comment, or a JSDoc
 * ` * ` continuation that merely starts with `text`. Requiring the extracted
 * comment content to equal `text` rejects those — the relocated comment must
 * land as its own comment, not as a fragment of unrelated code or comment text.
 */
function comment_line_exists_in_output(output: string, text: string): boolean {
	return output.split('\n').some((line) => extract_comment_content(line) === text);
}

/**
 * Whole-comment-line contents of the lines immediately bordering a hunk's change
 * range (the line just before its start and just after its end) on one side.
 * Returns the extracted comment content for each border line that is a whole
 * comment line, dropping non-comment / empty borders.
 *
 * Used by `comment_position` Case 3: some sanctioned comment-relocation
 * divergences move a comment that the diff aligns as a CONTEXT (same) line —
 * because the comment text is byte-identical in both outputs — while the
 * surrounding structure (the discriminant parens of an empty `switch`, the
 * `} else {` split, a member chain's break) reshapes into the change hunk. The
 * comment then never appears inside the hunk's own added/removed lines; it sits
 * on the hunk's immediate border. Looking only at the IMMEDIATE border (not a
 * wide window) keeps the comment tied to THIS structural change.
 */
function border_comment_contents(
	lines: string[],
	range: { start: number; end: number } | null,
): string[] {
	if (!range) return [];
	const out: string[] = [];
	for (const idx of [range.start - 1, range.end + 1]) {
		const line = lines[idx];
		if (line === undefined) continue;
		const text = extract_comment_content(line);
		if (text.length > 0) out.push(text);
	}
	return out;
}

/**
 * The immediate previous/next lines around the first whole-comment-line in
 * `lines` whose content equals `text`, or `null` when no such comment line
 * exists. Beginning/end of file are reported as sentinels so they compare
 * unequal to any real line.
 *
 * Used by `comment_position` Case 3 to prove a bordering comment actually
 * RELOCATED rather than merely sitting beside a re-wrap: a genuinely relocated
 * comment lands in a different syntactic container, so BOTH its neighbors differ
 * between the two outputs. A stable comment that just happens to precede (or
 * follow) a width re-wrap keeps one neighbor identical — which this lets the
 * detector reject.
 */
function comment_line_neighbors(
	lines: string[],
	text: string,
): { prev: string; next: string } | null {
	for (let i = 0; i < lines.length; i++) {
		if (extract_comment_content(lines[i]) === text) {
			return { prev: (lines[i - 1] ?? '~bof').trim(), next: (lines[i + 1] ?? '~eof').trim() };
		}
	}
	return null;
}

/**
 * Whether two trimmed lines begin the SAME element — one is a (non-trivial)
 * prefix of the other. Rejects a FALSE relocation signal: when a stable comment
 * borders a width re-wrap, the element it precedes stays the same but its tail
 * wraps onto extra lines, so the comment's neighbor in one output is a prefix of
 * the neighbor in the other (`e: '${ssss` is a prefix of `e: '${ssss.aaa()}',`).
 * A genuine relocation lands the comment among entirely different tokens, where
 * neither neighbor begins the same element as its counterpart.
 */
function lines_begin_same_element(a: string, b: string): boolean {
	if (a === b) return true;
	const [short, long] = a.length <= b.length ? [a, b] : [b, a];
	return short.length >= 3 && long.startsWith(short);
}

/**
 * Strip ALL whitespace from a string. Used as a content-preservation gate: when
 * `strip_all_ws(ours) === strip_all_ws(prettier)` the entire ours/prettier
 * difference is whitespace-only, so the divergence is provably a pure-layout
 * reflow with no content loss — a single non-whitespace difference anywhere
 * fails the gate and disables the detector (so it can never mask a real loss).
 */
function strip_all_ws(s: string): string {
	return s.replace(/\s+/g, '');
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
		// Use the `﻿` escape rather than a literal BOM glyph in source — a raw
		// BOM byte in this file is an editing hazard (invisible, easily mangled).
		const BOM = '﻿';
		// Source starts with BOM, our output doesn't
		if (ctx.source.startsWith(BOM) && !ctx.ours.startsWith(BOM)) {
			// Verify prettier keeps BOM
			if (ctx.prettier.startsWith(BOM)) {
				// Find the hunk covering the BOM rather than assuming it is hunk 0:
				// the hunk whose prettier (removed) range starts at source line 0, or
				// failing that whose removed line still carries the BOM.
				const bom_hunk = ctx.hunks.find((h) =>
					h.prettier_range?.start === 0 || h.removed_lines.some((l) => l.startsWith(BOM))
				);
				const hunk_indices = bom_hunk ? [bom_hunk.index] : [];
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

const css_unit_serialize_case: DivergencePattern = {
	id: 'css_unit_serialize_case',
	description: 'CSS Hz/kHz/Q units serialized lowercase per spec',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: Values'],
	fixtures: ['css/values/units_serialize_case_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		// tsv lowercases EVERY unit to its spec-serialized form; prettier upcases the
		// three units whose canonical serialization is nonetheless lowercase — `5hz`→`5Hz`,
		// `1khz`→`1kHz`, `10q`→`10Q` (CSS Values 4 §6.2/§7.3). A hunk matches only when a
		// removed (prettier) line carries one of the upcased forms AND the added (ours) line
		// is its exact ASCII-case-lowered twin — a pure case swap, so it can never mask a
		// content change, and the reverse direction (ours upcasing) is not matched.
		const prettier_unit = /\d(?:Hz|kHz|Q)\b/;
		const ours_unit = /\d(?:hz|khz|q)\b/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;
			return hunk.removed_lines.some((removed) => {
				if (!prettier_unit.test(removed)) return false;
				const lowered = removed.toLowerCase();
				return hunk.added_lines.some(
					(added) => ours_unit.test(added) && added.toLowerCase() === lowered,
				);
			});
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_unit_serialize_case',
				confidence: 'certain',
				hunk_indices,
				reason: 'CSS Hz/kHz/Q serialized lowercase per spec (CSS Values 4 §6.2/§7.3)',
			};
		}
		return null;
	},
};

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
			// Prettier has a long at-rule line (> 100 chars) that ours wrapped.
			return long_line_rewrapped(hunk, prettier_lines, {
				line_predicate: (l) => atrule_pattern.test(l),
			});
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

const css_scss_directive_number: DivergencePattern = {
	id: 'css_scss_directive_number',
	description: 'SCSS-directive at-rule prelude numbers preserved verbatim (prettier normalizes)',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: At-Rules'],
	fixtures: ['css/at_rules/scss_directive_number_preserved_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		// SCSS/Sass directives prettier value-parses (and thus number-normalizes);
		// tsv treats their prelude as an opaque token stream and preserves it.
		const scss_directive =
			/@(?:include|mixin|if|else|for|each|while|debug|function|return|content|define-mixin|add-mixin)\b/;
		// The non-numeric skeleton (strip whitespace + number-format chars) must be
		// identical on both sides AND the numeric-token COUNT must match. The old
		// guard stripped digits/dots before comparing, which made the very thing it
		// was meant to protect — dropped numeric content — invisible (e.g.
		// `width: 100px` vs `width: px` compared skeleton-equal). Counting numeric
		// tokens on each side ensures a dropped (or added) number is caught: the
		// SCSS-number divergence only ever re-spells the SAME count of numbers
		// (`.5`→`0.5`, `1.50`→`1.5`), never drops one.
		const skeleton = (lines: string[]) => lines.join('\n').replace(/[\s\d.]/g, '');
		const number_token = /\d*\.\d+|\d+/g;
		const count_numbers = (lines: string[]) => (lines.join('\n').match(number_token) ?? []).length;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;
			if (hunk.removed_lines.length === 0 || hunk.added_lines.length === 0) return false;
			const joined = `${hunk.removed_lines.join('\n')}\n${hunk.added_lines.join('\n')}`;
			if (!scss_directive.test(joined)) return false;
			if (skeleton(hunk.removed_lines) !== skeleton(hunk.added_lines)) return false;
			// Numeric content may be re-spelled but never dropped/added.
			return count_numbers(hunk.removed_lines) === count_numbers(hunk.added_lines);
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_scss_directive_number',
				confidence: 'likely',
				hunk_indices,
				reason: 'SCSS-directive at-rule prelude preserved verbatim; prettier number-normalizes',
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
		'css/selectors/pseudo_class/compound_args_indent_long_prettier_divergence',
		'css/selectors/pseudo_class/nested_where_is_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;

			// Pseudo-args indent: tsv keys the extra indent on a real combinator, not
			// prettier's `nodes.length > 2` count, so a nested pseudo's argument list
			// (`:is(…)` inside `:where(…)`) sits one level shallower — a pure re-indent.
			if (is_pure_selector_reindent(hunk)) return true;

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

		// Nested-template shape: an interpolation `${` that opens a template or
		// array literal (a backtick appears before the interpolation closes), e.g.
		// `${[` … `` ` `` …  or `` ${` ``. Prettier keeps the whole nested construct
		// inline (overflowing print width); ours breaks the inner bracket. This is
		// NOT a plain `${expr}` (where no backtick precedes the closing `}`), so it
		// does not match the end-of-line `${` / `}`` break markers Case 1/2 key on.
		const nested_template_interpolation = /\$\{[^`}]*`/;

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

			// Case 3: Nested template / array inside an interpolation. Prettier keeps
			// the nested `${[`…`]}` (or `` ${`…` `` ) construct inline past print
			// width; ours breaks the inner bracket. The end-of-line `${` / `}`` markers
			// never appear here, so reuse the shared `long_line_rewrapped` shape —
			// which carries the ours-side re-wrap guard (more added than removed
			// lines) — keyed on a prettier line exhibiting the nested-template
			// interpolation. Without the re-wrap guard, a bug that mangled a wide
			// nested-template line in place would be claimed purely from its width.
			if (
				long_line_rewrapped(hunk, prettier_lines, {
					line_predicate: (l) => nested_template_interpolation.test(l),
				})
			) {
				return true;
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
		'svelte/blocks/if/last_block_prettier_divergence',
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
		// Imports are tab-indented when inside a Svelte `<script>` block, so allow
		// leading tabs. The keyword form may carry `type` (`import type { … }`), so
		// match `import` + any non-brace prefix before the opening `{`.
		// Prettier (inline) keeps the whole single-specifier import on one line:
		// `import { … } from '…';` — both braces present on the same line.
		const import_inline = /^\t*import\b[^{}]*\{[^{}]*\}/;
		// Ours (broken) ends the close line on the module path: `} from '…';`.
		const import_close = /^\t*\}\s*from\s+['"][^'"]+['"]/;
		const from_path = (l: string) => l.match(/from\s+(['"][^'"]+['"])/)?.[1] ?? null;

		// The module paths of every single-specifier import prettier kept INLINE past
		// print width — the divergence target. Keyed on the path (not on hunk
		// alignment), because consecutive imports make the LCS split the long inline
		// line and ours' broken close into separate hunks (so the original
		// same-hunk `opener + long-line` predicate missed real files).
		const long_inline_paths = new Set<string>();
		for (const l of ctx.prettier_lines!) {
			if (import_inline.test(l) && visual_width(l) > 100) {
				const p = from_path(l);
				if (p) long_inline_paths.add(p);
			}
		}
		if (long_inline_paths.size === 0) return null;

		// Ours-side re-wrap evidence — a long path only counts if OUR output actually
		// BROKE it into the multiline form (a `} from '<path>';` close on its own
		// line). An import ours merely edited in place (same single line, different
		// path) has no such close, so it is never claimed.
		const broken_paths = new Set<string>();
		for (const l of ctx.ours_lines!) {
			const p = import_close.test(l) ? from_path(l) : null;
			if (p && long_inline_paths.has(p)) broken_paths.add(p);
		}
		if (broken_paths.size === 0) return null;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Prettier side: the long inline import itself.
			const removed_long = hunk.removed_lines.some(
				(l) =>
					import_inline.test(l) && visual_width(l) > 100 && broken_paths.has(from_path(l) ?? ''),
			);
			// Ours side: the broken close `} from '<same path>';`.
			const added_break = hunk.added_lines.some(
				(l) => import_close.test(l) && broken_paths.has(from_path(l) ?? ''),
			);
			return removed_long || added_break;
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

		// Ours-side evidence guard. The documented divergence is: ours expands the
		// call args (extra lines) while prettier breaks at the member chain. Bare
		// substring presence is NOT enough — a real bug on a line that merely
		// contains `require.resolve(` would otherwise be claimed. Require:
		//   1. The module pattern appears in OURS' added lines — the divergent break
		//      is in our output, not merely somewhere in the prettier side.
		//   2. Ours genuinely re-wrapped (more added than removed lines). A hunk
		//      where ours collapsed onto fewer lines, or where the pattern only
		//      shows up on prettier's removed side, is not this divergence.
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const ours_has_module_break = hunk.added_lines.some((l) => module_patterns.test(l));
			if (!ours_has_module_break) return false;
			return hunk.added_lines.length > hunk.removed_lines.length;
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
		// where prettier's line exceeds 100 chars and ours re-wrapped it.
		const union_in_generic = /[<>].*\|\s*(?:null|void|undefined)/;

		const hunk_indices = find_matching_hunks(
			ctx.hunks,
			(hunk) =>
				long_line_rewrapped(hunk, prettier_lines, {
					line_predicate: (l) => union_in_generic.test(l),
				}),
		);

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

const non_null_paren_base: DivergencePattern = {
	id: 'non_null_paren_base',
	description:
		'Non-null assertion on a parenthesized base: tsv hangs the outer parens, prettier hugs the inner call',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: ['typescript/expressions/member/non_null_paren_base_long_prettier_divergence'],
	detect(ctx) {
		// Prettier hugs the inner call under a non-null assertion: the base's two
		// closing parens collapse onto one line right before `!`, e.g. `))!.ok`.
		const prettier_hugs = /\)\)!\??\./;
		// tsv hangs the outer parens: the inner `)` lands on its own line, then a
		// line that begins with `)!.member` (single close, then the non-null member).
		const ours_hangs = /^\s*\)!\??\./;

		const hunk_indices = find_matching_hunks(
			ctx.hunks,
			(hunk) =>
				hunk.removed_lines.some((l) => prettier_hugs.test(l)) &&
				hunk.added_lines.some((l) => ours_hangs.test(l)),
		);

		if (hunk_indices.length > 0) {
			return {
				pattern: 'non_null_paren_base',
				confidence: 'likely',
				hunk_indices,
				reason:
					'Non-null assertion on a parenthesized base: tsv hangs the outer parens, prettier hugs the inner call',
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
	// The committed fill-after-inline fixtures carry the trailing text (and the
	// over-width line) AFTER the inline element's closing tag, with the close tag
	// itself on a separate line — so the long re-wrapped line never contains an
	// inline close tag. That generic "prettier fills past print width, we break"
	// shape is owned by `fill_101_boundary` (which detects them). This detector
	// keys on an inline close tag ON the long line; keeping that predicate intact
	// is what distinguishes it from the broad boundary case, so those fixtures
	// belong to `fill_101_boundary`.
	fixtures: [],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		const prettier_lines = ctx.prettier_lines!;
		const inline_close_tag =
			/<\/(?:span|a|strong|em|code|b|i|small|abbr|sub|sup|mark|cite|q|time|data|kbd|samp|var|dfn|ins|del|u|s)>/;

		// Check each hunk for prettier lines with long inline element lines.
		// Ours-side evidence guard (shared shape): a long prettier line with an
		// inline close tag is not enough — require that ours actually re-wrapped it
		// (more added than removed lines). Without this, a bug where ours emits the
		// same long line (no legitimate fill break) would be claimed solely from
		// prettier's width.
		const hunk_indices = find_matching_hunks(
			ctx.hunks,
			(hunk) =>
				long_line_rewrapped(hunk, prettier_lines, {
					line_predicate: (l) => inline_close_tag.test(l),
				}),
		);

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
	// The committed fixture (`<pre>` with multiline attrs) hugs `">{expr}</pre>`
	// on the attr line in prettier while ours breaks the `>` — but the `>` is not
	// alone on a line (it carries `{expr}</pre>`), and the `<pre` open tag is a
	// context line that falls OUTSIDE the single change hunk's range, so neither
	// the `>`-alone predicate nor the whitespace-sensitive-element context check
	// this detector keys on can fire. The conformance doc bins this fixture with
	// the fill-boundary family (prettier fills past print width, we break), which
	// `fill_101_boundary` detects — so the fixture is claimed there.
	fixtures: [],
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

const comment_preserved: DivergencePattern = {
	id: 'comment_preserved',
	description: 'We preserve a comment inside {…}/a tag that Prettier drops',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Attributes', 'Svelte: Elements'],
	fixtures: [
		'svelte/syntax/comments/expr_trailing_prettier_divergence',
		'svelte/syntax/comments/expr_trailing_line_prettier_divergence',
		'svelte/tags/debug/debug_comment_prettier_divergence',
		'svelte/tags/debug/debug_comma_comment_prettier_divergence',
	],
	// The "we preserve / Prettier DROPS a comment" family (◆content_preservation).
	// `comment_position` deliberately can't claim these — its content guard requires
	// the comment in BOTH outputs, and a dropped comment is absent from prettier's —
	// so this dedicated detector keys on the opposite, safe signal.
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Strip JS/Svelte comments + all whitespace, leaving only code glyphs — so
		// two lines that differ ONLY by a comment (and its reflow) compare equal.
		const strip_code = (s: string): string =>
			s.replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '').replace(/\s+/g, '');
		const has_comment = (s: string): boolean => /\/\*[\s\S]*?\*\/|\/\//.test(s);

		// Claim a hunk where an OURS (added) line carries a comment and, with the
		// comment stripped, reproduces a PRETTIER (removed) line's code — directly,
		// or once the `}` prettier reflowed onto that line is rejoined from the next
		// ours line (the `{expr // c⏎}` line-comment form). DIRECTIONAL BY
		// CONSTRUCTION: the signal is a comment on the OURS side, so it can never
		// fire on an ours-side DROP (that has the comment on the prettier/removed
		// side) — a data-loss is never masked as `known`, and `safety.ts` guards
		// real char-loss independently.
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const removed_code = hunk.removed_lines.filter((p) => !has_comment(p)).map(strip_code);
			const added = hunk.added_lines;
			for (let i = 0; i < added.length; i++) {
				const a = added[i];
				if (!has_comment(a)) continue;
				const a_code = strip_code(a);
				if (a_code === '') continue;
				const a_joined = a_code + (i + 1 < added.length ? strip_code(added[i + 1]) : '');
				if (removed_code.some((p) => p === a_code || p === a_joined)) return true;
			}
			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'comment_preserved',
				confidence: 'likely',
				hunk_indices,
				reason: 'We preserve a comment inside {…}/a tag that Prettier drops',
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
	fixtures: [
		'svelte/blocks/each/long_prettier_divergence',
		'svelte/blocks/await/long_prettier_divergence',
		'svelte/blocks/key/long_prettier_divergence',
		'svelte/blocks/if/long_prettier_divergence',
		'svelte/blocks/if/in_inline_element_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		const prettier_lines = ctx.prettier_lines!;
		const block_expr_pattern = /\{#(?:if|each|await|key)/;

		// Check each hunk for block expressions that exceed 100 chars in prettier range.
		// Ours-side evidence guard: a 101-110 wide prettier block line is not enough —
		// require that ours actually broke it (more added than removed lines). Without
		// this, a bug somewhere in the 101-110 band gets claimed purely from prettier's
		// width even though ours did not legitimately re-break the block condition.
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			const has_short_overflow_block = p_lines.some(
				(l) => block_expr_pattern.test(l) && visual_width(l) > 100 && visual_width(l) <= 110,
			);
			if (!has_short_overflow_block) return false;
			return hunk.added_lines.length > hunk.removed_lines.length;
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

const annotation_continuation_indent: DivergencePattern = {
	id: 'annotation_continuation_indent',
	description:
		'tsv indents a `: Type` annotation continuation one level when a line comment trails the colon; prettier keeps the type flush',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['Uniform Forced-Continuation Indent', 'Comment Position Philosophy'],
	fixtures: ['typescript/types/comments/annotation_continuation_indent_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;
		const ours_lines = ctx.ours_lines!;

		// A `:` immediately after an annotation target (identifier / `)` / `]` / `}` /
		// `>`) carrying a trailing line comment — the colon→type continuation that tsv
		// drops to its own line indented one level (the shared `build_type_annotation_doc`
		// rule), where prettier keeps the type flush. A line-leading `:` (a ternary
		// branch) is excluded by requiring a preceding word/closer. The continuation
		// itself is a pure re-indent (indentation-only), so no content can be lost.
		const annotation_colon_comment = /[\w)\]}>][ \t]*:[ \t]*\/\//;
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_pure_reindent(hunk)) return false;
			const start = hunk.ours_range?.start;
			if (start == null || start === 0) return false;
			return annotation_colon_comment.test(ours_lines[start - 1] ?? '');
		});

		if (hunk_indices.length === 0) return null;
		return {
			pattern: 'annotation_continuation_indent',
			confidence: 'likely',
			hunk_indices,
			reason: 'colon→type annotation continuation indents one level after a trailing line comment',
		};
	},
};

const inline_content_block_style: DivergencePattern = {
	id: 'inline_content_block_style',
	description:
		'tsv lays out inline element/block content block-style (tags intact, content on its own line); prettier dangles the tag delimiters / hugs the content',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Inline content block-style', 'Svelte: Blocks'],
	fixtures: [
		'svelte/elements/inline_sibling_gt_dangle_prettier_divergence',
		'svelte/elements/block_body_drop_nested_siblings_prettier_divergence',
		'svelte/elements/block_multiline_attrs_content_hug_prettier_divergence',
		'svelte/elements/inline_if_sibling_fill_long_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// SAFETY GATE — the whole ours/prettier difference is whitespace-only, so no
		// content can be lost: this is provably a pure-layout reflow. A single
		// non-whitespace difference anywhere (a dropped comment, a normalized quote, a
		// real content change) fails the gate and disables the detector, so it can
		// never mask a content loss. This content-preservation proof is also what
		// makes claiming the whole diff below sound: there is provably nothing hidden
		// in any hunk, unlike a width/property heuristic that only inspects one line.
		if (strip_all_ws(ctx.ours) !== strip_all_ws(ctx.prettier)) return null;

		// FAMILY SIGNATURE — confirm the reflow is the block-style design choice, not
		// some other whitespace-only difference. Two markers on a CHANGED line, both
		// produced only by tsv keeping a construct intact and dropping its content to
		// its own line:
		//   - a *dangled* tag delimiter: a closing tag whose `>` moved off (`…</tag` at
		//     EOL, no `>`) or a `>` that moved to the start of a line (`>` alone, or
		//     prefixing the hugged content / next tag); or
		//   - a *dropped block body*: a control-flow head (`{#if …}` / `{#each …}` /
		//     `{#await …}` / `{#key …}` / `{#snippet …}`) sitting ALONE on one of OUR
		//     lines, where prettier hugged the body onto the head line (the §Svelte:
		//     Blocks uniform body-drop). Ours-side only, and "alone" — inside `<pre>`
		//     the body stays hugged so ours never isolates the head, so this does not
		//     reach the `<pre>` print-width case.
		// Other whitespace-only divergences carry NEITHER marker — verbatim
		// `format-ignore`, empty-destructure `{}` vs `{ }`, a moved attribute-list
		// comment, a `<pre>` print-width attr wrap — and stay unclaimed for their own
		// detector (broader open-tag / element-alone markers were tried and rejected:
		// they false-match exactly those forms). One body-drop variant stays uncovered:
		// where prettier instead wraps an element's *attributes* (no `>` dangle and the
		// head was never hugged), which has no safe marker distinct from those forms.
		const dangle_close = /<\/[A-Za-z][\w-]*[ \t]*$/; //                  `</tag` at EOL
		const dangle_open = /^[ \t]*>/; //                                  `>` starts a line
		const block_head_alone = /^[ \t]*\{#(?:if|each|await|key|snippet)\b[^}]*\}[ \t]*$/;
		let has_signature = false;
		for (const hunk of ctx.hunks) {
			if (
				hunk.removed_lines.concat(hunk.added_lines).some((l) => dangle_close.test(l) || dangle_open.test(l)) ||
				hunk.added_lines.some((l) => block_head_alone.test(l))
			) {
				has_signature = true;
				break;
			}
		}
		if (!has_signature) return null;

		// The reflow is one content-preserving block-style relocation spanning the
		// whole diff (the relocated content and the dangled delimiters land in
		// separate, sometimes non-adjacent hunks), so claim every hunk.
		return {
			pattern: 'inline_content_block_style',
			confidence: 'likely',
			hunk_indices: ctx.hunks.map((h) => h.index),
			reason:
				'inline/block content laid out block-style (tags intact, content on its own line); prettier dangles the tag delimiters',
		};
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
			return long_line_rewrapped(hunk, prettier_lines, {
				line_predicate: (l) => /^\t+[\w-]+:\s*.+/.test(l),
			});
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
		'svelte/elements/fill_after_inline_prettier_divergence',
		'svelte/elements/fill_multiple_expr_long_prettier_divergence',
		'svelte/elements/block_multiline_attrs_content_hug_prettier_divergence',
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
			// Case 2: Same or fewer lines, but all our lines fit within print width.
			// Require at least one added line — `every` is vacuously true for a
			// removal-only hunk (empty added_lines), which would otherwise claim a
			// prettier line we simply DELETED as a print-width rewrap.
			const ours_all_fit = hunk.added_lines.length > 0 &&
				hunk.added_lines.every((l) => visual_width(l) <= 100);
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
		'typescript/statements/for/empty_clauses_comment_prettier_divergence',
		'typescript/statements/for/of_line_comment_prettier_divergence',
		'typescript/statements/do_while/open_paren_comment_prettier_divergence',
		'typescript/statements/try/catch_between_comment_prettier_divergence',
		'typescript/statements/try/line_comment_absorbed_prettier_divergence',
		'typescript/statements/labeled/comment_prettier_divergence',
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
		// Namespace/module body open-delimiter trailing comment kept on the delimiter line
		'typescript/declarations/namespace/open_brace_comment_prettier_divergence',
		// Class/interface/enum body open-delimiter trailing comment kept on the delimiter line
		'typescript/statements/class/open_brace_comment_prettier_divergence',
		'typescript/statements/interface/open_brace_comment_prettier_divergence',
		'typescript/declarations/enum/open_brace_comment_prettier_divergence',
		// Type literal open-delimiter trailing comment kept on the delimiter line
		'typescript/types/type_literal_open_brace_comment_prettier_divergence',
		// Import/export specifier braces open-delimiter trailing comment kept on the delimiter line
		'typescript/modules/imports/open_brace_comment_prettier_divergence',
		'typescript/modules/exports/open_brace_comment_prettier_divergence',
		// Tuple type open-delimiter trailing comment kept on the delimiter line
		'typescript/types/tuple/open_bracket_comment_prettier_divergence',
		// Type-argument list open-delimiter trailing comment kept on the delimiter line (multi-arg)
		'typescript/types/type_argument_open_angle_comment_prettier_divergence',
		// Call/`new`-expression type-argument list open-delimiter trailing comment kept on the delimiter line (multi-arg)
		'typescript/expressions/calls/type_args_open_angle_comment_prettier_divergence',
		// Retained parenthesized union member: block comment kept inside the parens
		'typescript/types/union_intersection_retained_paren_comment_prettier_divergence',
		// Retained parenthesized union FIRST member: leading line comment kept inside the parens
		'typescript/types/union_intersection_retained_paren_leading_line_comment_prettier_divergence',
		// Retained parenthesized intersection member: block comment kept inside the parens
		'typescript/types/retained_paren_intersection_member_comment_prettier_divergence',
		// Import/export keyword-to-braces comments
		'typescript/modules/imports/empty_keyword_comment_prettier_divergence',
		'typescript/modules/exports/empty_keyword_comment_prettier_divergence',
		'typescript/modules/imports/empty_type_keyword_comment_prettier_divergence',
		'typescript/modules/exports/empty_type_keyword_comment_prettier_divergence',
		'typescript/modules/imports/type_keyword_comment_prettier_divergence',
		'typescript/modules/exports/type_keyword_comment_prettier_divergence',
		'typescript/modules/imports/default_keyword_comment_prettier_divergence',
		'typescript/modules/imports/namespace_keyword_comment_prettier_divergence',
		'typescript/modules/exports/all_keyword_comment_prettier_divergence',
		'typescript/modules/exports/all_namespace_keyword_comment_prettier_divergence',
		// Binding/specifiers-to-`from` gap comments
		'typescript/modules/imports/from_comment_prettier_divergence',
		'typescript/modules/exports/from_comment_prettier_divergence',
		// Import-attributes header (source-to-`with`, `with`-to-`{`) gap comments
		'typescript/modules/imports/with_keyword_comment_prettier_divergence',
		// Sequence operand outer-edge comments float out of the sequence parens
		// (call context matches prettier's fixed point; statement context keeps the
		// trailing comment before `;`)
		'typescript/expressions/sequence/operand_edge_comment_prettier_divergence',
		'typescript/expressions/sequence/operand_edge_comment_stmt_prettier_divergence',
		// NOTE: the Svelte `expr_trailing` / `debug_comment` fixtures are NOT
		// claimed here. Prettier DROPS those comments, so they fail this pattern's
		// "comment exists as a whole line in BOTH outputs" content guard by design
		// (loosening it would let a dropped comment be masked as `known` — see the
		// safety reclassification in corpus_compare_format.ts). They are an uncovered
		// "we preserve, Prettier drops" divergence, not a relocation, and surface
		// in `divergence:audit` as uncovered rather than being falsely claimed.
	],
	detect(ctx) {
		const js_comment_pattern = /\/\/|\/\*|\*\//;
		const ours_lines = ctx.ours_lines!;
		const prettier_lines = ctx.prettier_lines!;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const added_comment_lines = hunk.added_lines.filter((l) => js_comment_pattern.test(l));
			const removed_comment_lines = hunk.removed_lines.filter((l) => js_comment_pattern.test(l));

			// Case 3: Comment-driven STRUCTURAL relocation. Some sanctioned
			// comment-position divergences relocate a comment whose text is identical
			// in both outputs, so the diff aligns it as a CONTEXT (same) line and the
			// hunk carries only the structural reshape it triggered (empty-`switch`
			// discriminant parens, the `} else {` split, a member chain breaking
			// before a trailing-member comment). The comment is then NOT inside the
			// hunk — it borders it.
			//
			// Claim such a hunk only when a whole-comment-line is the IMMEDIATE border
			// of the hunk in BOTH outputs, that comment text exists as a whole comment
			// line in BOTH (the content guard: a comment prettier or ours DROPPED can
			// never satisfy "exists as a whole line in both", so a data-loss is never
			// masked — the same guarantee Case 1/2 rely on), AND the comment genuinely
			// RELOCATED: both its immediate neighbors differ between the two outputs.
			// The relocation check is what separates a true position divergence (the
			// comment landed in a different syntactic container — empty-`switch`
			// parens vs body, before vs after `else`, mid-chain vs after `=`) from a
			// STABLE comment that merely borders a width re-wrap (where one neighbor —
			// e.g. a blank line or the unchanged statement above — stays identical).
			if (added_comment_lines.length === 0 && removed_comment_lines.length === 0) {
				const ours_border = border_comment_contents(ours_lines, hunk.ours_range);
				const prettier_border = border_comment_contents(prettier_lines, hunk.prettier_range);
				if (ours_border.length === 0 || prettier_border.length === 0) return false;
				const prettier_border_set = new Set(prettier_border);
				return ours_border.some((text) => {
					if (text.length < 3 || !prettier_border_set.has(text)) return false;
					if (
						!comment_line_exists_in_output(ctx.ours, text) ||
						!comment_line_exists_in_output(ctx.prettier, text)
					) return false;
					// Relocation evidence: both neighbors of the comment differ AND
					// neither neighbor merely BEGINS THE SAME ELEMENT as its counterpart
					// (which would be a stable comment bordering a width re-wrap of the
					// element it precedes, not a relocation). A genuine relocation lands
					// the comment among entirely different tokens on both sides.
					const o = comment_line_neighbors(ours_lines, text);
					const p = comment_line_neighbors(prettier_lines, text);
					return o !== null && p !== null &&
						o.prev !== p.prev && o.next !== p.next &&
						!lines_begin_same_element(o.prev, p.prev) &&
						!lines_begin_same_element(o.next, p.next);
				});
			}

			// Case 1: Comment on one side only — verify it was MOVED (appears as a
			// WHOLE comment line in the other side's output), not incidentally
			// included by reformatting. Whole-comment matching (not the looser
			// prefix-substring form) keeps the text from matching inside a string
			// literal, a longer comment, or a JSDoc continuation — which directly
			// feeds the safety reclassification, so it must not over-match.
			if (added_comment_lines.length > 0 && removed_comment_lines.length === 0) {
				return added_comment_lines.some((l) => {
					const text = extract_comment_content(l);
					return text.length >= 3 && comment_line_exists_in_output(ctx.prettier, text);
				});
			}
			if (removed_comment_lines.length > 0 && added_comment_lines.length === 0) {
				return removed_comment_lines.some((l) => {
					const text = extract_comment_content(l);
					return text.length >= 3 && comment_line_exists_in_output(ctx.ours, text);
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

			// Fallback: a preserved line comment inside a parenthesized
			// union/intersection member forces that member to expand to its broken
			// leading-`|`/`&` form (the retained-paren-union-line-comment
			// divergence), while Prettier keeps it inline and relocates the comment.
			// The expansion keeps the parens and only rearranges the inner
			// separator layout — strip comments, separators (`|`/`&`), and
			// whitespace from both sides (KEEP parens, so a genuine paren-wrapping
			// reformat with incidental comments is not equalized). If the remaining
			// content is identical AND ours did not DROP a separator (ours `|`/`&`
			// count >= prettier's — the expansion only ever ADDS them), the hunk is
			// purely comment-driven union/intersection layout. The separator-count
			// guard keeps a genuine dropped-`|`/`&` (content loss) from being masked.
			const strip_layout = (lines: string[]) =>
				lines.map(strip_comments).join('').replace(/[|&\s]/g, '');
			const count_separators = (lines: string[]) =>
				lines.map(strip_comments).join('').match(/[|&]/g)?.length ?? 0;
			const layout_added = strip_layout(hunk.added_lines);
			if (
				layout_added.length > 0 &&
				layout_added === strip_layout(hunk.removed_lines) &&
				count_separators(hunk.added_lines) >= count_separators(hunk.removed_lines)
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

const single_type_param_comma: DivergencePattern = {
	id: 'single_type_param_comma',
	description:
		'Single unconstrained arrow type param stays bare `<T>`; prettier-in-Svelte forces `<T,>`',
	languages: ['svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: [
		'typescript/expressions/arrow/generic/single_type_param_prettier_divergence',
		'typescript/typescript_specific/generics/const_type_param_arrow_prettier_divergence',
	],
	detect(ctx) {
		// Svelte only: prettier force-adds the JSX-disambiguating comma to a single
		// unconstrained arrow type param when it has no `.ts` filepath — exactly the
		// embedded-Svelte case. On the pure-.ts path prettier strips it, so tsv and
		// prettier agree and there is nothing to detect.
		if (ctx.language !== 'svelte') return null;

		// Prettier: `<T,>` / `<T = string,>` / `<const T,>` — a single type param (no
		// interior `<`, `>`, or `,`) immediately followed by `,>` on the same line. A
		// wrapped multi-line list puts the comma and `>` on different lines, so it can't
		// match here (and tsv emits that trailing comma too — not a divergence).
		const prettier_comma = /<([^<>,\n]+),>/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			for (const removed of hunk.removed_lines) {
				const m = prettier_comma.exec(removed);
				if (!m) continue;
				// Ours has the same construct without the disambiguating comma.
				const bare = `<${m[1]}>`;
				if (hunk.added_lines.some((added) => added.includes(bare))) return true;
			}
			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'single_type_param_comma',
				confidence: 'certain',
				hunk_indices,
				reason:
					'Single unconstrained arrow type param stays bare `<T>` (tsv emits no JSX); prettier-in-Svelte forces `<T,>`',
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
	description: 'JSDoc type cast parens preserved (prettier-TS strips)',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['JSDoc / paren semantics'],
	fixtures: ['typescript/syntax/comments/jsdoc_type_cast_ts_prettier_divergence'],
	detect(ctx) {
		// JSDoc type casts (`/** @type {T} */ (expr)`) are a TypeScript assertion
		// whose parens are semantically required. tsv preserves them everywhere;
		// prettier's oxc-ts backend strips them in TS contexts (`.ts`, `lang="ts"`).
		// In plain-JS `<script>` prettier preserves too, so that's a match — only
		// the TS-context direction (ours keeps / prettier drops) is a divergence.
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;

		// We keep parens: /** @type {T} */ (expr)
		// Prettier (oxc-ts) strips them: /** @type {T} */ expr
		const jsdoc_cast_with_parens = /@(?:type|satisfies)\s*\{[^}]*\}\s*\*\/\s*\(/;
		const jsdoc_cast_without_parens = /@(?:type|satisfies)\s*\{[^}]*\}\s*\*\/\s*[^(]/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const prettier_without_parens = hunk.removed_lines.some((l) =>
				jsdoc_cast_without_parens.test(l)
			);
			const ours_with_parens = hunk.added_lines.some((l) => jsdoc_cast_with_parens.test(l));
			return prettier_without_parens && ours_with_parens;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'jsdoc_type_cast_parens',
				confidence: 'certain',
				hunk_indices,
				reason: 'JSDoc type cast parens preserved (required for the cast; prettier-TS strips)',
			};
		}
		return null;
	},
};

/**
 * Tabs-only alignment: tsv renders Prettier's sub-tab alignment (a closing
 * delimiter at `tabs + 2 spaces`) as a whole tab instead. Same visual width at
 * `tab_width = 2`, so each changed line pairs a prettier `\t+ +X` (tabs then
 * spaces) with an identical-content ours `\t+X` (pure tabs). Surfaces wherever
 * Prettier's `align(2, …)` lands a delimiter at the alignment column — union
 * members with breaking object/generic types, parenthesized intersections, etc.
 */
const tabs_only_alignment: DivergencePattern = {
	id: 'tabs_only_alignment',
	description: 'Sub-tab alignment rendered as whole tabs (no tabs+spaces mix)',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['Tabs-Only Alignment (No Sub-Tab Spaces)'],
	fixtures: [
		'typescript/types/union_object_member_prettier_divergence',
		'typescript/types/union_hug_object_prettier_divergence',
		'typescript/types/union_parens_object_prettier_divergence',
		'typescript/types/union_intersection_object_long_prettier_divergence',
		'typescript/types/nested_generic_member_long_prettier_divergence',
		'typescript/types/union_fn_type_member_long_prettier_divergence',
		'typescript/types/union_paren_union_member_long_prettier_divergence',
		'typescript/types/comments/union_member_long_line_comment_prettier_divergence',
		'typescript/types/comments/union_paren_member_long_line_comment_prettier_divergence',
	],
	detect(ctx) {
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Whole hunk must be reindent-only: each prettier (removed) line pairs
			// 1:1 with an ours (added) line of identical trailing content, where
			// prettier's indent is tabs-then-spaces and ours is pure tabs of equal
			// visual width. Requiring every pair avoids claiming mixed hunks.
			if (
				hunk.removed_lines.length === 0 ||
				hunk.removed_lines.length !== hunk.added_lines.length
			) {
				return false;
			}
			return hunk.removed_lines.every((removed, i) => {
				const added = hunk.added_lines[i];
				if (removed.trimStart() !== added.trimStart()) return false;
				const removed_lead = removed.slice(0, removed.length - removed.trimStart().length);
				const added_lead = added.slice(0, added.length - added.trimStart().length);
				// prettier: tabs then ≥1 space; ours: pure tabs (≥1)
				if (!/^\t+ +$/.test(removed_lead) || !/^\t+$/.test(added_lead)) return false;
				return visual_width(removed_lead) === visual_width(added_lead);
			});
		});
		if (hunk_indices.length === 0) return null;
		return {
			pattern: 'tabs_only_alignment',
			confidence: 'certain',
			hunk_indices,
			reason: 'Prettier sub-tab alignment (tabs + spaces) rendered as whole tabs',
		};
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
	css_unit_serialize_case,
	css_atrule_spec_spacing,
	css_atrule_long_wrap,
	css_atrule_stable_quirk,
	css_scss_directive_number,
	css_selector_divergence,
	css_comment_stable_quirk,

	// 3. Feature-specific patterns
	template_literal_width,
	block_expression_logical,
	single_specifier_import,
	member_expression_call,
	return_type_generic_union,
	non_null_paren_base,
	tabs_only_alignment,
	annotation_continuation_indent,

	// 4. Svelte-specific patterns
	menu_block,
	inline_content_hug,
	inline_content_block_style,
	fill_after_inline,
	block_multiline_attrs_hug,
	comment_preserved,
	short_expr_100,

	// 5. Semantic preservation patterns
	instantiation_parens,
	single_type_param_comma,
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
