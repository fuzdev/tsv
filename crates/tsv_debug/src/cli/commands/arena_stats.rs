use argh::FromArgs;
use std::path::Path;

use crate::cli::CliError;
use crate::cli::commands::profile::resolve_profile_files;
use tsv_cli::cli::input::ParserType;
use tsv_lang::doc::DocText;
use tsv_lang::doc::arena::{DocArena, DocNode};
use tsv_lang::estimated_ast_arena_capacity;

/// Histogram the `DocArena` node population produced by formatting a corpus — the
/// data behind the doc-IR memory levers (node-size shrink, the arena pre-size
/// heuristic).
///
/// Formats every file into a fresh arena and walks `borrow_nodes()`, reporting:
///
/// - **nodes/byte** — actual doc-node density vs the `with_source_size_hint`
///   heuristic (2/byte); the gap is the arena over-allocation. Includes per-file
///   density percentiles (p50/p90/p95/p99/max) — what a safe hint must clear.
/// - **capacity fill %** — used vs reserved node slots (how much of the pre-sized
///   `Vec` is dead reservation).
/// - **DocNode variant histogram** — which node kinds dominate the arena `Vec`
///   the render/`fits`/build loops linearly scan (so shrinking the dominant
///   variant's size is what would move cache density).
/// - **DocText sub-histogram** — for `Text` nodes, the `Static` / `Owned` /
///   `SourceSpan` / `Symbol` split (which text representation to target).
///
/// Covers `.ts` / `.svelte.ts` / `.svelte` / `.css` (each formatted by its own
/// printer into the shared doc arena). Pure Rust, no Deno.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "arena_stats")]
pub struct ArenaStatsCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// measure the `reset()`-reuse high-water instead: format every file into ONE
    /// arena (reset between files, as the CLI/FFI/WASM multi-file drivers do) and
    /// report the peak retained node/children capacity — the gate that a lower
    /// pre-size hint doesn't grow the batch reuse footprint
    #[argh(switch)]
    reuse: bool,

    /// file paths, directories, or glob patterns
    #[argh(positional)]
    paths: Vec<String>,
}

/// Fixed DocNode variant order (stable output; matches `classify_node`).
const NODE_KINDS: &[&str] = &[
    "Text",
    "Concat",
    "Line",
    "Group",
    "Indent",
    "Dedent",
    "Fill",
    "IfBreak",
    "IndentIfBreak",
    "WithContext",
    "MultilineText",
    "LineSuffix",
    "LineSuffixBoundary",
    "BreakParent",
    "IsolatedGroup",
    "Align",
];
const TEXT_KINDS: &[&str] = &["Static", "Owned", "SourceSpan", "Symbol"];

#[derive(Default)]
struct Stats {
    files: u64,
    bytes: u64,
    nodes: u64,
    capacity: u64,
    children: u64,
    children_capacity: u64,
    /// Per-file densities (nodes|children / byte) — the distribution a safe
    /// pre-size hint must clear at its chosen percentile so the dense tail doesn't
    /// realloc (the mean under-provisions it). Sorted before reporting.
    node_density: Vec<f64>,
    children_density: Vec<f64>,
    node_hist: std::collections::HashMap<&'static str, u64>,
    text_hist: std::collections::HashMap<&'static str, u64>,
}

impl ArenaStatsCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let (files, _skipped) = resolve_profile_files(&self.paths, |_| false)?;

        if self.reuse {
            return run_reuse(&files);
        }

        let mut stats = Stats::default();
        let mut parse_errors = 0usize;

        for path in &files {
            let parser = ParserType::from_extension(&path.to_string_lossy());
            if collect_file(path, parser, &mut stats).is_err() {
                parse_errors += 1;
            }
        }

        if stats.files == 0 {
            eprintln!("No formattable files found (.ts / .svelte.ts / .svelte / .css).");
            return Ok(());
        }

        stats.node_density.sort_by(f64::total_cmp);
        stats.children_density.sort_by(f64::total_cmp);

        if self.json {
            print_json(&stats, parse_errors);
        } else {
            print_report(&stats, parse_errors);
        }
        Ok(())
    }
}

/// Format every file through ONE arena (reset between files, as the multi-file
/// CLI/FFI/WASM drivers do) and report the peak retained node/children `Vec`
/// capacity — the `reset()` high-water. It is bounded by the single largest file's
/// actual usage (not the per-file hint), so lowering the pre-size hint can only
/// leave it flat or shrink it; this prints the number that proves it.
fn run_reuse(files: &[std::path::PathBuf]) -> Result<(), CliError> {
    let mut arena: Option<DocArena> = None;
    let (mut max_node_cap, mut max_child_cap) = (0usize, 0usize);
    let (mut max_node_len, mut max_child_len) = (0usize, 0usize);
    let (mut n, mut parse_errors) = (0u64, 0usize);

    for path in files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        match &mut arena {
            None => arena = Some(DocArena::for_source(&source)),
            Some(a) => a.reset(),
        }
        // `arena` is `Some` after the match above; the `else` never fires (it
        // just avoids an `expect`/`unwrap` on the hot path).
        let Some(a) = arena.as_ref() else { continue };
        let bump = bumpalo::Bump::with_capacity(estimated_ast_arena_capacity(source.len()));
        let ok = match ParserType::from_extension(&path.to_string_lossy()) {
            ParserType::TypeScript => tsv_ts::parse(&source, &bump)
                .map(|ast| {
                    let _ = tsv_ts::format_in(&ast, &source, a);
                })
                .is_ok(),
            ParserType::Svelte => tsv_svelte::parse(&source, &bump)
                .map(|ast| {
                    let _ = tsv_svelte::format_in(&ast, &source, a);
                })
                .is_ok(),
            ParserType::Css => tsv_css::parse(&source, &bump)
                .map(|ast| {
                    let _ = tsv_css::format_in(&ast, &source, a);
                })
                .is_ok(),
        };
        if !ok {
            parse_errors += 1;
            continue;
        }
        n += 1;
        let nodes = a.borrow_nodes();
        let children = a.borrow_children();
        max_node_cap = max_node_cap.max(nodes.capacity());
        max_node_len = max_node_len.max(nodes.len());
        max_child_cap = max_child_cap.max(children.capacity());
        max_child_len = max_child_len.max(children.len());
    }

    let node_bytes = size_of::<DocNode>();
    let retained = max_node_cap * node_bytes + max_child_cap * size_of::<u32>();
    eprintln!("reset()-reuse high-water — {n} files ({parse_errors} parse errors)\n");
    eprintln!(
        "  nodes:    peak used {max_node_len}  / retained cap {max_node_cap}  (slack {:.1}%)",
        pct(
            (max_node_cap - max_node_len) as u64,
            max_node_cap.max(1) as u64
        )
    );
    eprintln!(
        "  children: peak used {max_child_len}  / retained cap {max_child_cap}  (slack {:.1}%)",
        pct(
            (max_child_cap - max_child_len) as u64,
            max_child_cap.max(1) as u64
        )
    );
    eprintln!(
        "  retained arena footprint ≈ {retained} B  ({} B nodes + {} B children)",
        max_node_cap * node_bytes,
        max_child_cap * size_of::<u32>()
    );
    Ok(())
}

/// Format one file into a fresh arena and fold its node population into `stats`.
/// Parse failures return `Err(())` (counted by the caller), never abort the walk.
#[allow(clippy::cast_precision_loss)]
fn collect_file(path: &Path, parser: ParserType, stats: &mut Stats) -> Result<(), ()> {
    let source = std::fs::read_to_string(path).map_err(|_| ())?;
    let bump = bumpalo::Bump::with_capacity(estimated_ast_arena_capacity(source.len()));
    let arena = DocArena::for_source(&source);

    match parser {
        ParserType::TypeScript => {
            let ast = tsv_ts::parse(&source, &bump).map_err(|_| ())?;
            let _ = tsv_ts::format_in(&ast, &source, &arena);
        }
        ParserType::Svelte => {
            let ast = tsv_svelte::parse(&source, &bump).map_err(|_| ())?;
            let _ = tsv_svelte::format_in(&ast, &source, &arena);
        }
        ParserType::Css => {
            let ast = tsv_css::parse(&source, &bump).map_err(|_| ())?;
            let _ = tsv_css::format_in(&ast, &source, &arena);
        }
    }

    let nodes = arena.borrow_nodes();
    let children = arena.borrow_children();
    let len = source.len().max(1) as f64;
    stats.files += 1;
    stats.bytes += source.len() as u64;
    stats.nodes += nodes.len() as u64;
    stats.capacity += nodes.capacity() as u64;
    stats.children += children.len() as u64;
    stats.children_capacity += children.capacity() as u64;
    stats.node_density.push(nodes.len() as f64 / len);
    stats.children_density.push(children.len() as f64 / len);
    for n in nodes.iter() {
        *stats.node_hist.entry(classify_node(n)).or_default() += 1;
        if let DocNode::Text(t) = n {
            *stats.text_hist.entry(classify_text(t)).or_default() += 1;
        }
    }
    Ok(())
}

fn classify_node(n: &DocNode) -> &'static str {
    match n {
        DocNode::Text(_) => "Text",
        DocNode::MultilineText(_) => "MultilineText",
        DocNode::Line(_) => "Line",
        DocNode::Indent(_) => "Indent",
        DocNode::Dedent(_) => "Dedent",
        DocNode::Align { .. } => "Align",
        DocNode::Group { .. } => "Group",
        DocNode::IfBreak { .. } => "IfBreak",
        DocNode::IndentIfBreak { .. } => "IndentIfBreak",
        DocNode::Concat(_) => "Concat",
        DocNode::Fill(_) => "Fill",
        DocNode::WithContext { .. } => "WithContext",
        DocNode::LineSuffix(_) => "LineSuffix",
        DocNode::LineSuffixBoundary => "LineSuffixBoundary",
        DocNode::BreakParent => "BreakParent",
        DocNode::IsolatedGroup { .. } => "IsolatedGroup",
    }
}

fn classify_text(t: &DocText) -> &'static str {
    match t {
        DocText::Static(..) => "Static",
        DocText::Owned(..) => "Owned",
        DocText::SourceSpan(..) => "SourceSpan",
        DocText::Symbol(..) => "Symbol",
    }
}

#[allow(clippy::cast_precision_loss)]
fn pct(part: u64, whole: u64) -> f64 {
    part as f64 * 100.0 / whole.max(1) as f64
}

/// Value at percentile `p` (0..=100) of a pre-sorted slice (nearest-rank).
fn percentile(sorted: &[f64], p: usize) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p * (sorted.len() - 1) + 50) / 100;
    sorted[idx]
}

/// One-line percentile summary of a per-file density distribution.
fn density_line(label: &str, sorted: &[f64]) {
    eprintln!(
        "    {label}: p50={:.2} p90={:.2} p95={:.2} p99={:.2} max={:.2}",
        percentile(sorted, 50),
        percentile(sorted, 90),
        percentile(sorted, 95),
        percentile(sorted, 99),
        sorted.last().copied().unwrap_or(0.0),
    );
}

#[allow(clippy::cast_precision_loss)]
fn print_report(s: &Stats, parse_errors: usize) {
    eprintln!(
        "DocArena node stats — {} files, {} bytes ({parse_errors} parse errors)\n",
        s.files, s.bytes
    );
    eprintln!(
        "  nodes:    {:>9}  ({:.3}/byte mean; heuristic pre-sizes 2/byte)",
        s.nodes,
        s.nodes as f64 / s.bytes.max(1) as f64,
    );
    eprintln!(
        "            reserved {}  → fill {:.1}%  (over-alloc {:.1}×)",
        s.capacity,
        pct(s.nodes, s.capacity),
        s.capacity as f64 / s.nodes.max(1) as f64
    );
    density_line("per-file nodes/byte   ", &s.node_density);
    eprintln!(
        "  children: {:>9}  ({:.3}/byte mean; heuristic pre-sizes 1/byte)",
        s.children,
        s.children as f64 / s.bytes.max(1) as f64,
    );
    eprintln!(
        "            reserved {}  → fill {:.1}%  (over-alloc {:.1}×)",
        s.children_capacity,
        pct(s.children, s.children_capacity),
        s.children_capacity as f64 / s.children.max(1) as f64
    );
    density_line("per-file children/byte", &s.children_density);
    eprintln!();

    eprintln!("  DocNode variants (share of all nodes):");
    for kind in NODE_KINDS {
        if let Some(&c) = s.node_hist.get(kind) {
            eprintln!("    {kind:>18} {c:>10}  {:5.1}%", pct(c, s.nodes));
        }
    }
    let text_total: u64 = s.text_hist.values().sum();
    eprintln!("\n  DocText sub-variants (share of Text = {text_total} nodes):");
    for kind in TEXT_KINDS {
        if let Some(&c) = s.text_hist.get(kind) {
            eprintln!(
                "    {kind:>18} {c:>10}  {:5.1}% of Text  ({:5.1}% of all)",
                pct(c, text_total),
                pct(c, s.nodes)
            );
        }
    }
}

fn print_json(s: &Stats, parse_errors: usize) {
    let hist_json = |kinds: &[&str], h: &std::collections::HashMap<&'static str, u64>| {
        let entries: Vec<String> = kinds
            .iter()
            .filter_map(|k| h.get(k).map(|c| format!("\"{k}\":{c}")))
            .collect();
        format!("{{{}}}", entries.join(","))
    };
    let density_json = |sorted: &[f64]| {
        format!(
            "{{\"p50\":{:.4},\"p90\":{:.4},\"p95\":{:.4},\"p99\":{:.4},\"max\":{:.4}}}",
            percentile(sorted, 50),
            percentile(sorted, 90),
            percentile(sorted, 95),
            percentile(sorted, 99),
            sorted.last().copied().unwrap_or(0.0),
        )
    };
    println!(
        "{{\"files\":{},\"bytes\":{},\"nodes\":{},\"capacity\":{},\"children\":{},\"children_capacity\":{},\"node_density\":{},\"children_density\":{},\"parse_errors\":{parse_errors},\"node_variants\":{},\"text_variants\":{}}}",
        s.files,
        s.bytes,
        s.nodes,
        s.capacity,
        s.children,
        s.children_capacity,
        density_json(&s.node_density),
        density_json(&s.children_density),
        hist_json(NODE_KINDS, &s.node_hist),
        hist_json(TEXT_KINDS, &s.text_hist),
    );
}
