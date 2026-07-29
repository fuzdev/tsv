use super::*;

/// The skeleton sweep report.
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct SkeletonReport {
    /// Tests that passed the test-level in-scope filter and have >=1 in-scope
    /// variant.
    pub in_scope_tests: usize,
    /// In-scope variants graded (parsed or parse-rejected).
    pub in_scope_variants: usize,
    /// In-scope variants that parsed and have no on-disk baseline (expect-clean).
    pub expect_clean_graded: usize,
    /// Expect-clean variants that graded clean (zero diagnostics). Gate: must
    /// equal `expect_clean_graded`.
    pub clean_pass: usize,
    /// Expect-clean variants that graded non-clean (gate failure list).
    pub clean_fail: Vec<CleanFail>,
    /// In-scope variants that parsed and DO have a baseline.
    pub baselined_parsed: usize,

    // --- family grading ---
    /// Parsed-with-baseline variants family-graded (not carved by predicate v1).
    pub family_graded_variants: usize,
    /// ...of those, whose baseline carries at least one family code.
    pub family_positive_variants: usize,
    /// Family diagnostics that matched (file, line, col, code).
    pub family_match: usize,
    /// Family baseline diagnostics with no matching diagnostic of ours (classified
    /// below). Expected to be all merge/lib until S4/S5 land.
    pub family_missing: usize,
    /// Family diagnostics we emit that the baseline lacks. **Gate: must be 0.**
    pub family_extra: usize,
    /// Right code + file, wrong position (greedy-paired).
    pub family_span_mismatch: usize,

    // --- related-info grading (its own pinned channel; does NOT gate the
    // per-variant primary verdict) — graded only for matched primaries ---
    /// Related-info entries that matched (code, file, line, col).
    pub related_match: usize,
    /// Baseline related entries with no matching related of ours.
    pub related_missing: usize,
    /// Related entries we emit the baseline lacks.
    pub related_extra: usize,
    /// Right code + file, wrong position (greedy-paired).
    pub related_span_mismatch: usize,
    /// Sample related over-emissions.
    pub related_extra_samples: Vec<String>,
    /// Sample related misses.
    pub related_missing_samples: Vec<String>,

    /// ...missing, tallied by classified cause (see [`MissingCause`]; keyed so
    /// new causes are a variant + a pin row, not a new field). Read via
    /// [`SkeletonReport::missing`]. [`MissingCause::Other`] **gates at 0**.
    pub missing_by_cause: BTreeMap<MissingCause, usize>,
    /// Variants carved out by predicate v1 rule (a): tsv parses clean but the
    /// baseline carries a non-bind TS1xxx code (recovery-AST incomparability).
    pub carve_out_rule_a: usize,
    /// ...of those, whose baseline also carries a family code.
    pub carve_out_rule_a_family: usize,
    /// In-scope variants that set `moduleDetection` (a watch item — module-ness is
    /// inert for the family cascade, so the parse-once shortcut stays valid).
    pub module_detection_variants: usize,
    /// Sample extra diagnostics (gate failures to fix).
    pub extra_samples: Vec<String>,
    /// Sample unattributed misses (candidate cascade bugs).
    pub missing_other_samples: Vec<String>,
    /// Sample span mismatches.
    pub span_mismatch_samples: Vec<String>,
    /// In-scope variants tsv parse-rejected (census; informational).
    pub parse_rejected_total: usize,
    /// ...of those, with no on-disk baseline (a likely tsv over-rejection).
    pub parse_rejected_no_baseline: usize,
    /// ...with a TS1xxx-only baseline (ambiguous: tsgo parse error or grammar).
    pub parse_rejected_ts1xxx_only: usize,
    /// ...with a baseline carrying non-TS1xxx codes (tsv rejects what tsgo checked).
    pub parse_rejected_other: usize,
    /// In-scope parsed variants that needed the `Goal::Script` retry (census).
    pub script_retry: usize,
    /// Tests whose check panicked (caught) and are NOT crash-excluded. Gate:
    /// must be empty.
    pub panics: Vec<PanicRecord>,
    /// Tests skipped by the crash-exclusion ledger (tracked parser aborts/panics).
    pub excluded_crashes: usize,

    // --- lib base ---
    /// Distinct lib `.d.ts` files parsed + bound this run (informational).
    pub lib_files_bound: usize,
    /// Distinct resolved lib sets folded into a base this run (informational).
    pub lib_sets_built: usize,
    /// Lib files that failed to parse (`file: error`). **Gate: must be empty.**
    pub lib_parse_errors: Vec<String>,
    /// Referenced lib files not found on disk. **Gate: must be empty.**
    pub lib_missing_files: Vec<String>,
    /// Unrecognized `@lib` / `/// <reference lib>` names. **Gate: must be empty.**
    pub lib_unknown_names: Vec<String>,
    /// Lib files that bound as an external module with no `declare global {}` block —
    /// their globals would silently fold to nothing. **Gate: must be empty.**
    pub lib_external_no_globals: Vec<String>,
    /// Catchable-panic exclusions that no longer panic (a fix landed) — the entry
    /// is stale and must be dropped. **Gate: must be empty.**
    pub stale_exclusions: Vec<String>,
    /// Total bound nodes across in-scope tests (informational).
    pub total_nodes: u64,
    /// Wall-clock of the sweep in milliseconds (EXCLUDED from the committed report —
    /// machine-varying).
    pub wall_ms: u128,

    // --- deterministic per-code breakdown (the committed report's per-code table) ---
    /// Family diagnostics that matched, keyed by TS code (sorted for determinism).
    pub family_match_by_code: BTreeMap<u32, usize>,
    /// Family baseline diagnostics with no match, keyed by TS code (sorted).
    pub family_missing_by_code: BTreeMap<u32, usize>,

    // --- optional artifacts (empty on a normal green run) ---
    /// Per-variant verdict rows for `--emit-manifest` (empty unless requested).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manifest_entries: Vec<ManifestEntry>,
    /// Failing variants with a pre-rendered diff — written to `.diff` artifacts when
    /// the gates fail (empty when green).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failing_variants: Vec<FailingVariant>,
}

/// Sum a per-code map's entries whose code is in `codes` (the sub-family partition
/// behind `dup_*` / `flow_*`).
fn sub_family_sum(by_code: &BTreeMap<u32, usize>, codes: &[u32]) -> usize {
    by_code
        .iter()
        .filter(|(code, _)| codes.contains(code))
        .map(|(_, count)| *count)
        .sum()
}

impl SkeletonReport {
    /// A family's matches (partition of `family_match_by_code` by its code set).
    #[must_use]
    pub fn family_match_for(&self, family: &GradedFamily) -> usize {
        sub_family_sum(&self.family_match_by_code, family.codes)
    }

    /// A family's misses (partition of `family_missing_by_code` by its code set).
    #[must_use]
    pub fn family_missing_for(&self, family: &GradedFamily) -> usize {
        sub_family_sum(&self.family_missing_by_code, family.codes)
    }

    /// The missing count attributed to `cause` (0 when the cause never fired).
    #[must_use]
    pub fn missing(&self, cause: MissingCause) -> usize {
        self.missing_by_cause.get(&cause).copied().unwrap_or(0)
    }

    /// The duplicate/conflict sub-family matches (partition of `family_match_by_code`).
    #[must_use]
    pub fn dup_match(&self) -> usize {
        self.family_match_for(&FAMILIES[0])
    }

    /// The flow sub-family matches (partition of `family_match_by_code`).
    #[must_use]
    pub fn flow_match(&self) -> usize {
        self.family_match_for(&FAMILIES[1])
    }

    /// The duplicate/conflict sub-family misses (partition of `family_missing_by_code`).
    #[must_use]
    pub fn dup_missing(&self) -> usize {
        self.family_missing_for(&FAMILIES[0])
    }

    /// The flow sub-family misses (partition of `family_missing_by_code`).
    #[must_use]
    pub fn flow_missing(&self) -> usize {
        self.family_missing_for(&FAMILIES[1])
    }

    /// Print the human summary.
    pub fn print(&self) {
        println!("tsc_conformance run");
        println!("===================");
        println!("In-scope tests:            {}", self.in_scope_tests);
        println!("In-scope variants:         {}", self.in_scope_variants);
        println!("  parsed, expect-clean:    {}", self.expect_clean_graded);
        println!("    graded clean:          {}", self.clean_pass);
        println!("    graded NON-clean:      {}", self.clean_fail.len());
        println!("  parsed, baselined:       {}", self.baselined_parsed);
        println!("  parse-rejected:          {}", self.parse_rejected_total);
        println!(
            "    no baseline:           {}",
            self.parse_rejected_no_baseline
        );
        println!(
            "    TS1xxx-only baseline:  {}",
            self.parse_rejected_ts1xxx_only
        );
        println!("    other baseline:        {}", self.parse_rejected_other);
        println!("Script-goal retries:       {}", self.script_retry);
        println!("Bound nodes (total):       {}", self.total_nodes);
        println!();
        println!(
            "Family grading (dup 2300/2451/2567/2528 + merge 2397/2649/2664/2671; flow 7027/7028)"
        );
        println!("---------------------------------------------------------------");
        println!("Graded variants:           {}", self.family_graded_variants);
        println!(
            "  ...family-positive:      {}",
            self.family_positive_variants
        );
        let per_family = |get: &dyn Fn(&GradedFamily) -> usize| -> String {
            FAMILIES
                .iter()
                .map(|f| format!("{} {}", f.key, get(f)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!(
            "  match:                   {} ({})",
            self.family_match,
            per_family(&|f| self.family_match_for(f))
        );
        println!(
            "  missing:                 {} ({})",
            self.family_missing,
            per_family(&|f| self.family_missing_for(f))
        );
        // One line per classified cause (label-aligned; `Other` is the gate).
        const CAUSE_LINES: [(MissingCause, &str, &str); 5] = [
            (MissingCause::Merge, "    merge-path:            ", ""),
            (MissingCause::Lib, "    lib-conflict:          ", ""),
            (
                MissingCause::DeferredLateBound,
                "    late-bound (deferred): ",
                " (needs the type engine — literal-type / unique-symbol computed member names)",
            ),
            (
                MissingCause::DeferredCfa,
                "    cfa (deferred):        ",
                " (needs the CFA type engine — never-returning sigs / assertion predicates / switch exhaustiveness / structural reachability)",
            ),
            (
                MissingCause::Other,
                "    other (GATE=0):        ",
                " (unclassified family miss — a same-table cascade bug)",
            ),
        ];
        for (cause, label, note) in CAUSE_LINES {
            println!("{label}{}{note}", self.missing(cause));
        }
        println!("  extra (GATE=0):          {}", self.family_extra);
        println!("  span_mismatch:           {}", self.family_span_mismatch);
        println!("Related-info (matched primaries; own channel, non-gating)");
        println!("  related match:           {}", self.related_match);
        println!("  related missing:         {}", self.related_missing);
        println!("  related extra:           {}", self.related_extra);
        println!("  related span_mismatch:   {}", self.related_span_mismatch);
        for s in &self.related_missing_samples {
            println!("  REL-MISSING {s}");
        }
        for s in &self.related_extra_samples {
            println!("  REL-EXTRA {s}");
        }
        println!("Carve-out rule (a):        {}", self.carve_out_rule_a);
        println!(
            "  ...family-positive:      {}",
            self.carve_out_rule_a_family
        );
        println!(
            "moduleDetection variants:  {} (watch; inert for family)",
            self.module_detection_variants
        );
        for s in &self.extra_samples {
            println!("  EXTRA {s}");
        }
        for s in &self.missing_other_samples {
            println!("  MISSING-OTHER {s}");
        }
        for s in &self.span_mismatch_samples {
            println!("  SPAN {s}");
        }
        println!();
        println!("Lib base");
        println!("  lib files bound:         {}", self.lib_files_bound);
        println!("  lib sets folded:         {}", self.lib_sets_built);
        println!(
            "  lib parse errors:        {} (GATE=0)",
            self.lib_parse_errors.len()
        );
        println!(
            "  lib missing files:       {} (GATE=0)",
            self.lib_missing_files.len()
        );
        println!(
            "  lib unknown names:       {} (GATE=0)",
            self.lib_unknown_names.len()
        );
        println!(
            "  lib external no-globals: {} (GATE=0)",
            self.lib_external_no_globals.len()
        );
        for e in &self.lib_parse_errors {
            println!("  LIB-PARSE-ERR {e}");
        }
        for f in &self.lib_missing_files {
            println!("  LIB-MISSING {f}");
        }
        for n in &self.lib_unknown_names {
            println!("  LIB-UNKNOWN {n}");
        }
        for f in &self.lib_external_no_globals {
            println!("  LIB-EXT-NO-GLOBALS {f}");
        }
        println!();
        println!("Panics (caught):           {}", self.panics.len());
        println!("Crash-excluded (tracked):  {}", self.excluded_crashes);
        if !self.stale_exclusions.is_empty() {
            println!(
                "Stale crash-exclusions:    {} (drop them)",
                self.stale_exclusions.len()
            );
        }
        println!("Wall-clock:                {} ms", self.wall_ms);
        if !self.clean_fail.is_empty() {
            println!();
            for f in &self.clean_fail {
                println!("  CLEAN-FAIL {} ({} diagnostics)", f.variant, f.diagnostics);
            }
        }
        for p in &self.panics {
            println!("  PANIC {} — {}", p.test, p.payload);
        }
    }
}

// ===========================================================================
// check-test: the inner dev loop over one test.
// ===========================================================================

/// One diagnostic line (ours or the baseline's) for the check-test diff.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct DiagLine {
    /// The file the diagnostic points at (or `null` for a global one). A lib file
    /// (`lib.es5.d.ts`) with `null` line/col is a masked lib-sourced entry.
    pub file: Option<String>,
    /// 1-based line (`null` for a global or masked-lib diagnostic).
    pub line: Option<u32>,
    /// 1-based column (`null` for a global or masked-lib diagnostic).
    pub col: Option<u32>,
    /// The `TS<code>` number.
    pub code: u32,
    /// The diagnostic's related-info entries (empty for a baseline summary line).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<DiagLine>,
}

/// The `check-test` report for one test/variant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckTestReport {
    /// The corpus test's relative path.
    pub test: String,
    /// The suite (`compiler` / `conformance`).
    pub suite: String,
    /// The variant description, or `(default)`.
    pub variant: String,
    /// The joined baseline name, or `None` when the variant is expect-clean.
    pub baseline: Option<String>,
    /// Whether the variant is expect-clean (no on-disk baseline).
    pub expect_clean: bool,
    /// Whether tsv parse-rejected the program.
    pub parse_rejected: bool,
    /// The parse error message, when rejected.
    pub parse_error: Option<String>,
    /// Our diagnostics (empty while the checker is a no-op).
    pub ours: Vec<DiagLine>,
    /// The baseline's summary-block diagnostics (the expected set).
    pub baseline_summary: Vec<DiagLine>,
}

impl CheckTestReport {
    /// Print the human diff (ours vs the baseline summary).
    pub fn print(&self) {
        println!(
            "check-test: {}/{}  variant={}",
            self.suite, self.test, self.variant
        );
        if self.parse_rejected {
            println!(
                "  tsv PARSE-REJECTED: {}",
                self.parse_error.as_deref().unwrap_or("(no message)")
            );
        }
        if self.expect_clean {
            println!("  baseline: (none — expect-clean)");
        } else {
            println!("  baseline: {}", self.baseline.as_deref().unwrap_or("?"));
        }
        println!();
        println!("  ours ({}):", self.ours.len());
        for d in &self.ours {
            println!("    {}", fmt_diag(d));
            for r in &d.related {
                println!("      related {}", fmt_diag(r));
            }
        }
        if self.ours.is_empty() {
            println!("    (none)");
        }
        println!("  baseline ({}):", self.baseline_summary.len());
        for d in &self.baseline_summary {
            println!("    {}", fmt_diag(d));
        }
        if self.baseline_summary.is_empty() {
            println!("    (none)");
        }
    }
}

/// Format one diagnostic line for the human diff.
fn fmt_diag(d: &DiagLine) -> String {
    match (&d.file, d.line, d.col) {
        (Some(file), Some(line), Some(col)) => format!("{file}({line},{col}): TS{}", d.code),
        // A masked lib entry (file, no location) or a global one.
        (Some(file), _, _) => format!("{file}(--,--): TS{}", d.code),
        _ => format!("error TS{} (global)", d.code),
    }
}
