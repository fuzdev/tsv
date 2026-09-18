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
//! never rebuilt and nothing is allocated. Clones are shallow (children are
//! `&'arena T`, so a rebuilt node copies pointers, never subtrees).
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

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ArrowFunctionExpression, AssignmentExpression, AssignmentOperator,
    AssignmentPattern, BinaryOperator, BlockStatement, CatchClause, ClassBody, ClassDeclaration,
    ClassExpression, ClassMember, Expression, ExpressionKind, ForInOfLeft, ForInit,
    FunctionDeclaration, FunctionExpression, Identifier, MemberExpression, MethodDefinition,
    ObjectPattern, ObjectPatternProperty, ObjectProperty, Property, PropertyDefinition,
    RestElement, Statement, StatementKind, StaticBlock, SwitchCase, UpdateOperator,
};

use crate::CompileError;
use crate::analyze::{NameSet, store_read_base};
use crate::build::Builder;
use crate::refusal::Refusal;

/// Rewrite every store access (and script-position `$derived` read) in `stmts`,
/// returning `None` when nothing changed (the caller keeps the original slice).
/// See the [module docs](self). The minted `d()` reads take the callee's tight
/// span ([`Builder::call_expr`]), so they never sweep a carried script comment —
/// no comment gate is needed here.
pub(crate) fn rewrite_store_accesses<'arena>(
    b: &mut Builder<'arena>,
    source: &str,
    store_names: &NameSet,
    store_shadowed: &NameSet,
    derived_names: &NameSet,
    stmts: &'arena [Statement<'arena>],
) -> Result<Option<&'arena [Statement<'arena>]>, CompileError> {
    let mut rewriter = StoreRewriter {
        b,
        source,
        store_names,
        store_shadowed,
        derived_names,
    };
    rewriter.statements(stmts)
}

struct StoreRewriter<'a, 'arena> {
    b: &'a mut Builder<'arena>,
    source: &'a str,
    store_names: &'a NameSet,
    store_shadowed: &'a NameSet,
    derived_names: &'a NameSet,
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
fn member_root<'e>(expr: &'e Expression<'e>) -> &'e Expression<'e> {
    let mut node = expr;
    loop {
        match &node.kind {
            ExpressionKind::MemberExpression(m) => node = m.object,
            ExpressionKind::ParenthesizedExpression(p) => node = p.expression,
            _ => return node,
        }
    }
}

/// Rebuild a slice through a per-item method, sharing the original when no item
/// changed (mirrors [`crate::erase`]'s `map_slice!`).
macro_rules! map_slice {
    ($self:ident, $items:expr, $method:ident) => {{
        let items = $items;
        let arena = $self.b.arena;
        let mut out: Option<BumpVec<'arena, _>> = None;
        for (i, item) in items.iter().enumerate() {
            match $self.$method(item)? {
                None => {
                    if let Some(vec) = out.as_mut() {
                        vec.push(item.clone());
                    }
                }
                Some(new) => {
                    out.get_or_insert_with(|| {
                        let mut vec = BumpVec::with_capacity_in(items.len(), arena);
                        vec.extend_from_slice(&items[..i]);
                        vec
                    })
                    .push(new);
                }
            }
        }
        out.map(BumpVec::into_bump_slice)
    }};
}

impl<'arena> StoreRewriter<'_, 'arena> {
    // ── Store recognition ──────────────────────────────────────────────────

    /// The `$`-stripped base of a `$name` identifier that resolves to a top-level
    /// component binding (a store), else `None`. Mirrors `template_value`'s
    /// `bare_store_read`.
    ///
    /// The name is DECODED: a plain identifier is its span slice (the fast path —
    /// no arena string touched), while a unicode-escaped `$`-identifier
    /// (`$count` written `$count`) reads its decoded `&'arena str`, so it is read
    /// as the store the oracle sees (the oracle decodes `node.name`).
    fn store_base(&self, id: &Identifier<'_>) -> Option<String> {
        if id.escaped_name.is_some() {
            // An escaped `$`-identifier decodes to a store name the oracle treats
            // exactly as its plain spelling.
            let name = id.name(self.source);
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
    fn derived_read(&self, id: &Identifier<'_>) -> bool {
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
    fn pattern_targets_store(&self, left: &Expression<'_>) -> bool {
        match &left.kind {
            ExpressionKind::Identifier(id) => self.store_base(id).is_some(),
            ExpressionKind::MemberExpression(m) => self.pattern_targets_store(m.object),
            ExpressionKind::ParenthesizedExpression(p) => self.pattern_targets_store(p.expression),
            ExpressionKind::ObjectPattern(obj) => obj.properties.iter().any(|prop| match prop {
                ObjectPatternProperty::Property(p) => self.pattern_targets_store(p.value),
                ObjectPatternProperty::RestElement(rest) => {
                    self.pattern_targets_store(rest.argument)
                }
            }),
            ExpressionKind::ArrayPattern(arr) => arr
                .elements
                .iter()
                .flatten()
                .any(|element| self.pattern_targets_store(element)),
            ExpressionKind::AssignmentPattern(a) => self.pattern_targets_store(a.left),
            ExpressionKind::RestElement(r) => self.pattern_targets_store(r.argument),
            _ => false,
        }
    }

    // ── Statements ─────────────────────────────────────────────────────────

    fn statements(
        &mut self,
        stmts: &'arena [Statement<'arena>],
    ) -> Result<Option<&'arena [Statement<'arena>]>, CompileError> {
        Ok(map_slice!(self, stmts, statement))
    }

    /// A statement in a single-statement position (an `if` branch, a loop body).
    fn statement_ref(
        &mut self,
        stmt: &'arena Statement<'arena>,
    ) -> Result<Option<&'arena Statement<'arena>>, CompileError> {
        Ok(self.statement(stmt)?.map(|new| &*self.b.arena.alloc(new)))
    }

    #[expect(clippy::too_many_lines)]
    fn statement(
        &mut self,
        stmt: &Statement<'arena>,
    ) -> Result<Option<Statement<'arena>>, CompileError> {
        use tsv_ts::ast::internal as ast;
        // The header span: a rebuilt statement keeps the original's.
        let span = stmt.span;
        Ok(match &stmt.kind {
            StatementKind::ExpressionStatement(s) => {
                self.expr_ref(s.expression)?.map(|expression| Statement {
                    span,
                    kind: StatementKind::ExpressionStatement(ast::ExpressionStatement {
                        expression,
                        ..s.clone()
                    }),
                })
            }
            StatementKind::VariableDeclaration(decl) => {
                map_slice!(self, decl.declarations, variable_declarator).map(|declarations| {
                    Statement::from_variable_declaration(ast::VariableDeclaration {
                        declarations,
                        ..decl.clone()
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
                self.block(block)?.map(Statement::from_block_statement)
            }
            StatementKind::FunctionDeclaration(decl) => self
                .function_declaration(decl)?
                .map(|d| Statement::from_function_declaration(self.b.arena.alloc(d))),
            StatementKind::ClassDeclaration(decl) => {
                let super_class = match decl.super_class {
                    Some(sc) => self.expr_ref(sc)?.map(Some),
                    None => None,
                };
                let body = self.class_body(&decl.body)?;
                if super_class.is_none() && body.is_none() {
                    None
                } else {
                    Some(Statement::from_class_declaration(self.b.arena.alloc(
                        ClassDeclaration {
                            super_class: super_class.unwrap_or(decl.super_class),
                            body: body.unwrap_or_else(|| decl.body.clone()),
                            ..(*decl).clone()
                        },
                    )))
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
                let init = match &s.init {
                    Some(init) => self.for_init(init)?.map(Some),
                    None => None,
                };
                let test = match &s.test {
                    Some(test) => self.expr(test)?.map(Some),
                    None => None,
                };
                let update = match &s.update {
                    Some(update) => self.expr(update)?.map(Some),
                    None => None,
                };
                let body = self.statement_ref(s.body)?;
                if init.is_none() && test.is_none() && update.is_none() && body.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::ForStatement(ast::ForStatement {
                            init: match init {
                                Some(v) => v.map(|x| &*self.b.arena.alloc(x)),
                                None => s.init,
                            },
                            test: match test {
                                Some(v) => v.map(|x| &*self.b.arena.alloc(x)),
                                None => s.test,
                            },
                            update: match update {
                                Some(v) => v.map(|x| &*self.b.arena.alloc(x)),
                                None => s.update,
                            },
                            body: body.unwrap_or(s.body),
                        }),
                    })
                }
            }
            StatementKind::ForInStatement(s) => {
                let left = self.for_in_of_left(s.left)?;
                let right = self.expr(s.right)?;
                let body = self.statement_ref(s.body)?;
                if left.is_none() && right.is_none() && body.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::ForInStatement(ast::ForInStatement {
                            left: match left {
                                Some(x) => self.b.arena.alloc(x),
                                None => s.left,
                            },
                            right: match right {
                                Some(x) => self.b.arena.alloc(x),
                                None => s.right,
                            },
                            body: body.unwrap_or(s.body),
                        }),
                    })
                }
            }
            StatementKind::ForOfStatement(s) => {
                let left = self.for_in_of_left(s.left)?;
                let right = self.expr(s.right)?;
                let body = self.statement_ref(s.body)?;
                if left.is_none() && right.is_none() && body.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::ForOfStatement(ast::ForOfStatement {
                            left: match left {
                                Some(x) => self.b.arena.alloc(x),
                                None => s.left,
                            },
                            right: match right {
                                Some(x) => self.b.arena.alloc(x),
                                None => s.right,
                            },
                            body: body.unwrap_or(s.body),
                            ..s.clone()
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
                let cases = map_slice!(self, s.cases, switch_case);
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
            StatementKind::TryStatement(s) => {
                let block = self.block(&s.block)?;
                let handler = match &s.handler {
                    Some(handler) => self.catch_clause(handler)?.map(Some),
                    None => None,
                };
                let finalizer = match &s.finalizer {
                    Some(finalizer) => self.block(finalizer)?.map(Some),
                    None => None,
                };
                if block.is_none() && handler.is_none() && finalizer.is_none() {
                    None
                } else {
                    Some(Statement {
                        span,
                        kind: StatementKind::TryStatement(ast::TryStatement {
                            block: block.unwrap_or_else(|| s.block.clone()),
                            handler: match handler {
                                Some(v) => v.map(|x| &*self.b.arena.alloc(x)),
                                None => s.handler,
                            },
                            finalizer: finalizer.unwrap_or_else(|| s.finalizer.clone()),
                        }),
                    })
                }
            }
            StatementKind::ThrowStatement(s) => {
                self.expr_ref(s.argument)?.map(|argument| Statement {
                    span,
                    kind: StatementKind::ThrowStatement(ast::ThrowStatement { argument }),
                })
            }
            StatementKind::LabeledStatement(s) => {
                self.statement_ref(s.body)?.map(|body| Statement {
                    span,
                    kind: StatementKind::LabeledStatement(ast::LabeledStatement {
                        body,
                        ..s.clone()
                    }),
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

    fn for_init(
        &mut self,
        init: &ForInit<'arena>,
    ) -> Result<Option<ForInit<'arena>>, CompileError> {
        Ok(match init {
            ForInit::VariableDeclaration(decl) => {
                map_slice!(self, decl.declarations, variable_declarator).map(|declarations| {
                    ForInit::VariableDeclaration(tsv_ts::ast::internal::VariableDeclaration {
                        declarations,
                        ..decl.clone()
                    })
                })
            }
            ForInit::Expression(expr) => self.expr(expr)?.map(ForInit::Expression),
        })
    }

    fn for_in_of_left(
        &mut self,
        left: &ForInOfLeft<'arena>,
    ) -> Result<Option<ForInOfLeft<'arena>>, CompileError> {
        Ok(match left {
            ForInOfLeft::VariableDeclaration(decl) => {
                map_slice!(self, decl.declarations, variable_declarator).map(|declarations| {
                    ForInOfLeft::VariableDeclaration(tsv_ts::ast::internal::VariableDeclaration {
                        declarations,
                        ..decl.clone()
                    })
                })
            }
            ForInOfLeft::Pattern(pattern) => self.expr(pattern)?.map(ForInOfLeft::Pattern),
        })
    }

    fn switch_case(
        &mut self,
        case: &SwitchCase<'arena>,
    ) -> Result<Option<SwitchCase<'arena>>, CompileError> {
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

    fn variable_declarator(
        &mut self,
        declarator: &tsv_ts::ast::internal::VariableDeclarator<'arena>,
    ) -> Result<Option<tsv_ts::ast::internal::VariableDeclarator<'arena>>, CompileError> {
        // The `id` is a binding pattern — its binding NAMES are not reads, but a
        // default (`let { a = $count } = …`) is, so recurse a pattern for its
        // defaults. A plain-identifier binding (`let d = …`) has no read
        // sub-position and MUST be left alone: a top-level `$derived` name in
        // binding position would otherwise be rewritten to `d()`. (Nested /
        // shadowing binding names are refused by the derived-shadow check in
        // `compile_server`, so the only binding-position derived name reaching here
        // is a top-level declarator id.) A `$name` never appears in binding
        // position, so this is a no-op for the store rewrite.
        let id = match &declarator.id {
            Expression {
                kind: ExpressionKind::Identifier(_),
                ..
            } => None,
            other => self.expr(other)?,
        };
        let init = match &declarator.init {
            Some(init) => self.expr(init)?.map(Some),
            None => None,
        };
        if id.is_none() && init.is_none() {
            return Ok(None);
        }
        Ok(Some(tsv_ts::ast::internal::VariableDeclarator {
            id: id.map_or(declarator.id, |id| &*self.b.arena.alloc(id)),
            init: init.map_or(declarator.init, |init| {
                init.map(|init| &*self.b.arena.alloc(init))
            }),
            ..declarator.clone()
        }))
    }

    // ── Functions and classes ──────────────────────────────────────────────

    fn function_declaration(
        &mut self,
        func: &FunctionDeclaration<'arena>,
    ) -> Result<Option<FunctionDeclaration<'arena>>, CompileError> {
        let params = map_slice!(self, func.params, expr);
        let body = self.block(&func.body)?;
        if params.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(FunctionDeclaration {
            params: params.unwrap_or(func.params),
            body: body.unwrap_or_else(|| func.body.clone()),
            ..func.clone()
        }))
    }

    fn function_expression(
        &mut self,
        func: &FunctionExpression<'arena>,
    ) -> Result<Option<FunctionExpression<'arena>>, CompileError> {
        let params = map_slice!(self, func.params, expr);
        let body = self.block(&func.body)?;
        if params.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(FunctionExpression {
            params: params.unwrap_or(func.params),
            body: body.unwrap_or_else(|| func.body.clone()),
            ..func.clone()
        }))
    }

    fn arrow(
        &mut self,
        arrow: &ArrowFunctionExpression<'arena>,
    ) -> Result<Option<ArrowFunctionExpression<'arena>>, CompileError> {
        let params = map_slice!(self, arrow.params, expr);
        let body = match &arrow.body {
            ArrowFunctionBody::Expression(expr) => {
                self.expr_ref(expr)?.map(ArrowFunctionBody::Expression)
            }
            ArrowFunctionBody::BlockStatement(block) => {
                self.block(block)?.map(ArrowFunctionBody::BlockStatement)
            }
        };
        if params.is_none() && body.is_none() {
            return Ok(None);
        }
        Ok(Some(ArrowFunctionExpression {
            params: params.unwrap_or(arrow.params),
            body: body.unwrap_or_else(|| arrow.body.clone()),
            ..arrow.clone()
        }))
    }

    fn class_expression(
        &mut self,
        class: &ClassExpression<'arena>,
    ) -> Result<Option<ClassExpression<'arena>>, CompileError> {
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
            body: body.unwrap_or_else(|| class.body.clone()),
            ..class.clone()
        }))
    }

    fn class_body(
        &mut self,
        body: &ClassBody<'arena>,
    ) -> Result<Option<ClassBody<'arena>>, CompileError> {
        Ok(
            map_slice!(self, body.body, class_member).map(|members| ClassBody {
                body: members,
                span: body.span,
            }),
        )
    }

    fn class_member(
        &mut self,
        member: &ClassMember<'arena>,
    ) -> Result<Option<ClassMember<'arena>>, CompileError> {
        Ok(match member {
            ClassMember::MethodDefinition(m) => {
                // A non-computed key is a name, not a read.
                let key = if m.computed { self.expr(&m.key)? } else { None };
                let value = self.function_expression(&m.value)?;
                if key.is_none() && value.is_none() {
                    None
                } else {
                    Some(ClassMember::MethodDefinition(MethodDefinition {
                        key: key.unwrap_or_else(|| m.key.clone()),
                        value: value.unwrap_or_else(|| m.value.clone()),
                        ..m.clone()
                    }))
                }
            }
            ClassMember::PropertyDefinition(p) => {
                let key = if p.computed { self.expr(&p.key)? } else { None };
                let value = match &p.value {
                    Some(value) => self.expr(value)?.map(Some),
                    None => None,
                };
                if key.is_none() && value.is_none() {
                    None
                } else {
                    Some(ClassMember::PropertyDefinition(PropertyDefinition {
                        key: key.unwrap_or_else(|| p.key.clone()),
                        value: value.unwrap_or_else(|| p.value.clone()),
                        ..p.clone()
                    }))
                }
            }
            ClassMember::StaticBlock(block) => self.statements(block.body)?.map(|stmts| {
                ClassMember::StaticBlock(StaticBlock {
                    body: stmts,
                    span: block.span,
                })
            }),
            // Type-only — no value children.
            ClassMember::IndexSignature(_) => None,
        })
    }

    // ── Expressions ────────────────────────────────────────────────────────

    fn expr_ref(
        &mut self,
        expr: &'arena Expression<'arena>,
    ) -> Result<Option<&'arena Expression<'arena>>, CompileError> {
        Ok(self.expr(expr)?.map(|new| &*self.b.arena.alloc(new)))
    }

    /// Rewrite an expression, materializing the result (the original clone when
    /// unchanged) — for a value position that always needs a concrete node.
    fn rewrite_value(
        &mut self,
        expr: &'arena Expression<'arena>,
    ) -> Result<Expression<'arena>, CompileError> {
        Ok(self.expr(expr)?.unwrap_or_else(|| expr.clone()))
    }

    #[expect(clippy::too_many_lines)]
    fn expr(
        &mut self,
        expr: &Expression<'arena>,
    ) -> Result<Option<Expression<'arena>>, CompileError> {
        use tsv_ts::ast::internal as ast;
        Ok(match &expr.kind {
            // ── The store / derived leaves ─────────────────────────────────
            ExpressionKind::Identifier(id) => match self.store_base(id) {
                Some(base) => {
                    if self.store_shadowed.contains(&base) {
                        return unsupported(Refusal::StoreScopedSubscription);
                    }
                    let is_derived = self.derived_names.contains(&base);
                    Some(self.b.store_get(&base, is_derived, id.span))
                }
                // A plain read of a `$derived` binding → `d()` (the script analog
                // of the template value walk). Name-only positions (a non-computed
                // member property / object-or-class key) never reach here — their
                // callers don't descend them — and binding-position ids are skipped
                // by `variable_declarator`.
                None if self.derived_read(id) => {
                    let arena = self.b.arena;
                    let callee: &'arena Expression<'arena> =
                        arena.alloc(Expression::from_identifier(id.clone()));
                    Some(self.b.call_expr(callee, &[]))
                }
                None => None,
            },
            ExpressionKind::AssignmentExpression(assign) => self.assignment(assign, expr.span)?,
            ExpressionKind::UpdateExpression(update) => self.update(update, expr.span)?,

            // ── Recursion ──────────────────────────────────────────────────
            ExpressionKind::CallExpression(call) => {
                let callee = self.expr_ref(call.callee)?;
                let arguments = map_slice!(self, call.arguments, expr_element);
                if callee.is_none() && arguments.is_none() {
                    None
                } else {
                    Some(Expression {
                        span: expr.span,
                        kind: ExpressionKind::CallExpression(ast::CallExpression {
                            callee: callee.unwrap_or(call.callee),
                            arguments: arguments.unwrap_or(call.arguments),
                            ..call.clone()
                        }),
                    })
                }
            }
            ExpressionKind::NewExpression(new) => {
                let callee = self.expr_ref(new.callee)?;
                let arguments = map_slice!(self, new.arguments, expr_element);
                if callee.is_none() && arguments.is_none() {
                    None
                } else {
                    Some(Expression {
                        span: expr.span,
                        kind: ExpressionKind::NewExpression(ast::NewExpression {
                            callee: callee.unwrap_or(new.callee),
                            arguments: arguments.unwrap_or(new.arguments),
                            ..new.clone()
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
                        span: expr.span,
                        kind: ExpressionKind::MemberExpression(MemberExpression {
                            object: object.unwrap_or(member.object),
                            property: property.unwrap_or(member.property),
                            ..member.clone()
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
                        span: expr.span,
                        kind: ExpressionKind::BinaryExpression(ast::BinaryExpression {
                            left: left.unwrap_or(binary.left),
                            right: right.unwrap_or(binary.right),
                            // `relexes_as_type_arguments` rides along: it is a claim about
                            // the SOURCE bytes this rewrite invalidates, and inheriting is the
                            // SAFE direction — a stale `true` costs a redundant paren pair, a
                            // stale `false` an output nothing reparses.
                            ..binary.clone()
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
                        span: expr.span,
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
                    Expression::from_unary_expression(ast::UnaryExpression {
                        argument,
                        ..unary.clone()
                    })
                })
            }
            ExpressionKind::ArrayExpression(arr) => {
                map_slice!(self, arr.elements, opt_expr_element).map(|elements| Expression {
                    span: expr.span,
                    kind: ExpressionKind::ArrayExpression(ast::ArrayExpression {
                        elements,
                        ..arr.clone()
                    }),
                })
            }
            ExpressionKind::ObjectExpression(obj) => {
                map_slice!(self, obj.properties, object_property).map(|properties| Expression {
                    span: expr.span,
                    kind: ExpressionKind::ObjectExpression(ast::ObjectExpression {
                        properties,
                        ..obj.clone()
                    }),
                })
            }
            ExpressionKind::ArrowFunctionExpression(arrow) => {
                self.arrow(arrow)?.map(|a| Expression {
                    span: expr.span,
                    kind: ExpressionKind::ArrowFunctionExpression(self.b.arena.alloc(a)),
                })
            }
            ExpressionKind::FunctionExpression(func) => self
                .function_expression(func)?
                .map(|f| Expression::from_function_expression(self.b.arena.alloc(f))),
            ExpressionKind::ClassExpression(class) => {
                self.class_expression(class)?.map(|c| Expression {
                    span: expr.span,
                    kind: ExpressionKind::ClassExpression(self.b.arena.alloc(c)),
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
                map_slice!(self, template.expressions, expr_element).map(|expressions| {
                    Expression::from_template_literal(ast::TemplateLiteral {
                        expressions,
                        ..template.clone()
                    })
                })
            }
            ExpressionKind::TaggedTemplateExpression(tagged) => {
                let tag = self.expr_ref(tagged.tag)?;
                let quasi = map_slice!(self, tagged.quasi.expressions, expr_element);
                if tag.is_none() && quasi.is_none() {
                    None
                } else {
                    Some(Expression {
                        span: expr.span,
                        kind: ExpressionKind::TaggedTemplateExpression(self.b.arena.alloc(
                            ast::TaggedTemplateExpression {
                                tag: tag.unwrap_or(tagged.tag),
                                quasi: quasi.map_or_else(
                                    || tagged.quasi.clone(),
                                    |expressions| ast::TemplateLiteral {
                                        expressions,
                                        ..tagged.quasi.clone()
                                    },
                                ),
                                ..(*tagged).clone()
                            },
                        )),
                    })
                }
            }
            ExpressionKind::AwaitExpression(node) => {
                self.expr_ref(node.argument)?.map(|argument| Expression {
                    span: expr.span,
                    kind: ExpressionKind::AwaitExpression(ast::AwaitExpression { argument }),
                })
            }
            ExpressionKind::YieldExpression(node) => match node.argument {
                Some(argument) => self.expr_ref(argument)?.map(|argument| Expression {
                    span: expr.span,
                    kind: ExpressionKind::YieldExpression(ast::YieldExpression {
                        argument: Some(argument),
                        ..node.clone()
                    }),
                }),
                None => None,
            },
            ExpressionKind::SequenceExpression(seq) => {
                map_slice!(self, seq.expressions, expr_element).map(|expressions| Expression {
                    span: expr.span,
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
                        span: expr.span,
                        kind: ExpressionKind::ImportExpression(ast::ImportExpression {
                            source: source.unwrap_or(import.source),
                            options: options.unwrap_or(import.options),
                            ..import.clone()
                        }),
                    })
                }
            }
            ExpressionKind::ParenthesizedExpression(paren) => {
                self.expr_ref(paren.expression)?
                    .map(|expression| Expression {
                        span: expr.span,
                        kind: ExpressionKind::ParenthesizedExpression(
                            ast::ParenthesizedExpression { expression },
                        ),
                    })
            }

            // ── Patterns (their DEFAULTS are reads) ────────────────────────
            ExpressionKind::ObjectPattern(pattern) => {
                map_slice!(self, pattern.properties, object_pattern_property).map(|properties| {
                    Expression {
                        span: expr.span,
                        kind: ExpressionKind::ObjectPattern(ObjectPattern {
                            properties,
                            ..pattern.clone()
                        }),
                    }
                })
            }
            ExpressionKind::ArrayPattern(pattern) => map_slice!(self, pattern.elements, opt_expr)
                .map(|elements| Expression {
                    span: expr.span,
                    kind: ExpressionKind::ArrayPattern(tsv_ts::ast::internal::ArrayPattern {
                        elements,
                        ..pattern.clone()
                    }),
                }),
            ExpressionKind::AssignmentPattern(pattern) => {
                // `left` is a binding pattern (not a read); `right` is the default.
                let left = self.expr_ref(pattern.left)?;
                let right = self.expr_ref(pattern.right)?;
                if left.is_none() && right.is_none() {
                    None
                } else {
                    Some(Expression {
                        span: expr.span,
                        kind: ExpressionKind::AssignmentPattern(AssignmentPattern {
                            left: left.unwrap_or(pattern.left),
                            right: right.unwrap_or(pattern.right),
                            ..pattern.clone()
                        }),
                    })
                }
            }
            ExpressionKind::RestElement(rest) => self.expr_ref(rest.argument)?.map(|argument| {
                Expression::from_rest_element(RestElement {
                    argument,
                    ..rest.clone()
                })
            }),

            // ── Leaves, and nodes that cannot appear post-erasure ──────────
            // The TypeScript wrappers (`x as T`, `x!`, `<T>x`, `x satisfies T`,
            // `f<T>`, a parameter property) and `JsdocCast` are all removed by
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
            | ExpressionKind::JsdocCast(_) => None,
        })
    }

    /// An element of a reference slice (call / `new` arguments, sequence and template
    /// expressions) — the `map_slice!` contract with `T = &Expression`: a rebuilt
    /// element is arena-allocated so the rebuilt slice holds references too.
    fn expr_element(
        &mut self,
        expr: &'arena Expression<'arena>,
    ) -> Result<Option<&'arena Expression<'arena>>, CompileError> {
        Ok(self.expr(expr)?.map(|new| &*self.b.arena.alloc(new)))
    }

    /// An array-literal element slot (`Option<&Expression>`, `None` a hole) — the
    /// reference twin of `opt_expr`, whose array-pattern slots hold the element by
    /// value; a rebuilt element is arena-allocated. `&Option<&_>` is the `map_slice!`
    /// contract's `&T` in.
    #[expect(
        clippy::option_option,
        clippy::trivially_copy_pass_by_ref,
        clippy::ref_option_ref
    )]
    fn opt_expr_element(
        &mut self,
        element: &Option<&'arena Expression<'arena>>,
    ) -> Result<Option<Option<&'arena Expression<'arena>>>, CompileError> {
        Ok(match element {
            Some(expr) => self.expr_element(expr)?.map(Some),
            None => None,
        })
    }

    // `Option<Option<T>>`: outer `None` = element unchanged, `Some(inner)` = a
    // rebuilt element (`inner` preserves the hole/value distinction). `&Option`
    // matches the slice-element shape (mirrors `erase::opt_expr`).
    #[expect(clippy::option_option, clippy::ref_option)]
    fn opt_expr(
        &mut self,
        element: &'arena Option<Expression<'arena>>,
    ) -> Result<Option<Option<Expression<'arena>>>, CompileError> {
        // A hole (`[, x]`) has nothing to rewrite.
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
            // An object-LITERAL property: a shorthand `{ $s }` (≡ `{ $s: $s }`)
            // whose value is rewritten must un-shorthand — the value is no longer
            // the key identifier (`{ $s: $.store_get(…) }`).
            ObjectProperty::Property(prop) => {
                self.property(prop, true)?.map(ObjectProperty::Property)
            }
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
            // A destructuring-PATTERN property: shorthand means the key equals the
            // BOUND name (`{ value = default }`), which rewriting the default never
            // changes — so it stays shorthand (`{ value = $.store_get(…) }`, NOT
            // `{ value: value = … }`).
            ObjectPatternProperty::Property(prop) => self
                .property(prop, false)?
                .map(ObjectPatternProperty::Property),
            ObjectPatternProperty::RestElement(rest) => {
                self.expr_ref(rest.argument)?.map(|argument| {
                    ObjectPatternProperty::RestElement(RestElement {
                        argument,
                        ..rest.clone()
                    })
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
        prop: &Property<'arena>,
        unshorthand_on_change: bool,
    ) -> Result<Option<Property<'arena>>, CompileError> {
        let key = if prop.computed {
            self.expr(prop.key)?
        } else {
            None
        };
        let value = self.expr(prop.value)?;
        if key.is_none() && value.is_none() {
            return Ok(None);
        }
        let shorthand = prop.shorthand && !(unshorthand_on_change && value.is_some());
        Ok(Some(Property {
            key: key.map_or(prop.key, |key| &*self.b.arena.alloc(key)),
            value: value.map_or(prop.value, |value| &*self.b.arena.alloc(value)),
            shorthand,
            ..prop.clone()
        }))
    }

    // ── Writes and updates ─────────────────────────────────────────────────

    /// Classify an assignment/update target: a bare `$name` store (the supported
    /// write), a member/destructuring write over a store (refused), or a plain
    /// lvalue (recurse for reads).
    fn classify_target(&self, left: &Expression<'_>) -> Result<StoreTarget, CompileError> {
        if let ExpressionKind::Identifier(id) = &left.kind {
            return Ok(match self.store_base(id) {
                Some(base) => StoreTarget::Bare(base),
                None => StoreTarget::NotStore,
            });
        }
        // A member chain rooted at a store `$obj.foo = …` → `$.store_mutate` (not
        // implemented). A member rooted at a plain lvalue (`x[$c] = …`) is not a
        // store write — its store lives in a read position, so recurse instead.
        if matches!(left.kind, ExpressionKind::MemberExpression(_)) {
            if let ExpressionKind::Identifier(root) = &member_root(left).kind
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
        assign: &AssignmentExpression<'arena>,
        span: Span,
    ) -> Result<Option<Expression<'arena>>, CompileError> {
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
                    let bin_ref: &'arena Expression<'arena> = self.b.arena.alloc(bin);
                    self.rewrite_value(bin_ref)?
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
                            ..assign.clone()
                        }),
                    }))
                }
            }
        }
    }

    fn update(
        &mut self,
        update: &tsv_ts::ast::internal::UpdateExpression<'arena>,
        span: Span,
    ) -> Result<Option<Expression<'arena>>, CompileError> {
        // A bare `$name++` / `++$name` store update.
        if let ExpressionKind::Identifier(id) = &update.argument.kind
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
        if matches!(update.argument.kind, ExpressionKind::MemberExpression(_))
            && let ExpressionKind::Identifier(root) = &member_root(update.argument).kind
            && self.store_base(root).is_some()
        {
            return unsupported(Refusal::StoreMemberWrite);
        }
        Ok(self.expr_ref(update.argument)?.map(|argument| Expression {
            span,
            kind: ExpressionKind::UpdateExpression(tsv_ts::ast::internal::UpdateExpression {
                argument,
                ..update.clone()
            }),
        }))
    }
}
