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
	fixturePath: string;
	/** Fixture name from markdown link */
	fixtureName: string;
}

/** Coverage report for a single pattern */
export interface PatternCoverage {
	patternId: string;
	description: string;
	documentedFixtures: string[];
	claimedFixtures: string[];
	uncoveredFixtures: string[];
}

/** Full audit report */
export interface AuditReport {
	/** All divergences documented in conformance_prettier.md */
	documented: DocumentedDivergence[];
	/** Fixtures covered by at least one pattern */
	coveredFixtures: string[];
	/** Fixtures with no pattern coverage */
	uncoveredFixtures: string[];
	/** Per-pattern coverage details */
	patternCoverage: PatternCoverage[];
	/** Patterns that claim fixtures not in the doc */
	orphanedPatternFixtures: { patternId: string; fixtures: string[] }[];
	/** Summary stats */
	stats: {
		totalDocumented: number;
		totalCovered: number;
		totalUncovered: number;
		coveragePercent: number;
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
export function parseConformancePrettierMd(content: string): DocumentedDivergence[] {
	const divergences: DocumentedDivergence[] = [];
	const lines = content.split('\n');

	let currentSection = '';

	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];

		// Track section headings (### level)
		if (line.startsWith('### ')) {
			currentSection = line.slice(4).trim();
			continue;
		}

		// Skip non-table lines and header/separator rows
		if (!line.startsWith('|')) continue;
		if (line.includes('---')) continue;

		// Find fixture link in the line - this is the most reliable anchor
		const fixtureMatch = line.match(/\[([^\]]+)\]\(\.\.\/tests\/fixtures\/([^)]+)\/?(?:\)|\s)/);
		if (!fixtureMatch) continue;

		const [, fixtureName, fixturePath] = fixtureMatch;

		// Skip header rows (fixture column would be "Fixture")
		if (fixtureName.toLowerCase() === 'fixture') continue;

		// Extract reason by finding the cell before the fixture link
		// Split by | but be careful of escaped pipes in backticks
		const beforeFixture = line.slice(0, line.indexOf(fixtureMatch[0]));
		const cells = splitTableRow(beforeFixture);

		// cells should be: ['', feature, reason, ''] or similar
		// We want the second-to-last non-empty cell as the reason
		const nonEmptyCells = cells.filter((c) => c.trim());
		const reason = nonEmptyCells.length >= 2 ? nonEmptyCells[nonEmptyCells.length - 1].trim() : '';
		const feature = nonEmptyCells.length >= 2 ? nonEmptyCells[nonEmptyCells.length - 2].trim() : '';

		// Skip if we couldn't extract valid data
		if (!feature || feature.toLowerCase() === 'feature') continue;

		divergences.push({
			section: currentSection,
			feature,
			reason,
			fixtureName: fixtureName.trim(),
			fixturePath: fixturePath.trim().replace(/\/$/, ''), // Remove trailing slash
		});
	}

	return divergences;
}

/**
 * Split a table row by | while respecting backtick-quoted content.
 * Handles cases like `||` where pipes appear inside code spans.
 */
function splitTableRow(row: string): string[] {
	const cells: string[] = [];
	let current = '';
	let inBacktick = false;

	for (let i = 0; i < row.length; i++) {
		const char = row[i];

		if (char === '`') {
			inBacktick = !inBacktick;
			current += char;
		} else if (char === '|' && !inBacktick) {
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
export async function loadDocumentedDivergences(): Promise<DocumentedDivergence[]> {
	const docPath = new URL('../../../../docs/conformance_prettier.md', import.meta.url).pathname;
	const content = await Deno.readTextFile(docPath);
	return parseConformancePrettierMd(content);
}

/**
 * Generate a full audit report comparing documented divergences against detection patterns.
 */
export async function generateAuditReport(): Promise<AuditReport> {
	const documented = await loadDocumentedDivergences();
	const documentedPaths = new Set(documented.map((d) => d.fixturePath));

	// Collect all fixtures claimed by patterns
	const patternFixtures = new Map<string, Set<string>>();
	const allClaimedFixtures = new Set<string>();

	for (const pattern of PATTERNS) {
		const fixtures = new Set(pattern.fixtures || []);
		patternFixtures.set(pattern.id, fixtures);
		for (const f of fixtures) {
			allClaimedFixtures.add(f);
		}
	}

	// Calculate coverage
	const coveredFixtures: string[] = [];
	const uncoveredFixtures: string[] = [];

	for (const path of documentedPaths) {
		if (allClaimedFixtures.has(path)) {
			coveredFixtures.push(path);
		} else {
			uncoveredFixtures.push(path);
		}
	}

	// Per-pattern coverage — use fixtures array as primary link
	// (conformanceSections is kept for display/grouping metadata only)
	const patternCoverage: PatternCoverage[] = PATTERNS.map((pattern) => {
		const claimed = pattern.fixtures || [];
		// Fixtures the pattern claims that are documented in conformance_prettier.md
		const documentedInClaimed = claimed.filter((f) => documentedPaths.has(f));
		// Fixtures the pattern claims that aren't documented (orphaned at pattern level)
		const undocumentedInClaimed = claimed.filter((f) => !documentedPaths.has(f));

		return {
			patternId: pattern.id,
			description: pattern.description,
			documentedFixtures: documentedInClaimed,
			claimedFixtures: claimed,
			uncoveredFixtures: undocumentedInClaimed,
		};
	});

	// Find orphaned pattern fixtures (claimed but not documented)
	const orphanedPatternFixtures: { patternId: string; fixtures: string[] }[] = [];
	for (const pattern of PATTERNS) {
		const claimed = pattern.fixtures || [];
		const orphaned = claimed.filter((f) => !documentedPaths.has(f));
		if (orphaned.length > 0) {
			orphanedPatternFixtures.push({ patternId: pattern.id, fixtures: orphaned });
		}
	}

	const stats = {
		totalDocumented: documentedPaths.size,
		totalCovered: coveredFixtures.length,
		totalUncovered: uncoveredFixtures.length,
		coveragePercent: documentedPaths.size > 0
			? Math.round((coveredFixtures.length / documentedPaths.size) * 100)
			: 100,
	};

	return {
		documented,
		coveredFixtures,
		uncoveredFixtures,
		patternCoverage,
		orphanedPatternFixtures,
		stats,
	};
}

/**
 * Format audit report for terminal output.
 */
export function formatAuditReport(report: AuditReport): string {
	const lines: string[] = [];

	lines.push('Divergence Detection Audit Report');
	lines.push('='.repeat(50));
	lines.push('');

	// Summary stats
	lines.push(`Documented divergences: ${report.stats.totalDocumented}`);
	lines.push(`Covered by patterns:    ${report.stats.totalCovered}`);
	lines.push(`Uncovered:              ${report.stats.totalUncovered}`);
	lines.push(`Coverage:               ${report.stats.coveragePercent}%`);
	lines.push('');

	// Uncovered fixtures (grouped by section)
	if (report.uncoveredFixtures.length > 0) {
		lines.push('Uncovered Fixtures (no pattern detects these):');
		lines.push('-'.repeat(50));

		// Group by section
		const bySection = new Map<string, DocumentedDivergence[]>();
		for (const fixture of report.uncoveredFixtures) {
			const doc = report.documented.find((d) => d.fixturePath === fixture);
			if (doc) {
				const list = bySection.get(doc.section) || [];
				list.push(doc);
				bySection.set(doc.section, list);
			}
		}

		for (const [section, fixtures] of bySection) {
			lines.push(`\n  ${section}:`);
			for (const f of fixtures) {
				lines.push(`    - ${f.fixtureName} (${f.reason})`);
				lines.push(`      ${f.fixturePath}`);
			}
		}
		lines.push('');
	}

	// Orphaned pattern fixtures
	if (report.orphanedPatternFixtures.length > 0) {
		lines.push('Orphaned Pattern Fixtures (claimed but not documented):');
		lines.push('-'.repeat(50));
		for (const { patternId, fixtures } of report.orphanedPatternFixtures) {
			lines.push(`\n  ${patternId}:`);
			for (const f of fixtures) {
				lines.push(`    - ${f}`);
			}
		}
		lines.push('');
	}

	// Pattern coverage summary
	lines.push('Pattern Coverage Summary:');
	lines.push('-'.repeat(50));
	for (const pc of report.patternCoverage) {
		const claimed = pc.claimedFixtures.length;
		const status = claimed > 0 ? `${claimed} fixtures` : 'NO FIXTURES';
		lines.push(`  ${pc.patternId.padEnd(30)} ${status}`);
	}

	return lines.join('\n');
}
