//! TypeScript type erasure — the compiler's `remove_typescript_nodes`.
//!
//! A tree→tree pre-pass over a `tsv_ts` `Program`'s statements producing a
//! **type-free** statement list the rest of the pipeline consumes unchanged.
//! The source is never modified, and neither the parser nor the printer gains a
//! mode: erasure is an AST→AST transformation, so it lives in the transform
//! slot — which is also where the oracle puts it (`remove_typescript_nodes`,
//! run over `fragment` + `instance` + `module` in phase 1, before
//! `analyze_component`).
//!
//! **Structural sharing.** Every entry point returns `Option<T>`: `None` means
//! *unchanged*, so the caller reuses the borrow and nothing is allocated. A
//! subtree with no TypeScript beneath it is never rebuilt. Clones are shallow —
//! children are `&'arena T` / `&'arena [T]`, so a rebuilt node copies pointers,
//! never a subtree.
//!
//! **Exhaustive by construction.** The `Statement` and `Expression` matches have
//! no catch-all arm. A new AST variant fails compilation here instead of
//! silently passing TypeScript through — that exhaustiveness *is* the safety
//! argument, because `compile`'s output-reparse self-validation cannot catch a
//! missed erase (tsv's parser is TypeScript-permissive, so a surviving
//! annotation still parses, flows through the pipeline, and prints verbatim).
//! [`Erased::changed`] is the second half of that guarantee: re-running the
//! eraser over the *finished* program must report no change.
//!
//! **Refuse, don't erase.** Constructs with runtime semantics a type-erasure
//! walk would silently delete — and the ones the oracle mis-compiles — are
//! [`Refusal`]s, never guessed output. See [`Eraser::statement`].
//!
//! **Comments.** Erasure deletes source regions, and the oracle's surviving-comment
//! placement is emergent (a stale-span artifact of its printer's flush points),
//! not a rule tsv can port. So every erased region is recorded in
//! [`Erased::regions`] and a comment intersecting one refuses. The refusal
//! **window** is wider than the erased span on both sides:
//!
//! - **forward**, to the start of the next surviving token — so a comment past an
//!   erased annotation (`let x: Foo /* c */ = v`, which the oracle re-anchors onto
//!   the initializer) is caught ([`next_token_pos`]);
//! - **backward**, to the end of the previous surviving token, but *only* for a
//!   region detached from it — a `return_type` after `)`, an `implements` clause, a
//!   `<T>` list ([`prev_token_end`], [`Eraser::drop_region_from`]). A whole-statement
//!   drop deliberately does **not** reach backward: a JSDoc above an erased
//!   `interface` survives onto the next statement, exactly where the oracle puts it.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;
use tsv_lang::source_scan::{TriviaProfile, skip_trivia};
use tsv_ts::ast::internal::{
    ArrayPattern, ArrowFunctionBody, ArrowFunctionExpression, AssignmentPattern, BlockStatement,
    CatchClause, ClassBody, ClassDeclaration, ClassExpression, ClassMember, EmptyStatement,
    ExportDefaultValue, ExportKind, Expression, ForInOfLeft, ForInit, FunctionDeclaration,
    FunctionExpression, Identifier, ImportKind, ImportSpecifier, MethodDefinition, MethodKind,
    ObjectPattern, ObjectPatternProperty, ObjectProperty, Property, PropertyDefinition,
    PropertyModifier, RestElement, Statement, StaticBlock, SwitchCase, TSModuleDeclarationBody,
    TemplateLiteral, VariableDeclaration, VariableDeclarator,
};

use crate::CompileError;
use crate::refusal::Refusal;

/// The product of erasing a statement list.
pub(crate) struct Erased<'arena> {
    /// The type-free statements — the input slice itself when nothing changed.
    pub(crate) body: &'arena [Statement<'arena>],
    /// Every erased source region, already extended to its comment-refusal
    /// window (see [`Eraser::drop_region`]). In walk order.
    pub(crate) regions: Vec<Span>,
    /// Whether the tree was rebuilt at all. `false` proves the input carried no
    /// TypeScript-only node **and** no `JsdocCast` — the property the self-check
    /// asserts of the finished program.
    pub(crate) changed: bool,
    /// Whether TypeScript-only *syntax* was erased. Distinct from
    /// [`Self::changed`]: unwrapping a `JsdocCast` (`/** @type {T} */ (x)`) is a
    /// compile-path normalization of **valid JavaScript**, so it must not make a
    /// `lang`-less script look like TypeScript.
    pub(crate) typescript: bool,
}

/// Erase every TypeScript-only construct from a statement list.
pub(crate) fn erase_statements<'arena>(
    arena: &'arena bumpalo::Bump,
    source: &str,
    stmts: &'arena [Statement<'arena>],
) -> Result<Erased<'arena>, CompileError> {
    let mut eraser = Eraser::new(arena, source);
    let erased = eraser.statements(stmts)?;
    debug_assert!(
        !eraser.typescript || !eraser.regions.is_empty(),
        "every TypeScript erasure must record its region (the refusal window depends on it)"
    );
    Ok(Erased {
        changed: erased.is_some(),
        typescript: eraser.typescript,
        body: erased.unwrap_or(stmts),
        regions: eraser.regions,
    })
}

/// Erase every TypeScript-only construct from a single expression — the
/// per-expression entry point for the Svelte template's borrow points (`{expr}`
/// tags, attribute values, block tests, `{@const}`/`{#each}`/`{#snippet}`
/// patterns), where the erasure applies at the borrow and the Svelte AST itself
/// is never rebuilt.
///
/// Returns the erased expression (`None` = no TypeScript, reuse the borrow) and
/// the erased source regions (feed them through the same comment-refusal window
/// as the script's).
// TODO: wire into the template borrow points (`wrap_value_expr`'s call sites,
// the four pattern positions, `SnippetBlock.type_parameters`). Until then the
// transform refuses TypeScript in a template expression.
#[allow(dead_code)]
pub(crate) fn erase_expression<'arena>(
    arena: &'arena bumpalo::Bump,
    source: &str,
    expr: &Expression<'arena>,
) -> Result<(Option<Expression<'arena>>, Vec<Span>), CompileError> {
    let mut eraser = Eraser::new(arena, source);
    let erased = eraser.expr(expr)?;
    Ok((erased, eraser.regions))
}

/// Position of the next surviving token at or after `from`: skips whitespace and
/// both comment forms via the trivia-aware cursor (never a raw `find`).
///
/// This is the **forward** half of an erased region's comment-refusal window: a
/// comment glued to the region's tail (`let x: Foo /* c */ = v`, which the
/// oracle re-anchors onto the initializer) sits inside it. Public so the
/// `erase_comment_census` diagnostic measures the same rule the compiler
/// enforces.
#[must_use]
pub fn next_token_pos(source: &str, from: u32) -> u32 {
    let bytes = source.as_bytes();
    let end = bytes.len();
    let mut pos = (from as usize).min(end);
    loop {
        while pos < end && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        match skip_trivia(bytes, pos, end, TriviaProfile::COMMENTS) {
            Some(next) if next > pos => pos = next,
            _ => return pos as u32,
        }
    }
}

/// End of the last surviving token strictly before `to`, scanning forward from
/// `anchor` (a position known to be outside trivia) with the trivia-aware
/// cursor. A string literal counts as a token; a comment does not.
///
/// This is the **backward** half of the window, and it is why the window can be
/// anchored on the AST rather than on a backward byte scan. An erased region
/// whose start is not glued to its preceding token — a `return_type` (preceded
/// by `)`), an `implements` clause, a `<T>` parameter/argument list — would
/// otherwise leak the comment sitting immediately before it: the printer never
/// queries that byte range through the erased node (it is gone), but the
/// *enclosing* node's gap window still spans it, so the comment prints — in the
/// `implements` case, **twice**.
fn prev_token_end(source: &str, anchor: u32, to: u32) -> u32 {
    let bytes = source.as_bytes();
    let end = (to as usize).min(bytes.len());
    let mut pos = (anchor as usize).min(end);
    let mut last = pos as u32;
    while pos < end {
        // A comment is trivia (skip, don't count); a string literal is a token.
        let is_comment = bytes[pos] == b'/';
        if let Some(next) = skip_trivia(bytes, pos, end, TriviaProfile::JS)
            && next > pos
        {
            if !is_comment {
                last = u32::try_from(next).unwrap_or(to).min(to);
            }
            pos = next;
            continue;
        }
        if !bytes[pos].is_ascii_whitespace() {
            last = u32::try_from(pos + 1).unwrap_or(to);
        }
        pos += 1;
    }
    last
}

fn unsupported<T>(reason: Refusal) -> Result<T, CompileError> {
    Err(CompileError::Unsupported(reason))
}

/// The end of a binding's erased TypeScript tail (`?` / `: T`).
///
/// The node's own span is usually tail-anchored over the annotation — but not
/// always: a Svelte **block pattern** (`{#each x as y: T}`, `{@const a: T = v}`)
/// is parsed by `tsv_svelte`, which leaves the binding's span on the bare name
/// and hangs the annotation off it as a sibling. Taking the max covers both
/// span models, so the erased region is never empty (an empty one would record
/// no refusal window at all).
fn tail_end(
    span: Span,
    type_annotation: Option<&tsv_ts::ast::internal::TSTypeAnnotation<'_>>,
) -> u32 {
    span.end
        .max(type_annotation.map_or(0, |annotation| annotation.span.end))
}

/// The rebuilt-list accumulator, materialized lazily: until the first change is
/// seen the output stays `None` and the caller keeps the original borrow; on the
/// first change the untouched prefix (`items[..index]`) is copied in once.
fn rebuilt_list<'out, 'arena, T: Clone>(
    out: &'out mut Option<BumpVec<'arena, T>>,
    arena: &'arena bumpalo::Bump,
    items: &[T],
    index: usize,
) -> &'out mut BumpVec<'arena, T> {
    out.get_or_insert_with(|| {
        let mut vec = BumpVec::with_capacity_in(items.len(), arena);
        vec.extend_from_slice(&items[..index]);
        vec
    })
}

/// A statement's erasure outcome.
enum StmtOut<'arena> {
    /// No TypeScript below — reuse the borrow.
    Keep,
    /// Rebuilt without its TypeScript.
    Replace(Statement<'arena>),
    /// TypeScript-only — the statement disappears.
    Drop,
}

/// A class member's erasure outcome (same shape as [`StmtOut`]).
enum MemberOut<'arena> {
    Keep,
    Replace(ClassMember<'arena>),
    Drop,
}

/// The erasable parts of a class header, shared by `ClassDeclaration` and
/// `ClassExpression` (which differ only in whether `declare` exists).
struct ClassHead<'a, 'arena> {
    span: Span,
    r#abstract: bool,
    /// End of the class name, when it has one — the backward bound for a
    /// name-less `implements` clause.
    id_end: Option<u32>,
    type_parameters: Option<Span>,
    /// End of the `extends` expression, when there is one.
    super_class_end: Option<u32>,
    super_type_parameters: Option<Span>,
    implements: &'a [tsv_ts::ast::internal::TSInterfaceHeritage<'arena>],
}

struct Eraser<'arena, 'src> {
    arena: &'arena bumpalo::Bump,
    source: &'src str,
    /// Erased regions, already extended to their comment-refusal windows.
    regions: Vec<Span>,
    /// Set by every TypeScript-only erasure (never by the `JsdocCast` unwrap).
    typescript: bool,
    /// Whether the parameter list currently being erased is a **constructor's**.
    /// The oracle rejects a parameter property only there, and only when it
    /// carries `readonly`/an accessibility modifier — see
    /// [`Eraser::parameter_property`].
    constructor_params: bool,
}

/// Rebuild a slice through a per-item eraser method, sharing the original when
/// no item changed. The prefix of unchanged items is copied only once a change
/// is seen (shallow clones — pointers, never subtrees).
macro_rules! map_slice {
    ($self:ident, $items:expr, $method:ident) => {{
        let items = $items;
        let arena = $self.arena;
        let mut out: Option<BumpVec<'arena, _>> = None;
        for (i, item) in items.iter().enumerate() {
            match $self.$method(item)? {
                None => {
                    if let Some(vec) = out.as_mut() {
                        vec.push(item.clone());
                    }
                }
                Some(new) => rebuilt_list(&mut out, arena, items, i).push(new),
            }
        }
        out.map(BumpVec::into_bump_slice)
    }};
}

impl<'arena, 'src> Eraser<'arena, 'src> {
    fn new(arena: &'arena bumpalo::Bump, source: &'src str) -> Self {
        Self {
            arena,
            source,
            regions: Vec::new(),
            typescript: false,
            constructor_params: false,
        }
    }
}

impl<'arena> Eraser<'arena, '_> {
    /// Record an erased TypeScript region, extending it forward to the next
    /// surviving token — the comment-refusal window. Use for a region that
    /// **starts at** its preceding surviving token (a whole statement, an
    /// identifier's `: T` tail, an `as T` tail): the comment before it belongs
    /// to the enclosing context and legitimately survives (a JSDoc above an
    /// erased `interface` lands on the next statement, exactly as the oracle
    /// places it).
    fn drop_region(&mut self, span: Span) {
        self.typescript = true;
        self.push_window(span.start, span.end);
    }

    /// Record an erased TypeScript region whose start is **detached** from the
    /// preceding token (a `return_type` after `)`, an `implements` clause, a
    /// `<T>` list), extending the window backward to that token's end as well as
    /// forward. `anchor` is any position before the region and outside trivia —
    /// the enclosing node's start serves.
    fn drop_region_from(&mut self, anchor: u32, span: Span) {
        self.typescript = true;
        let start = prev_token_end(self.source, anchor, span.start);
        self.push_window(start.min(span.start), span.end);
    }

    fn push_window(&mut self, start: u32, end: u32) {
        if end > start {
            let window_end = next_token_pos(self.source, end);
            self.regions.push(Span::new(start, window_end.max(end)));
        }
    }

    // ── Statements ─────────────────────────────────────────────────────────

    fn statements(
        &mut self,
        stmts: &'arena [Statement<'arena>],
    ) -> Result<Option<&'arena [Statement<'arena>]>, CompileError> {
        let arena = self.arena;
        let mut out: Option<BumpVec<'arena, Statement<'arena>>> = None;
        for (i, stmt) in stmts.iter().enumerate() {
            match self.statement(stmt)? {
                StmtOut::Keep => {
                    if let Some(vec) = out.as_mut() {
                        vec.push(stmt.clone());
                    }
                }
                StmtOut::Replace(new) => rebuilt_list(&mut out, arena, stmts, i).push(new),
                StmtOut::Drop => {
                    rebuilt_list(&mut out, arena, stmts, i);
                }
            }
        }
        Ok(out.map(BumpVec::into_bump_slice))
    }

    /// A statement in a single-statement position (an `if` branch, a loop body).
    /// A dropped statement becomes `;` — no such position can legally hold a
    /// TypeScript-only declaration, so this is unreachable in valid input, but
    /// it keeps the transformation total.
    fn statement_ref(
        &mut self,
        stmt: &'arena Statement<'arena>,
    ) -> Result<Option<&'arena Statement<'arena>>, CompileError> {
        Ok(match self.statement(stmt)? {
            StmtOut::Keep => None,
            StmtOut::Replace(new) => Some(self.arena.alloc(new)),
            StmtOut::Drop => Some(self.arena.alloc(Statement::EmptyStatement(EmptyStatement {
                span: stmt.span(),
            }))),
        })
    }

    /// The statement inventory. Exhaustive on purpose — no catch-all arm.
    ///
    /// Mirrors the oracle's `remove_typescript_nodes.js` visitor table, probed
    /// against `svelte.compile()` shape by shape:
    ///
    /// - **DROP**: `type`/`interface` declarations, `TSDeclareFunction` (a
    ///   `declare function` or an overload signature), a `declare`d
    ///   variable/class, a type-only import/export (whole declaration, or a
    ///   named list that filters to empty), and a type-only `namespace`.
    /// - **REFUSE**: value `enum` (**including `declare enum`** — the oracle's
    ///   visitor has no `declare` carve-out), a `namespace` with any value
    ///   member, `import x = require(…)`, `export = …`, `export as namespace …`.
    ///   The first two are `typescript_invalid_feature` hard errors in the
    ///   oracle; the last three the oracle *mis-compiles* (no visitor case, so
    ///   they survive into the component function as invalid JS) — tsv refuses
    ///   rather than reproduce that.
    fn statement(&mut self, stmt: &Statement<'arena>) -> Result<StmtOut<'arena>, CompileError> {
        Ok(match stmt {
            // ── TypeScript-only statements: dropped whole ──────────────────
            Statement::TSTypeAliasDeclaration(decl) => {
                self.drop_region(decl.span);
                StmtOut::Drop
            }
            Statement::TSInterfaceDeclaration(decl) => {
                self.drop_region(decl.span);
                StmtOut::Drop
            }
            Statement::TSDeclareFunction(decl) => {
                self.drop_region(decl.span);
                StmtOut::Drop
            }

            // ── Refuse-don't-erase: runtime semantics, or an oracle bug ────
            Statement::TSEnumDeclaration(_) => return unsupported(Refusal::TsEnum),
            Statement::TSImportEqualsDeclaration(_) => return unsupported(Refusal::TsImportEquals),
            Statement::TSExportAssignment(_) => return unsupported(Refusal::TsExportAssignment),
            Statement::TSNamespaceExportDeclaration(_) => {
                return unsupported(Refusal::TsNamespaceExport);
            }

            // A `namespace`/`module` drops only when its whole body erases away;
            // any surviving member means it lowers to an IIFE at runtime, which
            // the oracle rejects (`namespaces with non-type nodes`) — with no
            // `declare` carve-out either way.
            Statement::TSModuleDeclaration(decl) => {
                let mark = self.regions.len();
                let type_only = self.module_body_is_type_only(decl.body.as_ref())?;
                // The whole declaration's span subsumes whatever its body
                // recorded, so the inner regions collapse into this one.
                self.regions.truncate(mark);
                if !type_only {
                    return unsupported(Refusal::TsNamespaceWithValue);
                }
                self.drop_region(decl.span);
                StmtOut::Drop
            }

            // ── Ordinary statements ────────────────────────────────────────
            Statement::VariableDeclaration(decl) => {
                // `declare const x: T;` is ambient — dropped whole.
                if decl.declare {
                    self.drop_region(decl.span);
                    return Ok(StmtOut::Drop);
                }
                match self.variable_declaration(decl)? {
                    None => StmtOut::Keep,
                    Some(new) => StmtOut::Replace(Statement::VariableDeclaration(new)),
                }
            }
            Statement::ExpressionStatement(stmt) => match self.expr(&stmt.expression)? {
                None => StmtOut::Keep,
                Some(expression) => StmtOut::Replace(Statement::ExpressionStatement(
                    tsv_ts::ast::internal::ExpressionStatement {
                        expression,
                        ..stmt.clone()
                    },
                )),
            },
            Statement::ReturnStatement(stmt) => match stmt.argument.as_ref() {
                Some(argument) => match self.expr(argument)? {
                    None => StmtOut::Keep,
                    Some(argument) => StmtOut::Replace(Statement::ReturnStatement(
                        tsv_ts::ast::internal::ReturnStatement {
                            argument: Some(argument),
                            ..stmt.clone()
                        },
                    )),
                },
                None => StmtOut::Keep,
            },
            Statement::BlockStatement(block) => match self.block(block)? {
                None => StmtOut::Keep,
                Some(new) => StmtOut::Replace(Statement::BlockStatement(new)),
            },
            Statement::FunctionDeclaration(decl) => match self.function_declaration(decl)? {
                None => StmtOut::Keep,
                Some(new) => StmtOut::Replace(Statement::FunctionDeclaration(new)),
            },
            Statement::ClassDeclaration(decl) => {
                // `declare class C {}` is ambient — dropped whole.
                if decl.declare {
                    self.drop_region(decl.span);
                    return Ok(StmtOut::Drop);
                }
                match self.class_declaration(decl)? {
                    None => StmtOut::Keep,
                    Some(new) => StmtOut::Replace(Statement::ClassDeclaration(new)),
                }
            }
            Statement::ImportDeclaration(decl) => {
                // `import type { X } from 'm'` — the whole declaration.
                if decl.import_kind == ImportKind::Type {
                    self.drop_region(decl.span);
                    return Ok(StmtOut::Drop);
                }
                // A per-specifier `type` marker (`import { type X, Y }`). A list
                // that filters to EMPTY drops the whole statement — the oracle's
                // `if (specifiers.length === 0) return b.empty`, so `import
                // { type X } from 'm'` emits nothing (not `import {}`, not a
                // bare side-effect import). A declaration that carried no
                // specifiers at all (`import 'm'`, `import {} from 'm'`) is
                // untouched.
                if decl.specifiers.is_empty()
                    || !decl.specifiers.iter().any(|spec| {
                        matches!(spec, ImportSpecifier::Named(named)
                            if named.import_kind == ImportKind::Type)
                    })
                {
                    return Ok(StmtOut::Keep);
                }
                let mark = self.regions.len();
                let mut kept: BumpVec<'arena, ImportSpecifier<'arena>> =
                    BumpVec::with_capacity_in(decl.specifiers.len(), self.arena);
                for spec in decl.specifiers {
                    match spec {
                        ImportSpecifier::Named(named) if named.import_kind == ImportKind::Type => {
                            self.drop_region(named.span);
                        }
                        _ => kept.push(spec.clone()),
                    }
                }
                if kept.is_empty() {
                    // Subsumed by the whole-declaration region.
                    self.regions.truncate(mark);
                    self.drop_region(decl.span);
                    return Ok(StmtOut::Drop);
                }
                StmtOut::Replace(Statement::ImportDeclaration(
                    tsv_ts::ast::internal::ImportDeclaration {
                        specifiers: kept.into_bump_slice(),
                        ..decl.clone()
                    },
                ))
            }
            Statement::ExportNamedDeclaration(decl) => {
                // `export type { X }` / `export type * …` — the whole statement.
                if decl.export_kind == ExportKind::Type {
                    self.drop_region(decl.span);
                    return Ok(StmtOut::Drop);
                }
                // `export interface X {}` / `export type X = …` / `export
                // declare const x` — the exported declaration erases away, so
                // the export goes with it.
                if let Some(declaration) = decl.declaration {
                    let mark = self.regions.len();
                    return Ok(match self.statement(declaration)? {
                        StmtOut::Keep => StmtOut::Keep,
                        StmtOut::Drop => {
                            self.regions.truncate(mark);
                            self.drop_region(decl.span);
                            StmtOut::Drop
                        }
                        StmtOut::Replace(new) => {
                            StmtOut::Replace(Statement::ExportNamedDeclaration(
                                tsv_ts::ast::internal::ExportNamedDeclaration {
                                    declaration: Some(self.arena.alloc(new)),
                                    ..decl.clone()
                                },
                            ))
                        }
                    });
                }
                // `export { type A, b }` — the inline type marker.
                if !decl
                    .specifiers
                    .iter()
                    .any(|spec| spec.export_kind == ExportKind::Type)
                {
                    return Ok(StmtOut::Keep);
                }
                let mark = self.regions.len();
                let mut kept: BumpVec<'arena, tsv_ts::ast::internal::ExportSpecifier<'arena>> =
                    BumpVec::with_capacity_in(decl.specifiers.len(), self.arena);
                for spec in decl.specifiers {
                    if spec.export_kind == ExportKind::Type {
                        self.drop_region(spec.span);
                    } else {
                        kept.push(spec.clone());
                    }
                }
                if kept.is_empty() {
                    self.regions.truncate(mark);
                    self.drop_region(decl.span);
                    return Ok(StmtOut::Drop);
                }
                StmtOut::Replace(Statement::ExportNamedDeclaration(
                    tsv_ts::ast::internal::ExportNamedDeclaration {
                        specifiers: kept.into_bump_slice(),
                        ..decl.clone()
                    },
                ))
            }
            Statement::ExportDefaultDeclaration(decl) => {
                let declaration = match &decl.declaration {
                    ExportDefaultValue::Expression(expr) => {
                        self.expr(expr)?.map(ExportDefaultValue::Expression)
                    }
                    ExportDefaultValue::FunctionDeclaration(func) => self
                        .function_declaration(func)?
                        .map(ExportDefaultValue::FunctionDeclaration),
                    ExportDefaultValue::ClassDeclaration(class) => self
                        .class_declaration(class)?
                        .map(ExportDefaultValue::ClassDeclaration),
                    // `export default function f(): T;` / `export default
                    // interface F {}` — the oracle keeps both (its visitor
                    // returns the node without descending), emitting a bodiless
                    // signature / an interface. Both are `export default`, which
                    // runes mode rejects outright, so the instance-script export
                    // refusal covers them; nothing to erase.
                    ExportDefaultValue::TSDeclareFunction(_)
                    | ExportDefaultValue::TSInterfaceDeclaration(_) => None,
                };
                match declaration {
                    None => StmtOut::Keep,
                    Some(declaration) => StmtOut::Replace(Statement::ExportDefaultDeclaration(
                        tsv_ts::ast::internal::ExportDefaultDeclaration {
                            declaration,
                            ..decl.clone()
                        },
                    )),
                }
            }
            Statement::ExportAllDeclaration(decl) => {
                if decl.export_kind == ExportKind::Type {
                    self.drop_region(decl.span);
                    return Ok(StmtOut::Drop);
                }
                StmtOut::Keep
            }

            // ── Control flow ───────────────────────────────────────────────
            Statement::IfStatement(stmt) => {
                let test = self.expr(&stmt.test)?;
                let consequent = self.statement_ref(stmt.consequent)?;
                let alternate = match stmt.alternate {
                    Some(alt) => self.statement_ref(alt)?.map(Some),
                    None => None,
                };
                if test.is_none() && consequent.is_none() && alternate.is_none() {
                    return Ok(StmtOut::Keep);
                }
                StmtOut::Replace(Statement::IfStatement(tsv_ts::ast::internal::IfStatement {
                    test: test.unwrap_or_else(|| stmt.test.clone()),
                    consequent: consequent.unwrap_or(stmt.consequent),
                    alternate: alternate.unwrap_or(stmt.alternate),
                    span: stmt.span,
                }))
            }
            Statement::ForStatement(stmt) => {
                let init = match &stmt.init {
                    Some(init) => self.for_init(init)?.map(Some),
                    None => None,
                };
                let test = match &stmt.test {
                    Some(test) => self.expr(test)?.map(Some),
                    None => None,
                };
                let update = match &stmt.update {
                    Some(update) => self.expr(update)?.map(Some),
                    None => None,
                };
                let body = self.statement_ref(stmt.body)?;
                if init.is_none() && test.is_none() && update.is_none() && body.is_none() {
                    return Ok(StmtOut::Keep);
                }
                StmtOut::Replace(Statement::ForStatement(
                    tsv_ts::ast::internal::ForStatement {
                        init: init.unwrap_or_else(|| stmt.init.clone()),
                        test: test.unwrap_or_else(|| stmt.test.clone()),
                        update: update.unwrap_or_else(|| stmt.update.clone()),
                        body: body.unwrap_or(stmt.body),
                        span: stmt.span,
                    },
                ))
            }
            Statement::ForInStatement(stmt) => {
                let left = self.for_in_of_left(&stmt.left)?;
                let right = self.expr(&stmt.right)?;
                let body = self.statement_ref(stmt.body)?;
                if left.is_none() && right.is_none() && body.is_none() {
                    return Ok(StmtOut::Keep);
                }
                StmtOut::Replace(Statement::ForInStatement(
                    tsv_ts::ast::internal::ForInStatement {
                        left: left.unwrap_or_else(|| stmt.left.clone()),
                        right: right.unwrap_or_else(|| stmt.right.clone()),
                        body: body.unwrap_or(stmt.body),
                        span: stmt.span,
                    },
                ))
            }
            Statement::ForOfStatement(stmt) => {
                let left = self.for_in_of_left(&stmt.left)?;
                let right = self.expr(&stmt.right)?;
                let body = self.statement_ref(stmt.body)?;
                if left.is_none() && right.is_none() && body.is_none() {
                    return Ok(StmtOut::Keep);
                }
                StmtOut::Replace(Statement::ForOfStatement(
                    tsv_ts::ast::internal::ForOfStatement {
                        left: left.unwrap_or_else(|| stmt.left.clone()),
                        right: right.unwrap_or_else(|| stmt.right.clone()),
                        body: body.unwrap_or(stmt.body),
                        ..stmt.clone()
                    },
                ))
            }
            Statement::WhileStatement(stmt) => {
                let test = self.expr(&stmt.test)?;
                let body = self.statement_ref(stmt.body)?;
                if test.is_none() && body.is_none() {
                    return Ok(StmtOut::Keep);
                }
                StmtOut::Replace(Statement::WhileStatement(
                    tsv_ts::ast::internal::WhileStatement {
                        test: test.unwrap_or_else(|| stmt.test.clone()),
                        body: body.unwrap_or(stmt.body),
                        span: stmt.span,
                    },
                ))
            }
            Statement::DoWhileStatement(stmt) => {
                let body = self.statement_ref(stmt.body)?;
                let test = self.expr(&stmt.test)?;
                if test.is_none() && body.is_none() {
                    return Ok(StmtOut::Keep);
                }
                StmtOut::Replace(Statement::DoWhileStatement(
                    tsv_ts::ast::internal::DoWhileStatement {
                        body: body.unwrap_or(stmt.body),
                        test: test.unwrap_or_else(|| stmt.test.clone()),
                        span: stmt.span,
                    },
                ))
            }
            Statement::SwitchStatement(stmt) => {
                let discriminant = self.expr(&stmt.discriminant)?;
                let cases = map_slice!(self, stmt.cases, switch_case);
                if discriminant.is_none() && cases.is_none() {
                    return Ok(StmtOut::Keep);
                }
                StmtOut::Replace(Statement::SwitchStatement(
                    tsv_ts::ast::internal::SwitchStatement {
                        discriminant: discriminant.unwrap_or_else(|| stmt.discriminant.clone()),
                        cases: cases.unwrap_or(stmt.cases),
                        span: stmt.span,
                    },
                ))
            }
            Statement::TryStatement(stmt) => {
                let block = self.block(&stmt.block)?;
                let handler = match &stmt.handler {
                    Some(handler) => self.catch_clause(handler)?.map(Some),
                    None => None,
                };
                let finalizer = match &stmt.finalizer {
                    Some(finalizer) => self.block(finalizer)?.map(Some),
                    None => None,
                };
                if block.is_none() && handler.is_none() && finalizer.is_none() {
                    return Ok(StmtOut::Keep);
                }
                StmtOut::Replace(Statement::TryStatement(
                    tsv_ts::ast::internal::TryStatement {
                        block: block.unwrap_or_else(|| stmt.block.clone()),
                        handler: handler.unwrap_or_else(|| stmt.handler.clone()),
                        finalizer: finalizer.unwrap_or_else(|| stmt.finalizer.clone()),
                        span: stmt.span,
                    },
                ))
            }
            Statement::ThrowStatement(stmt) => match self.expr(&stmt.argument)? {
                None => StmtOut::Keep,
                Some(argument) => StmtOut::Replace(Statement::ThrowStatement(
                    tsv_ts::ast::internal::ThrowStatement {
                        argument,
                        span: stmt.span,
                    },
                )),
            },
            Statement::LabeledStatement(stmt) => match self.statement_ref(stmt.body)? {
                None => StmtOut::Keep,
                Some(body) => StmtOut::Replace(Statement::LabeledStatement(
                    tsv_ts::ast::internal::LabeledStatement {
                        body,
                        ..stmt.clone()
                    },
                )),
            },

            // ── No TypeScript-bearing children ─────────────────────────────
            Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_) => StmtOut::Keep,
        })
    }

    /// Whether a `namespace`/`module` body erases away completely — the oracle's
    /// fork: all-type → drop, any surviving member → `typescript_invalid_feature`
    /// (a value namespace lowers to an IIFE, which no erasure can express).
    fn module_body_is_type_only(
        &mut self,
        body: Option<&TSModuleDeclarationBody<'arena>>,
    ) -> Result<bool, CompileError> {
        Ok(match body {
            // `declare module 'name';` — a shorthand ambient module, no body.
            None => true,
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                match self.statements(block.body)? {
                    // Nothing erased: every member survives, so an empty block is
                    // the only type-only shape.
                    None => block.body.is_empty(),
                    Some(erased) => erased.is_empty(),
                }
            }
            // `namespace A.B { … }` — the dotted form nests a module declaration
            // where the oracle's visitor assumes a block, then calls
            // `node.body.body.map(…)` on it and THROWS. Not a compilable shape at
            // any body content, so refuse rather than guess (same class as
            // `import =` / `export as namespace` / a class index signature).
            Some(TSModuleDeclarationBody::TSModuleDeclaration(_)) => {
                return unsupported(Refusal::TsDottedNamespace);
            }
        })
    }

    fn for_init(
        &mut self,
        init: &ForInit<'arena>,
    ) -> Result<Option<ForInit<'arena>>, CompileError> {
        Ok(match init {
            ForInit::VariableDeclaration(decl) => self
                .variable_declaration(decl)?
                .map(ForInit::VariableDeclaration),
            ForInit::Expression(expr) => self.expr(expr)?.map(ForInit::Expression),
        })
    }

    fn for_in_of_left(
        &mut self,
        left: &ForInOfLeft<'arena>,
    ) -> Result<Option<ForInOfLeft<'arena>>, CompileError> {
        Ok(match left {
            ForInOfLeft::VariableDeclaration(decl) => self
                .variable_declaration(decl)?
                .map(ForInOfLeft::VariableDeclaration),
            ForInOfLeft::Pattern(pattern) => self.expr(pattern)?.map(ForInOfLeft::Pattern),
        })
    }

    fn switch_case(
        &mut self,
        case: &SwitchCase<'arena>,
    ) -> Result<Option<SwitchCase<'arena>>, CompileError> {
        let test = match &case.test {
            Some(test) => self.expr(test)?.map(Some),
            None => None,
        };
        let consequent = self.statements(case.consequent)?;
        if test.is_none() && consequent.is_none() {
            return Ok(None);
        }
        Ok(Some(SwitchCase {
            test: test.unwrap_or_else(|| case.test.clone()),
            consequent: consequent.unwrap_or(case.consequent),
            span: case.span,
        }))
    }

    fn catch_clause(
        &mut self,
        clause: &CatchClause<'arena>,
    ) -> Result<Option<CatchClause<'arena>>, CompileError> {
        let param = match &clause.param {
            Some(param) => self.expr(param)?.map(Some),
            None => None,
        };
        let body = self.block(&clause.body)?;
        if param.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(CatchClause {
            param: param.unwrap_or_else(|| clause.param.clone()),
            body: body.unwrap_or_else(|| clause.body.clone()),
            span: clause.span,
        }))
    }

    fn block(
        &mut self,
        block: &BlockStatement<'arena>,
    ) -> Result<Option<BlockStatement<'arena>>, CompileError> {
        Ok(self.statements(block.body)?.map(|body| BlockStatement {
            body,
            span: block.span,
        }))
    }

    /// Erase a variable declaration **in place** — the `declare` keyword is
    /// cleared, not dropped. The whole-statement drop for an ambient `declare
    /// const x: T;` belongs to the statement arm, which is the only position
    /// where dropping is meaningful; a for-head can't hold one, but clearing the
    /// keyword here keeps the transformation total (no silent hole in the
    /// "`changed == false` proves no TypeScript" invariant).
    fn variable_declaration(
        &mut self,
        decl: &VariableDeclaration<'arena>,
    ) -> Result<Option<VariableDeclaration<'arena>>, CompileError> {
        if decl.declare {
            self.drop_region(Span::new(
                decl.span.start,
                decl.span.start + "declare".len() as u32,
            ));
        }
        let declarations = map_slice!(self, decl.declarations, variable_declarator);
        if !decl.declare && declarations.is_none() {
            return Ok(None);
        }
        Ok(Some(VariableDeclaration {
            declarations: declarations.unwrap_or(decl.declarations),
            declare: false,
            ..decl.clone()
        }))
    }

    fn variable_declarator(
        &mut self,
        declarator: &VariableDeclarator<'arena>,
    ) -> Result<Option<VariableDeclarator<'arena>>, CompileError> {
        let id = self.expr(&declarator.id)?;
        let init = match &declarator.init {
            Some(init) => self.expr(init)?.map(Some),
            None => None,
        };
        // `let x!: T` — the definite-assignment `!` lives on the declarator but
        // sits inside the id's span tail, so the annotation's own erased region
        // already covers it. Without an annotation (`let x!;`) the id erases to
        // nothing, so record the `!` here.
        if declarator.definite
            && id.is_none()
            && let Expression::Identifier(ident) = &declarator.id
        {
            self.drop_region(Span::new(ident.name_span().end, ident.span.end));
        }
        if id.is_none() && init.is_none() && !declarator.definite {
            return Ok(None);
        }
        Ok(Some(VariableDeclarator {
            id: id.unwrap_or_else(|| declarator.id.clone()),
            init: init.unwrap_or_else(|| declarator.init.clone()),
            definite: false,
            span: declarator.span,
        }))
    }

    // ── Functions and classes ──────────────────────────────────────────────

    /// The parameter list, with the TypeScript `this` pseudo-parameter dropped
    /// (`function f(this: T, x)` → `function f(x)`) — the oracle's
    /// `remove_this_param`, which applies to function declarations and function
    /// expressions only, never to arrows.
    ///
    /// `anchor` is the `(` offset (the backward window bound for the dropped
    /// `this`); `constructor` says whether a parameter property here is the
    /// oracle's rejected shape (see [`Self::parameter_property`]).
    fn params(
        &mut self,
        params: &'arena [Expression<'arena>],
        anchor: u32,
        drop_this: bool,
        constructor: bool,
    ) -> Result<Option<&'arena [Expression<'arena>]>, CompileError> {
        // Save/restore: a default-value arrow inside a constructor parameter list
        // erases its own (non-constructor) params without clearing the flag for
        // the sibling parameters that follow.
        let saved = std::mem::replace(&mut self.constructor_params, constructor);
        let result = self.params_inner(params, anchor, drop_this);
        self.constructor_params = saved;
        result
    }

    fn params_inner(
        &mut self,
        params: &'arena [Expression<'arena>],
        anchor: u32,
        drop_this: bool,
    ) -> Result<Option<&'arena [Expression<'arena>]>, CompileError> {
        let drop_first = drop_this && params.first().is_some_and(|p| self.is_this_param(p));
        if !drop_first {
            return Ok(map_slice!(self, params, expr));
        }
        // The dropped `this` takes its trailing comma with it, and the window
        // reaches back over the `(` so a comment before it can't leak.
        let this = params[0].span();
        let region_end = params.get(1).map_or(this.end, |next| next.span().start);
        self.drop_region_from(anchor, Span::new(this.start, region_end));
        let rest = &params[1..];
        let erased = map_slice!(self, rest, expr);
        Ok(Some(erased.unwrap_or_else(|| {
            let mut kept = BumpVec::with_capacity_in(rest.len(), self.arena);
            kept.extend_from_slice(rest);
            kept.into_bump_slice()
        })))
    }

    /// The TypeScript `this` pseudo-parameter: a bare `this` identifier binding
    /// (acorn's `isThisParam`, mirrored by `tsv_ts`'s parser).
    fn is_this_param(&self, param: &Expression<'arena>) -> bool {
        let Expression::Identifier(id) = param else {
            return false;
        };
        id.name_span().extract(self.source) == "this"
    }

    fn function_declaration(
        &mut self,
        func: &FunctionDeclaration<'arena>,
    ) -> Result<Option<FunctionDeclaration<'arena>>, CompileError> {
        if let Some(type_parameters) = &func.type_parameters {
            self.drop_region_from(func.span.start, type_parameters.span);
        }
        if let Some(return_type) = &func.return_type {
            self.drop_region_from(func.params_start, return_type.span);
        }
        let params = self.params(func.params, func.params_start, true, false)?;
        let body = self.block(&func.body)?;
        if func.type_parameters.is_none()
            && func.return_type.is_none()
            && params.is_none()
            && body.is_none()
        {
            return Ok(None);
        }
        Ok(Some(FunctionDeclaration {
            type_parameters: None,
            return_type: None,
            params: params.unwrap_or(func.params),
            body: body.unwrap_or_else(|| func.body.clone()),
            ..func.clone()
        }))
    }

    fn function_expression(
        &mut self,
        func: &FunctionExpression<'arena>,
        constructor: bool,
    ) -> Result<Option<FunctionExpression<'arena>>, CompileError> {
        if let Some(type_parameters) = &func.type_parameters {
            self.drop_region_from(func.span.start, type_parameters.span);
        }
        if let Some(return_type) = &func.return_type {
            self.drop_region_from(func.params_start, return_type.span);
        }
        let params = self.params(func.params, func.params_start, true, constructor)?;
        let body = self.block(&func.body)?;
        if func.type_parameters.is_none()
            && func.return_type.is_none()
            && params.is_none()
            && body.is_none()
        {
            return Ok(None);
        }
        Ok(Some(FunctionExpression {
            type_parameters: None,
            return_type: None,
            params: params.unwrap_or(func.params),
            body: body.unwrap_or_else(|| func.body.clone()),
            ..func.clone()
        }))
    }

    fn arrow(
        &mut self,
        arrow: &ArrowFunctionExpression<'arena>,
    ) -> Result<Option<ArrowFunctionExpression<'arena>>, CompileError> {
        let params_start = arrow.params_start.unwrap_or(arrow.span.start);
        if let Some(type_parameters) = &arrow.type_parameters {
            self.drop_region_from(arrow.span.start, type_parameters.span);
        }
        if let Some(return_type) = &arrow.return_type {
            self.drop_region_from(params_start, return_type.span);
        }
        // An arrow keeps a leading `this` binding — it is an ordinary parameter
        // name there, and the oracle's `remove_this_param` never visits arrows.
        let params = self.params(arrow.params, params_start, false, false)?;
        let body = match &arrow.body {
            ArrowFunctionBody::Expression(expr) => {
                self.expr_ref(expr)?.map(ArrowFunctionBody::Expression)
            }
            ArrowFunctionBody::BlockStatement(block) => {
                self.block(block)?.map(ArrowFunctionBody::BlockStatement)
            }
        };
        if arrow.type_parameters.is_none()
            && arrow.return_type.is_none()
            && params.is_none()
            && body.is_none()
        {
            return Ok(None);
        }
        Ok(Some(ArrowFunctionExpression {
            type_parameters: None,
            return_type: None,
            params: params.unwrap_or(arrow.params),
            body: body.unwrap_or_else(|| arrow.body.clone()),
            ..arrow.clone()
        }))
    }

    fn class_declaration(
        &mut self,
        class: &ClassDeclaration<'arena>,
    ) -> Result<Option<ClassDeclaration<'arena>>, CompileError> {
        self.refuse_decorators(class.decorators)?;
        let erased_head = self.class_head(&ClassHead {
            span: class.span,
            r#abstract: class.r#abstract,
            id_end: class.id.as_ref().map(|id| id.span.end),
            type_parameters: class.type_parameters.as_ref().map(|tp| tp.span),
            super_class_end: class.super_class.map(|s| s.span().end),
            super_type_parameters: class.super_type_parameters.as_ref().map(|tp| tp.span),
            implements: class.implements,
        });
        let super_class = match class.super_class {
            Some(expr) => self.expr_ref(expr)?.map(Some),
            None => None,
        };
        let body = self.class_body(&class.body)?;
        if !erased_head && super_class.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(ClassDeclaration {
            r#abstract: false,
            implements: &[],
            type_parameters: None,
            super_type_parameters: None,
            super_class: super_class.unwrap_or(class.super_class),
            body: body.unwrap_or_else(|| class.body.clone()),
            ..class.clone()
        }))
    }

    fn class_expression(
        &mut self,
        class: &ClassExpression<'arena>,
    ) -> Result<Option<ClassExpression<'arena>>, CompileError> {
        self.refuse_decorators(class.decorators)?;
        let erased_head = self.class_head(&ClassHead {
            span: class.span,
            r#abstract: class.r#abstract,
            id_end: class.id.as_ref().map(|id| id.span.end),
            type_parameters: class.type_parameters.as_ref().map(|tp| tp.span),
            super_class_end: class.super_class.map(|s| s.span().end),
            super_type_parameters: class.super_type_parameters.as_ref().map(|tp| tp.span),
            implements: class.implements,
        });
        let super_class = match class.super_class {
            Some(expr) => self.expr_ref(expr)?.map(Some),
            None => None,
        };
        let body = self.class_body(&class.body)?;
        if !erased_head && super_class.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(ClassExpression {
            r#abstract: false,
            implements: &[],
            type_parameters: None,
            super_type_parameters: None,
            super_class: super_class.unwrap_or(class.super_class),
            body: body.unwrap_or_else(|| class.body.clone()),
            ..class.clone()
        }))
    }

    /// Record the erased regions of a class header (`abstract`, `<T>`,
    /// `extends Base<T>`'s type arguments, `implements …`). Returns whether
    /// anything was erased.
    fn class_head(&mut self, class: &ClassHead<'_, 'arena>) -> bool {
        // `abstract` has no span of its own; it LEADS the declaration, so its
        // window must not reach backward — a JSDoc above the class survives onto
        // the emitted class, exactly as the oracle places it.
        if class.r#abstract {
            self.drop_region(Span::new(
                class.span.start,
                class.span.start + "abstract".len() as u32,
            ));
        }
        if let Some(span) = class.type_parameters {
            self.drop_region_from(class.span.start, span);
        }
        if let Some(span) = class.super_type_parameters {
            self.drop_region_from(class.span.start, span);
        }
        if let Some(last) = class.implements.last() {
            // The `implements` KEYWORD carries no span, so the clause's window
            // runs from the end of the last SURVIVING header token — the
            // superclass if there is one, else the class name. Starting it at the
            // first heritage entry instead leaves the keyword, and any comment
            // around it, outside every window: the class then prints without its
            // `implements`, but the enclosing gap windows still sweep the comment
            // — two of them do, so it emits TWICE.
            let header_end = class
                .super_class_end
                .or(class.id_end)
                .unwrap_or(class.span.start);
            self.drop_region(Span::new(header_end, last.span.end));
        }
        class.r#abstract
            || class.type_parameters.is_some()
            || class.super_type_parameters.is_some()
            || !class.implements.is_empty()
    }

    fn class_body(
        &mut self,
        body: &ClassBody<'arena>,
    ) -> Result<Option<ClassBody<'arena>>, CompileError> {
        let arena = self.arena;
        let members = body.body;
        let mut out: Option<BumpVec<'arena, ClassMember<'arena>>> = None;
        for (i, member) in members.iter().enumerate() {
            match self.class_member(member)? {
                MemberOut::Keep => {
                    if let Some(vec) = out.as_mut() {
                        vec.push(member.clone());
                    }
                }
                MemberOut::Replace(new) => rebuilt_list(&mut out, arena, members, i).push(new),
                MemberOut::Drop => {
                    rebuilt_list(&mut out, arena, members, i);
                }
            }
        }
        Ok(out.map(|members| ClassBody {
            body: members.into_bump_slice(),
            span: body.span,
        }))
    }

    fn class_member(
        &mut self,
        member: &ClassMember<'arena>,
    ) -> Result<MemberOut<'arena>, CompileError> {
        Ok(match member {
            // `[key: string]: T` — a pure type construct, but the oracle's strip
            // pass has no case for it and its transform then *crashes*. Refuse
            // rather than guess at output the oracle cannot produce.
            ClassMember::IndexSignature(_) => return unsupported(Refusal::TsIndexSignature),
            ClassMember::StaticBlock(block) => match self.statements(block.body)? {
                None => MemberOut::Keep,
                Some(body) => MemberOut::Replace(ClassMember::StaticBlock(StaticBlock {
                    body,
                    span: block.span,
                })),
            },
            ClassMember::MethodDefinition(method) => self.method_definition(method)?,
            ClassMember::PropertyDefinition(prop) => self.property_definition(prop)?,
        })
    }

    fn method_definition(
        &mut self,
        method: &MethodDefinition<'arena>,
    ) -> Result<MemberOut<'arena>, CompileError> {
        self.refuse_decorators(method.decorators)?;
        // An `abstract` member declares no runtime behavior — the oracle drops it.
        if method.r#abstract {
            self.drop_region(method.span);
            return Ok(MemberOut::Drop);
        }
        // A bodiless, non-abstract method is an overload signature (or an
        // ambient member outside a `declare` class). The oracle has no case for
        // it: the signature survives its strip pass and then collides with the
        // implementation (`duplicate_class_field`), or prints as invalid JS.
        // `is_bodyless` mirrors the wire writer's `TSDeclareMethod` predicate.
        let func = &method.value;
        if func.body.body.is_empty() && func.body.span.start == func.body.span.end {
            return unsupported(Refusal::TsOverloadSignature);
        }
        let erased_head = method.accessibility.is_some()
            || method.r#override
            || method.optional
            || func.type_parameters.is_some();
        if erased_head {
            if method.accessibility.is_some() || method.r#override {
                self.drop_region(Span::new(method.span.start, method.key.span().start));
            }
            // The `?` marker and the `<T>` clause both sit between the key and
            // the parameter list's `(`.
            if method.optional || func.type_parameters.is_some() {
                self.drop_region(Span::new(method.key.span().end, func.params_start));
            }
        }
        let key = self.expr(&method.key)?;
        let value = self.function_expression(func, method.kind == MethodKind::Constructor)?;
        if !erased_head && key.is_none() && value.is_none() {
            return Ok(MemberOut::Keep);
        }
        Ok(MemberOut::Replace(ClassMember::MethodDefinition(
            MethodDefinition {
                key: key.unwrap_or_else(|| method.key.clone()),
                value: value.unwrap_or_else(|| func.clone()),
                accessibility: None,
                r#override: false,
                r#abstract: false,
                optional: false,
                ..method.clone()
            },
        )))
    }

    fn property_definition(
        &mut self,
        prop: &PropertyDefinition<'arena>,
    ) -> Result<MemberOut<'arena>, CompileError> {
        self.refuse_decorators(prop.decorators)?;
        // `declare x: T` — an ambient field, dropped by the oracle's `ClassBody`
        // visitor.
        if prop.declare {
            self.drop_region(prop.span);
            return Ok(MemberOut::Drop);
        }
        // `abstract x: T` and `accessor x = 1` have no strip case in the oracle:
        // the first survives its walk and prints as `abstract x;` (invalid JS),
        // the second is a `typescript_invalid_feature` hard error. Refuse both.
        if prop.r#abstract {
            return unsupported(Refusal::TsAbstractProperty);
        }
        if prop.accessor {
            return unsupported(Refusal::TsAccessorField);
        }
        let erased_modifiers = prop.accessibility.is_some() || prop.readonly || prop.r#override;
        if erased_modifiers {
            self.drop_region(Span::new(prop.span.start, prop.key.span().start));
        }
        // `?`/`!` and `: T` form one contiguous tail between the key and the
        // initializer (or the member's end).
        let erased_tail = prop.modifier != PropertyModifier::None || prop.type_annotation.is_some();
        if erased_tail {
            let tail_end = prop
                .value
                .as_ref()
                .map_or(prop.span.end, |value| value.span().start);
            self.drop_region(Span::new(prop.key.span().end, tail_end));
        }
        let key = self.expr(&prop.key)?;
        let value = match &prop.value {
            Some(value) => self.expr(value)?.map(Some),
            None => None,
        };
        if !erased_modifiers && !erased_tail && key.is_none() && value.is_none() {
            return Ok(MemberOut::Keep);
        }
        Ok(MemberOut::Replace(ClassMember::PropertyDefinition(
            PropertyDefinition {
                key: key.unwrap_or_else(|| prop.key.clone()),
                value: value.unwrap_or_else(|| prop.value.clone()),
                type_annotation: None,
                accessibility: None,
                readonly: false,
                r#override: false,
                r#abstract: false,
                modifier: PropertyModifier::None,
                ..prop.clone()
            },
        )))
    }

    /// Decorators are a `typescript_invalid_feature` hard error in the oracle
    /// (`@dec` on a class, member, or parameter) — and without `lang="ts"` they
    /// are a plain-JS parse error. Never erased, never emitted.
    fn refuse_decorators(
        &self,
        decorators: Option<&'arena [tsv_ts::ast::internal::Decorator<'arena>]>,
    ) -> Result<(), CompileError> {
        if decorators.is_some_and(|list| !list.is_empty()) {
            return unsupported(Refusal::Decorator);
        }
        Ok(())
    }

    // ── Expressions ────────────────────────────────────────────────────────

    fn expr_ref(
        &mut self,
        expr: &'arena Expression<'arena>,
    ) -> Result<Option<&'arena Expression<'arena>>, CompileError> {
        Ok(self.expr(expr)?.map(|new| &*self.arena.alloc(new)))
    }

    /// The expression inventory. Exhaustive on purpose — no catch-all arm.
    ///
    /// The five TypeScript wrappers **unwrap to their inner expression**
    /// (`x as T` → `x`, `x!` → `x`, `x satisfies T` → `x`, `<T>x` → `x`,
    /// `f<T>` → `f`), and so does the internal `JsdocCast` wrapper — the oracle's
    /// AST has no such node (it parses without `preserveParens`), so
    /// `/** @type {T} */ (1)` is simply `1` with a leading comment there; keeping
    /// tsv's wrapper would print the parens the oracle drops *and* block the
    /// static fold. Patterns and binding identifiers **field-drop** their
    /// annotations and `?`/`!` markers.
    ///
    /// Parens are not a hazard: `tsv_ts` parses with `preserve_parens: false`
    /// and re-derives them from precedence, exactly as the oracle's printer
    /// does — so `(x as T).y` erases to `x.y` and `(a + b as T) * c` keeps the
    /// parens it needs.
    #[allow(clippy::too_many_lines)]
    fn expr(
        &mut self,
        expr: &Expression<'arena>,
    ) -> Result<Option<Expression<'arena>>, CompileError> {
        Ok(match expr {
            // ── The five TypeScript wrappers: unwrap to inner ──────────────
            Expression::TSAsExpression(node) => {
                self.drop_region(Span::new(node.expression.span().end, node.span.end));
                Some(
                    self.expr(node.expression)?
                        .unwrap_or_else(|| node.expression.clone()),
                )
            }
            Expression::TSSatisfiesExpression(node) => {
                self.drop_region(Span::new(node.expression.span().end, node.span.end));
                Some(
                    self.expr(node.expression)?
                        .unwrap_or_else(|| node.expression.clone()),
                )
            }
            Expression::TSNonNullExpression(node) => {
                self.drop_region(Span::new(node.expression.span().end, node.span.end));
                Some(
                    self.expr(node.expression)?
                        .unwrap_or_else(|| node.expression.clone()),
                )
            }
            Expression::TSInstantiationExpression(node) => {
                self.drop_region(Span::new(node.expression.span().end, node.span.end));
                Some(
                    self.expr(node.expression)?
                        .unwrap_or_else(|| node.expression.clone()),
                )
            }
            Expression::TSTypeAssertion(node) => {
                self.drop_region(Span::new(node.span.start, node.expression.span().start));
                Some(
                    self.expr(node.expression)?
                        .unwrap_or_else(|| node.expression.clone()),
                )
            }

            Expression::TSParameterProperty(node) => Some(self.parameter_property(node)?),

            // ── Binding identifiers ────────────────────────────────────────
            Expression::Identifier(id) => self.identifier(id)?.map(Expression::Identifier),

            // ── Patterns ───────────────────────────────────────────────────
            Expression::ObjectPattern(pattern) => {
                self.refuse_decorators(pattern.decorators)?;
                self.pattern_tail(
                    pattern.span,
                    pattern.optional,
                    pattern.type_annotation.as_ref(),
                );
                let properties = map_slice!(self, pattern.properties, object_pattern_property);
                let erased = pattern.optional || pattern.type_annotation.is_some();
                if !erased && properties.is_none() {
                    None
                } else {
                    Some(Expression::ObjectPattern(ObjectPattern {
                        properties: properties.unwrap_or(pattern.properties),
                        optional: false,
                        type_annotation: None,
                        ..pattern.clone()
                    }))
                }
            }
            Expression::ArrayPattern(pattern) => {
                self.refuse_decorators(pattern.decorators)?;
                self.pattern_tail(
                    pattern.span,
                    pattern.optional,
                    pattern.type_annotation.as_ref(),
                );
                let elements = map_slice!(self, pattern.elements, opt_expr);
                let erased = pattern.optional || pattern.type_annotation.is_some();
                if !erased && elements.is_none() {
                    None
                } else {
                    Some(Expression::ArrayPattern(ArrayPattern {
                        elements: elements.unwrap_or(pattern.elements),
                        optional: false,
                        type_annotation: None,
                        ..pattern.clone()
                    }))
                }
            }
            Expression::AssignmentPattern(pattern) => {
                self.refuse_decorators(pattern.decorators)?;
                let left = self.expr_ref(pattern.left)?;
                let right = self.expr_ref(pattern.right)?;
                if left.is_none() && right.is_none() {
                    None
                } else {
                    Some(Expression::AssignmentPattern(AssignmentPattern {
                        left: left.unwrap_or(pattern.left),
                        right: right.unwrap_or(pattern.right),
                        ..pattern.clone()
                    }))
                }
            }
            Expression::RestElement(rest) => self.rest_element(rest)?.map(Expression::RestElement),

            // ── Calls carry type arguments ─────────────────────────────────
            Expression::CallExpression(call) => {
                if let Some(type_arguments) = &call.type_arguments {
                    // `f /* c */ <T>(x)` — the `<T>` list is detached from the
                    // callee, so the window reaches back to it.
                    self.drop_region_from(call.callee.span().start, type_arguments.span);
                }
                let callee = self.expr_ref(call.callee)?;
                let arguments = map_slice!(self, call.arguments, expr);
                if call.type_arguments.is_none() && callee.is_none() && arguments.is_none() {
                    None
                } else {
                    Some(Expression::CallExpression(
                        tsv_ts::ast::internal::CallExpression {
                            callee: callee.unwrap_or(call.callee),
                            type_arguments: None,
                            arguments: arguments.unwrap_or(call.arguments),
                            ..call.clone()
                        },
                    ))
                }
            }
            Expression::NewExpression(new) => {
                if let Some(type_arguments) = &new.type_arguments {
                    self.drop_region_from(new.callee.span().start, type_arguments.span);
                }
                let callee = self.expr_ref(new.callee)?;
                let arguments = map_slice!(self, new.arguments, expr);
                if new.type_arguments.is_none() && callee.is_none() && arguments.is_none() {
                    None
                } else {
                    Some(Expression::NewExpression(
                        tsv_ts::ast::internal::NewExpression {
                            callee: callee.unwrap_or(new.callee),
                            type_arguments: None,
                            arguments: arguments.unwrap_or(new.arguments),
                            span: new.span,
                        },
                    ))
                }
            }
            Expression::TaggedTemplateExpression(tagged) => {
                if let Some(type_arguments) = &tagged.type_arguments {
                    self.drop_region_from(tagged.tag.span().start, type_arguments.span);
                }
                let tag = self.expr_ref(tagged.tag)?;
                let quasi = self.template_literal(&tagged.quasi)?;
                if tagged.type_arguments.is_none() && tag.is_none() && quasi.is_none() {
                    None
                } else {
                    Some(Expression::TaggedTemplateExpression(
                        tsv_ts::ast::internal::TaggedTemplateExpression {
                            tag: tag.unwrap_or(tagged.tag),
                            type_arguments: None,
                            quasi: quasi.unwrap_or_else(|| tagged.quasi.clone()),
                            span: tagged.span,
                        },
                    ))
                }
            }

            // ── Functions and classes ──────────────────────────────────────
            Expression::ArrowFunctionExpression(arrow) => {
                self.arrow(arrow)?.map(Expression::ArrowFunctionExpression)
            }
            Expression::FunctionExpression(func) => self
                .function_expression(func, false)?
                .map(Expression::FunctionExpression),
            Expression::ClassExpression(class) => self
                .class_expression(class)?
                .map(Expression::ClassExpression),

            // ── Plain recursion ────────────────────────────────────────────
            Expression::ObjectExpression(obj) => map_slice!(self, obj.properties, object_property)
                .map(|properties| {
                    Expression::ObjectExpression(tsv_ts::ast::internal::ObjectExpression {
                        properties,
                        ..obj.clone()
                    })
                }),
            Expression::ArrayExpression(arr) => {
                map_slice!(self, arr.elements, opt_expr).map(|elements| {
                    Expression::ArrayExpression(tsv_ts::ast::internal::ArrayExpression {
                        elements,
                        ..arr.clone()
                    })
                })
            }
            Expression::UnaryExpression(unary) => self.expr_ref(unary.argument)?.map(|argument| {
                Expression::UnaryExpression(tsv_ts::ast::internal::UnaryExpression {
                    argument,
                    ..unary.clone()
                })
            }),
            Expression::UpdateExpression(update) => {
                self.expr_ref(update.argument)?.map(|argument| {
                    Expression::UpdateExpression(tsv_ts::ast::internal::UpdateExpression {
                        argument,
                        ..update.clone()
                    })
                })
            }
            Expression::BinaryExpression(binary) => {
                let left = self.expr_ref(binary.left)?;
                let right = self.expr_ref(binary.right)?;
                if left.is_none() && right.is_none() {
                    None
                } else {
                    Some(Expression::BinaryExpression(
                        tsv_ts::ast::internal::BinaryExpression {
                            left: left.unwrap_or(binary.left),
                            right: right.unwrap_or(binary.right),
                            ..binary.clone()
                        },
                    ))
                }
            }
            Expression::MemberExpression(member) => {
                let object = self.expr_ref(member.object)?;
                let property = self.expr_ref(member.property)?;
                if object.is_none() && property.is_none() {
                    None
                } else {
                    Some(Expression::MemberExpression(
                        tsv_ts::ast::internal::MemberExpression {
                            object: object.unwrap_or(member.object),
                            property: property.unwrap_or(member.property),
                            ..member.clone()
                        },
                    ))
                }
            }
            Expression::ConditionalExpression(cond) => {
                let test = self.expr_ref(cond.test)?;
                let consequent = self.expr_ref(cond.consequent)?;
                let alternate = self.expr_ref(cond.alternate)?;
                if test.is_none() && consequent.is_none() && alternate.is_none() {
                    None
                } else {
                    Some(Expression::ConditionalExpression(
                        tsv_ts::ast::internal::ConditionalExpression {
                            test: test.unwrap_or(cond.test),
                            consequent: consequent.unwrap_or(cond.consequent),
                            alternate: alternate.unwrap_or(cond.alternate),
                            span: cond.span,
                        },
                    ))
                }
            }
            Expression::SpreadElement(spread) => self.expr_ref(spread.argument)?.map(|argument| {
                Expression::SpreadElement(tsv_ts::ast::internal::SpreadElement {
                    argument,
                    span: spread.span,
                })
            }),
            Expression::TemplateLiteral(template) => self
                .template_literal(template)?
                .map(Expression::TemplateLiteral),
            Expression::AwaitExpression(node) => self.expr_ref(node.argument)?.map(|argument| {
                Expression::AwaitExpression(tsv_ts::ast::internal::AwaitExpression {
                    argument,
                    span: node.span,
                })
            }),
            Expression::YieldExpression(node) => match node.argument {
                Some(argument) => self.expr_ref(argument)?.map(|argument| {
                    Expression::YieldExpression(tsv_ts::ast::internal::YieldExpression {
                        argument: Some(argument),
                        ..node.clone()
                    })
                }),
                None => None,
            },
            Expression::SequenceExpression(seq) => {
                map_slice!(self, seq.expressions, expr).map(|expressions| {
                    Expression::SequenceExpression(tsv_ts::ast::internal::SequenceExpression {
                        expressions,
                        span: seq.span,
                    })
                })
            }
            Expression::AssignmentExpression(assign) => {
                let left = self.expr_ref(assign.left)?;
                let right = self.expr_ref(assign.right)?;
                if left.is_none() && right.is_none() {
                    None
                } else {
                    Some(Expression::AssignmentExpression(
                        tsv_ts::ast::internal::AssignmentExpression {
                            left: left.unwrap_or(assign.left),
                            right: right.unwrap_or(assign.right),
                            ..assign.clone()
                        },
                    ))
                }
            }
            Expression::ImportExpression(import) => {
                let source = self.expr_ref(import.source)?;
                let options = match import.options {
                    Some(options) => self.expr_ref(options)?.map(Some),
                    None => None,
                };
                if source.is_none() && options.is_none() {
                    None
                } else {
                    Some(Expression::ImportExpression(
                        tsv_ts::ast::internal::ImportExpression {
                            source: source.unwrap_or(import.source),
                            options: options.unwrap_or(import.options),
                            ..import.clone()
                        },
                    ))
                }
            }
            // `/** @type {T} */ (expr)` — an internal-only wrapper recording the
            // cast's semantically-required parens. The oracle has NO such node
            // (it parses without `preserveParens`), so its AST is the inner
            // expression carrying the JSDoc as a detached leading comment: it
            // prints `= /** @type {T} */ 1` and folds the `1`. Unwrap to match on
            // both counts. Valid JavaScript, so this is NOT a TypeScript erasure
            // — it records no region and never trips the `lang="ts"` gate; the
            // JSDoc sits before the wrapper's own span and survives untouched.
            Expression::JsdocCast(cast) => {
                Some(self.expr(cast.inner)?.unwrap_or_else(|| cast.inner.clone()))
            }
            Expression::ParenthesizedExpression(paren) => {
                self.expr_ref(paren.expression)?.map(|expression| {
                    Expression::ParenthesizedExpression(
                        tsv_ts::ast::internal::ParenthesizedExpression {
                            expression,
                            span: paren.span,
                        },
                    )
                })
            }

            // ── Leaves ─────────────────────────────────────────────────────
            Expression::Literal(_)
            | Expression::PrivateIdentifier(_)
            | Expression::RegexLiteral(_)
            | Expression::ThisExpression(_)
            | Expression::Super(_)
            | Expression::MetaProperty(_) => None,
        })
    }

    /// A binding identifier's TypeScript tail (`?` / `: T`) lives inside its own
    /// span, past the bare name — the node span is tail-anchored. The span is
    /// deliberately **not** re-derived: it is only ever used as a comment-window
    /// boundary or a delimiter-scan start, and shrinking it would make the
    /// initializer scan cross the erased type (a `=>` inside a function type
    /// would read as the `=`). Comments in the window refuse instead.
    ///
    /// The trigger is the AST, never the span: a *synthetic* identifier carries
    /// its name out-of-band (`raw_len: 0`) and may steal the span of the node it
    /// replaces, so "span extends past the name" is not a TypeScript signal.
    fn identifier(
        &mut self,
        id: &Identifier<'arena>,
    ) -> Result<Option<Identifier<'arena>>, CompileError> {
        self.refuse_decorators(id.decorators())?;
        // A plain *reference* never populates `extra` — the overwhelmingly
        // common case exits here on one `Option::is_none()` and a bool.
        if !id.optional && id.type_annotation().is_none() {
            return Ok(None);
        }
        self.drop_region(Span::new(
            id.name_span().end,
            tail_end(id.span, id.type_annotation()),
        ));
        Ok(Some(Identifier {
            optional: false,
            extra: None,
            ..id.clone()
        }))
    }

    /// Record the erased tail of a destructuring pattern (`{a}?: T`): the `?` and
    /// the annotation both trail the pattern's closing bracket. The window reaches
    /// back over any trivia to that bracket, so a comment between them
    /// (`{a} /* c */ : T`) can't leak.
    fn pattern_tail(
        &mut self,
        span: Span,
        optional: bool,
        type_annotation: Option<&tsv_ts::ast::internal::TSTypeAnnotation<'arena>>,
    ) {
        if !optional && type_annotation.is_none() {
            return;
        }
        let end = tail_end(span, type_annotation);
        // The `?` sits before the annotation; without one it is the tail's last byte.
        let start = type_annotation.map_or_else(
            || end.saturating_sub(1).max(span.start),
            |annotation| annotation.span.start,
        );
        self.drop_region_from(span.start, Span::new(start, end));
    }

    /// A constructor parameter property (`constructor(public x: number)`).
    ///
    /// The oracle rejects it **only** when it carries `readonly` or an
    /// accessibility modifier *and* sits in a constructor
    /// (`remove_typescript_nodes.js` `TSParameterProperty`) — those synthesize
    /// `this.x = x` into the body, so unwrapping to the bare parameter would
    /// silently drop behavior. Every other shape (a lone `override`, or a
    /// modifier outside a constructor) the oracle simply **unwraps**;
    /// probe-confirmed: `constructor(override x: number)` compiles to
    /// `constructor(x)`.
    fn parameter_property(
        &mut self,
        node: &tsv_ts::ast::internal::TSParameterProperty<'arena>,
    ) -> Result<Expression<'arena>, CompileError> {
        if self.constructor_params && (node.readonly || node.accessibility.is_some()) {
            return unsupported(Refusal::TsParameterProperty);
        }
        self.drop_region(Span::new(node.span.start, node.parameter.span().start));
        Ok(self
            .expr(node.parameter)?
            .unwrap_or_else(|| node.parameter.clone()))
    }

    fn rest_element(
        &mut self,
        rest: &RestElement<'arena>,
    ) -> Result<Option<RestElement<'arena>>, CompileError> {
        self.pattern_tail(rest.span, rest.optional, rest.type_annotation.as_ref());
        let argument = self.expr_ref(rest.argument)?;
        if !rest.optional && rest.type_annotation.is_none() && argument.is_none() {
            return Ok(None);
        }
        Ok(Some(RestElement {
            argument: argument.unwrap_or(rest.argument),
            optional: false,
            type_annotation: None,
            span: rest.span,
        }))
    }

    /// An array/array-pattern element slot — `None` is a hole (`[a, , b]`). The
    /// nesting is the `map_slice!` contract (`&T` in, `Option<T>` out, with
    /// `T = Option<Expression>`), not a modelling choice.
    #[allow(clippy::option_option, clippy::ref_option)]
    fn opt_expr(
        &mut self,
        element: &Option<Expression<'arena>>,
    ) -> Result<Option<Option<Expression<'arena>>>, CompileError> {
        Ok(match element {
            Some(expr) => self.expr(expr)?.map(Some),
            None => None,
        })
    }

    fn object_property(
        &mut self,
        property: &ObjectProperty<'arena>,
    ) -> Result<Option<ObjectProperty<'arena>>, CompileError> {
        Ok(match property {
            ObjectProperty::Property(prop) => self.property(prop)?.map(ObjectProperty::Property),
            ObjectProperty::SpreadElement(spread) => {
                self.expr_ref(spread.argument)?.map(|argument| {
                    ObjectProperty::SpreadElement(tsv_ts::ast::internal::SpreadElement {
                        argument,
                        span: spread.span,
                    })
                })
            }
        })
    }

    fn object_pattern_property(
        &mut self,
        property: &ObjectPatternProperty<'arena>,
    ) -> Result<Option<ObjectPatternProperty<'arena>>, CompileError> {
        Ok(match property {
            ObjectPatternProperty::Property(prop) => {
                self.property(prop)?.map(ObjectPatternProperty::Property)
            }
            ObjectPatternProperty::RestElement(rest) => self
                .rest_element(rest)?
                .map(ObjectPatternProperty::RestElement),
        })
    }

    fn property(
        &mut self,
        prop: &Property<'arena>,
    ) -> Result<Option<Property<'arena>>, CompileError> {
        let key = self.expr(&prop.key)?;
        let value = self.expr(&prop.value)?;
        if key.is_none() && value.is_none() {
            return Ok(None);
        }
        Ok(Some(Property {
            key: key.unwrap_or_else(|| prop.key.clone()),
            value: value.unwrap_or_else(|| prop.value.clone()),
            ..prop.clone()
        }))
    }

    fn template_literal(
        &mut self,
        template: &TemplateLiteral<'arena>,
    ) -> Result<Option<TemplateLiteral<'arena>>, CompileError> {
        Ok(
            map_slice!(self, template.expressions, expr).map(|expressions| TemplateLiteral {
                expressions,
                ..template.clone()
            }),
        )
    }
}
