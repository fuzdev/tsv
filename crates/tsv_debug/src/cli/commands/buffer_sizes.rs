use argh::FromArgs;
use std::path::Path;

use crate::cli::CliError;
use crate::cli::commands::profile::resolve_profile_files;
use tsv_cli::cli::input::ParserType;
use tsv_lang::Comment;
use tsv_lang::estimated_ast_arena_capacity;
use tsv_ts::ast::internal::{ImportSpecifier, Statement};

/// Histogram the print-time buffer-size distributions used to tune the TS
/// printer's `SmallVec` inline capacities.
///
/// Two parse-time metrics, both static properties of the AST (load-independent,
/// no perf noise — the clean alternative to varying an inline `N` and
/// re-reading heaptrack spill counts):
///
/// - **import named-specifier count per import declaration** — sizes the
///   `named_specs` buffer in `statements/modules/mod.rs`.
/// - **line count per multi-line block comment** — the population the parked
///   line-offset scratch (`borrow_line_spans_scratch`) iterates in
///   `comments/render.rs`.
///
/// With the **`buffer_stats` feature** (off by default — the record hooks sit
/// in the chain printer's hot path), each file is additionally *formatted* and
/// four printer-buffer populations are sampled at their construction
/// chokepoints (`tsv_ts::printer::buffer_stats`), so inline-`N` claims about
/// them are measured data rather than doc-comment prose:
///
/// - **`ChainNodeVec`** — nodes per linearized chain
/// - **`ChainGroupVec`** — groups per `group_chain_nodes` call
/// - **`ChainGroup.nodes`** — nodes per built chain group
/// - **leading-comment `CommentVec`** — comments per
///   `collect_leading_comments` call (the type's dominant site)
///
/// The report labels carry each type's *current* inline `N`, read from the
/// types themselves (`tsv_ts::inline_capacities`) so they can't drift.
///
/// Covers **both** standalone TypeScript (`.ts` / `.svelte.ts`) **and** Svelte
/// `<script>` / `{expr}` content — the latter is formatted by the embedded TS
/// printer, so it feeds the *same* buffers and belongs in the *same*
/// distribution (no flag, no separate pass: a `.svelte`-excluding default would
/// undercount, since most of the corpus is `.svelte`). `.css` is skipped (CSS
/// has its own printer buffers). Imports come from top-level statements
/// (standalone bodies and the instance/module `<script>` programs); imports
/// inside `declare module` are rare and excluded.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "buffer_sizes")]
pub struct BufferSizesCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// file paths, directories, or glob patterns
    #[argh(positional)]
    paths: Vec<String>,
}

impl BufferSizesCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let (files, _skipped) = resolve_profile_files(&self.paths, |_| false)?;

        #[cfg(feature = "buffer_stats")]
        tsv_ts::set_buffer_stats(true);

        let mut named_specs: Vec<usize> = Vec::new();
        let mut comment_lines: Vec<usize> = Vec::new();
        let mut files_parsed = 0usize;
        let mut parse_errors = 0usize;

        for path in &files {
            let parser = ParserType::from_extension(&path.to_string_lossy());
            if parser == ParserType::Css {
                continue; // CSS uses its own printer buffers, not the TS ones
            }
            files_parsed += 1;
            if collect_file(path, parser, &mut named_specs, &mut comment_lines).is_err() {
                parse_errors += 1;
            }
        }

        if files_parsed == 0 {
            eprintln!("No TS/Svelte files found (.ts / .svelte.ts / .svelte).");
            return Ok(());
        }

        named_specs.sort_unstable();
        comment_lines.sort_unstable();

        #[cfg(feature = "buffer_stats")]
        let printer_stats = {
            let mut stats = tsv_ts::take_buffer_stats();
            stats.chain_nodes.sort_unstable();
            stats.chain_groups.sort_unstable();
            stats.group_nodes.sort_unstable();
            stats.leading_comments.sort_unstable();
            stats
        };

        if self.json {
            let fields = json_fields(&named_specs, &comment_lines, files_parsed, parse_errors);
            #[cfg(feature = "buffer_stats")]
            let fields = format!("{fields},{}", printer_stats_json_fields(&printer_stats));
            println!("{{{fields}}}");
        } else {
            print_report(&named_specs, &comment_lines, files_parsed, parse_errors);
            #[cfg(feature = "buffer_stats")]
            print_printer_stats(&printer_stats);
            #[cfg(not(feature = "buffer_stats"))]
            eprintln!("\n(chain/comment printer-buffer histograms need `--features buffer_stats`)");
        }
        Ok(())
    }
}

/// Parse one TS or Svelte file and push its buffer-size samples. Parse failures
/// return `Err(())` (counted by the caller) rather than aborting the corpus walk.
fn collect_file(
    path: &Path,
    parser: ParserType,
    named_specs: &mut Vec<usize>,
    comment_lines: &mut Vec<usize>,
) -> Result<(), ()> {
    let source = std::fs::read_to_string(path).map_err(|_| ())?;
    let arena = bumpalo::Bump::with_capacity(estimated_ast_arena_capacity(source.len()));

    let mut interner = tsv_lang::Interner::new();
    match parser {
        ParserType::TypeScript => {
            let ast = tsv_ts::parse(&source, &arena, &mut interner).map_err(|_| ())?;
            collect_imports(ast.body, named_specs);
            collect_comments(ast.comments, &source, comment_lines);
            // Run the real printer so the armed buffer_stats hooks sample the
            // actual chain/comment buffer populations. Output is discarded.
            #[cfg(feature = "buffer_stats")]
            drop(tsv_ts::format(&ast, &source, &interner));
        }
        ParserType::Svelte => {
            let root = tsv_svelte::parse(&source, &arena, &mut interner).map_err(|_| ())?;
            // The instance/module `<script>` programs feed the same import
            // printer; `Root.comments` unifies every script + `{expr}` comment
            // (the exact population `render.rs` renders).
            if let Some(script) = root.instance {
                collect_imports(script.content.body, named_specs);
            }
            if let Some(script) = root.module {
                collect_imports(script.content.body, named_specs);
            }
            collect_comments(&root.comments, &source, comment_lines);
            #[cfg(feature = "buffer_stats")]
            drop(tsv_svelte::format(&root, &source, &interner));
        }
        ParserType::Css => {}
    }
    Ok(())
}

/// Report the four printer-buffer populations sampled during the format runs.
/// Each label's inline `N` is read from the buffer type itself, so a re-tuned
/// capacity can't leave a stale label here.
#[cfg(feature = "buffer_stats")]
fn print_printer_stats(stats: &tsv_ts::BufferStats) {
    let caps = tsv_ts::inline_capacities();
    eprintln!();
    print_metric(
        &format!(
            "ChainNodeVec (nodes per linearized chain; inline N={})",
            caps.chain_nodes
        ),
        &stats.chain_nodes,
        &[4, 8, 12, 16],
    );
    eprintln!();
    print_metric(
        &format!(
            "ChainGroupVec (groups per group_chain_nodes call; inline N={})",
            caps.chain_groups
        ),
        &stats.chain_groups,
        &[2, 4, 6, 8],
    );
    eprintln!();
    print_metric(
        &format!(
            "ChainGroup.nodes (nodes per built chain group; inline N={})",
            caps.group_nodes
        ),
        &stats.group_nodes,
        &[2, 4, 6, 8],
    );
    eprintln!();
    print_metric(
        &format!(
            "CommentVec (comments per collect_leading_comments call; inline N={})",
            caps.leading_comments
        ),
        &stats.leading_comments,
        &[2, 4, 6, 8],
    );
}

/// The printer-buffer populations as JSON object fields (no braces), merged
/// into the single `--json` object after the parse-time fields.
#[cfg(feature = "buffer_stats")]
fn printer_stats_json_fields(stats: &tsv_ts::BufferStats) -> String {
    format!(
        "\"chain_nodes\":{},\"chain_groups\":{},\"group_nodes\":{},\"leading_comments\":{}",
        metric_json(&stats.chain_nodes),
        metric_json(&stats.chain_groups),
        metric_json(&stats.group_nodes),
        metric_json(&stats.leading_comments),
    )
}

/// Count Named specifiers per import declaration in a statement body.
fn collect_imports(body: &[Statement<'_>], named_specs: &mut Vec<usize>) {
    for stmt in body {
        if let Statement::ImportDeclaration(decl) = stmt {
            // Count every import: the buffer is created per-import regardless, so
            // 0-named (default/namespace-only) imports belong in the spill-rate
            // denominator.
            let n = decl
                .specifiers
                .iter()
                .filter(|s| matches!(s, ImportSpecifier::Named(_)))
                .count();
            named_specs.push(n);
        }
    }
}

/// Record the line count of every multi-line block comment (the `render.rs`
/// population): the comment's content split on `\n`.
fn collect_comments(comments: &[Comment], source: &str, comment_lines: &mut Vec<usize>) {
    for comment in comments {
        if comment.is_block && comment.multiline {
            comment_lines.push(comment.content(source).split('\n').count());
        }
    }
}

/// Value at percentile `p` (0..=100) of a pre-sorted slice (nearest-rank).
fn percentile(sorted: &[usize], p: usize) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    let idx = (p * (sorted.len() - 1) + 50) / 100;
    sorted[idx]
}

#[allow(clippy::cast_precision_loss)]
fn mean(sorted: &[usize]) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted.iter().sum::<usize>() as f64 / sorted.len() as f64
}

/// Fraction of samples strictly greater than `n` (the spill rate at inline `N`).
#[allow(clippy::cast_precision_loss)]
fn spill_rate(sorted: &[usize], n: usize) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let spilled = sorted.iter().filter(|&&v| v > n).count();
    spilled as f64 / sorted.len() as f64 * 100.0
}

fn print_report(named: &[usize], comments: &[usize], files: usize, parse_errors: usize) {
    eprintln!("Buffer-size histograms — {files} TS/Svelte files ({parse_errors} parse errors)\n");

    print_metric(
        "named_specs  (named-import-specifiers per import declaration)",
        named,
        &[4, 6, 8, 12],
    );
    eprintln!();
    print_metric(
        "CommentLines (lines per multi-line block comment)",
        comments,
        &[8, 12, 16, 24],
    );
}

fn print_metric(title: &str, sorted: &[usize], inline_candidates: &[usize]) {
    eprintln!("{title}");
    if sorted.is_empty() {
        eprintln!("  (no samples)");
        return;
    }
    eprintln!(
        "  n={}  min={}  p50={}  p90={}  p95={}  p99={}  max={}  mean={:.2}",
        sorted.len(),
        sorted[0],
        percentile(sorted, 50),
        percentile(sorted, 90),
        percentile(sorted, 95),
        percentile(sorted, 99),
        sorted[sorted.len() - 1],
        mean(sorted),
    );
    eprintln!("  spill rate at candidate inline N (% of samples that would heap-allocate):");
    for &n in inline_candidates {
        eprintln!("    N={n:<3} spill={:.2}%", spill_rate(sorted, n));
    }
}

/// One `{"n":…,"p50":…,…}` histogram-summary object for a sorted sample list.
fn metric_json(sorted: &[usize]) -> String {
    format!(
        "{{\"n\":{},\"p50\":{},\"p90\":{},\"p95\":{},\"p99\":{},\"max\":{},\"mean\":{:.4}}}",
        sorted.len(),
        percentile(sorted, 50),
        percentile(sorted, 90),
        percentile(sorted, 95),
        percentile(sorted, 99),
        sorted.last().copied().unwrap_or(0),
        mean(sorted),
    )
}

/// The parse-time metrics as JSON object fields (no braces), so the
/// feature-gated printer-buffer fields can merge into the same `--json` object.
fn json_fields(named: &[usize], comments: &[usize], files: usize, parse_errors: usize) -> String {
    format!(
        "\"files\":{files},\"parse_errors\":{parse_errors},\"named_specs\":{},\"comment_lines\":{}",
        metric_json(named),
        metric_json(comments),
    )
}
