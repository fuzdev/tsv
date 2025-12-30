/**
 * Simple line-based diff utilities.
 *
 * Uses LCS (Longest Common Subsequence) algorithm to compute diffs.
 */

/** Line diff result */
export interface DiffLine {
	type: 'same' | 'add' | 'remove';
	line: string;
}

/**
 * Generate a simple unified diff between two strings.
 *
 * @param a - The original/expected string
 * @param b - The new/actual string
 * @returns Array of diff lines with type annotations
 */
export function diffLines(a: string, b: string): DiffLine[] {
	const aLines = a.split('\n');
	const bLines = b.split('\n');
	const result: DiffLine[] = [];

	const lcs = computeLCS(aLines, bLines);
	let ai = 0,
		bi = 0,
		li = 0;

	while (ai < aLines.length || bi < bLines.length) {
		if (li < lcs.length && ai < aLines.length && aLines[ai] === lcs[li]) {
			if (bi < bLines.length && bLines[bi] === lcs[li]) {
				result.push({ type: 'same', line: aLines[ai] });
				ai++;
				bi++;
				li++;
			} else {
				result.push({ type: 'add', line: bLines[bi] });
				bi++;
			}
		} else if (ai < aLines.length && (li >= lcs.length || aLines[ai] !== lcs[li])) {
			result.push({ type: 'remove', line: aLines[ai] });
			ai++;
		} else if (bi < bLines.length) {
			result.push({ type: 'add', line: bLines[bi] });
			bi++;
		}
	}

	return result;
}

/** Compute longest common subsequence of two string arrays */
function computeLCS(a: string[], b: string[]): string[] {
	const m = a.length,
		n = b.length;
	const dp: number[][] = Array.from({ length: m + 1 }, () => Array(n + 1).fill(0));

	for (let i = 1; i <= m; i++) {
		for (let j = 1; j <= n; j++) {
			if (a[i - 1] === b[j - 1]) {
				dp[i][j] = dp[i - 1][j - 1] + 1;
			} else {
				dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
			}
		}
	}

	// Backtrack to find LCS
	const lcs: string[] = [];
	let i = m,
		j = n;
	while (i > 0 && j > 0) {
		if (a[i - 1] === b[j - 1]) {
			lcs.unshift(a[i - 1]);
			i--;
			j--;
		} else if (dp[i - 1][j] > dp[i][j - 1]) {
			i--;
		} else {
			j--;
		}
	}

	return lcs;
}

/**
 * Format a diff for terminal output with colors.
 *
 * @param diff - The diff lines to format
 * @param useColor - Whether to use ANSI color codes (default: true)
 * @returns Formatted string lines
 */
export function formatDiffForTerminal(diff: DiffLine[], useColor = true): string[] {
	return diff.map((d) => {
		const prefix = d.type === 'add' ? '+' : d.type === 'remove' ? '-' : ' ';
		if (!useColor) {
			return `${prefix}${d.line}`;
		}
		const color = d.type === 'add' ? '\x1b[32m' : d.type === 'remove' ? '\x1b[31m' : '';
		const reset = d.type === 'same' ? '' : '\x1b[0m';
		return `${color}${prefix}${d.line}${reset}`;
	});
}
