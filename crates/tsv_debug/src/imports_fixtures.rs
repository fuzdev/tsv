//! Import-record fixture discovery, file conventions, and the oracle.
//!
//! The import fixtures live in their own tree, `tests/fixtures_imports/`, since their claim
//! is the records `tsv_ts::parse_imports` / `tsv_svelte::parse_imports` collect, not a parse
//! tree or a formatting fixed point. Each fixture directory holds:
//!
//! - `input.ts` or `input.svelte` — the source;
//! - `expected.json` — the records in the JSON a binding returns
//!   (`tsv_ts::__private::write_imports_json`), tab-indented. Always oracle-generated
//!   (`imports_fixtures_update`), never hand-written;
//! - `README.md` — what the fixture pins;
//! - `oracle.txt`, only when the canonical parser rejects the input (below).
//!
//! The same hierarchy rule as the other fixture trees applies: every directory is either a
//! fixture (it has an input) or a container (it has subdirectories and at most a
//! `README.md`), never both, never neither.
//!
//! ## The oracle
//!
//! [`imports_from_estree`] collects the records from an ESTree wire — the canonical
//! parser's (acorn-typescript for `input.ts`; Svelte's `parse` for `input.svelte`, whose
//! `instance.content` / `module.content` are the two script programs). It reads that tree
//! alone, never tsv's AST, so it grades tsv's collector independently. The rules it
//! implements are the record semantics both sides share — [`RULES`] states them, in one
//! place.
//!
//! A TypeScript input is read at Module goal, and at Script goal when that is a parse
//! error, as `tsv_ts::parse_imports` reads it. The generator also runs the same walk over
//! tsv's own parse wire at that goal and requires the identical records, so a fixture
//! pins only what both parsers' trees agree on — still without reading tsv's collector.
//!
//! The canonical parser has no import phases: acorn-typescript rejects `import source`,
//! `import defer`, `import.source()` and `import.defer()`. A fixture over one carries an
//! `oracle.txt` whose first line is `tsv-wire` and whose remaining lines say why; its
//! records come from the same walk over **tsv's own parse wire** (`parse_to_json`, graded
//! against the canonical parsers by the main fixture tree), so the provenance is visible in
//! the tree. The generator fails on a rejected input with no marker, and on a marker whose
//! input the canonical parser accepts.

use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// The import fixture tree root, relative to the repo root.
pub const IMPORTS_FIXTURES_DIR: &str = "tests/fixtures_imports";

/// The committed records' file name.
pub const EXPECTED_JSON: &str = "expected.json";

/// Every fixture's (and an optional container's) description.
pub const README: &str = "README.md";

/// The marker naming a fixture whose oracle reads tsv's parse wire.
pub const ORACLE_MARKER: &str = "oracle.txt";

/// The marker's first line.
pub const TSV_WIRE: &str = "tsv-wire";

/// The oracle's rules, in one place: what [`imports_from_estree`] implements, and what
/// tsv's collector must match. Printed by nothing; read by the maintainer of either side.
pub const RULES: &str = "\
records, in source order (sorted by start; for Svelte, both scripts together):
- ImportDeclaration -> static
- ExportNamedDeclaration with a `source` -> re_export; ExportAllDeclaration -> re_export
- ImportExpression -> dynamic (its options are `options` — vanilla acorn's field, which
  Svelte's plain `<script>` takes — or else acorn-typescript's `arguments[0]`)
- TSImportEqualsDeclaration whose moduleReference is a TSExternalModuleReference ->
  import_equals (the entity-name form `import x = A.B` is no record)
- inside a namespace body (any TSModuleDeclaration's, nested ones included), a static
  import, a re-export and an import-equals are no records: tsc rejects them there (TS1147,
  TS1194). A dynamic import there is a record
span: the node's start/end
specifier: a string Literal -> {value, start + 1, end - 1}; a TemplateLiteral with no
  expressions -> its one quasi's {cooked, start, end}; anything else (dynamic only) -> null
typeOnly: the declaration's importKind/exportKind `type` -> statement; else, when it has at
  least one specifier and every one is an ImportSpecifier/ExportSpecifier whose
  importKind/exportKind is `type` -> specifiers; else null. Dynamic: null
phase: the node's `phase`, or null
attributes: static/re-export -> the clause from its `with` keyword, found after the source
  literal past whitespace and comments and before the declaration's end, through its
  closing `}` (null when there is no keyword); an `assert` clause there fails the oracle
  (not attribute syntax for tsv, which rejects it); dynamic -> the options' start/end, or
  null
hasModuleSyntax: true iff
  - an ImportDeclaration, ExportNamedDeclaration, ExportDefaultDeclaration or
    ExportAllDeclaration is among a program's TOP-LEVEL statements (an export inside a
    namespace body does not make the file a module), or
  - a MetaProperty `import.meta` is anywhere walked, or
  - an AwaitExpression, a ForOfStatement with `await: true`, or a VariableDeclaration of
    kind `await using` (a `for` head's included), is outside every FunctionDeclaration /
    FunctionExpression / ArrowFunctionExpression (a method's value is a
    FunctionExpression), StaticBlock, field initializer (a PropertyDefinition's or
    AccessorProperty's `value`), TSModuleDeclaration and TSEnumDeclaration — tsc reads a
    namespace body and an enum member initializer as function-like for `await` (TS1308)
  A Svelte component answers for both scripts together.
types are not walked: a TSInterfaceDeclaration, TSTypeAliasDeclaration or TSIndexSignature
  (a class's index signature is a member in acorn's tree) node, and the
  value of any key that holds a type — typeAnnotation (an assertion's type included; its
  operand is `expression`), returnType, typeParameters, typeArguments,
  superTypeParameters, superTypeArguments, implements. A computed key inside a type is an
  ImportExpression in acorn's tree all the same, and no record. Any other TS* node the walk
  reaches that is not runtime code fails the oracle, naming the node.
ambient code is not walked: a node with `declare: true`; a TSModuleDeclaration that is
  `global` or string-named; a TSDeclareFunction / TSDeclareMethod (an overload, a bodiless
  or abstract method); an abstract class member (`abstract: true`, or a TSAbstract* node).
  Everything else is walked: non-declare namespace bodies, enum member initializers,
  `export =` operands, and the operands of the assertions the type strip refuses (an
  angle-bracket assertion, and an `as` / `satisfies` whose blanking would regroup the
  expression around it).
";

/// A fixture's source language, named by its input file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportsLanguage {
    /// `input.ts`: acorn-typescript, `tsv_ts::parse_imports`.
    TypeScript,
    /// `input.svelte`: Svelte's `parse`, `tsv_svelte::parse_imports`.
    Svelte,
}

impl ImportsLanguage {
    /// The input file name.
    pub const fn input_file(self) -> &'static str {
        match self {
            Self::TypeScript => "input.ts",
            Self::Svelte => "input.svelte",
        }
    }

    const ALL: [Self; 2] = [Self::TypeScript, Self::Svelte];
}

/// A discovered import fixture.
#[derive(Debug, Clone)]
pub struct ImportsFixture {
    /// Cwd-relative path to the fixture directory.
    pub path: PathBuf,
    /// Path relative to the fixture root (e.g. `static/default`).
    pub relative_path: String,
    /// The input's language.
    pub language: ImportsLanguage,
}

impl ImportsFixture {
    /// Whether this fixture matches any of the case-insensitive substring filters (empty
    /// filters match everything).
    pub fn matches_filters(&self, filters: &[String]) -> bool {
        crate::fixtures::path_matches_filters(&self.relative_path, filters)
    }

    /// The input file's path.
    pub fn input_path(&self) -> PathBuf {
        self.path.join(self.language.input_file())
    }

    /// The committed records' path.
    pub fn expected_path(&self) -> PathBuf {
        self.path.join(EXPECTED_JSON)
    }

    /// Read the input.
    ///
    /// # Errors
    ///
    /// When the input can't be read.
    pub fn read_input(&self) -> Result<String, String> {
        let path = self.input_path();
        fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))
    }

    /// The fixture's `oracle.txt` reason, `None` when it has no marker.
    ///
    /// # Errors
    ///
    /// When the marker can't be read, or its first line is not `tsv-wire`, or it gives no
    /// reason.
    pub fn tsv_wire_reason(&self) -> Result<Option<String>, String> {
        let path = self.path.join(ORACLE_MARKER);
        if !path.exists() {
            return Ok(None);
        }
        let text =
            fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let mut lines = text.lines();
        if lines.next() != Some(TSV_WIRE) {
            return Err(format!(
                "{}: the first line must be `{TSV_WIRE}`",
                path.display()
            ));
        }
        let reason = lines.collect::<Vec<_>>().join("\n").trim().to_owned();
        if reason.is_empty() {
            return Err(format!(
                "{}: say why the canonical parser cannot be the oracle",
                path.display()
            ));
        }
        Ok(Some(reason))
    }

    /// What is wrong with the fixture's file set: it must hold its input, `expected.json`
    /// and `README.md`, plus at most `oracle.txt`, and nothing else.
    pub fn structure_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for required in [EXPECTED_JSON, README] {
            if !self.path.join(required).is_file() {
                problems.push(format!("missing {required}"));
            }
        }
        match fs::read_dir(&self.path) {
            Ok(entries) => {
                let mut names: Vec<String> = entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                for name in names {
                    let known = name == self.language.input_file()
                        || name == EXPECTED_JSON
                        || name == README
                        || name == ORACLE_MARKER;
                    if !known {
                        problems.push(format!("unexpected file {name}"));
                    }
                }
            }
            Err(e) => problems.push(format!("reading the directory: {e}")),
        }
        problems
    }
}

/// Walk the fixture tree, enforcing the fixture-or-container rule. Returns the fixtures
/// sorted by relative path.
///
/// # Errors
///
/// When a directory is both a fixture and a container, or neither, holds two inputs, holds
/// a file a container may not, or can't be read.
pub fn walk_imports_fixtures(root: &Path) -> Result<Vec<ImportsFixture>, String> {
    let mut fixtures = Vec::new();
    walk_recursive(root, "", &mut fixtures)?;
    fixtures.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(fixtures)
}

fn walk_recursive(
    dir: &Path,
    relative: &str,
    fixtures: &mut Vec<ImportsFixture>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("reading {}: {e}", dir.display()))?;
    let mut subdirs = Vec::new();
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("reading {}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() {
            subdirs.push(name);
        } else {
            files.push(name);
        }
    }
    subdirs.sort();
    let inputs: Vec<ImportsLanguage> = ImportsLanguage::ALL
        .into_iter()
        .filter(|language| files.iter().any(|f| f == language.input_file()))
        .collect();
    let shown = if relative.is_empty() { "." } else { relative };
    match (inputs.as_slice(), subdirs.is_empty()) {
        ([language], true) => {
            fixtures.push(ImportsFixture {
                path: dir.to_path_buf(),
                relative_path: relative.to_owned(),
                language: *language,
            });
            Ok(())
        }
        ([], false) => {
            if let Some(file) = files.iter().find(|f| *f != README) {
                return Err(format!(
                    "{shown}: a container holds no file but README.md, found {file}"
                ));
            }
            for name in subdirs {
                let child = if relative.is_empty() {
                    name.clone()
                } else {
                    format!("{relative}/{name}")
                };
                walk_recursive(&dir.join(&name), &child, fixtures)?;
            }
            Ok(())
        }
        ([], true) => Err(format!(
            "{shown}: neither a fixture (no input) nor a container"
        )),
        ([_], false) => Err(format!("{shown}: a fixture with subdirectories")),
        _ => Err(format!("{shown}: more than one input")),
    }
}

/// tsv's records for `source` as the JSON `expected.json` pins.
///
/// A TypeScript input takes the unset goal (Module, retried as Script), as
/// `tsv_ts::parse_imports` reads it.
///
/// # Errors
///
/// tsv's parse error, displayed.
pub fn ours_json(language: ImportsLanguage, source: &str) -> Result<Value, String> {
    let bytes = match language {
        ImportsLanguage::TypeScript => {
            let imports = tsv_ts::parse_imports(source).map_err(|e| format!("{e:#}"))?;
            tsv_ts::__private::write_imports_json(&imports, source, tsv_ts::WIRE_COORDINATES)
        }
        ImportsLanguage::Svelte => {
            let imports = tsv_svelte::parse_imports(source).map_err(|e| format!("{e:#}"))?;
            tsv_ts::__private::write_imports_json(&imports, source, tsv_svelte::WIRE_COORDINATES)
        }
    };
    crate::json::from_slice(&bytes).map_err(|e| format!("the imports JSON does not parse: {e}"))
}

/// tsv's own parse wire for `source` at `goal` (a Svelte component's scripts are modules
/// whatever `goal` says) — the oracle's input for a `tsv-wire` fixture, and the
/// cross-check's for every other.
///
/// # Errors
///
/// tsv's parse error, displayed.
pub fn tsv_wire(
    language: ImportsLanguage,
    source: &str,
    goal: tsv_ts::Goal,
) -> Result<Value, String> {
    let wire = match language {
        ImportsLanguage::TypeScript => {
            tsv_ts::parse_to_json_with(source, tsv_ts::JsonOptions::new().goal(goal))
        }
        ImportsLanguage::Svelte => tsv_svelte::parse_to_json(source),
    }
    .map_err(|e| format!("{e:#}"))?;
    Ok(crate::json::wire_value(wire.as_bytes()))
}

/// The oracle over a whole-document ESTree `wire` of `source` in `language`: the script
/// programs it holds, walked by [`imports_from_estree`] over the source's UTF-16 units in
/// the wire's own coordinates (acorn counts a leading BOM; Svelte's `parse` strips it).
///
/// # Errors
///
/// As [`imports_from_estree`].
pub fn oracle_json(language: ImportsLanguage, source: &str, wire: &Value) -> Result<Value, String> {
    let (programs, text) = match language {
        ImportsLanguage::TypeScript => (vec![wire], source),
        ImportsLanguage::Svelte => {
            let programs = ["instance", "module"]
                .into_iter()
                .filter_map(|key| wire.get(key).and_then(|script| script.get("content")))
                .filter(|content| !content.is_null())
                .collect();
            (programs, source.strip_prefix('\u{FEFF}').unwrap_or(source))
        }
    };
    let units: Vec<u16> = text.encode_utf16().collect();
    imports_from_estree(&programs, &units)
}

/// The records of ESTree `programs` over `units`, the UTF-16 text their offsets index —
/// [`RULES`].
///
/// # Errors
///
/// A tree that breaks an assumption the rules rest on (a field missing or mistyped, an
/// attributes clause the scan can't find, an `assert` clause, a type node the walk reached
/// through a key it does not know holds a type): an oracle that fails loudly rather than
/// guess.
pub fn imports_from_estree(programs: &[&Value], units: &[u16]) -> Result<Value, String> {
    let mut walk = Walk {
        units,
        records: Vec::new(),
        has_module_syntax: false,
    };
    for program in programs {
        let body = program
            .get("body")
            .and_then(Value::as_array)
            .ok_or("a program with no `body` array")?;
        walk.has_module_syntax |= body.iter().any(|statement| {
            matches!(
                node_type(statement),
                Some(
                    "ImportDeclaration"
                        | "ExportNamedDeclaration"
                        | "ExportDefaultDeclaration"
                        | "ExportAllDeclaration"
                )
            )
        });
        walk.node(program, Context::default())?;
    }
    let Walk {
        mut records,
        has_module_syntax,
        ..
    } = walk;
    records.sort_by_key(|(start, _)| *start);
    let imports: Vec<Value> = records.into_iter().map(|(_, record)| record).collect();
    Ok(json!({ "hasModuleSyntax": has_module_syntax, "imports": imports }))
}

fn node_type(node: &Value) -> Option<&str> {
    node.get("type").and_then(Value::as_str)
}

/// The keys whose value is a type — never walked ([`RULES`]).
const TYPE_KEYS: &[&str] = &[
    "typeAnnotation",
    "returnType",
    "typeParameters",
    "typeArguments",
    "superTypeParameters",
    "superTypeArguments",
    "implements",
];

/// The node types that are types whole — never walked ([`RULES`]).
const TYPE_NODES: &[&str] = &[
    "TSInterfaceDeclaration",
    "TSTypeAliasDeclaration",
    "TSIndexSignature",
];

/// The TypeScript node types that hold (or are) runtime code, which the walk enters. Any
/// other `TS*` node it reaches is a type it should have skipped, and fails the walk: a
/// key holding a type that [`TYPE_KEYS`] misses.
const VALUE_TS_NODES: &[&str] = &[
    "TSAsExpression",
    "TSSatisfiesExpression",
    "TSTypeAssertion",
    "TSNonNullExpression",
    "TSInstantiationExpression",
    "TSParameterProperty",
    "TSEnumDeclaration",
    "TSEnumBody",
    "TSEnumMember",
    "TSModuleDeclaration",
    "TSModuleBlock",
    "TSExportAssignment",
    "TSImportEqualsDeclaration",
    "TSExternalModuleReference",
    "TSQualifiedName",
    "TSNamespaceExportDeclaration",
];

/// Where a node stands, for the rules that depend on it.
#[derive(Clone, Copy, Default)]
struct Context {
    /// Inside a function, a class static block, a field initializer, a namespace body or
    /// an enum: an `await` there is not a top-level one.
    in_function: bool,
    /// Inside a namespace body, where a static import, a re-export and an
    /// `import x = require('y')` are no records.
    in_namespace: bool,
}

struct Walk<'u> {
    units: &'u [u16],
    records: Vec<(u64, Value)>,
    has_module_syntax: bool,
}

impl Walk<'_> {
    fn node(&mut self, node: &Value, context: Context) -> Result<(), String> {
        match node {
            Value::Array(items) => {
                for item in items {
                    self.node(item, context)?;
                }
            }
            Value::Object(map) => self.object(map, context)?,
            _ => {}
        }
        Ok(())
    }

    fn object(&mut self, map: &Map<String, Value>, context: Context) -> Result<(), String> {
        if is_ambient(map) {
            return Ok(());
        }
        let kind = map.get("type").and_then(Value::as_str).unwrap_or_default();
        if TYPE_NODES.contains(&kind) {
            return Ok(());
        }
        if kind.starts_with("TS") && !VALUE_TS_NODES.contains(&kind) {
            return Err(format!(
                "the walk reached the type node `{kind}`: a key holding a type is missing \
                 from TYPE_KEYS"
            ));
        }
        self.has_module_syntax |= is_module_syntax_site(map, kind, context);
        if !(context.in_namespace && kind != "ImportExpression")
            && let Some(record) = record_of(map, self.units)?
        {
            self.records.push((offset(map, "start")?, record));
        }
        let mut inner = context;
        match kind {
            "FunctionDeclaration"
            | "FunctionExpression"
            | "ArrowFunctionExpression"
            | "StaticBlock"
            | "TSEnumDeclaration" => inner.in_function = true,
            "TSModuleDeclaration" => {
                inner.in_namespace = true;
                inner.in_function = true;
            }
            _ => {}
        }
        let field = matches!(kind, "PropertyDefinition" | "AccessorProperty");
        for (key, value) in map {
            if TYPE_KEYS.contains(&key.as_str()) {
                continue;
            }
            let mut child = inner;
            if field && key == "value" {
                child.in_function = true;
            }
            self.node(value, child)?;
        }
        Ok(())
    }
}

/// The `hasModuleSyntax` rule of [`RULES`] past the top-level declarations: an
/// `import.meta`, or an `await` / `for await` / `await using` outside every function.
fn is_module_syntax_site(map: &Map<String, Value>, kind: &str, context: Context) -> bool {
    let name = |key: &str| {
        map.get(key)
            .and_then(|node| node.get("name"))
            .and_then(Value::as_str)
    };
    match kind {
        "MetaProperty" => name("meta") == Some("import") && name("property") == Some("meta"),
        "AwaitExpression" => !context.in_function,
        "ForOfStatement" => !context.in_function && map.get("await") == Some(&Value::Bool(true)),
        "VariableDeclaration" => {
            !context.in_function && map.get("kind").and_then(Value::as_str) == Some("await using")
        }
        _ => false,
    }
}

/// The ambient-code rule of [`RULES`].
fn is_ambient(map: &Map<String, Value>) -> bool {
    let flag = |key: &str| map.get(key) == Some(&Value::Bool(true));
    if flag("declare") {
        return true;
    }
    match map.get("type").and_then(Value::as_str) {
        Some("TSDeclareFunction" | "TSDeclareMethod") => true,
        Some("TSModuleDeclaration") => {
            flag("global") || map.get("id").and_then(node_type) == Some("Literal")
        }
        Some(kind) if kind.starts_with("TSAbstract") => true,
        Some("MethodDefinition" | "PropertyDefinition" | "AccessorProperty") => flag("abstract"),
        _ => false,
    }
}

fn offset(map: &Map<String, Value>, key: &str) -> Result<u64, String> {
    map.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("a node with no numeric `{key}`"))
}

fn span_of(node: &Value) -> Result<(u64, u64), String> {
    let map = node.as_object().ok_or("a span read off a non-object")?;
    Ok((offset(map, "start")?, offset(map, "end")?))
}

/// The record a node is, if it is one.
fn record_of(map: &Map<String, Value>, units: &[u16]) -> Result<Option<Value>, String> {
    let kind = match map.get("type").and_then(Value::as_str) {
        Some("ImportDeclaration") => "static",
        Some("ExportNamedDeclaration") if map.get("source").is_some_and(|s| !s.is_null()) => {
            "re_export"
        }
        Some("ExportAllDeclaration") => "re_export",
        Some("ImportExpression") => "dynamic",
        Some("TSImportEqualsDeclaration")
            if map.get("moduleReference").and_then(node_type)
                == Some("TSExternalModuleReference") =>
        {
            "import_equals"
        }
        _ => return Ok(None),
    };
    let source = match kind {
        "import_equals" => map
            .get("moduleReference")
            .and_then(|reference| reference.get("expression")),
        _ => map.get("source"),
    }
    .ok_or("a record with no source")?;
    let specifier = specifier_of(source)?;
    let (type_only, phase, attributes) = match kind {
        "dynamic" => {
            let options = map.get("options").filter(|options| !options.is_null());
            let argument = || {
                map.get("arguments")
                    .and_then(Value::as_array)
                    .and_then(|arguments| arguments.first())
            };
            let attributes = match options.or_else(argument) {
                Some(options) => {
                    let (start, end) = span_of(options)?;
                    json!({ "start": start, "end": end })
                }
                None => Value::Null,
            };
            (Value::Null, phase_of(map), attributes)
        }
        _ => {
            let kind_key = if map.get("type").and_then(Value::as_str) == Some("ImportDeclaration")
                || kind == "import_equals"
            {
                "importKind"
            } else {
                "exportKind"
            };
            let attributes = if kind == "import_equals" {
                Value::Null
            } else {
                attributes_clause(map, source, units)?
            };
            (type_only_of(map, kind_key), phase_of(map), attributes)
        }
    };
    let (start, end) = (offset(map, "start")?, offset(map, "end")?);
    Ok(Some(json!({
        "kind": kind,
        "start": start,
        "end": end,
        "specifier": specifier,
        "typeOnly": type_only,
        "phase": phase,
        "attributes": attributes,
    })))
}

fn specifier_of(source: &Value) -> Result<Value, String> {
    match node_type(source) {
        Some("Literal") => match source.get("value") {
            Some(Value::String(value)) => {
                let (start, end) = span_of(source)?;
                Ok(json!({ "value": value, "start": start + 1, "end": end - 1 }))
            }
            _ => Ok(Value::Null),
        },
        Some("TemplateLiteral") => {
            let no_substitutions = source
                .get("expressions")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty);
            if !no_substitutions {
                return Ok(Value::Null);
            }
            let quasi = source
                .get("quasis")
                .and_then(Value::as_array)
                .and_then(|quasis| quasis.first())
                .ok_or("a template literal with no quasi")?;
            let cooked = quasi
                .get("value")
                .and_then(|value| value.get("cooked"))
                .cloned()
                .unwrap_or(Value::Null);
            let (start, end) = span_of(quasi)?;
            Ok(json!({ "value": cooked, "start": start, "end": end }))
        }
        _ => Ok(Value::Null),
    }
}

fn type_only_of(map: &Map<String, Value>, kind_key: &str) -> Value {
    let is_type = |node: &Value, key: &str| node.get(key).and_then(Value::as_str) == Some("type");
    if map.get(kind_key).and_then(Value::as_str) == Some("type") {
        return json!("statement");
    }
    let specifiers = map
        .get("specifiers")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let every_inline_type = !specifiers.is_empty()
        && specifiers
            .iter()
            .all(|specifier| match node_type(specifier) {
                Some("ImportSpecifier") => is_type(specifier, "importKind"),
                Some("ExportSpecifier") => is_type(specifier, "exportKind"),
                _ => false,
            });
    if every_inline_type {
        json!("specifiers")
    } else {
        Value::Null
    }
}

fn phase_of(map: &Map<String, Value>) -> Value {
    match map.get("phase") {
        Some(Value::String(phase)) => json!(phase),
        _ => Value::Null,
    }
}

/// A static import's or a re-export's attributes clause: from its `with` keyword, found
/// after the source literal past trivia, through its `}`. An `assert` there fails the walk
/// ([`RULES`]).
fn attributes_clause(
    map: &Map<String, Value>,
    source: &Value,
    units: &[u16],
) -> Result<Value, String> {
    let (_, source_end) = span_of(source)?;
    // the clause is inside the declaration: what follows its end is the next statement
    // (`import 'x'⏎assert(…)` is a call), never read
    let units = &units[..to_index(offset(map, "end")?).min(units.len())];
    let at = skip_trivia(units, to_index(source_end));
    let attributes = map.get("attributes").and_then(Value::as_array);
    if starts_with_word(units, at, "assert") {
        return Err(
            "an `assert` clause: not import-attribute syntax for tsv (deprecated, dropped for \
             `with`)"
                .to_owned(),
        );
    }
    let keyword = "with";
    let (true, Some(attributes)) = (starts_with_word(units, at, keyword), attributes) else {
        if attributes.is_some_and(|attributes| !attributes.is_empty()) {
            return Err("import attributes with no `with` keyword".to_owned());
        }
        return Ok(Value::Null);
    };
    let after_keyword = at + keyword.len();
    let close = match attributes.last() {
        Some(last) => {
            let (_, last_end) = span_of(last)?;
            let mut i = skip_trivia(units, to_index(last_end));
            if units.get(i) == Some(&u16::from(b',')) {
                i = skip_trivia(units, i + 1);
            }
            i
        }
        None => {
            let open = skip_trivia(units, after_keyword);
            if units.get(open) != Some(&u16::from(b'{')) {
                return Err("an attributes clause with no `{`".to_owned());
            }
            skip_trivia(units, open + 1)
        }
    };
    if units.get(close) != Some(&u16::from(b'}')) {
        return Err("an attributes clause with no `}`".to_owned());
    }
    Ok(json!({ "start": at, "end": close + 1 }))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a fixture is far below 4 GiB"
)]
fn to_index(offset: u64) -> usize {
    offset as usize
}

fn starts_with_word(units: &[u16], at: usize, word: &str) -> bool {
    let word: Vec<u16> = word.encode_utf16().collect();
    units.get(at..at + word.len()) == Some(word.as_slice())
        && units
            .get(at + word.len())
            .is_none_or(|&next| !is_identifier_unit(next))
}

fn is_identifier_unit(unit: u16) -> bool {
    char::from_u32(u32::from(unit)).is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// Skip whitespace, line terminators and comments from `i`.
fn skip_trivia(units: &[u16], mut i: usize) -> usize {
    let slash = u16::from(b'/');
    let star = u16::from(b'*');
    loop {
        match units.get(i) {
            Some(&unit) if is_space_unit(unit) => i += 1,
            Some(&unit) if unit == slash && units.get(i + 1) == Some(&slash) => {
                while units.get(i).is_some_and(|&unit| !is_line_terminator(unit)) {
                    i += 1;
                }
            }
            Some(&unit) if unit == slash && units.get(i + 1) == Some(&star) => {
                i += 2;
                while i < units.len() && !(units[i] == star && units.get(i + 1) == Some(&slash)) {
                    i += 1;
                }
                i = (i + 2).min(units.len());
            }
            _ => return i,
        }
    }
}

fn is_line_terminator(unit: u16) -> bool {
    matches!(unit, 0x0A | 0x0D | 0x2028 | 0x2029)
}

fn is_space_unit(unit: u16) -> bool {
    unit == 0xFEFF || char::from_u32(u32::from(unit)).is_some_and(char::is_whitespace)
}

/// `value` as `expected.json`'s bytes: tab-indented, one trailing newline.
///
/// # Errors
///
/// A serialization error.
pub fn render_expected(value: &Value) -> Result<String, String> {
    crate::json::to_json_with_tabs(value)
        .map(|text| text + "\n")
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn trivia_skips_both_comment_forms() {
        let text = units(" /* a */ // b\n\twith");
        assert_eq!(skip_trivia(&text, 0), 15);
    }

    #[test]
    fn a_keyword_is_a_whole_word() {
        assert!(starts_with_word(&units("with {"), 0, "with"));
        assert!(!starts_with_word(&units("without"), 0, "with"));
    }

    #[test]
    fn ambient_nodes_are_skipped() {
        let declared = json!({"type": "VariableDeclaration", "declare": true});
        let module = json!({"type": "TSModuleDeclaration", "id": {"type": "Literal"}});
        let namespace = json!({"type": "TSModuleDeclaration", "id": {"type": "Identifier"}});
        let abstract_class = json!({"type": "ClassDeclaration", "abstract": true});
        let object = |value: &Value| value.as_object().cloned().unwrap_or_default();
        assert!(is_ambient(&object(&declared)));
        assert!(is_ambient(&object(&module)));
        assert!(!is_ambient(&object(&namespace)));
        assert!(!is_ambient(&object(&abstract_class)));
    }
}
