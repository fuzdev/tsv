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
//! *unchanged*, so the caller keeps the input id or run and nothing is pushed. A
//! `Some` is not always a push: a wrapper that unwraps (`x as T`, `/** … */ (x)`)
//! over an unchanged inner expression yields that inner expression's host id. A
//! subtree with no TypeScript beneath it is never rebuilt. A rebuild is shallow —
//! children are ids and runs into the host stores, so a rebuilt node copies its
//! row with the changed edges swapped and pushes it, never a subtree. A rebuilt
//! list is the lazy-prefix rebuild (`build`'s [`rebuild_run`]): the first changed element
//! opens a frame and copies the unchanged prefix in one block.
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

use tsv_lang::Span;
use tsv_lang::source_scan::{TriviaProfile, skip_trivia, skip_trivia_run};
use tsv_lang::store::Run;
use tsv_ts::__private::{push, push_list};
use tsv_ts::ast::{
    ArrayPattern, ArrowFunctionBody, ArrowFunctionExpression, AssignmentPattern, AstStores,
    AstView, BindingExtraId, BlockStatement, CatchClause, ClassBody, ClassDeclaration,
    ClassExpression, ClassMember, Decorator, EmptyStatement, ExportDefaultDeclaration,
    ExportDefaultValue, ExportKind, ExportNamedDeclaration, ExprId, Expression, ExpressionKind,
    ForInOfLeft, ForInit, FunctionDeclaration, FunctionExpression, Identifier, ImportDeclaration,
    ImportKind, ImportSpecifier, LabeledStatement, MethodDefinition, MethodKind, ObjectPattern,
    ObjectPatternProperty, ObjectProperty, Property, PropertyDefinition, PropertyKind,
    PropertyModifier, RestElement, Statement, StatementKind, StaticBlock, StmtId, SwitchCase,
    TSInterfaceHeritage, TSModuleDeclarationBody, TSParameterProperty, TaggedTemplateExpression,
    TemplateLiteral, TryStatement, VariableDeclaration, VariableDeclarator,
};

use crate::CompileError;
use crate::build::{HasStores, Item, rebuild_run};
use crate::refusal::Refusal;
use crate::text_class::{is_js_whitespace, js_char_at};

/// The product of erasing a statement list.
pub(crate) struct Erased {
    /// The type-free statements — the input run itself when nothing changed.
    pub(crate) body: Run<StmtId>,
    /// Every erased source region, already extended to its comment-refusal
    /// window (see [`Eraser::drop_region`]). In walk order.
    pub(crate) regions: Vec<Span>,
    /// Whether the tree was rebuilt at all. `false` proves the input carried no
    /// TypeScript-only node **and** no `CommentBoundPair` — the property the self-check
    /// asserts of the finished program.
    pub(crate) changed: bool,
    /// Whether TypeScript-only *syntax* was erased. Distinct from
    /// [`Self::changed`]: unwrapping a `CommentBoundPair` (`/** @type {T} */ (x)`) is a
    /// compile-path normalization of **valid JS**, so it must not make a
    /// `lang`-less script look like TypeScript.
    pub(crate) typescript: bool,
}

/// Erase every TypeScript-only construct from a statement list, pushing the rows
/// of every rebuild into `stores` beside the parse's.
pub(crate) fn erase_statements(
    stores: &mut AstStores,
    source: &str,
    body: Run<StmtId>,
) -> Result<Erased, CompileError> {
    let mut eraser = Eraser::new(stores, source);
    let erased = eraser.statements(body)?;
    debug_assert!(
        !eraser.typescript || !eraser.regions.is_empty(),
        "every TypeScript erasure must record its region (the refusal window depends on it)"
    );
    Ok(Erased {
        changed: erased.is_some(),
        typescript: eraser.typescript,
        body: erased.unwrap_or(body),
        regions: eraser.regions,
    })
}

/// The product of erasing a single expression.
pub(crate) struct ErasedExpr {
    /// The type-free expression's id — a pushed rebuild, or the host id of an
    /// unwrapped wrapper's unchanged inner expression; `None` means *unchanged*, so
    /// the caller keeps the input id.
    pub(crate) expr: Option<ExprId>,
    /// Every erased source region, already extended to its comment-refusal
    /// window (see [`Eraser::drop_region`]). In walk order.
    pub(crate) regions: Vec<Span>,
    /// Whether TypeScript-only *syntax* was erased — the `lang="ts"` gate, which
    /// applies to the template exactly as it does to the script (the oracle's
    /// flag is document-wide). Distinct from `expr.is_some()`: unwrapping a
    /// `CommentBoundPair` rebuilds valid JS and must not read as TypeScript.
    pub(crate) typescript: bool,
}

/// Erase every TypeScript-only construct from a single expression — the
/// per-expression entry point for the Svelte template's borrow points (`{expr}`
/// tags, attribute values, block tests, `{@const}`/`{#each}` patterns), where the
/// erasure applies at the borrow and the Svelte AST itself is never rebuilt.
pub(crate) fn erase_expression(
    stores: &mut AstStores,
    source: &str,
    expr: ExprId,
) -> Result<ErasedExpr, CompileError> {
    let mut eraser = Eraser::new(stores, source);
    let expr = eraser.expr_ref(expr)?;
    Ok(ErasedExpr {
        expr,
        regions: eraser.regions,
        typescript: eraser.typescript,
    })
}

/// The product of erasing a single parameter, a row no id names (a `{#snippet}`
/// parameter, held by value in its list).
pub(crate) struct ErasedParam {
    /// The type-free parameter — `None` means *unchanged*, so the caller keeps the
    /// input row.
    pub(crate) param: Option<Expression>,
    /// Every erased source region, already extended to its comment-refusal
    /// window (see [`Eraser::drop_region`]). In walk order.
    pub(crate) regions: Vec<Span>,
    /// Whether TypeScript-only *syntax* was erased, as [`ErasedExpr::typescript`].
    pub(crate) typescript: bool,
}

/// Erase every TypeScript-only construct from one parameter, by value — the
/// expression inventory over the row itself. Not a parameter LIST's erasure: no
/// `this` pseudo-parameter is dropped and no parameter property is read as a
/// constructor's (a `{#snippet}` head is neither).
pub(crate) fn erase_parameter(
    stores: &mut AstStores,
    source: &str,
    param: Expression,
) -> Result<ErasedParam, CompileError> {
    let mut eraser = Eraser::new(stores, source);
    let param = eraser.expr_row(&param)?;
    Ok(ErasedParam {
        param,
        regions: eraser.regions,
        typescript: eraser.typescript,
    })
}

/// Position of the next surviving token at or after `from`: skips whitespace and
/// both comment forms via the trivia-aware cursor (never a raw `find`).
///
/// This is the **forward** half of an erased region's comment-refusal window: a
/// comment glued to the region's tail (`let x: Foo /* c */ = v`, which the
/// oracle re-anchors onto the initializer) sits inside it. Public so the
/// `erase_comment_census` diagnostic measures the same rule the compiler
/// enforces.
///
/// The class is [`is_js_whitespace`], never `u8::is_ascii_whitespace`; the shared
/// run-skipper owns the rest — the alternating whitespace/comment loop, the whole-char
/// step, and the end guard [`skip_trivia`] needs (a run reaching the end of the source is
/// the ordinary case here, and calling it at `end` would index out of bounds).
#[must_use]
pub fn next_token_pos(source: &str, from: u32) -> u32 {
    skip_trivia_run(
        source,
        from as usize,
        TriviaProfile::COMMENTS,
        is_js_whitespace,
    ) as u32
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
        // JS whitespace, not `u8::is_ascii_whitespace` — see `js_char_at`. BOTH
        // branches step a whole character: a multi-byte token character (a
        // `café` identifier) would otherwise leave `pos` on a continuation byte,
        // and `last` — a returned span position — inside one.
        let Some(c) = js_char_at(source, pos) else {
            break;
        };
        if !c.is_whitespace {
            last = u32::try_from(pos + c.len).unwrap_or(to).min(to);
        }
        pos += c.len;
    }
    last
}

fn unsupported<T>(reason: Refusal) -> Result<T, CompileError> {
    Err(CompileError::Unsupported(reason))
}

/// The end of a binding's erased TypeScript tail (`?` / `: T`), given the span of
/// its annotation when it has one.
///
/// The node's own span is usually tail-anchored over the annotation — but not
/// always: a Svelte **block pattern** (`{#each x as y: T}`, `{@const a: T = v}`)
/// is parsed by `tsv_svelte`, which leaves the binding's span on the bare name
/// and hangs the annotation off it as a sibling. Taking the max covers both
/// span models, so the erased region is never empty (an empty one would record
/// no refusal window at all).
fn tail_end(span: Span, type_annotation: Option<Span>) -> u32 {
    span.end
        .max(type_annotation.map_or(0, |annotation| annotation.end))
}

/// Decorators are a `typescript_invalid_feature` hard error in the oracle
/// (`@dec` on a class, member, or parameter) — and without `lang="ts"` they
/// are a plain-JS parse error. Never erased, never emitted.
fn refuse_decorators(decorators: Option<Run<Decorator>>) -> Result<(), CompileError> {
    if decorators.is_some_and(|list| !list.is_empty()) {
        return unsupported(Refusal::Decorator);
    }
    Ok(())
}

/// An expression's erasure, when it changed: a rebuilt row (not yet pushed) or an
/// existing node by id — what an unwrapped wrapper (`x as T`, `/** … */ (x)`)
/// yields when its inner expression is itself unchanged, so the host's node is
/// passed on by id rather than copied out and re-pushed.
#[derive(Clone, Copy)]
enum Edge {
    Row(Expression),
    Id(ExprId),
}

/// The erasable parts of a class header, shared by `ClassDeclaration` and
/// `ClassExpression` (which differ only in whether `declare` exists).
struct ClassHead {
    span: Span,
    r#abstract: bool,
    /// End of the class name, when it has one — the backward bound for a
    /// name-less `implements` clause.
    id_end: Option<u32>,
    type_parameters: Option<Span>,
    /// End of the `extends` expression, when there is one.
    super_class_end: Option<u32>,
    super_type_parameters: Option<Span>,
    implements: Run<TSInterfaceHeritage>,
}

/// What a class erasure produces, once resolved against the original — the
/// `ClassHead` sibling for the parts a class form must rebuild with. Reached only
/// when something WAS erased, so both fields are final values, not
/// `None`-means-unchanged options.
struct ClassParts {
    super_class: Option<ExprId>,
    body: ClassBody,
}

struct Eraser<'s, 'src> {
    /// The host stores: the input is read through their view, every rebuilt row
    /// pushed beside the parse's.
    stores: &'s mut AstStores,
    source: &'src str,
    /// Erased regions, already extended to their comment-refusal windows.
    regions: Vec<Span>,
    /// Set by every TypeScript-only erasure (never by the `CommentBoundPair` unwrap).
    typescript: bool,
    /// Whether the parameter list currently being erased is a **constructor's**.
    /// The oracle rejects a parameter property only there, and only when it
    /// carries `readonly`/an accessibility modifier — see
    /// [`Eraser::parameter_property`].
    constructor_params: bool,
}

impl HasStores for Eraser<'_, '_> {
    #[inline]
    fn stores(&mut self) -> &mut AstStores {
        self.stores
    }
}

impl<'s, 'src> Eraser<'s, 'src> {
    fn new(stores: &'s mut AstStores, source: &'src str) -> Self {
        Self {
            stores,
            source,
            regions: Vec::new(),
            typescript: false,
            constructor_params: false,
        }
    }
}

impl Eraser<'_, '_> {
    fn view(&self) -> AstView<'_> {
        self.stores.view()
    }

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

    /// The decorators a binding's extra row carries, refused as
    /// [`refuse_decorators`] refuses a decorator list.
    fn refuse_extra_decorators(&self, extra: Option<BindingExtraId>) -> Result<(), CompileError> {
        refuse_decorators(
            self.view()
                .opt_binding_extra(extra)
                .and_then(|extra| extra.decorators),
        )
    }

    // ── Edges ──────────────────────────────────────────────────────────────

    /// The id an erased expression is reached by: a rebuilt row pushed, an
    /// existing node's id as it is.
    fn push_edge(&mut self, edge: Edge) -> ExprId {
        match edge {
            Edge::Row(row) => push(self.stores, row),
            Edge::Id(id) => id,
        }
    }

    /// The row an erased expression is held by in a value position (a parameter
    /// list, a catch parameter): a rebuilt row as it is, an existing node's row
    /// copied out.
    fn edge_row(&self, edge: Edge) -> Expression {
        match edge {
            Edge::Row(row) => row,
            Edge::Id(id) => *self.view().expr(id),
        }
    }

    /// The expression `id` names, erased and pushed — `None` when unchanged.
    fn expr_ref(&mut self, id: ExprId) -> Result<Option<ExprId>, CompileError> {
        let row = *self.view().expr(id);
        Ok(self.expr(&row)?.map(|edge| self.push_edge(edge)))
    }

    /// An expression held by value, erased — `None` when unchanged.
    fn expr_row(&mut self, row: &Expression) -> Result<Option<Expression>, CompileError> {
        Ok(self.expr(row)?.map(|edge| self.edge_row(edge)))
    }

    /// What `id` erases to, as an edge — `id` itself when unchanged: the inner
    /// expression of an unwrapped wrapper.
    fn inner(&mut self, id: ExprId) -> Result<Edge, CompileError> {
        let row = *self.view().expr(id);
        Ok(self.expr(&row)?.unwrap_or(Edge::Id(id)))
    }

    /// An id list (call / `new` arguments, sequence and template expressions),
    /// rebuilt when an element changed.
    fn id_run(&mut self, run: Run<ExprId>) -> Result<Option<Run<ExprId>>, CompileError> {
        rebuild_run(
            self,
            run,
            |view, run| view.expr_list(run),
            |eraser, id| Ok(eraser.expr_ref(id)?.into()),
        )
    }

    /// An id list with holes (array and array-pattern elements), rebuilt when an
    /// element changed.
    fn opt_id_run(
        &mut self,
        run: Run<Option<ExprId>>,
    ) -> Result<Option<Run<Option<ExprId>>>, CompileError> {
        rebuild_run(
            self,
            run,
            |view, run| view.expr_opt_list(run),
            |eraser, element| {
                Ok(match element {
                    Some(id) => eraser.expr_ref(id)?.map(Some).into(),
                    None => Item::Keep,
                })
            },
        )
    }

    // ── Statements ─────────────────────────────────────────────────────────

    fn statements(&mut self, run: Run<StmtId>) -> Result<Option<Run<StmtId>>, CompileError> {
        rebuild_run(
            self,
            run,
            |view, run| view.stmt_list(run),
            |eraser, id| {
                let stmt = *eraser.view().stmt(id);
                Ok(match eraser.statement(&stmt)? {
                    Item::Keep => Item::Keep,
                    Item::Replace(new) => Item::Replace(push(eraser.stores, new)),
                    Item::Drop => Item::Drop,
                })
            },
        )
    }

    /// A statement in a single-statement position (an `if` branch, a loop body).
    /// A dropped statement becomes `;` — no such position can legally hold a
    /// TypeScript-only declaration, so this is unreachable in valid input, but
    /// it keeps the transformation total.
    fn statement_ref(&mut self, id: StmtId) -> Result<Option<StmtId>, CompileError> {
        let stmt = *self.view().stmt(id);
        Ok(match self.statement(&stmt)? {
            Item::Keep => None,
            Item::Replace(new) => Some(push(self.stores, new)),
            Item::Drop => Some(push(
                self.stores,
                Statement {
                    span: stmt.span,
                    kind: StatementKind::EmptyStatement(EmptyStatement),
                },
            )),
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
    fn statement(&mut self, stmt: &Statement) -> Result<Item<Statement>, CompileError> {
        // The header span: the drop-set payloads below carry none of their own, and
        // a rebuilt statement keeps the original's.
        let span = stmt.span;
        Ok(match stmt.kind {
            // ── TypeScript-only statements: dropped whole ──────────────────
            StatementKind::TSTypeAliasDeclaration(_) => {
                self.drop_region(span);
                Item::Drop
            }
            StatementKind::TSInterfaceDeclaration(decl) => {
                let decl_span = self.view().interface_decl(decl).span;
                self.drop_region(decl_span);
                Item::Drop
            }
            StatementKind::TSDeclareFunction(decl) => {
                let decl_span = self.view().declare_fn(decl).span;
                self.drop_region(decl_span);
                Item::Drop
            }

            // ── Refuse-don't-erase: runtime semantics, or an oracle bug ────
            StatementKind::TSEnumDeclaration(_) => return unsupported(Refusal::TsEnum),
            StatementKind::TSImportEqualsDeclaration(_) => {
                return unsupported(Refusal::TsImportEquals);
            }
            StatementKind::TSExportAssignment(_) => {
                return unsupported(Refusal::TsExportAssignment);
            }
            StatementKind::TSNamespaceExportDeclaration(_) => {
                return unsupported(Refusal::TsNamespaceExport);
            }

            // A `namespace`/`module` drops only when its whole body erases away;
            // any surviving member means it lowers to an IIFE at runtime, which
            // the oracle rejects (`namespaces with non-type nodes`) — with no
            // `declare` carve-out either way.
            StatementKind::TSModuleDeclaration(decl) => {
                let decl = *self.view().module_decl(decl);
                let mark = self.regions.len();
                let type_only = self.module_body_is_type_only(decl.body)?;
                // The whole declaration's span subsumes whatever its body
                // recorded, so the inner regions collapse into this one.
                self.regions.truncate(mark);
                if !type_only {
                    return unsupported(Refusal::TsNamespaceWithValue);
                }
                self.drop_region(decl.span);
                Item::Drop
            }

            // ── Ordinary statements ────────────────────────────────────────
            StatementKind::VariableDeclaration(decl) => {
                // `declare const x: T;` is ambient — dropped whole.
                if decl.declare {
                    self.drop_region(decl.span);
                    return Ok(Item::Drop);
                }
                match self.variable_declaration(&decl)? {
                    None => Item::Keep,
                    Some(new) => Item::Replace(Statement::from_variable_declaration(new)),
                }
            }
            StatementKind::ExpressionStatement(stmt) => match self.expr_ref(stmt.expression)? {
                None => Item::Keep,
                Some(expression) => Item::Replace(Statement {
                    span,
                    kind: StatementKind::ExpressionStatement(tsv_ts::ast::ExpressionStatement {
                        expression,
                        ..stmt
                    }),
                }),
            },
            StatementKind::ReturnStatement(stmt) => match stmt.argument {
                Some(argument) => match self.expr_ref(argument)? {
                    None => Item::Keep,
                    Some(argument) => Item::Replace(Statement {
                        span,
                        kind: StatementKind::ReturnStatement(tsv_ts::ast::ReturnStatement {
                            argument: Some(argument),
                        }),
                    }),
                },
                None => Item::Keep,
            },
            StatementKind::BlockStatement(block) => match self.block(&block)? {
                None => Item::Keep,
                Some(new) => Item::Replace(Statement::from_block_statement(new)),
            },
            StatementKind::FunctionDeclaration(decl) => {
                let func = *self.view().fn_decl(decl);
                match self.function_declaration(&func)? {
                    None => Item::Keep,
                    Some(new) => {
                        let id = push(self.stores, new);
                        Item::Replace(Statement::from_function_declaration(new.span, id))
                    }
                }
            }
            StatementKind::ClassDeclaration(decl) => {
                let class = *self.view().class_decl(decl);
                // `declare class C {}` is ambient — dropped whole.
                if class.declare {
                    self.drop_region(class.span);
                    return Ok(Item::Drop);
                }
                match self.class_declaration(&class)? {
                    None => Item::Keep,
                    Some(new) => {
                        let id = push(self.stores, new);
                        Item::Replace(Statement::from_class_declaration(new.span, id))
                    }
                }
            }
            StatementKind::ImportDeclaration(decl) => {
                let decl = *self.view().import_decl(decl);
                // `import type { X } from 'm'` — the whole declaration.
                if decl.import_kind == ImportKind::Type {
                    self.drop_region(span);
                    return Ok(Item::Drop);
                }
                // A per-specifier `type` marker (`import { type X, Y }`). A list
                // that filters to EMPTY drops the whole statement — the oracle's
                // `if (specifiers.length === 0) return b.empty`, so `import
                // { type X } from 'm'` emits nothing (not `import {}`, not a
                // bare side-effect import). A declaration that carried no
                // specifiers at all (`import 'm'`, `import {} from 'm'`) is
                // untouched.
                let specifiers = self.view().import_specs(decl.specifiers).to_vec();
                if specifiers.is_empty()
                    || !specifiers.iter().any(|spec| {
                        matches!(spec, ImportSpecifier::Named(named)
                            if named.import_kind == ImportKind::Type)
                    })
                {
                    return Ok(Item::Keep);
                }
                let mark = self.regions.len();
                let mut kept: Vec<ImportSpecifier> = Vec::with_capacity(specifiers.len());
                for spec in specifiers {
                    match spec {
                        ImportSpecifier::Named(named) if named.import_kind == ImportKind::Type => {
                            self.drop_region(named.span);
                        }
                        _ => kept.push(spec),
                    }
                }
                if kept.is_empty() {
                    // Subsumed by the whole-declaration region.
                    self.regions.truncate(mark);
                    self.drop_region(span);
                    return Ok(Item::Drop);
                }
                let specifiers = push_list(self.stores, &kept);
                let new = push(self.stores, ImportDeclaration { specifiers, ..decl });
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::ImportDeclaration(new),
                })
            }
            StatementKind::ExportNamedDeclaration(decl) => {
                let decl = *self.view().export_named(decl);
                // `export type { X }` / `export type * …` — the whole statement.
                if decl.export_kind == ExportKind::Type {
                    self.drop_region(span);
                    return Ok(Item::Drop);
                }
                // `export interface X {}` / `export type X = …` / `export
                // declare const x` — the exported declaration erases away, so
                // the export goes with it.
                if let Some(declaration) = decl.declaration {
                    let mark = self.regions.len();
                    let inner = *self.view().stmt(declaration);
                    return Ok(match self.statement(&inner)? {
                        Item::Keep => Item::Keep,
                        Item::Drop => {
                            self.regions.truncate(mark);
                            self.drop_region(span);
                            Item::Drop
                        }
                        Item::Replace(new) => {
                            let declaration = push(self.stores, new);
                            let new = push(
                                self.stores,
                                ExportNamedDeclaration {
                                    declaration: Some(declaration),
                                    ..decl
                                },
                            );
                            Item::Replace(Statement {
                                span,
                                kind: StatementKind::ExportNamedDeclaration(new),
                            })
                        }
                    });
                }
                // `export { type A, b }` — the inline type marker.
                let specifiers = self.view().export_specs(decl.specifiers).to_vec();
                if !specifiers
                    .iter()
                    .any(|spec| spec.export_kind == ExportKind::Type)
                {
                    return Ok(Item::Keep);
                }
                let mark = self.regions.len();
                let mut kept = Vec::with_capacity(specifiers.len());
                for spec in specifiers {
                    if spec.export_kind == ExportKind::Type {
                        self.drop_region(spec.span);
                    } else {
                        kept.push(spec);
                    }
                }
                if kept.is_empty() {
                    self.regions.truncate(mark);
                    self.drop_region(span);
                    return Ok(Item::Drop);
                }
                let specifiers = push_list(self.stores, &kept);
                let new = push(self.stores, ExportNamedDeclaration { specifiers, ..decl });
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::ExportNamedDeclaration(new),
                })
            }
            StatementKind::ExportDefaultDeclaration(decl) => {
                let decl = *self.view().export_default(decl);
                let declaration = match decl.declaration {
                    ExportDefaultValue::Expression(expr) => {
                        self.expr_ref(expr)?.map(ExportDefaultValue::Expression)
                    }
                    ExportDefaultValue::FunctionDeclaration(func) => self
                        .function_declaration(&func)?
                        .map(ExportDefaultValue::FunctionDeclaration),
                    ExportDefaultValue::ClassDeclaration(class) => self
                        .class_declaration(&class)?
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
                    None => Item::Keep,
                    Some(declaration) => {
                        let new = push(self.stores, ExportDefaultDeclaration { declaration });
                        Item::Replace(Statement {
                            span,
                            kind: StatementKind::ExportDefaultDeclaration(new),
                        })
                    }
                }
            }
            StatementKind::ExportAllDeclaration(decl) => {
                if self.view().export_all(decl).export_kind == ExportKind::Type {
                    self.drop_region(span);
                    return Ok(Item::Drop);
                }
                Item::Keep
            }

            // ── Control flow ───────────────────────────────────────────────
            StatementKind::IfStatement(stmt) => {
                let test = self.expr_ref(stmt.test)?;
                let consequent = self.statement_ref(stmt.consequent)?;
                let alternate = match stmt.alternate {
                    Some(alt) => self.statement_ref(alt)?.map(Some),
                    None => None,
                };
                if test.is_none() && consequent.is_none() && alternate.is_none() {
                    return Ok(Item::Keep);
                }
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::IfStatement(tsv_ts::ast::IfStatement {
                        test: test.unwrap_or(stmt.test),
                        consequent: consequent.unwrap_or(stmt.consequent),
                        alternate: alternate.unwrap_or(stmt.alternate),
                    }),
                })
            }
            StatementKind::ForStatement(stmt) => {
                let init = match stmt.init {
                    Some(init) => {
                        let head = *self.view().for_head(init);
                        self.for_init(&head)?
                            .map(|head| Some(push(self.stores, head)))
                    }
                    None => None,
                };
                let test = match stmt.test {
                    Some(test) => self.expr_ref(test)?.map(Some),
                    None => None,
                };
                let update = match stmt.update {
                    Some(update) => self.expr_ref(update)?.map(Some),
                    None => None,
                };
                let body = self.statement_ref(stmt.body)?;
                if init.is_none() && test.is_none() && update.is_none() && body.is_none() {
                    return Ok(Item::Keep);
                }
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::ForStatement(tsv_ts::ast::ForStatement {
                        init: init.unwrap_or(stmt.init),
                        test: test.unwrap_or(stmt.test),
                        update: update.unwrap_or(stmt.update),
                        body: body.unwrap_or(stmt.body),
                    }),
                })
            }
            StatementKind::ForInStatement(stmt) => {
                let left = self.for_left(stmt.left)?;
                let right = self.expr_ref(stmt.right)?;
                let body = self.statement_ref(stmt.body)?;
                if left.is_none() && right.is_none() && body.is_none() {
                    return Ok(Item::Keep);
                }
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::ForInStatement(tsv_ts::ast::ForInStatement {
                        left: left.unwrap_or(stmt.left),
                        right: right.unwrap_or(stmt.right),
                        body: body.unwrap_or(stmt.body),
                    }),
                })
            }
            StatementKind::ForOfStatement(stmt) => {
                let left = self.for_left(stmt.left)?;
                let right = self.expr_ref(stmt.right)?;
                let body = self.statement_ref(stmt.body)?;
                if left.is_none() && right.is_none() && body.is_none() {
                    return Ok(Item::Keep);
                }
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::ForOfStatement(tsv_ts::ast::ForOfStatement {
                        left: left.unwrap_or(stmt.left),
                        right: right.unwrap_or(stmt.right),
                        body: body.unwrap_or(stmt.body),
                        ..stmt
                    }),
                })
            }
            StatementKind::WhileStatement(stmt) => {
                let test = self.expr_ref(stmt.test)?;
                let body = self.statement_ref(stmt.body)?;
                if test.is_none() && body.is_none() {
                    return Ok(Item::Keep);
                }
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::WhileStatement(tsv_ts::ast::WhileStatement {
                        test: test.unwrap_or(stmt.test),
                        body: body.unwrap_or(stmt.body),
                    }),
                })
            }
            // Unreachable in practice: a Svelte `<script>` is Module code, so it is
            // strict and the parser refuses `with` there. The arm is a walk all the
            // same, so the variant is never silently dropped.
            StatementKind::WithStatement(stmt) => {
                let object = self.expr_ref(stmt.object)?;
                let body = self.statement_ref(stmt.body)?;
                if object.is_none() && body.is_none() {
                    return Ok(Item::Keep);
                }
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::WithStatement(tsv_ts::ast::WithStatement {
                        object: object.unwrap_or(stmt.object),
                        body: body.unwrap_or(stmt.body),
                    }),
                })
            }
            StatementKind::DoWhileStatement(stmt) => {
                let body = self.statement_ref(stmt.body)?;
                let test = self.expr_ref(stmt.test)?;
                if test.is_none() && body.is_none() {
                    return Ok(Item::Keep);
                }
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::DoWhileStatement(tsv_ts::ast::DoWhileStatement {
                        body: body.unwrap_or(stmt.body),
                        test: test.unwrap_or(stmt.test),
                    }),
                })
            }
            StatementKind::SwitchStatement(stmt) => {
                let discriminant = self.expr_ref(stmt.discriminant)?;
                let cases = rebuild_run(
                    self,
                    stmt.cases,
                    |view, run| view.cases(run),
                    |eraser, case| Ok(eraser.switch_case(&case)?.into()),
                )?;
                if discriminant.is_none() && cases.is_none() {
                    return Ok(Item::Keep);
                }
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::SwitchStatement(tsv_ts::ast::SwitchStatement {
                        discriminant: discriminant.unwrap_or(stmt.discriminant),
                        cases: cases.unwrap_or(stmt.cases),
                    }),
                })
            }
            StatementKind::TryStatement(id) => {
                // The payload is a row by id: a rebuild pushes a new row and points
                // the statement at it, leaving the host's row as it was.
                let stmt = *self.view().try_stmt(id);
                let block = self.block(&stmt.block)?;
                let handler = match stmt.handler {
                    Some(handler) => {
                        let clause = *self.view().catch(handler);
                        self.catch_clause(&clause)?
                            .map(|clause| Some(push(self.stores, clause)))
                    }
                    None => None,
                };
                let finalizer = match stmt.finalizer {
                    Some(finalizer) => self.block(&finalizer)?.map(Some),
                    None => None,
                };
                if block.is_none() && handler.is_none() && finalizer.is_none() {
                    return Ok(Item::Keep);
                }
                let new = push(
                    self.stores,
                    TryStatement {
                        block: block.unwrap_or(stmt.block),
                        handler: handler.unwrap_or(stmt.handler),
                        finalizer: finalizer.unwrap_or(stmt.finalizer),
                    },
                );
                Item::Replace(Statement {
                    span,
                    kind: StatementKind::TryStatement(new),
                })
            }
            StatementKind::ThrowStatement(stmt) => match self.expr_ref(stmt.argument)? {
                None => Item::Keep,
                Some(argument) => Item::Replace(Statement {
                    span,
                    kind: StatementKind::ThrowStatement(tsv_ts::ast::ThrowStatement { argument }),
                }),
            },
            StatementKind::LabeledStatement(id) => {
                // A row by id, rebuilt into a new row as the `try` payload is.
                let stmt = *self.view().labeled_stmt(id);
                match self.statement_ref(stmt.body)? {
                    None => Item::Keep,
                    Some(body) => {
                        let new = push(self.stores, LabeledStatement { body, ..stmt });
                        Item::Replace(Statement {
                            span,
                            kind: StatementKind::LabeledStatement(new),
                        })
                    }
                }
            }

            // ── No TypeScript-bearing children ─────────────────────────────
            StatementKind::BreakStatement(_)
            | StatementKind::ContinueStatement(_)
            | StatementKind::EmptyStatement(_)
            | StatementKind::DebuggerStatement(_) => Item::Keep,
        })
    }

    /// Whether a `namespace`/`module` body erases away completely — the oracle's
    /// fork: all-type → drop, any surviving member → `typescript_invalid_feature`
    /// (a value namespace lowers to an IIFE, which no erasure can express).
    fn module_body_is_type_only(
        &mut self,
        body: Option<TSModuleDeclarationBody>,
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

    fn for_init(&mut self, init: &ForInit) -> Result<Option<ForInit>, CompileError> {
        Ok(match init {
            ForInit::VariableDeclaration(decl) => self
                .variable_declaration(decl)?
                .map(ForInit::VariableDeclaration),
            &ForInit::Expression(expr) => self.expr_ref(expr)?.map(ForInit::Expression),
        })
    }

    /// The left side of a `for … in` / `for … of`, a row by id: a rebuild pushes a
    /// new row.
    fn for_left(
        &mut self,
        left: tsv_ts::ast::ForLeftId,
    ) -> Result<Option<tsv_ts::ast::ForLeftId>, CompileError> {
        let row = *self.view().for_left(left);
        let new = match row {
            ForInOfLeft::VariableDeclaration(decl) => self
                .variable_declaration(&decl)?
                .map(ForInOfLeft::VariableDeclaration),
            ForInOfLeft::Pattern(pattern) => self.expr_ref(pattern)?.map(ForInOfLeft::Pattern),
        };
        Ok(new.map(|new| push(self.stores, new)))
    }

    fn switch_case(&mut self, case: &SwitchCase) -> Result<Option<SwitchCase>, CompileError> {
        let test = match case.test {
            Some(test) => self.expr_ref(test)?.map(Some),
            None => None,
        };
        let consequent = self.statements(case.consequent)?;
        if test.is_none() && consequent.is_none() {
            return Ok(None);
        }
        Ok(Some(SwitchCase {
            test: test.unwrap_or(case.test),
            consequent: consequent.unwrap_or(case.consequent),
            span: case.span,
        }))
    }

    fn catch_clause(&mut self, clause: &CatchClause) -> Result<Option<CatchClause>, CompileError> {
        let param = match &clause.param {
            Some(param) => self.expr_row(param)?.map(Some),
            None => None,
        };
        let body = self.block(&clause.body)?;
        if param.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(CatchClause {
            param: param.unwrap_or(clause.param),
            body: body.unwrap_or(clause.body),
            span: clause.span,
        }))
    }

    fn block(&mut self, block: &BlockStatement) -> Result<Option<BlockStatement>, CompileError> {
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
        decl: &VariableDeclaration,
    ) -> Result<Option<VariableDeclaration>, CompileError> {
        if decl.declare {
            self.drop_region(Span::new(
                decl.span.start,
                decl.span.start + "declare".len() as u32,
            ));
        }
        let declarations = rebuild_run(
            self,
            decl.declarations,
            |view, run| view.declarators(run),
            |eraser, d| Ok(eraser.variable_declarator(&d)?.into()),
        )?;
        if !decl.declare && declarations.is_none() {
            return Ok(None);
        }
        Ok(Some(VariableDeclaration {
            declarations: declarations.unwrap_or(decl.declarations),
            declare: false,
            ..*decl
        }))
    }

    fn variable_declarator(
        &mut self,
        declarator: &VariableDeclarator,
    ) -> Result<Option<VariableDeclarator>, CompileError> {
        let id = self.expr_ref(declarator.id)?;
        let init = match declarator.init {
            Some(init) => self.expr_ref(init)?.map(Some),
            None => None,
        };
        // `let x!: T` — the definite-assignment `!` lives on the declarator but
        // sits inside the id's span tail, so the annotation's own erased region
        // already covers it. Without an annotation (`let x!;`) the id erases to
        // nothing, so record the `!` here.
        if declarator.definite
            && id.is_none()
            && let ExpressionKind::Identifier(ident) = self.view().expr(declarator.id).kind
        {
            self.drop_region(Span::new(ident.name_span().end, ident.span.end));
        }
        if id.is_none() && init.is_none() && !declarator.definite {
            return Ok(None);
        }
        Ok(Some(VariableDeclarator {
            id: id.unwrap_or(declarator.id),
            init: init.unwrap_or(declarator.init),
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
        params: Run<Expression>,
        anchor: u32,
        drop_this: bool,
        constructor: bool,
    ) -> Result<Option<Run<Expression>>, CompileError> {
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
        params: Run<Expression>,
        anchor: u32,
        drop_this: bool,
    ) -> Result<Option<Run<Expression>>, CompileError> {
        let view = self.view();
        let list = view.exprs(params);
        let first = list.first().copied();
        let second_start = list.get(1).map(|next| next.span.start);
        let Some(this) = first.filter(|first| drop_this && self.is_this_param(first)) else {
            return self.param_run(params);
        };
        // The dropped `this` takes its trailing comma with it, and the window
        // reaches back over the `(` so a comment before it can't leak. The kept
        // rest is a sub-run of the input, shared when nothing in it changed.
        let this = this.span;
        let region_end = second_start.unwrap_or(this.end);
        self.drop_region_from(anchor, Span::new(this.start, region_end));
        let rest = params.sub_run(1..params.len());
        Ok(Some(self.param_run(rest)?.unwrap_or(rest)))
    }

    /// A parameter list (held by value), rebuilt when a parameter changed.
    fn param_run(&mut self, run: Run<Expression>) -> Result<Option<Run<Expression>>, CompileError> {
        rebuild_run(
            self,
            run,
            |view, run| view.exprs(run),
            |eraser, param| Ok(eraser.expr_row(&param)?.into()),
        )
    }

    /// The TypeScript `this` pseudo-parameter: a bare `this` identifier binding
    /// (acorn's `isThisParam`, mirrored by `tsv_ts`'s parser).
    fn is_this_param(&self, param: &Expression) -> bool {
        let ExpressionKind::Identifier(id) = &param.kind else {
            return false;
        };
        id.name_span().extract(self.source) == "this"
    }

    fn function_declaration(
        &mut self,
        func: &FunctionDeclaration,
    ) -> Result<Option<FunctionDeclaration>, CompileError> {
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
            body: body.unwrap_or(func.body),
            ..*func
        }))
    }

    fn function_expression(
        &mut self,
        func: &FunctionExpression,
        constructor: bool,
    ) -> Result<Option<FunctionExpression>, CompileError> {
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
            body: body.unwrap_or(func.body),
            ..*func
        }))
    }

    fn arrow(
        &mut self,
        arrow: &ArrowFunctionExpression,
        span: Span,
    ) -> Result<Option<ArrowFunctionExpression>, CompileError> {
        let params_start = arrow.params_start.unwrap_or(span.start);
        if let Some(type_parameters) = &arrow.type_parameters {
            self.drop_region_from(span.start, type_parameters.span);
        }
        if let Some(return_type) = &arrow.return_type {
            self.drop_region_from(params_start, return_type.span);
        }
        // An arrow keeps a leading `this` binding — it is an ordinary parameter
        // name there, and the oracle's `remove_this_param` never visits arrows.
        let params = self.params(arrow.params, params_start, false, false)?;
        let body = match arrow.body {
            ArrowFunctionBody::Expression(expr) => {
                self.expr_ref(expr)?.map(ArrowFunctionBody::Expression)
            }
            ArrowFunctionBody::BlockStatement(block) => {
                self.block(&block)?.map(ArrowFunctionBody::BlockStatement)
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
            body: body.unwrap_or(arrow.body),
            ..*arrow
        }))
    }

    /// Erase a class's decorators, header, `extends` expression and body — the
    /// whole walk both class forms share, which differ only in the node type they
    /// rebuild. Returns `None` when nothing was erased (the pass's
    /// unchanged-means-share contract), else the resolved `extends`/body to
    /// rebuild with.
    ///
    /// The `ClassHead` literal stays at each call site on purpose: it is a
    /// mechanical field copy, and a struct literal must be exhaustive, so a new
    /// [`ClassHead`] field fails to compile at BOTH sites. This walk has no such
    /// guard — a rule added to one copy and not the other would just silently
    /// erase differently in expression position — which is why it, and not the
    /// literal, is the half worth sharing.
    fn class_parts(
        &mut self,
        head: &ClassHead,
        decorators: Option<Run<Decorator>>,
        super_class: Option<ExprId>,
        body: &ClassBody,
    ) -> Result<Option<ClassParts>, CompileError> {
        refuse_decorators(decorators)?;
        let erased_head = self.class_head(head);
        let new_super_class = match super_class {
            Some(expr) => self.expr_ref(expr)?.map(Some),
            None => None,
        };
        let new_body = self.class_body(body)?;
        if !erased_head && new_super_class.is_none() && new_body.is_none() {
            return Ok(None);
        }
        Ok(Some(ClassParts {
            super_class: new_super_class.unwrap_or(super_class),
            body: new_body.unwrap_or(*body),
        }))
    }

    fn class_declaration(
        &mut self,
        class: &ClassDeclaration,
    ) -> Result<Option<ClassDeclaration>, CompileError> {
        let view = self.view();
        let head = ClassHead {
            span: class.span,
            r#abstract: class.r#abstract,
            id_end: class.id.as_ref().map(|id| id.span.end),
            type_parameters: class.type_parameters.as_ref().map(|tp| tp.span),
            super_class_end: class.super_class.map(|s| view.expr(s).span.end),
            super_type_parameters: class.super_type_parameters.as_ref().map(|tp| tp.span),
            implements: class.implements,
        };
        let Some(parts) =
            self.class_parts(&head, class.decorators, class.super_class, &class.body)?
        else {
            return Ok(None);
        };
        Ok(Some(ClassDeclaration {
            r#abstract: false,
            implements: Run::EMPTY,
            type_parameters: None,
            super_type_parameters: None,
            super_class: parts.super_class,
            body: parts.body,
            ..*class
        }))
    }

    fn class_expression(
        &mut self,
        class: &ClassExpression,
        class_span: Span,
    ) -> Result<Option<ClassExpression>, CompileError> {
        let view = self.view();
        let head = ClassHead {
            span: class_span,
            r#abstract: class.r#abstract,
            id_end: class.id.as_ref().map(|id| id.span.end),
            type_parameters: class.type_parameters.as_ref().map(|tp| tp.span),
            super_class_end: class.super_class.map(|s| view.expr(s).span.end),
            super_type_parameters: class.super_type_parameters.as_ref().map(|tp| tp.span),
            implements: class.implements,
        };
        let Some(parts) =
            self.class_parts(&head, class.decorators, class.super_class, &class.body)?
        else {
            return Ok(None);
        };
        Ok(Some(ClassExpression {
            r#abstract: false,
            implements: Run::EMPTY,
            type_parameters: None,
            super_type_parameters: None,
            super_class: parts.super_class,
            body: parts.body,
            ..*class
        }))
    }

    /// Record the erased regions of a class header (`abstract`, `<T>`,
    /// `extends Base<T>`'s type arguments, `implements …`). Returns whether
    /// anything was erased.
    fn class_head(&mut self, class: &ClassHead) -> bool {
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
        let last_implements = self
            .view()
            .heritages(class.implements)
            .last()
            .map(|last| last.span.end);
        if let Some(last_end) = last_implements {
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
            self.drop_region(Span::new(header_end, last_end));
        }
        class.r#abstract
            || class.type_parameters.is_some()
            || class.super_type_parameters.is_some()
            || !class.implements.is_empty()
    }

    fn class_body(&mut self, body: &ClassBody) -> Result<Option<ClassBody>, CompileError> {
        let members = rebuild_run(
            self,
            body.body,
            |view, run| view.class_members(run),
            |eraser, member| eraser.class_member(&member),
        )?;
        Ok(members.map(|members| ClassBody {
            body: members,
            span: body.span,
        }))
    }

    fn class_member(&mut self, member: &ClassMember) -> Result<Item<ClassMember>, CompileError> {
        Ok(match member {
            // `[key: string]: T` — a pure type construct, but the oracle's strip
            // pass has no case for it and its transform then *crashes*. Refuse
            // rather than guess at output the oracle cannot produce. A decorated one
            // hits the oracle's decorator error first, so it refuses as a decorator.
            ClassMember::IndexSignature(sig) => {
                refuse_decorators(sig.decorators)?;
                return unsupported(Refusal::TsIndexSignature);
            }
            ClassMember::StaticBlock(block) => {
                refuse_decorators(block.decorators)?;
                match self.statements(block.body)? {
                    None => Item::Keep,
                    Some(body) => Item::Replace(ClassMember::StaticBlock(StaticBlock {
                        decorators: block.decorators,
                        body,
                        span: block.span,
                    })),
                }
            }
            ClassMember::MethodDefinition(method) => self.method_definition(method)?,
            ClassMember::PropertyDefinition(prop) => self.property_definition(prop)?,
        })
    }

    fn method_definition(
        &mut self,
        method: &MethodDefinition,
    ) -> Result<Item<ClassMember>, CompileError> {
        refuse_decorators(method.decorators)?;
        // An `abstract` member declares no runtime behavior — the oracle drops it.
        if method.r#abstract {
            self.drop_region(method.span);
            return Ok(Item::Drop);
        }
        // A bodiless, non-abstract method is an overload signature (or an
        // ambient member outside a `declare` class). The oracle has no case for
        // it: the signature survives its strip pass and then collides with the
        // implementation (`duplicate_class_field`), or prints as invalid JS.
        // `is_bodyless` mirrors the wire writer's `TSDeclareMethod` predicate.
        let func = method.value;
        if func.body.body.is_empty() && func.body.span.start == func.body.span.end {
            return unsupported(Refusal::TsOverloadSignature);
        }
        let erased_head = method.accessibility.is_some()
            || method.r#override
            || method.optional
            || func.type_parameters.is_some();
        if erased_head {
            let key_span = self.view().expr(method.key).span;
            if method.accessibility.is_some() || method.r#override {
                self.drop_region(Span::new(method.span.start, key_span.start));
            }
            // The `?` marker and the `<T>` clause both sit between the key and
            // the parameter list's `(`.
            if method.optional || func.type_parameters.is_some() {
                self.drop_region(Span::new(key_span.end, func.params_start));
            }
        }
        let key = self.expr_ref(method.key)?;
        let value = self.function_expression(&func, method.kind == MethodKind::Constructor)?;
        if !erased_head && key.is_none() && value.is_none() {
            return Ok(Item::Keep);
        }
        Ok(Item::Replace(ClassMember::MethodDefinition(
            MethodDefinition {
                key: key.unwrap_or(method.key),
                value: value.unwrap_or(func),
                accessibility: None,
                r#override: false,
                r#abstract: false,
                optional: false,
                ..*method
            },
        )))
    }

    fn property_definition(
        &mut self,
        prop: &PropertyDefinition,
    ) -> Result<Item<ClassMember>, CompileError> {
        refuse_decorators(prop.decorators)?;
        // `declare x: T` — an ambient field, dropped by the oracle's `ClassBody`
        // visitor.
        if prop.declare {
            self.drop_region(prop.span);
            return Ok(Item::Drop);
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
        let key_span = self.view().expr(prop.key).span;
        let erased_modifiers = prop.accessibility.is_some() || prop.readonly || prop.r#override;
        if erased_modifiers {
            self.drop_region(Span::new(prop.span.start, key_span.start));
        }
        // `?`/`!` and `: T` form one contiguous tail between the key and the
        // initializer (or the member's end).
        let erased_tail = prop.modifier != PropertyModifier::None || prop.type_annotation.is_some();
        if erased_tail {
            let tail_end = prop
                .value
                .map_or(prop.span.end, |value| self.view().expr(value).span.start);
            self.drop_region(Span::new(key_span.end, tail_end));
        }
        let key = self.expr_ref(prop.key)?;
        let value = match prop.value {
            Some(value) => self.expr_ref(value)?.map(Some),
            None => None,
        };
        if !erased_modifiers && !erased_tail && key.is_none() && value.is_none() {
            return Ok(Item::Keep);
        }
        Ok(Item::Replace(ClassMember::PropertyDefinition(
            PropertyDefinition {
                key: key.unwrap_or(prop.key),
                value: value.unwrap_or(prop.value),
                type_annotation: None,
                accessibility: None,
                readonly: false,
                r#override: false,
                r#abstract: false,
                modifier: PropertyModifier::None,
                ..*prop
            },
        )))
    }

    // ── Expressions ────────────────────────────────────────────────────────

    /// The expression inventory. Exhaustive on purpose — no catch-all arm.
    ///
    /// The five TypeScript wrappers **unwrap to their inner expression**
    /// (`x as T` → `x`, `x!` → `x`, `x satisfies T` → `x`, `<T>x` → `x`,
    /// `f<T>` → `f`), and so does the internal `CommentBoundPair` wrapper — the oracle's
    /// AST has no such node (it parses without `preserveParens`), so
    /// `/** @type {T} */ (1)` is simply `1` with a leading comment there; keeping
    /// tsv's wrapper would print the parens the oracle drops *and* block the
    /// static fold. An unwrapped inner expression that is itself unchanged is
    /// yielded by id ([`Edge::Id`]). Patterns and binding identifiers
    /// **field-drop** their annotations and `?`/`!` markers.
    ///
    /// Parens are not a hazard: every tree the compiler reads is paren-free — a
    /// `{#snippet}` head included, whose `preserveParens` parse is kept for the wire
    /// alone (`Root::snippet_wire_parameters`) — and the printer re-derives them from
    /// precedence, exactly as the oracle's printer does — so `(x as T).y` erases to
    /// `x.y` and `(a + b as T) * c` keeps the parens it needs.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per variant — the exhaustive inventory is the safety argument"
    )]
    fn expr(&mut self, expr: &Expression) -> Result<Option<Edge>, CompileError> {
        let span = expr.span;
        Ok(match expr.kind {
            // ── The five TypeScript wrappers: unwrap to inner ──────────────
            ExpressionKind::TSAsExpression(node) => {
                let inner_end = self.view().expr(node.expression).span.end;
                self.drop_region(Span::new(inner_end, span.end));
                Some(self.inner(node.expression)?)
            }
            ExpressionKind::TSSatisfiesExpression(node) => {
                let inner_end = self.view().expr(node.expression).span.end;
                self.drop_region(Span::new(inner_end, span.end));
                Some(self.inner(node.expression)?)
            }
            ExpressionKind::TSNonNullExpression(node) => {
                let inner_end = self.view().expr(node.expression).span.end;
                self.drop_region(Span::new(inner_end, span.end));
                Some(self.inner(node.expression)?)
            }
            ExpressionKind::TSInstantiationExpression(node) => {
                let inner_end = self.view().expr(node.expression).span.end;
                self.drop_region(Span::new(inner_end, span.end));
                Some(self.inner(node.expression)?)
            }
            ExpressionKind::TSTypeAssertion(node) => {
                let inner_start = self.view().expr(node.expression).span.start;
                self.drop_region(Span::new(span.start, inner_start));
                Some(self.inner(node.expression)?)
            }

            ExpressionKind::TSParameterProperty(node) => Some(self.parameter_property(node, span)?),

            // ── Binding identifiers ────────────────────────────────────────
            ExpressionKind::Identifier(id) => self
                .identifier(&id)?
                .map(|id| Edge::Row(Expression::from_identifier(id))),

            // ── Patterns ───────────────────────────────────────────────────
            ExpressionKind::ObjectPattern(pattern) => {
                self.refuse_extra_decorators(pattern.extra)?;
                let annotation = pattern.type_annotation(self.view()).map(|a| a.span);
                self.pattern_tail(span, pattern.optional, annotation);
                let properties = rebuild_run(
                    self,
                    pattern.properties,
                    |view, run| view.pat_props(run),
                    |eraser, p| Ok(eraser.object_pattern_property(&p)?.into()),
                )?;
                let erased = pattern.optional || annotation.is_some();
                if !erased && properties.is_none() {
                    None
                } else {
                    // Decorators were refused above, so the extra row held only the
                    // erased annotation: clearing it drops nothing that prints.
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::ObjectPattern(ObjectPattern {
                            properties: properties.unwrap_or(pattern.properties),
                            optional: false,
                            extra: None,
                        }),
                    }))
                }
            }
            ExpressionKind::ArrayPattern(pattern) => {
                self.refuse_extra_decorators(pattern.extra)?;
                let annotation = pattern.type_annotation(self.view()).map(|a| a.span);
                self.pattern_tail(span, pattern.optional, annotation);
                let elements = self.opt_id_run(pattern.elements)?;
                let erased = pattern.optional || annotation.is_some();
                if !erased && elements.is_none() {
                    None
                } else {
                    // Decorators refused above, as for the object pattern.
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::ArrayPattern(ArrayPattern {
                            elements: elements.unwrap_or(pattern.elements),
                            optional: false,
                            extra: None,
                        }),
                    }))
                }
            }
            ExpressionKind::AssignmentPattern(pattern) => {
                refuse_decorators(pattern.decorators)?;
                let left = self.expr_ref(pattern.left)?;
                let right = self.expr_ref(pattern.right)?;
                if left.is_none() && right.is_none() {
                    None
                } else {
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::AssignmentPattern(AssignmentPattern {
                            left: left.unwrap_or(pattern.left),
                            right: right.unwrap_or(pattern.right),
                            ..pattern
                        }),
                    }))
                }
            }
            ExpressionKind::RestElement(rest) => self
                .rest_element(&rest)?
                .map(|rest| Edge::Row(Expression::from_rest_element(rest))),

            // ── Calls carry type arguments ─────────────────────────────────
            ExpressionKind::CallExpression(call) => {
                if let Some(type_args) = call.type_args {
                    // `f /* c */ <T>(x)` — the `<T>` list is detached from the
                    // callee, so the window reaches back to it.
                    let view = self.view();
                    let (anchor, region) = (
                        view.expr(call.callee).span.start,
                        view.type_args(type_args).span,
                    );
                    self.drop_region_from(anchor, region);
                }
                let callee = self.expr_ref(call.callee)?;
                let arguments = self.id_run(call.arguments)?;
                if call.type_args.is_none() && callee.is_none() && arguments.is_none() {
                    None
                } else {
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::CallExpression(tsv_ts::ast::CallExpression {
                            callee: callee.unwrap_or(call.callee),
                            type_args: None,
                            arguments: arguments.unwrap_or(call.arguments),
                            ..call
                        }),
                    }))
                }
            }
            ExpressionKind::NewExpression(new) => {
                if let Some(type_args) = new.type_args {
                    let view = self.view();
                    let (anchor, region) = (
                        view.expr(new.callee).span.start,
                        view.type_args(type_args).span,
                    );
                    self.drop_region_from(anchor, region);
                }
                let callee = self.expr_ref(new.callee)?;
                let arguments = self.id_run(new.arguments)?;
                if new.type_args.is_none() && callee.is_none() && arguments.is_none() {
                    None
                } else {
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::NewExpression(tsv_ts::ast::NewExpression {
                            callee: callee.unwrap_or(new.callee),
                            type_args: None,
                            arguments: arguments.unwrap_or(new.arguments),
                        }),
                    }))
                }
            }
            ExpressionKind::TaggedTemplateExpression(id) => {
                let tagged = *self.view().tagged_tpl(id);
                if let Some(type_arguments) = &tagged.type_arguments {
                    let anchor = self.view().expr(tagged.tag).span.start;
                    self.drop_region_from(anchor, type_arguments.span);
                }
                let tag = self.expr_ref(tagged.tag)?;
                let quasi = self.template_literal(&tagged.quasi)?;
                if tagged.type_arguments.is_none() && tag.is_none() && quasi.is_none() {
                    None
                } else {
                    let new = push(
                        self.stores,
                        TaggedTemplateExpression {
                            tag: tag.unwrap_or(tagged.tag),
                            type_arguments: None,
                            quasi: quasi.unwrap_or(tagged.quasi),
                            ..tagged
                        },
                    );
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::TaggedTemplateExpression(new),
                    }))
                }
            }

            // ── Functions and classes ──────────────────────────────────────
            ExpressionKind::ArrowFunctionExpression(id) => {
                let arrow = *self.view().arrow_fn(id);
                match self.arrow(&arrow, span)? {
                    None => None,
                    Some(new) => {
                        let new = push(self.stores, new);
                        Some(Edge::Row(Expression {
                            span,
                            kind: ExpressionKind::ArrowFunctionExpression(new),
                        }))
                    }
                }
            }
            ExpressionKind::FunctionExpression(id) => {
                let func = *self.view().fn_expr(id);
                match self.function_expression(&func, false)? {
                    None => None,
                    Some(new) => {
                        let id = push(self.stores, new);
                        Some(Edge::Row(Expression::from_function_expression(
                            new.span, id,
                        )))
                    }
                }
            }
            ExpressionKind::ClassExpression(id) => {
                let class = *self.view().class_expr(id);
                match self.class_expression(&class, span)? {
                    None => None,
                    Some(new) => {
                        let new = push(self.stores, new);
                        Some(Edge::Row(Expression {
                            span,
                            kind: ExpressionKind::ClassExpression(new),
                        }))
                    }
                }
            }

            // ── Plain recursion ────────────────────────────────────────────
            ExpressionKind::ObjectExpression(obj) => rebuild_run(
                self,
                obj.properties,
                |view, run| view.obj_props(run),
                |eraser, p| Ok(eraser.object_property(&p)?.into()),
            )?
            .map(|properties| {
                Edge::Row(Expression {
                    span,
                    kind: ExpressionKind::ObjectExpression(tsv_ts::ast::ObjectExpression {
                        properties,
                        ..obj
                    }),
                })
            }),
            ExpressionKind::ArrayExpression(arr) => {
                self.opt_id_run(arr.elements)?.map(|elements| {
                    Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::ArrayExpression(tsv_ts::ast::ArrayExpression {
                            elements,
                            ..arr
                        }),
                    })
                })
            }
            ExpressionKind::UnaryExpression(unary) => {
                self.expr_ref(unary.argument)?.map(|argument| {
                    Edge::Row(Expression::from_unary_expression(
                        tsv_ts::ast::UnaryExpression { argument, ..unary },
                    ))
                })
            }
            ExpressionKind::UpdateExpression(update) => {
                self.expr_ref(update.argument)?.map(|argument| {
                    Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::UpdateExpression(tsv_ts::ast::UpdateExpression {
                            argument,
                            ..update
                        }),
                    })
                })
            }
            ExpressionKind::BinaryExpression(binary) => {
                let left = self.expr_ref(binary.left)?;
                let right = self.expr_ref(binary.right)?;
                if left.is_none() && right.is_none() {
                    None
                } else {
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::BinaryExpression(tsv_ts::ast::BinaryExpression {
                            left: left.unwrap_or(binary.left),
                            right: right.unwrap_or(binary.right),
                            // `relexes_as_type_arguments` rides along: it is a claim about
                            // the SOURCE bytes this rewrite invalidates, and inheriting is the
                            // SAFE direction — a stale `true` costs a redundant paren pair, a
                            // stale `false` an output nothing reparses.
                            ..binary
                        }),
                    }))
                }
            }
            ExpressionKind::MemberExpression(member) => {
                let object = self.expr_ref(member.object)?;
                let property = self.expr_ref(member.property)?;
                if object.is_none() && property.is_none() {
                    None
                } else {
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::MemberExpression(tsv_ts::ast::MemberExpression {
                            object: object.unwrap_or(member.object),
                            property: property.unwrap_or(member.property),
                            ..member
                        }),
                    }))
                }
            }
            ExpressionKind::ConditionalExpression(cond) => {
                let test = self.expr_ref(cond.test)?;
                let consequent = self.expr_ref(cond.consequent)?;
                let alternate = self.expr_ref(cond.alternate)?;
                if test.is_none() && consequent.is_none() && alternate.is_none() {
                    None
                } else {
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::ConditionalExpression(
                            tsv_ts::ast::ConditionalExpression {
                                test: test.unwrap_or(cond.test),
                                consequent: consequent.unwrap_or(cond.consequent),
                                alternate: alternate.unwrap_or(cond.alternate),
                            },
                        ),
                    }))
                }
            }
            ExpressionKind::SpreadElement(spread) => {
                self.expr_ref(spread.argument)?.map(|argument| {
                    Edge::Row(Expression::from_spread_element(
                        tsv_ts::ast::SpreadElement {
                            argument,
                            span: spread.span,
                        },
                    ))
                })
            }
            ExpressionKind::TemplateLiteral(template) => self
                .template_literal(&template)?
                .map(|template| Edge::Row(Expression::from_template_literal(span, template))),
            ExpressionKind::AwaitExpression(node) => {
                self.expr_ref(node.argument)?.map(|argument| {
                    Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::AwaitExpression(tsv_ts::ast::AwaitExpression {
                            argument,
                        }),
                    })
                })
            }
            ExpressionKind::YieldExpression(node) => match node.argument {
                Some(argument) => self.expr_ref(argument)?.map(|argument| {
                    Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::YieldExpression(tsv_ts::ast::YieldExpression {
                            argument: Some(argument),
                            ..node
                        }),
                    })
                }),
                None => None,
            },
            ExpressionKind::SequenceExpression(seq) => {
                self.id_run(seq.expressions)?.map(|expressions| {
                    Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::SequenceExpression(tsv_ts::ast::SequenceExpression {
                            expressions,
                        }),
                    })
                })
            }
            ExpressionKind::AssignmentExpression(assign) => {
                let left = self.expr_ref(assign.left)?;
                let right = self.expr_ref(assign.right)?;
                if left.is_none() && right.is_none() {
                    None
                } else {
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::AssignmentExpression(
                            tsv_ts::ast::AssignmentExpression {
                                left: left.unwrap_or(assign.left),
                                right: right.unwrap_or(assign.right),
                                ..assign
                            },
                        ),
                    }))
                }
            }
            ExpressionKind::ImportExpression(import) => {
                let source = self.expr_ref(import.source)?;
                let options = match import.options {
                    Some(options) => self.expr_ref(options)?.map(Some),
                    None => None,
                };
                if source.is_none() && options.is_none() {
                    None
                } else {
                    Some(Edge::Row(Expression {
                        span,
                        kind: ExpressionKind::ImportExpression(tsv_ts::ast::ImportExpression {
                            source: source.unwrap_or(import.source),
                            options: options.unwrap_or(import.options),
                            ..import
                        }),
                    }))
                }
            }
            // `/** @type {T} */ (expr)` — an internal-only wrapper recording the
            // cast's semantically-required parens. The oracle has NO such node
            // (it parses without `preserveParens`), so its AST is the inner
            // expression carrying the JSDoc as a detached leading comment: it
            // prints `= /** @type {T} */ 1` and folds the `1`. Unwrap to match on
            // both counts. Valid JS, so this is NOT a TypeScript erasure —
            // it records no region and never trips the `lang="ts"` gate.
            //
            // The wrapper OWNS its JSDoc comment (`Comment::owned_by_node`), and
            // dropping the wrapper leaves that comment printed by nothing. The
            // compile path releases it back to the positional machinery — see
            // `transform_server::collect_script_comments`.
            ExpressionKind::CommentBoundPair(cast) => {
                let inner = self.view().comment_pair(cast).inner;
                Some(self.inner(inner)?)
            }
            ExpressionKind::ParenthesizedExpression(paren) => {
                let contents = *paren.unreachable_contents(self.view());
                self.expr(&contents)?
            }

            // ── Leaves ─────────────────────────────────────────────────────
            ExpressionKind::Literal(_)
            | ExpressionKind::PrivateIdentifier(_)
            | ExpressionKind::RegexLiteral(_)
            | ExpressionKind::ThisExpression(_)
            | ExpressionKind::Super(_)
            | ExpressionKind::MetaProperty(_) => None,
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
    /// its name out-of-band (`name_len: 0`) and may steal the span of the node it
    /// replaces, so "span extends past the name" is not a TypeScript signal.
    ///
    /// Decorators are refused first, so the cleared extra row held only the
    /// annotation.
    fn identifier(&mut self, id: &Identifier) -> Result<Option<Identifier>, CompileError> {
        self.refuse_extra_decorators(id.extra)?;
        // A plain *reference* never populates `extra` — the overwhelmingly
        // common case exits here on one `Option::is_none()` and a bool.
        let annotation = id.type_annotation(self.view()).map(|a| a.span);
        if !id.optional && annotation.is_none() {
            return Ok(None);
        }
        self.drop_region(Span::new(id.name_span().end, tail_end(id.span, annotation)));
        Ok(Some(Identifier {
            optional: false,
            extra: None,
            ..*id
        }))
    }

    /// Record the erased tail of a destructuring pattern (`{a}?: T`): the `?` and
    /// the annotation both trail the pattern's closing bracket. The window reaches
    /// back over any trivia to that bracket, so a comment between them
    /// (`{a} /* c */ : T`) can't leak.
    fn pattern_tail(&mut self, span: Span, optional: bool, type_annotation: Option<Span>) {
        if !optional && type_annotation.is_none() {
            return;
        }
        let end = tail_end(span, type_annotation);
        // The `?` sits before the annotation; without one it is the tail's last byte.
        let start = type_annotation.map_or_else(
            || end.saturating_sub(1).max(span.start),
            |annotation| annotation.start,
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
        node: TSParameterProperty,
        span: Span,
    ) -> Result<Edge, CompileError> {
        if self.constructor_params && (node.readonly || node.accessibility.is_some()) {
            return unsupported(Refusal::TsParameterProperty);
        }
        let parameter_start = self.view().expr(node.parameter).span.start;
        self.drop_region(Span::new(span.start, parameter_start));
        self.inner(node.parameter)
    }

    /// A rest element's erasure. Its extra row carries only an annotation (a rest
    /// element takes no decorator), so a rebuild clears it.
    fn rest_element(&mut self, rest: &RestElement) -> Result<Option<RestElement>, CompileError> {
        let annotation = rest.type_annotation(self.view()).map(|a| a.span);
        self.pattern_tail(rest.span, rest.optional, annotation);
        let argument = self.expr_ref(rest.argument)?;
        if !rest.optional && annotation.is_none() && argument.is_none() {
            return Ok(None);
        }
        Ok(Some(RestElement {
            argument: argument.unwrap_or(rest.argument),
            optional: false,
            extra: None,
            span: rest.span,
        }))
    }

    fn object_property(
        &mut self,
        property: &ObjectProperty,
    ) -> Result<Option<ObjectProperty>, CompileError> {
        Ok(match property {
            ObjectProperty::Property(prop) => self.property(prop)?.map(ObjectProperty::Property),
            ObjectProperty::SpreadElement(spread) => {
                self.expr_ref(spread.argument)?.map(|argument| {
                    ObjectProperty::SpreadElement(tsv_ts::ast::SpreadElement {
                        argument,
                        span: spread.span,
                    })
                })
            }
        })
    }

    fn object_pattern_property(
        &mut self,
        property: &ObjectPatternProperty,
    ) -> Result<Option<ObjectPatternProperty>, CompileError> {
        Ok(match property {
            ObjectPatternProperty::Property(prop) => {
                self.property(prop)?.map(ObjectPatternProperty::Property)
            }
            ObjectPatternProperty::RestElement(rest) => self
                .rest_element(rest)?
                .map(ObjectPatternProperty::RestElement),
        })
    }

    fn property(&mut self, prop: &Property) -> Result<Option<Property>, CompileError> {
        let erased_key = self.expr_ref(prop.key)?;
        let erased_value = self.expr_ref(prop.value)?;
        let changed = erased_key.is_some() || erased_value.is_some();
        let key = erased_key.unwrap_or(prop.key);
        let value = erased_value.unwrap_or(prop.value);
        if let Some(shorthand) = self.same_name_shorthand(prop, key, value) {
            return Ok(Some(shorthand));
        }
        Ok(changed.then_some(Property {
            key,
            value,
            ..*prop
        }))
    }

    /// A longhand `init` property whose key and value name the same identifier
    /// (`{ a: a }`, `{ a: a = 1 }` in a pattern), rebuilt as the shorthand the oracle
    /// prints. esrap's `Property` visitor collapses exactly that shape to its value
    /// (`!computed && kind === 'init' && key.type === 'Identifier' && value.type ===
    /// 'Identifier' && key.name === value.name`, reading an `AssignmentPattern`'s
    /// `left`), and it runs after Svelte's transforms, so a value those rewrite (a
    /// `$derived` read, a store) no longer matches — which the transform reproduces
    /// by expanding the shorthand again. Names compare decoded, as acorn's `name`
    /// does. The dropped `key:` gap is recorded as a comment window like an erased
    /// region, since the shorthand leaves no window for a comment there.
    fn same_name_shorthand(
        &mut self,
        prop: &Property,
        key: ExprId,
        value: ExprId,
    ) -> Option<Property> {
        if prop.shorthand || prop.computed || prop.method || prop.kind != PropertyKind::Init {
            return None;
        }
        let view = self.view();
        let ExpressionKind::Identifier(key_id) = view.expr(key).kind else {
            return None;
        };
        let base = match view.expr(value).kind {
            ExpressionKind::AssignmentPattern(assign) => assign.left,
            _ => value,
        };
        let ExpressionKind::Identifier(base_id) = view.expr(base).kind else {
            return None;
        };
        if key_id.name(self.source, view) != base_id.name(self.source, view) {
            return None;
        }
        self.push_window(key_id.span.end, base_id.span.start);
        Some(Property {
            key: base,
            value,
            shorthand: true,
            ..*prop
        })
    }

    fn template_literal(
        &mut self,
        template: &TemplateLiteral,
    ) -> Result<Option<TemplateLiteral>, CompileError> {
        Ok(self
            .id_run(template.expressions)?
            .map(|expressions| TemplateLiteral {
                expressions,
                ..*template
            }))
    }
}
