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
///   syntax, TypeScript in a plain script). Out of scope for parity. Each such
///   file is also probed with tsv's `compile()`: a success is reported in the
///   OVER-ACCEPTANCE section (a refusal-contract gap — nothing invalid in
///   runes mode may compile), without affecting the exit code.
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
    /// tsv refused (`Unsupported`), keyed on the refusal's stable
    /// [`Refusal::bucket_key`](tsv_svelte_compile::Refusal::bucket_key).
    Refused(String),
    /// The oracle rejected the source, keyed on the Svelte error code (or the
    /// error's first line when no code is present). `tsv_over_accepts` records
    /// whether tsv's `compile()` nevertheless succeeded on it — always a gap
    /// (nothing invalid in runes mode may compile), reported loudly, though
    /// the bucket itself stays out of the exit gate.
    OracleRejected {
        code: String,
        tsv_over_accepts: bool,
    },
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
        // completion-ordered stream. The visited/seen sets persist across roots:
        // symlink cycles can't loop the walk, and overlapping roots don't
        // double-count — the first root to reach a file wins its attribution.
        let mut groups: Vec<GroupInfo> = Vec::new();
        let mut items: Vec<(usize, PathBuf)> = Vec::new();
        let mut visited = VisitedSet::default();
        for (gi, root) in self.paths.iter().enumerate() {
            let p = Path::new(root);
            if !p.exists() {
                eprintln!("Error: path not found: {root}");
                return Err(CliError::Failed);
            }
            let mut files = Vec::new();
            collect_svelte_files(p, &mut visited, &mut files);
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
    FileOutcome {
        group,
        path,
        bucket,
    }
}

/// The per-file compile-parity pipeline. Oracle-first: an oracle rejection is
/// out of scope for parity (and needs no tsv output), so it short-circuits
/// before tsv even runs; a tsv refusal short-circuits before either side is
/// canonicalized. Only both-compiled files reach the canonical-form comparison.
async fn classify(source: &str) -> Bucket {
    // Oracle side. A `ToolError` carrying a `svelte.dev/e/{code}` URL is the
    // Svelte compiler *rejecting* the source (legacy mode, invalid syntax,
    // TS-in-plain-script); a ToolError WITHOUT a code is a sidecar-internal
    // failure and must not inflate the oracle_rejected bucket (it would slip
    // the exit gate). Every other DenoError is a harness/sidecar failure too.
    let oracle = match deno::svelte_compile(source, SvelteGenerate::Server, false).await {
        Ok(o) => o,
        Err(DenoError::ToolError { message }) => {
            return match oracle_reject_code(&message) {
                // Over-acceptance probe: an oracle-rejected component that tsv
                // compiles anyway is a refusal-contract gap (the `$:` class) —
                // cheap to check here since tsv's compile is pure Rust.
                Some(code) => Bucket::OracleRejected {
                    code,
                    tsv_over_accepts: compile(source, &CompileOptions::default()).is_ok(),
                },
                None => Bucket::Error("oracle-tool", first_line(&message)),
            };
        }
        Err(e) => return Bucket::Error("oracle-sidecar", e.to_string()),
    };

    // tsv side. `Unsupported` is the honest refusal contract; a `Parse` error
    // means tsv's Svelte parser rejected a component the oracle compiled — a
    // parser gap worth surfacing loudly, not a silent skip. `CorruptOutput` is
    // the compile self-validation firing: a divergent shape slipped every
    // guard and emitted unparseable JS — always a compiler bug.
    let ours = match compile(source, &CompileOptions::default()) {
        Ok(o) => o,
        Err(CompileError::Unsupported(reason)) => {
            return Bucket::Refused(reason.bucket_key().into_owned());
        }
        Err(CompileError::Parse(e)) => return Bucket::Error("tsv-parse", e.to_string()),
        Err(CompileError::CorruptOutput(e)) => {
            return Bucket::Error("tsv-corrupt-output", e.to_string());
        }
        // The type-erasure self-check firing: a TypeScript-only node survived
        // into the emitted program — a missed erase case, always a compiler bug
        // (and one the output reparse cannot see, since the annotation parses).
        Err(CompileError::TypeErasureLeak(span)) => {
            return Bucket::Error("tsv-type-erasure-leak", format!("at {span:?}"));
        }
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

/// Extract the Svelte error code from a genuine oracle rejection
/// (`https://svelte.dev/e/{code}` in the message). The code cleanly separates
/// the buckets — `legacy_*` (legacy mode), `js_parse_error` (which includes
/// TS-in-a-plain-script), etc. A ToolError without a code is NOT a rejection
/// (a sidecar-internal failure) — the caller routes it to the error bucket.
fn oracle_reject_code(message: &str) -> Option<String> {
    const MARKER: &str = "svelte.dev/e/";
    let idx = message.find(MARKER)?;
    let code: String = message[idx + MARKER.len()..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if code.is_empty() { None } else { Some(code) }
}

/// The first non-empty line of `s`, bounded — error-detail projection.
fn first_line(s: &str) -> String {
    let first = s
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    truncate(first, 160)
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

/// Canonicalized-path identity sets for the corpus walk: `dirs` breaks symlink
/// cycles (a directory is descended once, ever), `files` dedupes files reached
/// through multiple roots or links (the first reach wins group attribution).
#[derive(Default)]
struct VisitedSet {
    dirs: std::collections::HashSet<PathBuf>,
    files: std::collections::HashSet<PathBuf>,
}

impl VisitedSet {
    /// Record `path`'s canonical identity in `set`; `false` when already
    /// present (skip) or the path doesn't canonicalize (dangling link — skip).
    fn first_visit(set: &mut std::collections::HashSet<PathBuf>, path: &Path) -> bool {
        match std::fs::canonicalize(path) {
            Ok(canon) => set.insert(canon),
            Err(_) => false,
        }
    }
}

/// Recursively collect in-scope `.svelte` files under `path`, skipping the usual
/// non-source directories (so it can point straight at a repo). `.svelte-kit`
/// and other dot-directories fall out via the hidden-directory skip. `visited`
/// makes the walk cycle-safe and duplicate-free (see [`VisitedSet`]).
fn collect_svelte_files(path: &Path, visited: &mut VisitedSet, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if is_svelte_file(path) && VisitedSet::first_visit(&mut visited.files, path) {
            out.push(path.to_path_buf());
        }
        return;
    }
    if !VisitedSet::first_visit(&mut visited.dirs, path) {
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
            collect_svelte_files(&child, visited, out);
        } else if is_svelte_file(&child) && VisitedSet::first_visit(&mut visited.files, &child) {
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
    over_acceptance: Vec<ReasonCount>,
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
        let mut over_accept: BTreeMap<String, ReasonAgg> = BTreeMap::new();
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
                Bucket::OracleRejected {
                    code,
                    tsv_over_accepts,
                } => {
                    totals.oracle_rejected += 1;
                    gs.oracle_rejected += 1;
                    oracle_rej.entry(code.clone()).or_default().add(&path);
                    if *tsv_over_accepts {
                        over_accept.entry(code.clone()).or_default().add(&path);
                    }
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
            over_acceptance: sort_reasons(over_accept),
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
        if !self.over_acceptance.is_empty() {
            let total: usize = self.over_acceptance.iter().map(|r| r.count).sum();
            println!(
                "\nOVER-ACCEPTANCE ({total}) — oracle-rejected but tsv compiles; each is a refusal-contract gap:"
            );
            print_reasons("By oracle code", &self.over_acceptance, usize::MAX);
        }

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
            println!("\nErrors ({}):", self.totals.error);
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
            over_acceptance: &'a [ReasonCount],
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
            over_acceptance: &self.over_acceptance,
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
    fn oracle_reject_code_requires_svelte_code() {
        let msg = "Cannot use `export let` in runes mode — use `$props()` instead\nhttps://svelte.dev/e/legacy_export_invalid";
        assert_eq!(
            oracle_reject_code(msg).as_deref(),
            Some("legacy_export_invalid")
        );
        // No code URL → not a rejection (the caller buckets it as an ERROR so
        // sidecar-internal failures can't inflate oracle_rejected).
        assert_eq!(oracle_reject_code("weird sidecar failure\n"), None);
        assert_eq!(oracle_reject_code("see svelte.dev/e/"), None);
        assert_eq!(first_line("\n  weird failure  \nmore"), "weird failure");
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

    #[test]
    fn collect_dedupes_overlapping_roots_and_survives_symlink_cycles() {
        // Overlapping roots (a parent and its child) must not double-count, and
        // a symlink cycle must not loop the walk.
        let dir = std::env::temp_dir().join(format!("tsv_corpus_walk_test_{}", std::process::id()));
        let sub = dir.join("sub");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("a.svelte"), "<p>a</p>").unwrap();
        std::fs::write(sub.join("b.svelte"), "<p>b</p>").unwrap();
        // A cycle: sub/loop -> the root dir.
        #[cfg(unix)]
        std::os::unix::fs::symlink(&dir, sub.join("loop")).unwrap();

        let mut visited = VisitedSet::default();
        let mut first = Vec::new();
        collect_svelte_files(&dir, &mut visited, &mut first);
        assert_eq!(first.len(), 2, "cycle must not duplicate: {first:?}");

        // The overlapping second root finds nothing new (first root won).
        let mut second = Vec::new();
        collect_svelte_files(&sub, &mut visited, &mut second);
        assert!(second.is_empty(), "overlap must dedupe: {second:?}");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
