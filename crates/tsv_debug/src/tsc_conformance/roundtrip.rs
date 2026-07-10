//! The P0 round-trip self-check: parse every tsgo `.errors.txt` baseline,
//! re-render it through [`super::render`], and byte-compare against the original.
//!
//! This proves the parser and the renderer in one move, with zero checker code:
//! a 100% byte-identical pass means the recovered model is a faithful,
//! reversible view of the baseline format. Failures are bucketed by their most
//! salient format feature (the P0 work-list); the pass count is a two-sided
//! regression pin.
//!
//! Known residual (measured 2026-07-09 vs pin `168e7015`): a small set of
//! ANSI-colored `pretty=true` baselines (out of the ported rune-path scope) and
//! a couple of baselines whose related-info carries a message chain whose deeper
//! (4-space) continuation lines are byte-ambiguous with source lines. Both are
//! reported honestly rather than hidden by loosening the comparison.

use super::baseline::parse_baseline;
use super::discovery::Baseline;
use super::render::{render_baseline, self_assertion_violations};
use std::collections::BTreeMap;

/// Cap on the message excerpt shown for a failing baseline (in chars).
const EXCERPT_CHARS: usize = 120;
/// Examples retained per bucket.
const EXAMPLES_PER_BUCKET: usize = 3;

/// One example of a failing baseline.
#[derive(Debug, serde::Serialize)]
pub struct FailExample {
    /// The baseline's relative path.
    pub path: String,
    /// Why it failed (a parse-error reason, or `render mismatch`).
    pub reason: String,
    /// 0-based index of the first differing line (render mismatches only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_diff_line: Option<usize>,
    /// The expected line at the first difference (truncated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// The rendered line at the first difference (truncated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub got: Option<String>,
}

/// One failure bucket (a format feature) with its count and a few examples.
#[derive(Debug, serde::Serialize)]
pub struct BucketCount {
    /// The bucket name (e.g. `pretty`, `related_info`).
    pub bucket: String,
    /// Number of failing baselines in this bucket.
    pub count: usize,
    /// Up to [`EXAMPLES_PER_BUCKET`] example failures.
    pub examples: Vec<FailExample>,
}

/// The round-trip report.
#[derive(Debug, serde::Serialize)]
pub struct RoundtripReport {
    /// Baselines checked.
    pub files_checked: usize,
    /// Baselines that round-tripped byte-identically.
    pub byte_identical: usize,
    /// `byte_identical / files_checked * 100`.
    pub pass_rate: f64,
    /// Baselines whose parsed model tripped a ported self-assertion (should be 0).
    pub self_assertion_violations: usize,
    /// Failure buckets, sorted by descending count.
    pub buckets: Vec<BucketCount>,
    /// Every failing baseline's path (sorted); only shown in `--verbose`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub failing_paths: Vec<String>,
}

/// Parse → render → byte-compare every baseline, folding results into a report.
pub fn run_roundtrip(baselines: &[Baseline]) -> RoundtripReport {
    let mut byte_identical = 0usize;
    let mut assertion_violations = 0usize;
    let mut bucket_map: BTreeMap<&'static str, Vec<FailExample>> = BTreeMap::new();
    let mut failing_paths: Vec<String> = Vec::new();

    for baseline in baselines {
        let content = match std::fs::read_to_string(&baseline.path) {
            Ok(c) => c,
            Err(e) => {
                record(
                    &mut bucket_map,
                    &mut failing_paths,
                    "read_error",
                    FailExample {
                        path: baseline.relative_path.clone(),
                        reason: format!("read error: {e}"),
                        first_diff_line: None,
                        expected: None,
                        got: None,
                    },
                );
                continue;
            }
        };

        match parse_baseline(&content) {
            Ok(parsed) => {
                if !self_assertion_violations(&parsed).is_empty() {
                    assertion_violations += 1;
                }
                let rendered = render_baseline(&parsed);
                if rendered == content {
                    byte_identical += 1;
                } else {
                    let (line, expected, got) = first_diff(&content, &rendered);
                    record(
                        &mut bucket_map,
                        &mut failing_paths,
                        categorize(&content),
                        FailExample {
                            path: baseline.relative_path.clone(),
                            reason: "render mismatch".to_string(),
                            first_diff_line: Some(line),
                            expected: Some(expected),
                            got: Some(got),
                        },
                    );
                }
            }
            Err(reason) => {
                record(
                    &mut bucket_map,
                    &mut failing_paths,
                    categorize(&content),
                    FailExample {
                        path: baseline.relative_path.clone(),
                        reason,
                        first_diff_line: None,
                        expected: None,
                        got: None,
                    },
                );
            }
        }
    }

    let files_checked = baselines.len();
    let pass_rate = pct(byte_identical, files_checked);

    let mut buckets: Vec<BucketCount> = bucket_map
        .into_iter()
        .map(|(bucket, mut examples)| {
            let count = examples.len();
            examples.truncate(EXAMPLES_PER_BUCKET);
            BucketCount {
                bucket: bucket.to_string(),
                count,
                examples,
            }
        })
        .collect();
    buckets.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.bucket.cmp(&b.bucket)));
    failing_paths.sort();

    RoundtripReport {
        files_checked,
        byte_identical,
        pass_rate,
        self_assertion_violations: assertion_violations,
        buckets,
        failing_paths,
    }
}

/// Push a failure into its bucket and onto the failing-paths list.
fn record(
    bucket_map: &mut BTreeMap<&'static str, Vec<FailExample>>,
    failing_paths: &mut Vec<String>,
    bucket: &'static str,
    example: FailExample,
) {
    failing_paths.push(example.path.clone());
    bucket_map.entry(bucket).or_default().push(example);
}

/// `count / total * 100`, guarding division by zero.
#[allow(clippy::cast_precision_loss)] // counts stay well within f64 precision
fn pct(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (count as f64 / total as f64) * 100.0
    }
}

/// The first differing `CRLF`-line between the original and the render, with a
/// truncated excerpt of each (`<EOF>` when one side ran out first).
fn first_diff(original: &str, rendered: &str) -> (usize, String, String) {
    let a: Vec<&str> = original.split("\r\n").collect();
    let b: Vec<&str> = rendered.split("\r\n").collect();
    let mut k = 0usize;
    while k < a.len() && k < b.len() && a[k] == b[k] {
        k += 1;
    }
    let excerpt = |s: Option<&&str>| s.map_or_else(|| "<EOF>".to_string(), |v| truncate(v));
    (k, excerpt(a.get(k)), excerpt(b.get(k)))
}

/// Truncate to [`EXCERPT_CHARS`] chars (char-safe), appending an ellipsis.
fn truncate(s: &str) -> String {
    if s.chars().count() <= EXCERPT_CHARS {
        s.to_string()
    } else {
        let head: String = s.chars().take(EXCERPT_CHARS).collect();
        format!("{head}…")
    }
}

/// Bucket a failing baseline by its most salient format feature (priority
/// order). This is a heuristic taxonomy — the correlate of the failure, not a
/// proof of its cause — and doubles as the P0 work-list.
fn categorize(content: &str) -> &'static str {
    if content.contains('\u{1b}') {
        // ANSI escape → the colored pretty=true path (out of the rune-path scope).
        "pretty"
    } else if content.contains("!!! related") {
        "related_info"
    } else if has_summary_chain(content) {
        "chain"
    } else if content.contains("(--,--)") || content.contains(":--:--") {
        "lib_mask"
    } else if has_multiline_span(content) {
        "multiline_span"
    } else if !content.is_ascii() {
        "astral_rune"
    } else {
        "other"
    }
}

/// A message-chain continuation in the summary block (a leading-space line
/// before the first `====` section).
fn has_summary_chain(content: &str) -> bool {
    for line in content.split("\r\n") {
        if line.starts_with("==== ") {
            break;
        }
        if line.starts_with(' ') && !line.trim().is_empty() {
            return true;
        }
    }
    false
}

/// Two consecutive squiggle-only lines — the signature of a multi-line span.
fn has_multiline_span(content: &str) -> bool {
    let is_squiggle = |line: &str| {
        line.strip_prefix("    ").is_some_and(|body| {
            body.contains('~') && body.bytes().all(|b| matches!(b, b' ' | b'\t' | b'~'))
        })
    };
    let lines: Vec<&str> = content.split("\r\n").collect();
    lines
        .windows(2)
        .any(|w| is_squiggle(w[0]) && is_squiggle(w[1]))
}

impl RoundtripReport {
    /// Print the human report; `verbose` also lists every failing path.
    pub fn print(&self, verbose: bool) {
        println!("tsgo conformance — round-trip self-check");
        println!("========================================");
        println!("Files checked:    {}", self.files_checked);
        println!("Byte-identical:   {}", self.byte_identical);
        println!("Pass rate:        {:.3}%", self.pass_rate);
        println!("Self-assert fails: {}", self.self_assertion_violations);
        if self.buckets.is_empty() {
            println!("\nAll baselines round-trip byte-identically.");
            return;
        }
        println!("\nFailure buckets (feature correlate, priority-ordered):");
        for b in &self.buckets {
            println!("  {:>5}  {}", b.count, b.bucket);
            for ex in &b.examples {
                println!("           {} — {}", ex.path, ex.reason);
                if let (Some(l), Some(e), Some(g)) =
                    (ex.first_diff_line, ex.expected.as_ref(), ex.got.as_ref())
                {
                    println!("             @line {l}");
                    println!("             expected: {e}");
                    println!("             got:      {g}");
                }
            }
        }
        if verbose {
            println!("\nAll failing paths:");
            for p in &self.failing_paths {
                println!("  {p}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_diff_locates_line() {
        let a = "x\r\ny\r\nz";
        let b = "x\r\nY\r\nz";
        let (line, exp, got) = first_diff(a, b);
        assert_eq!(line, 1);
        assert_eq!(exp, "y");
        assert_eq!(got, "Y");
    }

    #[test]
    fn categorize_priority() {
        assert_eq!(categorize("\u{1b}[96mx\u{1b}[0m"), "pretty");
        assert_eq!(
            categorize("a.ts(1,1): error TS1: x\r\n!!! related TS2 a:1:1: y"),
            "related_info"
        );
        assert_eq!(
            categorize("a.ts(1,1): error TS1: x\r\n  chained\r\n\r\n\r\n==== a"),
            "chain"
        );
        assert_eq!(categorize("lib.d.ts(--,--): error TS1: x"), "lib_mask");
        assert_eq!(categorize("a.ts(1,1): error TS1: x"), "other");
    }

    #[test]
    fn multiline_span_detected() {
        assert!(has_multiline_span("    ~~~~\r\n    ~~~~"));
        assert!(!has_multiline_span("    code;\r\n    ~~~~"));
    }
}
