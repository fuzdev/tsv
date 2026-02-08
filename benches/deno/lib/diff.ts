/**
 * Simple line-based diff utilities.
 *
 * Uses LCS (Longest Common Subsequence) algorithm to compute diffs.
 */

/** Number of digits needed to display `n` (minimum 1) */
function digitWidth(n: number): number {
	return n === 0 ? 1 : Math.floor(Math.log10(n)) + 1;
}

/** Default tab width for visual width calculations (matches prettier) */
const TAB_WIDTH = 2;

/** Only show line widths when they exceed this threshold */
const LINE_WIDTH_THRESHOLD = 90;

/** Expand tabs to spaces for consistent display */
function expandTabs(line: string, tabWidth: number = TAB_WIDTH): string {
	return line.replace(/\t/g, ' '.repeat(tabWidth));
}

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

/** A contiguous group of changed lines in a diff, with surrounding context. */
export interface DiffHunk {
	/** 0-based index of this hunk */
	index: number;
	/** All diff lines in this hunk (including context lines adjacent to changes) */
	lines: DiffLine[];
	/** Line range in "ours" (added side) that this hunk covers, or null if only removals */
	oursRange: { start: number; end: number } | null;
	/** Line range in "prettier" (removed side) that this hunk covers, or null if only additions */
	prettierRange: { start: number; end: number } | null;
	/** Lines added (ours-only) in this hunk */
	addedLines: string[];
	/** Lines removed (prettier-only) in this hunk */
	removedLines: string[];
}

/**
 * Extract diff hunks from a flat DiffLine array.
 *
 * A hunk is a contiguous group of changes (add/remove lines). Any context (same) line
 * between changes separates hunks. Line numbers for both sides are tracked.
 */
export function extractHunks(diff: DiffLine[]): DiffHunk[] {
	const hunks: DiffHunk[] = [];
	let currentLines: DiffLine[] = [];
	let addedLines: string[] = [];
	let removedLines: string[] = [];

	// Track line numbers for both sides
	let oursLine = 0; // "add" lines increment this
	let prettierLine = 0; // "remove" lines increment this

	let hunkOursStart: number | null = null;
	let hunkOursEnd: number | null = null;
	let hunkPrettierStart: number | null = null;
	let hunkPrettierEnd: number | null = null;

	function flushHunk(): void {
		if (currentLines.length === 0) return;

		hunks.push({
			index: hunks.length,
			lines: currentLines,
			oursRange: hunkOursStart !== null && hunkOursEnd !== null
				? { start: hunkOursStart, end: hunkOursEnd }
				: null,
			prettierRange: hunkPrettierStart !== null && hunkPrettierEnd !== null
				? { start: hunkPrettierStart, end: hunkPrettierEnd }
				: null,
			addedLines,
			removedLines,
		});

		currentLines = [];
		addedLines = [];
		removedLines = [];
		hunkOursStart = null;
		hunkOursEnd = null;
		hunkPrettierStart = null;
		hunkPrettierEnd = null;
	}

	for (const d of diff) {
		if (d.type === 'same') {
			// Context line closes any open hunk
			flushHunk();
			oursLine++;
			prettierLine++;
		} else if (d.type === 'add') {
			if (hunkOursStart === null) hunkOursStart = oursLine;
			hunkOursEnd = oursLine;
			currentLines.push(d);
			addedLines.push(d.line);
			oursLine++;
		} else {
			// remove
			if (hunkPrettierStart === null) hunkPrettierStart = prettierLine;
			hunkPrettierEnd = prettierLine;
			currentLines.push(d);
			removedLines.push(d.line);
			prettierLine++;
		}
	}

	flushHunk();
	return hunks;
}

/**
 * Filter diff to only include lines within N lines of context around changes.
 *
 * @param diff - The full diff lines
 * @param contextLines - Number of context lines to show around changes (default: 3)
 * @returns Filtered diff with ellipsis markers for skipped regions
 */
export function filterDiffContext(diff: DiffLine[], contextLines = 3): DiffLine[] {
	if (diff.length === 0) return [];

	// Find indices of all changed lines
	const changedIndices: number[] = [];
	for (let i = 0; i < diff.length; i++) {
		if (diff[i].type !== 'same') {
			changedIndices.push(i);
		}
	}

	if (changedIndices.length === 0) return [];

	// Build set of indices to include (changed lines + context)
	const includeIndices = new Set<number>();
	for (const idx of changedIndices) {
		for (
			let i = Math.max(0, idx - contextLines);
			i <= Math.min(diff.length - 1, idx + contextLines);
			i++
		) {
			includeIndices.add(i);
		}
	}

	// Build result with ellipsis markers for gaps
	const result: DiffLine[] = [];
	let lastIncluded = -1;

	for (let i = 0; i < diff.length; i++) {
		if (includeIndices.has(i)) {
			// Add ellipsis if there's a gap
			if (lastIncluded >= 0 && i > lastIncluded + 1) {
				result.push({ type: 'same', line: '...' });
			}
			result.push(diff[i]);
			lastIncluded = i;
		}
	}

	return result;
}

/**
 * Format a diff for terminal output with colors.
 *
 * Shows line lengths for changed lines exceeding threshold as right-aligned suffix.
 *
 * @param diff - The diff lines to format
 * @param useColor - Whether to use ANSI color codes (default: true)
 * @returns Formatted string lines
 */
export function formatDiffForTerminal(diff: DiffLine[], useColor = true): string[] {
	// Expand tabs for consistent display, then find max width among lines exceeding threshold
	const expandedLines = diff.map((d) => ({
		...d,
		expanded: expandTabs(d.line),
	}));

	let maxWidth = 0;
	for (const d of expandedLines) {
		if (d.type !== 'same' && d.expanded.length > LINE_WIDTH_THRESHOLD) {
			maxWidth = Math.max(maxWidth, d.expanded.length);
		}
	}
	const numWidth = digitWidth(maxWidth);

	return expandedLines.map((d) => {
		const prefix = d.type === 'add' ? '+' : d.type === 'remove' ? '-' : ' ';
		const width = d.expanded.length;

		if (d.type === 'same') {
			// Unchanged lines: no width suffix
			return ` ${d.expanded}`;
		}

		// Changed lines: show width only if exceeds threshold
		const color = useColor ? (d.type === 'add' ? '\x1b[32m' : '\x1b[31m') : '';
		const reset = useColor ? '\x1b[0m' : '';

		if (width > LINE_WIDTH_THRESHOLD) {
			// Pad to max width + 2 spaces, then right-aligned width
			const padding = maxWidth - width + 2;
			const widthStr = String(width).padStart(numWidth, ' ');
			return `${color}${prefix}${d.expanded}${' '.repeat(padding)}${widthStr}${reset}`;
		}

		// No width suffix for lines at or below threshold
		return `${color}${prefix}${d.expanded}${reset}`;
	});
}
