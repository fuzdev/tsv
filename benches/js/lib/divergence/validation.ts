/**
 * Divergence detection validation - cross-reference patterns against conformance_prettier.md
 *
 * Provides auditability by:
 * 1. Parsing conformance_prettier.md to extract all documented divergences
 * 2. Mapping each documented fixture to its conformance section and reason
 * 3. Running every pattern against each documented fixture's committed prettier
 *    forms to find which are actually DETECTED
 * 4. Reporting the genuine detection gaps, and separately the `fixtures[]`
 *    listing drift
 *
 * **Coverage is computed, not declared.** The audit used to answer "is this
 * fixture covered?" by looking it up in the patterns' hand-maintained
 * `fixtures[]` arrays — which measures bookkeeping, not detection. The two
 * diverge badly: the great majority of fixtures the old audit called uncovered
 * are detected by some pattern and merely unlisted. Worse, a hand-maintained
 * mirror of a computable fact drifts, and that drift produced every mislisting
 * and every stale path this audit has had to repair.
 *
 * So detection is measured by actually running the detectors
 * (`fixture_cases.ts` — the same machinery `fixture_coverage_test.ts` uses),
 * and `fixtures[]` is demoted to what it is genuinely good for: an EXPLICIT
 * ASSERTION that a named pattern claims a named fixture, gated by that test.
 * A listing gap is now reported as bookkeeping, not as a coverage hole.
 */

import { detect_divergences, PATTERNS } from './patterns.ts';
import { build_cases, build_context, fixture_dir_exists } from './fixture_cases.ts';

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
	/** Claimed fixtures absent from `conformance_prettier.md` — orphans at pattern level. */
	undocumented_fixtures: string[];
	/**
	 * Documented fixtures this pattern actually DETECTS — measured by running
	 * `detect()`, so independent of what `claimed_fixtures` says. The difference
	 * between the two is this pattern's listing drift.
	 */
	detected_fixtures: string[];
}

/**
 * Whether a documented divergence is seen by the detectors — the same
 * three-level hunk coverage the corpus comparison classifies with, plus the
 * ungradeable case a fixture (unlike a corpus file) can be in.
 */
export type DetectionStatus =
	/** Every hunk explained, in at least one prettier form the fixture pins (`all_explained`). */
	| 'explained'
	/**
	 * Some hunks explained, some not (`partial`). NOT a success: this is the
	 * masking the hunk-aware classifier exists to surface, so it is counted and
	 * listed on its own rather than folded in with `explained`.
	 */
	| 'partial'
	/** The fixture pins a prettier form, and no pattern claims any hunk in it. */
	| 'undetected'
	/** The fixture pins no (ours, prettier) pair, so detection can't be asked. */
	| 'ungradeable';

/** Per-fixture empirical detection result. */
export interface FixtureDetection {
	fixture_path: string;
	status: DetectionStatus;
	/** Pattern ids that claimed a hunk; empty unless `explained` or `partial`. */
	patterns: string[];
	/** Hunks no pattern explained; set only on `partial`. */
	unexplained_hunks?: number;
	/** Why an `ungradeable` fixture could not be graded. */
	reason?: 'no_prettier_form' | 'no_input' | 'missing_directory';
}

/** Full audit report */
export interface AuditReport {
	/** All divergences documented in conformance_prettier.md */
	documented: DocumentedDivergence[];
	/** Per-fixture detection result, measured by running the detectors */
	detection: FixtureDetection[];
	/** Fixtures whose every hunk some pattern explains */
	explained_fixtures: string[];
	/** Fixtures with an unexplained hunk left over — masked triage items */
	partial_fixtures: string[];
	/** Fixtures that pin a prettier form no pattern explains at all — the real gaps */
	undetected_fixtures: string[];
	/** Fixtures pinning no prettier form, so detection is unanswerable */
	ungradeable_fixtures: string[];
	/**
	 * Fully-explained fixtures absent from every pattern's `fixtures[]` array.
	 *
	 * Pure bookkeeping: the detector sees them, the listing doesn't say so. Not
	 * a coverage hole — the old audit counted these as uncovered, which is what
	 * made its headline measure the arrays rather than the detectors.
	 */
	unlisted_but_explained: string[];
	/** Per-pattern coverage details */
	pattern_coverage: PatternCoverage[];
	/** Patterns that claim fixtures not in the doc (the directory DOES exist) */
	orphaned_pattern_fixtures: { pattern_id: string; fixtures: string[] }[];
	/**
	 * Patterns that claim a fixture path with no directory on disk.
	 *
	 * Reported apart from `orphaned_pattern_fixtures` because the two mean opposite
	 * things: an orphan is a real fixture awaiting a doc entry (a documentation gap),
	 * while a missing path is a BROKEN REFERENCE — a fixture that was renamed or that
	 * lost its `_prettier_divergence` suffix when its divergence was resolved, leaving
	 * a listing pointing at nothing. Folded together, a broken reference reads as a
	 * doc gap and survives indefinitely; eight did. Gated by `fixture_coverage_test`.
	 */
	missing_pattern_fixtures: { pattern_id: string; fixtures: string[] }[];
	/** Summary stats */
	stats: {
		total_documented: number;
		total_explained: number;
		total_partial: number;
		total_undetected: number;
		total_ungradeable: number;
		/** `explained / documented` — the honest detection rate. */
		coverage_percent: number;
		/**
		 * `explained / (documented - ungradeable)` — detection over what could
		 * actually be graded. Reported alongside because an ungradeable fixture
		 * is neither a success nor a gap, and folding it into either lies.
		 */
		gradeable_percent: number;
		/** Documented fixtures named by some pattern's `fixtures[]` array. */
		total_listed: number;
		/** Explained but unlisted — the bookkeeping drift, not a coverage hole. */
		total_unlisted_but_explained: number;
	};
}

/**
 * Fixture link anchor: `[name](../tests/fixtures/path/)`.
 *
 * The fixture-name class excludes `|` and backtick so a `[` inside a
 * backticked feature cell (e.g. "Array literal `[` trailing") can't start
 * a spurious match that swallows the cell up to the real link's `]`.
 */
const FIXTURE_LINK_RE = /\[([^\]|`]+)\]\(\.\.\/tests\/fixtures\/([^)]+?)\/?\)/g;

/**
 * Parse conformance_prettier.md to extract all documented divergences.
 *
 * The doc anchors divergences with fixture links in three formats:
 *
 * - table rows — `| feature | reason | [name](../tests/fixtures/path/) |`
 *   (handles escaped pipes in backticked feature names, e.g. `||`)
 * - list items — `- feature — [name](../tests/fixtures/path/)`
 * - prose paragraphs — `**Feature**: … [name](../tests/fixtures/path/) …`
 *
 * All fixture links on a line are extracted (prose lines often cite several).
 * Only `*_prettier_divergence`-suffixed paths (including
 * `_svelte_prettier_divergence`) count as documented divergences — other
 * fixture links are match/contrast anchors ("where tsv matches"), not
 * divergence claims.
 */
export function parse_conformance_prettier_md(content: string): DocumentedDivergence[] {
	const divergences: DocumentedDivergence[] = [];
	const lines = content.split('\n');

	let current_section = '';

	for (let i = 0; i < lines.length; i++) {
		const line = lines[i];

		// Track section headings (### and #### levels; most recent wins)
		const heading_match = line.match(/^#{3,4}\s+(.+)/);
		if (heading_match) {
			current_section = heading_match[1].trim();
			continue;
		}

		const is_table_row = line.startsWith('|');
		if (is_table_row && line.includes('---')) continue; // separator row

		for (const fixture_match of line.matchAll(FIXTURE_LINK_RE)) {
			const fixture_name = fixture_match[1].trim();
			const fixture_path = fixture_match[2].trim().replace(/\/$/, '');

			// Skip header rows (fixture column would be "Fixture")
			if (fixture_name.toLowerCase() === 'fixture') continue;

			// Only divergence-suffixed fixtures are documented divergences
			const last_segment = fixture_path.split('/').pop() ?? '';
			if (!last_segment.endsWith('_prettier_divergence')) continue;

			let feature = '';
			let reason = '';

			if (is_table_row) {
				// Extract feature + reason from the cells before the fixture link
				const before_fixture = line.slice(0, fixture_match.index);
				const non_empty_cells = split_table_row(before_fixture).filter((c) => c.trim());
				if (non_empty_cells.length >= 2) {
					feature = non_empty_cells[non_empty_cells.length - 2].trim();
					reason = non_empty_cells[non_empty_cells.length - 1].trim();
				}
				// Skip if we couldn't extract valid data
				if (!feature || feature.toLowerCase() === 'feature') continue;
			} else if (/^\s*[-*]\s/.test(line)) {
				// List item: feature is the text between the bullet and the
				// em-dash before the link (falls back to the fixture name)
				const before_fixture = line.slice(0, fixture_match.index);
				feature = before_fixture
					.replace(/^\s*[-*]\s*/, '')
					.replace(/\s*[—–]\s*$/, '')
					.trim() || fixture_name;
			} else {
				// Prose paragraph: use the bold prefix when present
				const bold_match = line.match(/^\*\*([^*]+)\*\*/);
				feature = bold_match ? bold_match[1].replace(/:$/, '').trim() : fixture_name;
			}

			divergences.push({
				section: current_section,
				feature,
				reason,
				fixture_name,
				fixture_path,
			});
		}
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
 * Grade one documented fixture by running the real classifier over each prettier
 * form it pins.
 *
 * Routed through `detect_divergences` rather than looping `pattern.detect()`
 * directly, so this asks exactly the question the corpus comparison asks —
 * including the per-pattern language filter and, crucially, the three-level hunk
 * coverage. A fixture whose hunks are only PARTLY claimed must not read as
 * detected: that is the masking the hunk-aware design exists to prevent (a file
 * with one explained and one unexplained hunk is `partial`, not `known`), and a
 * binary detected/undetected metric would re-introduce it one level up.
 *
 * Across a fixture's several witnesses the rule is coverage, not exhaustiveness —
 * the best classification wins. Sibling variants deliberately exercise different
 * authorings (`scope_complex` pins three), and a variant's own authoring quirks
 * can add hunks the fixture's divergence never meant to pin, so demanding every
 * witness be fully explained would fail fixtures that are correctly detected.
 */
async function detect_fixture(fixture_path: string): Promise<FixtureDetection> {
	if (!fixture_dir_exists(fixture_path)) {
		return { fixture_path, status: 'ungradeable', patterns: [], reason: 'missing_directory' };
	}

	const cases = await build_cases(fixture_path);
	if (cases === 'no_input') {
		return { fixture_path, status: 'ungradeable', patterns: [], reason: 'no_input' };
	}
	if (cases.length === 0) {
		return { fixture_path, status: 'ungradeable', patterns: [], reason: 'no_prettier_form' };
	}

	const claiming = new Set<string>();
	let best: DetectionStatus = 'undetected';
	let unexplained = 0;
	for (const detection_case of cases) {
		const result = detect_divergences(build_context(detection_case));
		for (const match of result.matches) {
			if (match.hunk_indices.length > 0) claiming.add(match.pattern);
		}
		if (result.classification === 'all_explained') {
			if (best !== 'explained') {
				best = 'explained';
				unexplained = 0;
			}
		} else if (result.classification === 'partial' && best === 'undetected') {
			best = 'partial';
			unexplained = result.unexplained_hunks.length;
		}
	}

	return {
		fixture_path,
		status: best,
		patterns: [...claiming].sort(),
		...(best === 'partial' ? { unexplained_hunks: unexplained } : {}),
	};
}

/**
 * Generate a full audit report comparing documented divergences against detection patterns.
 */
export async function generate_audit_report(): Promise<AuditReport> {
	const documented = await load_documented_divergences();
	const documented_paths = new Set(documented.map((d) => d.fixture_path));

	// Collect all fixtures claimed by patterns
	const all_claimed_fixtures = new Set<string>();
	for (const pattern of PATTERNS) {
		for (const f of pattern.fixtures || []) all_claimed_fixtures.add(f);
	}

	// Empirical detection — the coverage question, answered by running detectors
	const detection: FixtureDetection[] = [];
	for (const path of documented_paths) {
		detection.push(await detect_fixture(path));
	}
	detection.sort((a, b) => a.fixture_path.localeCompare(b.fixture_path));

	const of_status = (status: DetectionStatus): string[] =>
		detection.filter((d) => d.status === status).map((d) => d.fixture_path);
	const explained_fixtures = of_status('explained');
	const partial_fixtures = of_status('partial');
	const undetected_fixtures = of_status('undetected');
	const ungradeable_fixtures = of_status('ungradeable');
	const unlisted_but_explained = explained_fixtures.filter((f) => !all_claimed_fixtures.has(f));
	const listed_documented = [...documented_paths].filter((f) => all_claimed_fixtures.has(f));

	// Which patterns detected which documented fixtures, inverted from `detection`
	const detected_by_pattern = new Map<string, string[]>();
	for (const d of detection) {
		for (const id of d.patterns) {
			const list = detected_by_pattern.get(id) ?? [];
			list.push(d.fixture_path);
			detected_by_pattern.set(id, list);
		}
	}

	// Per-pattern view — `claimed_*` reflect the hand-maintained `fixtures[]`
	// array (the explicit assertions), `detected_fixtures` what the detector
	// actually sees. Their difference is the pattern's listing drift.
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
			undocumented_fixtures: undocumented_in_claimed,
			detected_fixtures: detected_by_pattern.get(pattern.id) ?? [],
		};
	});

	// Split claimed-but-undocumented into the two cases that look alike in a list
	// but mean opposite things: the directory exists (a doc gap) vs it does not
	// (a broken reference). See `missing_pattern_fixtures`.
	const orphaned_pattern_fixtures: { pattern_id: string; fixtures: string[] }[] = [];
	const missing_pattern_fixtures: { pattern_id: string; fixtures: string[] }[] = [];
	for (const pattern of PATTERNS) {
		const claimed = pattern.fixtures || [];
		const undocumented = claimed.filter((f) => !documented_paths.has(f));
		const missing = claimed.filter((f) => !fixture_dir_exists(f));
		const orphaned = undocumented.filter((f) => !missing.includes(f));
		if (orphaned.length > 0) {
			orphaned_pattern_fixtures.push({ pattern_id: pattern.id, fixtures: orphaned });
		}
		if (missing.length > 0) {
			missing_pattern_fixtures.push({ pattern_id: pattern.id, fixtures: missing });
		}
	}

	const gradeable = documented_paths.size - ungradeable_fixtures.length;
	const stats = {
		total_documented: documented_paths.size,
		total_explained: explained_fixtures.length,
		total_partial: partial_fixtures.length,
		total_undetected: undetected_fixtures.length,
		total_ungradeable: ungradeable_fixtures.length,
		coverage_percent: documented_paths.size > 0
			? Math.round((explained_fixtures.length / documented_paths.size) * 100)
			: 100,
		gradeable_percent: gradeable > 0
			? Math.round((explained_fixtures.length / gradeable) * 100)
			: 100,
		total_listed: listed_documented.length,
		total_unlisted_but_explained: unlisted_but_explained.length,
	};

	return {
		documented,
		detection,
		explained_fixtures,
		partial_fixtures,
		undetected_fixtures,
		ungradeable_fixtures,
		unlisted_but_explained,
		pattern_coverage,
		orphaned_pattern_fixtures,
		missing_pattern_fixtures,
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

	// Summary stats — detection is measured by running the detectors; the
	// `fixtures[]` listing is reported below it as bookkeeping, not coverage.
	const s = report.stats;
	lines.push(`Documented divergences: ${s.total_documented}`);
	lines.push(`Fully explained:        ${s.total_explained}`);
	lines.push(`Partial (hunks left):   ${s.total_partial}`);
	lines.push(`Undetected (real gaps): ${s.total_undetected}`);
	lines.push(`Ungradeable:            ${s.total_ungradeable}  (pin no prettier form to test)`);
	lines.push(`Detection:              ${s.coverage_percent}%  (${s.gradeable_percent}% of gradeable)`);
	lines.push('');
	lines.push(`Listed in fixtures[]:   ${s.total_listed}`);
	lines.push(`Explained but unlisted: ${s.total_unlisted_but_explained}  (bookkeeping, not a gap)`);
	lines.push('');

	/** Group a fixture-path list by its conformance-doc section, for display. */
	const push_by_section = (heading: string, paths: string[]): void => {
		if (paths.length === 0) return;
		lines.push(heading);
		lines.push('-'.repeat(50));
		const by_section = new Map<string, DocumentedDivergence[]>();
		for (const fixture of paths) {
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
				const detected = report.detection.find((d) => d.fixture_path === f.fixture_path);
				const left = detected?.unexplained_hunks;
				lines.push(
					`    - ${f.fixture_name}${f.reason ? ` (${f.reason})` : ''}` +
						(left ? `  [${left} hunk(s) unexplained]` : ''),
				);
				lines.push(`      ${f.fixture_path}`);
			}
		}
		lines.push('');
	};

	// The two actionable lists. `partial` is listed separately and ahead of
	// nothing-detected because it is the quieter failure: a pattern IS attached,
	// so it reads as covered until you count hunks.
	push_by_section(
		'Undetected Fixtures (pin a prettier form, no pattern explains it):',
		report.undetected_fixtures,
	);
	push_by_section(
		'Partially Explained Fixtures (some hunks claimed, some left over):',
		report.partial_fixtures,
	);

	// Ungradeable — neither a success nor a gap; listed so the number is legible
	if (report.ungradeable_fixtures.length > 0) {
		lines.push('Ungradeable Fixtures (no committed prettier form to detect against):');
		lines.push('-'.repeat(50));
		for (const path of report.ungradeable_fixtures) {
			const reason = report.detection.find((d) => d.fixture_path === path)?.reason;
			lines.push(`  - ${path}${reason && reason !== 'no_prettier_form' ? `  [${reason}]` : ''}`);
		}
		lines.push('');
	}

	// Broken references — listed first, above the orphans they used to hide among
	if (report.missing_pattern_fixtures.length > 0) {
		lines.push('BROKEN Pattern Fixtures (claimed path has no directory on disk):');
		lines.push('-'.repeat(50));
		for (const { pattern_id, fixtures } of report.missing_pattern_fixtures) {
			lines.push(`\n  ${pattern_id}:`);
			for (const f of fixtures) {
				lines.push(`    - ${f}`);
			}
		}
		lines.push('');
		lines.push('  Fix the path or unlist it — a renamed fixture, or one that lost its');
		lines.push('  _prettier_divergence suffix when its divergence was resolved.');
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

	// Pattern summary — `listed` is the explicit assertions the coverage test
	// gates, `detects` what the detector actually claims among documented
	// fixtures. A pattern with detects=0 explains no documented divergence
	// (which does not by itself make it dead — it may fire only on corpus code).
	lines.push('Pattern Summary  (listed = fixtures[] entries, detects = measured):');
	lines.push('-'.repeat(50));
	for (const pc of report.pattern_coverage) {
		const listed = pc.claimed_fixtures.length;
		const detects = pc.detected_fixtures.length;
		lines.push(
			`  ${pc.pattern_id.padEnd(34)} listed ${String(listed).padStart(3)}   detects ${
				String(detects).padStart(3)
			}`,
		);
	}

	return lines.join('\n');
}
