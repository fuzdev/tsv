//! The report-only by-(node, edge) rollup: folds the fine per-site shape tallies into a
//! ranked, coarse emitter work-list. Split out of `gap_audit.rs` for navigability.

use std::collections::{BTreeMap, BTreeSet};

use crate::audit::node_edge::NodeEdgeKey;

use super::{Kind, Tally};

/// One cluster row in the ranked (worst-first) by-node work-list — one `(node, edge)` and its
/// EXACT per-site hit tally, read straight off [`Tally::node_edge_hits`].
struct ClusterRow {
    key: NodeEdgeKey,
    hits: usize,
    /// How many **distinct gaps** landed in this cluster — `(file, whitespace-run)` deduped
    /// (see `NodeClusterAccum::gaps`). The RANKING metric: raw hits over-weight
    /// whitespace-adjacent gaps ~3.7× (an N-wide run yields ~N injectable offsets, a glued
    /// gap one), so sorting on hits systematically deprioritizes the glued-gap families.
    gaps: usize,
    /// How many distinct site shapes landed in this cluster.
    shapes: usize,
    /// The lexicographically smallest shape in the cluster, shown as its example.
    example_shape: String,
    /// Exact per-[`Kind`] hit tallies — the kind-composition cell. Read before slicing a
    /// cluster: an all-SWALLOW #1 has zero pinned-ratchet presence, so its yield is
    /// silent-corruption fixes, not line retirement (slice 1's lesson, now a column).
    kinds: BTreeMap<Kind, usize>,
    /// The edge's boundary class — `leading` / `trailing` / `interior` (see [`edge_class`]).
    /// Boundary edges are the fused-`text()`-with-no-query territory; interior edges are the
    /// element-comma-seam family.
    edge_class: &'static str,
}

/// Classify a `(node, edge)` edge string as a node-boundary or interior gap: `^→…` is the
/// node's **leading** region, `…→$` its **trailing** region, anything else an **interior**
/// inter-child gap. A childless node's `^→$` classes as leading by first-match — one region,
/// arbitrary but stable.
fn edge_class(edge: &str) -> &'static str {
    if edge.starts_with("^→") {
        "leading"
    } else if edge.ends_with("→$") {
        "trailing"
    } else {
        "interior"
    }
}

/// A cluster's kind composition as compact `label N` fragments (`drop 12 · swal 3`),
/// `Kind`-enum order, nonzero only — the rank table cell and the by-node line suffix.
fn kind_summary(kinds: &BTreeMap<Kind, usize>) -> String {
    kinds
        .iter()
        .map(|(kind, n)| format!("{} {n}", kind.short_label()))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The by-node rollup, shared by the human `--by-node` view and the `--json` section.
///
/// Every field is an EXACT per-site tally accumulated at record time (see
/// [`Tally::node_edge_hits`]) — no canonical-example approximation, so no agreement measure to
/// carry. The one residual caveat is the [`Self::unresolved_count`] tail (offsets that key to no
/// node), zero over `tests/fixtures`.
pub(super) struct ByNodeRollup {
    /// Clusters ranked worst-first (distinct gaps desc, then hits desc, then key).
    clusters: Vec<ClusterRow>,
    grand_total: usize,
    /// Σ per-cluster distinct-gap counts — the gap-share denominator. Strictly speaking
    /// "gap-slots": the rare gap whose hits key to two clusters (an injected vs a
    /// mapped-back bystander attribution) counts once per cluster.
    grand_total_gaps: usize,
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
            gaps: accum.gaps.len(),
            shapes: accum.shapes.len(),
            // BTreeSet is sorted, so `.next()` is the lexicographically smallest shape. An accum
            // always carries ≥1 shape (it's created when a hit is folded), so the default is dead.
            example_shape: accum.shapes.iter().next().cloned().unwrap_or_default(),
            kinds: accum.kind_hits.clone(),
            edge_class: edge_class(&key.edge),
        })
        .collect();
    let grand_total_gaps: usize = clusters.iter().map(|c| c.gaps).sum();
    // Worst-first by DISTINCT GAPS (the de-biased metric — see `ClusterRow::gaps`), hits as
    // the first tie-break, then the key, so the ranking is deterministic.
    clusters.sort_by(|a, b| {
        b.gaps
            .cmp(&a.gaps)
            .then_with(|| b.hits.cmp(&a.hits))
            .then_with(|| a.key.cmp(&b.key))
    });

    ByNodeRollup {
        clusters,
        grand_total,
        grand_total_gaps,
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
                // Boundary class: `leading` / `trailing` / `interior` — the fused-head/tail
                // vs element-comma-seam split, so a consumer can slice the work-list by it.
                "edge_class": c.edge_class,
                "hits": c.hits,
                "gaps": c.gaps,
                "shapes": c.shapes,
                "share": share_of(c.hits, rollup.grand_total),
                "gaps_share": share_of(c.gaps, rollup.grand_total_gaps),
                "example_shape": c.example_shape,
                // Per-kind hit tallies, full snapshot labels as keys, nonzero only — an
                // all-SWALLOW cluster has zero pinned-ratchet presence, which changes what a
                // slice against it yields.
                "kinds": c.kinds
                    .iter()
                    .map(|(kind, n)| (kind.label().to_string(), serde_json::json!(n)))
                    .collect::<serde_json::Map<String, serde_json::Value>>(),
            })
        })
        .collect();

    let mut m = serde_json::Map::new();
    m.insert("by_node".to_string(), serde_json::Value::Array(by_node));
    m.insert(
        "by_node_unresolved".to_string(),
        serde_json::json!(rollup.unresolved_count),
    );
    // The ranking-metric version stamp. 1 = hits-sorted (pre-gap-dedup); 2 = distinct-gap
    // sorted with `gaps`/`gaps_share` per cluster. `--since` deliberately keeps diffing
    // HITS (exact and comparable across both metrics, so old baselines stay usable); a
    // consumer that wants gap-based diffs keys on this stamp.
    m.insert("by_node_metric".to_string(), serde_json::json!(2));
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
            "  {:>6} gap(s)  {:>7}×  {:>4} shape(s)  {key:<42}  {:<8}  {:<22}  e.g. {}",
            c.gaps,
            c.hits,
            c.shapes,
            c.edge_class,
            kind_summary(&c.kinds),
            c.example_shape
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
         totals are EXACT per-site tallies. Ranked by DISTINCT GAPS (whitespace-run deduped) — \
         raw hits over-weight whitespace-adjacent gaps ~3.7× vs glued ones."
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

/// Print the top-`top` by-(node, edge) clusters as a **paste-ready markdown table** — the
/// ranked emitter work-list, fattest-first, so a burn-down effort can refresh its tracking
/// table by pasting instead of parsing `--json` and hand-transcribing. Report-only:
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
    lines.push("| # | cluster | class | gaps | hits | shapes | kinds | gap share |".to_string());
    lines.push("| ---: | --- | --- | ---: | ---: | ---: | --- | ---: |".to_string());
    for (i, c) in rollup.clusters.iter().take(n).enumerate() {
        lines.push(format!(
            "| {} | `{}` | {} | {} | {} | {} | {} | {}% |",
            i + 1,
            c.key,
            c.edge_class,
            c.gaps,
            c.hits,
            c.shapes,
            kind_summary(&c.kinds),
            tenths_pct(c.gaps, rollup.grand_total_gaps)
        ));
    }
    let top_gaps: usize = rollup.clusters.iter().take(n).map(|c| c.gaps).sum();
    let top_hits: usize = rollup.clusters.iter().take(n).map(|c| c.hits).sum();
    lines.push(format!(
        "\ntop-{n} clusters cover {top_gaps}/{} distinct gaps ({}%) · {top_hits}/{} findings · \
         ranked by distinct gaps (whitespace-run deduped) · regenerate via `deno task gaps:audit:rank`",
        rollup.grand_total_gaps,
        pct_of(top_gaps, rollup.grand_total_gaps),
        rollup.grand_total
    ));

    let out = lines.join("\n");
    if json {
        eprintln!("{out}");
    } else {
        println!("{out}");
    }
}

/// Load a prior `gap_audit --json` output as a JSON value — the one read both `--since`
/// sections (cluster and shape) extract from.
///
/// `None` on an unreadable path or invalid JSON, already WARNED to stderr. Report-only, so a
/// bad `--since` path must never fail the gate or the exit code, only skip the diff.
fn load_since_json(path: &str) -> Option<serde_json::Value> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("warning: --since: cannot read {path} ({e}); skipping the diff");
            return None;
        }
    };
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("warning: --since: {path} is not valid JSON ({e}); skipping the diff");
            None
        }
    }
}

/// Extract a baseline's `by_node` array into `(node, edge) → hits`.
///
/// `None` when the array is absent (warned); `Some(map)` may legitimately be **empty** (a
/// `--json` run over a corpus with zero clusters). The `Some(empty)` / `None` split is why the
/// caller can't collapse both to "empty map" — an empty-but-valid baseline still yields a diff
/// (every current cluster reads as new).
fn since_cluster_map(
    value: &serde_json::Value,
    path: &str,
) -> Option<BTreeMap<(String, String), usize>> {
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

/// Extract a baseline's `shapes` array into `(kind, shape) → (count, payloads)` — the
/// shape-level side of the `--since` diff, `payloads` joined `,` exactly as the snapshot
/// column renders it. `None` when the array is absent (warned).
fn since_shape_map(
    value: &serde_json::Value,
    path: &str,
) -> Option<BTreeMap<(String, String), (usize, String)>> {
    let Some(arr) = value.get("shapes").and_then(serde_json::Value::as_array) else {
        eprintln!(
            "warning: --since: {path} has no `shapes` array (was it produced by \
             `gap_audit --json`?); skipping the shape diff"
        );
        return None;
    };
    Some(
        arr.iter()
            .filter_map(|s| {
                let kind = s.get("kind")?.as_str()?.to_string();
                let shape = s.get("shape")?.as_str()?.to_string();
                let count = usize::try_from(s.get("count")?.as_u64()?).ok()?;
                let payloads = s
                    .get("payloads")?
                    .as_array()?
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(",");
                Some(((kind, shape), (count, payloads)))
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

/// A runaway guard on each printed mover list (cluster and shape alike) — a slice moves a
/// handful of rows, so this only trips on a stale/mismatched baseline. Not `--top`: `--top`
/// sizes the `--rank` table, while a diff wants EVERY changed row (a hidden regression would
/// defeat the purpose).
const SINCE_MOVER_CAP: usize = 80;

/// Append a mover list to `lines`: each row through `render`, capped at [`SINCE_MOVER_CAP`]
/// with an `… and N more` tail — or `empty` alone when nothing moved, so a quiet section still
/// states its claim instead of vanishing.
fn push_mover_rows<T>(
    lines: &mut Vec<String>,
    rows: &[T],
    render: impl Fn(&T) -> String,
    empty: &str,
) {
    for row in rows.iter().take(SINCE_MOVER_CAP) {
        lines.push(render(row));
    }
    if rows.len() > SINCE_MOVER_CAP {
        lines.push(format!("  … and {} more", rows.len() - SINCE_MOVER_CAP));
    }
    if rows.is_empty() {
        lines.push(empty.to_string());
    }
}

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

/// One changed-shape row in the shape-level diff.
#[derive(Debug, PartialEq)]
struct ShapeMover {
    /// The `(kind label, site shape)` key.
    key: (String, String),
    /// Baseline `(count, payloads)` — `None` when the shape is new this run.
    then: Option<(usize, String)>,
    /// This run's `(count, payloads)` — `None` when the shape is gone.
    now: Option<(usize, String)>,
    /// Count delta (an absent side reads 0) — negative is the burn-down's win.
    delta: isize,
}

/// The changed-shape rows: every `(kind, shape)` in EITHER map whose `(count, payloads)`
/// differs — the exact per-shape delta the ratchet (shape-set only) and a payload-only diff
/// are both blind to. A count move at an already-pinned shape, a payload-set change, and an
/// added/retired shape all surface here; an unchanged shape is dropped. Sorted
/// biggest-reduction-first, ties by key. Pure, so it unit-tests without touching stdout.
fn shape_movers(
    now: &BTreeMap<(String, String), (usize, String)>,
    baseline: &BTreeMap<(String, String), (usize, String)>,
) -> Vec<ShapeMover> {
    let mut keys: BTreeSet<(String, String)> = now.keys().cloned().collect();
    keys.extend(baseline.keys().cloned());
    let mut movers: Vec<ShapeMover> = keys
        .into_iter()
        .filter_map(|k| {
            let then = baseline.get(&k).cloned();
            let now_entry = now.get(&k).cloned();
            if then == now_entry {
                return None;
            }
            let then_count = then.as_ref().map_or(0, |(n, _)| *n);
            let now_count = now_entry.as_ref().map_or(0, |(n, _)| *n);
            Some(ShapeMover {
                key: k,
                delta: now_count as isize - then_count as isize,
                then,
                now: now_entry,
            })
        })
        .collect();
    movers.sort_by(|a, b| a.delta.cmp(&b.delta).then_with(|| a.key.cmp(&b.key)));
    movers
}

/// Print the DELTA of this run against a prior `--json` baseline, in three report-only
/// sections — the one-command form of the pre-merge discipline ("zero deltas on
/// `(kind, shape) → (count, payloads)`", a far stronger claim than the ratchet's verdict):
///
/// 1. **ranking diff** — per-cluster hit deltas (`(CallExpression, arguments→$)  2861 → 2790
///    (−71)`), biggest reduction first: "did my slice move its target cluster?"
/// 2. **shape diff** — per-`(kind, shape)` `(count, payloads)` deltas, including moves at
///    already-pinned shapes, which the ratchet (shape-set only) cannot see.
/// 3. **seed eligibility** — `files` / `dirty` / `parse-skipped` deltas, printed only when
///    changed: a fix that makes a dirty seed clean hands the audit NEW seeds, so a count rise
///    on an existing shape may be new coverage rather than a regression.
///
/// EVERY changed row is shown (capped only by [`SINCE_MOVER_CAP`], a runaway guard, not by
/// `--top`). A bad baseline warns and skips (see [`load_since_json`]), never failing the gate.
/// Under `--json` it prints to stderr.
pub(super) fn report_since(rollup: &ByNodeRollup, total: &Tally, path: &str, json: bool) {
    let Some(value) = load_since_json(path) else {
        return; // the loader already warned
    };
    let mut lines: Vec<String> = Vec::new();
    if let Some(baseline) = since_cluster_map(&value, path) {
        push_ranking_section(&mut lines, rollup, &baseline, path);
    }
    if let Some(baseline) = since_shape_map(&value, path) {
        push_shape_section(&mut lines, total, &baseline, path);
    }
    push_eligibility_section(&mut lines, &value, total);

    let out = lines.join("\n");
    if json {
        eprintln!("{out}");
    } else {
        println!("{out}");
    }
}

/// The ranking-diff section ([`report_since`] item 1): per-cluster hit deltas vs the baseline.
fn push_ranking_section(
    lines: &mut Vec<String>,
    rollup: &ByNodeRollup,
    baseline: &BTreeMap<(String, String), usize>,
    path: &str,
) {
    let now: BTreeMap<(String, String), usize> = rollup
        .clusters
        .iter()
        .map(|c| ((c.key.node_type.clone(), c.key.edge.clone()), c.hits))
        .collect();
    let movers = since_movers(&now, baseline);
    let net: isize = movers.iter().map(|m| m.delta).sum();
    lines.push(format!(
        "\nranking diff vs {path} — {} cluster(s) changed (net {net:+} findings):",
        movers.len()
    ));
    push_mover_rows(
        lines,
        &movers,
        |m| {
            let (node, edge) = &m.key;
            format!(
                "  ({node}, {edge})  {} → {}  ({:+})",
                m.then, m.now, m.delta
            )
        },
        "  (no cluster moved — identical ranking)",
    );
}

/// The shape-diff section ([`report_since`] item 2): per-`(kind, shape)` `(count, payloads)`
/// deltas vs the baseline.
fn push_shape_section(
    lines: &mut Vec<String>,
    total: &Tally,
    baseline: &BTreeMap<(String, String), (usize, String)>,
    path: &str,
) {
    let now: BTreeMap<(String, String), (usize, String)> = total
        .shapes
        .iter()
        .map(|((kind, shape), agg)| {
            (
                (kind.label().to_string(), shape.clone()),
                (
                    agg.count,
                    agg.payloads.iter().copied().collect::<Vec<_>>().join(","),
                ),
            )
        })
        .collect();
    let movers = shape_movers(&now, baseline);
    lines.push(format!(
        "\nshape diff vs {path} — {} shape(s) changed:",
        movers.len()
    ));
    push_mover_rows(
        lines,
        &movers,
        render_shape_mover,
        "  (no shape-level delta — every (kind, shape) → (count, payloads) matches the baseline)",
    );
}

/// One shape-mover row: counts and delta, plus the status suffix — `(NEW)` / `(gone)` for a
/// one-sided row, the `[old → new]` payload sets when only they changed.
fn render_shape_mover(m: &ShapeMover) -> String {
    let then_count = m.then.as_ref().map_or(0, |(n, _)| *n);
    let now_count = m.now.as_ref().map_or(0, |(n, _)| *n);
    let status = match (&m.then, &m.now) {
        (None, Some(_)) => "  (NEW)".to_string(),
        (Some(_), None) => "  (gone)".to_string(),
        (Some((_, then_payloads)), Some((_, now_payloads))) if then_payloads != now_payloads => {
            format!("  [{then_payloads} → {now_payloads}]")
        }
        _ => String::new(),
    };
    let (kind, shape) = &m.key;
    format!(
        "  {kind:<14} {shape:<24} {then_count} → {now_count}  ({:+}){status}",
        m.delta
    )
}

/// The seed-eligibility section ([`report_since`] item 3), printed only when it moved: a diff
/// over a CHANGED seed set is measuring two things at once, and a count rise from a
/// newly-eligible seed is indistinguishable from a regression without this line.
fn push_eligibility_section(lines: &mut Vec<String>, value: &serde_json::Value, total: &Tally) {
    let then_files = value.get("files").and_then(serde_json::Value::as_u64);
    let then_skipped = value
        .get("parse_skipped")
        .and_then(serde_json::Value::as_u64);
    let then_dirty = value
        .get("dirty_files")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len);
    let moved = then_files.is_some_and(|n| n != total.files_done as u64)
        || then_skipped.is_some_and(|n| n != total.parse_skipped as u64)
        || then_dirty.is_some_and(|n| n != total.dirty_files.len());
    if !moved {
        return;
    }
    let or_unknown = |n: Option<u64>| n.map_or_else(|| "?".to_string(), |n| n.to_string());
    lines.push(format!(
        "\n⚠ seed eligibility changed: files {} → {} · dirty {} → {} · parse-skipped {} → \
         {} — a seed entering or leaving the injectable set shifts counts on existing \
         shapes; new coverage and regression read alike in the shape diff above.",
        or_unknown(then_files),
        total.files_done,
        then_dirty.map_or_else(|| "?".to_string(), |n| n.to_string()),
        total.dirty_files.len(),
        or_unknown(then_skipped),
        total.parse_skipped,
    ));
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
                    skip_sites: Vec::new(),
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

    /// One keyed hit at `offset` in `source`, for the gap-dedup tests below.
    fn keyed_hit<'a>(source: &'a str, path: &'a str, offset: usize, edge: &NodeEdgeKey) -> Hit<'a> {
        Hit {
            kind: Kind::Dropped,
            payload: Payload::Block,
            path,
            source,
            injection_offset: offset,
            attribution_offset: offset,
            text: "/* c */".to_string(),
            skip_sites: Vec::new(),
            injected: true,
            node_edge: Some(edge.clone()),
        }
    }

    /// The ranking metric is DISTINCT GAPS, not hits: three hits spread across one 3-wide
    /// whitespace run dedup to ONE gap, while two glued hits in two different gaps stay TWO —
    /// so the glued cluster outranks the whitespace one despite fewer hits. Exactly the
    /// ~3.7× whitespace bias the dedup exists to remove (an N-wide run yields ~N injectable
    /// offsets; ranking on hits deprioritizes the glued-gap families).
    #[test]
    fn ranking_dedups_whitespace_runs_and_sorts_by_distinct_gaps() {
        let ws_edge = node_edge("CallExpression", "arguments→$");
        let glued_edge = node_edge("TSArrayType", "elementType→$");
        let mut tally = Tally::default();
        // `"a   b"`: offsets 1, 2, 3 are all inside the one 3-wide whitespace run — one gap.
        for offset in [1, 2, 3] {
            tally.record(keyed_hit("a   b", "ws.ts", offset, &ws_edge), true);
        }
        // `"x.y"` offsets 1 and 2 are glued (no preceding whitespace) — two distinct gaps.
        for offset in [1, 2] {
            tally.record(keyed_hit("x.y", "glued.ts", offset, &glued_edge), true);
        }

        let rollup = compute_by_node(&tally);
        assert_eq!(rollup.grand_total, 5, "hits stay exact");
        assert_eq!(
            rollup.grand_total_gaps, 3,
            "1 whitespace run + 2 glued gaps"
        );
        // The glued cluster (2 gaps, 2 hits) outranks the whitespace one (1 gap, 3 hits).
        assert_eq!(rollup.clusters[0].key, glued_edge);
        assert_eq!(rollup.clusters[0].gaps, 2);
        assert_eq!(rollup.clusters[0].hits, 2);
        assert_eq!(rollup.clusters[1].key, ws_edge);
        assert_eq!(rollup.clusters[1].gaps, 1);
        assert_eq!(rollup.clusters[1].hits, 3);
    }

    /// `gap_run_start` normalizes every offset in one ASCII-whitespace run to its first byte,
    /// leaves a glued offset alone, and guards the 0/OOB edges.
    #[test]
    fn gap_run_start_normalizes_within_a_run() {
        use super::super::gap_run_start;
        //         0123456
        let src = "a \t\n b";
        for offset in [2, 3, 4, 5] {
            assert_eq!(
                gap_run_start(src, offset),
                1,
                "offset {offset} is in the run"
            );
        }
        assert_eq!(gap_run_start(src, 1), 1, "run start maps to itself");
        assert_eq!(gap_run_start("x.y", 2), 2, "glued offset is its own gap");
        assert_eq!(gap_run_start("x.y", 0), 0);
        assert_eq!(
            gap_run_start("  ", 9),
            0,
            "offset clamped, then scans the run"
        );
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
            std::iter::once(cluster("Call", "a→$", 100)).collect();
        assert!(since_movers(&m, &m).is_empty());
    }

    /// The boundary/interior split — `^→…` leading, `…→$` trailing, else interior; a
    /// childless node's `^→$` classes leading by first-match.
    #[test]
    fn edge_class_splits_boundary_from_interior() {
        assert_eq!(edge_class("^→test"), "leading");
        assert_eq!(edge_class("arguments→$"), "trailing");
        assert_eq!(edge_class("id→init"), "interior");
        assert_eq!(edge_class("types→types"), "interior");
        assert_eq!(edge_class("^→$"), "leading");
    }

    /// The kind cell renders `Kind`-enum order, nonzero only, in the compact labels.
    #[test]
    fn kind_summary_renders_enum_order_nonzero_only() {
        let kinds: BTreeMap<Kind, usize> = [(Kind::Swallow, 3), (Kind::Dropped, 12)]
            .into_iter()
            .collect();
        assert_eq!(kind_summary(&kinds), "drop 12 · swal 3");
        assert_eq!(kind_summary(&BTreeMap::new()), "");
    }

    /// Clusters carry their exact kind composition and edge class through the rollup — the
    /// columns that stop a gap-ranked all-SWALLOW #1 from reading as ratchet burn-down.
    #[test]
    fn clusters_carry_kind_composition_and_edge_class() {
        let edge = node_edge("SwitchCase", "^→test");
        let mut tally = Tally::default();
        tally.record(keyed_hit("a b", "p.ts", 1, &edge), true);
        tally.record(
            Hit {
                kind: Kind::Swallow,
                payload: Payload::Line,
                path: "p.ts",
                source: "a b",
                injection_offset: 1,
                attribution_offset: 1,
                text: "// c".to_string(),
                skip_sites: Vec::new(),
                injected: true,
                node_edge: Some(edge.clone()),
            },
            true,
        );

        let rollup = compute_by_node(&tally);
        assert_eq!(rollup.clusters.len(), 1);
        assert_eq!(rollup.clusters[0].edge_class, "leading");
        assert_eq!(kind_summary(&rollup.clusters[0].kinds), "drop 1 · swal 1");
    }

    fn shape_entry(
        kind: &str,
        shape: &str,
        count: usize,
        payloads: &str,
    ) -> ((String, String), (usize, String)) {
        (
            (kind.to_string(), shape.to_string()),
            (count, payloads.to_string()),
        )
    }

    /// The shape-level diff surfaces every delta the ratchet is blind to: a count move at an
    /// already-known shape, a payload-set change at an unchanged count, a retired shape, and
    /// a new one — sorted biggest reduction first; an unchanged shape is dropped.
    #[test]
    fn shape_movers_catches_count_payload_new_and_gone() {
        let baseline: BTreeMap<(String, String), (usize, String)> = [
            shape_entry("DROPPED", "a⟨⟩.", 100, "block"), // count −20
            shape_entry("DROPPED", "b⟨⟩.", 10, "block"),  // payload-only change
            shape_entry("SWALLOW", "c⟨⟩;", 7, "line"),    // gone
            shape_entry("DROPPED", "same⟨⟩", 5, "block"), // unchanged → dropped
        ]
        .into_iter()
        .collect();
        let now: BTreeMap<(String, String), (usize, String)> = [
            shape_entry("DROPPED", "a⟨⟩.", 80, "block"),
            shape_entry("DROPPED", "b⟨⟩.", 10, "block,line"),
            shape_entry("DOUBLE-PRINTED", "d⟨⟩=", 4, "block"), // new
            shape_entry("DROPPED", "same⟨⟩", 5, "block"),
        ]
        .into_iter()
        .collect();

        let movers = shape_movers(&now, &baseline);
        assert_eq!(movers.len(), 4, "the unchanged shape is excluded");
        // Delta-ascending: −20, gone (−7), payload-only (0), new (+4).
        assert_eq!(movers[0].key.1, "a⟨⟩.");
        assert_eq!(movers[0].delta, -20);
        assert_eq!(movers[1].key.1, "c⟨⟩;");
        assert!(movers[1].now.is_none(), "a retired shape reads as gone");
        assert_eq!(movers[2].key.1, "b⟨⟩.");
        assert_eq!(
            movers[2].delta, 0,
            "a payload-only change still surfaces — count and ratchet are both blind to it"
        );
        assert_eq!(movers[3].key.1, "d⟨⟩=");
        assert!(movers[3].then.is_none(), "an added shape reads as new");
    }

    /// The shape-mover row's status suffix: `(NEW)` / `(gone)` for a one-sided row, the
    /// `[old → new]` payload sets when only they changed, nothing on a plain count move.
    #[test]
    fn render_shape_mover_status_suffixes() {
        let mover = |then: Option<(usize, &str)>, now: Option<(usize, &str)>| {
            let then = then.map(|(n, p)| (n, p.to_string()));
            let now = now.map(|(n, p)| (n, p.to_string()));
            let delta = now.as_ref().map_or(0, |(n, _)| *n as isize)
                - then.as_ref().map_or(0, |(n, _)| *n as isize);
            ShapeMover {
                key: ("DROPPED".to_string(), "a⟨⟩.".to_string()),
                then,
                now,
                delta,
            }
        };
        assert!(
            render_shape_mover(&mover(Some((100, "block")), Some((80, "block"))))
                .ends_with("100 → 80  (-20)"),
            "a plain count move carries no suffix"
        );
        assert!(
            render_shape_mover(&mover(None, Some((4, "block")))).ends_with("0 → 4  (+4)  (NEW)")
        );
        assert!(
            render_shape_mover(&mover(Some((7, "line")), None)).ends_with("7 → 0  (-7)  (gone)")
        );
        assert!(
            render_shape_mover(&mover(Some((10, "block")), Some((10, "block,line"))))
                .ends_with("10 → 10  (+0)  [block → block,line]"),
            "a payload-only change names both sets"
        );
    }
}
