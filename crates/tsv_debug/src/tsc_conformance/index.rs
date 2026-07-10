//! The corpus-input self-check runner: index the tsc corpus, expand every test's
//! variants, and prove three invariants against the on-disk baselines.
//!
//! Zero checker code — this only relates the corpus inputs to the committed
//! baselines:
//!
//! 1. **Baseline join** — every on-disk baseline maps to exactly one non-skipped
//!    derived (test, variant); a baseline mapping to only skipped variants, to
//!    none, or ambiguously to several is a failure.
//! 2. **Unit-text round-trip** — for each baselined non-pretty test, the units the
//!    directive parser splits out reproduce (as a multiset) the baseline's `====`
//!    section bodies.
//! 3. **Denominators** — the sizing counts (scanned, per-extension, multi-file,
//!    JSX, JS-flavored, pretty, skip classes, variants, expect-clean) are the
//!    CLI's exact pins.
//!
//! The whole run also enforces that every `// @` directive in a non-skipped test
//! is recognized — an unknown directive is a hard harness failure and here a gate
//! failure (it means the ported option universe is incomplete).

use crate::tsc_conformance::corpus::{
    BasenameCollision, CorpusTest, basename_collisions, discover_corpus, read_corpus_file,
};
use crate::tsc_conformance::directives::{
    classify_units, extract_settings, harness_current_directory, section_display_name, split_units,
};
use crate::tsc_conformance::discovery::{Baseline, discover_baselines};
use crate::tsc_conformance::options_meta::{
    DEFAULT_USE_CASE_SENSITIVE_FILE_NAMES, HARNESS_FORCED_DEFAULTS, SKIPPED_TESTS,
    is_known_directive, strict_members, variant_is_unsupported,
};
use crate::tsc_conformance::roundtrip::is_pretty;
use crate::tsc_conformance::variants::{config_name, expand};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// One derived (test, variant) that produces a given baseline name.
struct Derived {
    /// Index into the per-test records.
    test_idx: usize,
    /// Whether the variant is skipped by the unsupported-option classes.
    skipped: bool,
}

/// Per-test record retained for the unit round-trip gate: the section-ordered
/// units — each a `(name, body-lines)` pair — that a baseline's `====` sections
/// must reproduce.
struct TestRecord {
    relative_path: String,
    unit_count: usize,
    /// Whether the test carried a tsconfig unit. Its `FileNames` glob resolution is
    /// out of scope, so the section order isn't authoritative; the gate falls back
    /// to a body multiset for these (name/order comparison deferred).
    tsconfig_unresolved: bool,
    /// `(unit name, body physical lines)` in baseline-section order
    /// (`Concatenate(tsConfigFiles, toBeCompiled, otherFiles)`).
    units: Vec<(String, Vec<String>)>,
}

/// A unit round-trip mismatch (a test whose split units don't reproduce its
/// baseline's section bodies).
#[derive(Debug, Clone, serde::Serialize)]
pub struct UnitMismatch {
    /// The baseline's `suite/name.errors.txt` path.
    pub baseline: String,
    /// The corpus test's relative path.
    pub test: String,
    /// A short reason.
    pub reason: String,
}

/// The `index` report: denominators plus the three gates' results.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexReport {
    // --- denominators ---
    /// Total corpus files scanned.
    pub total_scanned: usize,
    /// `.ts` files.
    pub ts_count: usize,
    /// `.tsx` files.
    pub tsx_count: usize,
    /// `.js` files.
    pub js_count: usize,
    /// Tests skipped by the static 45-entry list (test-level).
    pub skipped_tests: usize,
    /// Non-skipped tests with more than one unit.
    pub multi_file: usize,
    /// Non-skipped tests with a single unit.
    pub single_file: usize,
    /// Non-skipped tests that are JSX-scoped (`.tsx` / `@jsx` / under a `jsx/`
    /// path).
    pub jsx_scoped: usize,
    /// Non-skipped tests that are JS-flavored (`@checkJs` / `@allowJs` / `.js`).
    pub js_flavored: usize,
    /// Non-skipped tests carrying `@pretty`.
    pub pretty_tests: usize,
    /// Total variants across non-skipped tests (including variant-level skips).
    pub variant_total: usize,
    /// Variants skipped by the unsupported-option classes (variant-level).
    pub skipped_variants: usize,
    /// Non-skipped variants.
    pub nonskip_variants: usize,
    /// Non-skipped variants with no on-disk baseline (expect-clean).
    pub expect_clean: usize,
    /// `(suite, basename)` collisions across corpus nesting.
    pub basename_collisions: usize,
    /// The collisions themselves (for the report).
    pub collisions: Vec<BasenameCollision>,
    /// Tests whose variant product exceeded the cap (a harness failure).
    pub cap_exceeded: usize,
    /// varyBy include values with no normalized identity across all expanded tests
    /// (tsgo hard-fails on each; this harness keeps them as graceful `Other`
    /// variants). Pinned so a corpus pull can't slip in phantom variants unseen.
    pub unknown_includes: usize,
    /// Tests carrying a tsconfig/jsconfig unit (whose `FileNames` glob resolution
    /// is out of scope; their section split is by multiset, not order).
    pub tests_with_tsconfig: usize,

    // --- gate 1: baseline join ---
    /// Total on-disk baselines.
    pub baselines_total: usize,
    /// Baselines matched to exactly one non-skipped derived variant.
    pub join_matched: usize,
    /// On-disk baselines with no derived variant.
    pub join_unmatched: Vec<String>,
    /// On-disk baselines mapping only to skipped variants.
    pub join_skipped_with_baseline: Vec<String>,
    /// On-disk baselines mapping ambiguously to several non-skipped variants.
    pub join_ambiguous: Vec<String>,

    // --- gate 2: unit round-trip ---
    /// Non-pretty baselined tests whose units were checked.
    pub unit_roundtrip_checked: usize,
    /// Pretty baselines skipped by gate 2.
    pub unit_roundtrip_pretty_skipped: usize,
    /// Unit round-trip mismatches.
    pub unit_roundtrip_mismatches: Vec<UnitMismatch>,

    // --- directive universe ---
    /// Unknown `// @` directives in non-skipped tests (must be empty).
    pub unknown_directives: Vec<String>,

    // --- options-model substrate (reported, not gated) ---
    /// Number of harness-forced compiler defaults (distinct from compiler
    /// defaults).
    pub harness_forced_defaults: usize,
    /// Number of `strict`-family members (options that inherit `strict`).
    pub strict_family: usize,
    /// The harness default for case-sensitive file names.
    pub case_sensitive_default: bool,
}

/// Split an on-disk baseline's `suite/name.errors.txt` relative path into
/// `(suite, name)`.
fn split_baseline_key(relative_path: &str) -> Option<(&str, &str)> {
    relative_path.split_once('/')
}

/// Run the corpus-input index over a typescript-go checkout.
pub fn run_index(checkout: &Path) -> Result<IndexReport, String> {
    let corpus = discover_corpus(checkout)?;
    let baselines =
        discover_baselines(&crate::tsc_conformance::discovery::baselines_dir(checkout))?;
    let collisions = basename_collisions(&corpus);

    let mut report = IndexReport {
        total_scanned: corpus.len(),
        ts_count: 0,
        tsx_count: 0,
        js_count: 0,
        skipped_tests: 0,
        multi_file: 0,
        single_file: 0,
        jsx_scoped: 0,
        js_flavored: 0,
        pretty_tests: 0,
        variant_total: 0,
        skipped_variants: 0,
        nonskip_variants: 0,
        expect_clean: 0,
        basename_collisions: collisions.len(),
        collisions,
        cap_exceeded: 0,
        unknown_includes: 0,
        tests_with_tsconfig: 0,
        baselines_total: baselines.len(),
        join_matched: 0,
        join_unmatched: Vec::new(),
        join_skipped_with_baseline: Vec::new(),
        join_ambiguous: Vec::new(),
        unit_roundtrip_checked: 0,
        unit_roundtrip_pretty_skipped: 0,
        unit_roundtrip_mismatches: Vec::new(),
        unknown_directives: Vec::new(),
        harness_forced_defaults: HARNESS_FORCED_DEFAULTS.len(),
        strict_family: strict_members().len(),
        case_sensitive_default: DEFAULT_USE_CASE_SENSITIVE_FILE_NAMES,
    };

    let mut derived: HashMap<(&'static str, String), Vec<Derived>> = HashMap::new();
    let mut tests: Vec<TestRecord> = Vec::new();
    let mut unknown: BTreeMap<String, String> = BTreeMap::new();

    for test in &corpus {
        match test.extension.as_str() {
            "ts" => report.ts_count += 1,
            "tsx" => report.tsx_count += 1,
            "js" => report.js_count += 1,
            _ => {}
        }
        if SKIPPED_TESTS.contains(&test.basename.as_str()) {
            report.skipped_tests += 1;
            continue;
        }

        let content = read_corpus_file(&test.path)?;
        let settings = extract_settings(&content);

        for key in settings.keys() {
            if !is_known_directive(key) {
                unknown
                    .entry(key.clone())
                    .or_insert_with(|| test.relative_path.clone());
            }
        }

        let units = split_units(&content, &test.basename);
        let unit_count = units.len();
        if unit_count > 1 {
            report.multi_file += 1;
        } else {
            report.single_file += 1;
        }
        if is_jsx_scoped(test, &settings) {
            report.jsx_scoped += 1;
        }
        if is_js_flavored(test, &settings) {
            report.js_flavored += 1;
        }
        if settings.contains_key("pretty") {
            report.pretty_tests += 1;
        }

        let expansion = expand(&settings);
        if expansion.cap_exceeded {
            report.cap_exceeded += 1;
            continue;
        }
        report.unknown_includes += expansion.unknown_includes;

        // Order the units per the baseline `====` section order
        // (`Concatenate(tsConfigFiles, toBeCompiled, otherFiles)`), resolving each
        // unit's baseline display name so the round-trip can compare `(name, body)`
        // positionally.
        let current_directory = harness_current_directory(&settings);
        let classified = classify_units(units, &settings);
        if classified.tsconfig_unresolved {
            report.tests_with_tsconfig += 1;
        }
        let unit_pairs: Vec<(String, Vec<String>)> = classified
            .section_order()
            .iter()
            .map(|u| {
                (
                    section_display_name(&u.name, &current_directory),
                    split_content_lines(&u.content),
                )
            })
            .collect();

        let test_idx = tests.len();
        for variant in &expansion.variants {
            report.variant_total += 1;
            let skipped = variant_is_unsupported(&variant.config);
            if skipped {
                report.skipped_variants += 1;
            } else {
                report.nonskip_variants += 1;
            }
            let name = config_name(&test.basename, &variant.description);
            derived
                .entry((test.suite, name))
                .or_default()
                .push(Derived { test_idx, skipped });
        }
        tests.push(TestRecord {
            relative_path: test.relative_path.clone(),
            unit_count,
            tsconfig_unresolved: classified.tsconfig_unresolved,
            units: unit_pairs,
        });
    }

    report.unknown_directives = unknown.into_keys().collect();

    // --- Gate 1: baseline join ---
    let mut ondisk: HashMap<(&str, String), &Baseline> = HashMap::new();
    for baseline in &baselines {
        if let Some((suite, name)) = split_baseline_key(&baseline.relative_path) {
            ondisk.insert((suite, name.to_string()), baseline);
        }
    }

    for ((suite, name), baseline) in &ondisk {
        let key = (*suite, name.clone());
        match derived.get(&key) {
            None => report.join_unmatched.push(baseline.relative_path.clone()),
            Some(entries) => {
                let nonskip: Vec<&Derived> = entries.iter().filter(|d| !d.skipped).collect();
                if nonskip.is_empty() {
                    report
                        .join_skipped_with_baseline
                        .push(baseline.relative_path.clone());
                } else if nonskip.len() > 1 {
                    report.join_ambiguous.push(baseline.relative_path.clone());
                } else {
                    report.join_matched += 1;
                    // --- Gate 2: unit round-trip (non-pretty only) ---
                    check_unit_roundtrip(baseline, &tests[nonskip[0].test_idx], &mut report);
                }
            }
        }
    }

    // Expect-clean: every non-skipped derived name with no on-disk baseline.
    // `derived` is keyed by `(suite, name)`, so each iteration is a distinct name.
    for ((suite, name), entries) in &derived {
        if entries.iter().all(|d| d.skipped) {
            continue;
        }
        if !ondisk.contains_key(&(*suite, name.clone())) {
            report.expect_clean += 1;
        }
    }

    report.join_unmatched.sort();
    report.join_skipped_with_baseline.sort();
    report.join_ambiguous.sort();
    Ok(report)
}

/// Compare a test's split units to its baseline's `====` sections. For a test
/// without a tsconfig unit the comparison is **positional** — each section's
/// `(name, body)` must equal the derived unit at the same index, in
/// `Concatenate(toBeCompiled, otherFiles)` order — so section naming and ordering
/// are proven, not just body content. For a tsconfig test the `FileNames` glob
/// resolution is out of scope (the `toBeCompiled`/`otherFiles` split isn't
/// authoritative), so the comparison falls back to a body multiset. Pretty
/// baselines are counted and skipped (their body layout is a separate renderer
/// path).
fn check_unit_roundtrip(baseline: &Baseline, record: &TestRecord, report: &mut IndexReport) {
    let content = match std::fs::read_to_string(&baseline.path) {
        Ok(c) => c,
        Err(e) => {
            report.unit_roundtrip_mismatches.push(UnitMismatch {
                baseline: baseline.relative_path.clone(),
                test: record.relative_path.clone(),
                reason: format!("read error: {e}"),
            });
            return;
        }
    };
    if is_pretty(&content) {
        report.unit_roundtrip_pretty_skipped += 1;
        return;
    }
    let parsed = match crate::tsc_conformance::baseline::parse_baseline(&content) {
        Ok(p) => p,
        Err(reason) => {
            report.unit_roundtrip_mismatches.push(UnitMismatch {
                baseline: baseline.relative_path.clone(),
                test: record.relative_path.clone(),
                reason: format!("baseline parse: {reason}"),
            });
            return;
        }
    };

    report.unit_roundtrip_checked += 1;

    let reason = if record.tsconfig_unresolved {
        // Body multiset only (FileNames ordering deferred).
        let mut unit_lines: Vec<&Vec<String>> = record.units.iter().map(|(_, body)| body).collect();
        let mut section_lines: Vec<&Vec<String>> =
            parsed.sections.iter().map(|s| &s.src_lines).collect();
        unit_lines.sort();
        section_lines.sort();
        (unit_lines != section_lines).then(|| {
            format!(
                "unit/section body mismatch ({} units vs {} sections)",
                record.unit_count,
                parsed.sections.len()
            )
        })
    } else {
        // Positional `(name, body)` comparison.
        positional_mismatch(record, &parsed.sections)
    };

    if let Some(reason) = reason {
        report.unit_roundtrip_mismatches.push(UnitMismatch {
            baseline: baseline.relative_path.clone(),
            test: record.relative_path.clone(),
            reason,
        });
    }
}

/// Positionally compare a non-tsconfig test's section-ordered units against a
/// baseline's `====` sections; `None` on an exact match, else a short reason
/// naming the first divergence.
fn positional_mismatch(
    record: &TestRecord,
    sections: &[crate::tsc_conformance::baseline::Section],
) -> Option<String> {
    if record.units.len() != sections.len() {
        return Some(format!(
            "unit/section count mismatch ({} units vs {} sections)",
            record.units.len(),
            sections.len()
        ));
    }
    for (i, ((name, body), section)) in record.units.iter().zip(sections).enumerate() {
        if name != &section.name {
            return Some(format!(
                "section {i} name mismatch (unit {name:?} vs section {:?})",
                section.name
            ));
        }
        if body != &section.src_lines {
            return Some(format!("section {i} ({name:?}) body mismatch"));
        }
    }
    None
}

/// Split a unit's content into physical lines on `\r?\n` (the section-body
/// coordinate the baseline reprints).
fn split_content_lines(content: &str) -> Vec<String> {
    let bytes = content.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            let mut end = i;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            lines.push(content[start..end].to_string());
            start = i + 1;
        }
        i += 1;
    }
    lines.push(content[start..].to_string());
    lines
}

/// Whether a test is JSX-scoped: a `.tsx` file, an `@jsx` directive, or a path
/// under a `jsx/` directory. Shared with the skeleton runner's in-scope predicate.
#[must_use]
pub fn is_jsx_scoped(test: &CorpusTest, settings: &BTreeMap<String, String>) -> bool {
    test.extension == "tsx"
        || settings.contains_key("jsx")
        || test.relative_path.contains("/jsx/")
        || test.relative_path.starts_with("jsx/")
}

/// Whether a test is JS-flavored: `@checkJs` / `@allowJs`, or a `.js` file.
/// Shared with the skeleton runner's in-scope predicate.
#[must_use]
pub fn is_js_flavored(test: &CorpusTest, settings: &BTreeMap<String, String>) -> bool {
    settings.contains_key("checkjs") || settings.contains_key("allowjs") || test.extension == "js"
}

impl IndexReport {
    /// Print the human summary.
    pub fn print(&self, verbose: bool) {
        println!("tsc_conformance — corpus index");
        println!("==============================");
        println!("Total scanned:            {}", self.total_scanned);
        println!(
            "  .ts / .tsx / .js:       {} / {} / {}",
            self.ts_count, self.tsx_count, self.js_count
        );
        println!("Skipped tests (45-list):  {}", self.skipped_tests);
        println!(
            "Single-file / multi-file: {} / {}",
            self.single_file, self.multi_file
        );
        println!("JSX-scoped:               {}", self.jsx_scoped);
        println!("JS-flavored:              {}", self.js_flavored);
        println!("Pretty tests:             {}", self.pretty_tests);
        println!("Basename collisions:      {}", self.basename_collisions);
        println!("Cap-exceeded tests:       {}", self.cap_exceeded);
        println!("Unknown includes:         {}", self.unknown_includes);
        println!("Tests with tsconfig:      {}", self.tests_with_tsconfig);
        println!();
        println!("Variants (non-skipped tests): {}", self.variant_total);
        println!("  skipped (unsupported):      {}", self.skipped_variants);
        println!("  non-skipped:                {}", self.nonskip_variants);
        println!("    with baseline:            {}", self.join_matched);
        println!("    expect-clean:             {}", self.expect_clean);
        println!();
        println!("Gate 1 — baseline join");
        println!("  on-disk baselines:        {}", self.baselines_total);
        println!("  matched:                  {}", self.join_matched);
        println!("  unmatched:                {}", self.join_unmatched.len());
        println!(
            "  only-skipped w/ baseline: {}",
            self.join_skipped_with_baseline.len()
        );
        println!("  ambiguous:                {}", self.join_ambiguous.len());
        println!();
        println!("Gate 2 — unit-text round-trip");
        println!(
            "  checked (non-pretty):     {}",
            self.unit_roundtrip_checked
        );
        println!(
            "  pretty skipped:           {}",
            self.unit_roundtrip_pretty_skipped
        );
        println!(
            "  mismatches:               {}",
            self.unit_roundtrip_mismatches.len()
        );
        println!();
        println!(
            "Unknown directives:         {}",
            self.unknown_directives.len()
        );
        println!();
        println!(
            "Substrate: {} harness-forced defaults, {} strict-family members, case-sensitive={}",
            self.harness_forced_defaults, self.strict_family, self.case_sensitive_default
        );

        if verbose {
            for u in &self.join_unmatched {
                println!("  UNMATCHED {u}");
            }
            for u in &self.join_skipped_with_baseline {
                println!("  SKIPPED-WITH-BASELINE {u}");
            }
            for u in &self.join_ambiguous {
                println!("  AMBIGUOUS {u}");
            }
            for m in &self.unit_roundtrip_mismatches {
                println!("  UNIT-MISMATCH {} ({}) — {}", m.baseline, m.test, m.reason);
            }
            for d in &self.unknown_directives {
                println!("  UNKNOWN-DIRECTIVE {d}");
            }
            for c in &self.collisions {
                println!("  COLLISION {}/{} {:?}", c.suite, c.basename, c.paths);
            }
        }
    }
}
