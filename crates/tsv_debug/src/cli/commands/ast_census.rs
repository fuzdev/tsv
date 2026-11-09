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
//! - **Unwrapped wrappers** — `ParenthesizedExpression` and `CommentBoundPair` are
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
//!
//! # `--arity`
//!
//! For every array-valued field on the wire, the distribution of list lengths
//! by `Parent.field`: lists of length 0, 1, 2 and 3 or more, that last bucket
//! split as 3-8, 9-16 and 17 or more, the longest list, the total lists and the
//! total elements. An array counts once for its own length, so a nested
//! list adds to its own field and not to its parent's elements. An array under
//! a typeless object extends the path (`Root.options.attributes`). A `null`
//! element is an element (`ArrayExpression.elements` holes occupy slots), and
//! a list of strings counts like any other (`OnDirective.modifiers`).
//!
//! Two kinds of wire array are not child lists and have no row: the comment
//! arrays (`comments`, `leadingComments`, `trailingComments`, which the writer
//! derives from the detached comment table) and the arrays with no internal
//! list behind them (`Root.js`, a constant empty array, and
//! `ImportExpression.arguments`, which holds the node's one optional `options`
//! expression). The walk still descends into their elements.
//!
//! ⚠️ **The wire is not the internal AST, so a row is not a slice field.** The
//! census reads what the writer emits; where that differs from `&'arena [T]`:
//!
//! - **Only an array counts.** A field that is sometimes `null`, a boolean or an
//!   object counts only when it is an array. `Attribute.value` is the one where
//!   that hides slices: internally `Option<&[AttributeValue]>`, its boolean form
//!   is `None` and a bare `{expr}` is a length-one slice the writer emits as a
//!   plain object, so the field's 1 bucket is a lower bound.
//! - **The 0 bucket is the writer's, not the parser's.** `decorators`,
//!   `implements` and `TSInterfaceDeclaration.extends` are omitted when empty,
//!   and a list inside an absent optional node (`TSTypeParameterInstantiation.params`)
//!   has no list at all, so none of them can read 0. The reverse too:
//!   `ImportDeclaration.attributes` and its export kin are `Option`s the writer
//!   emits as `[]` when `None` only in a Svelte component that does not parse as
//!   TypeScript (the first `<script>` with a `lang` decides, for both scripts), and
//!   omits elsewhere, so the row covers those components plus explicit clauses, and
//!   its 0 bucket counts absent lists alongside an explicit `with {}`.
//! - **Wire names, wire merges.** A row carries the wire's spelling
//!   (`StyleSheet.children` and `StyleSheetFile.children` for CSS's `nodes`,
//!   `SelectorList.children` for its `selectors`, `TSFunctionType.parameters`
//!   for `params`), and a template literal type's `types` reads as
//!   `TemplateLiteral.expressions`, in one row with the value form's.
//! - **Lists the wire does not have.** CSS value and condition lists
//!   (`CssValue` arguments and value lists, `ConditionPart`, `ConditionSegment`)
//!   are emitted as strings and have no row. The reverse exists too: `::part()`'s
//!   flat name list is emitted as nested selector lists, which land in the
//!   `SelectorList`, `ComplexSelector` and `RelativeSelector` rows.

use argh::FromArgs;
use serde_json::Value;
use std::collections::HashMap;

use crate::audit::properties::tsv_parse_to_value;
use crate::cli::CliError;
use tsv_cli::cli::input::ParserType;

use super::profile::{pct, resolve_profile_files};
use super::type_sizes;

/// Census of AST node kinds over a corpus.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "ast_census")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is one independent CLI switch"
)]
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

    /// also census list lengths by `Parent.field` (0 / 1 / 2 / 3+, the 3+ tail, longest, totals)
    #[argh(switch)]
    arity: bool,

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
    /// `Parent.field` -> list lengths (only filled under `--arity`).
    arity: HashMap<String, ListArity>,
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
            if self.arity {
                walk_arity(&value, None, &mut census.arity);
            }
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
                self.arity.then(|| self.rank_arity(&census.arity)),
                self.bytes,
            );
        } else {
            print_table(
                &census,
                &rows,
                self.slots.then(|| self.rank(&census.slots)),
                self.arity.then(|| self.rank_arity(&census.arity)),
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

    /// Most lists first, ties broken by name; `--min` and `--top` read the list count.
    fn rank_arity(&self, arity: &HashMap<String, ListArity>) -> Vec<(String, ListArity)> {
        let mut rows: Vec<(String, ListArity)> = arity
            .iter()
            .filter(|(_, a)| a.lists() >= self.min)
            .map(|(name, a)| (name.clone(), *a))
            .collect();
        rows.sort_by(|a, b| b.1.lists().cmp(&a.1.lists()).then_with(|| a.0.cmp(&b.0)));
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

/// List-length tallies for one `Parent.field`.
#[derive(Clone, Copy, Default)]
struct ListArity {
    /// Lists of length 0, 1, 2, and 3 or more.
    buckets: [usize; 4],
    /// The 3-or-more bucket split: lists of length 3-8, 9-16, and 17 or more.
    tail: [usize; 3],
    /// The longest list seen.
    max: usize,
    elements: usize,
}

impl ListArity {
    fn record(&mut self, len: usize) {
        self.buckets[len.min(3)] += 1;
        match len {
            0..=2 => {}
            3..=8 => self.tail[0] += 1,
            9..=16 => self.tail[1] += 1,
            _ => self.tail[2] += 1,
        }
        self.max = self.max.max(len);
        self.elements += len;
    }

    fn merge(&mut self, other: &Self) {
        for (bucket, n) in self.buckets.iter_mut().zip(other.buckets) {
            *bucket += n;
        }
        for (bucket, n) in self.tail.iter_mut().zip(other.tail) {
            *bucket += n;
        }
        self.max = self.max.max(other.max);
        self.elements += other.elements;
    }

    fn lists(&self) -> usize {
        self.buckets.iter().sum()
    }

    fn at_most_two(&self) -> usize {
        self.buckets[0] + self.buckets[1] + self.buckets[2]
    }
}

/// Every wire array that is a child list, by the rule in the module doc.
fn is_child_list(field: &str) -> bool {
    let key = field.rsplit('.').next().unwrap_or(field);
    !matches!(key, "comments" | "leadingComments" | "trailingComments")
        && !matches!(field, "Root.js" | "ImportExpression.arguments")
}

/// Walk the wire AST, tallying every array by the `Parent.field` it sits in.
///
/// A separate walk from [`walk`] because the two name a position differently: a
/// slot keeps the nearest typed parent's field, while a list under a typeless
/// object needs the whole path (`Root.options.attributes`, not `Root.options`).
/// It visits arrays and not nodes, so an empty list is tallied like any other.
fn walk_arity(value: &Value, field: Option<&str>, arity: &mut HashMap<String, ListArity>) {
    match value {
        Value::Object(map) => {
            let node_type = map.get("type").and_then(Value::as_str);
            for (key, child) in map {
                if key == "loc" || !(child.is_array() || child.is_object()) {
                    continue;
                }
                let next = match (node_type, field) {
                    (Some(t), _) => format!("{t}.{key}"),
                    (None, Some(f)) => format!("{f}.{key}"),
                    (None, None) => key.clone(),
                };
                walk_arity(child, Some(&next), arity);
            }
        }
        Value::Array(items) => {
            if let Some(field) = field
                && is_child_list(field)
            {
                arity
                    .entry(field.to_owned())
                    .or_default()
                    .record(items.len());
            }
            for item in items {
                // An array directly inside an array is a list of its own, not
                // more of the outer field's.
                if let (Value::Array(_), Some(field)) = (item, field) {
                    walk_arity(item, Some(&format!("{field}[]")), arity);
                } else {
                    walk_arity(item, field, arity);
                }
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

#[expect(clippy::cast_precision_loss)]
fn megabytes(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn print_table(
    census: &Census,
    rows: &[Row],
    slot_rows: Option<Vec<Row>>,
    arity_rows: Option<Vec<(String, ListArity)>>,
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
            pct(row.count, total)
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
                pct(row.count, slot_total)
            );
        }
    }

    if let Some(arity_rows) = arity_rows {
        print_arity_table(&census.arity, &arity_rows);
    }
}

fn print_arity_table(arity: &HashMap<String, ListArity>, rows: &[(String, ListArity)]) {
    let name_w = rows
        .iter()
        .map(|(name, _)| name.len())
        .max()
        .unwrap_or(0)
        .max("all fields".len());
    println!();
    println!(
        "{:<name_w$}  {:>9}  {:>9}  {:>8}  {:>8}  {:>8}  {:>8} | {:>7}  {:>7}  {:>7} | {:>6}  {:>7}",
        "list field",
        "lists",
        "elements",
        "len 0",
        "len 1",
        "len 2",
        "len 3+",
        "3-8",
        "9-16",
        "17+",
        "max",
        "<=2"
    );
    let print_row = |name: &str, a: &ListArity| {
        println!(
            "{:<name_w$}  {:>9}  {:>9}  {:>8}  {:>8}  {:>8}  {:>8} | {:>7}  {:>7}  {:>7} | {:>6}  {:>6.2}%",
            name,
            a.lists(),
            a.elements,
            a.buckets[0],
            a.buckets[1],
            a.buckets[2],
            a.buckets[3],
            a.tail[0],
            a.tail[1],
            a.tail[2],
            a.max,
            pct(a.at_most_two(), a.lists()),
        );
    };
    for (name, a) in rows {
        print_row(name, a);
    }
    print_row("all fields", &arity_total(arity));
}

/// Every field's tallies summed, ignoring `--min` and `--top`.
fn arity_total(arity: &HashMap<String, ListArity>) -> ListArity {
    let mut total = ListArity::default();
    for a in arity.values() {
        total.merge(a);
    }
    total
}

fn arity_json(a: &ListArity) -> Value {
    serde_json::json!({
        "lists": a.lists(),
        "elements": a.elements,
        "len_0": a.buckets[0],
        "len_1": a.buckets[1],
        "len_2": a.buckets[2],
        "len_3_plus": a.buckets[3],
        "len_3_to_8": a.tail[0],
        "len_9_to_16": a.tail[1],
        "len_17_plus": a.tail[2],
        "max": a.max,
        "share_at_most_2": pct(a.at_most_two(), a.lists()),
    })
}

fn print_json(
    census: &Census,
    rows: &[Row],
    slot_rows: Option<Vec<Row>>,
    arity_rows: Option<Vec<(String, ListArity)>>,
    bytes: bool,
) {
    let total: usize = census.nodes.values().sum();
    let nodes: Vec<_> = rows
        .iter()
        .map(|r| {
            let mut node = serde_json::json!({
                "name": r.name,
                "count": r.count,
                "share": pct(r.count, total),
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
                    "share": pct(r.count, slot_total),
                })
            })
            .collect();
        out["slots"] = Value::Array(slots);
    }
    if let Some(arity_rows) = arity_rows {
        let fields: Vec<_> = arity_rows
            .iter()
            .map(|(name, a)| {
                let mut row = arity_json(a);
                row["field"] = name.as_str().into();
                row
            })
            .collect();
        out["arity"] = Value::Array(fields);
        out["arity_total"] = arity_json(&arity_total(&census.arity));
    }
    super::print_json_pretty(&out);
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

    fn arity_of(value: &Value) -> HashMap<String, ListArity> {
        let mut arity = HashMap::new();
        walk_arity(value, None, &mut arity);
        arity
    }

    /// Each length lands in its bucket and the elements sum across them, with
    /// the empty list counted like any other.
    #[test]
    fn list_lengths_fill_the_four_buckets() {
        let id = serde_json::json!({ "type": "Identifier" });
        let wire = serde_json::json!({
            "type": "Program",
            "body": [
                { "type": "CallExpression", "arguments": [] },
                { "type": "CallExpression", "arguments": [id] },
                { "type": "CallExpression", "arguments": [id, id] },
                { "type": "CallExpression", "arguments": [id, id, id] },
                { "type": "CallExpression", "arguments": [id, id, id, id, id] },
                { "type": "CallExpression", "arguments": [] }
            ]
        });
        let arity = arity_of(&wire);
        let args = arity["CallExpression.arguments"];
        assert_eq!(args.buckets, [2, 1, 1, 2]);
        assert_eq!(args.lists(), 6);
        assert_eq!(args.elements, 11);
        assert_eq!(args.at_most_two(), 4);
        assert_eq!(arity["Program.body"].buckets, [0, 0, 0, 1]);
        assert_eq!(arity_total(&arity).lists(), 7);
    }

    /// The tail splits the 3-or-more bucket at its boundaries, and a list
    /// shorter than 3 stays out of it.
    #[test]
    fn the_tail_splits_at_eight_and_sixteen() {
        let mut a = ListArity::default();
        for len in [0, 2, 3, 8, 9, 16, 17, 40] {
            a.record(len);
        }
        assert_eq!(a.buckets, [1, 0, 1, 6]);
        assert_eq!(a.tail, [2, 2, 2]);
        assert_eq!(a.tail.iter().sum::<usize>(), a.buckets[3]);
        assert_eq!(a.max, 40);
        assert_eq!(a.elements, 95);
    }

    /// A merge sums the counts and keeps the larger maximum from either side.
    #[test]
    fn a_merge_keeps_the_longest_list_either_side_saw() {
        let mut short = ListArity::default();
        short.record(1);
        short.record(5);
        let mut long = ListArity::default();
        long.record(20);

        let mut total = ListArity::default();
        total.merge(&long);
        total.merge(&short);
        assert_eq!(total.max, 20);
        assert_eq!(total.buckets, [0, 1, 0, 2]);
        assert_eq!(total.tail, [1, 0, 1]);
        assert_eq!(total.elements, 26);

        let mut reversed = short;
        reversed.merge(&long);
        assert_eq!(reversed.max, 20);

        let arity = HashMap::from([("A.x".to_owned(), short), ("B.x".to_owned(), long)]);
        assert_eq!(arity_total(&arity).max, 20);
    }

    #[test]
    fn the_json_row_names_each_tail_bucket() {
        let mut a = ListArity::default();
        for len in [3, 9, 9, 17, 17, 17] {
            a.record(len);
        }
        let row = arity_json(&a);
        assert_eq!(row["len_3_plus"], 6);
        assert_eq!(row["len_3_to_8"], 1);
        assert_eq!(row["len_9_to_16"], 2);
        assert_eq!(row["len_17_plus"], 3);
        assert_eq!(row["max"], 17);
    }

    #[test]
    fn arity_rows_rank_by_lists_then_name() {
        let mut arity = HashMap::new();
        for (name, lists) in [("B.x", 2), ("A.x", 2), ("C.x", 5), ("D.x", 1)] {
            let entry: &mut ListArity = arity.entry(name.to_owned()).or_default();
            for _ in 0..lists {
                entry.record(1);
            }
        }
        let command = AstCensusCommand {
            json: false,
            bytes: false,
            slots: false,
            arity: true,
            min: 2,
            top: Some(3),
            paths: vec![],
        };
        let names: Vec<_> = command
            .rank_arity(&arity)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert_eq!(names, ["C.x", "A.x", "B.x"]);
    }

    /// A field counts only when it is an array, whatever else it holds.
    #[test]
    fn only_an_array_value_counts() {
        let wire = serde_json::json!({
            "type": "Fragment",
            "nodes": [
                { "type": "Attribute", "value": null },
                { "type": "Attribute", "value": true },
                { "type": "Attribute", "value": { "type": "ExpressionTag" } },
                { "type": "Attribute", "value": [{ "type": "Text" }] }
            ]
        });
        let arity = arity_of(&wire);
        assert_eq!(arity["Attribute.value"].lists(), 1);
        assert_eq!(arity["Attribute.value"].elements, 1);
    }

    /// A nested list tallies under its own field and adds only one element to
    /// the list holding its node; a `null` hole is an element.
    #[test]
    fn a_nested_list_counts_for_its_own_field_only() {
        let wire = serde_json::json!({
            "type": "ArrayExpression",
            "elements": [
                null,
                { "type": "ArrayExpression", "elements": [
                    { "type": "ObjectExpression", "properties": [] }
                ] }
            ]
        });
        let arity = arity_of(&wire);
        let elements = arity["ArrayExpression.elements"];
        assert_eq!(elements.buckets, [0, 1, 1, 0]);
        assert_eq!(elements.elements, 3);
        assert_eq!(arity["ObjectExpression.properties"].buckets, [1, 0, 0, 0]);
    }

    /// An array directly inside an array is a list of its own.
    #[test]
    fn an_array_of_arrays_keeps_the_inner_lists_apart() {
        let wire = serde_json::json!({ "type": "X", "rows": [[1, 2], []] });
        let arity = arity_of(&wire);
        assert_eq!(arity["X.rows"].buckets, [0, 0, 1, 0]);
        assert_eq!(arity["X.rows"].elements, 2);
        assert_eq!(arity["X.rows[]"].buckets, [1, 0, 1, 0]);
    }

    /// The arrays that are not child lists have no row, and what they hold is
    /// still walked.
    #[test]
    fn non_child_list_arrays_have_no_row() {
        let wire = serde_json::json!({
            "type": "Root",
            "js": [],
            "comments": [{ "type": "Line" }],
            "fragment": {
                "type": "Fragment",
                "nodes": [{
                    "type": "ImportExpression",
                    "loc": { "start": { "line": 1, "column": 0 } },
                    "arguments": [{ "type": "ObjectExpression", "properties": [] }],
                    "leadingComments": [{ "type": "Block" }],
                    "trailingComments": []
                }]
            }
        });
        let arity = arity_of(&wire);
        let mut names: Vec<_> = arity.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["Fragment.nodes", "ObjectExpression.properties"]);
    }

    /// A typeless object extends the path, so a list keeps the field it sits
    /// under instead of reading as its holder's slot.
    #[test]
    fn a_typeless_object_extends_the_field_path() {
        let wire = serde_json::json!({
            "type": "Root",
            "options": { "attributes": [{ "type": "Attribute", "value": [] }] }
        });
        let arity = arity_of(&wire);
        assert_eq!(arity["Root.options.attributes"].lists(), 1);
        assert_eq!(arity["Attribute.value"].buckets, [1, 0, 0, 0]);
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
        assert_eq!(rows[0].size, Some(size_of::<tsv_ts::ast::Property>()));
        assert_eq!(rows[2].size, None, "a name off the board reports no size");
        // `StringCooked` resolves only if the two crates happen to agree; the
        // assertion is that the join never picks one of two disagreeing widths.
        let ts = size_of::<tsv_ts::ast::StringCooked<'_>>();
        let css = size_of::<tsv_css::ast::StringCooked<'_>>();
        assert_eq!(rows[1].size, (ts == css).then_some(ts));
    }
}
