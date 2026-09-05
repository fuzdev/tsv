/**
 * Divergence pattern detection - identify known intentional differences from Prettier.
 *
 * Each pattern corresponds to a documented divergence in the `conformance_prettier*.md` family.
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
	/** Pattern ID (matches the `conformance_prettier*.md` family) */
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
	/** Section names from the `conformance_prettier*.md` family this pattern covers */
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

/** tsv's print width — the column every ours-side width test is asked against. */
const PRINT_WIDTH = 100;

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

/** Escape `text` for use verbatim inside a `RegExp` source — every metacharacter backslashed. */
const escape_regex = (text: string): string => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

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
 * Does OURS hold the print-width limit across everything it emitted in this hunk?
 *
 * The ours-side half of every "prettier overran the width, we broke" claim, and the
 * one that carries it: a wide PRETTIER line says only that the divergence is
 * width-shaped, so without this the pattern grades nobody's width and files a real
 * tsv over-width line as the sanctioned divergence. That laundering has no other
 * instrument — `width:audit` is the only gate that measures a column and it runs over
 * `tests/fixtures` alone, so real-code over-width is visible only here.
 *
 * A line COUNT is not a substitute: a hunk where tsv breaks more lines than prettier
 * and still leaves one at 105 satisfies "we re-wrapped" and fails this.
 *
 * Empty `added_lines` is the vacuity trap — `every` is true of nothing, so a
 * removal-only hunk (a wide prettier line we simply DELETED) would otherwise read as
 * a print-width rewrap. Zero lines cannot hold a limit, so it is asked explicitly.
 */
function ours_holds_print_width(hunk: HunkLines): boolean {
	return (
		hunk.added_lines.length > 0 && hunk.added_lines.every((l) => visual_width(l) <= PRINT_WIDTH)
	);
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
	options: { min_width?: number; line_predicate: (line: string) => boolean }
): boolean {
	const min_width = options.min_width ?? PRINT_WIDTH;
	const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
	const has_long_match = p_lines.some(
		(l) => options.line_predicate(l) && visual_width(l) > min_width
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
			line_end: line_at(end)
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
	regions: CodeRegion[]
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
 * The two line lists a whitespace-shape test reads. Named so the tests can run over a
 * NORMALIZED pair (see [`fold_dropped_closer`]) rather than only over a literal hunk.
 */
type HunkLines = Pick<DiffHunk, 'added_lines' | 'removed_lines'>;

/**
 * A hunk re-rooted past a leading line that is the construct's own HEAD, or `null` when it
 * does not open with one.
 *
 * A continuation hunk normally starts at the continuation, with the head sitting above it in
 * our file. It starts one line EARLIER when the head line itself changed — which it does at
 * an unprefixed `{…}`, where tsv puts a space between the delimiter and a trailing comment
 * (`{ // c`) that prettier welds (`{// c`). That is still a whitespace-only difference, so
 * the head is handed back for the depth measurement and the rest is measured as before.
 *
 * Whitespace-only is checked across the WHOLE line, not just its indent: the difference here
 * is interior.
 */
function split_leading_head(hunk: HunkLines): { head: string; rest: HunkLines } | null {
	const { added_lines: add, removed_lines: rem } = hunk;
	if (add.length < 2 || rem.length < 2) return null;
	const bare = (line: string): string => line.replace(/\s+/g, '');
	if (bare(add[0]) !== bare(rem[0])) return null;
	return {
		head: add[0],
		rest: { added_lines: add.slice(1), removed_lines: rem.slice(1) }
	};
}

/**
 * The same hunk with a construct's CLOSER folded back onto the line above it, or `null`
 * when the hunk is not that shape.
 *
 * tsv drops a braced head's closer to its own line whenever something indents the head's
 * content, so a comment-forced continuation there shows up as ONE more added line than
 * removed (`\texpr}` → `\texpr` + `}`) and the pure-re-indent tests reject it — even though
 * the divergence is still whitespace-only, and still the one §Uniform Forced-Continuation
 * Indent describes ("the `}` column moves with the indent and is the same question").
 * Folding restores the shape those tests were written for.
 *
 * The fold is deliberately narrow: exactly one extra line, made of closers alone. That keeps
 * the safety argument the caller rests on — the folded pair still differs only in
 * whitespace, so claiming the hunk cannot mask a content change.
 */
function fold_dropped_closer(hunk: DiffHunk): HunkLines | null {
	const add = hunk.added_lines;
	if (add.length !== hunk.removed_lines.length + 1 || add.length < 2) return null;
	const closer = add[add.length - 1].trim();
	if (!/^[)\]}]+$/.test(closer)) return null;
	const folded = add.slice(0, -1);
	folded[folded.length - 1] += closer;
	return { added_lines: folded, removed_lines: hunk.removed_lines };
}

/**
 * Whether a hunk is a pure re-indent: ours and prettier carry the same lines in the
 * same order, each differing only by leading whitespace (so no token can be lost),
 * with at least one line's indentation actually changing. Indentation-only by
 * construction, so claiming such a hunk can never mask a content change.
 */
function is_pure_reindent(hunk: HunkLines): boolean {
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
function indents_one_level_below(hunk: HunkLines, head: string): boolean {
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
	return (
		output.includes(`// ${text}`) || output.includes(`/* ${text}`) || output.includes(` * ${text}`)
	);
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
	range: { start: number; end: number } | null
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
	text: string
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
		'svelte/syntax/format_ignore/range_prettier_divergence'
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
			reason:
				'construct preserved verbatim under a `format-ignore` directive prettier does not honor'
		};
	}
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
		'typescript/syntax/whitespace/bom_prettier_divergence'
	],
	detect(ctx) {
		// Use the `\uFEFF` escape rather than a literal BOM glyph in source — a raw
		// BOM byte in this file is an editing hazard (invisible, easily mangled).
		const BOM = '\uFEFF';
		// Source starts with BOM, our output doesn't
		if (ctx.source.startsWith(BOM) && !ctx.ours.startsWith(BOM)) {
			// Verify prettier keeps BOM
			if (ctx.prettier.startsWith(BOM)) {
				// Find the hunk covering the BOM rather than assuming it is hunk 0:
				// the hunk whose prettier (removed) range starts at source line 0, or
				// failing that whose removed line still carries the BOM.
				const bom_hunk = ctx.hunks.find(
					(h) => h.prettier_range?.start === 0 || h.removed_lines.some((l) => l.startsWith(BOM))
				);
				const hunk_indices = bom_hunk ? [bom_hunk.index] : [];
				return {
					pattern: 'bom_strip',
					confidence: 'certain',
					hunk_indices,
					reason: 'BOM (byte order mark) removed'
				};
			}
		}
		return null;
	}
};

const self_closing_nonvoid: DivergencePattern = {
	id: 'self_closing_nonvoid',
	description: 'Non-void HTML element self-closing normalization',
	languages: ['svelte'],
	conformance_sections: ['Svelte/HTML'],
	fixtures: [
		'svelte/elements/self_closing_nonvoid_prettier_divergence',
		'svelte/elements/ws_sensitive_self_closing_kinds_prettier_divergence'
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
			for (const [self_lines, close_lines] of [
				[hunk.added_lines, hunk.removed_lines],
				[hunk.removed_lines, hunk.added_lines]
			]) {
				for (const line of self_lines) {
					const re = /<([a-zA-Z][\w.-]*)[^>]*\/>/g;
					let m;
					while ((m = re.exec(line)) !== null) {
						const tag_name = escape_regex(m[1]);
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
			)
				return true;
			if (
				hunk.added_lines.some((l) => self_closing_end.test(l)) &&
				hunk.removed_lines.some((l) => explicit_close_end.test(l))
			)
				return true;
			// Orphaned remove-only: prettier has self-closing non-void HTML that we removed
			if (
				hunk.added_lines.length === 0 &&
				hunk.removed_lines.every((l) => self_closing_nonvoid_tag.test(l))
			)
				return true;
			// Orphaned add-only: we added empty explicit-close HTML that prettier didn't have
			if (
				hunk.removed_lines.length === 0 &&
				hunk.added_lines.every((l) => empty_explicit_close.test(l))
			)
				return true;
			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'self_closing_nonvoid',
				confidence: 'likely',
				hunk_indices,
				reason: 'Non-void HTML element self-closing normalization'
			};
		}
		return null;
	}
};

const attr_value_single_quote: DivergencePattern = {
	id: 'attr_value_single_quote',
	description: 'Attribute / style: / this= value with a literal " kept single-quoted',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Attributes'],
	fixtures: [
		'svelte/attributes/value_double_quote_prettier_divergence',
		'svelte/directives/style/value_double_quote_prettier_divergence',
		'svelte/special_elements/svelte_element_this_double_quote_prettier_divergence'
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
				const name = escape_regex(m[1]);
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
				reason: 'Value with a literal " kept single-quoted (prettier corrupts to double quotes)'
			};
		}
		return null;
	}
};

const svelte_element_this_string: DivergencePattern = {
	id: 'svelte_element_this_string',
	description:
		'<svelte:element this={…}> string literal single-quoted (prettier hardcodes double quotes)',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Elements'],
	fixtures: ['svelte/special_elements/svelte_element_this_string_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// prettier-plugin-svelte prints a brace-wrapped string literal in `this={…}` as a
		// hardcoded `"${value}"`, ignoring `singleQuote`; tsv hands the literal to the normal
		// string printer, so it comes out single-quoted like every other JS string. The
		// fingerprint is the pair: OUR line carries `this={'value'}` and a PRETTIER line in the
		// same hunk carries `this={"value"}` with the SAME value — nothing else spells a
		// single-quoted literal directly inside `this={…}`, and requiring the double-quoted
		// twin keeps a genuine content change (a different string) unclaimed. The plain
		// `this="value"` attribute form is not matched: prettier reaches it only by collapsing a
		// paren- or comment-prefixed literal, a structural rewrite that drops the comment, and
		// that case stays honestly unclaimed rather than vouched by a quote-swap detector.
		const ours_single = /\bthis=\{'([^'\\]*)'\}/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			for (const line of hunk.added_lines) {
				const m = ours_single.exec(line);
				if (!m) continue;
				const value = escape_regex(m[1]);
				const prettier_double = new RegExp(`\\bthis=\\{"${value}"\\}`);
				if (hunk.removed_lines.some((l) => prettier_double.test(l))) return true;
			}
			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'svelte_element_this_string',
				confidence: 'certain',
				hunk_indices,
				reason:
					'<svelte:element this={…}> string literal single-quoted (prettier ignores singleQuote here)'
			};
		}
		return null;
	}
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
				reason: 'Standalone empty statement (;) removed'
			};
		}
		return null;
	}
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
				reason: 'Ratio spacing normalized in CSS'
			};
		}
		return null;
	}
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
					(added) => ours_unit.test(added) && added.toLowerCase() === lowered
				);
			});
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_unit_serialize_case',
				confidence: 'certain',
				hunk_indices,
				reason: 'CSS Hz/kHz/Q serialized lowercase per spec (CSS Values 4 §6.2/§7.3)'
			};
		}
		return null;
	}
};

const css_atrule_spec_spacing: DivergencePattern = {
	id: 'css_atrule_spec_spacing',
	description: 'CSS at-rule keyword spacing normalized per spec',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: At-Rules'],
	fixtures: [
		'css/at_rules/container_spacing_prettier_divergence',
		'css/at_rules/media_boolean_spacing_prettier_divergence'
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

			return (
				(removed_missing_space && added_has_space) ||
				(removed_has_atrule && added_has_atrule && removed_missing_space)
			);
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_atrule_spec_spacing',
				confidence: 'certain',
				hunk_indices,
				reason: 'CSS at-rule keyword spacing normalized per spec (CSS Syntax 3 §4.3.4)'
			};
		}
		return null;
	}
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
		'css/at_rules/supports_long_prettier_divergence'
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const prettier_lines = ctx.prettier_lines!;
		const atrule_pattern = /@(?:container|media|import|supports)/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;
			// Prettier has a long at-rule line (> 100 chars) that ours wrapped.
			return long_line_rewrapped(hunk, prettier_lines, {
				line_predicate: (l) => atrule_pattern.test(l)
			});
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_atrule_long_wrap',
				confidence: 'likely',
				hunk_indices,
				reason: 'CSS at-rule wraps at print width'
			};
		}
		return null;
	}
};

const css_atrule_stable_quirk: DivergencePattern = {
	id: 'css_atrule_stable_quirk',
	description: 'CSS at-rule stable quirk (Prettier preserves multiple forms)',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: At-Rules'],
	fixtures: [
		'css/at_rules/scope_complex_prettier_divergence',
		'css/at_rules/scope_selector_prettier_divergence'
	],
	detect(ctx) {
		if (ctx.language !== 'css' && ctx.language !== 'svelte') return null;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;

			const removed_joined = hunk.removed_lines.join('\n');
			const added_joined = hunk.added_lines.join('\n');

			// @layer with spacing quirks (extra spaces after commas)
			if (/@layer/.test(removed_joined) || /@layer/.test(added_joined)) {
				const removed_extra_spaces = hunk.removed_lines.some(
					(l) => /@layer/.test(l) && /,\s{2,}/.test(l)
				);
				const added_normalized = hunk.added_lines.some(
					(l) => /@layer/.test(l) && /, [^\s]/.test(l)
				);
				if (removed_extra_spaces && added_normalized) return true;
			}

			// @scope with spacing quirks (spaces inside parens, double spaces around
			// to, or a comma/combinator the author wrote tight)
			if (/@scope/.test(removed_joined) || /@scope/.test(added_joined)) {
				// Prettier adds spaces inside scope parens: ( .class ) vs (.class)
				const removed_has_quirk = hunk.removed_lines.some(
					(l) =>
						/@scope/.test(l) &&
						(/\( /.test(l) ||
							/ \)/.test(l) ||
							/\s{2,}to\s{2,}/.test(l) ||
							// tight comma / combinator preserved: (.x,.y), (a>b), (.x)to(.y)
							/,\S/.test(l) ||
							/\S[>+~]\S/.test(l) ||
							/\)to\(/.test(l))
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
				reason: 'CSS at-rule stable quirk (Prettier preserves multiple forms, we normalize)'
			};
		}
		return null;
	}
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
				reason: 'SCSS-directive at-rule prelude preserved verbatim; prettier number-normalizes'
			};
		}
		return null;
	}
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
		'css/selectors/pseudo_class/nested_where_is_long_prettier_divergence'
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
					removed_nth_content.length > 0 &&
					added_nth_content.length > 0 &&
					removed_nth_content.some(
						(l, i) =>
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
				reason: 'CSS selector formatting divergence'
			};
		}
		return null;
	}
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
		'css/tokens/comments/selector_list_prettier_divergence'
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
				reason: 'CSS comment position stable quirk (we normalize)'
			};
		}
		return null;
	}
};

// ─── Feature-specific patterns ──────────────────────────────────────────────

const template_literal_width: DivergencePattern = {
	id: 'template_literal_width',
	description: 'Template literal interpolation breaks to respect print width',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript: Template Literals'],
	fixtures: [
		'typescript/expressions/literals/template/interpolation_nested_template_prettier_divergence',
		'typescript/types/template_literal_type_long_prettier_divergence'
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
				(l) => break_after_dollar_brace.test(l) || closing_brace_backtick.test(l)
			);
			const removed_has_break = hunk.removed_lines.some(
				(l) => break_after_dollar_brace.test(l) || closing_brace_backtick.test(l)
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
					line_predicate: (l) => nested_template_interpolation.test(l)
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
				reason: 'Template interpolation breaks to respect print width'
			};
		}
		return null;
	}
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
			return (
				hunk.added_lines.some((l) => block_operator_break.test(l)) &&
				!hunk.removed_lines.some((l) => block_operator_break.test(l))
			);
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'block_expression_logical',
				confidence: 'likely',
				hunk_indices,
				reason: 'Logical expression in block condition broken to respect print width'
			};
		}
		return null;
	}
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
				reason: 'Member expression in call args breaks differently'
			};
		}
		return null;
	}
};

const member_chain_hug_convergence: DivergencePattern = {
	id: 'member_chain_hug_convergence',
	description:
		'Member-chain wide-last-argument hug printed in one pass (prettier reaches the same form on its second)',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: [
		'typescript/expressions/calls/chained/last_arg_hug_convergence_long_prettier_divergence'
	],
	detect(ctx) {
		// Prettier's single pass breaks the chain (its non-idempotent intermediate);
		// tsv prints prettier's own settled second pass: the chain flat with the last
		// call's object-rooted argument hugging. The match is a byte-precise
		// pure-reflow proof, so a real bug cannot ride it:
		//   1. prettier's side opens with a chain head plus >=1 member lines
		//      (trimmed lines starting with `.` / `?.`), the last opening the
		//      argument;
		//   2. ours' first line is EXACTLY those lines' trims joined (the flat
		//      chain — member joins are space-free);
		//   3. every remaining line is identical except prettier's carries exactly
		//      one extra tab (the broken chain's indent level).
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const removed = hunk.removed_lines;
			const added = hunk.added_lines;
			if (removed.length < 3 || added.length < 2) return false;
			let k = 1;
			while (k < removed.length && /^[?.]/.test(removed[k].trimStart())) k++;
			if (k < 2 || k >= removed.length) return false;
			const joined = removed
				.slice(0, k)
				.map((l) => l.trim())
				.join('');
			if (added[0].trim() !== joined) return false;
			const rest_removed = removed.slice(k);
			const rest_added = added.slice(1);
			if (rest_removed.length !== rest_added.length) return false;
			return rest_removed.every((l, i) => l === '\t' + rest_added[i]);
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'member_chain_hug_convergence',
				confidence: 'certain',
				hunk_indices,
				reason: 'Member chain collapsed to the hug prettier itself converges to on a second pass'
			};
		}
		return null;
	}
};

const return_type_generic_union: DivergencePattern = {
	id: 'return_type_generic_union',
	description: 'Return type generic with union wraps at print width',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: ['typescript/declarations/function/return_type_generic_union_long_prettier_divergence'],
	detect(ctx) {
		const prettier_lines = ctx.prettier_lines!;

		// Look for generic types with union (| null, | void, | undefined) in hunks
		// where prettier's line exceeds 100 chars and ours re-wrapped it.
		const union_in_generic = /[<>].*\|\s*(?:null|void|undefined)/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) =>
			long_line_rewrapped(hunk, prettier_lines, {
				line_predicate: (l) => union_in_generic.test(l)
			})
		);

		if (hunk_indices.length > 0) {
			return {
				pattern: 'return_type_generic_union',
				confidence: 'likely',
				hunk_indices,
				reason: 'Return type generic with union wraps at print width'
			};
		}
		return null;
	}
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
				hunk.added_lines.some((l) => ours_hangs.test(l))
		);

		if (hunk_indices.length > 0) {
			return {
				pattern: 'non_null_paren_base',
				confidence: 'likely',
				hunk_indices,
				reason:
					'Non-null assertion on a parenthesized base: tsv hangs the outer parens, prettier hugs the inner call'
			};
		}
		return null;
	}
};

// ─── Svelte-specific patterns ───────────────────────────────────────────────

/**
 * The doc's own enumeration is the element set: prettier-plugin-svelte's blockElements
 * list carries `ol`/`ul` and `details`/`li` but omits `menu` and `summary`, the two
 * elements the HTML spec gives identical UA display (conformance_prettier_svelte.md
 * §Svelte: Elements). tsv classifies both as block, so their content — and their
 * FOLLOWING sibling, which as an inline element would hug the closing tag in the
 * surrounding fill — lays out on its own line where prettier keeps the inline form.
 */
const spec_block_close = /<\/(?:menu|summary)/;
const spec_block_close_own_line = /^\s*<\/(?:menu|summary)>/;
const spec_block_open = /<(?:menu|summary)[\s>]/;
const spec_block_hugs_close = />[^<\n]*<\/(?:menu|summary)/;

const spec_block_elements: DivergencePattern = {
	id: 'spec_block_elements',
	description:
		'<menu>/<summary> treated as block elements (spec-compliant; prettier-plugin-svelte omits both from its blockElements list)',
	languages: ['svelte'],
	conformance_sections: ['Svelte/HTML'],
	fixtures: [
		'svelte/elements/menu_block_prettier_divergence',
		'svelte/elements/summary_block_prettier_divergence'
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// Look for hunks involving those elements where prettier hugs content
		// (inline formatting) and we expand it (block formatting)
		const ours_lines = ctx.ours_lines!;
		const prettier_lines = ctx.prettier_lines!;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Check for a close tag in removed lines (prettier hugs: content</menu on the
			// same line, with > possibly on next line — or a sibling glued after
			// </summary> on the same line)
			const removed_has_close = hunk.removed_lines.some((l) => spec_block_close.test(l));
			// Check for a close tag on added lines on its own line (we expand: block formatting)
			const added_has_close = hunk.added_lines.some((l) => spec_block_close_own_line.test(l));

			if (removed_has_close || added_has_close) return true;

			// Also check context: an open tag in surrounding lines
			const o_lines = ours_lines_in_hunk(ours_lines, hunk);
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			const context_lines = hunk.lines.filter((l) => l.type === 'same').map((l) => l.line);
			const all_lines = [...o_lines, ...p_lines, ...context_lines];
			const has_element = all_lines.some((l) => spec_block_open.test(l));

			if (!has_element) return false;

			// Prettier hugs: >{content} on same line as attribute
			const removed_hugs = hunk.removed_lines.some((l) => spec_block_hugs_close.test(l));
			// We expand: > on own line
			const added_breaks_gt = hunk.added_lines.some((l) => /^\t*>$/.test(l));

			return removed_hugs || added_breaks_gt;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'spec_block_elements',
				confidence: 'certain',
				hunk_indices,
				reason: '<menu>/<summary> treated as block element (prettier treats as inline)'
			};
		}
		return null;
	}
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
			const prettier_breaks =
				hunk.removed_lines.some((l) => /^\s*>(?!\/)/.test(l)) || />\s*$/.test(removed_joined);

			return ours_hugs && prettier_breaks;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'inline_content_hug',
				confidence: 'likely',
				hunk_indices,
				reason: 'Expression breaks internally vs bracket breaks'
			};
		}
		return null;
	}
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
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) =>
			long_line_rewrapped(hunk, prettier_lines, {
				line_predicate: (l) => inline_close_tag.test(l)
			})
		);

		if (hunk_indices.length > 0) {
			return {
				pattern: 'fill_after_inline',
				confidence: 'likely',
				hunk_indices,
				reason: 'Text after inline element breaks at print width'
			};
		}
		return null;
	}
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
		'svelte/attributes/attach_spread_paren_multiline_comment_prettier_divergence'
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
			s
				.replace(/\/\*[\s\S]*?\*\//g, '')
				.replace(/\/\/[^\n]*/g, '')
				.replace(/\s+/g, '');
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
				reason: 'We preserve a comment inside {…}/a tag that Prettier drops'
			};
		}
		return null;
	}
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
		'svelte/blocks/if/inline_element_long_prettier_divergence'
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
				(l) => block_expr_pattern.test(l) && visual_width(l) > 100 && visual_width(l) <= 110
			);
			if (!has_short_overflow_block) return false;
			return hunk.added_lines.length > hunk.removed_lines.length;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'short_expr_100',
				confidence: 'likely',
				hunk_indices,
				reason: 'Short expression in block condition exceeds 100 chars, we break'
			};
		}
		return null;
	}
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
function forced_continuation_site(
	prev_ours: string,
	first_added: string,
	language: string
): string | null {
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

	// Svelte braced heads — the head→value gap of every `{…}`, via the shared
	// `leading_line_comment_hangs_value`. Three spellings, each of which the TS
	// printer cannot produce, which is what keeps this clause from claiming an
	// ordinary object/block indent bug (the defect class it most resembles):
	//
	//  - a prefixed tag or block head (`{@html // c`, `{...// c`, `{#if // c`) —
	//    unambiguous markup;
	//  - an attribute or directive value (`data-attr={ // c`, `on:click={ // c`) — the
	//    space-less `={` is markup only, since a TS assignment prints ` = {`;
	//  - the bare `{expr}` tag as the whole line (`{ // c`), which is markup only in a
	//    SVELTE file: a line-leading `{` in TypeScript is a block statement, and one
	//    carrying a trailing `//` prints the same way, so this clause is language-gated
	//    rather than resting on the brace alone. (It used to rest on the weld — tsv never
	//    glued a `//` to an opening brace — but tsv now separates the two at an unprefixed
	//    braced head, so the weld is no longer the discriminator.)
	if (
		/\{(?:@(?:html|render|debug|attach)\b|\.\.\.|#\w+\b|:else if\b)[ \t]*\/\//.test(prev_ours) ||
		/=\{[ \t]*\/\//.test(prev_ours) ||
		(language === 'svelte' && /^[ \t]*\{[ \t]*\/\//.test(prev_ours))
	) {
		return 'Svelte braced head';
	}

	// No clause for an OWN-LINE comment leading the continuation: where the author
	// wrote the comment on its own line, prettier relocates the comment itself, so
	// the hunk carries that relocation and is not a pure re-indent at all —
	// `comment_position` claims it. A clause here would have no witness.
	return null;
}

const forced_continuation_indent: DivergencePattern = {
	id: 'forced_continuation_indent',
	description:
		'tsv indents a comment-forced continuation one level (annotation, declaration/module header, prefix type operator, key→`:` gap, Svelte braced head); prettier keeps it flush',
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
		// The Svelte-braced-head clause's witnesses, one per spelling: the family
		// sweep covers the prefixed tags and the `={` attribute values, the `on:`
		// sample covers a directive value whose expression self-expands, and the
		// expression-tag pair covers the bare `{//` line.
		'svelte/syntax/comments/expr_leading_line_prettier_divergence',
		'svelte/directives/on/line_comment_prettier_divergence',
		'svelte/syntax/comments/expression_tag_line_comment_prettier_divergence'
	],
	detect(ctx) {
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;
		const ours_lines = ctx.ours_lines!;

		// Which sites fired, for the reason line — a file's continuations can come from
		// more than one gap, and naming them is what makes `--explain` actionable.
		const sites = new Set<string>();
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Indentation-only by construction, so claiming the hunk can never mask a
			// content change — the whole basis for this detector being safe to widen. A
			// Svelte braced head also drops its CLOSER to its own line when the content
			// indents, which is one extra added line and still whitespace-only, so the
			// shape tests run over the folded pair ([`fold_dropped_closer`]).
			const folded = is_pure_reindent(hunk) ? hunk : fold_dropped_closer(hunk);
			if (folded === null) return false;
			// Normally the head sits above the hunk; it sits INSIDE it when the head line
			// itself changed ([`split_leading_head`], the unprefixed `{ // c` separator).
			// Both arms end in the same pair — a head line and the continuation under it —
			// but only the first needs a line above the hunk at all, so the
			// nothing-above-us guard belongs on that arm rather than ahead of the choice
			// (an unprefixed tag at line 0 is exactly the case it used to reject).
			const split = is_pure_reindent(folded) ? null : split_leading_head(folded);
			const lines = split?.rest ?? folded;
			let head: string;
			if (split !== null) {
				head = split.head;
			} else {
				const start = hunk.ours_range?.start;
				if (start == null || start === 0) return false;
				head = ours_lines[start - 1] ?? '';
			}
			if (!is_pure_reindent(lines)) return false;
			// "one level" below the head, as the rule states it: any other depth is a
			// different layout difference and stays unclaimed.
			if (!indents_one_level_below(lines, head)) return false;
			const site = forced_continuation_site(head, lines.added_lines[0], ctx.language);
			if (site === null) return false;
			sites.add(site);
			return true;
		});

		if (hunk_indices.length === 0) return null;
		return {
			pattern: 'forced_continuation_indent',
			confidence: 'likely',
			hunk_indices,
			reason: `comment-forced continuation indents one level where prettier keeps it flush (${[...sites].join(', ')})`
		};
	}
};

const inline_sibling_newline_flow: DivergencePattern = {
	id: 'inline_sibling_newline_flow',
	description:
		'tsv flows an inline sibling isolated by authored newlines back onto the content line (Svelte 5 collapses the inter-sibling run to one space, so the SPELLING of the separator carries no signal); prettier keeps the authored newline',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Inline content block-style'],
	fixtures: [
		'svelte/elements/inline_sibling_newline_flow_prettier_divergence',
		'svelte/expressions/angle_escaped_prettier_divergence'
	],
	// NOT a char-frequency alterer: the rule respells a separator (newline -> space) and
	// never adds or drops a semantic character. Left at the default `false` so it can
	// explain a hunk for bucketing but can never vouch for a SAFETY differential.
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		const ours_regions = ctx.ours_code_regions ?? [];
		const prettier_regions = ctx.prettier_code_regions ?? [];

		// A blank line is an authored separator the flow rule never crosses — it is one of
		// the rule's documented exclusions — so a hunk carrying one on either side is not
		// this pattern's business. Declining outright is also what keeps the weld below
		// honest: dropping blanks before welding would let an EATEN blank line satisfy the
		// equality (`A⏎⏎B` and `A B` both weld to `A B`), absorbing a real authoring-intent
		// change into a layout claim.
		const has_blank_line = (lines: string[]): boolean => lines.some((l) => l.trim().length === 0);

		// The normalized form both checks below read. Safe to compare directly once the
		// blank-line guard above has run: every line carries content.
		const trimmed_lines = (lines: string[]): string[] => lines.map((l) => l.trim());

		// FAMILY SIGNATURE: at least one welded seam must sit at a sibling boundary — the
		// left line ends a tag/expression (`>` or `}`) or the right line opens one (`<` or
		// `{`). A pure prose rewrap has neither and belongs to the fill patterns.
		const opens_tag = /^[<{]/;
		const has_sibling_seam = (trimmed: string[]): boolean =>
			trimmed.some(
				(left, i) =>
					i + 1 < trimmed.length &&
					(left.endsWith('>') || left.endsWith('}') || opens_tag.test(trimmed[i + 1]))
			);

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// A hunk inside <script>/<style> is program/string bytes, never a template
			// separator — refuse it outright. Checked per side against that side's own
			// line ranges, since the diff shifts them independently.
			if (
				overlaps_code_region(hunk.ours_range, ours_regions) ||
				overlaps_code_region(hunk.prettier_range, prettier_regions)
			) {
				return false;
			}
			if (hunk.added_lines.length === 0 || hunk.removed_lines.length === 0) return false;
			// DIRECTIONAL: flowing collapses lines — or, when the flowed run is over-width,
			// re-breaks at a LATER boundary (the equal-count arm below). This pattern never
			// claims a hunk where ours is the side that BROKE more.
			if (hunk.added_lines.length > hunk.removed_lines.length) return false;
			if (has_blank_line(hunk.removed_lines) || has_blank_line(hunk.added_lines)) return false;

			const prettier_trimmed = trimmed_lines(hunk.removed_lines);
			if (!has_sibling_seam(prettier_trimmed)) return false;

			const ours_trimmed = trimmed_lines(hunk.added_lines);
			const prettier_weld = prettier_trimmed.join(' ');
			const ours_weld = ours_trimmed.join(' ');

			// RE-PACK COMPOSITION (equal line counts): the flow rule respells the authored
			// newline as the space it renders as, and the fill then re-breaks the
			// over-width run at a later boundary — the line count doesn't drop, the break
			// MOVES. Claimable only in the pack direction, keyed on content: ours' first
			// line strictly EXTENDS prettier's (the break moved later), every ours line
			// holds the hard print-width limit, and the strict weld equality below proves
			// preservation. The mirror shape — ours breaking EARLIER than prettier though
			// the packed form fits — is a formatter defect (the deep inline-container
			// early break pins it) and fails the extension key, staying unclaimed.
			if (hunk.added_lines.length === hunk.removed_lines.length) {
				return (
					ours_trimmed[0].startsWith(prettier_trimmed[0] + ' ') &&
					ours_holds_print_width(hunk) &&
					prettier_weld === ours_weld
				);
			}

			// CONTENT-PRESERVATION PROOF: welding each side's trimmed lines with a single
			// space yields the same string. The rule's whole effect is respelling a line
			// boundary as the space it already renders as, so both spellings weld to one
			// form; anything else fails. A dropped or added character fails it, and so does
			// a change in intra-line spacing (the weld normalizes only at line boundaries,
			// never inside a line) — which is what keeps this strictly narrower than a
			// `strip_all_ws` equality. Crucially it also fails on the GLUE direction
			// (prettier `A⏎B` vs ours `AB`, no separator at all): that welds to `A B`
			// against `AB`, so a formatter that ate an inter-sibling space is never
			// absorbed here.
			if (prettier_weld === ours_weld) return true;

			// COMPOSED WITH THE FRAGMENT-EDGE TRIM. One hunk can carry this respelling AND
			// the Svelte-mirror boundary trim at once (`<a> x </a>⏎and y` → `<a>x</a> and
			// y`): the weld above fails on the trimmed run, and the trim rule's own
			// equality fails on the collapsed line, so the hunk falls through BOTH and the
			// file surfaces as `unknown` though every byte of it is sanctioned. Re-run the
			// weld through the trim's own normalizer (`collapse_fragment_edge_ws`, shared
			// with `svelte_boundary_ws_trim` below) so the composition is claimable without
			// either rule widening the class it claims alone.
			//
			// Still a proof, not an escape hatch: the normalizer erases only FRAGMENT-EDGE
			// runs — the exact class the compiler deletes — so an eaten INTER-SIBLING space
			// survives on both sides and still fails the equality, and the `count_ws` guard
			// mirrors the trim's direction (it only ever removes), so ours ADDING boundary
			// whitespace is never claimed here.
			return (
				count_ws(ours_weld) < count_ws(prettier_weld) &&
				collapse_fragment_edge_ws(prettier_weld) === collapse_fragment_edge_ws(ours_weld)
			);
		});

		if (hunk_indices.length === 0) return null;
		return {
			pattern: 'inline_sibling_newline_flow',
			confidence: 'likely',
			hunk_indices,
			reason:
				'inline sibling flowed back onto the content line (the authored newline respelled as the space it renders as); prettier keeps the newline'
		};
	}
};

const inline_content_block_style: DivergencePattern = {
	id: 'inline_content_block_style',
	description:
		'tsv lays out inline element/block content block-style (tags intact, content on its own line), breaks before an inline element whose text-line overflows so its opening tag starts a fresh line, and lays a whitespace-collapsing container (table/select/…) out block-style; prettier dangles the tag delimiters / hugs the content / dangles the opening tag / keeps the container inline',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Inline content block-style', 'Svelte: Blocks'],
	fixtures: [
		'svelte/tags/declaration_own_line_prettier_divergence',
		'svelte/blocks/snippet/own_line_prettier_divergence',
		'svelte/blocks/snippet/inline_element_long_prettier_divergence',
		'svelte/elements/inline_sibling_gt_dangle_prettier_divergence',
		'svelte/elements/block_body_drop_nested_siblings_prettier_divergence',
		'svelte/elements/block_multiline_attrs_content_hug_prettier_divergence',
		'svelte/elements/inline_if_sibling_fill_long_prettier_divergence',
		'svelte/elements/inline_content_hug_long_prettier_divergence',
		'svelte/elements/inline_break_before_wrap_long_prettier_divergence',
		'svelte/elements/inline_break_before_component_long_prettier_divergence',
		'svelte/elements/inline_break_before_void_long_prettier_divergence',
		'svelte/elements/ws_collapsing_containers_prettier_divergence',
		'svelte/elements/implicit_close_table_prettier_divergence'
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;

		// SAFETY GATE — a claim is admissible only where the ours/prettier text carries the
		// same non-whitespace characters in the same order, so "pure-layout reflow" is
		// proven rather than plausible and a real content change (a dropped comment, a
		// normalized quote) can never be absorbed. Held at two scopes, because the scope
		// decides how far one relocation may be claimed to reach:
		//   - WHOLE FILE — nothing is hidden in ANY hunk, so the reflow may claim the whole
		//     diff (see the claim below: its relocated content and dangled delimiters land
		//     in separate, sometimes non-adjacent hunks).
		//   - otherwise — some OTHER pattern's char-changing divergence coexists in the file
		//     (`self_closing_nonvoid`'s `<i />` → `<i></i>` beside this reflow, the
		//     `prettier/tests/format/html/tags/tags.html` shape). Disabling the detector
		//     wholesale there left this pattern's own hunks unexplained and collapsed
		//     `safety_vouched` on a file where every hunk was separately sanctioned — a
		//     gated SAFETY violation with no formatter change. So fall back to the same
		//     proof applied PER HUNK: strictly local, and for the hunk it claims strictly
		//     stronger than the whole-file form.
		const file_ws_only = strip_all_ws(ctx.ours) === strip_all_ws(ctx.prettier);
		const hunk_ws_only = (hunk: DiffHunk): boolean =>
			strip_all_ws(hunk.removed_lines.join('\n')) === strip_all_ws(hunk.added_lines.join('\n'));

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
		const block_branch_alone =
			/^[ \t]*\{[:/](?:else|then|catch|if|each|await|key|snippet)\b[^}]*\}[ \t]*$/;
		// A DECLARATION on its own our-line (§"A DECLARATION TAKES ITS OWN LINE"): a
		// `{@const}` / `{const …}` / `{let …}` tag, or a one-lined `{#snippet …}…{/snippet}`,
		// alone on an added line — produced only by tsv giving the declaration its own line
		// where prettier welded it to the content beside it (`docs{#snippet icon()}…`). The
		// multi-line snippet form is already covered by `block_head_alone` above; this is the
		// short body that stays on the head's line. Ours-side only.
		const declaration_alone =
			/^[ \t]*(?:\{@const\b.*\}|\{(?:const|let)\b.*\}|\{#snippet\b.*\{\/snippet\})[ \t]*$/;
		// The whitespace-collapsing-container block-style (§"reaches inter-sibling whitespace …
		// a whitespace-collapsing container"): inside `<table>`/`<tbody>`/`<thead>`/`<tfoot>`/
		// `<tr>`/`<colgroup>`/`<select>`/`<datalist>` the compiler removes inter-sibling
		// whitespace entirely, so tsv lays the container out block-style — each child on its own
		// line — where prettier keeps it inline on one line. The marker is one of those container
		// open/close tags sitting ALONE on an OUR line: prettier keeps content after the open tag
		// (`<select><option>…`), so the tag is never alone on its side. Content-keyed on the exact
		// `can_remove_entirely` name set — an ordinary parent's inter-sibling space is
		// render-significant and never block-styled this way, so it carries no such marker. This
		// is a subset of the whole-diff the whole-file `strip_all_ws` SAFETY gate above already
		// proved whitespace-only.
		const container_tag_alone =
			/^[ \t]*<\/?(?:table|tbody|thead|tfoot|tr|colgroup|select|datalist)\b[^<>]*>[ \t]*$/;
		// The comment-first arm: an inline element whose content OPENS with an HTML comment
		// (`<span><!--⏎-->…`). Block-style gives the content its own line, comment included —
		// the comment's `<!--` opens OUR line where prettier welds it to the open tag, and the
		// closing `-->` stands alone on our line where prettier welds it to the close tag.
		// Both markers are one-sided by construction: prettier never starts a line with the
		// delimiter it just welded, and a comment that opens its own line in BOTH outputs is
		// not on a changed line at all. Read as a PAIR across the hunk's two sides so a
		// same-shaped whitespace-only hunk with no weld on prettier's side stays unclaimed.
		const comment_open_alone = /^[ \t]*<!--/; //                         ours: `<!--` opens the line
		const welded_open_comment = /<[A-Za-z][\w.:-]*(?:[ \t][^<>]*)?><!--/; // prettier: `<tag><!--`
		const comment_close_alone = /^[ \t]*-->[ \t]*$/; //                  ours: `-->` alone
		const welded_close_comment = /-->[ \t]*<\/[A-Za-z][\w.:-]*>/; //     prettier: `--></tag>`
		const comment_first_signature = (hunk: DiffHunk): boolean =>
			(hunk.added_lines.some((l) => comment_open_alone.test(l)) &&
				hunk.removed_lines.some((l) => welded_open_comment.test(l))) ||
			(hunk.added_lines.some((l) => comment_close_alone.test(l)) &&
				hunk.removed_lines.some((l) => welded_close_comment.test(l)));
		// The break-before posture (§"The rule reaches the element's own *position*"): an
		// inline element preceded by same-line text that must wrap starts a FRESH line in
		// ours, where prettier dangles the OPENING tag on the overflowing text line. The
		// mirror of `dangle_close`, on the PRETTIER (removed) side — a `<tag` that begins
		// after same-line text + a space (the "dangle after a space" the rule forbids),
		// at EOL. Two prettier forms, both absent from ours (which broke before the tag):
		//   (a) the COMPLETE open tag trails the text and only the content/close dangle
		//       (`…using <MdnLink path="…">` at EOL — the collapse/block-style sub-shape); or
		//   (b) the open tag's ATTRIBUTES wrap, so the bare tag NAME trails the text
		//       (`…with the <TomeLink` at EOL) and `>`/`/>` lands on its own line.
		// The leading `\S[ \t]` (text + one space before `<`) is what keeps this off the
		// rejected broad "open-tag at line start" markers: an element that legitimately
		// begins its own line is indent-only before `<` and never matches. The element may
		// carry a glued word PREFIX (`= var(<StyleVariableButton …>` — the word and its
		// element are one welded unit, §Svelte: Inline content block-style; the unit
		// travels whole in ours, and prettier opens it mid-line), so `[^<>{}\s]*` admits
		// word bytes between the boundary space and the `<` — never tag/brace/space
		// structure, so the anchoring space stays the unit's own leading boundary. This is
		// the same weld `spaced_tag_travel`'s prefix scan-back walks for `{expr}` tags
		// (its stop set `[ \t<>{}]` is this class's complement) — keep the two in step.
		// `[A-Za-z]` after `<` excludes a closing `</tag`; the tag-name class admits
		// `:`/`.` /`-` (`<svelte:element`, `<Foo.Bar`, custom elements) like the dangle
		// markers above.
		const dangle_open_tag_after_text = /\S[ \t][^<>{}\s]*<[A-Za-z][\w.:-]*(?:[ \t][^<>]*>)?[ \t]*$/;
		const has_signature = (hunk: DiffHunk): boolean =>
			hunk.removed_lines
				.concat(hunk.added_lines)
				.some((l) => dangle_close.test(l) || dangle_open.test(l)) ||
			hunk.removed_lines.some((l) => dangle_open_tag_after_text.test(l)) ||
			comment_first_signature(hunk) ||
			hunk.added_lines.some(
				(l) =>
					block_head_alone.test(l) ||
					block_head_at_eol.test(l) ||
					block_branch_alone.test(l) ||
					container_tag_alone.test(l) ||
					declaration_alone.test(l)
			);
		const signature_hunks = ctx.hunks.filter(has_signature);
		if (signature_hunks.length === 0) return null;

		// The reflow is one content-preserving block-style relocation spanning the
		// whole diff (the relocated content and the dangled delimiters land in
		// separate, sometimes non-adjacent hunks), so claim every hunk. Only the
		// whole-file proof establishes that span, though — under the per-hunk fallback
		// a hunk is claimable only on its own evidence: it must carry the signature AND
		// prove preservation on its own lines. An unrelated whitespace divergence
		// (verbatim `format-ignore`, `{}` vs `{ }`) therefore still stays unclaimed for
		// its own detector rather than being absorbed here.
		const hunk_indices = file_ws_only
			? ctx.hunks.map((h) => h.index)
			: signature_hunks.filter(hunk_ws_only).map((h) => h.index);
		if (hunk_indices.length === 0) return null;

		return {
			pattern: 'inline_content_block_style',
			confidence: 'likely',
			hunk_indices,
			reason:
				'inline/block content laid out block-style (tags intact, content on its own line), or an inline element broken before onto a fresh line; prettier dangles the tag delimiters / dangles the opening tag'
		};
	}
};

/**
 * Index of the first `{` on `line` that no later `}` on the same line closes, or -1.
 *
 * Unmatched CLOSERS at the head (a continuation line inside a broken expression, `: ''}`)
 * are ignored — only an open the line itself leaves dangling counts as a mid-line-opened
 * or line-start-opened tag.
 */
const unmatched_open_brace = (line: string): number => {
	const open_stack: number[] = [];
	for (let i = 0; i < line.length; i++) {
		const c = line[i];
		if (c === '{') open_stack.push(i);
		else if (c === '}' && open_stack.length > 0) open_stack.pop();
	}
	return open_stack.length > 0 ? open_stack[0] : -1;
};

/**
 * Weld one diff side's lines into the flat spelling of its content. The join is
 * expression-aware: a break after `(`/`[` or before `)`/`]` welds to nothing (the flat form
 * of a broken call — `f(⏎'x'⏎)` is `f('x')`), every other line boundary to the one space it
 * stands for. Strictly narrower than a strip-all-ws equality: intra-line spacing is
 * preserved verbatim, so a dropped or added character — or an eaten space inside a line —
 * still fails the comparison.
 */
const weld_expression_lines = (lines: string[]): string => {
	let out = '';
	for (const l of lines) {
		const t = l.trim();
		if (out.length === 0) {
			out = t;
			continue;
		}
		const glue =
			out.endsWith('(') || out.endsWith('[') || t.startsWith(')') || t.startsWith(']') ? '' : ' ';
		out += glue + t;
	}
	return out;
};

/**
 * Whether the tag opening at `lines[at][brace]` is FORCED to break internally: its flat
 * spelling (continuation lines welded back through the balancing `}`) cannot fit within
 * print width even starting from the line's own indent — the best case any travel could
 * buy it. That is the only sanctioned reason for tsv to leave a tag's `{` open at end of
 * line (the travel rule's break-internally arm, and the hard-width second-tag context —
 * see `fill_expr_travel_middle_before_long_prettier_divergence`); a tag torn open despite
 * fitting flat is a formatter defect the caller must surface, never absorb. A tag that
 * never balances inside the bounded window answers false (under-claims).
 */
const tag_break_is_forced = (lines: string[], at: number, brace: number): boolean => {
	const first = lines[at];
	let flat = '';
	let depth = 0;
	const limit = Math.min(lines.length, at + 40);
	for (let k = at; k < limit; k++) {
		const seg = k === at ? first.slice(brace) : lines[k].trim();
		if (k > at) {
			flat +=
				flat.endsWith('(') || flat.endsWith('[') || seg.startsWith(')') || seg.startsWith(']')
					? ''
					: ' ';
		}
		for (const c of seg) {
			flat += c;
			if (c === '{') depth++;
			else if (c === '}') {
				depth--;
				if (depth === 0) {
					const indent = first.slice(0, first.length - first.trimStart().length);
					return visual_width(indent) + visual_width(flat) > 100;
				}
			}
		}
	}
	return false;
};

const spaced_tag_travel: DivergencePattern = {
	id: 'spaced_tag_travel',
	description:
		'tsv travels a spaced {expr} tag whose expression cannot fit flat to a fresh line (collapsing flat there when it fits, breaking internally when not); prettier stops measuring at the first internal break, keeps the tag on the text line and opens it mid-line',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Elements'],
	fixtures: [
		'svelte/elements/fill_spaced_tag_travel_long_prettier_divergence',
		'svelte/elements/fill_expr_travel_continuation_long_prettier_divergence',
		'svelte/elements/fill_expr_travel_middle_long_prettier_divergence',
		'svelte/elements/fill_expr_travel_middle_before_long_prettier_divergence'
	],
	// NOT a char-frequency alterer: the rule respells line boundaries (newline ↔ space, plus
	// the no-space joins at an expression's paren breaks) and never adds or drops a semantic
	// character. Left at the default `false` so it can explain a hunk for bucketing but can
	// never vouch for a SAFETY differential.
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;
		const ours_lines = ctx.ours_lines!;
		const ours_regions = ctx.ours_code_regions ?? [];
		const prettier_regions = ctx.prettier_code_regions ?? [];

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// Template markup only — a brace inside <script>/<style> is program bytes.
			if (
				overlaps_code_region(hunk.ours_range, ours_regions) ||
				overlaps_code_region(hunk.prettier_range, prettier_regions)
			) {
				return false;
			}
			if (hunk.added_lines.length === 0 || hunk.removed_lines.length === 0) return false;
			// A blank line is an authored separator this rule never touches.
			if (
				hunk.added_lines.some((l) => l.trim().length === 0) ||
				hunk.removed_lines.some((l) => l.trim().length === 0)
			) {
				return false;
			}

			// FAMILY SIGNATURE, prettier side: a tag OPENED mid-line after text and a SPACE —
			// an unmatched `{` with content before it and whitespace in front of the welded
			// unit's HEAD. The tag may carry a glued word prefix (`opcodes ({expr…` — the
			// word `(` and its tag are the smallest welded unit, §Print Width Philosophy),
			// so the boundary asked about is the whitespace before the PREFIX, reached by
			// scanning back over word bytes — never across tag/brace/space structure. The
			// spacing is load-bearing twice over: a line-START unit (j === 0) is the
			// traveled form (ours' own shape, which prettier only ever keeps, never
			// produces), and a unit glued straight onto the previous WORD with no boundary
			// at all (`token{thread.token_count !==` — prettier's fill packing a welded
			// word+tag pair past the width) belongs to the glued-pair rule, not this one,
			// so neither may claim here. (That glued-pair shape has no whitespace between
			// the word and its OWN start — the scan-back reaches line start or indent —
			// which is exactly what the j-checks refuse.)
			const prettier_midline = hunk.removed_lines.some((l) => {
				const i = unmatched_open_brace(l);
				if (i <= 0) return false;
				let j = i;
				while (j > 0 && !/[ \t<>{}]/.test(l[j - 1])) j--;
				if (j === 0) return false;
				const before = l[j - 1];
				return (before === ' ' || before === '\t') && l.slice(0, j).trim().length > 0;
			});
			if (!prettier_midline) return false;

			// OURS side: the traveled layout never overruns print width…
			if (!ours_holds_print_width(hunk)) return false;

			// …and never tears open a tag that could have rendered flat. An unmatched `{` on
			// an our-line is legitimate only where the expression is FORCED to break — wider
			// than print width even from a fresh line (`tag_break_is_forced`). A tag torn open
			// despite fitting is a formatter defect (zzz's DiskfileMetrics/ThreadListitem pin
			// that shape) and must surface as partial/unknown, never be absorbed here.
			let ours_has_unmatched = false; // among ADDED lines — the traveled side's own layout
			if (hunk.ours_range) {
				const in_range = ours_lines_in_hunk(ours_lines, hunk);
				for (let k = 0; k < in_range.length; k++) {
					const i = unmatched_open_brace(in_range[k]);
					if (i < 0) continue;
					if (hunk.added_lines.includes(in_range[k])) ours_has_unmatched = true;
					if (!tag_break_is_forced(ours_lines, hunk.ours_range.start + k, i)) return false;
				}
			}

			// WIDTH MOTIVE: travel exists only past print width. When the traveled tag broke
			// internally, the forced check above already proved the overflow (the tag alone
			// exceeds print width from its indent). Otherwise — travel-and-collapse-flat —
			// the hunk's own flat spelling must overflow from the traveled side's indent
			// (every committed travel fixture crosses 100 by construction). A short
			// respelling is some other reflow and stays unclaimed.
			if (!ours_has_unmatched) {
				const first_added = hunk.added_lines[0];
				const added_indent = first_added.slice(
					0,
					first_added.length - first_added.trimStart().length
				);
				if (
					visual_width(added_indent) + visual_width(weld_expression_lines(hunk.added_lines)) <=
					100
				) {
					return false;
				}
			}

			// CONTENT-PRESERVATION PROOF: both sides weld to one flat spelling.
			return weld_expression_lines(hunk.removed_lines) === weld_expression_lines(hunk.added_lines);
		});

		if (hunk_indices.length === 0) return null;
		return {
			pattern: 'spaced_tag_travel',
			confidence: 'likely',
			hunk_indices,
			reason:
				'spaced wide expression tag traveled to a fresh line (collapsing flat or breaking internally there); prettier keeps it on the text line and opens it mid-line'
		};
	}
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
	'gi'
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
/**
 * Whitespace character count — the DIRECTION guard for every trim claim: the
 * Svelte-mirror trim only ever removes, so a side that ADDED whitespace is some other
 * reflow. Shared with `inline_sibling_newline_flow`'s composed arm above.
 */
const count_ws = (s: string): number => (s.match(/[ \t\r\n]/g) ?? []).length;
const collapse_fragment_edge_ws = (
	s: string,
	regions: CodeRegion[] = compute_code_regions(s)
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
		'svelte/blocks/if/last_block_prettier_divergence'
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
					'render-free content-boundary whitespace trimmed (Svelte-mirror trim); prettier keeps the boundary space or expands the construct'
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
				'render-free content-boundary whitespace trimmed (Svelte-mirror trim); prettier keeps the boundary space or expands the construct'
		};
	}
};

// ─── Broad patterns (run last) ──────────────────────────────────────────────

const css_url_opaque: DivergencePattern = {
	id: 'css_url_opaque',
	description: 'Unquoted url() content kept verbatim; prettier reformats inside nested parens',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: Values'],
	fixtures: ['css/values/functions/url_nested_reformat_prettier_divergence'],
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
					nested_url.test(removed[i]) &&
					nested_url.test(added[i]) &&
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
				reason: 'unquoted url() content kept verbatim; prettier reformats inside the nested parens'
			};
		}
		return null;
	}
};

const css_value_wrap: DivergencePattern = {
	id: 'css_value_wrap',
	description: 'CSS property value wraps at print width',
	languages: ['css', 'svelte'],
	conformance_sections: ['CSS: Values'],
	fixtures: [
		'css/values/functions/transform_long_prettier_divergence',
		'css/values/lists/space_separated_long_wrap_prettier_divergence'
	],
	detect(ctx) {
		const prettier_lines = ctx.prettier_lines!;

		// Check each hunk for long CSS property values in prettier's range
		// AND verify we actually wrapped (more lines than prettier)
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			if (!is_in_css_context(hunk, ctx)) return false;
			return long_line_rewrapped(hunk, prettier_lines, {
				line_predicate: (l) => /^\t+[\w-]+:\s*.+/.test(l)
			});
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'css_value_wrap',
				confidence: 'likely',
				hunk_indices,
				reason: 'CSS property value wraps at print width'
			};
		}
		return null;
	}
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
		'svelte/elements/fill_expr_travel_boundary_long_prettier_divergence',
		'svelte/elements/fill_after_inline_prettier_divergence',
		'svelte/elements/fill_multiple_expr_long_prettier_divergence',
		'svelte/elements/block_multiline_attrs_content_hug_prettier_divergence',
		'svelte/attributes/multiline_value_inline_long_prettier_divergence'
	],
	detect(ctx) {
		const prettier_lines = ctx.prettier_lines!;
		let longest_prettier_overflow = 0;

		// A hunk matches when prettier ran to (or past) print width here and every line
		// ours emitted holds it. One walk of the prettier lines answers both the gate and
		// the reason string: the widest line IS the widest overflow once it clears the limit.
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const p_lines = prettier_lines_in_hunk(prettier_lines, hunk);
			const widest_prettier = p_lines.reduce((w, l) => Math.max(w, visual_width(l)), 0);
			// >= PRINT_WIDTH: includes lines at exactly print width, since the divergence
			// is that prettier fills right up to the limit while we break earlier.
			if (widest_prettier < PRINT_WIDTH) return false;
			if (!ours_holds_print_width(hunk)) return false;

			longest_prettier_overflow = Math.max(longest_prettier_overflow, widest_prettier);
			return true;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'fill_101_boundary',
				confidence: 'likely',
				hunk_indices,
				reason: `Prettier allows ${longest_prettier_overflow} chars, we break at print width`
			};
		}
		return null;
	}
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
		'typescript/expressions/sequence/operand_edge_comment_prettier_divergence'
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
					)
						return false;
					// Relocation evidence: both neighbors of the comment differ AND
					// neither neighbor merely BEGINS THE SAME ELEMENT as its counterpart
					// (which would be a stable comment bordering a width re-wrap of the
					// element it precedes, not a relocation). A genuine relocation lands
					// the comment among entirely different tokens on both sides.
					const o = comment_line_neighbors(ours_lines, text);
					const p = comment_line_neighbors(prettier_lines, text);
					return (
						o !== null &&
						p !== null &&
						o.prev !== p.prev &&
						o.next !== p.next &&
						!lines_begin_same_element(o.prev, p.prev) &&
						!lines_begin_same_element(o.next, p.next)
					);
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
			const lines_differ =
				added_comment_lines.length !== removed_comment_lines.length ||
				added_comment_lines.some((l, i) => l !== removed_comment_lines[i]);
			if (!lines_differ) return false;

			// Non-comment content must be similar — strip comments from both sides
			// and compare the trimmed non-empty lines. If the code itself changed
			// significantly, this is a formatting bug, not a comment position divergence.
			const strip_comments = (line: string) =>
				line
					.replace(/\/\/.*$/, '')
					.replace(/\/\*.*?\*\//g, '')
					.trim();
			const added_code = hunk.added_lines
				.map(strip_comments)
				.filter((l) => l.length > 0)
				.sort();
			const removed_code = hunk.removed_lines
				.map(strip_comments)
				.filter((l) => l.length > 0)
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
			const removed_code_unsorted = hunk.removed_lines
				.map(strip_comments)
				.filter((l) => l.length > 0);
			const normalize = (lines: string[]) => lines.join('').replace(/\s+/g, '');
			const normalized_added = normalize(added_code_unsorted);
			const normalized_removed = normalize(removed_code_unsorted);
			if (normalized_added.length <= 100 && normalized_added === normalized_removed) {
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
				lines
					.map(strip_comments)
					.join('')
					.replace(/[|&\s]/g, '');
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
				reason: 'Comment preserved where user placed it (Prettier relocates)'
			};
		}
		return null;
	}
};

const instantiation_parens: DivergencePattern = {
	id: 'instantiation_parens',
	description: 'Parens preserved in ternary/binary instantiation expressions',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: ['typescript/typescript_specific/assertions/instantiation_parens_prettier_divergence'],
	detect(ctx) {
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;

		// Ours preserves: (x ? y : z)<T> or (a + b)<T> — has )<
		// Prettier strips:  x ? y : z<T>  or  a + b<T>  — no )<
		const paren_before_type_args = /\)<[a-zA-Z]/;

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			const ours_has_parens = hunk.added_lines.some((l) => paren_before_type_args.test(l));
			const prettier_missing = hunk.removed_lines.some(
				(l) => !paren_before_type_args.test(l) && /[?+\-]\s.*<[a-zA-Z]/.test(l)
			);
			return ours_has_parens && prettier_missing;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'instantiation_parens',
				confidence: 'certain',
				hunk_indices,
				reason:
					'Parens preserved around ternary/binary in instantiation expression (changes semantics)'
			};
		}
		return null;
	}
};

const single_type_param_comma: DivergencePattern = {
	id: 'single_type_param_comma',
	description:
		'Single unconstrained arrow type param stays bare `<T>`; prettier-in-Svelte forces `<T,>`',
	languages: ['svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: [
		'typescript/expressions/arrow/generic/single_type_param_prettier_divergence',
		'typescript/typescript_specific/generics/const_type_param_arrow_prettier_divergence'
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
					'Single unconstrained arrow type param stays bare `<T>` (tsv emits no JSX); prettier-in-Svelte forces `<T,>`'
			};
		}
		return null;
	}
};

const block_comment_computed_member: DivergencePattern = {
	id: 'block_comment_computed_member',
	description: 'Block comment preserved inside computed member brackets',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript: Comments'],
	fixtures: ['typescript/syntax/comments/block_comment_computed_member_long_prettier_divergence'],
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
					'Block comment preserved inside computed member brackets (Prettier hoists, changing association)'
			};
		}
		return null;
	}
};

const block_comment_chain: DivergencePattern = {
	id: 'block_comment_chain',
	description: 'Block comment spacing in member chain normalization',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript: Comments'],
	fixtures: ['typescript/expressions/calls/chained/block_comment_chain_prettier_divergence'],
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
					'Block comment spacing in member chain differs (normalization-only, both reach same stable output)'
			};
		}
		return null;
	}
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
				reason: 'JSDoc type cast parens preserved (required for the cast; prettier-TS strips)'
			};
		}
		return null;
	}
};

const template_embedded_verbatim: DivergencePattern = {
	id: 'template_embedded_verbatim',
	description:
		'Tagged/decorator template body kept verbatim; prettier reformats the embedded language',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript: Template Literals'],
	fixtures: [
		'typescript/expressions/literals/template/embedded_language_verbatim_prettier_divergence'
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
				(l) => embedded_tag_template.test(l) && !hunk.removed_lines.includes(l)
			);
			return prettier_reflowed && ours_verbatim;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'template_embedded_verbatim',
				confidence: 'certain',
				hunk_indices,
				reason:
					'Tagged-template body kept verbatim (prettier reformats the embedded html/css/graphql language; tsv does no embedded formatting)'
			};
		}
		return null;
	}
};

const field_key_unquote: DivergencePattern = {
	id: 'field_key_unquote',
	description: 'Class field key unquoted; prettier keeps a valid-identifier field key quoted',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: ['typescript/declarations/class/field_key_unquote_prettier_divergence'],
	// Unquoting only drops the surrounding `'`, a char SAFETY already excludes, so this
	// pattern never moves the semantic char count — `may_alter_char_frequency` stays false.
	detect(ctx) {
		if (ctx.language !== 'typescript' && ctx.language !== 'svelte') return null;

		// tsv unquotes a valid-identifier CLASS FIELD key (`'x' = 1` → `x = 1`, `'count': T`
		// → `count: T`, `static 'total'` → `static total`); prettier unquotes class
		// method/accessor keys but keeps FIELD keys quoted. Object / type-literal / interface
		// keys agree (both unquote → no hunk), so a prettier-quoted → ours-unquoted key hunk is
		// only ever this class-field case.
		//
		// The quoted key is anchored at member-line start (after leading whitespace + optional
		// field modifiers), so a quoted *value* (`b = 'b'`, `const a = 'x'`) and the
		// reverse-direction enum case (prettier unquotes an enum key, tsv keeps it quoted —
		// its `'b'` is never line-initial) are excluded. ASCII-ident only: the astral ES2015
		// key (`'𐊧'`) is the separate `property_key_es2015_ident` divergence; numeric /
		// non-ident keys (`'0a'`, `'x-y'`, `'0'`) stay quoted in both. The quoted key is
		// followed by a field terminator — `=` (init), `:` (annotation), `?` (optional), or a
		// bare `;` / end (field declaration).
		const field_modifiers =
			'(?:(?:static|readonly|public|private|protected|declare|abstract|accessor|override)\\s+)*';
		const quoted_field_key = new RegExp(
			`^\\s*${field_modifiers}'([A-Za-z_$][A-Za-z0-9_$]*)'\\s*(?:[=:?;]|$)`
		);

		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			for (const p_line of hunk.removed_lines) {
				const m = quoted_field_key.exec(p_line);
				if (!m) continue;
				const ident = m[1];
				// The same field key, unquoted, must appear line-initial on an ours line (the
				// quoted form gone) — proving tsv dropped the quotes rather than some unrelated
				// edit removing the line.
				const bare = new RegExp(`^\\s*${field_modifiers}${ident}\\s*(?:[=:?;]|$)`);
				const unquoted_on_ours = hunk.added_lines.some(
					(o) => bare.test(o) && !o.includes(`'${ident}'`)
				);
				if (unquoted_on_ours) return true;
			}
			return false;
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'field_key_unquote',
				confidence: 'certain',
				hunk_indices,
				reason:
					'Class field key unquoted (tsv unquotes a valid-identifier class field key; prettier keeps it quoted)'
			};
		}
		return null;
	}
};

// ─── Pattern Registry ───────────────────────────────────────────────────────
//
// Ordered: specific → broad. Specific patterns run first for best explanations.
// Multiple patterns CAN claim the same hunk (by design).

/**
 * Retained parenthesized union member with an interior line comment: prettier
 * explodes the inner union one member per line (`| a` / `| b // c`), tsv applies
 * its union-fit layout and keeps it inline (`a | b // c`). The line comment forces
 * the parens open in both, so the only difference is the inner union's inline vs
 * exploded layout. Catalogued as **Retained paren union member line comment** in
 * conformance_prettier_ts_comments.md; the same shape is 18389's remaining unexplained hunk.
 *
 * Content-preservation proof: strip prettier's leading `| ` from each exploded
 * member and rejoin with ` | ` — a pure inline↔exploded reflow is byte-identical to
 * ours' inline line, so a dropped or added member (real content loss) breaks the
 * equality and is never claimed. `may_alter_char_frequency` stays false (fail-closed):
 * the reflow only toggles the leading pipes prettier adds, which the SAFETY
 * differential already treats as prettier-excess rather than ours-loss.
 */
const union_paren_member_inline: DivergencePattern = {
	id: 'union_paren_member_inline',
	description:
		'Interior line comment in a retained parenthesized union member: prettier explodes the inner union one-per-line, tsv keeps it inline (union-fit)',
	languages: ['typescript', 'svelte'],
	conformance_sections: ['TypeScript'],
	fixtures: ['typescript/types/union_intersection_retained_paren_line_comment_prettier_divergence'],
	detect(ctx) {
		const hunk_indices = find_matching_hunks(ctx.hunks, (hunk) => {
			// prettier (removed) explodes: >= 2 leading-pipe union members, nothing else.
			const removed = hunk.removed_lines.map((l) => l.trim());
			if (removed.length < 2) return false;
			if (!removed.every((l) => l.startsWith('| '))) return false;
			// ours (added) collapses to a single inline line.
			const added = hunk.added_lines.map((l) => l.trim());
			if (added.length !== 1) return false;
			// A pure inline<->exploded reflow: strip each exploded member's `| ` and
			// rejoin with ` | `; equality proves no member was dropped or added.
			const prettier_inline = removed.map((l) => l.slice(2)).join(' | ');
			return prettier_inline === added[0];
		});

		if (hunk_indices.length > 0) {
			return {
				pattern: 'union_paren_member_inline',
				confidence: 'likely',
				hunk_indices,
				reason:
					'tsv keeps a parenthesized union member inline (union-fit); prettier explodes it one member per line'
			};
		}
		return null;
	}
};

// `(?<![\w-])` and not `\b`: a hyphen is a word boundary, so `\blang=` claims
// `data-lang="pug"` and would let this pattern claim a hunk that is a real bug.
//
// Compiled once rather than per tag, unlike the other built-per-call regexes in this file:
// `foreign_body_line_regions` reaches this per LINE of both sides of every divergent Svelte
// file, where those ask once per file or per hunk.
const LANG_ATTR = /(?<![\w-])lang\s*=\s*(?:"([^"]*)"|'([^']*)')/;
const TYPE_ATTR = /(?<![\w-])type\s*=\s*(?:"([^"]*)"|'([^']*)')/;

/**
 * The narrowed `lang`/`type` value an opening tag names, mirroring
 * `internal::lang_attribute`'s reader rules over raw tag text: `lang` outranks `type`
 * whatever their order, the value is trimmed and `text/`-stripped, and an empty value
 * names nothing (falls through to `type`). Entity-spelled values are not decoded here —
 * a corpus heuristic; such a tag goes unclaimed and its file stays `unknown`.
 */
function tag_lang_value(tag: string): string | null {
	const pick = (attr: RegExp): string | null => {
		const m = attr.exec(tag);
		if (!m) return null;
		const narrowed = (m[1] ?? m[2] ?? '').trim().replace(/^text\//, '');
		return narrowed === '' ? null : narrowed;
	};
	return pick(LANG_ATTR) ?? pick(TYPE_ATTR);
}

/** A frozen-body region as line indices into one side's lines, tag lines included. */
interface ForeignBodyRegion {
	start: number;
	end: number;
}

/**
 * Mirrors `internal::EmbeddedLang::formattable_langs`, per tag. A hand copy: this is a TS
 * harness and cannot call the Rust predicate.
 *
 * ⚠️ The two drift directions are NOT symmetric, so keep this list in step in one direction
 * above all. **Wider than Rust's** (a name here that Rust freezes) is the safe half: no
 * region, the hunk goes unclaimed, and the file lands in `unknown`, which is gated. **Narrower
 * than Rust's** is the one that hides a bug: this file calls a body frozen that tsv has
 * started FORMATTING, the region is claimed, and a real printer bug inside it is filed as a
 * known divergence. That is the direction a future change takes — the day tsv gains an scss
 * parser, `scss` moves into the `style` set here, and a stale copy would blind the detector
 * to the new printer exactly when it is likeliest to be wrong.
 */
const FORMATTABLE_EMBEDDED_LANGS: Record<string, readonly string[]> = {
	script: [
		'ts',
		'typescript',
		'js',
		'javascript',
		'ecmascript',
		'application/javascript',
		'application/ecmascript',
		'module'
	],
	style: ['css'],
	template: ['html']
};

/** The closing tag each frozen-body region scans forward for. */
const EMBEDDED_CLOSING_TAG: Record<string, RegExp> = {
	script: /<\/script\s*>/,
	style: /<\/style\s*>/,
	template: /<\/template\s*>/
};

/**
 * The opening tag each frozen-body region starts at — module-level for the same reason as
 * `LANG_ATTR`: it is asked once per LINE.
 */
const EMBEDDED_OPENING_TAG = /<(script|style|template)\b([^>]*)>/;

/**
 * The line regions of frozen foreign-language bodies: each single-line opening
 * `<script|style|template …>` whose `lang`/`type` names a language tsv does not format at
 * that position (style: `css`; script: `ts`; template: `html`), through its closing tag.
 *
 * Every unrecognized shape FAILS OPEN — no region, so the hunk goes unclaimed and its file
 * lands in `unknown`, which is gated. A tag whose attributes wrap across lines is one such
 * shape; **an opening tag with no closing tag after it is the other**, and it is the one that
 * bites, because the natural spelling of the scan fails CLOSED: run `end` to the last line
 * and the region swallows the rest of the file, so every later hunk is claimed as this
 * pattern at `certain` confidence and a real divergence is filed as known. That is not a
 * hypothetical shape — this regex matches an opening tag wherever it appears, including
 * inside a comment (`<!-- example: <style lang="less"> -->`), inside an attribute value, and
 * on a self-closing `<template lang="pug" />`, none of which have a closing tag and all of
 * which parse. So an unterminated open is dropped rather than clamped.
 */
function foreign_body_line_regions(lines: string[]): ForeignBodyRegion[] {
	const regions: ForeignBodyRegion[] = [];
	for (let i = 0; i < lines.length; i++) {
		const open = EMBEDDED_OPENING_TAG.exec(lines[i]);
		if (!open) continue;
		const kind = open[1];
		const lang = tag_lang_value(open[2]);
		if (lang === null || FORMATTABLE_EMBEDDED_LANGS[kind].includes(lang)) continue;
		const close = EMBEDDED_CLOSING_TAG[kind];
		let end = i;
		while (end < lines.length && !close.test(lines[end].slice(end === i ? open.index : 0))) {
			end++;
		}
		if (end === lines.length) continue;
		regions.push({ start: i, end });
		i = end;
	}
	return regions;
}

const foreign_body_freeze: DivergencePattern = {
	id: 'foreign_body_freeze',
	description:
		'tsv freezes an embedded body whose lang/type names a language it does not format at that position (style: only css; script: the JS/TS family; template: only html); prettier formats scss/less with real parsers, unknown script langs via babel-ts, json/importmap scripts via its JSON parser, and unknown-lang templates as markup',
	languages: ['svelte'],
	conformance_sections: ['Svelte: Foreign-language embedded bodies'],
	fixtures: [
		'svelte/script/foreign_lang_frozen_prettier_divergence',
		'svelte/style/foreign_lang_frozen_prettier_divergence',
		'svelte/elements/style_foreign_lang_nested_prettier_divergence',
		'svelte/elements/template_foreign_lang_unknown_prettier_divergence',
		'svelte/elements/template_lang_trim_prettier_divergence'
	],
	detect(ctx) {
		if (ctx.language !== 'svelte') return null;
		const ours_lines = ctx.ours_lines ?? ctx.ours.split('\n');
		const prettier_lines = ctx.prettier_lines ?? ctx.prettier.split('\n');
		const ours_regions = foreign_body_line_regions(ours_lines);
		const prettier_regions = foreign_body_line_regions(prettier_lines);
		if (ours_regions.length === 0 && prettier_regions.length === 0) return null;

		const inside = (
			range: ForeignBodyRegion | null,
			regions: ForeignBodyRegion[]
		): boolean | null => {
			if (!range) return null;
			return regions.some((r) => range.start >= r.start && range.end <= r.end);
		};
		// A hunk is claimed only when every side it has lines on falls inside a frozen
		// region on that side — a diff outside the frozen bodies is not this pattern's.
		const hunk_indices = find_matching_hunks(ctx.hunks, (h) => {
			const ours_in = inside(h.ours_range, ours_regions);
			const prettier_in = inside(h.prettier_range, prettier_regions);
			if (ours_in === null && prettier_in === null) return false;
			return ours_in !== false && prettier_in !== false;
		});
		if (hunk_indices.length === 0) return null;
		return {
			pattern: 'foreign_body_freeze',
			confidence: 'certain',
			hunk_indices,
			reason: 'foreign-language embedded body: tsv keeps the author bytes, prettier formats it'
		};
	}
};

export const PATTERNS: DivergencePattern[] = [
	// 1. Language-specific narrow patterns (certain or rare)
	bom_strip,
	self_closing_nonvoid,
	attr_value_single_quote,
	svelte_element_this_string,
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
	field_key_unquote,
	block_expression_logical,
	member_expression_call,
	member_chain_hug_convergence,
	return_type_generic_union,
	non_null_paren_base,
	union_paren_member_inline,
	forced_continuation_indent,

	// 4. Svelte-specific patterns
	foreign_body_freeze,
	spec_block_elements,
	inline_content_hug,
	inline_sibling_newline_flow,
	inline_content_block_style,
	spaced_tag_travel,
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
	comment_position
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
		.filter((h) => hunk_alters_semantic_chars(h.removed_lines.join('\n'), h.added_lines.join('\n')))
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
		char_risky_hunks
	};
}
