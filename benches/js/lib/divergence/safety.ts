/**
 * Safety checks for formatter output - detect data loss (bugs).
 *
 * Uses character frequency comparison: formatting should only change whitespace
 * and punctuation, so semantic characters (letters, digits) should be preserved.
 * If source has more of any semantic character than formatted output, content was lost.
 *
 * Comparison is ASCII-case-INSENSITIVE. A formatter never recases semantic content
 * (identifiers and string bytes are preserved exactly); the only thing it recases is a
 * case-insensitive token — CSS units/keywords/hex, JS numeric `0xFF`/`1E10` — which is
 * canonicalization, never content loss. Folding ASCII case before counting lets such a
 * change cancel even when the two formatters canonicalize in OPPOSITE directions (tsv
 * lowercases every CSS unit to its spec-serialized form while prettier upcases the
 * `Hz`/`kHz`/`Q` trio — the `units_serialize_case` divergence), which a case-sensitive
 * count would otherwise report as a fabricated loss/addition. A genuine letter drop still
 * lowers the folded count, so real loss is unaffected.
 *
 * The count also excludes **leading union-member pipes** — the `|` a union type
 * grows when it breaks across lines (`A | B` → `| A⏎| B`). That extra pipe is pure
 * break layout (`| A | B` ≡ `A | B`), so when tsv breaks a union that prettier keeps
 * inline (the `return_type_generic_union` divergence, whose method-signature form
 * shows up in dense declaration files) the added `|` would otherwise fake a
 * `content_added` — and SAFETY is a gated bucket. Only the first, operand-less pipe
 * of a break is dropped; the inter-member separators stay counted, so a genuinely
 * dropped union member still lowers the count. See `is_leading_union_pipe`.
 *
 * Safety violations are BUGS, not intentional divergences.
 *
 * The check is DIFFERENTIAL against prettier (`check_safety_vs_prettier`):
 * prettier is the source of truth, so only the loss/addition OUR output incurs
 * beyond prettier's own normalizations counts. It runs in-process on strings
 * already in the Deno heap (source, our FFI output, prettier output), so it
 * never shells out and is a negligible fraction of per-file cost (prettier
 * dominates).
 */

export interface SafetyViolation {
	type: 'content_lost' | 'content_added';
	/** Total characters affected beyond prettier (lost for content_lost, added for content_added) */
	total: number;
	/**
	 * Per-character differential breakdown, `real > 0` only, sorted by `real` desc.
	 * `real` is the flagged amount (`max(0, ours - prettier)`); `ours` and
	 * `prettier` are each formatter's raw delta vs source, so a reader can see how
	 * much of `ours` is shared with prettier (e.g. `'|' real 2, ours 28, prettier
	 * 26` → 26 of 28 pipe-drops are shared, only 2 are real).
	 */
	chars: SafetyCharDelta[];
	/** Lines from source missing in formatted (content_lost), or extra in formatted (content_added) */
	missing_lines: string[];
	/** Human-readable summary */
	summary: string;
}

/** One character's differential delta in a {@link SafetyViolation}. */
export interface SafetyCharDelta {
	char: string;
	/** The flagged amount: `max(0, ours - prettier)`. */
	real: number;
	/** Characters this char our output dropped (content_lost) or added (content_added) vs source. */
	ours: number;
	/** Characters this char prettier dropped (content_lost) or added (content_added) vs source. */
	prettier: number;
}

/**
 * Characters that formatters legitimately change (excluded from safety check).
 *
 * - Whitespace: space, tab, newline, carriage return
 * - Quotes: ' " ` (style normalization)
 * - Separators: , ; (trailing commas, ASI)
 * - Parens: ( ) (optional in arrow params, grouping)
 *
 * We intentionally TRACK (do not exclude):
 * - Brackets: [ ] { } < > - losing these changes semantics
 * - All letters, digits, operators - these are content
 *
 * Known blind spots (fundamental tradeoffs):
 * - Parens excluded: a bug dropping parens around `(a + b) * c` → `a + b * c` is invisible
 * - Commas/semicolons excluded: removing commas from objects or adding semicolons passes
 * - Quote changes: `"x"` → `'x'` or `` `x` `` is invisible
 * - Reordering: swapping statements preserves character frequencies
 */
const FORMATTING_CHARS = new Set([
	// Whitespace
	' ',
	'\t',
	'\n',
	'\r',
	// Quotes (style normalization)
	"'",
	'"',
	'`',
	// Separators (trailing commas, ASI)
	',',
	';',
	// Parens (optional in arrow params, some grouping)
	'(',
	')',
]);

/**
 * Differential safety check: report only the data loss/addition OUR output incurs
 * BEYOND what prettier incurs.
 *
 * Prettier is the source of truth, so any character transformation prettier ALSO
 * performs (removing a redundant leading `|` in `type A = | C`, number
 * normalization, lowercasing CSS keywords, etc.) is a legitimate normalization,
 * not data loss — even though it drops the source character count. Comparing OUR
 * output directly against source flags those shared normalizations as false
 * positives. The differential isolates the REAL violation: per character,
 * `real = max(0, ours_delta - prettier_delta)`.
 *
 * This subsumes the old `ours !== prettier` guard: when `ours === prettier`,
 * every per-char delta matches prettier's and the real set is empty.
 *
 * Examples (validated on corpus files):
 * - `css/grid/grid.css`: ours drops `{'.':1,'0':4}`, prettier drops
 *   `{'.':1, lowercased letters...}`; REAL = `{'0':4}` (we strip trailing zeros
 *   in grid templates where prettier keeps them) → correctly KEPT.
 * - `typescript/union/union-parens.ts`: ours drops 28 `|`, prettier drops 26
 *   (shared leading-pipe removal); REAL = 2 extra `|` → correctly isolated.
 * - `typescript/comments/method_types.ts`: ours drops 56 comment chars, prettier
 *   drops none; REAL = 56 → genuine comment-drop, correctly KEPT.
 *
 * @param source - Original source code
 * @param ours - Our formatted output
 * @param prettier - Prettier's formatted output (source of truth)
 * @returns Array of REAL safety violations (empty = safe relative to prettier)
 */
export function check_safety_vs_prettier(
	source: string,
	ours: string,
	prettier: string,
): SafetyViolation[] {
	const source_counts = count_semantic_chars(source);
	const ours_counts = count_semantic_chars(ours);
	const prettier_counts = count_semantic_chars(prettier);

	// Per-char loss each output incurs relative to source, and the differential
	// remainder our output incurs beyond prettier.
	const lost = differential_deltas(
		excess_chars(source_counts, ours_counts),
		excess_chars(source_counts, prettier_counts),
	);
	const added = differential_deltas(
		excess_chars(ours_counts, source_counts),
		excess_chars(prettier_counts, source_counts),
	);

	const violations: SafetyViolation[] = [];
	// For content_lost the "missing" lines live in source; for content_added the
	// "extra" lines live in our output — so the scan text flips.
	if (lost.length > 0) violations.push(make_violation('content_lost', lost, source, ours));
	if (added.length > 0) violations.push(make_violation('content_added', added, ours, source));
	return violations;
}

/**
 * Per-char positive excess of `a` over `b`: `{char => a[char] - b[char]}` for
 * every char where `a` has strictly more than `b`.
 */
function excess_chars(
	a: Map<string, number>,
	b: Map<string, number>,
): Map<string, number> {
	const out = new Map<string, number>();
	for (const [char, a_count] of a) {
		const b_count = b.get(char) ?? 0;
		if (a_count > b_count) out.set(char, a_count - b_count);
	}
	return out;
}

/**
 * Build the differential per-char deltas: for each char our output drops/adds,
 * keep it only when `real = ours - prettier > 0` (loss/addition beyond what
 * prettier does), carrying the raw `ours`/`prettier` deltas for display. Sorted
 * by `real` desc.
 */
function differential_deltas(
	ours_excess: Map<string, number>,
	prettier_excess: Map<string, number>,
): SafetyCharDelta[] {
	const out: SafetyCharDelta[] = [];
	for (const [char, ours] of ours_excess) {
		const prettier = prettier_excess.get(char) ?? 0;
		const real = ours - prettier;
		if (real > 0) out.push({ char, real, ours, prettier });
	}
	out.sort((a, b) => b.real - a.real);
	return out;
}

/**
 * Assemble a {@link SafetyViolation} from its differential deltas.
 *
 * `scan_text`/`other_text` orient the missing-line heuristic: lost content is
 * located in `source` (absent from our output); added content is located in our
 * output (absent from `source`).
 */
function make_violation(
	type: SafetyViolation['type'],
	chars: SafetyCharDelta[],
	scan_text: string,
	other_text: string,
): SafetyViolation {
	const total = chars.reduce((sum, d) => sum + d.real, 0);
	const real_map = new Map(chars.map((d) => [d.char, d.real]));
	const missing_lines = find_missing_lines(scan_text, other_text, real_map);
	return { type, total, chars, missing_lines, summary: format_summary(total, chars) };
}

/**
 * Compact one-line summary: total beyond prettier, then up to 5 chars with their
 * shared context. The first char is labeled (`ours N, prettier M`); the rest use
 * the bare `(N, M)` form once the labels are established.
 */
function format_summary(total: number, chars: SafetyCharDelta[]): string {
	const shown = chars.slice(0, 5);
	const parts = shown.map((d, i) => {
		const ctx = i === 0 ? `ours ${d.ours}, prettier ${d.prettier}` : `${d.ours}, ${d.prettier}`;
		return `'${d.char}'×${d.real} (${ctx})`;
	});
	const more = chars.length > shown.length ? `, +${chars.length - shown.length} more` : '';
	return `${total} beyond prettier — ${parts.join(', ')}${more}`;
}

/**
 * Find lines from source that appear to be missing in formatted output.
 * Looks for lines containing the lost characters that don't appear in formatted.
 */
function find_missing_lines(
	source: string,
	formatted: string,
	lost_chars: Map<string, number>,
): string[] {
	const source_lines = source.split('\n');
	const formatted_normalized = normalize_for_comparison(formatted);
	const missing: string[] = [];

	for (const line of source_lines) {
		const trimmed = line.trim();
		if (!trimmed) continue;

		// Check if line contains any lost characters. The keys are ASCII-case-folded
		// (count_semantic_chars), so match case-insensitively to find a source line whose
		// letter appears uppercase.
		const trimmed_lower = trimmed.toLowerCase();
		let has_lost_char = false;
		for (const char of lost_chars.keys()) {
			if (trimmed_lower.includes(char)) {
				has_lost_char = true;
				break;
			}
		}
		if (!has_lost_char) continue;

		// Check if this line's content appears in formatted output
		const line_normalized = normalize_for_comparison(trimmed);
		if (line_normalized.length > 5 && !formatted_normalized.includes(line_normalized)) {
			missing.push(trimmed);
		}
	}

	return missing;
}

/**
 * Normalize text for comparison by removing formatting characters.
 */
function normalize_for_comparison(text: string): string {
	let result = '';
	for (const char of text) {
		if (!FORMATTING_CHARS.has(char)) {
			result += char;
		}
	}
	return result;
}

/**
 * Whether an ASCII/`char` code point ends a type or value *operand* — an
 * identifier char, a closing bracket, or a string quote. A `|` whose preceding
 * non-whitespace char is an operand-ender is INFIX (a union separator between two
 * members, or a bitwise-or) and carries content; a `|` preceded by anything else
 * has no left operand and is a leading union-member pipe (pure break layout). Any
 * non-ASCII code point counts as an operand char (a Unicode identifier letter).
 */
function is_operand_end_char(char: string): boolean {
	if (char.charCodeAt(0) > 127) return true;
	return /[\w)\]}'"`$]/.test(char);
}

/**
 * Whether a `|` at this position is a **leading union-member pipe** — the
 * separator tsv (and prettier) emit when a union type breaks across lines
 * (`Resolvable<⏎| A⏎| B⏎>`, or the bracket-hugged `Resolvable<| A`). It is pure
 * break layout, semantically identical to no leading pipe (`| A | B` ≡ `A | B`),
 * so it must not count as a semantic char — otherwise tsv breaking a union that
 * prettier keeps inline (the return-type-union divergence) reads as fabricated
 * `content_added`. Decided from the previous non-whitespace char (`prev`, and
 * `prev2` before it):
 *   - no left operand (`prev` not an operand-ender) → leading (exclude);
 *   - `||` (`prev === '|'` with the two pipes ADJACENT — `!ws_before`) → logical-or,
 *     both pipes are content (count); a *space-separated* `| |` (`ws_before`) is two
 *     leading union pipes (tsv can emit `| | | |` collapsing nested single-member
 *     unions), so it falls through to the leading check and is excluded;
 *   - `=>` (`prev === '>' && prev2 === '='`) → arrow has no operand, so a union
 *     broken right after it is leading; a generic-close `>` (any other `prev2`)
 *     DOES end an operand (count).
 * Only the FIRST pipe of a broken union is leading; the subsequent `| B`, `| C`
 * separators have the previous member as their left operand, so a broken union
 * counts the SAME number of pipes as its inline form — a dropped member still
 * removes a counted (infix) pipe and stays flagged.
 */
function is_leading_union_pipe(prev: string, prev2: string, ws_before: boolean): boolean {
	if (prev === '') return true; // start of text
	if (prev === '|' && !ws_before) return false; // adjacent `||` logical-or — count both
	if (prev === '>') return prev2 === '='; // `=>` has no operand; generic-close `>` does
	return !is_operand_end_char(prev);
}

/**
 * Count semantic (non-formatting) character frequencies in a string, folding ASCII
 * case (see the module doc — a case-only swap is canonicalization, never content
 * loss) and excluding leading union-member pipes (see `is_leading_union_pipe` — a
 * break-layout artifact, not content).
 */
function count_semantic_chars(text: string): Map<string, number> {
	const counts = new Map<string, number>();
	// Last two non-whitespace chars, tracked across newlines so a broken union's
	// `| B` line sees the previous member's last char as `prev` (an operand-ender).
	// `ws_before` records whether whitespace intervened since `prev`, so an adjacent
	// `||` is distinguished from a space-separated `| |` (two leading pipes).
	let prev = '';
	let prev2 = '';
	let ws_before = false;

	for (const char of text) {
		if (char === ' ' || char === '\t' || char === '\n' || char === '\r') {
			ws_before = true;
			continue;
		}
		if (char === '|' && is_leading_union_pipe(prev, prev2, ws_before)) {
			prev2 = prev;
			prev = char;
			ws_before = false;
			continue;
		}
		if (!FORMATTING_CHARS.has(char)) {
			const key = fold_ascii_case(char);
			counts.set(key, (counts.get(key) ?? 0) + 1);
		}
		prev2 = prev;
		prev = char;
		ws_before = false;
	}

	return counts;
}

/**
 * Whether a single diff hunk changes semantic character counts — i.e. whether this
 * hunk could be the one responsible for a file-level SAFETY differential.
 *
 * The SAFETY check itself is whole-file (it compares source/ours/prettier as three
 * strings), so on its own it cannot say WHICH hunk carried the flagged characters.
 * That gap is what let an unrelated pattern's coverage vouch for a differential it
 * had nothing to do with: on `prettier/tests/format/html/tags/tags.html` the entire
 * 9-char delta lives in one self-closing-tag hunk, while two pure-whitespace hunks
 * elsewhere in the file were equally load-bearing for the downgrade. Scoring each
 * hunk here restores the causal link — a whitespace-only hunk is never char-risky,
 * so it can never be asked to vouch, and losing its explanation can no longer flip
 * the file into the gated bucket.
 *
 * Uses the same folding/exclusion rules as the whole-file check (`count_semantic_chars`),
 * so "risky" here means exactly "would move the number SAFETY reports".
 */
export function hunk_alters_semantic_chars(removed: string, added: string): boolean {
	const before = count_semantic_chars(removed);
	const after = count_semantic_chars(added);
	if (before.size !== after.size) return true;
	for (const [char, count] of before) {
		if ((after.get(char) ?? 0) !== count) return true;
	}
	return false;
}

/**
 * Fold an ASCII uppercase letter (A–Z) to lowercase; every other code point
 * (digits, punctuation, non-ASCII, astral) is returned unchanged. Deliberately
 * ASCII-only — `String.prototype.toLowerCase` is Unicode-aware and would fold
 * locale-sensitive forms the CSS/JS case-insensitivity rules don't cover.
 */
function fold_ascii_case(char: string): string {
	const code = char.charCodeAt(0);
	return code >= 65 && code <= 90 ? String.fromCharCode(code + 32) : char;
}
