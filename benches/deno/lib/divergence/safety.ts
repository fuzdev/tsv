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
	/** Total characters lost */
	totalLost: number;
	/** Lines from source that appear to be missing in formatted output */
	missingLines: string[];
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

	const totalLost = [...lostChars.values()].reduce((a, b) => a + b, 0);

	// Find lines that are likely missing (contain lost chars and not in formatted)
	const missingLines = findMissingLines(source, formatted, lostChars);

	// Build summary showing the actual missing content
	let summary: string;
	if (missingLines.length > 0) {
		const preview = missingLines
			.slice(0, 3)
			.map((line) => line.trim().slice(0, 60))
			.join(' | ');
		summary = `${totalLost} chars lost. Missing: ${preview}${missingLines.length > 3 ? '...' : ''}`;
	} else {
		// Fallback to character list if we can't identify lines
		const charList = [...lostChars.entries()]
			.sort((a, b) => b[1] - a[1])
			.slice(0, 5)
			.map(([char, count]) => `'${char}'×${count}`)
			.join(', ');
		summary = `${totalLost} chars lost: ${charList}`;
	}

	return [
		{
			type: 'content_lost',
			totalLost,
			missingLines,
			summary,
		},
	];
}

/**
 * Find lines from source that appear to be missing in formatted output.
 * Looks for lines containing the lost characters that don't appear in formatted.
 */
function findMissingLines(
	source: string,
	formatted: string,
	lostChars: Map<string, number>,
): string[] {
	const sourceLines = source.split('\n');
	const formattedNormalized = normalizeForComparison(formatted);
	const missing: string[] = [];

	for (const line of sourceLines) {
		const trimmed = line.trim();
		if (!trimmed) continue;

		// Check if line contains any lost characters
		let hasLostChar = false;
		for (const char of lostChars.keys()) {
			if (trimmed.includes(char)) {
				hasLostChar = true;
				break;
			}
		}
		if (!hasLostChar) continue;

		// Check if this line's content appears in formatted output
		const lineNormalized = normalizeForComparison(trimmed);
		if (lineNormalized.length > 5 && !formattedNormalized.includes(lineNormalized)) {
			missing.push(trimmed);
		}
	}

	return missing;
}

/**
 * Normalize text for comparison by removing formatting characters.
 */
function normalizeForComparison(text: string): string {
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
function countSemanticChars(text: string): Map<string, number> {
	const counts = new Map<string, number>();

	for (const char of text) {
		if (FORMATTING_CHARS.has(char)) continue;
		counts.set(char, (counts.get(char) ?? 0) + 1);
	}

	return counts;
}
