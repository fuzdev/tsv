//! Import/export specifier normalization mirroring esrap's printing rule.
//!
//! Svelte's `compile()` reprints its generated module through esrap, and esrap
//! prints the `as` clause of an import/export **specifier** only when *both* name
//! sides are plain identifiers. If either side is a string `Literal`, esrap drops
//! the alias and prints only the identifier **binding** — the `local` for an
//! export specifier, the `local` binding for an import specifier. A drop-in
//! replacement must reproduce that (lossy) behavior byte-for-byte:
//!
//! | source                            | esrap emits          |
//! | --------------------------------- | -------------------- |
//! | `export { x as notdefault }`      | `export { x as notdefault }` (untouched) |
//! | `export { x as 'notdefault' }`    | `export { x }`       |
//! | `export { 'a-b' as notdefault }`  | `export { 'a-b' }`   |
//! | `export { 'a-b' as 'c-d' }`       | `export { 'a-b' }`   |
//! | `import { 'a-b' as loc }`         | `import { loc }`     |
//! | `import { thing as loc }`         | `import { thing as loc }` (untouched) |
//!
//! `export *` / `export * as 'str'` (an [`ExportAllDeclaration`]) keeps its name
//! verbatim in esrap — the drop rule is specifier-only — so it is deliberately
//! **not** touched here. `x as x` / `import { x as x }` (both sides identifiers)
//! is also untouched: this pass fires only when a `Literal` is involved, so the
//! separate identifier-self-alias gap is naturally left alone.
//!
//! **Mechanism, not a printer change.** tsv's specifier printer
//! ([`build_renamed_specifier_doc`]) emits `as right` iff `left.span() !=
//! right.span()`. This pass makes the two sides share a span by cloning the
//! surviving binding into the alias slot, so the printer takes its bare-form path
//! with no printer edit and no separate `esrap`-parity flag.
//!
//! **Scope.** A compiler-side AST transform over the SERVER-EMITTED programs only
//! — the `<script module>` body and the hoisted instance-script imports — run at
//! program-construction time in [`crate::transform_server`], never the parser,
//! formatter, or wire AST.
//!
//! **Structural sharing** follows [`crate::erase`]: `None` means *unchanged*, so a
//! list (or specifier) with no string-named specifier is never rebuilt and nothing
//! is allocated. Only a specifier's `exported` / `imported` sub-field is swapped —
//! the specifier's own span and the enclosing declaration span stay put, so the
//! comment-placement windows keyed on statement/block spans are undisturbed.
//!
//! **Comment safety (module body only).** The collapse has one lossy edge: esrap
//! DROPS the `as` alias but KEEPS a comment sitting in that alias gap, while tsv's
//! span collapse makes the specifier printer skip the gap (its `left.span() !=
//! right.span()` test now fails), which would silently DROP the comment — a
//! content-loss divergence. Only the module-body program carries comments (the F1
//! keep set, [`crate::transform_server`]'s `module_comments`); the synthetic `$`
//! import and the hoisted instance imports print comment-free. So the module body
//! goes through [`normalize_module_specifiers_checked`], which REFUSES
//! ([`crate::Refusal::CommentInDroppedSpecifierAlias`]) when a KEPT comment falls in
//! a collapsing specifier's skipped as-gap rather than dropping it; a comment
//! neither side emits is dropped by both (parity) and does not refuse. The plain
//! [`normalize_module_specifiers`] stays infallible for the comment-free
//! instance-import path.
//!
//! [`build_renamed_specifier_doc`]: the doc builder in
//! `tsv_ts::printer::statements::modules::specifier_list`

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Comment;
use tsv_ts::ast::internal::{
    ExportNamedDeclaration, ExportSpecifier, ImportDeclaration, ImportNamedSpecifier,
    ImportSpecifier, ModuleExportName, Statement,
};

use crate::transform_server::unsupported;
use crate::{CompileError, Refusal};

/// Normalize every import/export specifier whose name side is a string `Literal`
/// to esrap's bare form. Returns the input slice unchanged when no statement
/// carried such a specifier (structural sharing — nothing allocated).
///
/// Infallible: used for the comment-free programs (the `$` scaffold + hoisted
/// instance imports). The module-body path uses
/// [`normalize_module_specifiers_checked`], which guards against dropping a kept
/// as-gap comment.
pub(crate) fn normalize_module_specifiers<'arena>(
    arena: &'arena bumpalo::Bump,
    stmts: &'arena [Statement<'arena>],
) -> &'arena [Statement<'arena>] {
    let mut out: Option<BumpVec<'arena, Statement<'arena>>> = None;
    for (i, stmt) in stmts.iter().enumerate() {
        match normalize_statement(arena, stmt) {
            None => {
                if let Some(vec) = out.as_mut() {
                    vec.push(stmt.clone());
                }
            }
            Some(new) => rebuilt(&mut out, arena, stmts, i).push(new),
        }
    }
    out.map_or(stmts, BumpVec::into_bump_slice)
}

/// [`normalize_module_specifiers`] preceded by the module-body comment guard: a
/// KEPT comment (`module_comments`) sitting in a collapsing specifier's skipped
/// as-gap refuses ([`Refusal::CommentInDroppedSpecifierAlias`]) rather than being
/// silently dropped by the span collapse. Used only for the module-body program,
/// the sole server-emitted program that carries comments.
pub(crate) fn normalize_module_specifiers_checked<'arena>(
    arena: &'arena bumpalo::Bump,
    stmts: &'arena [Statement<'arena>],
    module_comments: &[Comment],
) -> Result<&'arena [Statement<'arena>], CompileError> {
    refuse_dropped_specifier_alias_comment(stmts, module_comments)?;
    Ok(normalize_module_specifiers(arena, stmts))
}

/// Lazily start the rebuilt list, back-filling the unchanged prefix `stmts[..i]`
/// the first time a statement is replaced (the [`crate::erase`] `rebuilt_list`
/// idiom, kept private so both list walks share it).
fn rebuilt<'out, 'arena, T: Clone>(
    out: &'out mut Option<BumpVec<'arena, T>>,
    arena: &'arena bumpalo::Bump,
    items: &[T],
    i: usize,
) -> &'out mut BumpVec<'arena, T> {
    out.get_or_insert_with(|| {
        let mut vec = BumpVec::with_capacity_in(items.len(), arena);
        vec.extend_from_slice(&items[..i]);
        vec
    })
}

/// The two — and only two — statement kinds that carry a named import/export
/// specifier list esrap's alias-drop applies to.
///
/// The **single exhaustive dispatch** shared by the collapse
/// ([`normalize_statement`]) and the comment guard
/// ([`refuse_dropped_specifier_alias_comment`]), so a new specifier-bearing
/// statement variant fails compilation HERE rather than silently escaping both.
enum SpecifierBearing<'a, 'arena> {
    Import(&'a ImportDeclaration<'arena>),
    Export(&'a ExportNamedDeclaration<'arena>),
}

/// Classify a statement as specifier-bearing (or not). `ExportAllDeclaration`
/// (`export * as 'str'`) is deliberately excluded — esrap keeps its name verbatim,
/// the drop rule being specifier-only — as is every other statement. Exhaustive on
/// purpose (see [`SpecifierBearing`]).
fn specifier_bearing<'a, 'arena>(
    stmt: &'a Statement<'arena>,
) -> Option<SpecifierBearing<'a, 'arena>> {
    match stmt {
        Statement::ImportDeclaration(decl) => Some(SpecifierBearing::Import(decl)),
        Statement::ExportNamedDeclaration(decl) => Some(SpecifierBearing::Export(decl)),
        // `export *` / `export * as 'str'` keeps its name verbatim; every other
        // statement carries no import/export specifier to normalize.
        Statement::ExportAllDeclaration(_)
        | Statement::BlockStatement(_)
        | Statement::FunctionDeclaration(_)
        | Statement::ClassDeclaration(_)
        | Statement::ExpressionStatement(_)
        | Statement::VariableDeclaration(_)
        | Statement::ReturnStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::IfStatement(_)
        | Statement::ForStatement(_)
        | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::WhileStatement(_)
        | Statement::DoWhileStatement(_)
        | Statement::SwitchStatement(_)
        | Statement::TryStatement(_)
        | Statement::LabeledStatement(_)
        | Statement::ExportDefaultDeclaration(_)
        | Statement::TSExportAssignment(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::TSNamespaceExportDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSDeclareFunction(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::TSModuleDeclaration(_) => None,
    }
}

/// Collapse the string-named specifiers of a single statement, or `None` when it
/// carries none to normalize (structural sharing). Rides the exhaustive
/// [`specifier_bearing`] classifier.
fn normalize_statement<'arena>(
    arena: &'arena bumpalo::Bump,
    stmt: &Statement<'arena>,
) -> Option<Statement<'arena>> {
    match specifier_bearing(stmt)? {
        SpecifierBearing::Import(decl) => {
            normalize_import_specifiers(arena, decl.specifiers).map(|specifiers| {
                Statement::ImportDeclaration(ImportDeclaration {
                    specifiers,
                    ..decl.clone()
                })
            })
        }
        // A `declaration`-form export (`export const x`) carries an empty specifier
        // list, so it rebuilds to `None` and passes through untouched.
        SpecifierBearing::Export(decl) => {
            normalize_export_specifiers(arena, decl.specifiers).map(|specifiers| {
                Statement::ExportNamedDeclaration(ExportNamedDeclaration {
                    specifiers,
                    ..decl.clone()
                })
            })
        }
    }
}

fn normalize_import_specifiers<'arena>(
    arena: &'arena bumpalo::Bump,
    specifiers: &'arena [ImportSpecifier<'arena>],
) -> Option<&'arena [ImportSpecifier<'arena>]> {
    let mut out: Option<BumpVec<'arena, ImportSpecifier<'arena>>> = None;
    for (i, spec) in specifiers.iter().enumerate() {
        let replacement = match spec {
            ImportSpecifier::Named(named) => {
                normalize_import_named(named).map(ImportSpecifier::Named)
            }
            // A default (`import x`) or namespace (`import * as x`) specifier has no
            // imported name to normalize.
            ImportSpecifier::Default(_) | ImportSpecifier::Namespace(_) => None,
        };
        match replacement {
            None => {
                if let Some(vec) = out.as_mut() {
                    vec.push(spec.clone());
                }
            }
            Some(new) => rebuilt(&mut out, arena, specifiers, i).push(new),
        }
    }
    out.map(BumpVec::into_bump_slice)
}

/// `import { 'a-b' as loc }` → collapse `imported` onto the local binding so the
/// printer's `left.span() == right.span()` bare-form path fires (`import { loc }`).
/// The import `local` is always an identifier binding (a string imported name
/// requires an `as` binding, and `import { 'a' as 'b' }` is a parse error), so the
/// only `Literal`-bearing shape is the `imported` side.
fn normalize_import_named<'arena>(
    named: &ImportNamedSpecifier<'arena>,
) -> Option<ImportNamedSpecifier<'arena>> {
    match named.imported {
        ModuleExportName::Literal(_) => Some(ImportNamedSpecifier {
            imported: ModuleExportName::Identifier(named.local.clone()),
            ..named.clone()
        }),
        ModuleExportName::Identifier(_) => None,
    }
}

fn normalize_export_specifiers<'arena>(
    arena: &'arena bumpalo::Bump,
    specifiers: &'arena [ExportSpecifier<'arena>],
) -> Option<&'arena [ExportSpecifier<'arena>]> {
    let mut out: Option<BumpVec<'arena, ExportSpecifier<'arena>>> = None;
    for (i, spec) in specifiers.iter().enumerate() {
        match normalize_export_spec(spec) {
            None => {
                if let Some(vec) = out.as_mut() {
                    vec.push(spec.clone());
                }
            }
            Some(new) => rebuilt(&mut out, arena, specifiers, i).push(new),
        }
    }
    out.map(BumpVec::into_bump_slice)
}

/// `export { x as 'y' }` / `export { 'a-b' as notdefault }` / `export { 'a-b' as
/// 'c-d' }` → collapse `exported` onto `local` so the printer emits the bare
/// `local` binding. esrap drops the alias when EITHER name side is a string
/// literal; the surviving side is always the `local`.
fn normalize_export_spec<'arena>(
    spec: &ExportSpecifier<'arena>,
) -> Option<ExportSpecifier<'arena>> {
    export_involves_literal(spec).then(|| ExportSpecifier {
        exported: spec.local.clone(),
        ..spec.clone()
    })
}

/// Whether an export specifier collapses — either name side is a string `Literal`.
fn export_involves_literal(spec: &ExportSpecifier<'_>) -> bool {
    matches!(spec.local, ModuleExportName::Literal(_))
        || matches!(spec.exported, ModuleExportName::Literal(_))
}

/// Refuse when a KEPT module-body comment sits in a collapsing specifier's skipped
/// as-gap.
///
/// The span collapse makes the specifier printer skip the gap between the surviving
/// side and the dropped side (its `left.span() != right.span()` test now fails), so
/// a comment there — which esrap KEEPS — would be dropped. Only `module_comments`
/// (the F1 keep set) matter: a comment neither side emits is dropped by both the
/// oracle and tsv (parity), so it must NOT refuse. Empty comment set ⇒ nothing to
/// guard. Rides the exhaustive [`specifier_bearing`] classifier.
fn refuse_dropped_specifier_alias_comment(
    stmts: &[Statement<'_>],
    module_comments: &[Comment],
) -> Result<(), CompileError> {
    if module_comments.is_empty() {
        return Ok(());
    }
    for stmt in stmts {
        match specifier_bearing(stmt) {
            Some(SpecifierBearing::Import(decl)) => {
                for spec in decl.specifiers {
                    // A string `imported` name is the only collapsing import shape
                    // (a string import name always has an `as` binding). The printer
                    // then skips the `[imported.end, local.start)` gap.
                    if let ImportSpecifier::Named(named) = spec
                        && matches!(named.imported, ModuleExportName::Literal(_))
                    {
                        refuse_if_comment_in_gap(
                            named.imported.span().end,
                            named.local.span.start,
                            module_comments,
                        )?;
                    }
                }
            }
            Some(SpecifierBearing::Export(decl)) => {
                for spec in decl.specifiers {
                    // A specifier collapses when EITHER name side is a string; the
                    // surviving side is always `local`, so the printer skips the
                    // `[local.end, exported.start)` gap.
                    if export_involves_literal(spec) {
                        refuse_if_comment_in_gap(
                            spec.local.span().end,
                            spec.exported.span().start,
                            module_comments,
                        )?;
                    }
                }
            }
            None => {}
        }
    }
    Ok(())
}

/// Refuse if any comment is fully contained in the printer's skipped as-gap
/// `[gap_start, gap_end)` — mirroring the specifier printer's own scan
/// (`has_comments_to_emit_between` / `comments_in_source_range`: `start <=
/// c.span.start && c.span.end <= end`). When the names carry no alias
/// (`export { 'a-b' } from`, local == exported) the range is inverted, so nothing
/// matches and no false refusal fires.
fn refuse_if_comment_in_gap(
    gap_start: u32,
    gap_end: u32,
    comments: &[Comment],
) -> Result<(), CompileError> {
    let dropped = comments
        .iter()
        .any(|c| gap_start <= c.span.start && c.span.end <= gap_end);
    if dropped {
        return Err(unsupported(Refusal::CommentInDroppedSpecifierAlias));
    }
    Ok(())
}
