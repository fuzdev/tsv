/**
 * Divergence pattern detection - identify known intentional differences from Prettier.
 *
 * Each pattern corresponds to a documented divergence in conformance_prettier.md.
 * These are NOT bugs - they are design choices.
 *
 * Patterns are ordered from most specific to most broad, but that ordering is
 * PRESENTATIONAL, not semantic: `detect_divergences` runs every pattern and
 * records every match — it does not stop at the first, and no pattern suppresses
 * another. Multiple patterns CAN claim the same hunk, and each computed field
 * (`explained_hunks`, `unexplained_hunks`, `classification`, `safety_vouched`)
 * is set-based, so a reordering yields byte-identical results. What the order
 * buys is that the most specific pattern is named FIRST where matches are joined
 * for display, which is the useful thing when triaging a file with several.
 */

import type { DiffHunk, DiffLine } from '../diff.ts';
import type { Language } from '../types.ts';
import { hunk_alters_semantic_chars } from './safety.ts';

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
	/** Pre-computed <script>/<style> regions for Svelte files (char + line spans) */
	ours_code_regions?: CodeRegion[];
	prettier_code_regions?: CodeRegion[];
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
	/**
	 * Whether this pattern's transformation can legitimately change semantic character
	 * counts — adding or removing letters, digits, or brackets, as opposed to only
	 * reflowing whitespace/quotes/commas/parens (the chars SAFETY already excludes).
	 *
	 * Only a pattern declaring this may vouch for a hunk that carries a SAFETY
	 * differential (see `detect_divergences`'s `safety_vouched`). Optional and defaulting
	 * to `false` so the gate fails CLOSED: a pattern that has not thought about the
	 * question cannot excuse content loss, and a new pattern is safe by omission.
	 *
	 * Setting this `true` is a promise that the pattern's own `detect` carries a
	 * content-preservation proof for whatever it claims — as `bom_strip` (byte-exact BOM
	 * prefix test), `self_closing_nonvoid` (matching tag names on both sides) and
	 * `comment_preserved` (the comment text must appear in ours) each do.
	 */
	may_alter_char_frequency?: boolean;
	/** Detection function */
	detect: (ctx: DetectionContext) => DivergenceMatch | null;
}

/**
 * Calculate visual width of a line (tabs = 2 spaces).
 *
 * Exported so the tests measure width the same way the detectors do — a second
 * copy there would let the two drift, and every width-keyed pattern is judged
 * against it.
 */
export function visual_width(line: string): number {
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
	/**
	 * Whether this coverage may excuse a file-level SAFETY differential.
	 *
	 * Stricter than `classification === 'all_explained'`, and deliberately a separate
	 * question: every hunk that actually moves the semantic character count must be
	 * claimed by a pattern declaring `may_alter_char_frequency`. Whitespace-only hunks
	 * still need explaining for the ordinary `partial`/`unknown` bucketing, but they can
	 * no longer prop up a SAFETY downgrade they had no part in causing.
	 */
	safety_vouched: boolean;
	/** Indices of hunks whose own added/removed lines move the semantic char count */
	char_risky_hunks: number[];
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
 * A `<script>` or `<style>` region — the two places in a Svelte file whose
 * bytes are program/string content rather than template markup.
 *
 * Carries both coordinate systems because its two consumers need different
 * ones: the boundary-whitespace collapse splices the raw text by CHAR offset,
 * while the hunk-level checks compare LINE ranges. Both are computed in the
 * same pass (see `compute_code_regions`) and cached per side, so neither
 * consumer rescans.
 *
 * The non-greedy body means a `</script>` inside a string ends the region
 * early, erring toward a SMALLER region — which under-claims rather than
 * over-claims for every consumer.
 */
interface CodeRegion {
	kind: 'script' | 'style';
	/** Char offsets into the side's full text, `[start, end)`. */
	start: number;
	end: number;
	/** 0-based INCLUSIVE line range the region spans. */
	line_start: number;
	line_end: number;
}

const raw_code_regions = /<(script|style)\b[^>]*>[\s\S]*?<\/\1\s*>/gi;

/**
 * The `<script>`/`<style>` regions of `text`, in source order.
 *
 * One linear pass: `matchAll` yields non-overlapping matches in increasing
 * index order, so the newline counter only ever moves forward and the char →
 * line conversion costs one scan of the text total, not one per region.
 */
function compute_code_regions(text: string): CodeRegion[] {
	const regions: CodeRegion[] = [];
	let scanned = 0;
	let line = 0;
	const line_at = (offset: number): number => {
		for (let i = scanned; i < offset; i++) {
			if (text.charCodeAt(i) === 10) line++;
		}
		scanned = offset;
		return line;
	};
	for (const m of text.matchAll(raw_code_regions)) {
		const start = m.index;
		const end = start + m[0].length;
		regions.push({
			kind: m[1].toLowerCase() as CodeRegion['kind'],
			start,
			end,
			line_start: line_at(start),
			line_end: line_at(end),
		});
	}
	return regions;
}

/** Whether a line index falls inside any `<style>` region. */
function is_line_in_style_block(line: number, regions: CodeRegion[]): boolean {
	return regions.some((r) => r.kind === 'style' && line >= r.line_start && line <= r.line_end);
}

/** Whether a hunk's line range overlaps any code region (either kind). */
function overlaps_code_region(
	range: { start: number; end: number } | null,
	regions: CodeRegion[],
): boolean {
	return (
		range !== null && regions.some((r) => range.start <= r.line_end && r.line_start <= range.end)
	);
}

/**
 * Pre-compute cached fields on a DetectionContext.
 * Called by detect_divergences before running patterns.
 */
export function enrich_detection_context(ctx: DetectionContext): void {
	ctx.ours_lines = ctx.ours.split('\n');
	ctx.prettier_lines = ctx.prettier.split('\n');
	if (ctx.language === 'svelte') {
		ctx.ours_code_regions = compute_code_regions(ctx.ours);
		ctx.prettier_code_regions = compute_code_regions(ctx.prettier);
	} else {
		ctx.ours_code_regions = [];
		ctx.prettier_code_regions = [];
	}
}

/**
 * Check if a hunk's context is within a CSS context.
 * For Svelte files, uses the pre-computed `<style>` regions.
 * For removal-only hunks, checks prettier's regions (not ours).
 */
function is_in_css_context(hunk: DiffHunk, ctx: DetectionContext): boolean {
	if (ctx.language === 'css') return true;
	if (ctx.language !== 'svelte') return false;

	// Use ours range when available; for removal-only hunks, use prettier range
	// against prettier's regions (fixes line index mismatch)
	if (hunk.ours_range) {
		return is_line_in_style_block(hunk.ours_range.start, ctx.ours_code_regions ?? []);
	}
	if (hunk.prettier_range) {
		return is_line_in_style_block(hunk.prettier_range.start, ctx.prettier_code_regions ?? []);
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

/** A line's leading whitespace, the unit indent comparisons are made in. */
function leading_ws(line: string): string {
	return /^[ \t]*/.exec(line)![0];
}

/**
 * Whether our side places a pure-re-indent hunk exactly ONE indent level below
 * the construct head above it — one tab, the only indent tsv emits.
 *
 * Measured against the HEAD, not against prettier's leading whitespace, because
 * that is how §Uniform Forced-Continuation Indent states the rule: the
 * continuation is indented one level so it "reads as part of its construct". What
 * prettier chose to emit is the divergence, so it cannot also be the baseline —
 * keying on it made the gate reject a continuation tsv had placed correctly
 * merely because prettier's own line was oddly indented (`\t {};`, a tab plus a
 * stray space).
 *
 * @param hunk - The pure-re-indent hunk under test
 * @param head - Our line immediately above it (the construct the comment split)
 */
function indents_one_level_below(hunk: DiffHunk, head: string): boolean {
	const added = hunk.added_lines;
	const removed = hunk.removed_lines;
	if (added.length === 0) return false;

	// The continuation's FIRST line lands exactly one level below the head.
	const base = leading_ws(head) + '\t';
	if (leading_ws(added[0]) !== base) return false;

	// Every following line is re-rooted onto that base keeping its OWN relative
	// depth. A continuation can be multi-line with internal structure — an
	// intersection hangs its members a further level in — and tsv shifts the whole
	// block, so requiring every line at `base` would reject the very case the
	// pattern was originally written for.
	const removed_base = leading_ws(removed[0]);
	for (let i = 1; i < added.length; i++) {
		const removed_ws = leading_ws(removed[i]);
		if (!removed_ws.startsWith(removed_base)) return false;
		if (leading_ws(added[i]) !== base + removed_ws.slice(removed_base.length)) return false;
	}
	return true;
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
 * Every line-comment text on a line, undoing prettier's MERGE of several
 * trailing line comments onto one.
 *
 * `extract_comment_content` takes everything after the first `//`, which is
 * right for one comment but wrong for the merged form: relocating two trailing
 * line comments onto a single line (`a // c1 // c2`) makes the second `//` mere
 * TEXT of the first — the information-losing merge tsv deliberately diverges
 * from by preserving position and continuation-indenting instead
 * (`docs/conformance_prettier.md` §Comment Position Philosophy). Read as one
 * comment, that side's text (`c1 // c2`) matches neither of ours, so the
 * detector missed precisely the canonical instances of the family.
 *
 * Splitting on the inner `//` inverts the merge. A `//` preceded by `:` is
 * skipped so a URL inside a comment (`// see http://x`) stays one text rather
 * than splitting into two spurious ones.
 *
 * Returns `[]` for a line with no `//` (a block comment — the caller falls back
 * to `extract_comment_content`).
 */
export function extract_line_comment_contents(line: string): string[] {
	const start = line.indexOf('//');
	if (start === -1) return [];
	const body = line.slice(start + 2);

	const parts: string[] = [];
	let current = '';
	for (let i = 0; i < body.length; i++) {
		if (body[i] === '/' && body[i + 1] === '/' && body[i - 1] !== ':') {
			parts.push(current);
			current = '';
			i++; // skip the second `/`
			continue;
		}
		current += body[i];
	}
	parts.push(current);

	return parts.map((p) => p.trim()).filter((p) => p.length > 0);
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

/**
 * Line indices in `lines` carrying a tsv-native `format-ignore` directive.
 *
 * Deliberately NOT the `prettier-ignore` family. Both spellings suppress
 * formatting in tsv, but prettier honors only its own — so a `prettier-ignore`d
 * construct is preserved by BOTH tools and produces no divergence at all. Only
 * the tsv-native spelling explains one, which is what keeps this keyed on the
 * actual cause rather than on "an ignore-ish comment is nearby".
 */
function format_ignore_directive_lines(lines: string[]): number[] {
	// The trimmed-content match mirrors `tsv_lang::is_format_ignore_directive` and
	// its two range siblings — the Rust side is the source of truth for the set.
	const directive = /(?:\/\/|\/\*|<!--)\s*format-ignore(?:-start|-end)?\s*(?:\*\/|-->)?\s*$/;
	const found: number[] = [];
	for (let i = 0; i < lines.length; i++) if (directive.test(lines[i])) found.push(i);
	return found;
}

const format_ignore_preserved: DivergencePattern = {
	id: 'format_ignore_preserved',
	description:
		'tsv honors a `format-ignore` directive and emits the construct verbatim; prettier does not recognize it and reformats',
	languages: ['typescript', 'css', 'svelte'],
	conformance_sections: ['Format-ignore directive'],
	fixtures: [
		'typescript/syntax/comments/format_ignore_prettier_divergence',
		'css/syntax/comments/format_ignore_prettier_divergence',
		'svelte/syntax/format_ignore/basic_prettier_divergence',
		'svelte/syntax/format_ignore/js_css_prettier_divergence',
		'svelte/syntax/format_ignore/css_nested_prettier_divergence',
		'svelte/syntax/format_ignore/css_atrule_decl_prettier_divergence',
		'svelte/syntax/format_ignore/range_prettier_divergence',
	],
	detect(ctx) {
		const ours_lines = ctx.ours_lines!;

		// SAFETY GATE — the entire ours/prettier difference is whitespace-only, so no
		// content can be lost: suppressing formatting provably only preserves the
		// author's layout. A single non-whitespace difference anywhere (a dropped
		// comment, a normalized quote, a real content change) fails the gate and
		// disables the detector, so it can never mask a content loss. This is also
		// what makes claiming a whole region sound rather than merely plausible — the
		// same proof `inline_content_block_style` rests on.
		if (strip_all_ws(ctx.ours) !== strip_all_ws(ctx.prettier)) return null;

		// FAMILY SIGNATURE — an actual tsv-native directive, in our output.
		const directives = format_ignore_directive_lines(ours_lines);
		if (directives.length === 0) return null;

		// Claim only hunks at or below the first directive. A divergence ABOVE every
		// directive cannot have been caused by one, and leaving it unclaimed keeps the
		// file `partial` — which is the honest verdict — instead of quietly absorbing
		// an unrelated layout difference into `known`.
		const first = directives[0];
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const start = hunk.ours_range?.start;
			return start != null && start >= first;
		});

		if (hunk_indices.length === 0) return null;
		return {
			pattern: 'format_ignore_preserved',
			confidence: 'certain',
			hunk_indices,
			reason: 'construct preserved verbatim under a `format-ignore` directive prettier does not honor',
		};
	},
};

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
	// U+FEFF is not in FORMATTING_CHARS, so stripping it moves the semantic count. The
	// detect below is byte-exact (source starts with the BOM, ours does not, prettier does).
	may_alter_char_frequency: true,
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
	fixtures: [
		'svelte/elements/self_closing_nonvoid_prettier_divergence',
		'svelte/elements/ws_sensitive_self_closing_kinds_prettier_divergence',
	],
	// `<i … />` → `<i …></i>` adds `<`, `/`, `>` and the tag name — real semantic chars.
	// The detect below proves preservation by matching the tag NAME on both sides.
	may_alter_char_frequency: true,
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

const attr_value_single_quote: DivergencePattern = {
	id: 'attr_value_single_quote',
	description: 'Attribute / style: / this= value with a literal " kept single-quoted',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Attributes'],
	fixtures: [
		'svelte/attributes/value_double_quote_prettier_divergence',
		'svelte/directives/style/value_double_quote_prettier_divergence',
		'svelte/special_elements/svelte_element_this_double_quote_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// tsv emits a quoted attribute / `style:` / `this=` value with SINGLE-quote
		// delimiters exactly when the value contains a literal `"` (double quotes
		// cannot hold it — HTML §13.1.2.3); prettier-plugin-svelte re-quotes with `"`
		// and corrupts. The unique fingerprint on OURS is a single-quoted value
		// carrying a `"` — every other value is double-quoted, so this shape appears
		// only for this divergence. Pair it with prettier's double-quoted form of the
		// same attribute name to stay airtight (a JS string in `<script>` never
		// produces the paired prettier form, since prettier also single-quotes it).
		const ours_single_dq = /(?:^|\s)([\w:@.-]+)='[^']*"[^']*'/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			for (const line of hunk.added_lines) {
				const m = ours_single_dq.exec(line);
				if (!m) continue;
				const name = m[1].replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
				const prettier_dq = new RegExp(`${name}="[^"]*"`);
				if (hunk.removed_lines.some((l) => prettier_dq.test(l))) return true;
			}
			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'attr_value_single_quote',
				confidence: 'certain',
				hunk_indices,
				reason: 'Value with a literal " kept single-quoted (prettier corrupts to double quotes)',
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
	// No fixture, and no pattern claims one. `empty_standalone` was listed here but
	// pins the BLANK LINES left behind — both formatters drop the `;`, so the
	// removed-standalone-`;` test below can never fire on it. It stays honestly
	// uncovered in `divergence:audit` rather than forced into an allowlist; a
	// blank-line-collapse detector would be the real fix. The pattern itself is
	// LIVE (3 corpus files via `--audit-patterns`), so it earns its keep regardless.
	fixtures: [],
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

			// @scope with spacing quirks (spaces inside parens, double spaces around
			// to, or a comma/combinator the author wrote tight)
			if (/@scope/.test(removed_joined) || /@scope/.test(added_joined)) {
				// Prettier adds spaces inside scope parens: ( .class ) vs (.class)
				const removed_has_quirk = hunk.removed_lines.some((l) =>
					/@scope/.test(l) &&
					(/\( /.test(l) || / \)/.test(l) || /\s{2,}to\s{2,}/.test(l) ||
						// tight comma / combinator preserved: (.x,.y), (a>b), (.x)to(.y)
						/,\S/.test(l) || /\S[>+~]\S/.test(l) || /\)to\(/.test(l))
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
	// Re-spelling a number changes digit counts (`.5` ↔ `0.5`). The detect below carries
	// the matching proof: identical non-numeric skeletons AND equal numeric-token counts,
	// so a number can be re-spelled but never dropped or added.
	may_alter_char_frequency: true,
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
		'typescript/expressions/literals/template/interpolation_nested_template_prettier_divergence',
		'typescript/types/template_literal_type_long_prettier_divergence',
		// TODO: `template_literal_type_conditional_long` was listed here but the
		// break markers below (`${` at EOL, `}\`` at line start) don't describe its
		// shape — the conditional type breaks at `?`/`:` INSIDE the interpolation.
		// `fill_101_boundary` claims it today; a marker for that shape would be the
		// real fix, but it must not swallow ordinary ternary breaks.
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
	// TODO: no fixture. `last_block` moved to `svelte_boundary_ws_trim`, which is
	// the divergence it actually pins (block-boundary space glue); this pattern
	// keys on a leading `&&`/`||`, which that fixture has none of.
	fixtures: [],
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
	// No fixture: `inline_content_hug_long` moved to `inline_content_block_style` —
	// inline content now lays out block-style, so that fixture records prettier
	// dangling the delimiter, not us hugging. The pattern itself is still LIVE —
	// `--audit-patterns` puts it at 31 corpus files — so only the listing was stale.
	fixtures: [],
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

const comment_preserved: DivergencePattern = {
	id: 'comment_preserved',
	description: 'We preserve a comment inside {…}/a tag that Prettier drops',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Attributes', 'Svelte: Elements'],
	// Keeping content prettier drops means ours has MORE semantic chars, by design. The
	// detect below requires the comment text to actually appear in our output.
	may_alter_char_frequency: true,
	fixtures: [
		'svelte/syntax/comments/expr_trailing_prettier_divergence',
		'svelte/syntax/comments/expr_trailing_line_prettier_divergence',
		'svelte/tags/debug/debug_comment_prettier_divergence',
		'svelte/tags/debug/debug_comma_comment_prettier_divergence',
		// Multi-line block comments — the shape the per-line pass cannot see, so these
		// pin the joined path specifically.
		'svelte/tags/debug/debug_multiline_comment_prettier_divergence',
		'svelte/expression_tag/paren_multiline_comment_prettier_divergence',
		'svelte/tags/html_render_paren_multiline_comment_prettier_divergence',
		'svelte/directives/value_paren_multiline_comment_prettier_divergence',
		'svelte/attributes/attach_spread_paren_multiline_comment_prettier_divergence',
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

			// A comment prettier dropped may span SEVERAL of our lines, and then no
			// single line carries a strippable `/* … */` at all — the opener sits on
			// one line and the closer on the next, so the per-line pass above sees
			// `{@debug /* c` and ` */ x}`, neither of which strips to anything. Join
			// the whole hunk per side and compare once: the comment is then complete
			// and strips cleanly.
			//
			// Prettier's side must carry NO comment, which is what makes this a DROP
			// rather than a relocation — the per-line pass gets that guard for free by
			// filtering commented lines out of `removed_code`, and joining loses it.
			// Without it the joined compare also matches a comment prettier MOVED (both
			// sides hold it, so the stripped code is equal either way) — e.g. an indexed
			// access where prettier hoists the comment out of the brackets. That is a
			// relocation for `comment_position` to claim, and claiming it here would be
			// doubly wrong: this pattern declares `may_alter_char_frequency`, so a
			// mis-claim can vouch a SAFETY differential, yet a relocation moves no chars
			// at all and the "ours has MORE semantic chars" justification does not hold.
			const removed_text = hunk.removed_lines.join('\n');
			if (has_comment(removed_text)) return false;
			const added_text = added.join('\n');
			if (!has_comment(added_text)) return false;
			const added_code = strip_code(added_text);
			if (added_code === '') return false;
			return added_code === strip_code(removed_text);
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
		'svelte/blocks/if/inline_element_long_prettier_divergence',
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

/**
 * Which §Uniform Forced-Continuation Indent site a re-indent sits under, or null.
 *
 * The rule is one rule — a **line** comment runs to end-of-line, so whatever the
 * author wrote after it cannot stay on that line; tsv keeps the comment where it
 * was written and drops the following token to a continuation line indented one
 * level, where prettier keeps it flush. The clauses below are the sites the doc
 * enumerates, each keyed on the line PRECEDING the hunk — the construct head the
 * comment split.
 *
 * Keying on that preceding line is what makes the detector safe to widen: an
 * ordinary indentation bug (a wrong conditional-type body indent, say) has no
 * comment above it and is never claimed, so this cannot mask the tsv defect class
 * it most resembles.
 *
 * @param prev_ours - Our line immediately above the hunk (the split construct head)
 * @param first_added - The hunk's first re-indented line, on our side
 */
function forced_continuation_site(prev_ours: string, first_added: string): string | null {
	// `: Type` annotations — a `:` after an annotation target (identifier / `)` /
	// `]` / `}` / `>`) carrying a trailing line comment, via the shared
	// `build_type_annotation_doc`. A line-leading `:` (a ternary branch) is excluded
	// by requiring the preceding word/closer. Block comments may sit in the gap
	// between the two (`[k: T] /* x */ : // c`) — a `/* */` does not run to
	// end-of-line, so it never forces the break and only ever separates the target
	// from its colon.
	if (/[\w)\]}>][ \t]*(?:\/\*[^*]*\*\/[ \t]*)*:[ \t]*\/\//.test(prev_ours)) {
		return 'colon→type annotation';
	}

	// Declaration and module headers — an `import`/`export` header gap whose
	// comment forces the tail (source, declarator, binding) onto its own line.
	if (/^[ \t]*(?:import|export)\b.*\/\//.test(prev_ours)) return 'module header';

	// The DECLARATION half of the same doc bullet ("keyword→name"): a line comment
	// in a declaration's keyword-to-name gap (`function // c⏎f()`, `class // c⏎C {}`,
	// `enum // c⏎E {}`), which drops the name to a continuation indented one level
	// where prettier keeps it flat. The keyword must IMMEDIATELY precede the comment,
	// so a body-open-brace comment (`function f() { // c`) — an entirely different
	// gap — cannot match.
	//
	// The keyword set is closed to the three that are RESERVED words, because those
	// are the only ones with a witness. `interface`/`namespace`/`module`/`type` are
	// contextual keywords: with the name on the next line the construct is not a
	// declaration at all — Svelte's parser and prettier both REJECT the first three,
	// and all four tools agree to read `type` as an expression statement (`type;`),
	// so none of them can produce this divergence.
	if (/\b(?:function|class|enum)\b\*?[ \t]*\/\//.test(prev_ours)) return 'declaration header';

	// Prefix type operators — the `keyof` / `typeof` / `infer` operand hang, shared
	// via `append_keyword_value_line_comments`. The keyword must immediately precede
	// the comment, so a `typeof` elsewhere on a longer line does not qualify.
	if (/\b(?:keyof|typeof|infer)[ \t]*\/\//.test(prev_ours)) return 'prefix type operator';

	// Before-`:` key/binding gap — the complement of the annotation case: the
	// comment sits after the key (or its `?`/`!` marker) and the whole `: type`
	// continuation drops a level, via `build_marker_colon_line_continuation`. The
	// continuation LEADING with `:` is the discriminator; without it a bare trailing
	// comment above an indented line would match almost anything.
	if (/\/\//.test(prev_ours) && /^[ \t]*:/.test(first_added)) return 'key→colon gap';

	// No clause for an OWN-LINE comment leading the continuation: where the author
	// wrote the comment on its own line, prettier relocates the comment itself, so
	// the hunk carries that relocation and is not a pure re-indent at all —
	// `comment_position` claims it. A clause here would have no witness.
	return null;
}

const forced_continuation_indent: DivergencePattern = {
	id: 'forced_continuation_indent',
	description:
		'tsv indents a comment-forced continuation one level (annotation, declaration/module header, prefix type operator, key→`:` gap); prettier keeps it flush',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['Uniform Forced-Continuation Indent', 'Comment Position Philosophy'],
	fixtures: [
		'typescript/types/comments/annotation_continuation_indent_prettier_divergence',
		'typescript/modules/imports/source_line_comment_prettier_divergence',
		'typescript/modules/exports/source_line_comment_prettier_divergence',
		'typescript/modules/exports/empty_no_from_line_comment_prettier_divergence',
		'typescript/types/infer/keyword_line_comment_prettier_divergence',
		'typescript/types/type_operator_keyword_line_comment_prettier_divergence',
		'typescript/types/type_members/index_signature_key_colon_line_comment_prettier_divergence',
		'typescript/types/type_members/index_signature_bracket_colon_value_line_comment_prettier_divergence',
		'typescript/declarations/class/index_signature_bracket_line_comment_positions_prettier_divergence',
		// The declaration-header clause's only witness, so it is listed deliberately:
		// if this fixture stops being claimed the clause is dead, and nothing else
		// would say so.
		'typescript/syntax/comments/keyword_name_line_comment_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;
		const ours_lines = ctx.ours_lines!;

		// Which sites fired, for the reason line — a file's continuations can come from
		// more than one gap, and naming them is what makes `--explain` actionable.
		const sites = new Set<string>();
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Indentation-only by construction, so claiming the hunk can never mask a
			// content change — the whole basis for this detector being safe to widen.
			if (!is_pure_reindent(hunk)) return false;
			const start = hunk.ours_range?.start;
			if (start == null || start === 0) return false;
			const head = ours_lines[start - 1] ?? '';
			// "one level" below the head, as the rule states it: any other depth is a
			// different layout difference and stays unclaimed.
			if (!indents_one_level_below(hunk, head)) return false;
			const site = forced_continuation_site(head, hunk.added_lines[0]);
			if (site === null) return false;
			sites.add(site);
			return true;
		});

		if (hunk_indices.length === 0) return null;
		return {
			pattern: 'forced_continuation_indent',
			confidence: 'likely',
			hunk_indices,
			reason: `comment-forced continuation indents one level where prettier keeps it flush (${[...sites].join(', ')})`,
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
		'svelte/elements/inline_content_hug_long_prettier_divergence',
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
		// The tag-name class admits `:` and `.` — a dangled `</svelte:element` or `</Foo.Bar` is
		// the same marker as `</div`, and a Svelte closing tag can carry either.
		const dangle_close = /<\/[A-Za-z][\w.:-]*[ \t]*$/; //               `</tag` at EOL
		const dangle_open = /^[ \t]*>/; //                                  `>` starts a line
		const block_head_alone = /^[ \t]*\{#(?:if|each|await|key|snippet)\b[^}]*\}[ \t]*$/;
		// The block body boundary is render-free, so tsv breaks it whenever the body renders
		// multiline while prettier welds the body to the tag. Two more ours-side markers for
		// that break, neither of which the `alone` head marker sees:
		//   - the head ENDS an our-line that has a prefix (a preceding sibling the block hugs
		//     — `{fn(x)}{#if …}` — where prettier hugged the body onto that same line); and
		//   - a branch / close tag (`{:else}`, `{:else if …}`, `{:then …}`, `{:catch …}`,
		//     `{/if}`, …) sits ALONE on an our-line, where prettier welded it to the body.
		// Both are produced only by tsv breaking a boundary prettier keeps hugged. The other
		// whitespace-only svelte divergences carry neither (verbatim `prettier-ignore` /
		// region-markers, `<svelte:element>` attr wrap, an html template literal) — they have
		// no block tag on a changed line at all.
		const block_head_at_eol = /\{#(?:if|each|await|key|snippet)\b[^}]*\}[ \t]*$/;
		const block_branch_alone = /^[ \t]*\{[:/](?:else|then|catch|if|each|await|key|snippet)\b[^}]*\}[ \t]*$/;
		let has_signature = false;
		for (const hunk of ctx.hunks) {
			if (
				hunk.removed_lines.concat(hunk.added_lines).some((l) => dangle_close.test(l) || dangle_open.test(l)) ||
				hunk.added_lines.some(
					(l) => block_head_alone.test(l) || block_head_at_eol.test(l) || block_branch_alone.test(l),
				)
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

/**
 * The whitespace class `svelte_boundary_ws_trim`'s collapse-equality erases: FRAGMENT-EDGE
 * runs only, mirroring the compiler's `clean_nodes` (which deletes every fragment-edge run
 * at compile). Inter-sibling runs are deliberately NOT erased — Svelte collapses those to
 * one space, they never vanish, so a formatter deleting one changes the render and must
 * fail the equality rather than be claimed as the sanctioned trim.
 */
/**
 * Void elements have no content fragment, so a run after their tag is inter-sibling.
 *
 * The authority is the Rust list — `VOID_ELEMENTS` in `crates/tsv_html/src/elements.rs`
 * (mirroring Svelte's `VOID_ELEMENT_NAMES`), which the formatter itself classifies
 * against; this is a hand-copy of it and must track it. `command` and `keygen` are
 * obsolete in the HTML spec but ARE void there, so they belong here too: omitting one
 * would make the lookbehind treat a run after its tag as a content boundary and erase
 * it, which OVER-claims (the dangerous direction). `!doctype` is excluded — it is the
 * one case-insensitive member, and it opens no content fragment either way.
 */
const VOID_ELEMENTS =
	'area|base|br|col|command|embed|hr|img|input|keygen|link|meta|param|source|track|wbr';
/**
 * After a non-void, non-self-closed element/component open tag — content start. The tag
 * body tolerates `>` inside quoted attribute values (`title="a > b"`) and inside braced
 * expressions up to one nesting level (`onclick={() => (x = !x)}` — arrow handlers are
 * ubiquitous in Svelte). Deeper brace nesting or a `<` in an attr fails the lookbehind
 * and under-claims (file lands in partial/unknown → triage), never over-claims. The
 * trailing `(?<!/>)` excludes a self-closed tag, which has no content fragment.
 */
const boundary_after_open_tag = new RegExp(
	String.raw`(?<=<(?!(?:${VOID_ELEMENTS})\b)[A-Za-z][^<>"'{}]*(?:(?:"[^"]*"|'[^']*'|\{(?:[^{}]|\{[^{}]*\})*\})[^<>"'{}]*)*>)(?<!/>)[ \t\r\n]+`,
	'gi',
);
/** Before a closing tag — content end. */
const boundary_before_close_tag = /[ \t\r\n]+(?=<\/)/g;
/**
 * After a block open/branch tag `{#…}` / `{:…}` — branch fragment start. One brace-nesting
 * level is admitted for destructuring (`{#each xs as {a, b}}`); deeper nesting fails the
 * lookbehind and under-claims, never over-claims.
 */
const boundary_after_block_tag = /(?<=\{[#:](?:[^{}]|\{[^{}]*\})*\})[ \t\r\n]+/g;
/** Before a block close/branch tag `{/…}` / `{:…}` — branch fragment end. */
const boundary_before_block_tag = /[ \t\r\n]+(?=\{[:/])/g;
const erase_fragment_edges = (s: string): string =>
	s
		.replace(boundary_after_open_tag, '')
		.replace(boundary_before_close_tag, '')
		.replace(boundary_after_block_tag, '')
		.replace(boundary_before_block_tag, '');
/**
 * Erase fragment-edge runs OUTSIDE `<script>`/`<style>`; code regions pass through
 * verbatim.
 *
 * The trim is a TEMPLATE policy, so the collapse leaves code interiors alone and the
 * per-hunk arm refuses hunks overlapping them (see `CodeRegion`): their whitespace is
 * program/string bytes, and erasing tag-shaped runs inside code would let ours deleting
 * whitespace inside a STRING (`` `a <b> c` `` template literal, a CSS
 * `content: 'a <b> c'`) satisfy the equality — content loss SAFETY can't see, since it
 * counts no whitespace.
 *
 * `regions` defaults to a fresh scan so the function works on any string; callers that
 * already hold the cached regions for `s` (the whole-file arm, via
 * `enrich_detection_context`) pass them to skip the rescan.
 */
const collapse_fragment_edge_ws = (
	s: string,
	regions: CodeRegion[] = compute_code_regions(s),
): string => {
	let out = '';
	let last = 0;
	for (const r of regions) {
		out += erase_fragment_edges(s.slice(last, r.start)) + s.slice(r.start, r.end);
		last = r.end;
	}
	return out + erase_fragment_edges(s.slice(last));
};

const svelte_boundary_ws_trim: DivergencePattern = {
	id: 'svelte_boundary_ws_trim',
	description:
		'tsv trims render-free content-boundary whitespace (the Svelte-mirror trim: the compiler removes every fragment edge run at compile); prettier keeps a boundary space or expands the construct',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Inline content block-style', 'Svelte: Blocks'],
	fixtures: [
		'svelte/elements/inline_boundary_whitespace_prettier_divergence',
		'svelte/elements/inline_boundary_whitespace_misc_prettier_divergence',
		'svelte/elements/title_boundary_whitespace_prettier_divergence',
		'svelte/elements/inline_empty_long_prettier_divergence',
		'svelte/blocks/boundary_space_trim_prettier_divergence',
		'svelte/blocks/await/boundary_space_trim_prettier_divergence',
		'svelte/blocks/if/spaces_prettier_divergence',
		'svelte/blocks/if/last_block_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// No file-level whitespace-only gate: each claim below carries its own
		// content-preservation proof (the collapse equality over the exact text it claims),
		// so trim hunks are claimable even in a file whose OTHER hunks carry a non-ws
		// divergence (e.g. the self-closing expansion `self_closing_nonvoid` explains) —
		// those other hunks stay unclaimed by this pattern.

		// FAMILY SIGNATURE: the two sides are IDENTICAL once every FRAGMENT-EDGE whitespace
		// run — the exact class the trim deletes (see `collapse_fragment_edge_ws` above) —
		// is removed from both, and ours carries strictly LESS whitespace (the trim only
		// removes; a diff where ours adds whitespace is some other reflow and stays
		// unclaimed). Inter-sibling runs (after `</x>`'s `>`, around `{expr}`, next to
		// text, after a void/self-closed tag) are render-SIGNIFICANT — Svelte collapses
		// them to one space, they never vanish — so they survive VERBATIM on both sides:
		// ours deleting one fails the equality and the file surfaces as unknown/partial
		// instead of `known`. A run not touching a fragment edge (a text-fill rewrap)
		// likewise survives on both sides and fails it.
		//
		// Tried WHOLE-FILE first: when the entire ours/prettier difference is this class,
		// every hunk is claimed at once — necessary, not just convenient, because the diff
		// often splits a trimmed line's removed/added forms into SEPARATE hunks around a
		// shared glued context line (`<span> hi</span>` → `<span>hi</span>` where an
		// identical glued line sits between them), leaving per-hunk pairs asymmetric.
		// A mixed file falls back to the per-hunk pair check for the trim hunks alone.
		const count_ws = (s: string): number => (s.match(/[ \t\r\n]/g) ?? []).length;
		const ours_regions = ctx.ours_code_regions ?? [];
		const prettier_regions = ctx.prettier_code_regions ?? [];
		if (
			collapse_fragment_edge_ws(ctx.prettier, prettier_regions) ===
				collapse_fragment_edge_ws(ctx.ours, ours_regions) &&
			count_ws(ctx.ours) < count_ws(ctx.prettier)
		) {
			return {
				pattern: 'svelte_boundary_ws_trim',
				confidence: 'likely',
				hunk_indices: ctx.hunks.map((h) => h.index),
				reason:
					'render-free content-boundary whitespace trimmed (Svelte-mirror trim); prettier keeps the boundary space or expands the construct',
			};
		}
		// A hunk inside a <script>/<style> region can never be a template trim — its
		// whitespace is program/string bytes — so refuse it outright. Checked per side
		// against that side's own line ranges (the regions sit at different lines when
		// the diff shifts them).
		const claimed: number[] = [];
		for (const hunk of ctx.hunks) {
			if (
				overlaps_code_region(hunk.ours_range, ours_regions) ||
				overlaps_code_region(hunk.prettier_range, prettier_regions)
			) {
				continue;
			}
			const ours_join = hunk.added_lines.join('\n');
			const prettier_join = hunk.removed_lines.join('\n');
			if (
				prettier_join !== ours_join &&
				collapse_fragment_edge_ws(prettier_join) === collapse_fragment_edge_ws(ours_join) &&
				count_ws(ours_join) < count_ws(prettier_join)
			) {
				claimed.push(hunk.index);
			}
		}
		if (claimed.length === 0) return null;

		return {
			pattern: 'svelte_boundary_ws_trim',
			confidence: 'likely',
			hunk_indices: claimed,
			reason:
				'render-free content-boundary whitespace trimmed (Svelte-mirror trim); prettier keeps the boundary space or expands the construct',
		};
	},
};

// ─── Broad patterns (run last) ──────────────────────────────────────────────

const css_url_opaque: DivergencePattern = {
	id: 'css_url_opaque',
	description: 'Unquoted url() content kept verbatim; prettier reformats inside nested parens',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: Values'],
	fixtures: [
		'css/values/functions/url_nested_reformat_prettier_divergence',
	],
	detect(ctx) {
		// A nested `(...)` inside an unquoted `url(...)` — the only place url content
		// (opaque per css-syntax §4.3.6, never re-parsed) and prettier's value
		// reformatter interact. `[^)'"]*` excludes quotes so a quoted `url("…")` — a
		// string, not opaque url content — never matches.
		const nested_url = /\burl\(\s*[^)'"]*\([^)]*\)/i;
		// Collapse whitespace immediately inside a `url(` open and before a `)` close —
		// the padding the url-token tokenizer trims (§4.3.6). A pair that becomes EQUAL
		// after this differs only in that outer padding (tsv trims it, prettier keeps
		// it — a *distinct* divergence, e.g. prettier's `url/url.css`), so it is NOT the
		// interior reformat this pattern documents; only a pair that still differs after
		// the strip (a comma/space change *inside* the nested group) is claimed.
		const strip_outer = (l: string) => l.replace(/\burl\(\s+/gi, 'url(').replace(/\s+\)/g, ')');
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;
			const { removed_lines: removed, added_lines: added } = hunk;
			// Interior reformatting is a line-for-line rewrite: the line count never
			// changes (only whitespace *inside* the url token moves). Requiring equal
			// counts + a per-line whitespace-only match excludes value-list re-wraps
			// (a line-count change — those are css_value_wrap / fill_101_boundary),
			// so this never claims a hunk whose real divergence is the wrap.
			if (removed.length === 0 || removed.length !== added.length) return false;
			let saw_interior_reformat = false;
			for (let i = 0; i < removed.length; i++) {
				// content-preservation gate: a single non-whitespace difference on any
				// line disables the detector, so it can never mask a real content change.
				if (strip_all_ws(removed[i]) !== strip_all_ws(added[i])) return false;
				if (
					nested_url.test(removed[i]) && nested_url.test(added[i]) &&
					strip_outer(removed[i]) !== strip_outer(added[i])
				) {
					saw_interior_reformat = true;
				}
			}
			return saw_interior_reformat;
		});
		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_url_opaque',
				confidence: 'likely',
				hunk_indices,
				reason: 'unquoted url() content kept verbatim; prettier reformats inside the nested parens',
			};
		}
		return null;
	},
};

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
		'css/values/lists/comma_separated_greedy_fill_prettier_divergence',
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
			//
			// A line may carry SEVERAL line comments once prettier merges them
			// (`a // c1 // c2`), so each line contributes every comment text on it —
			// see `extract_line_comment_contents`. Without that split the merged side
			// reads as one comment named `c1 // c2`, overlapping nothing, and the
			// hunk goes unclaimed even though its single-comment sibling is claimed.
			const comment_texts = (line: string): string[] => {
				const line_comments = extract_line_comment_contents(line);
				return line_comments.length > 0 ? line_comments : [extract_comment_content(line)];
			};
			const added_texts = added_comment_lines.flatMap(comment_texts).sort();
			const removed_texts = removed_comment_lines.flatMap(comment_texts).sort();

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

const template_embedded_verbatim: DivergencePattern = {
	id: 'template_embedded_verbatim',
	description: 'Tagged/decorator template body kept verbatim; prettier reformats the embedded language',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript: Template Literals'],
	fixtures: [
		'typescript/expressions/literals/template/embedded_language_verbatim_prettier_divergence',
	],
	detect(ctx) {
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;

		// Prettier's `embeddedLanguageFormatting` reformats a tagged template whose tag it
		// recognizes as an embedded language (html/css/graphql/gql) — collapsing embedded
		// HTML whitespace, expanding embedded CSS onto its own lines. tsv keeps the body
		// verbatim. Prettier-side signal (this is a prettier-side-only divergence — tsv does
		// nothing): prettier reflowed a recognized-tag template, and ours kept a one-line
		// `tag`…`` form prettier did not reproduce.
		const embedded_tag_template = /\b(?:html|css|graphql|gql)`/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const prettier_reflowed = hunk.removed_lines.some((l) => embedded_tag_template.test(l));
			const ours_verbatim = hunk.added_lines.some(
				(l) => embedded_tag_template.test(l) && !hunk.removed_lines.includes(l),
			);
			return prettier_reflowed && ours_verbatim;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'template_embedded_verbatim',
				confidence: 'certain',
				hunk_indices,
				reason:
					'Tagged-template body kept verbatim (prettier reformats the embedded html/css/graphql language; tsv does no embedded formatting)',
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
	attr_value_single_quote,
	empty_statement_removal,
	css_value_ratio,

	// 2. CSS-specific patterns
	css_url_opaque,
	css_unit_serialize_case,
	css_atrule_spec_spacing,
	css_atrule_long_wrap,
	css_atrule_stable_quirk,
	css_scss_directive_number,
	css_selector_divergence,
	css_comment_stable_quirk,

	// Directive-driven suppression — the most specific signal there is (an explicit
	// author directive), so it precedes every layout heuristic.
	format_ignore_preserved,

	// 3. Feature-specific patterns
	template_literal_width,
	template_embedded_verbatim,
	block_expression_logical,
	single_specifier_import,
	member_expression_call,
	return_type_generic_union,
	non_null_paren_base,
	forced_continuation_indent,

	// 4. Svelte-specific patterns
	menu_block,
	inline_content_hug,
	inline_content_block_style,
	svelte_boundary_ws_trim,
	fill_after_inline,
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

/** Pattern lookup by id, for resolving a `DivergenceMatch` back to its declaring pattern. */
const pattern_by_id = new Map(PATTERNS.map((p) => [p.id, p]));

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
	// Pre-compute cached fields (line arrays, code regions)
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

	// Hunk-scoped SAFETY vouching. `all_explained` alone is too weak to excuse a
	// character-frequency differential: it is a set-cover over hunk indices, so a pattern
	// covering some unrelated hunk is as load-bearing as the one covering the hunk that
	// actually carried the flagged characters. Score each hunk on its own lines, then
	// require every char-risky one to be claimed by a pattern that has declared it can
	// legitimately change char counts.
	const vouching_hunks = new Set<number>();
	for (const match of matches) {
		const pattern = pattern_by_id.get(match.pattern);
		if (!pattern?.may_alter_char_frequency) continue;
		for (const idx of match.hunk_indices) vouching_hunks.add(idx);
	}
	const char_risky_hunks = hunks
		.filter((h) =>
			hunk_alters_semantic_chars(h.removed_lines.join('\n'), h.added_lines.join('\n')),
		)
		.map((h) => h.index);
	const safety_vouched =
		unexplained_hunks.length === 0 && char_risky_hunks.every((idx) => vouching_hunks.has(idx));

	return {
		hunks,
		matches,
		explained_hunks,
		unexplained_hunks,
		classification,
		safety_vouched,
		char_risky_hunks,
	};
}
