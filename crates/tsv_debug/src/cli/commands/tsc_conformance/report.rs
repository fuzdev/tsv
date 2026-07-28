//! The run's reporting artifacts: the `--emit-manifest` JSON, the committed
//! `--report` (JSON + Markdown), the failure-diff dump, and the generic JSON
//! printer every subcommand shares. Split out of `tsc_conformance.rs` for
//! navigability.

use crate::cli::CliError;
use crate::tsc_conformance::runner::SkeletonReport;
use crate::tsc_conformance::{MissingCause, RunFilter};
use std::path::{Path, PathBuf};
use tsv_cli::json_utils::to_json_with_tabs;

use super::pins::RunPins;

/// The active triage filters echoed into a filtered manifest, so a consumer sees
/// exactly which slice it was run over. Absent on a full (unfiltered) run.
#[derive(serde::Serialize)]
struct ManifestFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variant: Option<String>,
}

/// The `--emit-manifest` wrapper: the per-variant report, the pins snapshot, and a
/// `filtered` marker (plus the echoed filters). A triage-filtered manifest holds only
/// a partial slice of variant rows and its pins were NOT enforced, so the marker keeps
/// a consumer from mistaking it for a full-run one.
#[derive(serde::Serialize)]
struct RunManifest<'a> {
    filtered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    filters: Option<ManifestFilters>,
    pins: RunPins,
    report: &'a SkeletonReport,
}

/// Assemble the manifest wrapper, marking whether the run was triage-filtered and
/// echoing the active filters (`--variant key=value` re-joined for display).
fn run_manifest<'a>(
    report: &'a SkeletonReport,
    filter: &RunFilter,
    pins: &RunPins,
) -> RunManifest<'a> {
    let filtered = filter.is_active();
    let filters = filtered.then(|| ManifestFilters {
        test: filter.test.clone(),
        code: filter.code,
        variant: filter.variant.as_ref().map(|(k, v)| format!("{k}={v}")),
    });
    RunManifest {
        filtered,
        filters,
        pins: *pins,
        report,
    }
}

/// Write the `--emit-manifest` JSON (per-variant verdicts + buckets + census + pins +
/// the `filtered` marker). Called only after the gates pass, so a bad manifest never
/// lands.
pub(super) fn write_manifest(
    report: &SkeletonReport,
    filter: &RunFilter,
    pins: &RunPins,
    path: &Path,
) -> Result<(), CliError> {
    let manifest = run_manifest(report, filter, pins);
    let file = std::fs::File::create(path).map_err(|e| {
        eprintln!("Error creating manifest {}: {e}", path.display());
        CliError::Failed
    })?;
    serde_json::to_writer(std::io::BufWriter::new(file), &manifest).map_err(|e| {
        eprintln!("Error writing manifest: {e}");
        CliError::Failed
    })?;
    println!(
        "Wrote manifest ({} variant rows) to {}",
        report.manifest_entries.len(),
        path.display()
    );
    Ok(())
}

/// Build the committed compact report as a JSON value — deterministic (sorted
/// per-code maps, wall-clock excluded) so re-runs are diff-clean.
fn build_report_value(report: &SkeletonReport, pins: &RunPins) -> serde_json::Value {
    serde_json::json!({
        "oracle": "tsgo committed .errors.txt baselines (bind + merge + flow family)",
        "denominators": {
            "in_scope_tests": report.in_scope_tests,
            "in_scope_variants": report.in_scope_variants,
            "expect_clean_graded": report.expect_clean_graded,
            "clean_pass": report.clean_pass,
            "baselined_parsed": report.baselined_parsed,
            "family_graded_variants": report.family_graded_variants,
            "family_positive_variants": report.family_positive_variants,
        },
        "family": {
            "match": report.family_match,
            "dup_match": report.dup_match(),
            "flow_match": report.flow_match(),
            "missing": {
                "total": report.family_missing,
                "dup": report.dup_missing(),
                "flow": report.flow_missing(),
                "merge_path": report.missing(MissingCause::Merge),
                "lib_conflict": report.missing(MissingCause::Lib),
                "late_bound": report.missing(MissingCause::DeferredLateBound),
                "cfa": report.missing(MissingCause::DeferredCfa),
                "other": report.missing(MissingCause::Other),
            },
            "extra": report.family_extra,
            "span_mismatch": report.family_span_mismatch,
        },
        "per_code": {
            "match": report.family_match_by_code,
            "missing": report.family_missing_by_code,
        },
        "related": {
            "match": report.related_match,
            "missing": report.related_missing,
            "extra": report.related_extra,
            "span_mismatch": report.related_span_mismatch,
        },
        "carve_outs": {
            "recovery_ast_rule_a": report.carve_out_rule_a,
            "recovery_ast_rule_a_family": report.carve_out_rule_a_family,
            "module_detection_variants": report.module_detection_variants,
        },
        "census": {
            "parse_rejected_total": report.parse_rejected_total,
            "parse_rejected_no_baseline": report.parse_rejected_no_baseline,
            "parse_rejected_ts1xxx_only": report.parse_rejected_ts1xxx_only,
            "parse_rejected_other": report.parse_rejected_other,
            "script_retry": report.script_retry,
            "crash_excluded": report.excluded_crashes,
        },
        "lib": {
            "files_bound": report.lib_files_bound,
            "sets_folded": report.lib_sets_built,
        },
        "pins": pins,
    })
}

/// Render the committed report's compact Markdown (the same deterministic data as
/// [`build_report_value`], for readers).
fn render_report_md(report: &SkeletonReport) -> String {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;
    let mut s = String::new();
    s.push_str("# tsc_conformance run — committed report\n\n");
    s.push_str(
        "Oracle: tsgo committed `.errors.txt` baselines (bind + merge + flow family). \
         Deterministic — wall-clock excluded.\n\n",
    );

    s.push_str("## Denominators\n\n");
    let _ = writeln!(s, "- in-scope tests: {}", report.in_scope_tests);
    let _ = writeln!(s, "- in-scope variants: {}", report.in_scope_variants);
    let _ = writeln!(
        s,
        "- expect-clean graded / clean pass: {} / {}",
        report.expect_clean_graded, report.clean_pass
    );
    let _ = writeln!(s, "- baselined + parsed: {}", report.baselined_parsed);
    let _ = writeln!(
        s,
        "- family graded / family-positive: {} / {}\n",
        report.family_graded_variants, report.family_positive_variants
    );

    s.push_str(
        "## Family (dup 2300 / 2451 / 2567 / 2528 + merge 2397 / 2649 / 2664 / 2671; \
         flow 7027 / 7028)\n\n",
    );
    let _ = writeln!(
        s,
        "- match: {} (dup {}, flow {})",
        report.family_match,
        report.dup_match(),
        report.flow_match()
    );
    let _ = writeln!(
        s,
        "- missing: {} (dup {}, flow {})",
        report.family_missing,
        report.dup_missing(),
        report.flow_missing()
    );
    let _ = writeln!(
        s,
        "  - by cause: merge-path {}, lib-conflict {}, late-bound {}, cfa {}, other {}",
        report.missing(MissingCause::Merge),
        report.missing(MissingCause::Lib),
        report.missing(MissingCause::DeferredLateBound),
        report.missing(MissingCause::DeferredCfa),
        report.missing(MissingCause::Other)
    );
    let _ = writeln!(s, "- extra (GATE=0): {}", report.family_extra);
    let _ = writeln!(s, "- span mismatch: {}\n", report.family_span_mismatch);

    s.push_str("## Per-code table\n\n");
    s.push_str("| code | match | missing |\n| --- | --- | --- |\n");
    let codes: BTreeSet<u32> = report
        .family_match_by_code
        .keys()
        .chain(report.family_missing_by_code.keys())
        .copied()
        .collect();
    for code in codes {
        let m = report.family_match_by_code.get(&code).copied().unwrap_or(0);
        let miss = report
            .family_missing_by_code
            .get(&code)
            .copied()
            .unwrap_or(0);
        let _ = writeln!(s, "| TS{code} | {m} | {miss} |");
    }
    s.push('\n');

    s.push_str("## Related-info channel (matched primaries)\n\n");
    let _ = writeln!(
        s,
        "- match / missing / extra / span-mismatch: {} / {} / {} / {}\n",
        report.related_match,
        report.related_missing,
        report.related_extra,
        report.related_span_mismatch
    );

    s.push_str("## Carve-outs\n\n");
    let _ = writeln!(
        s,
        "- recovery-AST rule (a): {} (family-positive {})",
        report.carve_out_rule_a, report.carve_out_rule_a_family
    );
    let _ = writeln!(
        s,
        "- moduleDetection variants (inert for family): {}\n",
        report.module_detection_variants
    );

    s.push_str("## Parse-divergence census\n\n");
    let _ = writeln!(
        s,
        "- parse-rejected: {} (no baseline {}, TS1xxx-only {}, other {})",
        report.parse_rejected_total,
        report.parse_rejected_no_baseline,
        report.parse_rejected_ts1xxx_only,
        report.parse_rejected_other
    );
    let _ = writeln!(s, "- script-goal retries: {}", report.script_retry);
    let _ = writeln!(
        s,
        "- crash-excluded (tracked): {}\n",
        report.excluded_crashes
    );

    s.push_str("## Lib base\n\n");
    let _ = writeln!(
        s,
        "- lib files bound / sets folded: {} / {}",
        report.lib_files_bound, report.lib_sets_built
    );

    s
}

/// Whether a run may write the committed `--report` artifact: it is a full-run
/// product (the pins hold only on a full run), so a filtered (triage) run must
/// refuse it. A run that didn't request `--report` trivially may.
pub(super) fn may_write_report(report_requested: bool, filtered: bool) -> bool {
    !(report_requested && filtered)
}

/// Write the committed compact report to `<base>.json` + `<base>.md` (full runs only;
/// deterministic). Called only after the gates pass.
pub(super) fn write_report(
    report: &SkeletonReport,
    pins: &RunPins,
    base: &Path,
) -> Result<(), CliError> {
    let json_path = PathBuf::from(format!("{}.json", base.display()));
    let md_path = PathBuf::from(format!("{}.md", base.display()));
    let value = build_report_value(report, pins);
    let mut json = to_json_with_tabs(&value).map_err(|e| {
        eprintln!("Error serializing report JSON: {e}");
        CliError::Failed
    })?;
    json.push('\n');
    std::fs::write(&json_path, json).map_err(|e| {
        eprintln!("Error writing {}: {e}", json_path.display());
        CliError::Failed
    })?;
    std::fs::write(&md_path, render_report_md(report)).map_err(|e| {
        eprintln!("Error writing {}: {e}", md_path.display());
        CliError::Failed
    })?;
    println!(
        "Wrote committed report to {} + {}",
        json_path.display(),
        md_path.display()
    );
    Ok(())
}

/// Dump each failing variant's ours-vs-baseline diff under
/// `target/tsc_conformance/diffs/` (a regression aid; a no-op when the run is green).
pub(super) fn write_diff_artifacts(report: &SkeletonReport) {
    if report.failing_variants.is_empty() {
        return;
    }
    let dir = Path::new("target/tsc_conformance/diffs");
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("  (could not create {}: {e})", dir.display());
        return;
    }
    eprintln!(
        "\nWrote {} failure diff artifact(s) under {}/:",
        report.failing_variants.len(),
        dir.display()
    );
    for fv in &report.failing_variants {
        let path = dir.join(format!(
            "{}__{}.diff",
            fv.suite,
            sanitize_artifact_name(&fv.config)
        ));
        match std::fs::write(&path, &fv.diff) {
            Ok(()) => eprintln!("  {} ({})", path.display(), fv.reason),
            Err(e) => eprintln!("  (failed to write {}: {e})", path.display()),
        }
    }
}

/// Replace path-hostile characters so a baseline identity is a safe artifact basename.
fn sanitize_artifact_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_whitespace() {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// Serialize a report to pretty JSON on stdout, tab-indented like every other
/// `tsv_debug` `--json` surface.
pub(super) fn print_json<T: serde::Serialize>(report: &T) -> Result<(), CliError> {
    match to_json_with_tabs(report) {
        Ok(json) => {
            println!("{json}");
            Ok(())
        }
        Err(e) => {
            eprintln!("Error serializing JSON: {e}");
            Err(CliError::Failed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::pins::measured_pins;
    use super::super::test_support::sample_report;
    use super::*;

    #[test]
    fn report_json_is_deterministic() {
        // Two builds from the same report serialize byte-for-byte identically (sorted
        // maps, no timing) — the committed artifact must be diff-clean across re-runs.
        let r = sample_report();
        let pins = measured_pins(&r);
        let a = serde_json::to_string_pretty(&build_report_value(&r, &pins)).unwrap();
        let b = serde_json::to_string_pretty(&build_report_value(&r, &pins)).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn report_md_is_deterministic() {
        let r = sample_report();
        assert_eq!(render_report_md(&r), render_report_md(&r));
    }

    #[test]
    fn per_code_table_iterates_sorted() {
        // The per-code table lists codes ascending regardless of map insertion order.
        let md = render_report_md(&sample_report());
        let p2300 = md.find("TS2300").expect("TS2300 row");
        let p2451 = md.find("TS2451").expect("TS2451 row");
        let p2528 = md.find("TS2528").expect("TS2528 row");
        assert!(
            p2300 < p2451 && p2451 < p2528,
            "per-code rows not ascending"
        );
    }

    #[test]
    fn wall_clock_excluded_from_report() {
        // Two reports differing ONLY in wall-clock produce identical JSON and Markdown —
        // machine-varying timing never reaches the committed content.
        let mut fast = sample_report();
        fast.wall_ms = 0;
        let mut slow = sample_report();
        slow.wall_ms = 987_654_321;
        assert_eq!(
            serde_json::to_string_pretty(&build_report_value(&fast, &measured_pins(&fast)))
                .unwrap(),
            serde_json::to_string_pretty(&build_report_value(&slow, &measured_pins(&slow)))
                .unwrap(),
        );
        assert_eq!(render_report_md(&fast), render_report_md(&slow));
    }

    #[test]
    fn sanitize_replaces_path_separators() {
        // Both slash flavors become `_` so a `suite/config` identity is one path segment.
        assert_eq!(sanitize_artifact_name("a/b\\c"), "a_b_c");
    }

    #[test]
    fn sanitize_replaces_whitespace() {
        assert_eq!(sanitize_artifact_name("a b\tc"), "a_b_c");
    }

    #[test]
    fn sanitize_preserves_baseline_identity() {
        // A real baseline name has no path-hostile chars; parens/`=`/`.` pass through.
        let name = "duplicateVar(target=es2015).errors.txt";
        assert_eq!(sanitize_artifact_name(name), name);
    }

    #[test]
    fn sanitize_is_not_injective() {
        // The mapping has no collision handling by design: distinct inputs whose only
        // difference is a slash-vs-space collapse to the same basename. Pins current
        // behavior (do not add collision handling here).
        assert_eq!(sanitize_artifact_name("a/b"), sanitize_artifact_name("a b"));
    }

    #[test]
    fn report_write_requires_full_run() {
        // A committed report is a full-run product: allowed unless requested on a
        // filtered run.
        assert!(may_write_report(false, false));
        assert!(may_write_report(false, true));
        assert!(may_write_report(true, false));
        assert!(!may_write_report(true, true));
    }

    #[test]
    fn manifest_marks_and_echoes_filtered_runs() {
        // A full (unfiltered) run: `filtered` is false and no `filters` object is
        // emitted — nothing distinguishes it from a plain full-run manifest.
        let r = sample_report();
        let pins = measured_pins(&r);
        let full = serde_json::to_value(run_manifest(&r, &RunFilter::default(), &pins)).unwrap();
        assert_eq!(full["filtered"], serde_json::json!(false));
        assert!(full.get("filters").is_none());

        // A triage-filtered run: `filtered` is true and the active filters are echoed
        // (an unset filter — here `--test` — is omitted; `--variant` is re-joined).
        let filter = RunFilter {
            test: None,
            code: Some(2300),
            variant: Some(("target".to_string(), "es2015".to_string())),
            family: None,
        };
        let filtered = serde_json::to_value(run_manifest(&r, &filter, &pins)).unwrap();
        assert_eq!(filtered["filtered"], serde_json::json!(true));
        assert_eq!(filtered["filters"]["code"], serde_json::json!(2300));
        assert_eq!(
            filtered["filters"]["variant"],
            serde_json::json!("target=es2015")
        );
        assert!(filtered["filters"].get("test").is_none());
    }
}
