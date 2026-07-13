//! Rune refusal walk over borrowed script statements — plus the collection
//! passes that ride the same traversal.
//!
//! The server transform rewrites a fixed set of rune shapes (top-level
//! `$props()` / `$state(…)` / `$derived(…)` declarator inits, statement-position
//! `$effect(…)`). Every other `$`-prefixed identifier in a walked value position
//! must REFUSE rather than pass through into runtime-broken JS — rune calls in
//! nested functions, member-form calls (`$props.id()`), and bare *references*
//! (`let x = $state;`, a future `$store` subscription) alike. Calls report
//! their callee root as a rune; name-only positions (non-computed member
//! properties / object keys) are not walked, so `obj.$foo` stays allowed.
//!
//! The same walk collects what the static evaluator (`analyze.rs`) needs:
//!
//! - **assignment/update target roots** (`updated` — an updated binding is
//!   never folded by the oracle), and
//! - **names declared in nested/block scopes** (`nested_declared` — a shadowed
//!   top-level name can't be trusted by this shadow-naive mutation collection,
//!   so the binding goes `Opaque` and refuses if it reaches an evaluated spine),
//!
//! and refuses **reads of derived bindings** (`derived_names`) — those must
//! become `d()` calls, which only the emitter's bare-expression positions can
//! express over borrowed code.
//!
//! The matches are exhaustive on purpose — a new `Statement`/`Expression`
//! variant fails compilation here instead of silently skipping the guard.
//! TS *type* positions are not walked (nothing in type position evaluates).

use std::collections::HashSet;

use tsv_ts::ast::internal::{
    ArrowFunctionBody, ClassBody, ClassMember, ExportDefaultValue, Expression, ForInOfLeft,
    ForInit, FunctionExpression, ObjectPatternProperty, ObjectProperty, Statement,
};

use crate::analyze::{NameSet, pattern_binding_names};
use crate::{CompileError, Refusal};

/// The walk's shared state: the source names resolve against, the collection
/// sinks, and the refusal set.
pub(crate) struct WalkCtx<'a> {
    pub source: &'a str,
    /// Assignment/update target root names (fed back as `updated` bindings).
    pub updated: &'a mut HashSet<String>,
    /// Names declared in nested function/block scopes (shadow candidates).
    pub nested_declared: &'a mut HashSet<String>,
    /// Derived binding names — reading one anywhere in walked code refuses.
    pub derived_names: &'a HashSet<String>,
    /// Current function-nesting depth (0 = the statement being walked).
    fn_depth: usize,
}

impl<'a> WalkCtx<'a> {
    pub fn new(
        source: &'a str,
        updated: &'a mut HashSet<String>,
        nested_declared: &'a mut HashSet<String>,
        derived_names: &'a HashSet<String>,
    ) -> Self {
        Self {
            source,
            updated,
            nested_declared,
            derived_names,
            fn_depth: 0,
        }
    }
}

/// Walk one borrowed statement: refuse stray runes and derived reads, collect
/// mutations and nested declarations. `depth` is the statement nesting depth
/// (0 = a top-level script statement, whose declarations are the top bindings).
pub(crate) fn walk_statement_guarded(
    stmt: &Statement<'_>,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    walk_statement(stmt, ctx, depth)
}

/// Walk one expression (template expressions, rewritten declarator pieces).
pub(crate) fn walk_expression_guarded(
    expr: &Expression<'_>,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    walk_expression(expr, ctx)
}

/// The `$`-prefixed name of a plain identifier, or `None`. Parsed identifiers
/// are span-identity (`escaped: None`); an interned (escaped) name is synthetic
/// (`$$renderer`, `$$props`, …) and never refused.
fn dollar_identifier_name<'s>(
    id: &tsv_ts::ast::internal::Identifier<'_>,
    source: &'s str,
) -> Option<&'s str> {
    let name = identifier_name(id, source)?;
    name.starts_with('$').then_some(name)
}

fn identifier_name<'s>(
    id: &tsv_ts::ast::internal::Identifier<'_>,
    source: &'s str,
) -> Option<&'s str> {
    if id.escaped_name.is_some() {
        return None;
    }
    let start = id.span.start as usize;
    Some(&source[start..start + id.name_len as usize])
}

/// The `$`-prefixed root-identifier name of a callee, peeled through member
/// accesses (`$state.raw`), non-null assertions, instantiations, and preserved
/// parens — `None` when the root is not a plain `$`-identifier.
fn dollar_callee_root<'s>(callee: &Expression<'_>, source: &'s str) -> Option<&'s str> {
    match callee {
        Expression::Identifier(id) => dollar_identifier_name(id, source),
        Expression::MemberExpression(member) => dollar_callee_root(member.object, source),
        Expression::TSNonNullExpression(non_null) => {
            dollar_callee_root(non_null.expression, source)
        }
        Expression::TSInstantiationExpression(inst) => dollar_callee_root(inst.expression, source),
        Expression::ParenthesizedExpression(paren) => dollar_callee_root(paren.expression, source),
        _ => None,
    }
}

fn rune_error(name: &str) -> CompileError {
    CompileError::Unsupported(Refusal::Rune {
        name: name.to_string(),
    })
}

/// Record the root identifier(s) of an assignment target (through member
/// chains and destructuring patterns) into `out` as reassigned/updated names.
///
/// Shared by the guard walk and the whole-component reassignment collection in
/// `needs_context` (which must see mutations inside dropped event handlers so a
/// reassigned binding is never statically folded).
pub(crate) fn assign_target_roots(target: &Expression<'_>, source: &str, out: &mut NameSet) {
    match target {
        Expression::Identifier(id) => {
            if let Some(name) = identifier_name(id, source) {
                out.insert(name.to_string());
            }
        }
        Expression::MemberExpression(m) => assign_target_roots(m.object, source, out),
        Expression::TSNonNullExpression(t) => assign_target_roots(t.expression, source, out),
        Expression::TSAsExpression(t) => assign_target_roots(t.expression, source, out),
        Expression::ParenthesizedExpression(p) => assign_target_roots(p.expression, source, out),
        Expression::ObjectPattern(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectPatternProperty::Property(p) => {
                        assign_target_roots(&p.value, source, out);
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        assign_target_roots(rest.argument, source, out);
                    }
                }
            }
        }
        Expression::ArrayPattern(arr) => {
            for element in arr.elements.iter().flatten() {
                assign_target_roots(element, source, out);
            }
        }
        Expression::AssignmentPattern(a) => assign_target_roots(a.left, source, out),
        Expression::RestElement(r) => assign_target_roots(r.argument, source, out),
        _ => {}
    }
}

/// Record the names a declaration pattern declares into `nested_declared`
/// (best-effort — unusual pattern shapes just record nothing extra).
fn collect_nested_declared(pattern: &Expression<'_>, ctx: &mut WalkCtx<'_>) {
    let mut names = Vec::new();
    if pattern_binding_names(pattern, ctx.source, &mut names).is_ok() {
        for name in names {
            ctx.nested_declared.insert(name);
        }
    }
}

fn enter_function(params: &[Expression<'_>], ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    ctx.fn_depth += 1;
    for param in params {
        collect_nested_declared(param, ctx);
        walk_expression(param, ctx)?;
    }
    Ok(())
}

fn walk_statements(
    stmts: &[Statement<'_>],
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    for stmt in stmts {
        walk_statement(stmt, ctx, depth)?;
    }
    Ok(())
}

fn walk_statement(
    stmt: &Statement<'_>,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    match stmt {
        Statement::ExpressionStatement(s) => walk_expression(&s.expression, ctx),
        Statement::VariableDeclaration(s) => walk_variable_declaration(s, ctx, depth),
        Statement::ReturnStatement(s) => walk_opt(s.argument.as_ref(), ctx),
        Statement::BlockStatement(s) => walk_statements(s.body, ctx, depth + 1),
        Statement::FunctionDeclaration(s) => {
            if (depth > 0 || ctx.fn_depth > 0)
                && let Some(id) = &s.id
                && let Some(name) = identifier_name(id, ctx.source)
            {
                ctx.nested_declared.insert(name.to_string());
            }
            enter_function(s.params, ctx)?;
            let result = walk_statements(s.body.body, ctx, depth + 1);
            ctx.fn_depth -= 1;
            result
        }
        Statement::ClassDeclaration(s) => walk_class_body(&s.body, ctx),
        Statement::ExportNamedDeclaration(s) => match &s.declaration {
            Some(decl) => walk_statement(decl, ctx, depth),
            None => Ok(()),
        },
        Statement::ExportDefaultDeclaration(s) => match &s.declaration {
            ExportDefaultValue::Expression(e) => walk_expression(e, ctx),
            ExportDefaultValue::FunctionDeclaration(f) => {
                enter_function(f.params, ctx)?;
                let result = walk_statements(f.body.body, ctx, depth + 1);
                ctx.fn_depth -= 1;
                result
            }
            ExportDefaultValue::ClassDeclaration(c) => walk_class_body(&c.body, ctx),
            ExportDefaultValue::TSDeclareFunction(_)
            | ExportDefaultValue::TSInterfaceDeclaration(_) => Ok(()),
        },
        Statement::IfStatement(s) => {
            walk_expression(&s.test, ctx)?;
            walk_statement(s.consequent, ctx, depth + 1)?;
            match s.alternate {
                Some(alt) => walk_statement(alt, ctx, depth + 1),
                None => Ok(()),
            }
        }
        Statement::ForStatement(s) => {
            match &s.init {
                Some(ForInit::VariableDeclaration(decl)) => {
                    // For-scope declarations are block-scoped — always shadow
                    // candidates regardless of depth.
                    for declarator in decl.declarations {
                        collect_nested_declared(&declarator.id, ctx);
                    }
                    walk_variable_declaration(decl, ctx, depth + 1)?;
                }
                Some(ForInit::Expression(e)) => walk_expression(e, ctx)?,
                None => {}
            }
            walk_opt(s.test.as_ref(), ctx)?;
            walk_opt(s.update.as_ref(), ctx)?;
            walk_statement(s.body, ctx, depth + 1)
        }
        Statement::ForInStatement(s) => {
            walk_for_left(&s.left, ctx, depth)?;
            walk_expression(&s.right, ctx)?;
            walk_statement(s.body, ctx, depth + 1)
        }
        Statement::ForOfStatement(s) => {
            walk_for_left(&s.left, ctx, depth)?;
            walk_expression(&s.right, ctx)?;
            walk_statement(s.body, ctx, depth + 1)
        }
        Statement::WhileStatement(s) => {
            walk_expression(&s.test, ctx)?;
            walk_statement(s.body, ctx, depth + 1)
        }
        Statement::DoWhileStatement(s) => {
            walk_statement(s.body, ctx, depth + 1)?;
            walk_expression(&s.test, ctx)
        }
        Statement::SwitchStatement(s) => {
            walk_expression(&s.discriminant, ctx)?;
            for case in s.cases {
                walk_opt(case.test.as_ref(), ctx)?;
                walk_statements(case.consequent, ctx, depth + 1)?;
            }
            Ok(())
        }
        Statement::TryStatement(s) => {
            walk_statements(s.block.body, ctx, depth + 1)?;
            if let Some(handler) = &s.handler {
                if let Some(param) = &handler.param {
                    collect_nested_declared(param, ctx);
                    walk_expression(param, ctx)?;
                }
                walk_statements(handler.body.body, ctx, depth + 1)?;
            }
            if let Some(finalizer) = &s.finalizer {
                walk_statements(finalizer.body, ctx, depth + 1)?;
            }
            Ok(())
        }
        Statement::ThrowStatement(s) => walk_expression(&s.argument, ctx),
        Statement::LabeledStatement(s) => walk_statement(s.body, ctx, depth + 1),
        // No expression-bearing children.
        Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::ImportDeclaration(_)
        | Statement::ExportAllDeclaration(_)
        | Statement::TSNamespaceExportDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSDeclareFunction(_) => Ok(()),
        // Enum/module bodies can carry initializer expressions; walking their
        // internals isn't wired yet, so refuse rather than under-guard.
        Statement::TSEnumDeclaration(_) | Statement::TSModuleDeclaration(_) => {
            Err(CompileError::Unsupported(Refusal::TsEnumOrModule))
        }
        Statement::TSExportAssignment(s) => walk_expression(&s.expression, ctx),
    }
}

fn walk_variable_declaration(
    decl: &tsv_ts::ast::internal::VariableDeclaration<'_>,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    for declarator in decl.declarations {
        if depth > 0 || ctx.fn_depth > 0 {
            collect_nested_declared(&declarator.id, ctx);
        }
        walk_expression(&declarator.id, ctx)?;
        walk_opt(declarator.init.as_ref(), ctx)?;
    }
    Ok(())
}

fn walk_for_left(
    left: &ForInOfLeft<'_>,
    ctx: &mut WalkCtx<'_>,
    depth: usize,
) -> Result<(), CompileError> {
    match left {
        ForInOfLeft::VariableDeclaration(decl) => {
            for declarator in decl.declarations {
                collect_nested_declared(&declarator.id, ctx);
            }
            walk_variable_declaration(decl, ctx, depth + 1)
        }
        ForInOfLeft::Pattern(pattern) => {
            assign_target_roots(pattern, ctx.source, ctx.updated);
            walk_expression(pattern, ctx)
        }
    }
}

fn walk_opt(expr: Option<&Expression<'_>>, ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    match expr {
        Some(e) => walk_expression(e, ctx),
        None => Ok(()),
    }
}

fn walk_expressions(exprs: &[Expression<'_>], ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    for expr in exprs {
        walk_expression(expr, ctx)?;
    }
    Ok(())
}

fn walk_function_expression(
    f: &FunctionExpression<'_>,
    ctx: &mut WalkCtx<'_>,
) -> Result<(), CompileError> {
    if let Some(id) = &f.id
        && let Some(name) = identifier_name(id, ctx.source)
    {
        ctx.nested_declared.insert(name.to_string());
    }
    enter_function(f.params, ctx)?;
    let result = walk_statements(f.body.body, ctx, 1);
    ctx.fn_depth -= 1;
    result
}

fn walk_class_body(body: &ClassBody<'_>, ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    for member in body.body {
        match member {
            ClassMember::MethodDefinition(m) => {
                if m.computed {
                    walk_expression(&m.key, ctx)?;
                }
                walk_function_expression(&m.value, ctx)?;
            }
            ClassMember::PropertyDefinition(p) => {
                if p.computed {
                    walk_expression(&p.key, ctx)?;
                }
                walk_opt(p.value.as_ref(), ctx)?;
            }
            ClassMember::StaticBlock(b) => {
                ctx.fn_depth += 1;
                let result = walk_statements(b.body, ctx, 1);
                ctx.fn_depth -= 1;
                result?;
            }
            ClassMember::IndexSignature(_) => {}
        }
    }
    Ok(())
}

fn walk_expression(expr: &Expression<'_>, ctx: &mut WalkCtx<'_>) -> Result<(), CompileError> {
    match expr {
        // The rune guard: any call/new whose callee roots in a `$`-identifier.
        Expression::CallExpression(call) => {
            if let Some(name) = dollar_callee_root(call.callee, ctx.source) {
                return Err(rune_error(name));
            }
            walk_expression(call.callee, ctx)?;
            walk_expressions(call.arguments, ctx)
        }
        Expression::NewExpression(new_expr) => {
            if let Some(name) = dollar_callee_root(new_expr.callee, ctx.source) {
                return Err(rune_error(name));
            }
            walk_expression(new_expr.callee, ctx)?;
            walk_expressions(new_expr.arguments, ctx)
        }

        // A bare `$`-prefixed identifier reference (`let x = $state;`, a
        // `$store` subscription) is oracle-rejected input — refuse. A derived
        // binding read outside the emitter's bare positions must become `d()`,
        // which borrowed code can't express — refuse. Name-only positions
        // (non-computed member properties / object keys) are never walked.
        Expression::Identifier(id) => {
            if let Some(name) = dollar_identifier_name(id, ctx.source) {
                // `$$slots` is a real runtime reference (the transform injects
                // `const $$slots = $.sanitize_slots($$props)`), not a rune.
                if name != "$$slots" {
                    return Err(CompileError::Unsupported(
                        Refusal::DollarPrefixedIdentifier {
                            name: name.to_string(),
                        },
                    ));
                }
            }
            if let Some(name) = identifier_name(id, ctx.source)
                && ctx.derived_names.contains(name)
            {
                return Err(CompileError::Unsupported(Refusal::DerivedBindingRead {
                    name: name.to_string(),
                }));
            }
            Ok(())
        }

        // Leaves.
        Expression::Literal(_)
        | Expression::PrivateIdentifier(_)
        | Expression::RegexLiteral(_)
        | Expression::ThisExpression(_)
        | Expression::Super(_)
        | Expression::MetaProperty(_) => Ok(()),

        Expression::ObjectExpression(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectProperty::Property(p) => {
                        if p.computed {
                            walk_expression(&p.key, ctx)?;
                        }
                        walk_expression(&p.value, ctx)?;
                    }
                    ObjectProperty::SpreadElement(s) => walk_expression(s.argument, ctx)?,
                }
            }
            Ok(())
        }
        Expression::ArrayExpression(arr) => {
            for element in arr.elements {
                walk_opt(element.as_ref(), ctx)?;
            }
            Ok(())
        }
        Expression::UnaryExpression(u) => walk_expression(u.argument, ctx),
        Expression::UpdateExpression(u) => {
            assign_target_roots(u.argument, ctx.source, ctx.updated);
            walk_expression(u.argument, ctx)
        }
        Expression::BinaryExpression(b) => {
            walk_expression(b.left, ctx)?;
            walk_expression(b.right, ctx)
        }
        Expression::MemberExpression(m) => {
            walk_expression(m.object, ctx)?;
            if m.computed {
                walk_expression(m.property, ctx)?;
            }
            Ok(())
        }
        Expression::ConditionalExpression(c) => {
            walk_expression(c.test, ctx)?;
            walk_expression(c.consequent, ctx)?;
            walk_expression(c.alternate, ctx)
        }
        Expression::ArrowFunctionExpression(a) => {
            enter_function(a.params, ctx)?;
            let result = match &a.body {
                ArrowFunctionBody::Expression(e) => walk_expression(e, ctx),
                ArrowFunctionBody::BlockStatement(b) => walk_statements(b.body, ctx, 1),
            };
            ctx.fn_depth -= 1;
            result
        }
        Expression::FunctionExpression(f) => walk_function_expression(f, ctx),
        Expression::ClassExpression(c) => walk_class_body(&c.body, ctx),
        Expression::SpreadElement(s) => walk_expression(s.argument, ctx),
        Expression::TemplateLiteral(t) => walk_expressions(t.expressions, ctx),
        Expression::TaggedTemplateExpression(t) => {
            walk_expression(t.tag, ctx)?;
            walk_expressions(t.quasi.expressions, ctx)
        }
        // Top-level/template `await` forces the oracle's async-component
        // shapes (blockers, thunked pushes) — not implemented, refuse. Inside
        // a nested function it is ordinary code and passes through.
        Expression::AwaitExpression(a) => {
            if ctx.fn_depth == 0 {
                return Err(CompileError::Unsupported(Refusal::TopLevelAwait));
            }
            walk_expression(a.argument, ctx)
        }
        Expression::YieldExpression(y) => match y.argument {
            Some(argument) => walk_expression(argument, ctx),
            None => Ok(()),
        },
        Expression::SequenceExpression(s) => walk_expressions(s.expressions, ctx),
        Expression::AssignmentExpression(a) => {
            assign_target_roots(a.left, ctx.source, ctx.updated);
            walk_expression(a.left, ctx)?;
            walk_expression(a.right, ctx)
        }
        Expression::ObjectPattern(p) => {
            for prop in p.properties {
                match prop {
                    ObjectPatternProperty::Property(prop) => {
                        if prop.computed {
                            walk_expression(&prop.key, ctx)?;
                        }
                        walk_expression(&prop.value, ctx)?;
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        walk_expression(rest.argument, ctx)?;
                    }
                }
            }
            Ok(())
        }
        Expression::ArrayPattern(p) => {
            for element in p.elements {
                walk_opt(element.as_ref(), ctx)?;
            }
            Ok(())
        }
        Expression::AssignmentPattern(p) => {
            walk_expression(p.left, ctx)?;
            walk_expression(p.right, ctx)
        }
        Expression::RestElement(r) => walk_expression(r.argument, ctx),
        Expression::TSTypeAssertion(t) => walk_expression(t.expression, ctx),
        Expression::TSAsExpression(t) => walk_expression(t.expression, ctx),
        Expression::TSSatisfiesExpression(t) => walk_expression(t.expression, ctx),
        Expression::TSInstantiationExpression(t) => walk_expression(t.expression, ctx),
        Expression::TSNonNullExpression(t) => walk_expression(t.expression, ctx),
        Expression::TSParameterProperty(t) => walk_expression(t.parameter, ctx),
        Expression::ImportExpression(i) => {
            walk_expression(i.source, ctx)?;
            match i.options {
                Some(options) => walk_expression(options, ctx),
                None => Ok(()),
            }
        }
        Expression::JsdocCast(j) => walk_expression(j.inner, ctx),
        Expression::ParenthesizedExpression(p) => walk_expression(p.expression, ctx),
    }
}
