use crate::cli::CliError;
use crate::compile_fixtures::with_trailing_newline;
use crate::deno::{self, DenoError, SvelteGenerate};
use crate::diff::{ColorChoice, DiffOptions, diff_to_string};
use argh::FromArgs;
use futures_util::StreamExt;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tsv_cli::json_utils::to_json_with_tabs;
use tsv_svelte_compile::{CompileError, CompileOptions, canonicalize_js, compile};

/// Run the Svelte compile-parity pipeline over corpora of `.svelte` files.
///
/// For every `.svelte` component under the given roots, compile with the
/// canonical Svelte compiler (the oracle) and with tsv, then compare the
/// canonical reprints of both sides. Every file lands in exactly one bucket:
///
/// - **parity** — both compiled and the canonical forms match.
/// - **refused** — tsv returned `Unsupported` (sub-bucketed by reason). A clean
///   "not yet," never a bug.
/// - **oracle-rejected** — the oracle rejected the source (legacy mode, invalid
///   syntax, TypeScript in a plain script). Out of scope for parity.
/// - **mismatch** — both compiled but the canonical forms differ. By the refusal
///   contract this is always a bug.
/// - **error** — a harness failure (sidecar, canonicalize, tsv parse rejection,
///   unreadable file).
///
/// Exit codes: 0 = clean (mismatch = 0, error = 0), 1 = a mismatch (a bug),
/// 2 = a harness error. Sidecar-dependent — kept out of `deno task check`; point
/// it at real repos and the Svelte test suites on demand.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compile_corpus_compare")]
pub struct CompileCorpusCompareCommand {
    /// list the discovered in-scope `.svelte` files without comparing
    #[argh(switch)]
    list: bool,

    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,

    /// directories or files to compare (each a reported group)
    #[argh(positional)]
    paths: Vec<String>,
}

/// One file's classification.
enum Bucket {
    /// Both sides compiled and the canonical forms matched.
    Parity,
    /// tsv refused (`Unsupported`), keyed on the normalized refusal reason.
    Refused(String),
    /// The oracle rejected the source, keyed on the Svelte error code (or the
    /// error's first line when no code is present).
    OracleRejected(String),
    /// Both sides compiled but the canonical forms differ (a bug); the bounded
    /// diff is carried for the report.
    Mismatch(String),
    /// A harness failure: `(kind, detail)`.
    Error(&'static str, String),
}

/// One file's outcome, tagged with its group (positional-root index) and path.
struct FileOutcome {
    group: usize,
    path: PathBuf,
    bucket: Bucket,
}

/// A positional root and how many in-scope files it holds.
struct GroupInfo {
    root: String,
    file_count: usize,
}

impl CompileCorpusCompareCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        if self.paths.is_empty() {
            eprintln!("Error: compile_corpus_compare needs at least one path");
            return Err(CliError::Failed);
        }

        // Discover in-scope `.svelte` files per group (positional root). Each
        // file carries its group index so per-root stats survive the flattened,
        // completion-ordered stream.
        let mut groups: Vec<GroupInfo> = Vec::new();
        let mut items: Vec<(usize, PathBuf)> = Vec::new();
        for (gi, root) in self.paths.iter().enumerate() {
            let p = Path::new(root);
            if !p.exists() {
                eprintln!("Error: path not found: {root}");
                return Err(CliError::Failed);
            }
            let mut files = Vec::new();
            collect_svelte_files(p, &mut files);
            files.sort();
            for f in &files {
                items.push((gi, f.clone()));
            }
            groups.push(GroupInfo {
                root: root.clone(),
                file_count: files.len(),
            });
        }

        if self.list {
            for (gi, path) in &items {
                println!("{}\t{}", groups[*gi].root, path.display());
            }
            println!(
                "\nTotal: {} files across {} root(s)",
                items.len(),
                groups.len()
            );
            return Ok(());
        }

        let rt = super::create_runtime();
        rt.block_on(self.run_async(groups, items))
    }

    async fn run_async(
        self,
        groups: Vec<GroupInfo>,
        items: Vec<(usize, PathBuf)>,
    ) -> Result<(), CliError> {
        let total = items.len();
        // Fan out over the bulk sidecar pool; the oracle compile is the only
        // per-file sidecar cost — tsv compile + canonicalize are pure Rust.
        let mut stream = super::spawn_work_stream(
            items,
            super::ResultOrder::Completion,
            |(group, path)| async move { classify_file(group, path).await },
        );
        let mut outcomes = Vec::with_capacity(total);
        while let Some(joined) = stream.next().await {
            outcomes.push(super::task_result(joined, "compile-corpus")?);
        }

        let report = Report::build(&groups, &outcomes);
        if self.json {
            report.print_json()?;
        } else {
            report.print_human();
        }

        // Mismatch is the headline finding (a compiler bug); a harness error
        // means some file got no verdict. Either is a non-zero exit.
        if report.totals.mismatch > 0 {
            Err(CliError::Failed)
        } else if report.totals.error > 0 {
            Err(CliError::Errored)
        } else {
            Ok(())
        }
    }
}

/// Read `path` and classify it, mapping an unreadable file to an error bucket.
async fn classify_file(group: usize, path: PathBuf) -> FileOutcome {
    let bucket = match std::fs::read_to_string(&path) {
        Ok(source) => classify(&source).await,
        Err(e) => Bucket::Error("read", e.to_string()),
    };
    FileOutcome { group, path, bucket }
}

/// The per-file compile-parity pipeline. Oracle-first: an oracle rejection is
/// out of scope for parity (and needs no tsv output), so it short-circuits
/// before tsv even runs; a tsv refusal short-circuits before either side is
/// canonicalized. Only both-compiled files reach the canonical-form comparison.
async fn classify(source: &str) -> Bucket {
    // Oracle side. A `ToolError` is the Svelte compiler *rejecting* the source
    // (legacy mode, invalid syntax, TS-in-plain-script); every other DenoError
    // is a genuine harness/sidecar failure.
    let oracle = match deno::svelte_compile(source, SvelteGenerate::Server, false).await {
        Ok(o) => o,
        Err(DenoError::ToolError { message }) => {
            return Bucket::OracleRejected(oracle_reject_reason(&message));
        }
        Err(e) => return Bucket::Error("oracle-sidecar", e.to_string()),
    };

    // tsv side. `Unsupported` is the honest refusal contract; a `Parse` error
    // means tsv's Svelte parser rejected a component the oracle compiled — a
    // parser gap worth surfacing loudly, not a silent skip.
    let ours = match compile(source, &CompileOptions::default()) {
        Ok(o) => o,
        Err(CompileError::Unsupported(reason)) => {
            return Bucket::Refused(normalize_refusal_reason(&reason));
        }
        Err(CompileError::Parse(e)) => return Bucket::Error("tsv-parse", e.to_string()),
    };

    // Both compiled — compare the canonical reprints (the parity bar).
    let oracle_canon = match canonicalize_js(&oracle.js) {
        Ok(c) => c,
        Err(e) => return Bucket::Error("canonicalize-oracle", e.to_string()),
    };
    // Self-check: the canonicalizer must be idempotent on the oracle output. A
    // violation is a canonicalizer bug, so surface it as a loud error.
    match canonicalize_js(&oracle_canon) {
        Ok(again) if again == oracle_canon => {}
        Ok(_) => return Bucket::Error("oracle-non-idempotent", String::new()),
        Err(e) => return Bucket::Error("oracle-recanonicalize", e.to_string()),
    }
    let ours_canon = match canonicalize_js(&ours.js) {
        Ok(c) => c,
        Err(e) => return Bucket::Error("canonicalize-ours", e.to_string()),
    };

    let js_match = ours_canon == oracle_canon;
    let ours_css = ours.css.as_deref().map(with_trailing_newline);
    let oracle_css = oracle.css.as_deref().map(with_trailing_newline);
    let css_match = ours_css == oracle_css;
    if js_match && css_match {
        return Bucket::Parity;
    }

    let mut diff = String::new();
    if !js_match {
        diff.push_str(&bounded_diff(&ours_canon, &oracle_canon));
    }
    if !css_match {
        diff.push_str("\n[css differs]\n");
        diff.push_str(&bounded_diff(
            ours_css.as_deref().unwrap_or(""),
            oracle_css.as_deref().unwrap_or(""),
        ));
    }
    Bucket::Mismatch(diff)
}

/// The `compile_compare` canonical-JS diff, color-free (so it is clean in a
/// stored/JSON report) and bounded to a sane number of lines.
fn bounded_diff(ours: &str, oracle: &str) -> String {
    let opts = DiffOptions::compile_compare().with_color_choice(ColorChoice::Never);
    bound_lines(&diff_to_string(ours, oracle, &opts), MAX_DIFF_LINES)
}

/// Maximum diff lines kept per mismatch (mismatches must be zero, but a
/// pathological one shouldn't flood the report).
const MAX_DIFF_LINES: usize = 60;

/// Truncate `s` to `max` lines, appending a marker when it was longer.
fn bound_lines(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, line) in s.lines().enumerate() {
        if i >= max {
            use std::fmt::Write;
            let _ = writeln!(out, "… (diff truncated at {max} lines)");
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Classify an oracle rejection by its Svelte error code
/// (`https://svelte.dev/e/{code}` in the message), falling back to the first
/// non-empty line. The code cleanly separates the buckets — `legacy_*` (legacy
/// mode), `js_parse_error` (which includes TS-in-a-plain-script), etc.
fn oracle_reject_reason(message: &str) -> String {
    const MARKER: &str = "svelte.dev/e/";
    if let Some(idx) = message.find(MARKER) {
        let code: String = message[idx + MARKER.len()..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !code.is_empty() {
            return code;
        }
    }
    let first = message
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    truncate(first, 80)
}

/// Collapse a `CompileError::Unsupported` reason to a stable bucket key.
///
/// Variable *identifiers/literals* the user chose (attribute/binding/class
/// names, `lang` values, component tags, runes) collapse to a `{placeholder}`
/// so, e.g., `event attribute onclick` and `event attribute onkeydown` share one
/// bucket. Closed-set *feature discriminants* (`template node {kind}`, `binding
/// pattern shape ({kind})`) are left intact — they name distinct unsupported
/// features, and keeping them apart is the useful signal. Static reasons pass
/// through verbatim (they are already the bucket).
fn normalize_refusal_reason(reason: &str) -> String {
    // Nested static-eval `Gray` messages — collapse to the outer template.
    if reason.starts_with("static evaluation not portable") {
        return "static evaluation not portable".to_string();
    }
    if reason.starts_with("static fold not portable") {
        return "static fold not portable".to_string();
    }
    // Rune-shaped refusals.
    if reason.starts_with("rune ") {
        return "rune {name}".to_string();
    }
    if reason.starts_with("$-prefixed identifier ") {
        return "$-prefixed identifier {name}".to_string();
    }
    if reason.starts_with("read of derived binding ") {
        return "read of derived binding {name}".to_string();
    }
    if reason.starts_with("member/call rooted at prop/import ") {
        return "member/call rooted at prop/import {name} also bound in a nested scope".to_string();
    }
    if reason.starts_with("block-scope binding ") && reason.ends_with(" shadows a $derived binding")
    {
        return "block-scope binding {name} shadows a $derived binding".to_string();
    }
    if reason.starts_with("css selector .") && reason.contains("matches no element") {
        return "css selector .{class} matches no element".to_string();
    }
    if reason.starts_with("lang=\"") && reason.contains("instance script") {
        return "lang=\"{lang}\" instance script".to_string();
    }
    if reason.starts_with("generated name ") && reason.ends_with(" collides with a user binding") {
        return "generated name {name} collides with a user binding".to_string();
    }
    if reason.starts_with("interpolated ") && reason.ends_with(" attribute on a styled component") {
        return "interpolated {name} attribute on a styled component".to_string();
    }
    if reason.starts_with("event attribute ") {
        return "event attribute {name}".to_string();
    }
    // `<name> …` element-tag interpolations.
    if reason.starts_with('<') {
        if reason.ends_with(" component (component rendering not implemented)") {
            return "<{name}> component".to_string();
        }
        if reason.ends_with(" (foreign namespace)") {
            return "<{name}> (foreign namespace)".to_string();
        }
        if reason.ends_with(" with children (oracle emits a `<!>` anchor)") {
            return "<{name}> with children".to_string();
        }
    }
    if reason.starts_with("template-level <") {
        return "template-level <{name}>".to_string();
    }
    if reason.starts_with("children on void element <") {
        return "children on void element <{name}>".to_string();
    }
    if reason.starts_with("value attribute on <") {
        return "value attribute on <{name}>".to_string();
    }
    reason.to_string()
}

/// Truncate `s` to `max` chars (char-boundary safe), appending `…` when cut.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// Recursively collect in-scope `.svelte` files under `path`, skipping the usual
/// non-source directories (so it can point straight at a repo). `.svelte-kit`
/// and other dot-directories fall out via the hidden-directory skip.
fn collect_svelte_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if is_svelte_file(path) {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let child = entry.path();
        if child.is_dir() {
            let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.')
                || name == "node_modules"
                || name == "dist"
                || name == "build"
                || name == "target"
            {
                continue;
            }
            collect_svelte_files(&child, out);
        } else if is_svelte_file(&child) {
            out.push(child);
        }
    }
}

/// A `.svelte` component file — not a `.svelte.ts`/`.svelte.js` module (those
/// end in `.ts`/`.js`, so the suffix test excludes them).
fn is_svelte_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".svelte"))
}

// ---- Aggregation & reporting ------------------------------------------------

/// Per-group (or total) bucket counts.
#[derive(Default, Clone, serde::Serialize)]
struct Stats {
    files: usize,
    parity: usize,
    refused: usize,
    oracle_rejected: usize,
    mismatch: usize,
    error: usize,
}

/// A reason with its count and a few example paths (capped).
#[derive(serde::Serialize)]
struct ReasonCount {
    reason: String,
    count: usize,
    examples: Vec<String>,
}

/// A mismatch — always a bug; carried in full.
#[derive(serde::Serialize)]
struct MismatchEntry {
    group: String,
    path: String,
    diff: String,
}

/// A harness error.
#[derive(serde::Serialize)]
struct ErrorEntry {
    group: String,
    path: String,
    kind: String,
    detail: String,
}

/// Accumulator for one reason's count + capped example paths.
#[derive(Default)]
struct ReasonAgg {
    count: usize,
    examples: Vec<String>,
}

impl ReasonAgg {
    const EXAMPLE_CAP: usize = 3;
    fn add(&mut self, path: &str) {
        self.count += 1;
        if self.examples.len() < Self::EXAMPLE_CAP {
            self.examples.push(path.to_string());
        }
    }
}

/// Cap on the number of error entries carried in the report.
const ERROR_CAP: usize = 100;

/// The full aggregated report.
struct Report {
    totals: Stats,
    groups: Vec<(String, Stats)>,
    refusal_reasons: Vec<ReasonCount>,
    oracle_rejected_reasons: Vec<ReasonCount>,
    mismatches: Vec<MismatchEntry>,
    errors: Vec<ErrorEntry>,
    errors_truncated: usize,
}

impl Report {
    fn build(groups: &[GroupInfo], outcomes: &[FileOutcome]) -> Self {
        let mut totals = Stats::default();
        let mut group_stats: Vec<Stats> = groups
            .iter()
            .map(|g| Stats {
                files: g.file_count,
                ..Stats::default()
            })
            .collect();
        totals.files = outcomes.len();

        let mut refusal: BTreeMap<String, ReasonAgg> = BTreeMap::new();
        let mut oracle_rej: BTreeMap<String, ReasonAgg> = BTreeMap::new();
        let mut mismatches = Vec::new();
        let mut errors = Vec::new();
        let mut errors_truncated = 0;

        let root_of = |gi: usize| groups[gi].root.clone();

        for o in outcomes {
            let gs = &mut group_stats[o.group];
            let path = o.path.display().to_string();
            match &o.bucket {
                Bucket::Parity => {
                    totals.parity += 1;
                    gs.parity += 1;
                }
                Bucket::Refused(reason) => {
                    totals.refused += 1;
                    gs.refused += 1;
                    refusal.entry(reason.clone()).or_default().add(&path);
                }
                Bucket::OracleRejected(reason) => {
                    totals.oracle_rejected += 1;
                    gs.oracle_rejected += 1;
                    oracle_rej.entry(reason.clone()).or_default().add(&path);
                }
                Bucket::Mismatch(diff) => {
                    totals.mismatch += 1;
                    gs.mismatch += 1;
                    mismatches.push(MismatchEntry {
                        group: root_of(o.group),
                        path,
                        diff: diff.clone(),
                    });
                }
                Bucket::Error(kind, detail) => {
                    totals.error += 1;
                    gs.error += 1;
                    if errors.len() < ERROR_CAP {
                        errors.push(ErrorEntry {
                            group: root_of(o.group),
                            path,
                            kind: (*kind).to_string(),
                            detail: detail.clone(),
                        });
                    } else {
                        errors_truncated += 1;
                    }
                }
            }
        }

        // Deterministic order: mismatches/errors by path.
        mismatches.sort_by(|a, b| a.path.cmp(&b.path));
        errors.sort_by(|a, b| a.path.cmp(&b.path));

        Report {
            totals,
            groups: groups
                .iter()
                .zip(group_stats)
                .map(|(g, s)| (g.root.clone(), s))
                .collect(),
            refusal_reasons: sort_reasons(refusal),
            oracle_rejected_reasons: sort_reasons(oracle_rej),
            mismatches,
            errors,
            errors_truncated,
        }
    }

    fn print_human(&self) {
        println!(
            "compile_corpus_compare — {} files across {} root(s)\n",
            self.totals.files,
            self.groups.len()
        );
        if self.groups.len() > 1 {
            println!("Per-root:");
            for (root, s) in &self.groups {
                println!("  {root}");
                println!("    {}", stats_line(s));
            }
            println!();
        }
        println!("Totals: {}", stats_line(&self.totals));

        print_reasons("Top refusal reasons", &self.refusal_reasons, 15);
        print_reasons(
            "Oracle-rejected reasons",
            &self.oracle_rejected_reasons,
            usize::MAX,
        );

        if !self.mismatches.is_empty() {
            println!("\nMISMATCHES ({}) — each is a bug:", self.mismatches.len());
            for m in &self.mismatches {
                println!("\n  [{}] {}", m.group, m.path);
                for line in m.diff.lines() {
                    println!("    {line}");
                }
            }
        }

        if !self.errors.is_empty() || self.errors_truncated > 0 {
            println!(
                "\nErrors ({}):",
                self.totals.error
            );
            for e in &self.errors {
                let detail = if e.detail.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", truncate(&e.detail.replace('\n', " "), 160))
                };
                println!("  [{}] {} [{}]{}", e.group, e.path, e.kind, detail);
            }
            if self.errors_truncated > 0 {
                println!("  … (+{} more errors)", self.errors_truncated);
            }
        }
    }

    fn print_json(&self) -> Result<(), CliError> {
        #[derive(serde::Serialize)]
        struct GroupJson<'a> {
            root: &'a str,
            #[serde(flatten)]
            stats: &'a Stats,
        }
        #[derive(serde::Serialize)]
        struct JsonReport<'a> {
            #[serde(flatten)]
            totals: &'a Stats,
            groups: Vec<GroupJson<'a>>,
            refusal_reasons: &'a [ReasonCount],
            oracle_rejected_reasons: &'a [ReasonCount],
            mismatches: &'a [MismatchEntry],
            errors: &'a [ErrorEntry],
            errors_truncated: usize,
        }
        let report = JsonReport {
            totals: &self.totals,
            groups: self
                .groups
                .iter()
                .map(|(root, stats)| GroupJson { root, stats })
                .collect(),
            refusal_reasons: &self.refusal_reasons,
            oracle_rejected_reasons: &self.oracle_rejected_reasons,
            mismatches: &self.mismatches,
            errors: &self.errors,
            errors_truncated: self.errors_truncated,
        };
        match to_json_with_tabs(&report) {
            Ok(json) => {
                println!("{json}");
                Ok(())
            }
            Err(e) => {
                eprintln!("Error serializing report: {e}");
                Err(CliError::Errored)
            }
        }
    }
}

/// Sort a reason map into a count-descending (then reason-ascending) list.
fn sort_reasons(map: BTreeMap<String, ReasonAgg>) -> Vec<ReasonCount> {
    let mut v: Vec<ReasonCount> = map
        .into_iter()
        .map(|(reason, agg)| ReasonCount {
            reason,
            count: agg.count,
            examples: agg.examples,
        })
        .collect();
    v.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.reason.cmp(&b.reason)));
    v
}

/// A one-line bucket summary.
fn stats_line(s: &Stats) -> String {
    format!(
        "files={} parity={} refused={} oracle_rejected={} mismatch={} error={}",
        s.files, s.parity, s.refused, s.oracle_rejected, s.mismatch, s.error
    )
}

/// Print a titled reason histogram, top `limit` rows.
fn print_reasons(title: &str, reasons: &[ReasonCount], limit: usize) {
    if reasons.is_empty() {
        return;
    }
    println!("\n{title}:");
    for r in reasons.iter().take(limit) {
        println!("  {:>6}  {}", r.count, r.reason);
    }
    if reasons.len() > limit {
        println!("  (+{} more reasons)", reasons.len() - limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizer_collapses_name_interpolations() {
        // The motivating case: distinct event-handler names share one bucket.
        assert_eq!(normalize_refusal_reason("event attribute onclick"), "event attribute {name}");
        assert_eq!(
            normalize_refusal_reason("event attribute onkeydown"),
            "event attribute {name}"
        );
        assert_eq!(normalize_refusal_reason("rune $state"), "rune {name}");
        assert_eq!(normalize_refusal_reason("rune $foo"), "rune {name}");
        assert_eq!(
            normalize_refusal_reason("lang=\"ts\" instance script (type stripping not implemented)"),
            "lang=\"{lang}\" instance script"
        );
        assert_eq!(
            normalize_refusal_reason("<Foo.Bar> component (component rendering not implemented)"),
            "<{name}> component"
        );
        assert_eq!(
            normalize_refusal_reason("children on void element <br>"),
            "children on void element <{name}>"
        );
        assert_eq!(
            normalize_refusal_reason("static evaluation not portable: string-to-number coercion"),
            "static evaluation not portable"
        );
    }

    #[test]
    fn normalizer_preserves_feature_kinds_and_static_reasons() {
        // Closed-set feature discriminants stay distinct.
        assert_eq!(
            normalize_refusal_reason("template node special element"),
            "template node special element"
        );
        assert_eq!(
            normalize_refusal_reason("template node {#snippet} block"),
            "template node {#snippet} block"
        );
        // Static reasons are already the bucket.
        assert_eq!(
            normalize_refusal_reason("instance-script export (component exports / $.bind_props not implemented)"),
            "instance-script export (component exports / $.bind_props not implemented)"
        );
    }

    #[test]
    fn oracle_reject_reason_extracts_svelte_code() {
        let msg = "Cannot use `export let` in runes mode — use `$props()` instead\nhttps://svelte.dev/e/legacy_export_invalid";
        assert_eq!(oracle_reject_reason(msg), "legacy_export_invalid");
        // No code URL → first non-empty line (bounded).
        assert_eq!(oracle_reject_reason("weird failure\n"), "weird failure");
    }

    #[test]
    fn bound_lines_truncates() {
        let s = "a\nb\nc\nd\n";
        assert_eq!(bound_lines(s, 2), "a\nb\n… (diff truncated at 2 lines)\n");
        assert_eq!(bound_lines(s, 10), "a\nb\nc\nd\n");
    }

    #[test]
    fn is_svelte_file_excludes_modules() {
        assert!(is_svelte_file(Path::new("Foo.svelte")));
        assert!(!is_svelte_file(Path::new("foo.svelte.ts")));
        assert!(!is_svelte_file(Path::new("foo.ts")));
    }
}
