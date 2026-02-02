/**
 * Safety checks for formatter output - detect data loss (bugs).
 *
 * Uses character frequency comparison: formatting should only change whitespace
 * and punctuation, so semantic characters (letters, digits) should be preserved.
 * If source has more of any semantic character than formatted output, content was lost.
 *
 * Safety violations are BUGS, not intentional divergences.
 */

export interface SafetyViolation {
	type: 'content_lost';
	/** Characters that were lost (with counts) */
	lostChars: Map<string, number>;
	/** Human-readable summary */
	summary: string;
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
 * Check for safety violations (data loss) between source and our formatted output.
 *
 * Compares character frequencies for semantic characters (letters, digits, etc.).
 * If source has more of any semantic character than formatted output, content was lost.
 *
 * This approach:
 * - Catches lost comments, identifiers, string contents, numbers
 * - No false positives from formatting changes (whitespace, punctuation)
 * - No false positives from regex misunderstanding string boundaries
 *
 * @param source - Original source code
 * @param formatted - Our formatted output
 * @returns Array of safety violations (empty = safe)
 */
export function checkSafety(source: string, formatted: string): SafetyViolation[] {
	const sourceCounts = countSemanticChars(source);
	const formattedCounts = countSemanticChars(formatted);

	// Find characters where source has MORE than formatted (content lost)
	const lostChars = new Map<string, number>();

	for (const [char, sourceCount] of sourceCounts) {
		const formattedCount = formattedCounts.get(char) ?? 0;
		if (sourceCount > formattedCount) {
			lostChars.set(char, sourceCount - formattedCount);
		}
	}

	if (lostChars.size === 0) {
		return [];
	}

	// Build human-readable summary
	const charList = [...lostChars.entries()]
		.sort((a, b) => b[1] - a[1]) // Sort by count descending
		.slice(0, 10) // Limit to first 10 for readability
		.map(([char, count]) => `'${char}' ×${count}`)
		.join(', ');

	const totalLost = [...lostChars.values()].reduce((a, b) => a + b, 0);

	return [
		{
			type: 'content_lost',
			lostChars,
			summary: `${totalLost} chars lost: ${charList}${lostChars.size > 10 ? '...' : ''}`,
		},
	];
}

/**
 * Count semantic (non-formatting) character frequencies in a string.
 * Only counts characters that represent actual content, not formatting.
 */
function countSemanticChars(text: string): Map<string, number> {
	const counts = new Map<string, number>();

	for (const char of text) {
		if (FORMATTING_CHARS.has(char)) continue;
		counts.set(char, (counts.get(char) ?? 0) + 1);
	}

	return counts;
}
