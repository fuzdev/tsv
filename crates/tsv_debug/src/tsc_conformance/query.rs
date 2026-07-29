//! Aggregations over the discovered baselines — the three `query` subqueries.
//!
//! Each function reads every baseline once, parses its summary block
//! ([`super::baseline::parse_summary_block`]), and folds the diagnostics into a
//! serializable report. The reports own their human-readable `print_*` rendering
//! so the command file stays thin.

use super::baseline::parse_summary_block;
use super::discovery::Baseline;
use std::collections::HashMap;

/// TS4xxx codes are the declaration-emit family.
const DECLARATION_EMIT_RANGE: std::ops::RangeInclusive<u32> = 4000..=4999;

/// Read a baseline's content, warning and skipping on a read error (the files
/// were just discovered, so this is essentially never hit).
fn read_baseline(baseline: &Baseline) -> Option<String> {
    match std::fs::read_to_string(&baseline.path) {
        Ok(content) => Some(content),
        Err(e) => {
            eprintln!("warning: could not read {}: {e}", baseline.path.display());
            None
        }
    }
}

/// `count / total * 100`, guarding division by zero.
#[allow(clippy::cast_precision_loss)] // diagnostic counts stay well within f64 precision
fn pct(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (count as f64 / total as f64) * 100.0
    }
}

// ---------------------------------------------------------------------------
// histogram
// ---------------------------------------------------------------------------

/// One code's row in the histogram.
#[derive(Debug, serde::Serialize)]
pub struct CodeCount {
    /// The `TS<code>` number.
    pub code: u32,
    /// Instance count across all baselines' summary blocks.
    pub count: usize,
    /// `count` as a percentage of all instances.
    pub pct: f64,
    /// Running percentage through this row (codes sorted by descending count).
    pub cumulative_pct: f64,
}

/// The `histogram` subquery report: per-code instance counts plus totals.
#[derive(Debug, serde::Serialize)]
pub struct HistogramReport {
    /// Number of `.errors.txt` files scanned.
    pub files_scanned: usize,
    /// Total diagnostic instances (summary lines) across all files.
    pub total_instances: usize,
    /// Instances carrying a `(line,col)` prefix.
    pub positional_instances: usize,
    /// Instances with no location (global / fileless).
    pub global_instances: usize,
    /// Number of distinct `TS<code>` values.
    pub distinct_codes: usize,
    /// Per-code rows, sorted by descending count (ties broken by ascending code).
    pub codes: Vec<CodeCount>,
}

/// Build the error-code instance histogram over every baseline.
pub fn histogram(baselines: &[Baseline]) -> HistogramReport {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    let mut total = 0usize;
    let mut positional = 0usize;
    let mut global = 0usize;

    for baseline in baselines {
        let Some(content) = read_baseline(baseline) else {
            continue;
        };
        for diag in parse_summary_block(&content) {
            *counts.entry(diag.code).or_insert(0) += 1;
            total += 1;
            if diag.file.is_some() {
                positional += 1;
            } else {
                global += 1;
            }
        }
    }

    // Descending by count, then ascending by code for a stable order.
    let mut ranked: Vec<(u32, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let distinct_codes = ranked.len();

    let mut cumulative = 0usize;
    let codes = ranked
        .into_iter()
        .map(|(code, count)| {
            cumulative += count;
            CodeCount {
                code,
                count,
                pct: pct(count, total),
                cumulative_pct: pct(cumulative, total),
            }
        })
        .collect();

    HistogramReport {
        files_scanned: baselines.len(),
        total_instances: total,
        positional_instances: positional,
        global_instances: global,
        distinct_codes,
        codes,
    }
}

impl HistogramReport {
    /// Print the human table: totals header, then every code row.
    pub fn print_table(&self) {
        println!("tsgo conformance — error-code histogram");
        println!("=======================================");
        println!("Files scanned:       {}", self.files_scanned);
        println!("Total instances:     {}", self.total_instances);
        println!("  positional:        {}", self.positional_instances);
        println!("  global (fileless): {}", self.global_instances);
        println!("Distinct codes:      {}", self.distinct_codes);
        println!();
        println!("{:>9}  {:>8}  {:>7}  {:>7}", "code", "count", "%", "cum%");
        println!(
            "{:>9}  {:>8}  {:>7}  {:>7}",
            "---------", "--------", "-------", "-------"
        );
        for row in &self.codes {
            println!(
                "{:>9}  {:>8}  {:>6.2}%  {:>6.2}%",
                format!("TS{}", row.code),
                row.count,
                row.pct,
                row.cumulative_pct,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// tests-by-code
// ---------------------------------------------------------------------------

/// The `tests-by-code` subquery report: the baseline files that mention a code.
#[derive(Debug, serde::Serialize)]
pub struct TestsByCodeReport {
    /// The queried `TS<code>`.
    pub code: u32,
    /// Number of baseline files whose summary block contains `code`.
    pub count: usize,
    /// Those files' relative paths, sorted.
    pub files: Vec<String>,
}

/// Find every baseline whose summary block contains `code` (once per file, not
/// per instance).
pub fn tests_by_code(baselines: &[Baseline], code: u32) -> TestsByCodeReport {
    let mut files = Vec::new();
    for baseline in baselines {
        let Some(content) = read_baseline(baseline) else {
            continue;
        };
        if parse_summary_block(&content).iter().any(|d| d.code == code) {
            files.push(baseline.relative_path.clone());
        }
    }
    files.sort();
    TestsByCodeReport {
        code,
        count: files.len(),
        files,
    }
}

impl TestsByCodeReport {
    /// Print the count line, then one file path per line (head-friendly).
    pub fn print(&self) {
        println!(
            "Baselines whose summary block contains TS{}: {}",
            self.code, self.count
        );
        println!();
        for file in &self.files {
            println!("{file}");
        }
    }
}

// ---------------------------------------------------------------------------
// denominators
// ---------------------------------------------------------------------------

/// The `denominators` subquery report: the sizing numbers a conformance-rate
/// denominator is drawn from.
#[derive(Debug, serde::Serialize)]
pub struct DenominatorsReport {
    /// Total `.errors.txt` baseline files.
    pub total_baselines: usize,
    /// Distinct test identities (relative path with any trailing `(…)` variant
    /// suffix stripped, deduped).
    pub distinct_identities: usize,
    /// Baselines carrying a `(key=value)` variant suffix.
    pub variant_suffixed: usize,
    /// Distinct identities backed by more than one baseline file (i.e. tests with
    /// more than one variant).
    pub identities_with_multiple_variants: usize,
    /// Baselines whose relative path contains `jsx` (case-insensitive) — a
    /// **lower bound** on JSX-scoped tests (precise detection needs the corpus).
    pub jsx_scoped_lower_bound: usize,
    /// Baselines whose summary block contains any TS4xxx (declaration-emit) code.
    pub baselines_with_declaration_emit_code: usize,
}

/// Compute the denominator sizing numbers over every baseline.
pub fn denominators(baselines: &[Baseline]) -> DenominatorsReport {
    let mut identity_counts: HashMap<String, usize> = HashMap::new();
    let mut variant_suffixed = 0usize;
    let mut jsx = 0usize;
    let mut declaration_emit = 0usize;

    for baseline in baselines {
        let (identity, had_variant) = test_identity(&baseline.relative_path);
        *identity_counts.entry(identity).or_insert(0) += 1;
        if had_variant {
            variant_suffixed += 1;
        }
        if baseline.relative_path.to_ascii_lowercase().contains("jsx") {
            jsx += 1;
        }
        if let Some(content) = read_baseline(baseline)
            && parse_summary_block(&content)
                .iter()
                .any(|d| DECLARATION_EMIT_RANGE.contains(&d.code))
        {
            declaration_emit += 1;
        }
    }

    let identities_with_multiple_variants = identity_counts.values().filter(|&&n| n > 1).count();

    DenominatorsReport {
        total_baselines: baselines.len(),
        distinct_identities: identity_counts.len(),
        variant_suffixed,
        identities_with_multiple_variants,
        jsx_scoped_lower_bound: jsx,
        baselines_with_declaration_emit_code: declaration_emit,
    }
}

impl DenominatorsReport {
    /// Print the human summary; `corpus_materialized` tunes the JSX caveat note.
    pub fn print_summary(&self, corpus_materialized: bool) {
        println!("tsgo conformance — denominators");
        println!("===============================");
        println!(
            "Total .errors.txt baselines:        {}",
            self.total_baselines
        );
        println!(
            "Distinct test identities:           {}",
            self.distinct_identities
        );
        println!("  (relative path, trailing (…) variant suffix stripped, deduped)");
        println!(
            "Variant-suffixed baselines:         {}",
            self.variant_suffixed
        );
        println!(
            "Tests with >1 variant:              {}",
            self.identities_with_multiple_variants
        );
        println!(
            "Baselines with a TS4xxx (declaration-emit) code: {}",
            self.baselines_with_declaration_emit_code
        );
        println!();
        println!(
            "JSX-scoped baselines (lower bound): {}",
            self.jsx_scoped_lower_bound
        );
        println!("  Heuristic: relative path contains \"jsx\" (case-insensitive).");
        println!("  Precise JSX detection needs @jsx directives / .tsx inputs from the");
        println!(
            "  corpus submodule, which is {}.",
            if corpus_materialized {
                "materialized"
            } else {
                "NOT materialized"
            }
        );
        if !corpus_materialized {
            println!("  Run `git submodule update --init` in ../typescript-go to materialize it.");
        }
    }
}

/// A test's identity and whether it carried a variant suffix: the relative path
/// with `.errors.txt` and any single trailing `(…)` group stripped from the file
/// name. A dotted case number (e.g. `foo.1`) is part of the identity, not a
/// variant — only a trailing parenthesized group is a variant.
fn test_identity(relative_path: &str) -> (String, bool) {
    let (dir, filename) = match relative_path.rsplit_once('/') {
        Some((d, f)) => (Some(d), f),
        None => (None, relative_path),
    };
    let base = filename.strip_suffix(".errors.txt").unwrap_or(filename);
    let stripped = strip_variant_suffix(base);
    let had_variant = stripped.len() != base.len();
    let identity = match dir {
        Some(d) => format!("{d}/{stripped}"),
        None => stripped.to_string(),
    };
    (identity, had_variant)
}

/// Strip a single trailing `(…)` variant group (with no nested parens), matching
/// the baselines' `(key=value)` suffix. Returns `base` unchanged when absent.
fn strip_variant_suffix(base: &str) -> &str {
    if let Some(inner) = base.strip_suffix(')')
        && let Some(open) = inner.rfind('(')
    {
        let group = &inner[open + 1..];
        if !group.contains('(') && !group.contains(')') {
            return &base[..open];
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_strips_single_variant_suffix() {
        let (id, had) = test_identity("conformance/foo(target=es2015).errors.txt");
        assert_eq!(id, "conformance/foo");
        assert!(had);
    }

    #[test]
    fn identity_keeps_dotted_case_number() {
        // `.1` is a distinct numbered test, not a variant suffix.
        let (id, had) = test_identity("conformance/topLevelAwaitErrors.1.errors.txt");
        assert_eq!(id, "conformance/topLevelAwaitErrors.1");
        assert!(!had);
    }

    #[test]
    fn identity_variant_on_dotted_case_number() {
        let (id, had) =
            test_identity("conformance/topLevelAwaitErrors.1(module=esnext).errors.txt");
        assert_eq!(id, "conformance/topLevelAwaitErrors.1");
        assert!(had);
    }

    #[test]
    fn identity_no_variant() {
        let (id, had) = test_identity("compiler/bar.errors.txt");
        assert_eq!(id, "compiler/bar");
        assert!(!had);
    }

    #[test]
    fn pct_guards_zero_total() {
        assert!((pct(0, 0) - 0.0).abs() < f64::EPSILON);
        assert!((pct(1, 4) - 25.0).abs() < f64::EPSILON);
    }
}
