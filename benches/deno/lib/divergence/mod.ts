/**
 * Divergence detection module - programmatic detection of known formatting divergences.
 *
 * Three main functions:
 * - `check_safety_vs_prettier()` - Differential data-loss check (ours beyond prettier) - BUGS
 * - `detect_divergences()` - Identify known pattern matches - INTENTIONAL DIFFERENCES
 * - `generate_audit_report()` - Cross-reference patterns against conformance_prettier.md
 */

export { check_safety_vs_prettier, type SafetyViolation } from './safety.ts';
export {
	detect_divergences,
	type DetectionContext,
	type DivergenceMatch,
	type DivergencePattern,
	enrich_detection_context,
	type HunkCoverageResult,
	PATTERNS,
} from './patterns.ts';
export {
	check_expected_error,
	EXPECTED_ERROR_PATTERNS,
	type ExpectedErrorPattern,
	type ExpectedErrorResult,
} from './expected_errors.ts';
export { type DiffHunk, extract_hunks } from '../diff.ts';
export {
	type AuditReport,
	type DocumentedDivergence,
	format_audit_report,
	generate_audit_report,
	load_documented_divergences,
	parse_conformance_prettier_md,
	type PatternCoverage,
} from './validation.ts';
