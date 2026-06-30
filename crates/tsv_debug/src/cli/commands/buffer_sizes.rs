use argh::FromArgs;
use std::path::Path;

use crate::cli::CliError;
use crate::cli::commands::profile::resolve_profile_files;
use tsv_cli::cli::input::ParserType;
use tsv_lang::Comment;
use tsv_lang::estimated_ast_arena_capacity;
use tsv_ts::ast::internal::{ImportSpecifier, Statement};

/// Histogram the print-time buffer-size distributions used to tune the TS
/// printer's `SmallVec` inline capacities (`named_specs`, `CommentLines`) and to
/// size the future context-indenting multiline-text doc node.
///
/// Two metrics, both static properties of the AST (load-independent, no perf
/// noise — the clean alternative to varying an inline `N` and re-reading
/// heaptrack spill counts):
///
/// - **import named-specifier count per import declaration** — sizes the
///   `named_specs` buffer in `statements/modules/mod.rs`.
/// - **line count per multi-line block comment** — sizes the `CommentLines`
///   buffer in `comments/render.rs`, and is the input distribution for the
///   per-line-`String` lever (the dominant comment alloc).
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

        if self.json {
            print_json(&named_specs, &comment_lines, files_parsed, parse_errors);
        } else {
            print_report(&named_specs, &comment_lines, files_parsed, parse_errors);
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

    match parser {
        ParserType::TypeScript => {
            let ast = tsv_ts::parse(&source, &arena).map_err(|_| ())?;
            collect_imports(ast.body, named_specs);
            collect_comments(&ast.comments, &source, comment_lines);
        }
        ParserType::Svelte => {
            let root = tsv_svelte::parse(&source, &arena).map_err(|_| ())?;
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
        }
        ParserType::Css => {}
    }
    Ok(())
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

fn print_json(named: &[usize], comments: &[usize], files: usize, parse_errors: usize) {
    let metric_json = |sorted: &[usize]| {
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
    };
    println!(
        "{{\"files\":{files},\"parse_errors\":{parse_errors},\"named_specs\":{},\"comment_lines\":{}}}",
        metric_json(named),
        metric_json(comments),
    );
}
