//! The report-only by-(node, edge) rollup: folds the fine per-site shape tallies into a
//! ranked, coarse emitter work-list. Split out of `gap_audit.rs` for navigability.

use crate::audit::node_edge::NodeEdgeKey;

use super::Tally;

/// One cluster row in the ranked (worst-first) by-node work-list — one `(node, edge)` and its
/// EXACT per-site hit tally, read straight off [`Tally::node_edge_hits`].
struct ClusterRow {
    key: NodeEdgeKey,
    hits: usize,
    /// How many distinct site shapes landed in this cluster.
    shapes: usize,
    /// The lexicographically smallest shape in the cluster, shown as its example.
    example_shape: String,
}

/// The by-node rollup, shared by the human `--by-node` view and the `--json` section.
///
/// Every field is an EXACT per-site tally accumulated at record time (see
/// [`Tally::node_edge_hits`]) — no canonical-example approximation, so no agreement measure to
/// carry. The one residual caveat is the [`Self::unresolved_count`] tail (offsets that key to no
/// node), zero over `tests/fixtures`.
pub(super) struct ByNodeRollup {
    /// Clusters ranked worst-first (hits desc, then key).
    clusters: Vec<ClusterRow>,
    grand_total: usize,
    unresolved_count: usize,
    total_shapes: usize,
}

/// Turn the run's EXACT record-time `(node, edge)` tallies into the ranked cluster work-list.
///
/// Pure over [`Tally::node_edge_hits`] — no file I/O, no parse. Every hit was keyed to its own
/// site's `(node, edge)` at record time (in `audit_file`), so a shape occurring in several
/// structural contexts is split across them per hit, not attributed wholesale to one canonical
/// example. Report-only: it feeds neither the gate nor the exit code.
///
/// Only ever called when record-time keying was on (`--by-node` / `--json`), so every finding is
/// accounted exactly once — the conservation invariant `grand_total + unresolved_count == Σ shape
/// counts` must hold (asserted below).
pub(super) fn compute_by_node(total: &Tally) -> ByNodeRollup {
    let grand_total: usize = total.node_edge_hits.values().map(|c| c.hits).sum();
    let unresolved_count = total.node_edge_unresolved;

    // Every hit is keyed exactly once — into a cluster or the unresolved tail — so the two must
    // sum to the run's total finding count. A miskey (a hit counted twice, or dropped) is the
    // "corpus can't grade it" class: it would leave every formatted file byte-identical. A PLAIN
    // `assert_eq!` (not `debug_assert_eq!`) so it fires under `--profile corpus`/release too — the
    // very profile the `--by-node` / `--json` report path runs in, where a `debug_assert` elides
    // and a conservation break would ship as silently-wrong report data. Cheap to keep loud: this
    // runs at most once per invocation over ~156 clusters, never a hot loop, and `tsv_debug` is
    // dev-only (never prod wasm/cli/ffi). It guards COUNT conservation only; correct-cluster keying
    // rests on the `sites.rs` node-edge unit suite plus `compute_by_node_splits_…`.
    assert_eq!(
        grand_total + unresolved_count,
        total.shapes.values().map(|agg| agg.count).sum::<usize>(),
        "record-time keying must account every finding once: clusters + unresolved == Σ shape counts"
    );

    let mut clusters: Vec<ClusterRow> = total
        .node_edge_hits
        .iter()
        .map(|(key, accum)| ClusterRow {
            key: key.clone(),
            hits: accum.hits,
            shapes: accum.shapes.len(),
            // BTreeSet is sorted, so `.next()` is the lexicographically smallest shape. An accum
            // always carries ≥1 shape (it's created when a hit is folded), so the default is dead.
            example_shape: accum.shapes.iter().next().cloned().unwrap_or_default(),
        })
        .collect();
    // Worst-first: the fattest emitter cluster is the highest-leverage fix. Ties break on the
    // key, so the ranking is deterministic.
    clusters.sort_by(|a, b| b.hits.cmp(&a.hits).then_with(|| a.key.cmp(&b.key)));

    ByNodeRollup {
        clusters,
        grand_total,
        unresolved_count,
        total_shapes: total.shapes.len(),
    }
}

/// `n/d` as a whole-percent, `0` when `d == 0` — the human view's share formatter.
fn pct_of(n: usize, d: usize) -> usize {
    if d > 0 { n * 100 / d } else { 0 }
}

/// `n/d` as a fraction rounded to four decimals, `0.0` when `d == 0` — the JSON view's share.
///
/// Both operands are finding COUNTS — comfortably under 2^52, so the `f64` cast is exact and the
/// precision-loss lint (the whole-corpus-scale caveat) does not apply, exactly as
/// [`metrics`](crate::cli::commands::metrics) allows it for the same reason.
#[allow(clippy::cast_precision_loss)]
fn share_of(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        ((n as f64 / d as f64) * 1e4).round() / 1e4
    }
}

/// The audit-specific top-level `--json` section `report::print_json` folds in beside the
/// envelope: `by_node`, the ranked cluster work-list per-slice tooling consumes — now EXACT
/// per-site tallies, not a canonical approximation — plus `by_node_unresolved`, the count in the
/// UNRESOLVED tail (offsets that keyed to no node; zero over `tests/fixtures`). Additive — the
/// envelope's own fields are untouched.
pub(super) fn by_node_json_sections(
    rollup: &ByNodeRollup,
) -> serde_json::Map<String, serde_json::Value> {
    let by_node: Vec<serde_json::Value> = rollup
        .clusters
        .iter()
        .map(|c| {
            serde_json::json!({
                "node": c.key.node_type,
                "edge": c.key.edge,
                "hits": c.hits,
                "shapes": c.shapes,
                "share": share_of(c.hits, rollup.grand_total),
                "example_shape": c.example_shape,
            })
        })
        .collect();

    let mut m = serde_json::Map::new();
    m.insert("by_node".to_string(), serde_json::Value::Array(by_node));
    m.insert(
        "by_node_unresolved".to_string(),
        serde_json::json!(rollup.unresolved_count),
    );
    m
}

/// Print the COARSE by-(node, edge) rollup — a ranked emitter work-list of EXACT per-site tallies.
///
/// A finding whose offset keys to no node falls into the `UNRESOLVED` tail (reported, never fatal;
/// zero over `tests/fixtures`). Report-only: computed after grading, it feeds neither the gate nor
/// the exit code. Under `--json` it prints to stderr, leaving the JSON document on stdout the sole
/// parseable output.
pub(super) fn report_by_node(rollup: &ByNodeRollup, json: bool) {
    let mut lines: Vec<String> = Vec::new();
    let unresolved = if rollup.unresolved_count > 0 {
        format!("  ·  {} finding(s) UNRESOLVED", rollup.unresolved_count)
    } else {
        String::new()
    };
    lines.push(format!(
        "\nby-node — {} emitter cluster(s) over {} finding(s) across {} shape(s){unresolved}",
        rollup.clusters.len(),
        rollup.grand_total,
        rollup.total_shapes
    ));
    lines.push(String::new());
    for c in &rollup.clusters {
        let key = c.key.to_string();
        lines.push(format!(
            "  {:>7}×  {:>4} shape(s)  {key:<42}  e.g. {}",
            c.hits, c.shapes, c.example_shape
        ));
    }
    let top10: usize = rollup.clusters.iter().take(10).map(|c| c.hits).sum();
    lines.push(format!(
        "\ntop-10 cluster(s) cover {top10}/{} findings ({}%)",
        rollup.grand_total,
        pct_of(top10, rollup.grand_total)
    ));
    lines.push(
        "note: each finding is keyed to its own site's (node, edge) at record time, so these \
         totals are EXACT per-site tallies."
            .to_string(),
    );

    let out = lines.join("\n");
    if json {
        eprintln!("{out}");
    } else {
        println!("{out}");
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Hit, Kind, Payload};
    use super::*;

    /// A `(node, edge)` key spelled out, so the split test reads as the clusters it asserts.
    fn node_edge(node_type: &str, edge: &str) -> NodeEdgeKey {
        NodeEdgeKey {
            node_type: node_type.to_string(),
            edge: edge.to_string(),
        }
    }

    /// Record-time keying splits ONE site-shape across its DISTINCT `(node, edge)` clusters — the
    /// exact thing the retired canonical approximation got wrong (a fat generic shape like `␣⟨⟩␣`
    /// landing wholly on one cluster). Three hits share the site-shape `␣⟨⟩␣` but carry two
    /// different node-edge keys; the rollup must split the count 2/1 across the two clusters, not
    /// lump all three onto one. The "corpus can't grade it" class: a miskey would still leave every
    /// formatted file byte-identical, so only this unit test catches it.
    #[test]
    fn compute_by_node_splits_one_shape_across_its_clusters() {
        let call = node_edge("CallExpression", "arguments→$");
        let prop = node_edge("Property", "key→value");
        let mut tally = Tally::default();
        // `source = "a  b"`, offset 2 (between the two spaces) → the site-shape `␣⟨⟩␣`. All three
        // hits share it, but two key to the call cluster and one to the property cluster.
        for edge in [&call, &call, &prop] {
            tally.record(
                Hit {
                    kind: Kind::Dropped,
                    payload: Payload::Block,
                    path: "p.ts",
                    source: "a  b",
                    injection_offset: 2,
                    attribution_offset: 2,
                    text: "/* c */".to_string(),
                    injected: true,
                    node_edge: Some(edge.clone()),
                },
                true,
            );
        }

        // One site-shape recorded three hits …
        assert_eq!(
            tally.shapes.len(),
            1,
            "all three hits share the `␣⟨⟩␣` site-shape"
        );
        assert_eq!(tally.shapes[&(Kind::Dropped, "␣⟨⟩␣".to_string())].count, 3);

        // … yet the rollup splits them EXACTLY across two clusters (2/1), never lumped onto one.
        let rollup = compute_by_node(&tally);
        assert_eq!(rollup.grand_total, 3, "every hit is accounted");
        assert_eq!(rollup.unresolved_count, 0, "both keys resolved");
        assert_eq!(rollup.clusters.len(), 2, "the shape spans two clusters");
        // Worst-first: the call cluster (2 hits) ranks before the property cluster (1 hit).
        assert_eq!(rollup.clusters[0].key, call);
        assert_eq!(rollup.clusters[0].hits, 2);
        assert_eq!(rollup.clusters[1].key, prop);
        assert_eq!(rollup.clusters[1].hits, 1);
    }

    /// A four-decimal share compares within one ULP-ish epsilon — `assert_eq!` on `f64` trips
    /// clippy's `float_cmp` and is brittle regardless.
    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// `share_of` guards its zero denominator and rounds to four decimals; `pct_of` likewise.
    #[test]
    fn share_and_pct_guard_zero_denominator() {
        assert!(approx(share_of(0, 0), 0.0));
        assert_eq!(pct_of(0, 0), 0);
        assert!(approx(share_of(1, 3), 0.3333));
        assert!(approx(share_of(2, 4), 0.5));
        assert_eq!(pct_of(1, 3), 33);
    }
}
