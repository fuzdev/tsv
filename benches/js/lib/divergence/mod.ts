/**
 * Divergence detection module - programmatic detection of known formatting divergences.
 *
 * Three main functions:
 * - `check_safety_vs_prettier()` - Differential data-loss check (ours beyond prettier) - BUGS
 * - `detect_divergences()` - Identify known pattern matches - INTENTIONAL DIFFERENCES
 * - `generate_audit_report()` - Cross-reference patterns against the `conformance_prettier*.md` family
 */

// Only the names the two consumers (corpus_compare_format.ts, divergence_audit.ts)
// actually import — everything else is reached from its concrete module directly.
export { check_safety_vs_prettier, type SafetyViolation } from './safety.ts';
export { detect_divergences, type HunkCoverageResult } from './patterns.ts';
export { check_expected_error } from './expected_errors.ts';
export { is_native_panic_error } from './panic_errors.ts';
export { format_audit_report, generate_audit_report } from './validation.ts';
