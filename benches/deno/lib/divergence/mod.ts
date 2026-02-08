/**
 * Divergence detection module - programmatic detection of known formatting divergences.
 *
 * Three main functions:
 * - `checkSafety()` - Compare character frequencies to detect data loss - BUGS
 * - `detectDivergences()` - Identify known pattern matches - INTENTIONAL DIFFERENCES
 * - `generateAuditReport()` - Cross-reference patterns against conformance_prettier.md
 */

export { checkSafety, type SafetyViolation } from './safety.ts';
export {
	detectDivergences,
	type DetectionContext,
	type DivergenceMatch,
	type DivergencePattern,
	type HunkCoverageResult,
	PATTERNS,
} from './patterns.ts';
export { type DiffHunk, extractHunks } from '../diff.ts';
export {
	type AuditReport,
	type DocumentedDivergence,
	formatAuditReport,
	generateAuditReport,
	loadDocumentedDivergences,
	parseConformancePrettierMd,
	type PatternCoverage,
} from './validation.ts';
