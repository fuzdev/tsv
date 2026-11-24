//! `key_order_audit` — one key order per node type on tsv's own wire.
//!
//! The wire's key order is tsv's own: keys, values and presence match the canonical parser,
//! the order does not have to, and the fixture gate compares tsv's wire with each
//! `expected*.json` as a key-order-insensitive `Value` (P2b, P4). What it does promise is
//! one order per node type, so a consumer — or a decoder generated from a field table —
//! meets each type's keys in one sequence. Nothing else holds that once the bytes stop
//! pinning order, so this audit states it: over tsv's own span-only wire for every fixture
//! document, every key sequence observed for a type must embed in ONE total order (a
//! topological sort of the "a before b" edges each sequence contributes; a cycle is the
//! report). It also flags a duplicate key in any object, which a `Value` read would
//! silently collapse to its last occurrence.
//!
//! **Child-bearing order.** One order per type is not enough on its own: Svelte's comment
//! walk (and its compiler's visitors) visit a node's children in key order, so the relative
//! order of the CHILD-BEARING keys — a key whose value is a positioned object (numeric
//! `start` and `end`) or an array holding one, and the two attached-comment lists by name —
//! must stay the canonical parser's. The
//! audit derives each type's child-bearing order from the oracle files the fixture tree
//! commits (`expected.json`, `expected_svelte.json`, each `expected_<stem>.json` pin; the
//! `{"error": …}` marker skipped) and requires tsv's to agree: the "a before b" edges both
//! sides contribute, over the keys either side ever saw child-bearing for the type, must
//! still sort. The keys Svelte's `remove_typescript_nodes` deletes before anything walks
//! them ([`TYPESCRIPT_ONLY_KEYS`]) are exempt — their position is tsv's to choose. An
//! oracle type that carries two child-bearing orders on its own is reported, never
//! silently exempted.
//!
//! **Documents** are `loc_wires`'s (every parseable source file in each fixture directory,
//! at the fixture's goal): inputs and variants, prettier-side forms included. A document tsv
//! rejects is skipped and counted — the fixture gate and `check:loc` own that verdict.
//!
//! **Keying.** An object with a string `type` keys by it. CSS nodes key apart, as
//! `css:<type>` (everything under a `StyleSheetFile` or a Svelte `StyleSheet`, its
//! `attributes` aside): CSS's `Block` and the JS comment `Block` share a name and nothing
//! else. A `type`-less object keys by where it sits, `~<parent>.<field>`, so its order is
//! held too.
//!
//! Pure Rust — no sidecar. Part of `npm run check` (`npm run order:audit`).

use super::loc_wires::documents;
use crate::cli::CliError;
use crate::fixtures::validation::parsed_input::{ParsedInput, parse_input};
use crate::fixtures::{self, FixtureFiles, InputType};
use argh::FromArgs;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

/// The child-bearing keys Svelte's `remove_typescript_nodes`
/// (`svelte/src/compiler/phases/1-parse/remove_typescript_nodes.js`) deletes before any
/// visitor walks the tree, so their position among a node's children is tsv's to choose.
const TYPESCRIPT_ONLY_KEYS: &[&str] = &[
    "typeAnnotation",
    "typeParameters",
    "typeArguments",
    "returnType",
    "implements",
    "superTypeParameters",
    "superTypeArguments",
];

/// `serde_json`'s private key for a number under `arbitrary_precision`.
const ARBITRARY_PRECISION_NUMBER: &str = "$serde_json::private::Number";

/// How many distinct sequences a conflict report lists per type.
const SEQUENCES_SHOWN: usize = 8;

/// Audit that every node type on tsv's own wire carries one key order.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "key_order_audit")]
pub struct KeyOrderAuditCommand {
    /// print every type's total key order (the order the audit derived)
    #[argh(switch)]
    orders: bool,

    /// fixture tree to walk (default: tests/fixtures)
    #[argh(positional, default = "PathBuf::from(\"tests/fixtures\")")]
    root: PathBuf,
}

/// A JSON value kept only as far as the audit needs: objects keep every key in order,
/// duplicates included; strings are kept for `type`.
enum Node {
    Object(Vec<(String, Node)>),
    Array(Vec<Node>),
    Str(String),
    Num,
    Other,
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }
    fn visit_bool<E>(self, _: bool) -> Result<Node, E> {
        Ok(Node::Other)
    }
    fn visit_i64<E>(self, _: i64) -> Result<Node, E> {
        Ok(Node::Num)
    }
    fn visit_u64<E>(self, _: u64) -> Result<Node, E> {
        Ok(Node::Num)
    }
    fn visit_f64<E>(self, _: f64) -> Result<Node, E> {
        Ok(Node::Num)
    }
    fn visit_unit<E>(self) -> Result<Node, E> {
        Ok(Node::Other)
    }
    fn visit_str<E>(self, v: &str) -> Result<Node, E> {
        Ok(Node::Str(v.to_owned()))
    }
    fn visit_string<E>(self, v: String) -> Result<Node, E> {
        Ok(Node::Str(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Node>()? {
            items.push(item);
        }
        Ok(Node::Array(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut entries = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            // `arbitrary_precision` (on in this crate's `serde_json`) hands a number to
            // `deserialize_any` as a one-entry map under this private key.
            if entries.is_empty() && key == ARBITRARY_PRECISION_NUMBER {
                map.next_value::<String>()?;
                return Ok(Node::Num);
            }
            let value = map.next_value::<Node>()?;
            entries.push((key, value));
        }
        Ok(Node::Object(entries))
    }
}

/// Every distinct key sequence seen per type label, each with the first document it came from.
type Observed = BTreeMap<String, HashMap<Vec<String>, String>>;

/// A duplicate key: the type label, the key, the document.
struct Duplicate {
    label: String,
    key: String,
    document: String,
}

/// What one side's documents (tsv's wires, or the oracle files) show.
#[derive(Default)]
struct Side {
    observed: Observed,
    /// Per type label, every key seen holding a positioned object or an array with one.
    child_keys: BTreeMap<String, HashSet<String>>,
    duplicates: Vec<Duplicate>,
}

/// Whether `node` is a positioned object: numeric `start` and `end`.
fn is_positioned(node: &Node) -> bool {
    let Node::Object(entries) = node else {
        return false;
    };
    let numeric = |name: &str| {
        entries
            .iter()
            .any(|(key, value)| key == name && matches!(value, Node::Num))
    };
    numeric("start") && numeric("end")
}

/// Whether `key` is one of the attached-comment lists, child-bearing by name: what one holds
/// need not be positioned (Svelte's preceding-HTML entry on a `<script>` `Program` is a
/// bare `{type, value}`), and its relative order is the walk's.
fn is_comment_list(key: &str) -> bool {
    key == "leadingComments" || key == "trailingComments"
}

/// Whether `value` makes its key child-bearing here: a positioned object, or an array
/// holding one.
fn is_child_bearing(value: &Node) -> bool {
    match value {
        Node::Array(items) => items.iter().any(is_positioned),
        _ => is_positioned(value),
    }
}

impl Side {
    /// Record `node`'s objects, flagging duplicate keys.
    fn walk(&mut self, node: &Node, css: bool, slot: &str, document: &str) {
        match node {
            Node::Array(items) => {
                for item in items {
                    self.walk(item, css, slot, document);
                }
            }
            Node::Object(entries) => {
                let ty = entries.iter().find_map(|(key, value)| match value {
                    Node::Str(s) if key == "type" => Some(s.as_str()),
                    _ => None,
                });
                let css = css || matches!(ty, Some("StyleSheetFile" | "StyleSheet"));
                let label = match ty {
                    Some(ty) if css => format!("css:{ty}"),
                    Some(ty) => ty.to_owned(),
                    None => format!("~{}", slot.trim_start_matches('~')),
                };
                let keys: Vec<String> = entries.iter().map(|(key, _)| key.clone()).collect();
                for (i, key) in keys.iter().enumerate() {
                    if keys[..i].contains(key) {
                        self.duplicates.push(Duplicate {
                            label: label.clone(),
                            key: key.clone(),
                            document: document.to_owned(),
                        });
                    }
                }
                for (key, value) in entries {
                    if is_comment_list(key) || is_child_bearing(value) {
                        self.child_keys
                            .entry(label.clone())
                            .or_default()
                            .insert(key.clone());
                    }
                }
                let sequences = self.observed.entry(label.clone()).or_default();
                sequences.entry(keys).or_insert_with(|| document.to_owned());
                for (key, value) in entries {
                    // A Svelte `<style>`'s attributes are template nodes, not CSS.
                    let child_css = css && !(ty == Some("StyleSheet") && key == "attributes");
                    self.walk(value, child_css, &format!("{label}.{key}"), document);
                }
            }
            Node::Str(_) | Node::Num | Node::Other => {}
        }
    }

    /// Read `bytes` as JSON and record it; `Err` carries the reader's message.
    fn record(&mut self, bytes: &[u8], css: bool, document: &str) -> Result<(), String> {
        let node = crate::json::from_slice::<Node>(bytes).map_err(|e| e.to_string())?;
        // The canonical parser's rejection marker holds no tree.
        if let Node::Object(entries) = &node
            && entries.len() == 1
            && entries[0].0 == "error"
        {
            return Ok(());
        }
        self.walk(&node, css, "root", document);
        Ok(())
    }
}

/// The one total order every sequence embeds in, or a cycle among the keys.
fn total_order<'a>(
    sequences: impl IntoIterator<Item = &'a [String]>,
) -> Result<Vec<String>, Vec<String>> {
    // Keys numbered by first appearance across the sequences in a fixed order, so the
    // derived order is deterministic.
    let mut ordered: Vec<&[String]> = sequences.into_iter().collect();
    ordered.sort_unstable();
    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut names: Vec<&str> = Vec::new();
    for sequence in &ordered {
        for key in *sequence {
            index.entry(key.as_str()).or_insert_with(|| {
                names.push(key.as_str());
                names.len() - 1
            });
        }
    }
    let n = names.len();
    let mut successors = vec![Vec::new(); n];
    let mut predecessors = vec![Vec::new(); n];
    for sequence in &ordered {
        for pair in sequence.windows(2) {
            let (a, b) = (index[pair[0].as_str()], index[pair[1].as_str()]);
            if !successors[a].contains(&b) {
                successors[a].push(b);
                predecessors[b].push(a);
            }
        }
    }
    let mut in_degree: Vec<usize> = predecessors.iter().map(Vec::len).collect();
    let mut done = vec![false; n];
    let mut order = Vec::with_capacity(n);
    while let Some(next) = (0..n).find(|&k| !done[k] && in_degree[k] == 0) {
        done[next] = true;
        order.push(names[next].to_owned());
        for &b in &successors[next] {
            in_degree[b] -= 1;
        }
    }
    if order.len() == n {
        return Ok(order);
    }
    // Every key left has a predecessor left: walk predecessors until one repeats.
    let mut at = (0..n).find(|&k| !done[k]).unwrap_or(0);
    let mut path = Vec::new();
    while !path.contains(&at) {
        path.push(at);
        at = predecessors[at]
            .iter()
            .copied()
            .find(|&p| !done[p])
            .unwrap_or(at);
    }
    let start = path.iter().position(|&k| k == at).unwrap_or(0);
    let mut cycle: Vec<String> = path[start..]
        .iter()
        .rev()
        .map(|&k| names[k].to_owned())
        .collect();
    cycle.push(cycle[0].clone());
    Err(cycle)
}

/// Print a type's distinct sequences, at most [`SEQUENCES_SHOWN`] of them.
fn print_sequences<'a>(sequences: impl IntoIterator<Item = (&'a [String], &'a str)>) {
    let mut listed: Vec<(&[String], &str)> = sequences.into_iter().collect();
    listed.sort_unstable();
    listed.dedup_by(|a, b| a.0 == b.0);
    for (keys, document) in listed.iter().take(SEQUENCES_SHOWN) {
        println!("    {}  ({document})", keys.join(","));
    }
    if listed.len() > SEQUENCES_SHOWN {
        println!("    … {} more", listed.len() - SEQUENCES_SHOWN);
    }
}

/// `observed`'s sequences for `label`, cut to `keys`, each with its document.
fn filtered<'a>(
    observed: &'a Observed,
    label: &str,
    keys: &HashSet<&str>,
) -> Vec<(Vec<String>, &'a str)> {
    observed.get(label).map_or_else(Vec::new, |sequences| {
        sequences
            .iter()
            .map(|(sequence, document)| {
                let cut = sequence
                    .iter()
                    .filter(|key| keys.contains(key.as_str()))
                    .cloned()
                    .collect();
                (cut, document.as_str())
            })
            .collect()
    })
}

impl KeyOrderAuditCommand {
    /// Run the command.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the tree can't be walked, a document or an oracle
    /// file can't be read, a wire can't be read back, a type carries two key orders, tsv's
    /// child-bearing order disagrees with the oracle's, an oracle type carries two
    /// child-bearing orders, an object repeats a key, or nothing was graded.
    pub fn run(self) -> Result<(), CliError> {
        let fixtures = fixtures::walk_fixtures(&self.root).map_err(|e| {
            eprintln!("Error: {e}");
            CliError::Failed
        })?;
        let mut ours = Side::default();
        let mut oracle = Side::default();
        let mut graded = 0usize;
        let mut oracle_files = 0usize;
        let mut rejected = 0usize;
        let mut failed = false;
        for fixture in &fixtures {
            let input_type = fixture.input_type();
            let css = input_type == InputType::Css;
            let files = FixtureFiles::scan(fixture);
            let pins = files.expected_variant.iter().map(|entry| entry.pin.clone());
            for name in [
                "expected.json".to_owned(),
                "expected_svelte.json".to_owned(),
            ]
            .into_iter()
            .chain(pins)
            {
                let path = fixture.path.join(&name);
                if !path.exists() {
                    continue;
                }
                let relative = format!("{}/{name}", fixture.relative_path);
                let recorded = std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| oracle.record(&bytes, css, &relative));
                if let Err(e) = recorded {
                    eprintln!("{relative}: {e}");
                    failed = true;
                    continue;
                }
                oracle_files += 1;
            }
            if fixture.tsv_rejects_path().exists() {
                continue;
            }
            for (name, _) in documents(fixture) {
                let relative = format!("{}/{name}", fixture.relative_path);
                let source = match fixtures::read_file(&fixture.path.join(&name)) {
                    Ok(source) => source,
                    Err(e) => {
                        eprintln!("{relative}: {e}");
                        failed = true;
                        continue;
                    }
                };
                let Ok(parsed) = parse_input(&source, input_type, fixture.goal()) else {
                    rejected += 1;
                    continue;
                };
                let wire = match &parsed {
                    ParsedInput::Svelte(ast) => {
                        tsv_svelte::__private::convert_ast_json_bytes(ast, &source)
                    }
                    ParsedInput::Ts(ast) => tsv_ts::__private::convert_ast_json_bytes(ast, &source),
                    ParsedInput::Css(ast) => {
                        tsv_css::__private::convert_ast_json_bytes(ast, &source)
                    }
                };
                if let Err(e) = ours.record(&wire, css, &relative) {
                    eprintln!("{relative}: the wire does not read back: {e}");
                    failed = true;
                    continue;
                }
                graded += 1;
            }
        }

        // One key order per type, over tsv's wire.
        let mut conflicts = 0usize;
        for (label, sequences) in &ours.observed {
            match total_order(sequences.keys().map(Vec::as_slice)) {
                Ok(order) => {
                    if self.orders {
                        println!("{label}: {}", order.join(", "));
                    }
                }
                Err(cycle) => {
                    conflicts += 1;
                    println!(
                        "CONFLICT {label}: {} key orders, cycle {}",
                        sequences.len(),
                        cycle.join(" < ")
                    );
                    print_sequences(
                        sequences
                            .iter()
                            .map(|(keys, document)| (keys.as_slice(), document.as_str())),
                    );
                }
            }
        }

        // The child-bearing keys' relative order, against the oracle's.
        let mut oracle_multi = 0usize;
        let mut child_conflicts = 0usize;
        let labels: BTreeSet<&String> = ours
            .child_keys
            .keys()
            .chain(oracle.child_keys.keys())
            .collect();
        for label in labels {
            let keys: HashSet<&str> = ours
                .child_keys
                .get(label)
                .into_iter()
                .chain(oracle.child_keys.get(label))
                .flatten()
                .map(String::as_str)
                .filter(|key| !TYPESCRIPT_ONLY_KEYS.contains(key))
                .collect();
            if keys.len() < 2 {
                continue;
            }
            let theirs = filtered(&oracle.observed, label, &keys);
            if let Err(cycle) = total_order(theirs.iter().map(|(keys, _)| keys.as_slice())) {
                oracle_multi += 1;
                println!(
                    "ORACLE {label}: the canonical parser's own child-bearing keys take more than \
                     one order, cycle {} — an exemption must be named deliberately; none is assumed",
                    cycle.join(" < ")
                );
                print_sequences(
                    theirs
                        .iter()
                        .map(|(keys, document)| (keys.as_slice(), *document)),
                );
                continue;
            }
            let mine = filtered(&ours.observed, label, &keys);
            let both = theirs.iter().chain(&mine).map(|(keys, _)| keys.as_slice());
            if let Err(cycle) = total_order(both) {
                child_conflicts += 1;
                println!(
                    "CHILD-ORDER {label}: tsv's child-bearing order disagrees with the oracle's, \
                     cycle {}",
                    cycle.join(" < ")
                );
                println!("  oracle:");
                print_sequences(
                    theirs
                        .iter()
                        .map(|(keys, document)| (keys.as_slice(), *document)),
                );
                println!("  tsv:");
                print_sequences(
                    mine.iter()
                        .map(|(keys, document)| (keys.as_slice(), *document)),
                );
            }
        }

        for side in [&ours, &oracle] {
            for duplicate in &side.duplicates {
                println!(
                    "DUPLICATE key {:?} in a {} object ({})",
                    duplicate.key, duplicate.label, duplicate.document
                );
            }
        }
        let duplicates = ours.duplicates.len() + oracle.duplicates.len();
        println!(
            "key_order_audit: {graded} documents graded ({rejected} tsv rejects skipped), \
             {oracle_files} oracle files read, {} types; {conflicts} with more than one key \
             order, {child_conflicts} child-bearing orders against the oracle's, {oracle_multi} \
             oracle types with more than one child-bearing order, {duplicates} duplicate keys",
            ours.observed.len(),
        );
        if graded == 0 || oracle_files == 0 {
            eprintln!("key_order_audit: nothing graded — refusing to pass");
            return Err(CliError::Failed);
        }
        if failed || conflicts > 0 || child_conflicts > 0 || oracle_multi > 0 || duplicates > 0 {
            return Err(CliError::Failed);
        }
        Ok(())
    }
}
