/**
 * Divergence detection validation - cross-reference patterns against conformance_prettier.md
 *
 * Provides auditability by:
 * 1. Parsing conformance_prettier.md to extract all documented divergences
 * 2. Mapping each documented fixture to its conformance section and reason
 * 3. Comparing against registered detection patterns
 * 4. Reporting coverage gaps
 */

import { PATTERNS } from './patterns.ts';

/** A documented divergence from conformance_prettier.md */
export interface DocumentedDivergence {
	/** Section heading (e.g., "CSS: At-Rules", "TypeScript: Template Literals") */
	section: string;
	/** Feature name from table (e.g., "@container spacing", "100/101 char boundary") */
	feature: string;
	/** Reason category (e.g., "Spec violation", "Design choice", "Stable quirk") */
	reason: string;
	/** Fixture path relative to tests/fixtures/ */
	fixture_path: string;
	/** Fixture name from markdown link */
	fixture_name: string;
}

/** Coverage report for a single pattern */
export interface PatternCoverage {
	pattern_id: string;
	description: string;
	documented_fixtures: string[];
	claimed_fixtures: string[];
	uncovered_fixtures: string[];
}

/** Full audit report */
export interface AuditReport {
	/** All divergences documented in conformance_prettier.md */
	documented: DocumentedDivergence[];
	/** Fixtures covered by at least one pattern */
	covered_fixtures: string[];
	/** Fixtures with no pattern coverage */
	uncovered_fixtures: string[];
	/** Per-pattern coverage details */
	pattern_coverage: PatternCoverage[];
	/** Patterns that claim fixtures not in the doc */
	orphaned_pattern_fixtures: { pattern_id: string; fixtures: string[] }[];
	/** Summary stats */
	stats: {
		total_documented: number;
		total_covered: number;
		total_uncovered: number;
		coverage_percent: number;
	};
}

/**
 * Parse conformance_prettier.md to extract all documented divergences.
 *
 * Parses markdown tables with format:
 * | Feature | Reason | Fixture |
 * | feature_name | reason_category | [fixture_name](../tests/fixtures/path/) |
 *
 * Handles edge cases like escaped pipes in feature names (e.g., `||`).
 */
export function parse_conformance_prettier_md(content: string): DocumentedDivergence[] {
	const divergences: DocumentedDivergence[] = [];
	const lines = content.split('\n');

	let current_section = '';

	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];

		// Track section headings (### level)
		if (line.startsWith('### ')) {
			current_section = line.slice(4).trim();
			continue;
		}

		// Skip non-table lines and header/separator rows
		if (!line.startsWith('|')) continue;
		if (line.includes('---')) continue;

		// Find fixture link in the line - this is the most reliable anchor.
		// The fixture-name class excludes `|` and backtick so a `[` inside a
		// backticked feature cell (e.g. "Array literal `[` trailing") can't start
		// a spurious match that swallows the cell up to the real link's `]`.
		const fixture_match = line.match(/\[([^\]|`]+)\]\(\.\.\/tests\/fixtures\/([^)]+)\/?(?:\)|\s)/);
		if (!fixture_match) continue;

		const [, fixture_name, fixture_path] = fixture_match;

		// Skip header rows (fixture column would be "Fixture")
		if (fixture_name.toLowerCase() === 'fixture') continue;

		// Extract reason by finding the cell before the fixture link
		// Split by | but be careful of escaped pipes in backticks
		const before_fixture = line.slice(0, line.indexOf(fixture_match[0]));
		const cells = split_table_row(before_fixture);

		// cells should be: ['', feature, reason, ''] or similar
		// We want the second-to-last non-empty cell as the reason
		const non_empty_cells = cells.filter((c) => c.trim());
		const reason = non_empty_cells.length >= 2
			? non_empty_cells[non_empty_cells.length - 1].trim()
			: '';
		const feature = non_empty_cells.length >= 2
			? non_empty_cells[non_empty_cells.length - 2].trim()
			: '';

		// Skip if we couldn't extract valid data
		if (!feature || feature.toLowerCase() === 'feature') continue;

		divergences.push({
			section: current_section,
			feature,
			reason,
			fixture_name: fixture_name.trim(),
			fixture_path: fixture_path.trim().replace(/\/$/, ''), // Remove trailing slash
		});
	}

	return divergences;
}

/**
 * Split a table row by | while respecting backtick-quoted content.
 * Handles cases like `||` where pipes appear inside code spans.
 */
function split_table_row(row: string): string[] {
	const cells: string[] = [];
	let current = '';
	let in_backtick = false;

	for (let i = 0; i < row.length; i++) {
		const char = row[i];

		if (char === '`') {
			in_backtick = !in_backtick;
			current += char;
		} else if (char === '|' && !in_backtick) {
			cells.push(current);
			current = '';
		} else {
			current += char;
		}
	}
	cells.push(current);

	return cells;
}

/**
 * Load and parse conformance_prettier.md from the repo.
 */
export async function load_documented_divergences(): Promise<DocumentedDivergence[]> {
	const doc_path = new URL('../../../../docs/conformance_prettier.md', import.meta.url).pathname;
	const content = await Deno.readTextFile(doc_path);
	return parse_conformance_prettier_md(content);
}

/**
 * Generate a full audit report comparing documented divergences against detection patterns.
 */
export async function generate_audit_report(): Promise<AuditReport> {
	const documented = await load_documented_divergences();
	const documented_paths = new Set(documented.map((d) => d.fixture_path));

	// Collect all fixtures claimed by patterns
	const pattern_fixtures = new Map<string, Set<string>>();
	const all_claimed_fixtures = new Set<string>();

	for (const pattern of PATTERNS) {
		const fixtures = new Set(pattern.fixtures || []);
		pattern_fixtures.set(pattern.id, fixtures);
		for (const f of fixtures) {
			all_claimed_fixtures.add(f);
		}
	}

	// Calculate coverage
	const covered_fixtures: string[] = [];
	const uncovered_fixtures: string[] = [];

	for (const path of documented_paths) {
		if (all_claimed_fixtures.has(path)) {
			covered_fixtures.push(path);
		} else {
			uncovered_fixtures.push(path);
		}
	}

	// Per-pattern coverage — use fixtures array as primary link
	// (conformance_sections is kept for display/grouping metadata only)
	const pattern_coverage: PatternCoverage[] = PATTERNS.map((pattern) => {
		const claimed = pattern.fixtures || [];
		// Fixtures the pattern claims that are documented in conformance_prettier.md
		const documented_in_claimed = claimed.filter((f) => documented_paths.has(f));
		// Fixtures the pattern claims that aren't documented (orphaned at pattern level)
		const undocumented_in_claimed = claimed.filter((f) => !documented_paths.has(f));

		return {
			pattern_id: pattern.id,
			description: pattern.description,
			documented_fixtures: documented_in_claimed,
			claimed_fixtures: claimed,
			uncovered_fixtures: undocumented_in_claimed,
		};
	});

	// Find orphaned pattern fixtures (claimed but not documented)
	const orphaned_pattern_fixtures: { pattern_id: string; fixtures: string[] }[] = [];
	for (const pattern of PATTERNS) {
		const claimed = pattern.fixtures || [];
		const orphaned = claimed.filter((f) => !documented_paths.has(f));
		if (orphaned.length > 0) {
			orphaned_pattern_fixtures.push({ pattern_id: pattern.id, fixtures: orphaned });
		}
	}

	const stats = {
		total_documented: documented_paths.size,
		total_covered: covered_fixtures.length,
		total_uncovered: uncovered_fixtures.length,
		coverage_percent: documented_paths.size > 0
			? Math.round((covered_fixtures.length / documented_paths.size) * 100)
			: 100,
	};

	return {
		documented,
		covered_fixtures,
		uncovered_fixtures,
		pattern_coverage,
		orphaned_pattern_fixtures,
		stats,
	};
}

/**
 * Format audit report for terminal output.
 */
export function format_audit_report(report: AuditReport): string {
	const lines: string[] = [];

	lines.push('Divergence Detection Audit Report');
	lines.push('='.repeat(50));
	lines.push('');

	// Summary stats
	lines.push(`Documented divergences: ${report.stats.total_documented}`);
	lines.push(`Covered by patterns:    ${report.stats.total_covered}`);
	lines.push(`Uncovered:              ${report.stats.total_uncovered}`);
	lines.push(`Coverage:               ${report.stats.coverage_percent}%`);
	lines.push('');

	// Uncovered fixtures (grouped by section)
	if (report.uncovered_fixtures.length > 0) {
		lines.push('Uncovered Fixtures (no pattern detects these):');
		lines.push('-'.repeat(50));

		// Group by section
		const by_section = new Map<string, DocumentedDivergence[]>();
		for (const fixture of report.uncovered_fixtures) {
			const doc = report.documented.find((d) => d.fixture_path === fixture);
			if (doc) {
				const list = by_section.get(doc.section) || [];
				list.push(doc);
				by_section.set(doc.section, list);
			}
		}

		for (const [section, fixtures] of by_section) {
			lines.push(`\n  ${section}:`);
			for (const f of fixtures) {
				lines.push(`    - ${f.fixture_name} (${f.reason})`);
				lines.push(`      ${f.fixture_path}`);
			}
		}
		lines.push('');
	}

	// Orphaned pattern fixtures
	if (report.orphaned_pattern_fixtures.length > 0) {
		lines.push('Orphaned Pattern Fixtures (claimed but not documented):');
		lines.push('-'.repeat(50));
		for (const { pattern_id, fixtures } of report.orphaned_pattern_fixtures) {
			lines.push(`\n  ${pattern_id}:`);
			for (const f of fixtures) {
				lines.push(`    - ${f}`);
			}
		}
		lines.push('');
	}

	// Pattern coverage summary
	lines.push('Pattern Coverage Summary:');
	lines.push('-'.repeat(50));
	for (const pc of report.pattern_coverage) {
		const claimed = pc.claimed_fixtures.length;
		const status = claimed > 0 ? `${claimed} fixtures` : 'NO FIXTURES';
		lines.push(`  ${pc.pattern_id.padEnd(30)} ${status}`);
	}

	return lines.join('\n');
}
