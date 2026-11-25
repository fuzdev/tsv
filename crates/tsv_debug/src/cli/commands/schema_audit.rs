//! `schema_audit` — tsv's wire against the AST schema table.
//!
//! The table (`crates/tsv_wasm/types/tsv_ast_schema.json`, read by `scripts/ast_schema.ts`) is the
//! one description of the wire JSON: an entry per wire node type, per named variant of one, and
//! per `type`-less object, each listing its fields in wire order with what each holds and when
//! it is present. The generated `tsv_ast.d.ts` is one reading of it; this audit holds the
//! writers to it, over tsv's own wire for every fixture document — both wires, the span-only one
//! and the `loc`-bearing one — and holds the table to the canonical parser where the order of
//! a node's children is the canonical parser's to choose.
//!
//! **What every object must do.** It resolves to an entry — a typed object by its `type` (a
//! node of the CSS tree, everything under a `StyleSheet` or `StyleSheetFile` but a Svelte
//! `StyleSheet`'s `attributes`, by its CSS entry: the CSS `Block` and the JS comment `Block`
//! share a name and nothing else), narrowed to the interfaces the parent field's `ts` admits
//! there (which is how a named variant is told from its base where a position admits one and not
//! the other) and to the entries whose `positions` name the parent slot; a `type`-less object by
//! the parent field's `entry`. Then, against some entry among those (every one that conforms is
//! exercised; the findings reported are the base entry's where it conforms, else the narrowest
//! variant's, else the fewest):
//! every key is a field of it (`EXTRA`), in its field order (`ORDER`); every field the
//! entry requires on this wire, in this document's dialect, is there and every one it rules out
//! is not (`PRESENCE` — the dialect is vanilla acorn's in a Svelte component whose first
//! `lang`-bearing script is not `lang="ts"`, as the writer decides it); every value is one of the
//! field's kinds — a string, a number, a boolean, an object, a list of them, `null` only where
//! the field is nullable (`VALUE`); a `loc` carries `character` only where its field says it
//! may; a string keeps its class's promise where a position can tell (`STRING`,
//! `string_class_problem`). An object that resolves to no entry at all is `UNDECLARED`, and
//! no object may repeat a key (`DUPLICATE`) — which a `JSON.parse` or a `Value` read collapses to its last
//! occurrence, so this check lives here, streaming the bytes key by key.
//!
//! **What the table must do.** Every entry and every field is exercised by some object of the
//! fixture wire; presence from the other side too — in each dialect an acorn entry was met in, a
//! field `optional` there is seen absent there, and present there where the table states the
//! dialect apart or the other dialect showed it; every flag a field carries — nullable, an item
//! hole, `character`, each kind and keyword value it lists — by some value, and a `pooled`
//! string seen built at least once (`UNEXERCISED`). So a field the writer stopped emitting, an
//! entry for a shape it never builds, or an `optional` the writer always emits is drift, not
//! documentation. The entries of one `type` — a base and its named variants — share ONE key
//! order (`TABLE`), the promise a consumer, or a decoder generated from the table, relies on.
//! Every name a field's `ts` spells is an alias or an interface the table declares.
//!
//! **Child-bearing order is the canonical parser's.** Svelte's comment walk and its compiler's
//! visitors visit a node's children in key order, so the relative order of a type's
//! CHILD-BEARING keys — a key whose value is a positioned object (numeric `start` and `end`) or
//! an array holding one, and the two attached-comment lists by name — must be the canonical
//! parser's. The audit reads the oracle's own key sequences from the oracle files the fixture
//! tree commits (`expected.json`, `expected_svelte.json`, each `expected_<stem>.json` pin; the
//! `{"error": …}` marker skipped) and requires the table's order for the type to agree:
//! the "a before b" edges both contribute, over the keys either side holds child-bearing, must
//! still sort (`CHILD-ORDER`). The fields the table marks `ts_only` — the keys Svelte's
//! `remove_typescript_nodes` deletes before anything walks them — are exempt: their position is
//! tsv's to choose. An oracle type whose own child-bearing keys take two orders is reported
//! (`ORACLE`), never silently exempted.
//!
//! **Documents** are `loc_wires`' (every parseable source file in each fixture directory, at the
//! fixture's goal): inputs and variants, prettier-side forms included. A document tsv rejects is
//! skipped and counted — the fixture gate and `check:loc` own that verdict.
//!
//! Pure Rust — no sidecar. Part of `npm run check` (`npm run schema:audit`).

use super::loc_wires::documents;
use crate::cli::CliError;
use crate::fixtures::validation::parsed_input::{ParsedInput, parse_input};
use crate::fixtures::{self, FixtureFiles, InputType};
use argh::FromArgs;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;

/// `serde_json`'s private key for a number under `arbitrary_precision`.
const ARBITRARY_PRECISION_NUMBER: &str = "$serde_json::private::Number";

/// How many distinct sequences a child-order report lists per type.
const SEQUENCES_SHOWN: usize = 8;

/// How many findings of one class are printed before the rest are only counted.
const FINDINGS_SHOWN: usize = 40;

/// Audit tsv's wire against the AST schema table.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "schema_audit")]
pub struct SchemaAuditCommand {
    /// the table (default: crates/tsv_wasm/types/tsv_ast_schema.json)
    #[argh(
        option,
        default = "PathBuf::from(\"crates/tsv_wasm/types/tsv_ast_schema.json\")"
    )]
    table: PathBuf,

    /// fixture tree to walk (default: tests/fixtures)
    #[argh(positional, default = "PathBuf::from(\"tests/fixtures\")")]
    root: PathBuf,
}

//
// The wire, read key by key.
//

/// A JSON value kept as far as the audit needs: objects keep every key in order, duplicates
/// included.
enum Node {
    Object(Vec<(String, Node)>),
    Array(Vec<Node>),
    Str(String),
    /// A number, as far as a script's `start` is read: `NaN` when it does not fit an `f64`.
    Num(f64),
    Bool,
    Null,
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
        Ok(Node::Bool)
    }
    #[expect(clippy::cast_precision_loss, reason = "an offset, far below 2^53")]
    fn visit_i64<E>(self, v: i64) -> Result<Node, E> {
        Ok(Node::Num(v as f64))
    }
    #[expect(clippy::cast_precision_loss, reason = "an offset, far below 2^53")]
    fn visit_u64<E>(self, v: u64) -> Result<Node, E> {
        Ok(Node::Num(v as f64))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Node, E> {
        Ok(Node::Num(v))
    }
    fn visit_unit<E>(self) -> Result<Node, E> {
        Ok(Node::Null)
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
                let digits = map.next_value::<String>()?;
                return Ok(Node::Num(digits.parse().unwrap_or(f64::NAN)));
            }
            let value = map.next_value::<Node>()?;
            entries.push((key, value));
        }
        Ok(Node::Object(entries))
    }
}

impl Node {
    /// The value of the string field `key` of an object.
    fn str_field(&self, key: &str) -> Option<&str> {
        let Node::Object(entries) = self else {
            return None;
        };
        entries.iter().find_map(|(k, v)| match v {
            Node::Str(s) if k == key => Some(s.as_str()),
            _ => None,
        })
    }

    fn field(&self, key: &str) -> Option<&Node> {
        let Node::Object(entries) = self else {
            return None;
        };
        entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

//
// The table.
//

/// What a field's value is on the wire (`scripts/ast_schema.ts`'s `FieldKind`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    Type,
    Offset,
    Loc,
    Boolean,
    Number,
    String,
    Strings,
    Child,
    List,
    Object,
    Comments,
    Data,
    Null,
}

impl Kind {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "type" => Self::Type,
            "offset" => Self::Offset,
            "loc" => Self::Loc,
            "boolean" => Self::Boolean,
            "number" => Self::Number,
            "string" => Self::String,
            "strings" => Self::Strings,
            "child" => Self::Child,
            "list" => Self::List,
            "object" => Self::Object,
            "comments" => Self::Comments,
            "data" => Self::Data,
            "null" => Self::Null,
            _ => return None,
        })
    }
}

/// When a field is on the wire.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Presence {
    Always,
    Optional,
    Locations,
    Absent,
}

impl Presence {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "always" => Self::Always,
            "optional" => Self::Optional,
            "locations" => Self::Locations,
            "absent" => Self::Absent,
            _ => return None,
        })
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one table key, read independently"
)]
struct Field {
    key: String,
    kinds: Vec<Kind>,
    /// The `.d.ts` member type, or the `entry` an `object` or `loc` field holds.
    ts: Option<String>,
    entry: Option<String>,
    presence: Presence,
    vanilla_acorn: Option<Presence>,
    nullable: bool,
    items_nullable: bool,
    ts_only: bool,
    character: bool,
    /// The string class (`string_class_problem` grades what a position can decide).
    string: Option<String>,
    /// A `keyword` string's closed vocabulary.
    values: Option<Vec<String>>,
}

struct Entry {
    /// How a finding names it: `Literal/RegexLiteral`, `css:Rule`, `~SourceLocation`.
    name: String,
    interface: Option<String>,
    mixin: bool,
    /// A named variant (not the base entry of its `type`).
    variant: bool,
    /// The parent slots (`Type.key`) it may stand in, where the table limits them.
    positions: Option<Vec<String>>,
    /// An acorn node — one carrying the attached-comment lists: the one kind of entry the
    /// dialect axis (`vanilla_acorn`) is about, so the only one tallied per dialect.
    acorn: bool,
    fields: Vec<Field>,
}

struct Schema {
    entries: Vec<Entry>,
    /// Typed entries by (CSS tree, `type`).
    by_type: HashMap<(bool, String), Vec<usize>>,
    /// `type`-less entries by interface.
    typeless: HashMap<String, usize>,
    /// Every alias's union members.
    aliases: HashMap<String, Vec<String>>,
    /// Every declared interface name.
    interfaces: HashSet<String>,
}

/// The type names a TypeScript type expression mentions (quoted literals skipped), or `None`
/// for an expression that admits anything (`unknown`).
fn type_names(ts: &str) -> Option<Vec<&str>> {
    const PRIMITIVES: &[&str] = &[
        "null", "true", "false", "string", "number", "boolean", "never", "bigint",
    ];
    let mut names = Vec::new();
    let mut quoted = false;
    let mut start = None;
    for (i, c) in ts.char_indices().chain(std::iter::once((ts.len(), ' '))) {
        if c == '\'' {
            quoted = !quoted;
        }
        let ident = !quoted && (c.is_ascii_alphanumeric() || c == '_');
        match (ident, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                let name = &ts[s..i];
                if name == "unknown" {
                    return None;
                }
                if !PRIMITIVES.contains(&name) {
                    names.push(name);
                }
                start = None;
            }
            _ => {}
        }
    }
    Some(names)
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn json_strings(value: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
}

fn json_flag(value: &serde_json::Value, key: &str) -> bool {
    value.get(key).and_then(serde_json::Value::as_bool) == Some(true)
}

impl Schema {
    /// Read the table; `Err` names what does not read. Its vocabulary is `scripts/ast_schema.ts`'s,
    /// whose reader `check:ast-schema` runs: this one refuses what it cannot grade by.
    fn read(bytes: &[u8]) -> Result<Self, String> {
        let table: serde_json::Value = crate::json::from_slice(bytes).map_err(|e| e.to_string())?;
        let sections = table
            .get("sections")
            .and_then(serde_json::Value::as_array)
            .ok_or("no `sections` array")?;
        let mut schema = Self {
            entries: Vec::new(),
            by_type: HashMap::new(),
            typeless: HashMap::new(),
            aliases: HashMap::new(),
            interfaces: HashSet::new(),
        };
        for item in sections
            .iter()
            .filter_map(|s| s.get("entries").and_then(serde_json::Value::as_array))
            .flatten()
        {
            if let Some(alias) = json_str(item, "alias") {
                let union = item
                    .get("union")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| format!("alias {alias}: no `union`"))?
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect();
                schema.aliases.insert(alias.to_owned(), union);
                continue;
            }
            let ty = json_str(item, "type");
            let css = json_flag(item, "css");
            let interface = json_str(item, "interface").map(str::to_owned);
            let name = match (ty, json_str(item, "variant")) {
                (Some(ty), Some(variant)) => {
                    format!("{}{ty}/{variant}", if css { "css:" } else { "" })
                }
                (Some(ty), None) => format!("{}{ty}", if css { "css:" } else { "" }),
                (None, _) => format!("~{}", interface.as_deref().unwrap_or("?")),
            };
            let mut fields = Vec::new();
            for field in item
                .get("fields")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| format!("{name}: no `fields`"))?
            {
                let key = json_str(field, "key")
                    .ok_or_else(|| format!("{name}: a field with no `key`"))?;
                let kinds = match field.get("kind") {
                    Some(serde_json::Value::String(k)) => vec![k.as_str()],
                    Some(serde_json::Value::Array(ks)) => {
                        ks.iter().filter_map(serde_json::Value::as_str).collect()
                    }
                    _ => Vec::new(),
                };
                let kinds = kinds
                    .into_iter()
                    .map(|k| Kind::parse(k).ok_or_else(|| format!("{name}.{key}: kind {k:?}")))
                    .collect::<Result<Vec<_>, _>>()?;
                if kinds.is_empty() {
                    return Err(format!("{name}.{key}: no `kind`"));
                }
                let presence = |k: &str| -> Result<Option<Presence>, String> {
                    json_str(field, k)
                        .map(|p| {
                            Presence::parse(p).ok_or_else(|| format!("{name}.{key}: {k} {p:?}"))
                        })
                        .transpose()
                };
                fields.push(Field {
                    key: key.to_owned(),
                    kinds,
                    ts: json_str(field, "ts")
                        .or_else(|| json_str(field, "entry"))
                        .map(str::to_owned),
                    entry: json_str(field, "entry").map(str::to_owned),
                    presence: presence("presence")?.unwrap_or(Presence::Always),
                    vanilla_acorn: presence("vanilla_acorn")?,
                    nullable: json_flag(field, "nullable"),
                    items_nullable: json_flag(field, "items_nullable"),
                    ts_only: json_flag(field, "ts_only"),
                    character: json_flag(field, "character"),
                    string: json_str(field, "string").map(str::to_owned),
                    values: json_strings(field, "values"),
                });
            }
            let index = schema.entries.len();
            if let Some(interface) = &interface {
                schema.interfaces.insert(interface.clone());
            }
            match ty {
                Some(ty) => schema
                    .by_type
                    .entry((css, ty.to_owned()))
                    .or_default()
                    .push(index),
                None => {
                    if let Some(interface) = &interface {
                        schema.typeless.insert(interface.clone(), index);
                    }
                }
            }
            schema.entries.push(Entry {
                name,
                interface,
                mixin: json_flag(item, "mixin"),
                variant: json_str(item, "variant").is_some(),
                positions: json_strings(item, "positions"),
                acorn: fields.iter().any(|f| f.kinds == [Kind::Comments]),
                fields,
            });
        }
        Ok(schema)
    }

    /// The interfaces a field's type admits at its position, or `None` when it admits any.
    /// A comment list with no `ts` of its own takes the mixin's.
    fn admitted(&self, field: &Field) -> Option<HashSet<String>> {
        let ts = match &field.ts {
            Some(ts) => ts.as_str(),
            None if field.kinds.contains(&Kind::Comments) => self
                .entries
                .iter()
                .filter(|e| e.mixin)
                .flat_map(|e| &e.fields)
                .find(|f| f.key == field.key)
                .and_then(|f| f.ts.as_deref())?,
            None => return None,
        };
        let mut out = HashSet::new();
        let mut pending: Vec<String> = type_names(ts)?.into_iter().map(str::to_owned).collect();
        let mut seen = HashSet::new();
        while let Some(name) = pending.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            if let Some(union) = self.aliases.get(&name) {
                for member in union {
                    pending.extend(type_names(member)?.into_iter().map(str::to_owned));
                }
            } else {
                out.insert(name);
            }
        }
        Some(out)
    }

    /// Every name a field's `ts` spells that the table does not declare.
    fn unresolved_names(&self) -> Vec<String> {
        let mut out = BTreeSet::new();
        let ts_of_fields = self
            .entries
            .iter()
            .flat_map(|e| e.fields.iter().filter_map(|f| f.ts.as_deref()));
        let ts_of_aliases = self.aliases.values().flatten().map(String::as_str);
        for ts in ts_of_fields.chain(ts_of_aliases) {
            for name in type_names(ts).unwrap_or_default() {
                if !self.aliases.contains_key(name) && !self.interfaces.contains(name) {
                    out.insert(name.to_owned());
                }
            }
        }
        out.into_iter().collect()
    }
}

//
// Grading.
//

/// One document's context.
#[derive(Clone, Copy)]
struct Doc<'a> {
    path: &'a str,
    /// The source in UTF-16 code units, the space the wire's offsets count in.
    source: &'a [u16],
    /// Vanilla acorn's dialect (a Svelte component whose scripts are not TypeScript).
    vanilla: bool,
    /// The `loc`-bearing wire.
    locations: bool,
}

impl Doc<'_> {
    /// The dialect's index into the per-dialect tallies: acorn-typescript's `0`, vanilla acorn's `1`.
    const fn dialect(self) -> usize {
        if self.vanilla { 1 } else { 0 }
    }
}

/// The dialects' names, by `Doc::dialect`.
const DIALECTS: [&str; 2] = ["", " under vanilla acorn"];

/// A field's presence in a dialect: `vanilla_acorn` where it names one, else `presence`.
fn presence_in(field: &Field, vanilla: bool) -> Presence {
    match (vanilla, field.vanilla_acorn) {
        (true, Some(p)) => p,
        _ => field.presence,
    }
}

/// What a field was seen holding, over every object matched to its entry.
#[derive(Default, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one table claim, exercised independently"
)]
struct FieldUse {
    /// Per dialect (`Doc::dialect`): seen present on an object of the entry, seen absent.
    present: [bool; 2],
    absent: [bool; 2],
    null: bool,
    hole: bool,
    character: bool,
    kinds: HashSet<Kind>,
    /// A `keyword` field's values seen.
    values: HashSet<String>,
    /// A `pooled` string seen that is no range of its enclosing span.
    built: bool,
}

/// The findings, by class and then by message: one finding repeats across documents, so each
/// distinct one is kept once, with how often it was made and the first document that made it.
#[derive(Default)]
struct Findings {
    by_class: BTreeMap<&'static str, BTreeMap<String, (usize, String)>>,
}

impl Findings {
    fn push(&mut self, class: &'static str, message: String, document: &str) {
        let seen = self
            .by_class
            .entry(class)
            .or_default()
            .entry(message)
            .or_insert_with(|| (0, document.to_owned()));
        seen.0 += 1;
    }

    /// How many distinct findings were made.
    fn total(&self) -> usize {
        self.by_class.values().map(BTreeMap::len).sum()
    }

    fn print(&self) {
        for (class, messages) in &self.by_class {
            for (message, (count, document)) in messages.iter().take(FINDINGS_SHOWN) {
                let at = if document.is_empty() {
                    String::new()
                } else {
                    format!("  ({document}")
                };
                let close = if document.is_empty() { "" } else { ")" };
                if *count > 1 {
                    println!("{class} {message}{at}, ×{count}{close}");
                } else {
                    println!("{class} {message}{at}{close}");
                }
            }
            if messages.len() > FINDINGS_SHOWN {
                println!("{class} … {} more", messages.len() - FINDINGS_SHOWN);
            }
        }
    }
}

struct Grader<'s> {
    schema: &'s Schema,
    /// Objects matched per entry per dialect, and per entry per field what its values were.
    matched: Vec<[usize; 2]>,
    uses: Vec<Vec<FieldUse>>,
    findings: Findings,
    /// Per oracle-comparable label, the entry indices tsv's objects under it resolved to.
    label_entries: BTreeMap<String, BTreeSet<usize>>,
    /// Per label, the keys tsv's wire was seen holding a child under (`is_child_bearing`).
    child_keys: BTreeMap<String, HashSet<String>>,
}

/// The value classes a field's value belongs to.
fn value_kinds(value: &Node, field: &Field) -> Vec<Kind> {
    field
        .kinds
        .iter()
        .copied()
        .filter(|kind| match (kind, value) {
            (Kind::Data, _)
            | (Kind::Null, Node::Null)
            | (Kind::Type | Kind::String, Node::Str(_))
            | (Kind::Offset | Kind::Number, Node::Num(_))
            | (Kind::Boolean, Node::Bool)
            | (Kind::Child | Kind::Object | Kind::Loc, Node::Object(_)) => true,
            (Kind::Strings, Node::Array(items)) => items.iter().all(|i| matches!(i, Node::Str(_))),
            (Kind::List | Kind::Comments, Node::Array(items)) => items.iter().all(|i| {
                matches!(i, Node::Object(_)) || (field.items_nullable && matches!(i, Node::Null))
            }),
            _ => false,
        })
        .collect()
}

impl Grader<'_> {
    /// The entries `node` may resolve to at its position: by `type` narrowed to what the parent
    /// field admits, or the parent field's `entry`.
    fn candidates(
        &self,
        node: &Node,
        css: bool,
        parent: Option<&Field>,
        slot: &str,
    ) -> Result<Vec<usize>, String> {
        if let Some(ty) = node.str_field("type") {
            let all = self
                .schema
                .by_type
                .get(&(css, ty.to_owned()))
                .ok_or_else(|| format!("{}{ty}", if css { "css:" } else { "" }))?;
            let all: Vec<usize> = all
                .iter()
                .copied()
                .filter(|&i| {
                    let entry = &self.schema.entries[i];
                    !entry.mixin
                        && entry
                            .positions
                            .as_ref()
                            .is_none_or(|at| at.iter().any(|p| p == slot))
                })
                .collect();
            if all.is_empty() {
                return Err(format!(
                    "{ty} at {slot}, where no entry of its type may stand"
                ));
            }
            let Some(admitted) = parent.and_then(|f| self.schema.admitted(f)) else {
                return Ok(all);
            };
            let narrowed: Vec<usize> = all
                .iter()
                .copied()
                .filter(|&i| {
                    self.schema.entries[i]
                        .interface
                        .as_ref()
                        .is_some_and(|name| admitted.contains(name))
                })
                .collect();
            if narrowed.is_empty() {
                let field = parent.map_or("?", |f| f.key.as_str());
                return Err(format!(
                    "{ty} where the parent's {field} admits {}",
                    parent.and_then(|f| f.ts.as_deref()).unwrap_or("?")
                ));
            }
            return Ok(narrowed);
        }
        let entry = parent.and_then(|f| f.entry.as_ref());
        match entry.and_then(|name| self.schema.typeless.get(name)) {
            Some(&index) => Ok(vec![index]),
            None => Err(format!(
                "a type-less object under {} with no `entry`",
                parent.map_or("?", |f| f.key.as_str())
            )),
        }
    }

    /// The findings of `node` against entry `index`, its own keys and values only.
    fn conform(
        &self,
        node: &Node,
        index: usize,
        enclosing: Option<(usize, usize)>,
        doc: Doc<'_>,
    ) -> Vec<(&'static str, String)> {
        let entry = &self.schema.entries[index];
        let Node::Object(pairs) = node else {
            return vec![("VALUE", format!("{}: not an object", entry.name))];
        };
        let mut out = Vec::new();
        let mut last = None;
        for (key, value) in pairs {
            let Some(at) = entry.fields.iter().position(|f| &f.key == key) else {
                out.push(("EXTRA", format!("{}.{key}", entry.name)));
                continue;
            };
            if last.is_some_and(|l| at <= l) {
                out.push((
                    "ORDER",
                    format!("{}.{key} out of the table's order", entry.name),
                ));
            }
            last = Some(at);
            let field = &entry.fields[at];
            if matches!(value, Node::Null) {
                if !field.nullable
                    && !field
                        .kinds
                        .iter()
                        .any(|k| matches!(k, Kind::Data | Kind::Null))
                {
                    out.push(("VALUE", format!("{}.{key}: null, not nullable", entry.name)));
                }
            } else if value_kinds(value, field).is_empty() {
                out.push((
                    "VALUE",
                    format!("{}.{key}: not {:?}", entry.name, field.kinds),
                ));
            }
            if let Node::Str(text) = value
                && let Some(class) = field.string.as_deref()
                && let Some(problem) =
                    string_class_problem(node, text, class, field, enclosing, doc.source)
            {
                out.push(("STRING", format!("{}.{key}: {problem}", entry.name)));
            }
            // a name's span (`name_loc`) always carries `character`; a node's (`loc`) only
            // where Svelte's own reader made the node
            if field.kinds.contains(&Kind::Loc)
                && field.entry.as_deref() == Some("SourceLocation")
                && !field.character
                && has_character(value)
            {
                out.push((
                    "VALUE",
                    format!(
                        "{}.{key}: `character` in a loc not marked `character`",
                        entry.name
                    ),
                ));
            }
        }
        for field in &entry.fields {
            let presence = presence_in(field, doc.vanilla);
            let present = pairs.iter().any(|(k, _)| k == &field.key);
            let wanted = match presence {
                Presence::Always => Some(true),
                Presence::Locations => Some(doc.locations),
                Presence::Absent => Some(false),
                Presence::Optional => None,
            };
            if wanted.is_some_and(|w| w != present) {
                out.push((
                    "PRESENCE",
                    format!(
                        "{}.{} {} ({presence:?}{})",
                        entry.name,
                        field.key,
                        if present { "present" } else { "absent" },
                        if doc.vanilla { ", vanilla acorn" } else { "" }
                    ),
                ));
            }
        }
        out
    }

    /// Grade `node` and everything under it. `slot` is the parent slot (`Type.key`), which also
    /// keys a `type`-less object's oracle-side label; `enclosing` the nearest positioned
    /// object's span around it, in UTF-16 units.
    fn grade(
        &mut self,
        node: &Node,
        css: bool,
        parent: Option<&Field>,
        slot: &str,
        enclosing: Option<(usize, usize)>,
        doc: Doc<'_>,
    ) {
        match node {
            Node::Array(items) => {
                for item in items {
                    self.grade(item, css, parent, slot, enclosing, doc);
                }
            }
            Node::Object(pairs) => {
                for (i, (key, _)) in pairs.iter().enumerate() {
                    if pairs[..i].iter().any(|(k, _)| k == key) {
                        self.findings.push(
                            "DUPLICATE",
                            format!("{key:?} in a {slot} object"),
                            doc.path,
                        );
                    }
                }
                let ty = node.str_field("type");
                let css = css || matches!(ty, Some("StyleSheetFile" | "StyleSheet"));
                let label = match ty {
                    Some(ty) if css => format!("css:{ty}"),
                    Some(ty) => ty.to_owned(),
                    None => format!("~{}", slot.trim_start_matches('~')),
                };
                let enclosing = span_of(node).or(enclosing);
                let candidates = match self.candidates(node, css, parent, slot) {
                    Ok(c) => c,
                    Err(what) => {
                        self.findings.push("UNDECLARED", what, doc.path);
                        return;
                    }
                };
                // Every candidate that conforms is exercised by the object; the one graded on is
                // the base entry where it conforms (a named variant's key set is a subset of its
                // base's, so an object both admit proves nothing about which it is), else the
                // narrowest conforming variant, else the candidate with the fewest findings.
                let schema = self.schema;
                let graded: Vec<(usize, Vec<(&'static str, String)>)> = candidates
                    .iter()
                    .map(|&i| (i, self.conform(node, i, enclosing, doc)))
                    .collect();
                let rank = |i: usize| (schema.entries[i].variant, schema.entries[i].fields.len());
                let chosen = graded
                    .iter()
                    .min_by_key(|(i, found)| (found.len(), rank(*i)))
                    .map(|(i, _)| *i);
                let Some(index) = chosen else {
                    return;
                };
                for (i, found) in &graded {
                    if *i == index {
                        for (class, message) in found {
                            self.findings.push(class, message.clone(), doc.path);
                        }
                    }
                    if *i == index || found.is_empty() {
                        self.record(*i, node, enclosing, doc);
                    }
                }
                self.label_entries
                    .entry(label.clone())
                    .or_default()
                    .insert(index);
                for (key, value) in pairs {
                    if is_child_bearing(key, value) {
                        self.child_keys
                            .entry(label.clone())
                            .or_default()
                            .insert(key.clone());
                    }
                }
                let entry = &schema.entries[index];
                for (key, value) in pairs {
                    let Some(field) = entry.fields.iter().find(|f| &f.key == key) else {
                        continue;
                    };
                    if field.kinds.contains(&Kind::Data) && field.kinds.len() == 1 {
                        continue;
                    }
                    // A Svelte `<style>`'s attributes are template nodes, not CSS.
                    let child_css = css && !(ty == Some("StyleSheet") && key == "attributes");
                    self.grade(
                        value,
                        child_css,
                        Some(field),
                        &format!("{label}.{key}"),
                        enclosing,
                        doc,
                    );
                }
            }
            Node::Str(_) | Node::Num(_) | Node::Bool | Node::Null => {}
        }
    }

    /// Tally what `node` exercises of entry `index`: the entry in this dialect, and per field its
    /// presence or absence, its value classes, flags, keyword values and built strings.
    fn record(
        &mut self,
        index: usize,
        node: &Node,
        enclosing: Option<(usize, usize)>,
        doc: Doc<'_>,
    ) {
        let Node::Object(pairs) = node else {
            return;
        };
        let entry = &self.schema.entries[index];
        let d = if entry.acorn { doc.dialect() } else { 0 };
        self.matched[index][d] += 1;
        for (field, use_) in entry.fields.iter().zip(self.uses[index].iter_mut()) {
            let Some((_, value)) = pairs.iter().find(|(k, _)| k == &field.key) else {
                use_.absent[d] = true;
                continue;
            };
            use_.present[d] = true;
            match value {
                Node::Null => use_.null = true,
                Node::Array(items) if items.iter().any(|i| matches!(i, Node::Null)) => {
                    use_.hole = true;
                }
                Node::Str(text) => {
                    if field.values.is_some() {
                        use_.values.insert(text.clone());
                    }
                    if field.string.as_deref() == Some("pooled")
                        && !within(text, enclosing, doc.source)
                    {
                        use_.built = true;
                    }
                }
                _ => {}
            }
            use_.kinds.extend(value_kinds(value, field));
            if field.character && has_character(value) {
                use_.character = true;
            }
        }
    }
}

/// A node's own `[start, end)`, when it carries numeric offsets.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a wire offset"
)]
fn span_of(node: &Node) -> Option<(usize, usize)> {
    match (node.field("start"), node.field("end")) {
        (Some(Node::Num(start)), Some(Node::Num(end))) => Some((*start as usize, *end as usize)),
        _ => None,
    }
}

/// Whether a `loc` value's positions carry `character`.
fn has_character(value: &Node) -> bool {
    ["start", "end"].iter().any(|end| {
        value
            .field(end)
            .and_then(|p| p.field("character"))
            .is_some()
    })
}

/// Whether `text` is a range of `source` within `enclosing` (the whole source where nothing
/// encloses it).
fn within(text: &str, enclosing: Option<(usize, usize)>, source: &[u16]) -> bool {
    let units: Vec<u16> = text.encode_utf16().collect();
    let region = match enclosing {
        Some((start, end)) => source.get(start..end).unwrap_or(&[]),
        None => source,
    };
    units.is_empty() || region.windows(units.len()).any(|w| w == units.as_slice())
}

/// Why `text`, a string field of `node`, breaks its class's promise, where a position can tell:
/// `own` is the node's own span of the source; `slice` a range of the source within the
/// nearest positioned object around it (`enclosing`); `keyword` one of the field's `values`.
/// `cooked` (a slice unless decoding changed it) and `pooled` (text the writer builds) make no
/// per-value promise; `pooled` is held to having been seen built at all (`FieldUse::built`).
fn string_class_problem(
    node: &Node,
    text: &str,
    class: &str,
    field: &Field,
    enclosing: Option<(usize, usize)>,
    source: &[u16],
) -> Option<String> {
    match class {
        "own" => {
            let Some((start, end)) = span_of(node) else {
                return Some("class `own` on a node with no span".to_owned());
            };
            let units: Vec<u16> = text.encode_utf16().collect();
            (source.get(start..end) != Some(units.as_slice()))
                .then(|| "class `own`, not the node's own span".to_owned())
        }
        "slice" => (!within(text, enclosing, source)).then(|| {
            "class `slice`, not a range of the source within its enclosing node".to_owned()
        }),
        "keyword" => match &field.values {
            Some(values) if values.iter().any(|v| v == text) => None,
            Some(_) => Some(format!("keyword {text:?} outside its `values`")),
            None => Some("class `keyword` with no `values`".to_owned()),
        },
        _ => None,
    }
}

/// Whether a Svelte document's wire is vanilla acorn's: its first top-level script (in source
/// order) carrying a `lang` decides, and only `lang="ts"` is TypeScript, its RAW text (an entity
/// spelling decodes to `ts` and still is not) — the writer's rule (`tsv_svelte`'s
/// `component_is_typescript`), read back from the wire.
fn svelte_is_vanilla(root: &Node) -> bool {
    let mut scripts: Vec<&Node> = ["module", "instance"]
        .iter()
        .filter_map(|key| root.field(key))
        .collect();
    scripts.sort_by(|a, b| {
        let start = |n: &Node| match n.field("start") {
            Some(Node::Num(v)) => *v,
            _ => f64::INFINITY,
        };
        start(a).total_cmp(&start(b))
    });
    for script in scripts {
        let Some(Node::Array(attributes)) = script.field("attributes") else {
            continue;
        };
        let lang = attributes
            .iter()
            .find(|a| a.str_field("name") == Some("lang"));
        if let Some(lang) = lang {
            let ts = matches!(lang.field("value"), Some(Node::Array(parts))
                if parts.len() == 1 && parts[0].str_field("raw") == Some("ts"));
            return !ts;
        }
    }
    true
}

//
// The oracle's child-bearing order.
//

/// What the oracle files show, per label: every distinct key sequence (with a file it came
/// from) and the keys seen child-bearing.
#[derive(Default)]
struct Oracle {
    sequences: BTreeMap<String, HashMap<Vec<String>, String>>,
    child_keys: BTreeMap<String, HashSet<String>>,
    duplicates: Vec<String>,
}

/// Whether `node` is a positioned object: numeric `start` and `end`.
fn is_positioned(node: &Node) -> bool {
    matches!(node.field("start"), Some(Node::Num(_)))
        && matches!(node.field("end"), Some(Node::Num(_)))
}

/// Whether `value` makes its key child-bearing: a positioned object, or an array holding one.
/// The attached-comment lists are child-bearing by name (`Oracle::walk`): a `<script>`
/// `Program`'s preceding-HTML entry is a positionless `{type, value}`.
fn is_child_bearing(key: &str, value: &Node) -> bool {
    key == "leadingComments"
        || key == "trailingComments"
        || match value {
            Node::Array(items) => items.iter().any(is_positioned),
            _ => is_positioned(value),
        }
}

impl Oracle {
    fn walk(&mut self, node: &Node, css: bool, slot: &str, file: &str) {
        match node {
            Node::Array(items) => {
                for item in items {
                    self.walk(item, css, slot, file);
                }
            }
            Node::Object(pairs) => {
                let ty = node.str_field("type");
                let css = css || matches!(ty, Some("StyleSheetFile" | "StyleSheet"));
                let label = match ty {
                    Some(ty) if css => format!("css:{ty}"),
                    Some(ty) => ty.to_owned(),
                    None => format!("~{}", slot.trim_start_matches('~')),
                };
                let keys: Vec<String> = pairs.iter().map(|(k, _)| k.clone()).collect();
                for (i, key) in keys.iter().enumerate() {
                    if keys[..i].contains(key) {
                        self.duplicates
                            .push(format!("{key:?} in a {label} object ({file})"));
                    }
                }
                for (key, value) in pairs {
                    if is_child_bearing(key, value) {
                        self.child_keys
                            .entry(label.clone())
                            .or_default()
                            .insert(key.clone());
                    }
                }
                self.sequences
                    .entry(label.clone())
                    .or_default()
                    .entry(keys)
                    .or_insert_with(|| file.to_owned());
                for (key, value) in pairs {
                    let child_css = css && !(ty == Some("StyleSheet") && key == "attributes");
                    self.walk(value, child_css, &format!("{label}.{key}"), file);
                }
            }
            Node::Str(_) | Node::Num(_) | Node::Bool | Node::Null => {}
        }
    }
}

/// The one total order every sequence embeds in, or a cycle among the keys.
fn total_order<'a>(
    sequences: impl IntoIterator<Item = &'a [String]>,
) -> Result<Vec<String>, Vec<String>> {
    // Keys numbered by first appearance across the sequences in a fixed order, so the derived
    // order is deterministic.
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

impl SchemaAuditCommand {
    /// Run the command.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Failed`] when the table does not read, the tree can't be walked, a
    /// document or an oracle file can't be read, a wire can't be read back, any finding the
    /// module doc names is made, or nothing was graded.
    pub fn run(self) -> Result<(), CliError> {
        let fail = |message: String| {
            eprintln!("schema_audit: {message}");
            CliError::Failed
        };
        let bytes = std::fs::read(&self.table)
            .map_err(|e| fail(format!("{}: {e}", self.table.display())))?;
        let schema =
            Schema::read(&bytes).map_err(|e| fail(format!("{}: {e}", self.table.display())))?;
        let fixtures = fixtures::walk_fixtures(&self.root).map_err(fail)?;
        let mut grader = Grader {
            schema: &schema,
            matched: vec![[0; 2]; schema.entries.len()],
            uses: schema
                .entries
                .iter()
                .map(|e| vec![FieldUse::default(); e.fields.len()])
                .collect(),
            findings: Findings::default(),
            label_entries: BTreeMap::new(),
            child_keys: BTreeMap::new(),
        };
        let mut oracle = Oracle::default();
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
                let read = std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        crate::json::from_slice::<Node>(&bytes).map_err(|e| e.to_string())
                    });
                match read {
                    Ok(node) => {
                        // The canonical parser's rejection marker holds no tree.
                        if !matches!(&node, Node::Object(entries) if entries.len() == 1 && entries[0].0 == "error")
                        {
                            oracle.walk(&node, css, "root", &relative);
                        }
                        oracle_files += 1;
                    }
                    Err(e) => {
                        eprintln!("{relative}: {e}");
                        failed = true;
                    }
                }
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
                let wires = match &parsed {
                    ParsedInput::Svelte(ast) => [
                        tsv_svelte::__private::convert_ast_json_bytes(ast, &source),
                        tsv_svelte::__private::convert_ast_json_bytes_with_locations(ast, &source),
                    ],
                    ParsedInput::Ts(ast) => [
                        tsv_ts::__private::convert_ast_json_bytes(ast, &source),
                        tsv_ts::__private::convert_ast_json_bytes_with_locations(ast, &source),
                    ],
                    ParsedInput::Css(ast) => [
                        tsv_css::__private::convert_ast_json_bytes(ast, &source),
                        tsv_css::__private::convert_ast_json_bytes_with_locations(ast, &source),
                    ],
                };
                // The wire's offsets index the source as each canonical parser reads it: Svelte's
                // `parse` and `parseCss` strip a leading byte-order mark first, acorn counts it
                // (`tsv_lang::LeadingBom`).
                let elided = !matches!(parsed, ParsedInput::Ts(_));
                let read = if elided {
                    source.strip_prefix('\u{feff}').unwrap_or(&source)
                } else {
                    &source
                };
                let units: Vec<u16> = read.encode_utf16().collect();
                for (locations, wire) in [false, true].into_iter().zip(&wires) {
                    let node = match crate::json::from_slice::<Node>(wire) {
                        Ok(node) => node,
                        Err(e) => {
                            eprintln!("{relative}: the wire does not read back: {e}");
                            failed = true;
                            continue;
                        }
                    };
                    let vanilla =
                        matches!(parsed, ParsedInput::Svelte(_)) && svelte_is_vanilla(&node);
                    let doc = Doc {
                        path: &relative,
                        source: &units,
                        vanilla,
                        locations,
                    };
                    grader.grade(&node, css, None, "root", None, doc);
                }
                graded += 1;
            }
        }
        let Grader {
            matched,
            uses,
            mut findings,
            label_entries,
            child_keys,
            ..
        } = grader;

        // The table: every entry and every flag exercised; one order per type; every name resolves.
        for (index, entry) in schema.entries.iter().enumerate() {
            if entry.mixin {
                continue;
            }
            let seen = matched[index];
            if seen == [0, 0] {
                findings.push(
                    "UNEXERCISED",
                    format!("{}: no object resolves to it", entry.name),
                    "",
                );
                continue;
            }
            for (field, use_) in entry.fields.iter().zip(&uses[index]) {
                let name = format!("{}.{}", entry.name, field.key);
                // The attached-comment lists are a rule, not a shape: the attachment walk reaches
                // essentially every acorn node, so an entry carries them whether or not a fixture
                // puts a comment there.
                if field.kinds == [Kind::Comments] {
                    continue;
                }
                if use_.present == [false, false] {
                    findings.push("UNEXERCISED", format!("{name}: never present"), "");
                    continue;
                }
                // Presence, from the side the per-object check cannot see. In each dialect an
                // object of the entry was met in, a field `optional` there was seen absent there,
                // and present there too — wherever the table states the dialect's presence apart
                // (`vanilla_acorn`), or the other dialect did show it: a field one dialect emits
                // and the other never does is a dialect fact the table must state.
                for d in 0..2 {
                    if seen[d] == 0 || presence_in(field, d == 1) != Presence::Optional {
                        continue;
                    }
                    let dialect = DIALECTS[d];
                    if !use_.absent[d] {
                        findings.push(
                            "UNEXERCISED",
                            format!("{name}: optional{dialect}, never absent"),
                            "",
                        );
                    }
                    let split = seen[1 - d] > 0 && use_.present[1 - d];
                    if (field.vanilla_acorn.is_some() || split) && !use_.present[d] {
                        findings.push(
                            "UNEXERCISED",
                            format!("{name}: optional{dialect}, never present"),
                            "",
                        );
                    }
                }
                if field.nullable && !use_.null {
                    findings.push("UNEXERCISED", format!("{name}: nullable, never null"), "");
                }
                if field.items_nullable && !use_.hole {
                    findings.push(
                        "UNEXERCISED",
                        format!("{name}: items_nullable, never a hole"),
                        "",
                    );
                }
                if field.character && !use_.character {
                    findings.push(
                        "UNEXERCISED",
                        format!("{name}: character, never carried"),
                        "",
                    );
                }
                for kind in &field.kinds {
                    if !use_.kinds.contains(kind) {
                        findings.push(
                            "UNEXERCISED",
                            format!("{name}: kind {kind:?} never seen"),
                            "",
                        );
                    }
                }
                for value in field.values.iter().flatten() {
                    if !use_.values.contains(value) {
                        findings.push(
                            "UNEXERCISED",
                            format!("{name}: keyword {value:?} never seen"),
                            "",
                        );
                    }
                }
                if field.string.as_deref() == Some("pooled") && !use_.built {
                    findings.push(
                        "UNEXERCISED",
                        format!("{name}: pooled, never seen built (always a range of its node)"),
                        "",
                    );
                }
            }
        }
        for (key, indices) in &schema.by_type {
            let sequences: Vec<Vec<String>> = indices
                .iter()
                .map(|&i| {
                    schema.entries[i]
                        .fields
                        .iter()
                        .map(|f| f.key.clone())
                        .collect()
                })
                .collect();
            if let Err(cycle) = total_order(sequences.iter().map(Vec::as_slice)) {
                findings.push(
                    "TABLE",
                    format!(
                        "the entries of {} take more than one key order, cycle {}",
                        key.1,
                        cycle.join(" < ")
                    ),
                    "",
                );
            }
        }
        for name in schema.unresolved_names() {
            findings.push(
                "TABLE",
                format!("a `ts` names {name}, which the table does not declare"),
                "",
            );
        }

        // The child-bearing keys' relative order, the table's against the oracle's.
        let mut child_conflicts = 0usize;
        let mut oracle_multi = 0usize;
        for (label, indices) in &label_entries {
            let order: Vec<String> = {
                let sequences: Vec<Vec<String>> = indices
                    .iter()
                    .map(|&i| {
                        schema.entries[i]
                            .fields
                            .iter()
                            .map(|f| f.key.clone())
                            .collect()
                    })
                    .collect();
                match total_order(sequences.iter().map(Vec::as_slice)) {
                    Ok(order) => order,
                    Err(_) => continue, // reported as TABLE above when the entries share a type
                }
            };
            let fields = || indices.iter().flat_map(|&i| &schema.entries[i].fields);
            let ts_only: HashSet<&str> = fields()
                .filter(|f| f.ts_only)
                .map(|f| f.key.as_str())
                .collect();
            let keys: HashSet<&str> = child_keys
                .get(label)
                .into_iter()
                .chain(oracle.child_keys.get(label))
                .flatten()
                .map(String::as_str)
                .filter(|key| !ts_only.contains(key))
                .collect();
            if keys.len() < 2 {
                continue;
            }
            let cut = |sequence: &[String]| -> Vec<String> {
                sequence
                    .iter()
                    .filter(|k| keys.contains(k.as_str()))
                    .cloned()
                    .collect()
            };
            let theirs: Vec<(Vec<String>, &str)> = oracle
                .sequences
                .get(label)
                .map(|s| {
                    s.iter()
                        .map(|(seq, file)| (cut(seq), file.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            if let Err(cycle) = total_order(theirs.iter().map(|(k, _)| k.as_slice())) {
                oracle_multi += 1;
                println!(
                    "ORACLE {label}: the canonical parser's own child-bearing keys take more than one \
                     order, cycle {} — an exemption must be named deliberately; none is assumed",
                    cycle.join(" < ")
                );
                print_sequences(theirs.iter().map(|(k, f)| (k.as_slice(), *f)));
                continue;
            }
            let table = cut(&order);
            let both = theirs
                .iter()
                .map(|(k, _)| k.as_slice())
                .chain(std::iter::once(table.as_slice()));
            if let Err(cycle) = total_order(both) {
                child_conflicts += 1;
                println!(
                    "CHILD-ORDER {label}: the table's child-bearing order disagrees with the oracle's, cycle {}",
                    cycle.join(" < ")
                );
                println!("  table: {}", table.join(","));
                println!("  oracle:");
                print_sequences(theirs.iter().map(|(k, f)| (k.as_slice(), *f)));
            }
        }
        for duplicate in &oracle.duplicates {
            findings.push("DUPLICATE", format!("oracle: {duplicate}"), "");
        }

        findings.print();
        let total = findings.total();
        println!(
            "schema_audit: {graded} documents graded on both wires ({rejected} tsv rejects skipped), \
             {oracle_files} oracle files read, {} table entries; {total} findings, {child_conflicts} \
             child-bearing orders against the oracle's, {oracle_multi} oracle types with more than \
             one child-bearing order",
            schema.entries.len(),
        );
        if graded == 0 || oracle_files == 0 {
            eprintln!("schema_audit: nothing graded — refusing to pass");
            return Err(CliError::Failed);
        }
        if failed || total > 0 || child_conflicts > 0 || oracle_multi > 0 {
            return Err(CliError::Failed);
        }
        Ok(())
    }
}
