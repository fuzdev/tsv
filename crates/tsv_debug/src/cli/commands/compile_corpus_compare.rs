use crate::cli::CliError;
use crate::compile_fixtures::with_trailing_newline;
use crate::deno::{self, DenoError, SvelteGenerate};
use crate::diff::{ColorChoice, DiffOptions, diff_to_string};
use argh::FromArgs;
use futures_util::StreamExt;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tsv_cli::json_utils::to_json_with_tabs;
use tsv_svelte_compile::{
    CompileError, CompileOptions, Parity, canonicalize_js, census, census_detected_buckets,
    compare_canonical, compile,
};

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
///
/// # `--census` (the sole-blocker refusal census)
///
/// tsv's `compile()` bails at the **first** unsupported construct, so the refusal
/// sub-buckets above are first-refusal-only and overstate any one class's parity
/// yield. `--census` re-prices them: over the same oracle-accepted, tsv-refused
/// files, it unions each file's real first-refusal with
/// [`census`](tsv_svelte_compile::census)'s independently-detected blocker set,
/// then reports — per class — its **sole-blocker** count (files it is the *only*
/// blocker of, so unlocking it yields exactly that many new parity files) and its
/// **co-blocker** count. A mandatory **exposure** line counts candidates whose
/// first-refusal is a class the census cannot detect independently
/// ([`census_detected_buckets`](tsv_svelte_compile::census_detected_buckets)) —
/// those files may hide an undetected co-blocker, so their sole counts could be
/// over-promised. Diagnostic only: it exits 0 unless a harness error occurs (2).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compile_corpus_compare")]
pub struct CompileCorpusCompareCommand {
    /// list the discovered in-scope `.svelte` files without comparing
    #[argh(switch)]
    list: bool,

    /// re-price the refusal buckets: per class, sole-blocker vs co-blocker counts
    /// over the oracle-accepted, tsv-refused files (diagnostic; see the command
    /// docs)
    #[argh(switch)]
    census: bool,

    /// emit a machine-readable JSON report
    #[argh(switch)]
    json: bool,

    /// directories or files to compare (each a reported group)
    #[argh(positional)]
    paths: Vec<String>,
}

/// One file's classification.
enum Bucket {
    /// Both sides compiled and the canonical forms matched. `tolerated` records
    /// that JS parity was reached only by tolerating a comment-POSITION difference
    /// (`compare_canonical` → [`Parity::CommentPosition`]), not byte-exactness.
    Parity { tolerated: bool },
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
        if self.census {
            rt.block_on(self.run_census_async(items))
        } else {
            rt.block_on(self.run_async(groups, items))
        }
    }

    async fn run_census_async(self, items: Vec<(usize, PathBuf)>) -> Result<(), CliError> {
        let total = items.len();
        let mut stream = super::spawn_work_stream(
            items,
            super::ResultOrder::Completion,
            |(_group, path)| async move { classify_census_file(path).await },
        );
        let mut outcomes = Vec::with_capacity(total);
        while let Some(joined) = stream.next().await {
            outcomes.push(super::task_result(joined, "compile-census")?);
        }

        let report = CensusReport::build(&outcomes);
        if self.json {
            report.print_json()?;
        } else {
            report.print_human();
        }
        // Diagnostic only — a harness error is the sole non-zero exit.
        if report.errors > 0 {
            Err(CliError::Errored)
        } else {
            Ok(())
        }
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

    // The parity bar tolerates comment-POSITION differences (tsv's comment
    // philosophy vs the oracle's esrap placement) — same code, same comment
    // sequence, no bundler annotation. A dropped/doubled/reordered/content-changed
    // comment, or any code difference, stays a MISMATCH. See `compare_canonical`.
    let js_parity = compare_canonical(&ours_canon, &oracle_canon);
    let ours_css = ours.css.as_deref().map(with_trailing_newline);
    let oracle_css = oracle.css.as_deref().map(with_trailing_newline);
    let css_match = ours_css == oracle_css;
    if js_parity.is_parity() && css_match {
        return Bucket::Parity {
            tolerated: js_parity == Parity::CommentPosition,
        };
    }

    let mut diff = String::new();
    if !js_parity.is_parity() {
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
    /// Subset of `parity` reached by tolerating a comment-position difference
    /// (not byte-exact). Surfaced so the tolerance is never silent.
    comment_position: usize,
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
    /// Files that reached parity only by tolerating a comment-position difference
    /// — `(group, path)`. Not a bug (tsv's comment placement), but surfaced so the
    /// tolerance is visible.
    comment_position: Vec<(String, String)>,
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
        let mut comment_position: Vec<(String, String)> = Vec::new();
        let mut errors = Vec::new();
        let mut errors_truncated = 0;

        let root_of = |gi: usize| groups[gi].root.clone();

        for o in outcomes {
            let gs = &mut group_stats[o.group];
            let path = o.path.display().to_string();
            match &o.bucket {
                Bucket::Parity { tolerated } => {
                    totals.parity += 1;
                    gs.parity += 1;
                    if *tolerated {
                        totals.comment_position += 1;
                        gs.comment_position += 1;
                        comment_position.push((root_of(o.group), path));
                    }
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

        // Deterministic order: mismatches/errors/tolerated by path.
        mismatches.sort_by(|a, b| a.path.cmp(&b.path));
        comment_position.sort_by(|a, b| a.1.cmp(&b.1));
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
            comment_position,
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

        if !self.comment_position.is_empty() {
            println!(
                "\nComment-position tolerated ({}) — parity by tsv's comment placement, not a bug:",
                self.comment_position.len()
            );
            for (group, path) in &self.comment_position {
                println!("  [{group}] {path}");
            }
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
            comment_position: &'a [(String, String)],
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
            comment_position: &self.comment_position,
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

/// A one-line bucket summary. `comment_pos` is a subset of `parity` (tolerated
/// comment-position differences), shown in parentheses so it never inflates the
/// bucket totals.
fn stats_line(s: &Stats) -> String {
    format!(
        "files={} parity={} (comment_pos={}) refused={} oracle_rejected={} mismatch={} error={}",
        s.files, s.parity, s.comment_position, s.refused, s.oracle_rejected, s.mismatch, s.error
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

// ---- Census (--census) ------------------------------------------------------

/// One file's census outcome.
enum CensusOutcome {
    /// Not a parity candidate — the oracle rejected it, or tsv reached parity.
    /// Out of scope for the census (it only re-prices refused files).
    Skipped,
    /// An oracle-accepted, tsv-refused file: the real first-refusal bucket key and
    /// the union of it with the census's independently-detected blocker keys.
    Candidate {
        first_key: String,
        union: Vec<String>,
        /// Whether the census's own findings **for this file** reproduced the real
        /// first-refusal. This is a per-file fact, distinct from global
        /// class-detectability: a dual-position class (`Rune`/`DerivedBindingRead`/
        /// `TopLevelAwait`) is globally "detected" (the census reaches its SCRIPT
        /// variant) yet may go unconfirmed on a file whose first-refusal is the
        /// TEMPLATE variant the census never reaches. Exposure keys on this, not on
        /// the global set.
        first_confirmed: bool,
    },
    /// A harness failure: `(kind, detail)`.
    Error(&'static str, String),
}

/// Read and classify one file for the census (oracle-first, like [`classify`]).
async fn classify_census_file(path: PathBuf) -> CensusOutcome {
    match std::fs::read_to_string(&path) {
        Ok(source) => classify_census(&source).await,
        Err(e) => CensusOutcome::Error("read", e.to_string()),
    }
}

/// The per-file census pipeline. Only oracle-accepted **and** tsv-refused files
/// are candidates (the parity-yield population); everything else is skipped.
async fn classify_census(source: &str) -> CensusOutcome {
    // Oracle side. A coded rejection is out of census scope (never a parity
    // candidate); an uncoded ToolError / other DenoError is a harness failure.
    match deno::svelte_compile(source, SvelteGenerate::Server, false).await {
        Ok(_) => {}
        Err(DenoError::ToolError { message }) => {
            return match oracle_reject_code(&message) {
                Some(_) => CensusOutcome::Skipped,
                None => CensusOutcome::Error("oracle-tool", first_line(&message)),
            };
        }
        Err(e) => return CensusOutcome::Error("oracle-sidecar", e.to_string()),
    }

    // tsv side. Only a refusal is a candidate; parity is out of scope, and the
    // bug/parse outcomes are harness errors (as in `classify`).
    let first = match compile(source, &CompileOptions::default()) {
        Ok(_) => return CensusOutcome::Skipped,
        Err(CompileError::Unsupported(reason)) => reason,
        Err(CompileError::Parse(e)) => return CensusOutcome::Error("tsv-parse", e.to_string()),
        Err(CompileError::CorruptOutput(e)) => {
            return CensusOutcome::Error("tsv-corrupt-output", e.to_string());
        }
        Err(CompileError::TypeErasureLeak(span)) => {
            return CensusOutcome::Error("tsv-type-erasure-leak", format!("at {span:?}"));
        }
    };

    // Census pass. It parsed once already inside `compile()`, so a parse error is
    // impossible here — surface it loudly if it somehow occurs.
    let detected = match census(source, &CompileOptions::default()) {
        Ok(d) => d,
        Err(e) => return CensusOutcome::Error("census", e.to_string()),
    };
    let first_key = first.bucket_key().into_owned();
    // Did the census's OWN findings reproduce this file's real first-refusal? This
    // is what exposure keys on — a global "is this class ever detectable" check
    // wrongly clears a dual-position class whose template variant the census never
    // reaches (its script variant shares the bucket key).
    let first_confirmed = detected
        .iter()
        .any(|reason| reason.bucket_key().as_ref() == first_key.as_str());
    let mut union = vec![first_key.clone()];
    for reason in &detected {
        let key = reason.bucket_key().into_owned();
        if !union.contains(&key) {
            union.push(key);
        }
    }
    CensusOutcome::Candidate {
        first_key,
        union,
        first_confirmed,
    }
}

/// One class's sole/co counts.
#[derive(Default, serde::Serialize)]
struct BlockerCount {
    bucket: String,
    /// Files where this class is the ONLY blocker — unlocking it yields exactly
    /// this many new parity files.
    sole: usize,
    /// Files where this class blocks alongside at least one other.
    co: usize,
    /// Whether the census detects this class independently. When `false` the class
    /// only ever enters a file's blocker set as the real first-refusal, so its
    /// `sole` count is an **upper bound** (an undetected co-blocker could lower it).
    detected: bool,
}

/// A first-refusal class the census did not reproduce on some candidate, and how
/// many candidates it went unconfirmed on — the exposure detail.
#[derive(serde::Serialize)]
struct DisclaimedCount {
    bucket: String,
    count: usize,
}

/// The aggregated census report.
struct CensusReport {
    candidates: usize,
    errors: usize,
    blockers: Vec<BlockerCount>,
    /// Candidates whose real first-refusal the census did **not** itself reproduce
    /// (a per-file fact) — their sole counts could be over-promised.
    exposure: usize,
    disclaimed: Vec<DisclaimedCount>,
    error_entries: Vec<(String, String)>,
}

impl CensusReport {
    fn build(outcomes: &[CensusOutcome]) -> Self {
        let detected: std::collections::HashSet<String> = census_detected_buckets()
            .into_iter()
            .map(std::borrow::Cow::into_owned)
            .collect();

        let mut sole: BTreeMap<String, usize> = BTreeMap::new();
        let mut co: BTreeMap<String, usize> = BTreeMap::new();
        let mut disclaimed: BTreeMap<String, usize> = BTreeMap::new();
        let mut candidates = 0;
        let mut errors = 0;
        let mut exposure = 0;
        let mut error_entries = Vec::new();

        for outcome in outcomes {
            match outcome {
                CensusOutcome::Skipped => {}
                CensusOutcome::Error(kind, detail) => {
                    errors += 1;
                    if error_entries.len() < ERROR_CAP {
                        error_entries.push(((*kind).to_string(), detail.clone()));
                    }
                }
                CensusOutcome::Candidate {
                    first_key,
                    union,
                    first_confirmed,
                } => {
                    candidates += 1;
                    if union.len() == 1 {
                        *sole.entry(union[0].clone()).or_default() += 1;
                    } else {
                        for key in union {
                            *co.entry(key.clone()).or_default() += 1;
                        }
                    }
                    // Exposure is PER-FILE: the census did not itself reproduce this
                    // file's real first-refusal, so it may hide an undetected
                    // co-blocker (and, for a dual-position class, the first-refusal
                    // itself is a position the census never reached). A global
                    // class-detectability check would wrongly clear those.
                    if !first_confirmed {
                        exposure += 1;
                        *disclaimed.entry(first_key.clone()).or_default() += 1;
                    }
                }
            }
        }

        // Merge sole + co into one per-bucket row, sorted sole-desc then co-desc.
        let mut keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        keys.extend(sole.keys().cloned());
        keys.extend(co.keys().cloned());
        let mut blockers: Vec<BlockerCount> = keys
            .into_iter()
            .map(|bucket| BlockerCount {
                sole: sole.get(&bucket).copied().unwrap_or(0),
                co: co.get(&bucket).copied().unwrap_or(0),
                detected: detected.contains(&bucket),
                bucket,
            })
            .collect();
        blockers.sort_by(|a, b| {
            b.sole
                .cmp(&a.sole)
                .then_with(|| b.co.cmp(&a.co))
                .then_with(|| a.bucket.cmp(&b.bucket))
        });

        let mut disclaimed: Vec<DisclaimedCount> = disclaimed
            .into_iter()
            .map(|(bucket, count)| DisclaimedCount { bucket, count })
            .collect();
        disclaimed.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.bucket.cmp(&b.bucket)));

        CensusReport {
            candidates,
            errors,
            blockers,
            exposure,
            disclaimed,
            error_entries,
        }
    }

    fn print_human(&self) {
        println!(
            "compile_corpus_compare --census — {} oracle-accepted, tsv-refused candidate(s)\n",
            self.candidates
        );
        println!(
            "Per refusal class (sole = unlocking it yields exactly that many new parity files):"
        );
        println!("  {:>6}  {:>6}  refusal class", "SOLE", "CO");
        for b in &self.blockers {
            // A `*` marks a class the census cannot detect independently — its
            // sole count is an upper bound (see the exposure line below).
            let mark = if b.detected { "" } else { " *" };
            println!("  {:>6}  {:>6}  {}{}", b.sole, b.co, b.bucket, mark);
        }
        println!(
            "  (* = census cannot detect this class independently; its SOLE count is an upper bound)"
        );

        // The mandatory exposure line — per-file: how many candidates the census
        // could not confirm the first-refusal of (so an undetected co-blocker, or
        // an unreached dual-position first-refusal, may hide there).
        println!(
            "\nEXPOSURE: {} of {} candidate(s) — the census did not itself reproduce that file's \
             real first-refusal",
            self.exposure, self.candidates
        );
        println!(
            "  (an undetected co-blocker, or a dual-position first-refusal in a position the \
             census never reaches, may hide there)"
        );
        if !self.disclaimed.is_empty() {
            println!("  Unconfirmed first-refusal classes (count):");
            for d in &self.disclaimed {
                println!("    {:>6}  {}", d.count, d.bucket);
            }
        }
        println!(
            "\nDetected-sole counts RANK parity yield (order-of-magnitude); they are NOT literal \
             '+N parity' promises — an EARLY detected class can hide a LATE disclaimed co-blocker \
             the census never sees, so EXPOSURE is a lower bound on the over-promise population."
        );
        println!(
            "Disclaimed classes (`*`) are the static-evaluator/overlay family, the emitter \
             refusals that read live per-emission state (styled-component attributes, \
             bind:/event/value attributes, block placement, component invocations), and the \
             pipeline-inline comment-family refusals. See the tsv_svelte_compile::census module docs."
        );

        if self.errors > 0 {
            println!("\nErrors ({}):", self.errors);
            for (kind, detail) in &self.error_entries {
                let detail = if detail.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", truncate(&detail.replace('\n', " "), 160))
                };
                println!("  [{kind}]{detail}");
            }
            if self.errors > self.error_entries.len() {
                println!(
                    "  … (+{} more errors)",
                    self.errors - self.error_entries.len()
                );
            }
        }
    }

    fn print_json(&self) -> Result<(), CliError> {
        #[derive(serde::Serialize)]
        struct JsonReport<'a> {
            candidates: usize,
            errors: usize,
            exposure: usize,
            blockers: &'a [BlockerCount],
            disclaimed: &'a [DisclaimedCount],
        }
        let report = JsonReport {
            candidates: self.candidates,
            errors: self.errors,
            exposure: self.exposure,
            blockers: &self.blockers,
            disclaimed: &self.disclaimed,
        };
        match to_json_with_tabs(&report) {
            Ok(json) => {
                println!("{json}");
                Ok(())
            }
            Err(e) => {
                eprintln!("Error serializing census report: {e}");
                Err(CliError::Errored)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn census_exposure_is_per_file_not_global() {
        // The per-file exposure fix: a candidate is exposed iff the census did not
        // itself reproduce that file's real first-refusal (`first_confirmed`),
        // regardless of whether that class is globally detectable. This models a
        // dual-position class whose bucket key IS in `census_detected_buckets()`
        // (so a global check would clear it) yet went unconfirmed on this file.
        let dual = "rune {name}".to_string(); // globally detected (script variant)
        assert!(
            census_detected_buckets().iter().any(|b| b.as_ref() == dual),
            "precondition: the class is globally detectable"
        );
        let outcomes = vec![
            // First-refusal reproduced by the census → confirmed, NOT exposed.
            CensusOutcome::Candidate {
                first_key: dual.clone(),
                union: vec![dual.clone()],
                first_confirmed: true,
            },
            // Same class as first-refusal, but the census did not reproduce it on
            // this file (template variant) → exposed, despite being globally
            // detectable. A global-set check would wrongly clear it.
            CensusOutcome::Candidate {
                first_key: dual.clone(),
                union: vec![dual.clone(), "non-class css selector".to_string()],
                first_confirmed: false,
            },
        ];
        let report = CensusReport::build(&outcomes);
        assert_eq!(report.candidates, 2);
        assert_eq!(report.exposure, 1, "only the unconfirmed file is exposed");
        assert_eq!(report.disclaimed.len(), 1);
        assert_eq!(report.disclaimed[0].bucket, dual);
        assert_eq!(report.disclaimed[0].count, 1);
    }

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
