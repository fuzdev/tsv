//! The single exhaustive answer to **"what does this script declare at script
//! scope?"**, plus the identifier-name helpers every script analysis reads.
//!
//! The script-side analog of [`crate::attr_refs`]'s shared template traversals:
//! a seam with more than one consumer, so the `Statement` enumeration exists
//! ONCE. Both the binding-table analysis ([`crate::script_bindings`]) and the
//! rune/store collision pre-pass ([`crate::script_collision`]) route through
//! [`each_script_declaration`]; a hand-copied enumeration with a `_ => {}` tail
//! is exactly how one goes stale and starts missing bindings, which for the
//! collision walk is an under-refusal — a MISMATCH.
//!
//! Target-independent: this is a fact about the script's scope, not about what
//! any transform emits.

use tsv_ts::ast::internal::{Expression, ImportDeclaration, ImportSpecifier, Statement};

/// Which of a script's `var` declarations [`each_script_declaration`] reports.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VarScope {
    /// Only the statements at the list's own level — the shape the binding-table
    /// analysis wants (it classifies *top-level declarations*, in source order).
    TopLevelOnly,
    /// Plus every `var` declarator hoisted out of a nested block, for-head, or
    /// other non-function statement. `var` is function-scoped, so such a
    /// declarator lands in the script's own scope exactly like a top-level one —
    /// which is what a "what does this script DECLARE" question must see.
    WithHoistedVars,
}

/// One binding a script's statements introduce at SCRIPT scope.
pub(crate) enum ScriptDeclaration<'arena> {
    /// A `var`/`let`/`const`/`using` declarator.
    Declarator {
        declarator: &'arena tsv_ts::ast::internal::VariableDeclarator<'arena>,
        /// Whether this declarator reached script scope by hoisting **through a
        /// POROUS scope** — a block, a for-head/body, a switch, a try/catch. The
        /// oracle re-declares such a `var` on the parent scope and passes NO
        /// initializer while doing so (`phases/scope.js:673-681` —
        /// `return this.parent.declare(node, kind, declaration_kind)`, the
        /// 4-argument `initial` defaulting to `null`), so the binding it creates
        /// reads as having no initializer at all. A declarator at the script's
        /// own level keeps its `initial`.
        initial_dropped: bool,
    },
    /// A `function name(…) {}` declaration's name.
    Function(&'arena tsv_ts::ast::internal::Identifier<'arena>),
    /// A `class Name {}` declaration's name.
    Class(&'arena tsv_ts::ast::internal::Identifier<'arena>),
    /// One `import` specifier's local name, with the declaration it came from —
    /// the oracle's `Binding.initial` for an import binding is the whole
    /// `ImportDeclaration`, and its `source` is what the `svelte/store` carve-out
    /// tests.
    Import {
        local: &'arena tsv_ts::ast::internal::Identifier<'arena>,
        declaration: &'arena ImportDeclaration<'arena>,
    },
}

/// The single answer to **"what does this script declare at script scope?"**.
///
/// Every analysis that needs that answer routes through here. The match is
/// **exhaustive on purpose** — a new `Statement` variant fails compilation rather
/// than being silently skipped, which is exactly how a hand-copied enumeration
/// with a `_ => {}` tail goes stale and starts missing bindings (an
/// under-refusal, i.e. a MISMATCH, for the collision walk below).
pub(crate) fn each_script_declaration<'arena, E>(
    stmts: &'arena [Statement<'arena>],
    var_scope: VarScope,
    f: &mut impl FnMut(ScriptDeclaration<'arena>) -> Result<(), E>,
) -> Result<(), E> {
    for stmt in stmts {
        script_declarations_of(stmt, var_scope, true, false, f)?;
    }
    Ok(())
}

/// `top` distinguishes the script's own statement list from a nested one. Below
/// the top level only a `var` reaches script scope: `let`/`const` are
/// block-scoped, and a nested `function`/`class` declaration is block-scoped too
/// (tsv is strict-mode-only, so Annex B function hoisting does not apply).
///
/// `porous` records whether at least one POROUS scope sits between `stmt` and the
/// script scope, because that changes what the oracle's binding CARRIES — see
/// [`ScriptDeclaration::Declarator`]'s `initial_dropped`. A porous scope is
/// exactly the set of containers `phases/scope.js` gives a `scope.child(true)`:
/// `BlockStatement`, `ForStatement` / `ForInStatement` / `ForOfStatement`,
/// `SwitchStatement` (all four via `create_block_scope`), and a `CatchClause`
/// with a parameter. An `if` / `while` / `do` / `try` statement gets no visitor at
/// all, so it is transparent — probe-verified: `if (x) var state = $state(0);`
/// keeps its initializer (the oracle does NOT reclassify it), while
/// `if (x) { var state = $state(0); }` loses it (the oracle DOES).
///
/// A `LabeledStatement` is transparent here, which is exact for every label but
/// one. The oracle's visitor (`phases/scope.js:1063-1069`) falls through to a bare
/// `next()` unless the label is at `path.length === 1`, is named `$`, AND the
/// script was scoped with `allow_reactive_declarations`. That flag is NOT
/// mode-keyed: `2-analyze/index.js:336-337` passes `false` for the MODULE script
/// and `true` for the INSTANCE script, in runes mode as much as in legacy mode. So
/// a top-level `$:` in the instance script does get a scope — a NON-porous one
/// (`scope.child()`, `porous = false` by default, `scope.js:702`), meaning its
/// `var` should not reach script scope at all, and this walk over-collects it.
/// Harmless: a top-level `$:` is invalid in runes mode and refuses on its own
/// path, so the only effect is which refusal bucket a `$: var state = …` document
/// lands in.
///
/// ⚠️ A **class body is opaque** here, on both the declaration and the expression
/// side. A class static block is the one nested statement list the oracle gives no
/// scope at all (`phases/scope.js` has no `StaticBlock` visitor), so a `var` there
/// really does reach script scope — but reaching every class body a statement can
/// hold means traversing every expression position of every statement, a
/// hand-enumerated surface that has twice shipped with holes. The collision check
/// handles the whole family with a lexical fence instead
/// ([`crate::script_collision::script_contains_static_block`]), so this walk
/// deliberately stops at a class body and no consumer depends on it descending.
fn script_declarations_of<'arena, E>(
    stmt: &'arena Statement<'arena>,
    var_scope: VarScope,
    top: bool,
    porous: bool,
    f: &mut dyn FnMut(ScriptDeclaration<'arena>) -> Result<(), E>,
) -> Result<(), E> {
    use tsv_ts::ast::internal::{ForInOfLeft, ForInit, VariableDeclarationKind};

    // Descending at all is only ever about collecting hoisted `var`s.
    let descend = matches!(var_scope, VarScope::WithHoistedVars);
    // A statement body that is transparent in the oracle's scope walk.
    macro_rules! nested {
        ($s:expr) => {
            if descend {
                script_declarations_of($s, var_scope, false, porous, f)?;
            }
        };
    }
    // A statement body the oracle wraps in a porous scope.
    macro_rules! nested_porous {
        ($s:expr) => {
            if descend {
                script_declarations_of($s, var_scope, false, true, f)?;
            }
        };
    }
    macro_rules! head_declaration {
        ($decl:expr) => {
            // A for-head is never the script's own statement list, so only `var` —
            // and the for statement's own scope is porous, so the initializer goes.
            if descend && $decl.kind == VariableDeclarationKind::Var {
                for declarator in $decl.declarations {
                    f(ScriptDeclaration::Declarator {
                        declarator,
                        initial_dropped: true,
                    })?;
                }
            }
        };
    }

    match stmt {
        Statement::VariableDeclaration(decl) => {
            if top || (descend && decl.kind == VariableDeclarationKind::Var) {
                for declarator in decl.declarations {
                    f(ScriptDeclaration::Declarator {
                        declarator,
                        initial_dropped: porous,
                    })?;
                }
            }
        }
        Statement::FunctionDeclaration(fun) => {
            if top && let Some(id) = fun.id.as_ref() {
                f(ScriptDeclaration::Function(id))?;
            }
            // A function BODY is a new function scope — no `var` escapes it.
        }
        Statement::ClassDeclaration(class) => {
            if top && let Some(id) = class.id.as_ref() {
                f(ScriptDeclaration::Class(id))?;
            }
            // A class body is deliberately OPAQUE here — see the ⚠️ note on this
            // function. A static block's `var` genuinely does reach script scope
            // in the oracle; `script_contains_static_block` is what covers it.
        }
        Statement::ImportDeclaration(import) => {
            for spec in import.specifiers {
                let local = match spec {
                    ImportSpecifier::Default(s) => &s.local,
                    ImportSpecifier::Named(s) => &s.local,
                    ImportSpecifier::Namespace(s) => &s.local,
                };
                f(ScriptDeclaration::Import {
                    local,
                    declaration: import,
                })?;
            }
        }
        // A module `export const`/`function`/`class`/`let`/`var` binds a
        // module-scope name the evaluator must see (an `export const a = 'ok'`
        // folds a template `{a}`), so recurse into the exported declaration at
        // the SAME level. (`export { a }` / `export … from` carry no
        // `declaration` and bind no new name.)
        Statement::ExportNamedDeclaration(export) => {
            if let Some(decl) = export.declaration {
                script_declarations_of(decl, var_scope, top, porous, f)?;
            }
        }
        // `export default function name() {}` binds `name`, but an instance-script
        // export and a module `export default` each refuse on their own path
        // (`Refusal::ModuleDefaultExport` / the instance-export refusal), so no
        // consumer of this walk can reach one.
        Statement::ExportDefaultDeclaration(_) => {}
        // Statement bodies that are NOT a new function scope: a `var` declared
        // inside one is function-scoped and lands in THIS script's scope.
        Statement::BlockStatement(block) => {
            for s in block.body {
                nested_porous!(s);
            }
        }
        Statement::IfStatement(stmt) => {
            nested!(stmt.consequent);
            if let Some(alternate) = stmt.alternate {
                nested!(alternate);
            }
        }
        Statement::ForStatement(stmt) => {
            if let Some(ForInit::VariableDeclaration(decl)) = stmt.init.as_ref() {
                head_declaration!(decl);
            }
            nested_porous!(stmt.body);
        }
        Statement::ForInStatement(stmt) => {
            if let ForInOfLeft::VariableDeclaration(decl) = &stmt.left {
                head_declaration!(decl);
            }
            nested_porous!(stmt.body);
        }
        Statement::ForOfStatement(stmt) => {
            if let ForInOfLeft::VariableDeclaration(decl) = &stmt.left {
                head_declaration!(decl);
            }
            nested_porous!(stmt.body);
        }
        Statement::WhileStatement(stmt) => nested!(stmt.body),
        Statement::DoWhileStatement(stmt) => nested!(stmt.body),
        Statement::LabeledStatement(stmt) => nested!(stmt.body),
        Statement::SwitchStatement(stmt) => {
            for case in stmt.cases {
                for s in case.consequent {
                    nested_porous!(s);
                }
            }
        }
        Statement::TryStatement(stmt) => {
            for s in stmt.block.body {
                nested_porous!(s);
            }
            if let Some(handler) = stmt.handler.as_ref() {
                for s in handler.body.body {
                    nested_porous!(s);
                }
            }
            if let Some(finalizer) = stmt.finalizer.as_ref() {
                for s in finalizer.body {
                    nested_porous!(s);
                }
            }
        }
        // Declare nothing at script scope. An EXPRESSION declares nothing at all
        // except through a class expression's static block, which is the fenced
        // family (see the ⚠️ note above), so no expression position is visited.
        // (A `return` only occurs inside a function, which is a scope boundary
        // this walk never enters.)
        Statement::ExpressionStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::ReturnStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::ExportAllDeclaration(_) => {}
        // TypeScript-only statements. Type erasure runs before every consumer of
        // this walk, so none of these survive to reach it; the arms exist so a
        // new variant still fails compilation here.
        Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSDeclareFunction(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::TSModuleDeclaration(_)
        | Statement::TSExportAssignment(_)
        | Statement::TSNamespaceExportDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_) => {}
    }
    Ok(())
}

pub(crate) fn plain_identifier_name(
    id: &tsv_ts::ast::internal::Identifier<'_>,
    source: &str,
) -> Option<String> {
    if id.escaped_name.is_some() {
        return None;
    }
    let start = id.span.start as usize;
    Some(source[start..start + id.name_len as usize].to_string())
}

pub(crate) fn identifier_binding_name(id: &Expression<'_>, source: &str) -> Option<String> {
    let Expression::Identifier(ident) = id else {
        return None;
    };
    plain_identifier_name(ident, source)
}
