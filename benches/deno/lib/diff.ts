/**
 * Simple line-based diff utilities.
 *
 * Uses LCS (Longest Common Subsequence) algorithm to compute diffs.
 */

/** Number of digits needed to display `n` (minimum 1) */
function digit_width(n: number): number {
	return n === 0 ? 1 : Math.floor(Math.log10(n)) + 1;
}

/** Default tab width for visual width calculations (matches prettier) */
const TAB_WIDTH = 2;

/** Only show line widths when they exceed this threshold */
const LINE_WIDTH_THRESHOLD = 90;

/** Expand tabs to spaces for consistent display */
function expand_tabs(line: string, tab_width: number = TAB_WIDTH): string {
	return line.replace(/\t/g, ' '.repeat(tab_width));
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
export function diff_lines(a: string, b: string): DiffLine[] {
	const a_lines = a.split('\n');
	const b_lines = b.split('\n');
	const result: DiffLine[] = [];

	const lcs = compute_lcs(a_lines, b_lines);
	let ai = 0,
		bi = 0,
		li = 0;

	while (ai < a_lines.length || bi < b_lines.length) {
		if (li < lcs.length && ai < a_lines.length && a_lines[ai] === lcs[li]) {
			if (bi < b_lines.length && b_lines[bi] === lcs[li]) {
				result.push({ type: 'same', line: a_lines[ai] });
				ai++;
				bi++;
				li++;
			} else {
				result.push({ type: 'add', line: b_lines[bi] });
				bi++;
			}
		} else if (ai < a_lines.length && (li >= lcs.length || a_lines[ai] !== lcs[li])) {
			result.push({ type: 'remove', line: a_lines[ai] });
			ai++;
		} else if (bi < b_lines.length) {
			result.push({ type: 'add', line: b_lines[bi] });
			bi++;
		}
	}

	return result;
}

/** Compute longest common subsequence of two string arrays */
function compute_lcs(a: string[], b: string[]): string[] {
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
	ours_range: { start: number; end: number } | null;
	/** Line range in "prettier" (removed side) that this hunk covers, or null if only additions */
	prettier_range: { start: number; end: number } | null;
	/** Lines added (ours-only) in this hunk */
	added_lines: string[];
	/** Lines removed (prettier-only) in this hunk */
	removed_lines: string[];
}

/**
 * Extract diff hunks from a flat DiffLine array.
 *
 * A hunk is a contiguous group of changes (add/remove lines). Any context (same) line
 * between changes separates hunks. Line numbers for both sides are tracked.
 */
export function extract_hunks(diff: DiffLine[]): DiffHunk[] {
	const hunks: DiffHunk[] = [];
	let current_lines: DiffLine[] = [];
	let added_lines: string[] = [];
	let removed_lines: string[] = [];

	// Track line numbers for both sides
	let ours_line = 0; // "add" lines increment this
	let prettier_line = 0; // "remove" lines increment this

	let hunk_ours_start: number | null = null;
	let hunk_ours_end: number | null = null;
	let hunk_prettier_start: number | null = null;
	let hunk_prettier_end: number | null = null;

	function flush_hunk(): void {
		if (current_lines.length === 0) return;

		hunks.push({
			index: hunks.length,
			lines: current_lines,
			ours_range: hunk_ours_start !== null && hunk_ours_end !== null
				? { start: hunk_ours_start, end: hunk_ours_end }
				: null,
			prettier_range: hunk_prettier_start !== null && hunk_prettier_end !== null
				? { start: hunk_prettier_start, end: hunk_prettier_end }
				: null,
			added_lines: added_lines,
			removed_lines: removed_lines,
		});

		current_lines = [];
		added_lines = [];
		removed_lines = [];
		hunk_ours_start = null;
		hunk_ours_end = null;
		hunk_prettier_start = null;
		hunk_prettier_end = null;
	}

	for (const d of diff) {
		if (d.type === 'same') {
			// Context line closes any open hunk
			flush_hunk();
			ours_line++;
			prettier_line++;
		} else if (d.type === 'add') {
			if (hunk_ours_start === null) hunk_ours_start = ours_line;
			hunk_ours_end = ours_line;
			current_lines.push(d);
			added_lines.push(d.line);
			ours_line++;
		} else {
			// remove
			if (hunk_prettier_start === null) hunk_prettier_start = prettier_line;
			hunk_prettier_end = prettier_line;
			current_lines.push(d);
			removed_lines.push(d.line);
			prettier_line++;
		}
	}

	flush_hunk();
	return hunks;
}

/**
 * Filter diff to only include lines within N lines of context around changes.
 *
 * @param diff - The full diff lines
 * @param context_lines - Number of context lines to show around changes (default: 3)
 * @returns Filtered diff with ellipsis markers for skipped regions
 */
export function filter_diff_context(diff: DiffLine[], context_lines = 3): DiffLine[] {
	if (diff.length === 0) return [];

	// Find indices of all changed lines
	const changed_indices: number[] = [];
	for (let i = 0; i < diff.length; i++) {
		if (diff[i].type !== 'same') {
			changed_indices.push(i);
		}
	}

	if (changed_indices.length === 0) return [];

	// Build set of indices to include (changed lines + context)
	const include_indices = new Set<number>();
	for (const idx of changed_indices) {
		for (
			let i = Math.max(0, idx - context_lines);
			i <= Math.min(diff.length - 1, idx + context_lines);
			i++
		) {
			include_indices.add(i);
		}
	}

	// Build result with ellipsis markers for gaps
	const result: DiffLine[] = [];
	let last_included = -1;

	for (let i = 0; i < diff.length; i++) {
		if (include_indices.has(i)) {
			// Add ellipsis if there's a gap
			if (last_included >= 0 && i > last_included + 1) {
				result.push({ type: 'same', line: '...' });
			}
			result.push(diff[i]);
			last_included = i;
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
export function format_diff_for_terminal(diff: DiffLine[], use_color = true): string[] {
	// Expand tabs for consistent display, then find max width among lines exceeding threshold
	const expanded_lines = diff.map((d) => ({
		...d,
		expanded: expand_tabs(d.line),
	}));

	let max_width = 0;
	for (const d of expanded_lines) {
		if (d.type !== 'same' && d.expanded.length > LINE_WIDTH_THRESHOLD) {
			max_width = Math.max(max_width, d.expanded.length);
		}
	}
	const num_width = digit_width(max_width);

	return expanded_lines.map((d) => {
		const prefix = d.type === 'add' ? '+' : d.type === 'remove' ? '-' : ' ';
		const width = d.expanded.length;

		if (d.type === 'same') {
			// Unchanged lines: no width suffix
			return ` ${d.expanded}`;
		}

		// Changed lines: show width only if exceeds threshold
		const color = use_color ? (d.type === 'add' ? '\x1b[32m' : '\x1b[31m') : '';
		const reset = use_color ? '\x1b[0m' : '';

		if (width > LINE_WIDTH_THRESHOLD) {
			// Pad to max width + 2 spaces, then right-aligned width
			const padding = max_width - width + 2;
			const width_str = String(width).padStart(num_width, ' ');
			return `${color}${prefix}${d.expanded}${' '.repeat(padding)}${width_str}${reset}`;
		}

		// No width suffix for lines at or below threshold
		return `${color}${prefix}${d.expanded}${reset}`;
	});
}
