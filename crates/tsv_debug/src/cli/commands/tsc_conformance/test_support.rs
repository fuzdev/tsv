//! Shared test-only fixtures for the tsc_conformance submodules. Both `pins`
//! (`measured_pins`) and `report` (the JSON/Markdown renderers) exercise the same
//! sample data, so it lives here once rather than as two copies that could
//! silently drift apart. Mirrors the `test_support` naming used elsewhere in the
//! workspace (e.g. `tsv_css::keyword_set::test_support`) for a test-only helper
//! shared across call sites.

use crate::tsc_conformance::MissingCause;
use crate::tsc_conformance::runner::SkeletonReport;

/// A report with the deterministic sections populated (per-code maps out of
/// natural order, a few counters, and a nonzero wall-clock).
pub(super) fn sample_report() -> SkeletonReport {
    SkeletonReport {
        in_scope_tests: 10,
        in_scope_variants: 12,
        expect_clean_graded: 4,
        clean_pass: 4,
        baselined_parsed: 8,
        family_graded_variants: 7,
        family_positive_variants: 3,
        family_match: 5,
        family_missing: 2,
        missing_by_cause: std::collections::BTreeMap::from([(MissingCause::Other, 2usize)]),
        family_extra: 0,
        related_match: 1,
        // Inserted out of ascending order — the BTreeMap and the render must sort.
        family_match_by_code: [(2528u32, 1usize), (2300, 2), (2451, 3)]
            .into_iter()
            .collect(),
        family_missing_by_code: [(2664u32, 1usize), (2451, 1)].into_iter().collect(),
        wall_ms: 123_456,
        ..SkeletonReport::default()
    }
}
