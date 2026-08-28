//! Per-node-kind census over a corpus — how many of each AST node a parse
//! actually builds, and what that population costs in slot bytes.
//!
//! **The recurring input the perf arc has never had a tool for.** Every density
//! trade asks the same two questions — *is this variant rare enough that boxing
//! it is free?* and *is this container populous enough that narrowing it is
//! worth anything?* — and every session has answered them by hand: a throwaway
//! counter table wired into the parser's construction sites, run once, read
//! off, and reverted. That instrument is right but unrepeatable, and the
//! sessions before it used grep proxies (lines-containing-`;` as a statement
//! denominator) crude enough to mis-rank a lever by 2×.
//!
//! The census also answers a question that has nothing to do with perf: *which
//! node kinds does the fixture tree never exercise?* — the same query, pointed
//! at `tests/fixtures` instead of at a real corpus.
//!
//! # What is counted, and the one blind spot
//!
//! Nodes are counted off the **wire AST** — the JSON `convert_ast_json` emits,
//! which the writer produces by walking the internal AST once and emitting one
//! object per node. So a count here is the parser's own construction count for
//! every node the wire names, with no sampling and no proxy.
//!
//! ⚠️ **An internal node the writer does not name has no row.** Two kinds:
//!
//! - **Unwrapped wrappers** — `ParenthesizedExpression` and `JsdocCast` are
//!   real `Expression` variants the parser builds and the writer prints
//!   *through*, so a corpus's parens are invisible here. A census of
//!   `Expression` values is therefore a LOWER BOUND, and the gap is exactly the
//!   parenthesized ones.
//! - **Slot enums** — `ForInit`, `ForInOfLeft`, `ArrowFunctionBody`,
//!   `AttributeValue` and their kin are internal enums whose variant payload is
//!   the wire node; the enum itself is a field's type, never an emitted object.
//!   `--slots` reaches these, since the slot they fill IS reported (as
//!   `Parent.field -> Child`).
//!
//! Everything the density ladder has actually asked about — `Property`,
//! `VariableDeclarator`, the statement heads, `TSPropertySignature`,
//! `MethodDefinition`, the `TS*Type` family — is a named wire node and is
//! counted exactly.
//!
//! # `--bytes`
//!
//! Joins each row against [`type_sizes`]' `size_of` board by
//! name, giving `count × size` — the slot-megabyte column that ranks a density
//! lever, and the whole table a session would otherwise build by hand. A wire
//! name with no type of that name on the board (or with two, in different
//! language crates) reports its count and leaves the byte columns blank rather
//! than guessing.

use argh::FromArgs;
use serde_json::Value;
use std::collections::HashMap;

use crate::audit::properties::tsv_parse_to_value;
use crate::cli::CliError;
use tsv_cli::cli::input::ParserType;

use super::profile::resolve_profile_files;
use super::type_sizes;

/// Census of AST node kinds over a corpus.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "ast_census")]
pub struct AstCensusCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// join each row against the `size_of` board: count x slot bytes
    #[argh(switch)]
    bytes: bool,

    /// also census the parent-field slots each node fills (`Parent.field -> Child`)
    #[argh(switch)]
    slots: bool,

    /// only rows with at least this many occurrences (default 1)
    #[argh(option, default = "1")]
    min: usize,

    /// only the N most populous rows
    #[argh(option)]
    top: Option<usize>,

    /// file paths, directories, or glob patterns
    #[argh(positional)]
    paths: Vec<String>,
}

/// The corpus-wide tallies.
#[derive(Default)]
struct Census {
    /// Wire node `type` -> occurrences.
    nodes: HashMap<String, usize>,
    /// `Parent.field -> Child` -> occurrences (only filled under `--slots`).
    slots: HashMap<String, usize>,
    files_parsed: usize,
    files_rejected: usize,
    source_bytes: usize,
}

impl AstCensusCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let (files, skipped) = resolve_profile_files(&self.paths, |_| true)?;

        let mut census = Census::default();
        for path in &files {
            let Ok(source) = std::fs::read_to_string(path) else {
                census.files_rejected += 1;
                continue;
            };
            let parser = ParserType::from_extension(&path.to_string_lossy());
            let Some(value) = tsv_parse_to_value(&source, parser) else {
                census.files_rejected += 1;
                continue;
            };
            census.files_parsed += 1;
            census.source_bytes += source.len();
            walk(&value, None, &mut census, self.slots);
        }

        if census.files_parsed == 0 {
            eprintln!("No files parsed successfully.");
            return Err(CliError::Failed);
        }

        let mut rows = self.rank(&census.nodes);
        if self.bytes {
            attach_sizes(&mut rows);
        }

        if self.json {
            print_json(
                &census,
                &rows,
                self.slots.then(|| self.rank(&census.slots)),
                self.bytes,
            );
        } else {
            print_table(
                &census,
                &rows,
                self.slots.then(|| self.rank(&census.slots)),
                skipped,
                self.bytes,
            );
        }
        Ok(())
    }

    /// Most populous first, ties broken by name so a re-run reads identically.
    fn rank(&self, counts: &HashMap<String, usize>) -> Vec<Row> {
        let mut rows: Vec<Row> = counts
            .iter()
            .filter(|(_, n)| **n >= self.min)
            .map(|(name, &count)| Row {
                name: name.clone(),
                count,
                size: None,
            })
            .collect();
        rows.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
        if let Some(n) = self.top {
            rows.truncate(n);
        }
        rows
    }
}

/// One census row.
struct Row {
    name: String,
    count: usize,
    /// The `size_of` the wire name joins to, when `--bytes` found exactly one.
    size: Option<usize>,
}

/// Walk the wire AST, counting every object that carries a `"type"` string.
///
/// `parent` is the `(type, field)` the current value sits in, so a node can be
/// attributed to the slot it fills — the reading that separates an object
/// literal's `Property` from a destructuring pattern's, which is the same
/// struct and so the same row.
fn walk(value: &Value, parent: Option<(&str, &str)>, census: &mut Census, slots: bool) {
    match value {
        Value::Object(map) => {
            let node_type = map.get("type").and_then(Value::as_str);
            if let Some(t) = node_type {
                *census.nodes.entry(t.to_owned()).or_default() += 1;
                if slots && let Some((parent_type, field)) = parent {
                    *census
                        .slots
                        .entry(format!("{parent_type}.{field} -> {t}"))
                        .or_default() += 1;
                }
            }
            for (key, child) in map {
                // `loc` / `start` / `end` are position payload, not structure —
                // they carry no `type` and so contribute nothing, but skipping
                // `loc` explicitly keeps the walk off the biggest sub-object in
                // the document.
                if key == "loc" {
                    continue;
                }
                let next = node_type.map(|t| (t, key.as_str()));
                walk(child, next.or(parent), census, slots);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, parent, census, slots);
            }
        }
        _ => {}
    }
}

/// Join rows against the `size_of` board by bare type name.
///
/// The wire's `type` strings are the acorn/Svelte node names and tsv's internal
/// structs are named after them, so the join is identity for nearly every row —
/// but it is a JOIN, not an assumption: a name the board does not hold, or
/// holds twice at different widths (the same name in two language crates),
/// leaves the byte columns empty instead of picking one.
fn attach_sizes(rows: &mut [Row]) {
    let board = type_sizes::board();
    let mut by_name: HashMap<&str, Option<usize>> = HashMap::new();
    for entry in &board {
        // `ts::Property` -> `Property`.
        let bare = entry.name.rsplit("::").next().unwrap_or(entry.name);
        by_name
            .entry(bare)
            .and_modify(|slot| {
                // Two crates define this name: keep it only if they agree.
                if *slot != Some(entry.size) {
                    *slot = None;
                }
            })
            .or_insert(Some(entry.size));
    }
    for row in rows {
        row.size = by_name.get(row.name.as_str()).copied().flatten();
    }
}

#[allow(clippy::cast_precision_loss)]
fn megabytes(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

#[allow(clippy::cast_precision_loss)]
fn share(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        count as f64 * 100.0 / total as f64
    }
}

fn print_table(
    census: &Census,
    rows: &[Row],
    slot_rows: Option<Vec<Row>>,
    skipped: usize,
    bytes: bool,
) {
    let total: usize = census.nodes.values().sum();
    let name_w = rows.iter().map(|r| r.name.len()).max().unwrap_or(4).max(4);

    if bytes {
        println!(
            "{:<name_w$}  {:>9}  {:>7}  {:>5}  {:>9}",
            "node", "count", "share", "size", "MB"
        );
    } else {
        println!("{:<name_w$}  {:>9}  {:>7}", "node", "count", "share");
    }
    for row in rows {
        print!(
            "{:<name_w$}  {:>9}  {:>6.2}%",
            row.name,
            row.count,
            share(row.count, total)
        );
        if bytes {
            match row.size {
                Some(size) => print!("  {:>5}  {:>9.2}", size, megabytes(row.count * size)),
                None => print!("  {:>5}  {:>9}", "-", "-"),
            }
        }
        println!();
    }

    println!();
    println!(
        "{} nodes over {} files ({:.2} MB of source); {} rejected, {} invalid-fixture skips",
        total,
        census.files_parsed,
        megabytes(census.source_bytes),
        census.files_rejected,
        skipped
    );
    if bytes {
        // The board's own headline: slot bytes against source bytes. perf160
        // read 1.37x for the containers it narrowed, and retired 80% of it.
        let slot_bytes: usize = rows
            .iter()
            .filter_map(|r| r.size.map(|s| s * r.count))
            .sum();
        println!(
            "{:.2} MB of slots in the rows shown, against {:.2} MB of source ({:.2}x)",
            megabytes(slot_bytes),
            megabytes(census.source_bytes),
            if census.source_bytes == 0 {
                0.0
            } else {
                megabytes(slot_bytes) / megabytes(census.source_bytes)
            }
        );
    }

    if let Some(slot_rows) = slot_rows {
        let slot_total: usize = census.slots.values().sum();
        let slot_w = slot_rows
            .iter()
            .map(|r| r.name.len())
            .max()
            .unwrap_or(4)
            .max(4);
        println!();
        println!("{:<slot_w$}  {:>9}  {:>7}", "slot", "count", "share");
        for row in &slot_rows {
            println!(
                "{:<slot_w$}  {:>9}  {:>6.2}%",
                row.name,
                row.count,
                share(row.count, slot_total)
            );
        }
    }
}

fn print_json(census: &Census, rows: &[Row], slot_rows: Option<Vec<Row>>, bytes: bool) {
    let total: usize = census.nodes.values().sum();
    let nodes: Vec<_> = rows
        .iter()
        .map(|r| {
            let mut node = serde_json::json!({
                "name": r.name,
                "count": r.count,
                "share": share(r.count, total),
            });
            // The byte columns appear only under `--bytes`. Emitting them
            // always would spell two different facts the same way: a `null`
            // meaning "the join found no type of this name" and a `null`
            // meaning "nobody asked for sizes".
            if bytes {
                node["size"] = r.size.into();
                node["slot_bytes"] = r.size.map(|s| s * r.count).into();
            }
            node
        })
        .collect();
    let mut out = serde_json::json!({
        "nodes": nodes,
        "total_nodes": total,
        "files_parsed": census.files_parsed,
        "files_rejected": census.files_rejected,
        "source_bytes": census.source_bytes,
    });
    if let Some(slot_rows) = slot_rows {
        let slot_total: usize = census.slots.values().sum();
        let slots: Vec<_> = slot_rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "slot": r.name,
                    "count": r.count,
                    "share": share(r.count, slot_total),
                })
            })
            .collect();
        out["slots"] = Value::Array(slots);
    }
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn census_of(value: &Value, slots: bool) -> Census {
        let mut census = Census::default();
        walk(value, None, &mut census, slots);
        census
    }

    /// Every object carrying a `"type"` string is one node, at any depth and
    /// through arrays.
    #[test]
    fn nodes_are_counted_through_arrays_and_nesting() {
        let wire = serde_json::json!({
            "type": "Program",
            "body": [
                { "type": "ExpressionStatement", "expression": { "type": "Identifier" } },
                { "type": "ExpressionStatement", "expression": { "type": "Identifier" } }
            ]
        });
        let census = census_of(&wire, false);
        assert_eq!(census.nodes.get("Program"), Some(&1));
        assert_eq!(census.nodes.get("ExpressionStatement"), Some(&2));
        assert_eq!(census.nodes.get("Identifier"), Some(&2));
    }

    /// `loc` is position payload the walk skips; it holds no `type`, so the
    /// skip changes no count — it only keeps the walk off the document's
    /// largest sub-object. Pinned so a future `loc` shape can't smuggle a node
    /// in either.
    #[test]
    fn the_position_payload_contributes_nothing() {
        let wire = serde_json::json!({
            "type": "Identifier",
            "loc": { "start": { "line": 1, "column": 0 }, "end": { "line": 1, "column": 1 } },
            "start": 0,
            "end": 1
        });
        let census = census_of(&wire, false);
        assert_eq!(census.nodes.len(), 1);
        assert_eq!(census.nodes.get("Identifier"), Some(&1));
    }

    /// The slot reading is what separates one struct's two populations — an
    /// object literal's `Property` from a destructuring pattern's.
    #[test]
    fn a_slot_names_the_parent_field_it_fills() {
        let wire = serde_json::json!({
            "type": "Program",
            "body": [
                { "type": "ObjectExpression", "properties": [{ "type": "Property" }] },
                { "type": "ObjectPattern", "properties": [
                    { "type": "Property" }, { "type": "Property" }
                ] }
            ]
        });
        let census = census_of(&wire, true);
        assert_eq!(census.nodes.get("Property"), Some(&3));
        assert_eq!(
            census.slots.get("ObjectExpression.properties -> Property"),
            Some(&1)
        );
        assert_eq!(
            census.slots.get("ObjectPattern.properties -> Property"),
            Some(&2)
        );
    }

    /// The `--bytes` join is by bare name against the `size_of` board, and a
    /// name the board holds at two different widths must report neither.
    #[test]
    fn the_size_join_refuses_an_ambiguous_name() {
        let mut rows = vec![
            Row {
                name: "Property".to_owned(),
                count: 10,
                size: None,
            },
            // Defined in both `tsv_ts` and `tsv_css` at different widths.
            Row {
                name: "StringCooked".to_owned(),
                count: 10,
                size: None,
            },
            Row {
                name: "NotAThing".to_owned(),
                count: 10,
                size: None,
            },
        ];
        attach_sizes(&mut rows);
        assert_eq!(
            rows[0].size,
            Some(size_of::<tsv_ts::ast::internal::Property<'_>>())
        );
        assert_eq!(rows[2].size, None, "a name off the board reports no size");
        // `StringCooked` resolves only if the two crates happen to agree; the
        // assertion is that the join never picks one of two disagreeing widths.
        let ts = size_of::<tsv_ts::ast::internal::StringCooked<'_>>();
        let css = size_of::<tsv_css::ast::internal::StringCooked<'_>>();
        assert_eq!(rows[1].size, (ts == css).then_some(ts));
    }
}
