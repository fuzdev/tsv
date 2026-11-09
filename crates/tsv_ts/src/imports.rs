//! Import records: which modules a source requests, where, and whether the request is
//! type-only.
//!
//! [`parse_imports`] runs the parser and collects one [`ImportRecord`] per module request:
//! a static `import`, a re-export (`export … from`, `export * from`), a dynamic
//! `import(…)`, and a TypeScript `import x = require('y')`. Records come out in source
//! order, every position a UTF-8 byte offset into the source ([`ByteSpan`]), the
//! specifier's value decoded on demand ([`ImportRecord::specifier_value`]).
//!
//! The records are collected by the type strip's walk (`strip/walk.rs`), run in a mode of
//! its own: [`strip`](crate::strip()) collects none, so it pays for none. Collecting, the
//! walk also descends into the four value subtrees the strip refuses (and so never needs
//! to walk): an enum's member initializers, a value namespace's body, an `export =`
//! operand, and the operand of an `as` / `satisfies` whose blanking would regroup the
//! expression around it.
//!
//! **Ambient code holds no records.** A `declare`d declaration, the body of a
//! `declare module 'x'` / `declare global` / `declare namespace`, and a bodiless
//! signature (an overload, an abstract member) describe types, not code that runs, so the
//! imports inside them are not collected.
//!
//! **A namespace body holds dynamic imports alone.** A static import, a re-export or an
//! `import x = require('y')` inside a namespace (nested ones included) is no record: tsc
//! rejects each there (TS1147, TS1194). An `import(…)` there is a record, as anywhere a
//! value runs.
//!
//! **Types are not walked.** A computed key inside an interface or a type literal
//! (`interface I { [import('./i')]: T }`) is an `import(…)` expression in the tree, but it
//! sits in a type and runs nothing — no record. So is a type-position import
//! (`let x: import('./a').T`, `typeof import('./a')`): a **known gap**, since a dependency
//! graph over types would want it. A consumer building a runtime graph drops type-only
//! requests anyway; a later type walk can add them.
//!
//! The fixtures under `tests/fixtures_imports/` define the records, graded by
//! `tests/imports_fixtures_tests.rs`.

use std::borrow::Cow;

use tsv_lang::Error;

use crate::Goal;
use crate::ast::Program;
use crate::strip::walk::{Mode, Walk};

/// A byte range in the source a record was collected from: `start..end`, UTF-8 byte
/// offsets (not the UTF-16 offsets tsv's bindings report). A source is capped at
/// 4 GiB − 1 bytes, so the offsets are `u32`.
///
/// ```
/// use tsv_ts::ByteSpan;
///
/// let span = ByteSpan { start: 8, end: 11 };
/// assert_eq!(span.range(), 8..11);
/// assert_eq!(span.text("import 'abc';"), "abc");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ByteSpan {
    /// The first byte's offset.
    pub start: u32,
    /// The offset one past the last byte (`start` for an empty span).
    pub end: u32,
}

impl ByteSpan {
    /// The span as the `Range<usize>` that indexes the source.
    #[must_use]
    pub const fn range(self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }

    /// The span's text in `source`.
    ///
    /// # Panics
    ///
    /// When the span is out of bounds of `source` or not on a character boundary — a span
    /// read against a source other than the one it was collected from.
    #[must_use]
    pub fn text(self, source: &str) -> &str {
        &source[self.range()]
    }
}

/// The import records of one source, and whether it uses module syntax.
///
/// ```
/// # fn main() -> tsv_ts::Result<()> {
/// let imports = tsv_ts::parse_imports("let x = 1;")?;
/// assert!(imports.records.is_empty());
/// assert!(!imports.has_module_syntax);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct Imports {
    /// Every module request, in source order.
    pub records: Vec<ImportRecord>,
    /// Whether the source uses syntax only a Module parses: an ECMAScript `import` or
    /// `export` declaration at its top level (`import type`, `export type`, `export {}`,
    /// `export default` and a local `export const` included; an `export` inside a
    /// namespace is TypeScript's namespace export, not module syntax), an `import.meta`
    /// anywhere outside types, or a top-level `await`, `for await` or `await using`
    /// (outside every function, class static block, field initializer, namespace body and
    /// enum member initializer — the last two function-like for `await` in tsc, TS1308).
    /// A dynamic `import(…)`, an `import x = require('y')`, an `export import x = …`, an
    /// `export =` and an `export as namespace` are not module syntax. This is the
    /// module-syntax core of what Node's ESM syntax detection reads (Node also weighs
    /// signals that are not module syntax, such as a top-level redeclaration of a CommonJS
    /// wrapper name). tsc's module indicator is a different set: it adds an
    /// `import x = require('y')`, an `export =` and any `export`-modified declaration
    /// (`export import X = require('y')` included), and omits a top-level `await` or
    /// `await using`.
    ///
    /// Always `false` for a source read at `Goal::Script`: that grammar rejects all of it
    /// (a top-level `await` there is an identifier).
    pub has_module_syntax: bool,
}

/// One module request.
///
/// `Copy`: a record holds spans into its source, never a copy of the text;
/// [`ImportRecord::specifier_value`] decodes the specifier when asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImportRecord {
    /// What form of request this is.
    pub kind: ImportRecordKind,
    /// The whole request: the statement for a static import, a re-export or an
    /// import-equals, the `import(…)` expression for a dynamic import.
    pub span: ByteSpan,
    /// The specifier literal's contents, its quotes (or backticks) excluded. `None` for a
    /// dynamic import whose specifier is computed — anything but a string literal or a
    /// template literal with no substitutions.
    pub specifier: Option<ByteSpan>,
    /// Whether the request is for types alone.
    pub type_only: TypeOnly,
    /// The import's phase: `import source` / `import defer`, `import.source(…)` /
    /// `import.defer(…)`.
    pub phase: ImportPhase,
    /// For a static import or a re-export, the attributes clause from its `with` keyword
    /// through its closing `}`; for a dynamic import, its options argument. `None` when
    /// there is none.
    pub attributes: Option<ByteSpan>,
}

impl ImportRecord {
    /// The specifier's value, decoded: a string literal's escapes, or a template
    /// literal's cooked value (a `\r` or `\r\n` read as `\n`) — the value tsv's parse
    /// wire carries. Borrowed from `source` when there is nothing to decode. `None` when
    /// the record has no specifier, and only then.
    ///
    /// The decoding is the parse wire's in its two lenient corners: an untagged template's
    /// escape with no cooked value, which tsv's parser defers rather than rejects, decodes
    /// as the wire decodes it (`` `\1` `` is U+0001), and a lone surrogate escape
    /// (`'\uD800'`), which a Rust string cannot hold, decodes to U+FFFD.
    ///
    /// ```
    /// # fn main() -> tsv_ts::Result<()> {
    /// let source = r"import('\x41');";
    /// let [record] = tsv_ts::parse_imports(source)?.records[..] else {
    ///     panic!("one record");
    /// };
    /// assert_eq!(record.specifier_value(source).as_deref(), Some("A"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Panics
    ///
    /// When `source` is not the text the record was collected from: the specifier's span
    /// is out of bounds of it or not on a character boundary, as [`ByteSpan::text`]
    /// panics, or its text holds an escape that does not decode — a source whose escapes
    /// do not decode does not parse, so no record of its own holds one.
    #[must_use]
    #[expect(
        clippy::panic,
        reason = "a record read against a source other than its own is a caller bug, \
                  as `ByteSpan::text` treats it"
    )]
    pub fn specifier_value<'s>(&self, source: &'s str) -> Option<Cow<'s, str>> {
        let span = self.specifier?;
        let raw = span.text(source);
        let is_template = span
            .start
            .checked_sub(1)
            .and_then(|delimiter| source.as_bytes().get(delimiter as usize))
            == Some(&b'`');
        let decoded = if is_template && raw.contains('\r') {
            let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
            crate::lexer::escapes::decode_string_escapes(&normalized)
        } else if raw.contains('\\') {
            crate::lexer::escapes::decode_string_escapes(raw)
        } else {
            return Some(Cow::Borrowed(raw));
        };
        match decoded {
            Ok(value) => Some(Cow::Owned(value)),
            Err(error) => panic!(
                "a specifier that does not decode ({error}): the record was not collected \
                 from this source"
            ),
        }
    }

    /// A dynamic import over `span` with `specifier`, every other field its default.
    #[cfg(test)]
    const fn dynamic(span: ByteSpan, specifier: Option<ByteSpan>) -> Self {
        Self {
            kind: ImportRecordKind::Dynamic,
            span,
            specifier,
            type_only: TypeOnly::No,
            phase: ImportPhase::None,
            attributes: None,
        }
    }
}

// A new variant compiles silently at every out-of-crate match, which `#[non_exhaustive]`
// forces to carry a `_` arm: binding code names it through `as_str`, never a `match`.
/// The form of an [`ImportRecord`]'s request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImportRecordKind {
    /// `import … from 'x'` or `import 'x'`, `import type` included.
    Static,
    /// `export … from 'x'` or `export * from 'x'`, `export type` included.
    ReExport,
    /// `import(…)`, `import.source(…)` and `import.defer(…)` included.
    Dynamic,
    /// TypeScript's `import x = require('x')`, `export import` and `import type` included.
    /// The entity-name form `import x = A.B` is an alias, not a request, and no record.
    ImportEquals,
}

impl ImportRecordKind {
    /// The kind's wire name — snake_case, the `kind` a JS record carries (`static`,
    /// `re_export`, `dynamic`, `import_equals`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::ReExport => "re_export",
            Self::Dynamic => "dynamic",
            Self::ImportEquals => "import_equals",
        }
    }
}

// A new variant compiles silently at every out-of-crate match, which `#[non_exhaustive]`
// forces to carry a `_` arm: binding code names it through `as_str`, never a `match`.
/// Whether an [`ImportRecord`]'s request is for types alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TypeOnly {
    /// A value request — or a mix of value and type specifiers.
    #[default]
    No,
    /// The statement is type-only: `import type`, `export type`, `export type *`,
    /// `import type x = require('y')`. Type stripping removes it.
    Statement,
    /// Every specifier is marked inline `type` (`import {type A, type B} from 'x'`), at
    /// least one and none a default or namespace import. Type stripping keeps the
    /// statement with its specifiers emptied (`import {} from 'x'`), so the stripped JS
    /// still loads the module; a dependency graph over types erased may drop it.
    Specifiers,
}

impl TypeOnly {
    /// The wire name — snake_case, the `typeOnly` a JS record carries (`statement`,
    /// `specifiers`), or `None` for [`TypeOnly::No`] (JS `null`).
    #[must_use]
    pub const fn as_str(self) -> Option<&'static str> {
        match self {
            Self::No => None,
            Self::Statement => Some("statement"),
            Self::Specifiers => Some("specifiers"),
        }
    }
}

// A new variant compiles silently at every out-of-crate match, which `#[non_exhaustive]`
// forces to carry a `_` arm: binding code names it through `as_str`, never a `match`.
/// An import's phase (the TC39 source-phase and deferred-import proposals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ImportPhase {
    /// An ordinary import, evaluated.
    #[default]
    None,
    /// `import source x from 'y'` / `import.source('y')`.
    Source,
    /// `import defer * as x from 'y'` / `import.defer('y')`.
    Defer,
}

impl ImportPhase {
    /// The wire name — `source` or `defer`, the `phase` a JS record and the parse wire
    /// carry — or `None` for an ordinary import (JS `null`, and no `phase` field on the
    /// parse wire).
    #[must_use]
    pub const fn as_str(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Source => Some("source"),
            Self::Defer => Some("defer"),
        }
    }
}

/// Collect the import records of TypeScript or JavaScript `source`.
///
/// No goal is named, so the source is read as [`strip()`](crate::strip()) reads it: as a
/// module, retried as a script only if that fails (see [Goals](crate#goals)).
/// [`parse_imports_with_goal`] names one exactly.
///
/// ```
/// use tsv_ts::{ImportRecordKind, TypeOnly};
///
/// # fn main() -> tsv_ts::Result<()> {
/// let source = "import type {T} from './t';\nconst b = await import(`./b`);";
/// let imports = tsv_ts::parse_imports(source)?;
/// assert!(imports.has_module_syntax);
/// let [t, b] = &imports.records[..] else {
///     panic!("two records");
/// };
/// assert_eq!((t.kind, t.type_only), (ImportRecordKind::Static, TypeOnly::Statement));
/// assert_eq!(t.span.text(source), "import type {T} from './t';");
/// assert_eq!(b.kind, ImportRecordKind::Dynamic);
/// assert_eq!(b.span.text(source), "import(`./b`)");
/// assert_eq!(b.specifier.map(|s| s.text(source)), Some("./b"));
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// A parse error, when `source` does not parse at either goal (the error reported is the
/// one [Goals](crate#goals) picks).
pub fn parse_imports(source: &str) -> Result<Imports, Error> {
    parse_imports_one_shot(source, None)
}

/// [`parse_imports`] at exactly `goal`, with no Module-to-Script fallback: a source the
/// goal's grammar rejects is an error even when the other goal would accept it.
///
/// ```
/// use tsv_ts::{Goal, parse_imports_with_goal};
///
/// assert!(parse_imports_with_goal("import a from 'a';", Goal::Script).is_err());
/// assert!(parse_imports_with_goal("import a from 'a';", Goal::Module).is_ok());
/// ```
///
/// # Errors
///
/// A parse error, when `source` does not parse at `goal`.
pub fn parse_imports_with_goal(source: &str, goal: Goal) -> Result<Imports, Error> {
    parse_imports_one_shot(source, Some(goal))
}

/// The one-shot entry points' arena: a fresh one sized to the source, dropped on return —
/// the [`Imports`] are owned.
fn parse_imports_one_shot(source: &str, goal: Option<Goal>) -> Result<Imports, Error> {
    let arena = bumpalo::Bump::with_capacity(tsv_lang::estimated_ast_arena_capacity(source.len()));
    parse_imports_in(source, goal, &arena)
}

/// [`parse_imports`] over a caller-owned `arena` — the hidden primitive beneath
/// [`parse_imports`] and [`parse_imports_with_goal`], for a hot loop that keeps one arena
/// warm across sources, reset by the caller between calls.
///
/// The parse is `__private::parse_with_goal_or_fallback`'s, as the strip's: `Some(goal)`
/// is exact, and `None` parses at `Module`, retried at `Script` only if that fails.
///
/// # Errors
///
/// As [`parse_imports`].
pub fn parse_imports_in(
    source: &str,
    goal: Option<Goal>,
    arena: &bumpalo::Bump,
) -> Result<Imports, Error> {
    parse_imports_into(source, goal, arena, &mut crate::ast::AstStores::new())
}

/// [`parse_imports_in`] into a caller's `stores`, for a warm path that keeps them across
/// sources.
///
/// # Errors
///
/// As [`parse_imports`].
pub fn parse_imports_into<'arena>(
    source: &str,
    goal: Option<Goal>,
    arena: &'arena bumpalo::Bump,
    stores: &mut crate::ast::AstStores<'arena>,
) -> Result<Imports, Error> {
    let program = crate::__private::parse_with_goal_or_fallback_into(source, goal, arena, stores)?;
    Ok(collect_imports(&[&program], source, arena))
}

/// The import records of parsed `programs`, which index `source` — the hidden seam an
/// embedding language collects through (`tsv_svelte`, over a component's two scripts,
/// whose spans are document offsets). The programs must come in source order, which is
/// then the records' order; `has_module_syntax` answers for all of them. `arena` holds
/// the walk's scratch, which is never read.
pub fn collect_imports(programs: &[&Program<'_>], source: &str, arena: &bumpalo::Bump) -> Imports {
    let mut walk = Walk::new(source, arena, Mode::Imports);
    for program in programs {
        walk.program(program);
    }
    let imports = walk.into_imports();
    debug_assert!(
        imports.records.is_sorted_by_key(|record| record.span.start),
        "the walk is pre-order, so its records come in source order"
    );
    imports
}

/// Write `imports` as the JSON a JS binding returns and the fixtures pin:
///
/// ```json
/// {"hasModuleSyntax": bool, "imports": [{"kind", "start", "end",
///   "specifier": {"value", "start", "end"} | null, "typeOnly": null | "statement" | "specifiers",
///   "phase": null | "source" | "defer", "attributes": {"start", "end"} | null}]}
/// ```
///
/// Every offset is a UTF-16 code unit offset in `coordinates` — the language's own
/// (`crate::WIRE_COORDINATES` for TypeScript, which counts a leading BOM; the Svelte
/// crate's elides it), so the records index the same text the language's parse wire
/// does. `value` is [`ImportRecord::specifier_value`], always a string: a record with a
/// specifier always has a value.
///
/// # Panics
///
/// When `source` is not the text the records were collected from.
#[cfg(feature = "json")]
pub fn write_imports_json(
    imports: &Imports,
    source: &str,
    coordinates: tsv_lang::WireCoordinates,
) -> Vec<u8> {
    use tsv_lang::{JsonWriter, Wire, WireTables};

    let tables = WireTables::new(source, coordinates, Wire::Span);
    let positions = tables.positions();
    let start_end = |w: &mut JsonWriter, span: ByteSpan| {
        w.raw("\"start\":");
        w.u32(positions.pos(span.start));
        w.raw(",\"end\":");
        w.u32(positions.pos(span.end));
    };
    let mut w = JsonWriter::with_capacity(64 + imports.records.len() * 160);
    w.raw(if imports.has_module_syntax {
        "{\"hasModuleSyntax\":true,\"imports\":["
    } else {
        "{\"hasModuleSyntax\":false,\"imports\":["
    });
    for (i, record) in imports.records.iter().enumerate() {
        if i > 0 {
            w.raw(",");
        }
        w.raw("{\"kind\":\"");
        w.raw(record.kind.as_str());
        w.raw("\",");
        start_end(&mut w, record.span);
        w.raw(",\"specifier\":");
        // `specifier_value` is `None` only when there is no specifier (it panics on a
        // foreign source), so this pairs every specifier with its value
        match record.specifier.zip(record.specifier_value(source)) {
            Some((specifier, value)) => {
                w.raw("{\"value\":");
                w.string(&value);
                w.raw(",");
                start_end(&mut w, specifier);
                w.raw("}");
            }
            None => w.raw("null"),
        }
        w.raw(",\"typeOnly\":");
        write_name_or_null(&mut w, record.type_only.as_str());
        w.raw(",\"phase\":");
        write_name_or_null(&mut w, record.phase.as_str());
        w.raw(",\"attributes\":");
        match record.attributes {
            Some(attributes) => {
                w.raw("{");
                start_end(&mut w, attributes);
                w.raw("}");
            }
            None => w.raw("null"),
        }
        w.raw("}");
    }
    w.raw("]}");
    w.into_bytes()
}

/// [`write_imports_json`] as a `String`: the same bytes, for a `&str` boundary.
///
/// # Panics
///
/// When `source` is not the text the records were collected from, as
/// [`write_imports_json`] does.
#[cfg(feature = "json")]
#[expect(clippy::expect_used)]
pub fn write_imports_json_string(
    imports: &Imports,
    source: &str,
    coordinates: tsv_lang::WireCoordinates,
) -> String {
    String::from_utf8(write_imports_json(imports, source, coordinates))
        .expect("the writer emits ASCII fragments and escaped strings: UTF-8")
}

/// An escape-free wire name as a JSON string, or `null`.
#[cfg(feature = "json")]
fn write_name_or_null(w: &mut tsv_lang::JsonWriter, name: Option<&'static str>) {
    match name {
        Some(name) => {
            w.raw("\"");
            w.raw(name);
            w.raw("\"");
        }
        None => w.raw("null"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_at(source: &str, specifier: &str) -> ImportRecord {
        let start = source
            .find(specifier)
            .expect("the specifier is in the source");
        let span = ByteSpan {
            start: start as u32,
            end: (start + specifier.len()) as u32,
        };
        ImportRecord::dynamic(
            ByteSpan {
                start: 0,
                end: source.len() as u32,
            },
            Some(span),
        )
    }

    #[test]
    fn a_specifier_without_escapes_is_borrowed() {
        let source = "import('./a.js');";
        let value = record_at(source, "./a.js").specifier_value(source);
        assert!(matches!(value, Some(Cow::Borrowed("./a.js"))));
    }

    #[test]
    fn a_string_specifier_decodes_its_escapes() {
        let source = "import('\\x41\\u{42}\\\nC');";
        let value = record_at(source, "\\x41\\u{42}\\\nC").specifier_value(source);
        assert_eq!(value.as_deref(), Some("ABC"));
    }

    #[test]
    fn a_template_specifier_reads_carriage_returns_as_line_feeds() {
        let source = "import(`a\r\nb\rc\\x41`);";
        let value = record_at(source, "a\r\nb\rc\\x41").specifier_value(source);
        assert_eq!(value.as_deref(), Some("a\nb\ncA"));
    }

    #[test]
    fn a_line_continuation_over_crlf_is_empty() {
        let source = "import('a\\\r\nb');";
        let value = record_at(source, "a\\\r\nb").specifier_value(source);
        assert_eq!(value.as_deref(), Some("ab"));
    }

    #[test]
    fn a_computed_dynamic_import_has_no_value() {
        let record = ImportRecord::dynamic(ByteSpan { start: 0, end: 9 }, None);
        assert_eq!(record.specifier_value("import(a)"), None);
    }

    #[test]
    #[should_panic(expected = "byte index")]
    fn a_span_past_the_source_panics() {
        let source = "import('./a.js');";
        let _ = record_at(source, "./a.js").specifier_value("import(");
    }

    #[test]
    #[should_panic(expected = "not collected from this source")]
    fn a_specifier_that_does_not_decode_panics() {
        let source = "import('\\x4');";
        let _ = record_at(source, "\\x4").specifier_value(source);
    }

    #[test]
    fn the_wire_names() {
        assert_eq!(ImportRecordKind::ReExport.as_str(), "re_export");
        assert_eq!(ImportRecordKind::ImportEquals.as_str(), "import_equals");
        assert_eq!(TypeOnly::No.as_str(), None);
        assert_eq!(TypeOnly::Specifiers.as_str(), Some("specifiers"));
        assert_eq!(ImportPhase::Defer.as_str(), Some("defer"));
    }

    fn records(source: &str) -> Vec<ImportRecord> {
        match parse_imports(source) {
            Ok(imports) => imports.records,
            Err(error) => panic!("{source:?} does not parse: {error:#}"),
        }
    }

    fn specifiers(source: &str) -> Vec<&str> {
        let mut out = Vec::new();
        for record in records(source) {
            let Some(specifier) = record.specifier else {
                panic!("a computed specifier in {source:?}");
            };
            out.push(specifier.text(source));
        }
        out
    }

    fn has_module_syntax(source: &str) -> bool {
        match parse_imports(source) {
            Ok(imports) => imports.has_module_syntax,
            Err(error) => panic!("{source:?} does not parse: {error:#}"),
        }
    }

    #[test]
    fn a_chain_keeps_source_order() {
        assert_eq!(
            specifiers("import('a').then(() => import('b'));"),
            ["a", "b"]
        );
        assert_eq!(
            specifiers("import('a')[import('b')](import('c')).d`${import('e')}`;"),
            ["a", "b", "c", "e"]
        );
    }

    #[test]
    fn a_long_chain_keeps_source_order() {
        // past `CHAIN_FRAMES` link operands, the walk takes a chain by its loop, which
        // walks the base and then each link's tail innermost first
        let links = ".m()".repeat(40);
        let source = format!("import('a'){links}.n(import('b')){links}.n(import('c') as T);");
        assert_eq!(specifiers(&source), ["a", "b", "c"]);
        let source = format!("(import('a') as T){links}.n(import('b'));");
        assert_eq!(specifiers(&source), ["a", "b"]);
    }

    #[test]
    fn the_refused_subtrees_are_walked_for_records() {
        for (source, expected) in [
            ("enum E { A = (import('a'), 1) }", &["a"][..]),
            ("namespace N { export const a = import('a'); }", &["a"]),
            ("namespace N.M { export const a = import('a'); }", &["a"]),
            (
                "namespace N { import b from 'b'; export * from 'c'; export const a = import('a'); }",
                &["a"],
            ),
            ("export = import('a');", &["a"]),
            ("let a = b + import('a') as T / d;", &["a"]),
            (
                "let a = b + import('a') satisfies T / import('c');",
                &["a", "c"],
            ),
            ("let a = -import('a') as T ** 2;", &["a"]),
        ] {
            assert!(
                crate::strip(source).is_err(),
                "{source:?} is not refused by the strip"
            );
            assert_eq!(specifiers(source), expected, "{source:?}");
        }
        // a regrouping assertion over a run of assertions past `CHAIN_FRAMES`, whose
        // inner links the walk takes by its chain loop
        let source = format!("let a = b + import('a'){} / d;", " as T".repeat(40));
        assert!(crate::strip(&source).is_err());
        assert_eq!(specifiers(&source), ["a"]);
    }

    #[test]
    fn module_syntax_beyond_the_fixtures() {
        for (source, expected) in [
            ("function f() { new.target; }", false),
            ("const o = { async m() { await a; } };", false),
            ("class C { a = async () => await b; }", false),
            ("class C { static { x; } m() { import.meta; } }", true),
            ("export = a;", false),
            ("export as namespace N;", false),
            ("export import a = require('./a');", false),
            ("namespace N { export const a = 1; }", false),
            ("export declare const a: number;", true),
            ("export type T = 1;", true),
            ("declare namespace N { export const a: number; }", false),
            ("class C extends (await b) {}", true),
            ("const a = { [await b]: 1 };", true),
            ("label: for await (const a of b) {}", true),
            ("async function* g() { for await (const a of b) {} }", false),
            ("(async () => { await a; })();", false),
            ("async function f() { await using a = b; }", false),
            ("{ await using a = b; }", true),
            ("namespace N { export const a = import.meta; }", true),
        ] {
            assert_eq!(has_module_syntax(source), expected, "{source:?}");
        }
    }

    #[test]
    fn the_record_fields() {
        let source = "export {type A, type B} from './a' /* c */ with { type: 'json', };";
        let [record] = records(source)[..] else {
            panic!("one record");
        };
        assert_eq!(record.kind, ImportRecordKind::ReExport);
        assert_eq!(record.type_only, TypeOnly::Specifiers);
        assert_eq!(
            record.attributes.map(|a| a.text(source)),
            Some("with { type: 'json', }")
        );
        let source = "import x = require('./x');\nimport type y = require('./y');";
        let [x, y] = records(source)[..] else {
            panic!("two records");
        };
        assert_eq!(
            (x.kind, x.type_only),
            (ImportRecordKind::ImportEquals, TypeOnly::No)
        );
        assert_eq!(y.type_only, TypeOnly::Statement);
        let source = "import('./a', {with: {type: 'json'}}); import(a);";
        let [a, computed] = records(source)[..] else {
            panic!("two records");
        };
        assert_eq!(
            a.attributes.map(|a| a.text(source)),
            Some("{with: {type: 'json'}}")
        );
        assert_eq!(computed.specifier, None);
        assert_eq!(computed.span.text(source), "import(a)");
    }

    #[cfg(feature = "json")]
    #[test]
    fn the_json_counts_utf16_units() {
        let source = "/* é😀 */ import('ü');";
        let start = source.find("import").expect("the import") as u32;
        let specifier = source.find('ü').expect("the specifier") as u32;
        let imports = Imports {
            records: vec![ImportRecord::dynamic(
                ByteSpan {
                    start,
                    end: source.len() as u32 - 1,
                },
                Some(ByteSpan {
                    start: specifier,
                    end: specifier + 2,
                }),
            )],
            has_module_syntax: false,
        };
        let json = write_imports_json(&imports, source, crate::WIRE_COORDINATES);
        assert_eq!(
            String::from_utf8(json).expect("UTF-8"),
            r#"{"hasModuleSyntax":false,"imports":[{"kind":"dynamic","start":10,"end":21,"specifier":{"value":"ü","start":18,"end":19},"typeOnly":null,"phase":null,"attributes":null}]}"#
        );
    }
}
