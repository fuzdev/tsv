/**
 * Safety checks for formatter output - detect data loss (bugs).
 *
 * Uses character frequency comparison: formatting should only change whitespace
 * and punctuation, so semantic characters (letters, digits) should be preserved.
 * If source has more of any semantic character than formatted output, content was lost.
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

		// Check if line contains any lost characters
		let has_lost_char = false;
		for (const char of lost_chars.keys()) {
			if (trimmed.includes(char)) {
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
 * Count semantic (non-formatting) character frequencies in a string.
 */
function count_semantic_chars(text: string): Map<string, number> {
	const counts = new Map<string, number>();

	for (const char of text) {
		if (FORMATTING_CHARS.has(char)) continue;
		counts.set(char, (counts.get(char) ?? 0) + 1);
	}

	return counts;
}
