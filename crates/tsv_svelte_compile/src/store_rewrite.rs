//! Store subscription (and script-position `$derived` read) rewriting for the
//! instance script — the script-position analog of the template value walk
//! (`template_value`'s `rewrite_template_value`).
//!
//! A tree→tree pass over the (already type-erased and rune-rewritten) instance
//! body that rewrites every `$name` store access the oracle's SSR transform
//! lowers to a runtime call:
//!
//! - a **read** `$count` → `$.store_get(($$store_subs ??= {}), '$count', count)`
//!   (`Identifier.js` → `serialize_get_binding`) — at any depth, in any value
//!   position (a declarator init, a function body, a `$.derived(() => …)` thunk);
//! - an **assignment** `$count = v` → `$.store_set(count, <v>)` and a compound
//!   `$count += v` → `$.store_set(count, $.store_get(…) + <v>)`, reconstructing
//!   the binary via [the oracle's `build_assignment_value`](https://github.com/sveltejs/svelte)
//!   (`AssignmentExpression.js`);
//! - an **update** `$count++` / `++$count` / `$count--` / `--$count` →
//!   `$.update_store[_pre]((…), '$count', count[, -1])` (`UpdateExpression.js`).
//!
//! It also rewrites a plain **`$derived` read** in a script position (a function
//! body, a top-level initializer, a `$.derived(() => …)` thunk) to the
//! derived-thunk call `d()` — the same lowering the template value walk applies,
//! extended to the script. A read is rewritten only in a genuine value position:
//! a name-only position (member property / object key) is never descended and a
//! binding-position id is skipped, so `let d = …` / `{ d: 1 }` / `o.d` stay
//! verbatim. A **write** to a derived (`d = v` / `d++`) and a *shadowed* derived
//! name are refused upstream (the rune guard and `compile_server`), so they never
//! reach this pass.
//!
//! **Structural sharing.** Like [`crate::erase`], every entry point returns
//! `Option<T>`: `None` means *unchanged*, so a subtree with no store access is
//! never rebuilt and nothing is pushed. A rebuild is shallow (children are ids
//! and runs, so a rebuilt node copies its row with the changed edges swapped and
//! pushes it, never a subtree), and a rebuilt list is the lazy-prefix rebuild
//! ([`rebuild_run`]).
//!
//! **Exhaustive by construction.** The `Statement` and `Expression` matches have
//! no catch-all — a new AST variant fails compilation here rather than silently
//! passing a store access through unrewritten. The one subtlety versus `erase`:
//! this pass must respect **name-only positions**. A non-computed member property
//! (`obj.$foo`) and a non-computed object/class key (`{ $foo: v }`) are *names*,
//! not reads, so they are never descended — descending would rewrite the name to
//! `$.store_get(…)`, corrupting the output.
//!
//! **Refuse, don't guess.** A store write the oracle lowers through a shape this
//! pass does not implement — a member write (`$obj.x = 5` → `$.store_mutate`), a
//! destructuring write (`[$count] = …` → an IIFE), or a subscription whose base
//! is bound in a nested scope (the oracle's `store_invalid_scoped_subscription`)
//! — is a [`Refusal`], never guessed output.
//!
//! This runs over the FINAL synthetic body (after erasure + rune rewrites), so a
//! store read inside a `$derived(arg)` → `$.derived(() => arg)` thunk is reached
//! through the synthetic arrow and rewritten too.

use tsv_lang::Span;
use tsv_lang::store::Run;
use tsv_ts::ast::{
    ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression, AssignmentOperator,
    AssignmentPattern, AstStores, AstView, BinaryOperator, BlockStatement, CatchClause, ClassBody,
    ClassDeclaration, ClassExpression, ClassMember, ExprId, Expression, ExpressionKind,
    ForInOfLeft, ForInit, FunctionDeclaration, FunctionExpression, Identifier, LabeledStatement,
    MemberExpression, MethodDefinition, ObjectPattern, ObjectPatternProperty, ObjectProperty,
    Property, PropertyDefinition, RestElement, Statement, StatementKind, StaticBlock, StmtId,
    SwitchCase, TaggedTemplateExpression, TryStatement, UpdateOperator, VariableDeclaration,
    VariableDeclarator,
};

use crate::CompileError;
use crate::analyze::{NameSet, store_read_base};
use crate::build::{Builder, HasStores, Item, rebuild_run};
use crate::refusal::Refusal;

/// Rewrite every store access (and script-position `$derived` read) in `stmts`,
/// returning `None` when nothing changed (the caller keeps the input run). See the
/// [module docs](self). The minted `d()` reads take the callee's tight span
/// ([`Builder::call_expr`]), so they never sweep a carried script comment — no
/// comment gate is needed here.
pub(crate) fn rewrite_store_accesses(
    b: &mut Builder<'_>,
    source: &str,
    store_names: &NameSet,
    store_shadowed: &NameSet,
    derived_names: &NameSet,
    stmts: Run<StmtId>,
) -> Result<Option<Run<StmtId>>, CompileError> {
    let mut rewriter = StoreRewriter {
        b,
        source,
        store_names,
        store_shadowed,
        derived_names,
    };
    rewriter.statements(stmts)
}

struct StoreRewriter<'a, 'b> {
    b: &'a mut Builder<'b>,
    source: &'a str,
    store_names: &'a NameSet,
    store_shadowed: &'a NameSet,
    derived_names: &'a NameSet,
}

impl HasStores for StoreRewriter<'_, '_> {
    #[inline]
    fn stores(&mut self) -> &mut AstStores {
        self.b.stores
    }
}

/// The classification of an assignment/update target.
enum StoreTarget {
    /// A bare `$name` store identifier — the supported write target.
    Bare(String),
    /// Not a store write (a plain lvalue) — recurse for reads instead.
    NotStore,
}

fn unsupported<T>(reason: Refusal) -> Result<T, CompileError> {
    Err(CompileError::Unsupported(reason))
}

/// The compound assignment operator's underlying binary/logical operator (the
/// oracle's `operator.slice(0, -1)`). tsv models `||`/`&&`/`??` as
/// [`BinaryOperator`] variants, so `b.logical` and `b.binary` unify here.
fn compound_binary_op(op: AssignmentOperator) -> BinaryOperator {
    match op {
        AssignmentOperator::AddAssign => BinaryOperator::Plus,
        AssignmentOperator::SubtractAssign => BinaryOperator::Minus,
        AssignmentOperator::MultiplyAssign => BinaryOperator::Star,
        AssignmentOperator::DivideAssign => BinaryOperator::Slash,
        AssignmentOperator::RemainderAssign => BinaryOperator::Percent,
        AssignmentOperator::ExponentiateAssign => BinaryOperator::StarStar,
        AssignmentOperator::LeftShiftAssign => BinaryOperator::LeftShift,
        AssignmentOperator::RightShiftAssign => BinaryOperator::RightShift,
        AssignmentOperator::UnsignedRightShiftAssign => BinaryOperator::UnsignedRightShift,
        AssignmentOperator::BitwiseOrAssign => BinaryOperator::Pipe,
        AssignmentOperator::BitwiseXorAssign => BinaryOperator::Caret,
        AssignmentOperator::BitwiseAndAssign => BinaryOperator::Ampersand,
        AssignmentOperator::LogicalOrAssign => BinaryOperator::PipePipe,
        AssignmentOperator::LogicalAndAssign => BinaryOperator::AmpersandAmpersand,
        AssignmentOperator::NullishAssign => BinaryOperator::QuestionQuestion,
        // `=` is handled at the call site (the value is the bare right-hand side),
        // never reaching this mapping.
        AssignmentOperator::Assign => BinaryOperator::Plus,
    }
}

/// Peel to the root of an assignment/update member target (`$obj.a.b` → `$obj`),
/// through parenthesization — the position that decides whether the write hits a
/// store.
fn member_root(view: AstView<'_>, expr: ExprId) -> &Expression {
    let mut node = view.expr(expr);
    loop {
        match &node.kind {
            ExpressionKind::MemberExpression(m) => node = view.expr(m.object),
            _ => return node,
        }
    }
}

impl StoreRewriter<'_, '_> {
    fn view(&self) -> AstView<'_> {
        self.b.view()
    }

    // ── Store recognition ──────────────────────────────────────────────────

    /// The `$`-stripped base of a `$name` identifier that resolves to a top-level
    /// component binding (a store), else `None`. Mirrors `template_value`'s
    /// `bare_store_read`.
    ///
    /// The name is DECODED: a plain identifier is its span slice (the fast path —
    /// no pool string touched), while a unicode-escaped `$`-identifier
    /// (`$count` written `$count`) reads its decoded name from the pool, so it is
    /// read as the store the oracle sees (the oracle decodes `node.name`).
    fn store_base(&self, id: &Identifier) -> Option<String> {
        if id.escaped_name.is_some() {
            // An escaped `$`-identifier decodes to a store name the oracle treats
            // exactly as its plain spelling.
            let name = id.name(self.source, self.view());
            let base = store_read_base(name)?;
            return self.store_names.contains(base).then(|| base.to_string());
        }
        let start = id.span.start as usize;
        let name = &self.source[start..start + id.name_len as usize];
        let base = store_read_base(name)?;
        self.store_names.contains(base).then(|| base.to_string())
    }

    /// Whether `id` is a plain (non-escaped) read of a `$derived` binding — the
    /// script analog of [`template_value::is_bare_derived_read`](crate::template_value::is_bare_derived_read). Such a read rewrites to
    /// the derived-thunk call `d()`. An escaped derived read is refused by the
    /// rune guard before this pass, and a *shadowed* derived name is refused by
    /// the whole-compile check in `compile_server`, so any read reaching here that
    /// this returns `true` for is the derived binding itself.
    fn derived_read(&self, id: &Identifier) -> bool {
        if id.escaped_name.is_some() {
            return false;
        }
        let start = id.span.start as usize;
        let name = &self.source[start..start + id.name_len as usize];
        self.derived_names.contains(name)
    }

    /// Whether any assignment-target binding leaf inside `left` is a store base —
    /// used to detect a store buried in a destructuring pattern. Mirrors
    /// [`assign_target_roots`](crate::rune_guard::assign_target_roots)'s pattern
    /// recursion, but tests each leaf with [`store_base`](Self::store_base) so an
    /// ESCAPED `$name` (`[$count] = …`) is decoded and detected — where the
    /// span-identity `assign_target_roots` skips it. That makes an escaped
    /// destructuring store write refuse (`StoreDestructuringWrite`) exactly as the
    /// plain `[$count] = …` does, rather than fall through and corrupt the target.
    /// The TypeScript assignment-target wrappers are absent here (this pass runs
    /// over the post-erase body), so they need no arm.
    fn pattern_targets_store(&self, left: ExprId) -> bool {
        let view = self.view();
        match &view.expr(left).kind {
            ExpressionKind::Identifier(id) => self.store_base(id).is_some(),
            ExpressionKind::MemberExpression(m) => self.pattern_targets_store(m.object),
            ExpressionKind::ObjectPattern(obj) => {
                view.pat_props(obj.properties)
                    .iter()
                    .any(|prop| match prop {
                        ObjectPatternProperty::Property(p) => self.pattern_targets_store(p.value),
                        ObjectPatternProperty::RestElement(rest) => {
                            self.pattern_targets_store(rest.argument)
                        }
                    })
            }
            ExpressionKind::ArrayPattern(arr) => view
                .expr_opt_list(arr.elements)
                .iter()
                .flatten()
                .any(|&element| self.pattern_targets_store(element)),
            ExpressionKind::AssignmentPattern(a) => self.pattern_targets_store(a.left),
            ExpressionKind::RestElement(r) => self.pattern_targets_store(r.argument),
            _ => false,
        }
    }

    // ── Lists ──────────────────────────────────────────────────────────────

    /// An id list (call / `new` arguments, sequence and template expressions),
    /// rebuilt when an element changed.
    fn id_run(&mut self, run: Run<ExprId>) -> Result<Option<Run<ExprId>>, CompileError> {
        rebuild_run(
            self,
            run,
            |view, run| view.expr_list(run),
            |rewriter, id| Ok(rewriter.expr_ref(id)?.into()),
        )
    }

    /// An id list with holes (array and array-pattern elements), rebuilt when an
    /// element changed. A hole (`[, x]`) has nothing to rewrite.
    fn opt_id_run(
        &mut self,
        run: Run<Option<ExprId>>,
    ) -> Result<Option<Run<Option<ExprId>>>, CompileError> {
        rebuild_run(
            self,
            run,
            |view, run| view.expr_opt_list(run),
            |rewriter, element| {
                Ok(match element {
                    Some(id) => rewriter.expr_ref(id)?.map(Some).into(),
                    None => Item::Keep,
                })
            },
        )
    }

    /// A parameter list (held by value), rebuilt when a parameter changed.
    fn param_run(&mut self, run: Run<Expression>) -> Result<Option<Run<Expression>>, CompileError> {
        rebuild_run(
            self,
            run,
            |view, run| view.exprs(run),
            |rewriter, param| Ok(rewriter.expr(&param)?.into()),
        )
    }

    fn declarators(
        &mut self,
        run: Run<VariableDeclarator>,
    ) -> Result<Option<Run<VariableDeclarator>>, CompileError> {
        rebuild_run(
            self,
            run,
            |view, run| view.declarators(run),
            |rewriter, declarator| Ok(rewriter.variable_declarator(&declarator)?.into()),
        )
    }

    // ── Statements ─────────────────────────────────────────────────────────

    fn statements(&mut self, run: Run<StmtId>) -> Result<Option<Run<StmtId>>, CompileError> {
        rebuild_run(
            self,
            run,
            |view, run| view.stmt_list(run),
            |rewriter, id| Ok(rewriter.statement_ref(id)?.into()),
        )
    }

    /// The statement `id` names, rewritten and pushed — `None` when unchanged.
    fn statement_ref(&mut self, id: StmtId) -> Result<Option<StmtId>, CompileError> {
        let stmt = *self.view().stmt(id);
        Ok(self.statement(&stmt)?.map(|new| self.b.push(new)))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per variant — the exhaustive inventory is the safety argument"
    )]
    fn statement(&mut self, stmt: &Statement) -> Result<Option<Statement>, CompileError> {
        use tsv_ts::ast;
        // The header span: a rebuilt statement keeps the original's.
        let span = stmt.span;
        Ok(match stmt.kind {
            StatementKind::ExpressionStatement(s) => {
                self.expr_ref(s.expression)?.map(|expression| Statement {
                    span,
                    kind: StatementKind::ExpressionStatement(ast::ExpressionStatement {
                        expression,
                        ..s
                    }),
                })
            }
            StatementKind::VariableDeclaration(decl) => {
                self.declarators(decl.declarations)?.map(|declarations| {
                    Statement::from_variable_declaration(VariableDeclaration {
                        declarations,
                        ..decl
                    })
                })
            }
            StatementKind::ReturnStatement(s) => match s.argument {
                Some(argument) => self.expr_ref(argument)?.map(|argument| Statement {
                    span,
                    kind: StatementKind::ReturnStatement(ast::ReturnStatement {
                        argument: Some(argument),
                    }),
                }),
                None => None,
            },
            StatementKind::BlockStatement(block) => {
                self.block(&block)?.map(Statement::from_block_statement)
            }
            StatementKind::FunctionDeclaration(id) => {
                let func = *self.view().fn_decl(id);
                self.function_declaration(&func)?.map(|new| {
                    let id = self.b.push(new);
                    Statement::from_function_declaration(new.span, id)
                })
            }
            StatementKind::ClassDeclaration(id) => {
                let decl = *self.view().class_decl(id);
                let super_class = match decl.super_class {
                    Some(sc) => self.expr_ref(sc)?.map(Some),
                    None => None,
                };
                let body = self.class_body(&decl.body)?;
                if super_class.is_none() && body.is_none() {
                    None
                } else {
                    let new = ClassDeclaration {
                        super_class: super_class.unwrap_or(decl.super_class),
                        body: body.unwrap_or(decl.body),
                        ..decl
                    };
                    let id = self.b.push(new);
                    Some(Statement::from_class_declaration(new.span, id))
                }
            }
            StatementKind::IfStatement(s) => {
                let test = self.expr_ref(s.test)?;
                let consequent = self.statement_ref(s.consequent)?;
                let alternate = match s.alternate {
                    Some(alt) => self.statement_ref(alt)?.map(Some),
                    None => None,
                };
                if test.is_none() && consequent.is_none() && alternate.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::IfStatement(ast::IfStatement {
                            test: test.unwrap_or(s.test),
                            consequent: consequent.unwrap_or(s.consequent),
                            alternate: alternate.unwrap_or(s.alternate),
                        }),
                    })
                }
            }
            StatementKind::ForStatement(s) => {
                let init = match s.init {
                    Some(init) => {
                        let head = *self.view().for_head(init);
                        self.for_init(&head)?.map(|head| Some(self.b.push(head)))
                    }
                    None => None,
                };
                let test = match s.test {
                    Some(test) => self.expr_ref(test)?.map(Some),
                    None => None,
                };
                let update = match s.update {
                    Some(update) => self.expr_ref(update)?.map(Some),
                    None => None,
                };
                let body = self.statement_ref(s.body)?;
                if init.is_none() && test.is_none() && update.is_none() && body.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::ForStatement(ast::ForStatement {
                            init: init.unwrap_or(s.init),
                            test: test.unwrap_or(s.test),
                            update: update.unwrap_or(s.update),
                            body: body.unwrap_or(s.body),
                        }),
                    })
                }
            }
            StatementKind::ForInStatement(s) => {
                let left = self.for_left(s.left)?;
                let right = self.expr_ref(s.right)?;
                let body = self.statement_ref(s.body)?;
                if left.is_none() && right.is_none() && body.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::ForInStatement(ast::ForInStatement {
                            left: left.unwrap_or(s.left),
                            right: right.unwrap_or(s.right),
                            body: body.unwrap_or(s.body),
                        }),
                    })
                }
            }
            StatementKind::ForOfStatement(s) => {
                let left = self.for_left(s.left)?;
                let right = self.expr_ref(s.right)?;
                let body = self.statement_ref(s.body)?;
                if left.is_none() && right.is_none() && body.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::ForOfStatement(ast::ForOfStatement {
                            left: left.unwrap_or(s.left),
                            right: right.unwrap_or(s.right),
                            body: body.unwrap_or(s.body),
                            ..s
                        }),
                    })
                }
            }
            StatementKind::WhileStatement(s) => {
                let test = self.expr_ref(s.test)?;
                let body = self.statement_ref(s.body)?;
                if test.is_none() && body.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::WhileStatement(ast::WhileStatement {
                            test: test.unwrap_or(s.test),
                            body: body.unwrap_or(s.body),
                        }),
                    })
                }
            }
            // Unreachable in practice: a Svelte `<script>` is Module code, so it is
            // strict and the parser refuses `with` there.
            StatementKind::WithStatement(s) => {
                let object = self.expr_ref(s.object)?;
                let body = self.statement_ref(s.body)?;
                if object.is_none() && body.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::WithStatement(ast::WithStatement {
                            object: object.unwrap_or(s.object),
                            body: body.unwrap_or(s.body),
                        }),
                    })
                }
            }
            StatementKind::DoWhileStatement(s) => {
                let body = self.statement_ref(s.body)?;
                let test = self.expr_ref(s.test)?;
                if body.is_none() && test.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::DoWhileStatement(ast::DoWhileStatement {
                            body: body.unwrap_or(s.body),
                            test: test.unwrap_or(s.test),
                        }),
                    })
                }
            }
            StatementKind::SwitchStatement(s) => {
                let discriminant = self.expr_ref(s.discriminant)?;
                let cases = rebuild_run(
                    self,
                    s.cases,
                    |view, run| view.cases(run),
                    |rewriter, case| Ok(rewriter.switch_case(&case)?.into()),
                )?;
                if discriminant.is_none() && cases.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::SwitchStatement(ast::SwitchStatement {
                            discriminant: discriminant.unwrap_or(s.discriminant),
                            cases: cases.unwrap_or(s.cases),
                        }),
                    })
                }
            }
            StatementKind::TryStatement(id) => {
                // The payload is a row by id: a rebuild pushes a new row and points
                // the statement at it, leaving the input's row as it was.
                let s = *self.view().try_stmt(id);
                let block = self.block(&s.block)?;
                let handler = match s.handler {
                    Some(handler) => {
                        let clause = *self.view().catch(handler);
                        self.catch_clause(&clause)?
                            .map(|clause| Some(self.b.push(clause)))
                    }
                    None => None,
                };
                let finalizer = match s.finalizer {
                    Some(finalizer) => self.block(&finalizer)?.map(Some),
                    None => None,
                };
                if block.is_none() && handler.is_none() && finalizer.is_none() {
                    None
                } else {
                    let new = self.b.push(TryStatement {
                        block: block.unwrap_or(s.block),
                        handler: handler.unwrap_or(s.handler),
                        finalizer: finalizer.unwrap_or(s.finalizer),
                    });
                    Some(Statement {
                        span,
                        kind: StatementKind::TryStatement(new),
                    })
                }
            }
            StatementKind::ThrowStatement(s) => {
                self.expr_ref(s.argument)?.map(|argument| Statement {
                    span,
                    kind: StatementKind::ThrowStatement(ast::ThrowStatement { argument }),
                })
            }
            StatementKind::LabeledStatement(id) => {
                // A row by id, rebuilt into a new row as the `try` payload is.
                let s = *self.view().labeled_stmt(id);
                self.statement_ref(s.body)?.map(|body| {
                    let new = self.b.push(LabeledStatement { body, ..s });
                    Statement {
                        span,
                        kind: StatementKind::LabeledStatement(new),
                    }
                })
            }

            // No store-bearing children, or a statement kind that cannot appear
            // in the erased/rune-rewritten body this pass runs over: imports are
            // hoisted out, exports and TypeScript-only statements are refused or
            // erased upstream (a surviving one is caught by the erase self-check,
            // never a silent miss here). Exhaustive on purpose — a NEW statement
            // variant fails compilation rather than silently skipping the rewrite.
            StatementKind::BreakStatement(_)
            | StatementKind::ContinueStatement(_)
            | StatementKind::EmptyStatement(_)
            | StatementKind::DebuggerStatement(_)
            | StatementKind::ImportDeclaration(_)
            | StatementKind::ExportNamedDeclaration(_)
            | StatementKind::ExportDefaultDeclaration(_)
            | StatementKind::ExportAllDeclaration(_)
            | StatementKind::TSNamespaceExportDeclaration(_)
            | StatementKind::TSImportEqualsDeclaration(_)
            | StatementKind::TSExportAssignment(_)
            | StatementKind::TSTypeAliasDeclaration(_)
            | StatementKind::TSInterfaceDeclaration(_)
            | StatementKind::TSDeclareFunction(_)
            | StatementKind::TSEnumDeclaration(_)
            | StatementKind::TSModuleDeclaration(_) => None,
        })
    }

    fn for_init(&mut self, init: &ForInit) -> Result<Option<ForInit>, CompileError> {
        Ok(match *init {
            ForInit::VariableDeclaration(decl) => {
                self.declarators(decl.declarations)?.map(|declarations| {
                    ForInit::VariableDeclaration(VariableDeclaration {
                        declarations,
                        ..decl
                    })
                })
            }
            ForInit::Expression(expr) => self.expr_ref(expr)?.map(ForInit::Expression),
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
            ForInOfLeft::VariableDeclaration(decl) => {
                self.declarators(decl.declarations)?.map(|declarations| {
                    ForInOfLeft::VariableDeclaration(VariableDeclaration {
                        declarations,
                        ..decl
                    })
                })
            }
            ForInOfLeft::Pattern(pattern) => self.expr_ref(pattern)?.map(ForInOfLeft::Pattern),
        };
        Ok(new.map(|new| self.b.push(new)))
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
            Some(param) => self.expr(param)?.map(Some),
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

    fn variable_declarator(
        &mut self,
        declarator: &VariableDeclarator,
    ) -> Result<Option<VariableDeclarator>, CompileError> {
        // The `id` is a binding pattern — its binding NAMES are not reads, but a
        // default (`let { a = $count } = …`) is, so recurse a pattern for its
        // defaults. A plain-identifier binding (`let d = …`) has no read
        // sub-position and MUST be left alone: a top-level `$derived` name in
        // binding position would otherwise be rewritten to `d()`. (Nested /
        // shadowing binding names are refused by the derived-shadow check in
        // `compile_server`, so the only binding-position derived name reaching here
        // is a top-level declarator id.) A `$name` never appears in binding
        // position, so this is a no-op for the store rewrite.
        let id = match self.view().expr(declarator.id).kind {
            ExpressionKind::Identifier(_) => None,
            _ => self.expr_ref(declarator.id)?,
        };
        let init = match declarator.init {
            Some(init) => self.expr_ref(init)?.map(Some),
            None => None,
        };
        if id.is_none() && init.is_none() {
            return Ok(None);
        }
        Ok(Some(VariableDeclarator {
            id: id.unwrap_or(declarator.id),
            init: init.unwrap_or(declarator.init),
            ..*declarator
        }))
    }

    // ── Functions and classes ──────────────────────────────────────────────

    fn function_declaration(
        &mut self,
        func: &FunctionDeclaration,
    ) -> Result<Option<FunctionDeclaration>, CompileError> {
        let params = self.param_run(func.params)?;
        let body = self.block(&func.body)?;
        if params.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(FunctionDeclaration {
            params: params.unwrap_or(func.params),
            body: body.unwrap_or(func.body),
            ..*func
        }))
    }

    fn function_expression(
        &mut self,
        func: &FunctionExpression,
    ) -> Result<Option<FunctionExpression>, CompileError> {
        let params = self.param_run(func.params)?;
        let body = self.block(&func.body)?;
        if params.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(FunctionExpression {
            params: params.unwrap_or(func.params),
            body: body.unwrap_or(func.body),
            ..*func
        }))
    }

    fn arrow(
        &mut self,
        arrow: &ArrowFunctionExpression,
    ) -> Result<Option<ArrowFunctionExpression>, CompileError> {
        let params = self.param_run(arrow.params)?;
        let body = match arrow.body {
            ArrowFunctionBody::Expression(expr) => {
                self.expr_ref(expr)?.map(ArrowFunctionBody::Expression)
            }
            ArrowFunctionBody::BlockStatement(block) => {
                self.block(&block)?.map(ArrowFunctionBody::BlockStatement)
            }
        };
        if params.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(ArrowFunctionExpression {
            params: params.unwrap_or(arrow.params),
            body: body.unwrap_or(arrow.body),
            ..*arrow
        }))
    }

    fn class_expression(
        &mut self,
        class: &ClassExpression,
    ) -> Result<Option<ClassExpression>, CompileError> {
        let super_class = match class.super_class {
            Some(sc) => self.expr_ref(sc)?.map(Some),
            None => None,
        };
        let body = self.class_body(&class.body)?;
        if super_class.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(ClassExpression {
            super_class: super_class.unwrap_or(class.super_class),
            body: body.unwrap_or(class.body),
            ..*class
        }))
    }

    fn class_body(&mut self, body: &ClassBody) -> Result<Option<ClassBody>, CompileError> {
        let members = rebuild_run(
            self,
            body.body,
            |view, run| view.class_members(run),
            |rewriter, member| Ok(rewriter.class_member(&member)?.into()),
        )?;
        Ok(members.map(|members| ClassBody {
            body: members,
            span: body.span,
        }))
    }

    fn class_member(&mut self, member: &ClassMember) -> Result<Option<ClassMember>, CompileError> {
        Ok(match member {
            ClassMember::MethodDefinition(m) => {
                // A non-computed key is a name, not a read.
                let key = if m.computed {
                    self.expr_ref(m.key)?
                } else {
                    None
                };
                let value = self.function_expression(&m.value)?;
                if key.is_none() && value.is_none() {
                    None
                } else {
                    Some(ClassMember::MethodDefinition(MethodDefinition {
                        key: key.unwrap_or(m.key),
                        value: value.unwrap_or(m.value),
                        ..*m
                    }))
                }
            }
            ClassMember::PropertyDefinition(p) => {
                let key = if p.computed {
                    self.expr_ref(p.key)?
                } else {
                    None
                };
                let value = match p.value {
                    Some(value) => self.expr_ref(value)?.map(Some),
                    None => None,
                };
                if key.is_none() && value.is_none() {
                    None
                } else {
                    Some(ClassMember::PropertyDefinition(PropertyDefinition {
                        key: key.unwrap_or(p.key),
                        value: value.unwrap_or(p.value),
                        ..*p
                    }))
                }
            }
            ClassMember::StaticBlock(block) => self
                .statements(block.body)?
                .map(|body| ClassMember::StaticBlock(StaticBlock { body, ..*block })),
            // Type-only — no value children.
            ClassMember::IndexSignature(_) => None,
        })
    }

    // ── Expressions ────────────────────────────────────────────────────────

    /// The expression `id` names, rewritten and pushed — `None` when unchanged.
    ///
    /// A plain `$derived` read is lowered here rather than in [`Self::expr`],
    /// because here the read has an id: the `d()` call takes it as its callee, where
    /// the row-level arm would have to copy the host identifier out and push it
    /// again. The checks keep the arm's order — a store base first (the arm's
    /// store lowering and its shadow refusal), a derived read only when there is
    /// none.
    fn expr_ref(&mut self, id: ExprId) -> Result<Option<ExprId>, CompileError> {
        let row = *self.view().expr(id);
        if let ExpressionKind::Identifier(ident) = row.kind
            && self.store_base(&ident).is_none()
            && self.derived_read(&ident)
        {
            // An identifier expression's span is its identifier's (every builder of
            // one goes through `Expression::from_identifier`), so the callee's span —
            // and with it the call's — is the one a pushed copy would have carried.
            debug_assert_eq!(row.span, ident.span);
            let call = self.b.call_expr(id, &[]);
            return Ok(Some(self.b.push(call)));
        }
        Ok(self.expr(&row)?.map(|new| self.b.push(new)))
    }

    /// Rewrite an expression, materializing the result — `id` itself when
    /// unchanged, a pushed row when rewritten — for a value position that always
    /// needs a concrete node.
    fn rewrite_value(&mut self, id: ExprId) -> Result<ExprId, CompileError> {
        Ok(self.expr_ref(id)?.unwrap_or(id))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per variant — the exhaustive inventory is the safety argument"
    )]
    fn expr(&mut self, expr: &Expression) -> Result<Option<Expression>, CompileError> {
        use tsv_ts::ast;
        let span = expr.span;
        Ok(match expr.kind {
            // ── The store / derived leaves ─────────────────────────────────
            ExpressionKind::Identifier(id) => match self.store_base(&id) {
                Some(base) => {
                    if self.store_shadowed.contains(&base) {
                        return unsupported(Refusal::StoreScopedSubscription);
                    }
                    let is_derived = self.derived_names.contains(&base);
                    Some(self.b.store_get(&base, is_derived, id.span))
                }
                // A plain read of a `$derived` binding → `d()` is lowered by
                // `expr_ref`, which has the read's id. The identifier rows that reach
                // this arm directly are the ones held by value — a bare parameter, a
                // catch parameter, and a `ParenthesizedExpression`'s contents,
                // unreachable in the paren-free tree (`unreachable_contents` panics
                // in debug builds) — and the first two are bindings, never reads: a binding
                // named for a `$derived` is in `nested_declared` (the rune guard's
                // `enter_function` / catch arm), which `compile_server` refuses as
                // `DerivedReadShadowed` before this pass runs.
                None => None,
            },
            ExpressionKind::AssignmentExpression(assign) => self.assignment(&assign, span)?,
            ExpressionKind::UpdateExpression(update) => self.update(update, span)?,

            // ── Recursion ──────────────────────────────────────────────────
            ExpressionKind::CallExpression(call) => {
                let callee = self.expr_ref(call.callee)?;
                let arguments = self.id_run(call.arguments)?;
                if callee.is_none() && arguments.is_none() {
                    None
                } else {
                    Some(Expression {
                        span,
                        kind: ExpressionKind::CallExpression(ast::CallExpression {
                            callee: callee.unwrap_or(call.callee),
                            arguments: arguments.unwrap_or(call.arguments),
                            ..call
                        }),
                    })
                }
            }
            ExpressionKind::NewExpression(new) => {
                let callee = self.expr_ref(new.callee)?;
                let arguments = self.id_run(new.arguments)?;
                if callee.is_none() && arguments.is_none() {
                    None
                } else {
                    Some(Expression {
                        span,
                        kind: ExpressionKind::NewExpression(ast::NewExpression {
                            callee: callee.unwrap_or(new.callee),
                            arguments: arguments.unwrap_or(new.arguments),
                            ..new
                        }),
                    })
                }
            }
            ExpressionKind::MemberExpression(member) => {
                let object = self.expr_ref(member.object)?;
                // A non-computed property is a NAME, never a store read.
                let property = if member.computed {
                    self.expr_ref(member.property)?
                } else {
                    None
                };
                if object.is_none() && property.is_none() {
                    None
                } else {
                    Some(Expression {
                        span,
                        kind: ExpressionKind::MemberExpression(MemberExpression {
                            object: object.unwrap_or(member.object),
                            property: property.unwrap_or(member.property),
                            ..member
                        }),
                    })
                }
            }
            ExpressionKind::BinaryExpression(binary) => {
                let left = self.expr_ref(binary.left)?;
                let right = self.expr_ref(binary.right)?;
                if left.is_none() && right.is_none() {
                    None
                } else {
                    Some(Expression {
                        span,
                        kind: ExpressionKind::BinaryExpression(ast::BinaryExpression {
                            left: left.unwrap_or(binary.left),
                            right: right.unwrap_or(binary.right),
                            // `relexes_as_type_arguments` rides along: it is a claim about
                            // the SOURCE bytes this rewrite invalidates, and inheriting is the
                            // SAFE direction — a stale `true` costs a redundant paren pair, a
                            // stale `false` an output nothing reparses.
                            ..binary
                        }),
                    })
                }
            }
            ExpressionKind::ConditionalExpression(cond) => {
                let test = self.expr_ref(cond.test)?;
                let consequent = self.expr_ref(cond.consequent)?;
                let alternate = self.expr_ref(cond.alternate)?;
                if test.is_none() && consequent.is_none() && alternate.is_none() {
                    None
                } else {
                    Some(Expression {
                        span,
                        kind: ExpressionKind::ConditionalExpression(ast::ConditionalExpression {
                            test: test.unwrap_or(cond.test),
                            consequent: consequent.unwrap_or(cond.consequent),
                            alternate: alternate.unwrap_or(cond.alternate),
                        }),
                    })
                }
            }
            ExpressionKind::UnaryExpression(unary) => {
                self.expr_ref(unary.argument)?.map(|argument| {
                    Expression::from_unary_expression(ast::UnaryExpression { argument, ..unary })
                })
            }
            ExpressionKind::ArrayExpression(arr) => {
                self.opt_id_run(arr.elements)?.map(|elements| Expression {
                    span,
                    kind: ExpressionKind::ArrayExpression(ast::ArrayExpression { elements, ..arr }),
                })
            }
            ExpressionKind::ObjectExpression(obj) => rebuild_run(
                self,
                obj.properties,
                |view, run| view.obj_props(run),
                |rewriter, property| Ok(rewriter.object_property(&property)?.into()),
            )?
            .map(|properties| Expression {
                span,
                kind: ExpressionKind::ObjectExpression(ast::ObjectExpression { properties, ..obj }),
            }),
            ExpressionKind::ArrowFunctionExpression(id) => {
                let arrow = *self.view().arrow_fn(id);
                self.arrow(&arrow)?.map(|new| Expression {
                    span,
                    kind: ExpressionKind::ArrowFunctionExpression(self.b.push(new)),
                })
            }
            ExpressionKind::FunctionExpression(id) => {
                let func = *self.view().fn_expr(id);
                self.function_expression(&func)?.map(|new| {
                    let id = self.b.push(new);
                    Expression::from_function_expression(new.span, id)
                })
            }
            ExpressionKind::ClassExpression(id) => {
                let class = *self.view().class_expr(id);
                self.class_expression(&class)?.map(|new| Expression {
                    span,
                    kind: ExpressionKind::ClassExpression(self.b.push(new)),
                })
            }
            ExpressionKind::SpreadElement(spread) => {
                self.expr_ref(spread.argument)?.map(|argument| {
                    Expression::from_spread_element(ast::SpreadElement {
                        argument,
                        span: spread.span,
                    })
                })
            }
            ExpressionKind::TemplateLiteral(template) => {
                self.id_run(template.expressions)?.map(|expressions| {
                    Expression::from_template_literal(
                        span,
                        ast::TemplateLiteral {
                            expressions,
                            ..template
                        },
                    )
                })
            }
            ExpressionKind::TaggedTemplateExpression(id) => {
                let tagged = *self.view().tagged_tpl(id);
                let tag = self.expr_ref(tagged.tag)?;
                let quasi = self.id_run(tagged.quasi.expressions)?;
                if tag.is_none() && quasi.is_none() {
                    None
                } else {
                    let new = self.b.push(TaggedTemplateExpression {
                        tag: tag.unwrap_or(tagged.tag),
                        quasi: quasi.map_or(tagged.quasi, |expressions| ast::TemplateLiteral {
                            expressions,
                            ..tagged.quasi
                        }),
                        ..tagged
                    });
                    Some(Expression {
                        span,
                        kind: ExpressionKind::TaggedTemplateExpression(new),
                    })
                }
            }
            ExpressionKind::AwaitExpression(node) => {
                self.expr_ref(node.argument)?.map(|argument| Expression {
                    span,
                    kind: ExpressionKind::AwaitExpression(ast::AwaitExpression { argument }),
                })
            }
            ExpressionKind::YieldExpression(node) => match node.argument {
                Some(argument) => self.expr_ref(argument)?.map(|argument| Expression {
                    span,
                    kind: ExpressionKind::YieldExpression(ast::YieldExpression {
                        argument: Some(argument),
                        ..node
                    }),
                }),
                None => None,
            },
            ExpressionKind::SequenceExpression(seq) => {
                self.id_run(seq.expressions)?.map(|expressions| Expression {
                    span,
                    kind: ExpressionKind::SequenceExpression(ast::SequenceExpression {
                        expressions,
                    }),
                })
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
                    Some(Expression {
                        span,
                        kind: ExpressionKind::ImportExpression(ast::ImportExpression {
                            source: source.unwrap_or(import.source),
                            options: options.unwrap_or(import.options),
                            ..import
                        }),
                    })
                }
            }

            // ── Patterns (their DEFAULTS are reads) ────────────────────────
            ExpressionKind::ObjectPattern(pattern) => rebuild_run(
                self,
                pattern.properties,
                |view, run| view.pat_props(run),
                |rewriter, property| Ok(rewriter.object_pattern_property(&property)?.into()),
            )?
            .map(|properties| Expression {
                span,
                kind: ExpressionKind::ObjectPattern(ObjectPattern {
                    properties,
                    ..pattern
                }),
            }),
            ExpressionKind::ArrayPattern(pattern) => {
                self.opt_id_run(pattern.elements)?
                    .map(|elements| Expression {
                        span,
                        kind: ExpressionKind::ArrayPattern(ast::ArrayPattern {
                            elements,
                            ..pattern
                        }),
                    })
            }
            ExpressionKind::AssignmentPattern(pattern) => {
                // `left` is a binding pattern (not a read); `right` is the default.
                let left = self.expr_ref(pattern.left)?;
                let right = self.expr_ref(pattern.right)?;
                if left.is_none() && right.is_none() {
                    None
                } else {
                    Some(Expression {
                        span,
                        kind: ExpressionKind::AssignmentPattern(AssignmentPattern {
                            left: left.unwrap_or(pattern.left),
                            right: right.unwrap_or(pattern.right),
                            ..pattern
                        }),
                    })
                }
            }
            ExpressionKind::RestElement(rest) => self
                .expr_ref(rest.argument)?
                .map(|argument| Expression::from_rest_element(RestElement { argument, ..rest })),

            // ── Leaves, and nodes that cannot appear post-erasure ──────────
            // The TypeScript wrappers (`x as T`, `x!`, `<T>x`, `x satisfies T`,
            // `f<T>`, a parameter property) and `CommentBoundPair` are all removed by
            // `erase` before this pass; a survivor is caught by the erase
            // self-check on the finished program, never a silent store miss here.
            // Exhaustive on purpose — a NEW expression variant fails compilation.
            ExpressionKind::Literal(_)
            | ExpressionKind::PrivateIdentifier(_)
            | ExpressionKind::RegexLiteral(_)
            | ExpressionKind::ThisExpression(_)
            | ExpressionKind::Super(_)
            | ExpressionKind::MetaProperty(_)
            | ExpressionKind::TSTypeAssertion(_)
            | ExpressionKind::TSAsExpression(_)
            | ExpressionKind::TSSatisfiesExpression(_)
            | ExpressionKind::TSInstantiationExpression(_)
            | ExpressionKind::TSNonNullExpression(_)
            | ExpressionKind::TSParameterProperty(_)
            | ExpressionKind::CommentBoundPair(_) => None,
            ExpressionKind::ParenthesizedExpression(paren) => {
                let contents = *paren.unreachable_contents(self.view());
                self.expr(&contents)?
            }
        })
    }

    fn object_property(
        &mut self,
        property: &ObjectProperty,
    ) -> Result<Option<ObjectProperty>, CompileError> {
        Ok(match property {
            // An object-LITERAL property: a shorthand `{ $s }` (≡ `{ $s: $s }`)
            // whose value is rewritten must un-shorthand — the value is no longer
            // the key identifier (`{ $s: $.store_get(…) }`).
            ObjectProperty::Property(prop) => {
                self.property(prop, true)?.map(ObjectProperty::Property)
            }
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
            // A destructuring-PATTERN property: shorthand means the key equals the
            // BOUND name (`{ value = default }`), which rewriting the default never
            // changes — so it stays shorthand (`{ value = $.store_get(…) }`, NOT
            // `{ value: value = … }`).
            ObjectPatternProperty::Property(prop) => self
                .property(prop, false)?
                .map(ObjectPatternProperty::Property),
            ObjectPatternProperty::RestElement(rest) => {
                self.expr_ref(rest.argument)?.map(|argument| {
                    ObjectPatternProperty::RestElement(RestElement { argument, ..*rest })
                })
            }
        })
    }

    /// An object/pattern property. A non-computed key is a NAME, never a read —
    /// only a computed key is descended. `unshorthand_on_change` un-shorthands a
    /// shorthand whose value changed: `true` for an object literal (the value IS
    /// the key identifier), `false` for a destructuring pattern (shorthand tracks
    /// the bound name, and only the *default* is rewritten).
    fn property(
        &mut self,
        prop: &Property,
        unshorthand_on_change: bool,
    ) -> Result<Option<Property>, CompileError> {
        let key = if prop.computed {
            self.expr_ref(prop.key)?
        } else {
            None
        };
        let value = self.expr_ref(prop.value)?;
        if key.is_none() && value.is_none() {
            return Ok(None);
        }
        let shorthand = prop.shorthand && !(unshorthand_on_change && value.is_some());
        Ok(Some(Property {
            key: key.unwrap_or(prop.key),
            value: value.unwrap_or(prop.value),
            shorthand,
            ..*prop
        }))
    }

    // ── Writes and updates ─────────────────────────────────────────────────

    /// Classify an assignment/update target: a bare `$name` store (the supported
    /// write), a member/destructuring write over a store (refused), or a plain
    /// lvalue (recurse for reads).
    fn classify_target(&self, left: ExprId) -> Result<StoreTarget, CompileError> {
        let view = self.view();
        let target = view.expr(left);
        if let ExpressionKind::Identifier(id) = &target.kind {
            return Ok(match self.store_base(id) {
                Some(base) => StoreTarget::Bare(base),
                None => StoreTarget::NotStore,
            });
        }
        // A member chain rooted at a store `$obj.foo = …` → `$.store_mutate` (not
        // implemented). A member rooted at a plain lvalue (`x[$c] = …`) is not a
        // store write — its store lives in a read position, so recurse instead.
        if matches!(target.kind, ExpressionKind::MemberExpression(_)) {
            if let ExpressionKind::Identifier(root) = &member_root(view, left).kind
                && self.store_base(root).is_some()
            {
                return unsupported(Refusal::StoreMemberWrite);
            }
            return Ok(StoreTarget::NotStore);
        }
        // A destructuring pattern with a store target (`[$count] = …`) → an IIFE
        // (not implemented). A pattern whose stores are all in read positions is
        // handled by recursion.
        if self.pattern_targets_store(left) {
            return unsupported(Refusal::StoreDestructuringWrite);
        }
        Ok(StoreTarget::NotStore)
    }

    fn assignment(
        &mut self,
        assign: &AssignmentExpression,
        span: Span,
    ) -> Result<Option<Expression>, CompileError> {
        match self.classify_target(assign.left)? {
            StoreTarget::Bare(base) => {
                if self.store_shadowed.contains(&base) {
                    return unsupported(Refusal::StoreScopedSubscription);
                }
                let value = if assign.operator == AssignmentOperator::Assign {
                    self.rewrite_value(assign.right)?
                } else {
                    // `$count += v` → `$.store_set(count, $count + v)`: reconstruct
                    // the binary the oracle's `build_assignment_value` produces,
                    // then rewrite the `$count` read (and any read in `v`) inside.
                    let binop = compound_binary_op(assign.operator);
                    let bin = self.b.binary(assign.left, binop, assign.right);
                    let bin = self.b.push(bin);
                    self.rewrite_value(bin)?
                };
                Ok(Some(self.b.store_set(&base, value, span)))
            }
            StoreTarget::NotStore => {
                let left = self.expr_ref(assign.left)?;
                let right = self.expr_ref(assign.right)?;
                if left.is_none() && right.is_none() {
                    Ok(None)
                } else {
                    Ok(Some(Expression {
                        span,
                        kind: ExpressionKind::AssignmentExpression(AssignmentExpression {
                            left: left.unwrap_or(assign.left),
                            right: right.unwrap_or(assign.right),
                            ..*assign
                        }),
                    }))
                }
            }
        }
    }

    fn update(
        &mut self,
        update: tsv_ts::ast::UpdateExpression,
        span: Span,
    ) -> Result<Option<Expression>, CompileError> {
        let view = self.view();
        let argument = view.expr(update.argument);
        // A bare `$name++` / `++$name` store update.
        if let ExpressionKind::Identifier(id) = &argument.kind
            && let Some(base) = self.store_base(id)
        {
            if self.store_shadowed.contains(&base) {
                return unsupported(Refusal::StoreScopedSubscription);
            }
            let decrement = update.operator == UpdateOperator::Decrement;
            return Ok(Some(self.b.update_store(
                &base,
                update.prefix,
                decrement,
                span,
            )));
        }
        // A member update rooted at a store (`$obj.x++`) → `$.store_mutate` (not
        // implemented). A member over a plain lvalue recurses (its store is a read).
        if matches!(argument.kind, ExpressionKind::MemberExpression(_))
            && let ExpressionKind::Identifier(root) = &member_root(view, update.argument).kind
            && self.store_base(root).is_some()
        {
            return unsupported(Refusal::StoreMemberWrite);
        }
        Ok(self.expr_ref(update.argument)?.map(|argument| Expression {
            span,
            kind: ExpressionKind::UpdateExpression(tsv_ts::ast::UpdateExpression {
                argument,
                ..update
            }),
        }))
    }
}
