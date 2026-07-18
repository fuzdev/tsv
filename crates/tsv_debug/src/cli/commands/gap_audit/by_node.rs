//! The report-only by-(node, edge) rollup: folds the fine per-site shape tallies into a
//! ranked, coarse emitter work-list. Split out of `gap_audit.rs` for navigability.

use std::collections::{BTreeMap, BTreeSet};

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

/// `n/d` as tenths-of-a-percent (`9.6`), `0.0` when `d == 0` — the rank table's share column,
/// one decimal finer than [`pct_of`] so the sub-10% head is legible. Integer math (both operands
/// are finding counts), so no `f64` cast.
///
/// The last of three deliberately-distinct share formatters, each pinned to one output shape:
/// [`pct_of`] (whole-percent `usize`, the human view), [`share_of`] (4-decimal `f64` fraction, the
/// JSON view), and this one (one-decimal string, the markdown table). The outputs differ, so
/// they're not unified — a fourth caller would be the moment to parameterize rather than add a
/// fourth.
fn tenths_pct(n: usize, d: usize) -> String {
    let permille = if d > 0 { n * 1000 / d } else { 0 };
    format!("{}.{}", permille / 10, permille % 10)
}

/// Print the top-`top` by-(node, edge) clusters as a **paste-ready markdown table** for
/// TODO_GAPS §Status — the ranked emitter work-list Phase 1 reads fattest-first, so a session can
/// refresh §Status by pasting instead of parsing `--json` and hand-transcribing. Report-only:
/// computed after grading, it feeds neither the gate nor the exit code. Under `--json` it prints to
/// stderr, leaving the JSON document on stdout the sole parseable output.
pub(super) fn report_rank(rollup: &ByNodeRollup, top: usize, json: bool) {
    let n = top.min(rollup.clusters.len());
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "\n**Top {n} by-(node, edge) clusters** ({} findings / {} shapes / {} clusters):\n",
        rollup.grand_total,
        rollup.total_shapes,
        rollup.clusters.len()
    ));
    lines.push("| # | cluster | hits | shapes | share |".to_string());
    lines.push("| ---: | --- | ---: | ---: | ---: |".to_string());
    for (i, c) in rollup.clusters.iter().take(n).enumerate() {
        lines.push(format!(
            "| {} | `{}` | {} | {} | {}% |",
            i + 1,
            c.key,
            c.hits,
            c.shapes,
            tenths_pct(c.hits, rollup.grand_total)
        ));
    }
    let top_hits: usize = rollup.clusters.iter().take(n).map(|c| c.hits).sum();
    lines.push(format!(
        "\ntop-{n} clusters cover {top_hits}/{} findings ({}%) · regenerate via `deno task gaps:audit:rank`",
        rollup.grand_total,
        pct_of(top_hits, rollup.grand_total)
    ));

    let out = lines.join("\n");
    if json {
        eprintln!("{out}");
    } else {
        println!("{out}");
    }
}

/// Load a prior `gap_audit --json` output's `by_node` array into `(node, edge) → hits`.
///
/// `None` on any failure — unreadable path, invalid JSON, or no `by_node` array — each already
/// WARNED to stderr; `Some(map)` on a successful load, which may legitimately be **empty** (a
/// `--json` run over a corpus with zero clusters). Report-only, so a bad `--since` path must never
/// fail the gate or the exit code, only skip the diff. The `Some(empty)` / `None` split is why the
/// caller can't collapse both to "empty map" — an empty-but-valid baseline still yields a diff
/// (every current cluster reads as new).
fn load_since_baseline(path: &str) -> Option<BTreeMap<(String, String), usize>> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("warning: --since: cannot read {path} ({e}); skipping the ranking diff");
            return None;
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "warning: --since: {path} is not valid JSON ({e}); skipping the ranking diff"
            );
            return None;
        }
    };
    let Some(arr) = value.get("by_node").and_then(serde_json::Value::as_array) else {
        eprintln!(
            "warning: --since: {path} has no `by_node` array (was it produced by \
             `gap_audit --json`?); skipping the ranking diff"
        );
        return None;
    };
    Some(
        arr.iter()
            .filter_map(|c| {
                let node = c.get("node")?.as_str()?.to_string();
                let edge = c.get("edge")?.as_str()?.to_string();
                let hits = usize::try_from(c.get("hits")?.as_u64()?).ok()?;
                Some(((node, edge), hits))
            })
            .collect(),
    )
}

/// One changed-cluster row in the ranking diff.
#[derive(Debug, PartialEq)]
struct Mover {
    /// The `(node, edge)` cluster key.
    key: (String, String),
    /// Hit count in the baseline (0 when the cluster is new).
    then: usize,
    /// Hit count in this run (0 when the cluster is gone).
    now: usize,
    /// `now - then` — negative is the burn-down's win, positive a regression.
    delta: isize,
}

/// A runaway guard on the printed mover list — a slice moves a handful of clusters, so this only
/// trips on a stale/mismatched baseline. Not `--top`: `--top` sizes the `--rank` table, while a
/// diff wants EVERY changed cluster (a hidden regression would defeat the purpose).
const SINCE_MOVER_CAP: usize = 80;

/// The changed-cluster rows for the ranking diff: for every `(node, edge)` in EITHER map whose hit
/// count differs, `(key, then, now, delta)` — a cluster absent from one side reads as 0 there (gone
/// → `n → 0`, new → `0 → n`), an unchanged cluster is dropped. Sorted biggest-reduction-first
/// (delta ascending — the burn-down's win at the top, regressions at the bottom), ties broken by
/// key so the diff is deterministic. Pure, so it unit-tests without touching stdout.
fn since_movers(
    now: &BTreeMap<(String, String), usize>,
    baseline: &BTreeMap<(String, String), usize>,
) -> Vec<Mover> {
    let mut keys: BTreeSet<(String, String)> = now.keys().cloned().collect();
    keys.extend(baseline.keys().cloned());
    let mut movers: Vec<Mover> = keys
        .into_iter()
        .filter_map(|k| {
            let then = baseline.get(&k).copied().unwrap_or(0);
            let now_hits = now.get(&k).copied().unwrap_or(0);
            let delta = now_hits as isize - then as isize;
            (delta != 0).then_some(Mover {
                key: k,
                then,
                now: now_hits,
                delta,
            })
        })
        .collect();
    movers.sort_by(|a, b| a.delta.cmp(&b.delta).then_with(|| a.key.cmp(&b.key)));
    movers
}

/// Print the per-cluster ranking DELTA of this run against a prior `--json` baseline — "did my
/// slice move the cluster?" (`(CallExpression, arguments→$)  2861 → 2790  (−71)`). Only clusters
/// whose hit count CHANGED are listed, biggest reduction first (the burn-down's win at the top),
/// then the biggest regressions; a cluster gone to zero or newly appearing is shown against 0.
/// EVERY changed cluster is shown (capped only by [`SINCE_MOVER_CAP`], a runaway guard, not by
/// `--top`). Report-only: a bad baseline warns and skips (see [`load_since_baseline`]), never
/// failing the gate. Under `--json` it prints to stderr.
pub(super) fn report_since(rollup: &ByNodeRollup, path: &str, json: bool) {
    let Some(baseline) = load_since_baseline(path) else {
        return; // the loader already warned
    };
    let now: BTreeMap<(String, String), usize> = rollup
        .clusters
        .iter()
        .map(|c| ((c.key.node_type.clone(), c.key.edge.clone()), c.hits))
        .collect();
    let movers = since_movers(&now, &baseline);

    let mut lines: Vec<String> = Vec::new();
    let net: isize = movers.iter().map(|m| m.delta).sum();
    lines.push(format!(
        "\nranking diff vs {path} — {} cluster(s) changed (net {net:+} findings):",
        movers.len()
    ));
    for m in movers.iter().take(SINCE_MOVER_CAP) {
        let (node, edge) = &m.key;
        lines.push(format!(
            "  ({node}, {edge})  {} → {}  ({:+})",
            m.then, m.now, m.delta
        ));
    }
    if movers.len() > SINCE_MOVER_CAP {
        lines.push(format!("  … and {} more", movers.len() - SINCE_MOVER_CAP));
    }
    if movers.is_empty() {
        lines.push("  (no cluster moved — identical ranking)".to_string());
    }

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

    /// `tenths_pct` renders one decimal and guards the zero denominator — the rank table's share.
    #[test]
    fn tenths_pct_one_decimal_and_zero_guard() {
        assert_eq!(tenths_pct(2861, 29811), "9.5"); // 9.597% truncates to 9.5
        assert_eq!(tenths_pct(0, 0), "0.0");
        assert_eq!(tenths_pct(1, 3), "33.3"); // 33.33%
        assert_eq!(tenths_pct(1, 1), "100.0");
    }

    fn cluster(node: &str, edge: &str, hits: usize) -> ((String, String), usize) {
        ((node.to_string(), edge.to_string()), hits)
    }

    fn mover(node: &str, edge: &str, then: usize, now: usize, delta: isize) -> Mover {
        Mover {
            key: (node.to_string(), edge.to_string()),
            then,
            now,
            delta,
        }
    }

    /// The `--since` diff: an unchanged cluster is dropped, a gone one reads `n → 0`, a new one
    /// `0 → n`, and the rows sort biggest-reduction-first with `net = Σ delta`. This is exactly the
    /// logic the `--top`/`--since` doc mismatch would have slipped past — pinned here.
    #[test]
    fn since_movers_diffs_gone_new_unchanged_and_sorts() {
        let baseline: BTreeMap<(String, String), usize> = [
            cluster("Call", "a→$", 100), // reduced −20
            cluster("Arr", "e→$", 50),   // increased +40
            cluster("Gone", "x→y", 7),   // absent now → 0
            cluster("Same", "s→$", 12),  // unchanged → dropped
        ]
        .into_iter()
        .collect();
        let now: BTreeMap<(String, String), usize> = [
            cluster("Call", "a→$", 80),
            cluster("Arr", "e→$", 90),
            cluster("New", "n→$", 5), // absent in baseline → new
            cluster("Same", "s→$", 12),
        ]
        .into_iter()
        .collect();

        let movers = since_movers(&now, &baseline);
        // Four moved; "Same" (unchanged) is excluded.
        assert_eq!(movers.len(), 4);
        // Biggest reduction first: Call (−20), Gone (−7), then New (+5), Arr (+40).
        assert_eq!(movers[0], mover("Call", "a→$", 100, 80, -20));
        assert_eq!(movers[1], mover("Gone", "x→y", 7, 0, -7));
        assert_eq!(movers[2], mover("New", "n→$", 0, 5, 5));
        assert_eq!(movers[3], mover("Arr", "e→$", 50, 90, 40));
        let net: isize = movers.iter().map(|m| m.delta).sum();
        assert_eq!(net, 18); // −20 −7 +5 +40
    }

    /// Identical maps yield no movers (the "no cluster moved" path).
    #[test]
    fn since_movers_empty_when_identical() {
        let m: BTreeMap<(String, String), usize> =
            [cluster("Call", "a→$", 100)].into_iter().collect();
        assert!(since_movers(&m, &m).is_empty());
    }
}
