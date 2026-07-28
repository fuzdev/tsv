use super::*;
use tsv_lang::Span;

// --- FlowProduct -----------------------------------------------------------

/// Small construction counters, surfaced for the density / dead-label-row
/// perf report (they are not consumed by any checker phase).
#[derive(Clone, Copy, Debug, Default)]
pub struct FlowStats {
    /// Branch labels created (`createBranchLabel`).
    pub branch_labels: u32,
    /// Branch labels that collapsed at `finishFlowLabel` (0 or 1 antecedent),
    /// leaving a dead row — the fraction to watch (INTERNALS §Flow graph).
    pub dead_labels: u32,
}

/// The owned, arena-free, file-local flow product carried **dark** in a
/// `BoundUnit` (nothing consumes it until F3; F1a builds it and `--dump-flow`
/// renders it). C15-relocatable by construction.
pub struct FlowProduct {
    /// The flow graph.
    pub graph: FlowGraph,
    /// Per-`NodeId` flow attachment (`None` where tsgo attaches nil — including
    /// non-leaf nodes cleared in dead code; a dead *leaf* keeps
    /// `Some(unreachable)`).
    pub flow_of_node: Vec<Option<FlowNodeId>>,
    /// Per-node flag bytes, one per [`NodeId`] (minted zeroed here — the flow
    /// walk is the sole writer today), with the `Unreachable` bit set during the
    /// dead-code walk (`NODE_FLAGS_UNREACHABLE`).
    pub node_flags: Vec<u8>,
    /// Function-body + `SourceFile` end-of-flow anchors (binder.go:1561,1569),
    /// sorted by `NodeId`.
    pub end_flow: Vec<(NodeId, FlowNodeId)>,
    /// Constructor + class-static-block return-flow anchors ONLY
    /// (binder.go:1575), sorted by `NodeId`. Every other tsgo `ReturnFlowNode`
    /// write/read is dead plumbing and is not ported.
    pub return_flow: Vec<(NodeId, FlowNodeId)>,
    /// Case-clause fallthrough anchors: the reachable exit flow of each non-last
    /// clause (tsgo's `clause.AsCaseOrDefaultClause().FallthroughFlowNode`,
    /// binder.go:2121), sorted by `NodeId`.
    pub fallthrough_flow: Vec<(NodeId, FlowNodeId)>,
    /// Construction counters.
    pub stats: FlowStats,
}

impl FlowProduct {
    /// The `end_flow` anchor for a node, if any (small sorted anchor list).
    #[must_use]
    pub fn end_flow_of(&self, node: NodeId) -> Option<FlowNodeId> {
        self.end_flow
            .binary_search_by_key(&node, |&(n, _)| n)
            .ok()
            .map(|i| self.end_flow[i].1)
    }

    /// The `return_flow` anchor for a node, if any (constructor / static block).
    #[must_use]
    pub fn return_flow_of(&self, node: NodeId) -> Option<FlowNodeId> {
        self.return_flow
            .binary_search_by_key(&node, |&(n, _)| n)
            .ok()
            .map(|i| self.return_flow[i].1)
    }

    /// The `fallthrough_flow` anchor for a case clause, if any (the reachable
    /// exit flow of a non-last clause).
    #[must_use]
    pub fn fallthrough_flow_of(&self, node: NodeId) -> Option<FlowNodeId> {
        self.fallthrough_flow
            .binary_search_by_key(&node, |&(n, _)| n)
            .ok()
            .map(|i| self.fallthrough_flow[i].1)
    }
}

// --- DOT renderer (formatControlFlowGraph reference) -----------------------

/// Render one unit's flow graph to Graphviz DOT — the `--dump-flow` product.
/// Backward DFS from the `SourceFile`/function end-of-flow anchors (and return
/// anchors) with cycle detection, after Strada's `formatControlFlowGraph`
/// (flag→header label, subject-node source text, backward edges). `node_spans`
/// is the F0 `BoundFile::spans` column (subject text = `source[span]`).
#[must_use]
pub fn render_flow_dot(product: &FlowProduct, node_spans: &[Span], source: &str) -> String {
    use std::fmt::Write as _;
    let g = &product.graph;
    let mut out = String::new();
    out.push_str("digraph flow {\n");
    out.push_str("  rankdir=BT;\n");
    out.push_str("  node [shape=box, fontname=\"monospace\"];\n");

    let mut seen = vec![false; g.node_count() as usize + 1];
    let mut stack: Vec<FlowNodeId> = Vec::new();
    // Roots: every end_flow / return_flow anchor (the exits), plus id 1 so a
    // fully-unreachable graph still renders the singleton.
    for &(_, f) in product.end_flow.iter().chain(product.return_flow.iter()) {
        stack.push(f);
    }
    stack.push(FlowNodeId::UNREACHABLE);

    while let Some(id) = stack.pop() {
        if seen[id.index() + 1] {
            continue;
        }
        seen[id.index() + 1] = true;
        let label = flow_node_label(g, id, node_spans, source);
        let _ = writeln!(out, "  N{} [label=\"{}\"];", id.get(), escape_dot(&label));
        for ante in g.antecedents_iter(id) {
            let _ = writeln!(out, "  N{} -> N{};", id.get(), ante.get());
            stack.push(ante); // cycle-guarded by `seen`
        }
    }

    // Anchor edges (dashed) so the exits are visible.
    for (node, f) in &product.end_flow {
        let _ = writeln!(
            out,
            "  END_{n} [shape=doublecircle, label=\"end#{n}\"];\n  END_{n} -> N{f} [style=dashed];",
            n = node.get(),
            f = f.get()
        );
    }
    out.push_str("}\n");
    out
}

fn flow_node_label(g: &FlowGraph, id: FlowNodeId, node_spans: &[Span], source: &str) -> String {
    let flags = g.flags(id);
    let header = flow_flag_header(flags);
    if flags.contains(FlowFlags::REDUCE_LABEL) {
        // The `subject` slot is a payload index, not a NodeId — read the target
        // through the payload, never subject().
        let data = g.reduce_label_data(id);
        return format!("#{} {}→N{}", id.get(), header, data.target.get());
    }
    if flags.contains(FlowFlags::SWITCH_CLAUSE) {
        // A SwitchClause node's `subject` slot is a payload index, not a NodeId —
        // read the switch text + clause range through the payload, never subject().
        let data = g.switch_clause_data(id);
        let span = node_spans[data.switch.index()];
        let text = span.extract(source);
        let text = text.split('\n').next().unwrap_or(text);
        let text = match text.char_indices().nth(24) {
            Some((idx, _)) => &text[..idx],
            None => text,
        };
        return format!(
            "#{} {}[{},{}): {}",
            id.get(),
            header,
            data.clause_start,
            data.clause_end,
            text
        );
    }
    if let Some(node) = g.subject(id) {
        let span = node_spans[node.index()];
        let text = span.extract(source);
        let text = text.split('\n').next().unwrap_or(text);
        // Truncate on a char boundary (byte-slicing `&text[..32]` panics when a
        // multibyte char straddles byte 32).
        let text = match text.char_indices().nth(32) {
            Some((idx, _)) => &text[..idx],
            None => text,
        };
        format!("#{} {}: {}", id.get(), header, text)
    } else {
        format!("#{} {}", id.get(), header)
    }
}

/// The most salient flag as a short header label (label/condition/start/…).
fn flow_flag_header(flags: FlowFlags) -> &'static str {
    if flags.contains(FlowFlags::UNREACHABLE) {
        "unreachable"
    } else if flags.contains(FlowFlags::START) {
        "start"
    } else if flags.contains(FlowFlags::LOOP_LABEL) {
        "loop"
    } else if flags.contains(FlowFlags::BRANCH_LABEL) {
        "branch"
    } else if flags.contains(FlowFlags::ASSIGNMENT) {
        "assign"
    } else if flags.contains(FlowFlags::TRUE_CONDITION) {
        "true"
    } else if flags.contains(FlowFlags::FALSE_CONDITION) {
        "false"
    } else if flags.contains(FlowFlags::SWITCH_CLAUSE) {
        "switch"
    } else if flags.contains(FlowFlags::REDUCE_LABEL) {
        "reduce"
    } else if flags.contains(FlowFlags::CALL) {
        "call"
    } else {
        "flow"
    }
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
