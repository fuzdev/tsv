//! Rune refusal walk over borrowed script statements.
//!
//! The server transform rewrites exactly one rune shape (a top-level
//! `let … = $props()` declarator init). Every other `$`-prefixed identifier in
//! a walked value position must REFUSE rather than pass through into
//! runtime-broken JS — rune calls in statement position (`$effect(() => {})`),
//! nested functions (`function f() { let c = $state(0); }`), member-form calls
//! (`$state.raw([])`, `$props.id()`), and bare *references* (`let x = $state;`,
//! a future `$store` subscription) alike. This module is that guarantee: an
//! exhaustive walk of every expression-bearing position, refusing any
//! `$`-prefixed identifier reference it reaches. Calls report the callee root
//! as a rune for the clearer message; non-computed member property names and
//! non-computed object keys are *names*, not references, and stay allowed
//! (`obj.$foo` is fine).
//!
//! The matches are exhaustive on purpose — a new `Statement`/`Expression`
//! variant fails compilation here instead of silently skipping the guard.
//! TS *type* positions are not walked (nothing in type position evaluates).

use tsv_ts::ast::internal::{
    ArrowFunctionBody, ClassBody, ClassMember, ExportDefaultValue, Expression, ForInOfLeft,
    ForInit, FunctionExpression, ObjectPatternProperty, ObjectProperty, Statement,
};

use crate::CompileError;

/// Refuse any rune call anywhere in `stmt` (see module docs). The one sanctioned
/// exception — the top-level `$props()` declarator init — is excluded by the
/// caller walking around it, not by this guard.
pub(crate) fn refuse_runes_in_statement(
    stmt: &Statement<'_>,
    source: &str,
) -> Result<(), CompileError> {
    walk_statement(stmt, source)
}

/// Refuse any rune call anywhere in `expr`.
pub(crate) fn refuse_runes_in_expression(
    expr: &Expression<'_>,
    source: &str,
) -> Result<(), CompileError> {
    walk_expression(expr, source)
}

/// The `$`-prefixed name of a plain identifier, or `None`. Parsed identifiers
/// are span-identity (`escaped: None`); an interned (escaped) name is synthetic
/// (`$$renderer`, `$$props`, …) and never refused.
fn dollar_identifier_name<'s>(
    id: &tsv_ts::ast::internal::Identifier<'_>,
    source: &'s str,
) -> Option<&'s str> {
    if id.escaped_name.is_some() {
        return None;
    }
    let start = id.span.start as usize;
    let name = &source[start..start + id.name_len as usize];
    name.starts_with('$').then_some(name)
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
    CompileError::Unsupported(format!("rune {name}"))
}

fn walk_statements(stmts: &[Statement<'_>], source: &str) -> Result<(), CompileError> {
    for stmt in stmts {
        walk_statement(stmt, source)?;
    }
    Ok(())
}

fn walk_statement(stmt: &Statement<'_>, source: &str) -> Result<(), CompileError> {
    match stmt {
        Statement::ExpressionStatement(s) => walk_expression(&s.expression, source),
        Statement::VariableDeclaration(s) => walk_variable_declaration(s, source),
        Statement::ReturnStatement(s) => walk_opt(s.argument.as_ref(), source),
        Statement::BlockStatement(s) => walk_statements(s.body, source),
        Statement::FunctionDeclaration(s) => {
            walk_expressions(s.params, source)?;
            walk_statements(s.body.body, source)
        }
        Statement::ClassDeclaration(s) => walk_class_body(&s.body, source),
        Statement::ExportNamedDeclaration(s) => match &s.declaration {
            Some(decl) => walk_statement(decl, source),
            None => Ok(()),
        },
        Statement::ExportDefaultDeclaration(s) => match &s.declaration {
            ExportDefaultValue::Expression(e) => walk_expression(e, source),
            ExportDefaultValue::FunctionDeclaration(f) => {
                walk_expressions(f.params, source)?;
                walk_statements(f.body.body, source)
            }
            ExportDefaultValue::ClassDeclaration(c) => walk_class_body(&c.body, source),
            ExportDefaultValue::TSDeclareFunction(_)
            | ExportDefaultValue::TSInterfaceDeclaration(_) => Ok(()),
        },
        Statement::IfStatement(s) => {
            walk_expression(&s.test, source)?;
            walk_statement(s.consequent, source)?;
            match s.alternate {
                Some(alt) => walk_statement(alt, source),
                None => Ok(()),
            }
        }
        Statement::ForStatement(s) => {
            match &s.init {
                Some(ForInit::VariableDeclaration(decl)) => {
                    walk_variable_declaration(decl, source)?;
                }
                Some(ForInit::Expression(e)) => walk_expression(e, source)?,
                None => {}
            }
            walk_opt(s.test.as_ref(), source)?;
            walk_opt(s.update.as_ref(), source)?;
            walk_statement(s.body, source)
        }
        Statement::ForInStatement(s) => {
            walk_for_left(&s.left, source)?;
            walk_expression(&s.right, source)?;
            walk_statement(s.body, source)
        }
        Statement::ForOfStatement(s) => {
            walk_for_left(&s.left, source)?;
            walk_expression(&s.right, source)?;
            walk_statement(s.body, source)
        }
        Statement::WhileStatement(s) => {
            walk_expression(&s.test, source)?;
            walk_statement(s.body, source)
        }
        Statement::DoWhileStatement(s) => {
            walk_statement(s.body, source)?;
            walk_expression(&s.test, source)
        }
        Statement::SwitchStatement(s) => {
            walk_expression(&s.discriminant, source)?;
            for case in s.cases {
                walk_opt(case.test.as_ref(), source)?;
                walk_statements(case.consequent, source)?;
            }
            Ok(())
        }
        Statement::TryStatement(s) => {
            walk_statements(s.block.body, source)?;
            if let Some(handler) = &s.handler {
                walk_opt(handler.param.as_ref(), source)?;
                walk_statements(handler.body.body, source)?;
            }
            if let Some(finalizer) = &s.finalizer {
                walk_statements(finalizer.body, source)?;
            }
            Ok(())
        }
        Statement::ThrowStatement(s) => walk_expression(&s.argument, source),
        Statement::LabeledStatement(s) => walk_statement(s.body, source),
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
        Statement::TSEnumDeclaration(_) | Statement::TSModuleDeclaration(_) => Err(
            CompileError::Unsupported("TS enum/module declaration in instance script".to_string()),
        ),
        Statement::TSExportAssignment(s) => walk_expression(&s.expression, source),
    }
}

fn walk_variable_declaration(
    decl: &tsv_ts::ast::internal::VariableDeclaration<'_>,
    source: &str,
) -> Result<(), CompileError> {
    for declarator in decl.declarations {
        walk_expression(&declarator.id, source)?;
        walk_opt(declarator.init.as_ref(), source)?;
    }
    Ok(())
}

fn walk_for_left(left: &ForInOfLeft<'_>, source: &str) -> Result<(), CompileError> {
    match left {
        ForInOfLeft::VariableDeclaration(decl) => walk_variable_declaration(decl, source),
        ForInOfLeft::Pattern(pattern) => walk_expression(pattern, source),
    }
}

fn walk_opt(expr: Option<&Expression<'_>>, source: &str) -> Result<(), CompileError> {
    match expr {
        Some(e) => walk_expression(e, source),
        None => Ok(()),
    }
}

fn walk_expressions(exprs: &[Expression<'_>], source: &str) -> Result<(), CompileError> {
    for expr in exprs {
        walk_expression(expr, source)?;
    }
    Ok(())
}

fn walk_function_expression(f: &FunctionExpression<'_>, source: &str) -> Result<(), CompileError> {
    walk_expressions(f.params, source)?;
    walk_statements(f.body.body, source)
}

fn walk_class_body(body: &ClassBody<'_>, source: &str) -> Result<(), CompileError> {
    for member in body.body {
        match member {
            ClassMember::MethodDefinition(m) => {
                if m.computed {
                    walk_expression(&m.key, source)?;
                }
                walk_function_expression(&m.value, source)?;
            }
            ClassMember::PropertyDefinition(p) => {
                if p.computed {
                    walk_expression(&p.key, source)?;
                }
                walk_opt(p.value.as_ref(), source)?;
            }
            ClassMember::StaticBlock(b) => walk_statements(b.body, source)?,
            ClassMember::IndexSignature(_) => {}
        }
    }
    Ok(())
}

fn walk_expression(expr: &Expression<'_>, source: &str) -> Result<(), CompileError> {
    match expr {
        // The guard itself: any call/new whose callee roots in a `$`-identifier.
        Expression::CallExpression(call) => {
            if let Some(name) = dollar_callee_root(call.callee, source) {
                return Err(rune_error(name));
            }
            walk_expression(call.callee, source)?;
            walk_expressions(call.arguments, source)
        }
        Expression::NewExpression(new_expr) => {
            if let Some(name) = dollar_callee_root(new_expr.callee, source) {
                return Err(rune_error(name));
            }
            walk_expression(new_expr.callee, source)?;
            walk_expressions(new_expr.arguments, source)
        }

        // A bare `$`-prefixed identifier reference (`let x = $state;`, a
        // `$store` subscription) is oracle-rejected input — refuse. Positions
        // that carry names rather than references (non-computed member
        // properties / object keys) are never walked, so `obj.$foo` stays fine.
        Expression::Identifier(id) => match dollar_identifier_name(id, source) {
            Some(name) => Err(CompileError::Unsupported(format!(
                "$-prefixed identifier {name}"
            ))),
            None => Ok(()),
        },

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
                            walk_expression(&p.key, source)?;
                        }
                        walk_expression(&p.value, source)?;
                    }
                    ObjectProperty::SpreadElement(s) => walk_expression(s.argument, source)?,
                }
            }
            Ok(())
        }
        Expression::ArrayExpression(arr) => {
            for element in arr.elements {
                walk_opt(element.as_ref(), source)?;
            }
            Ok(())
        }
        Expression::UnaryExpression(u) => walk_expression(u.argument, source),
        Expression::UpdateExpression(u) => walk_expression(u.argument, source),
        Expression::BinaryExpression(b) => {
            walk_expression(b.left, source)?;
            walk_expression(b.right, source)
        }
        Expression::MemberExpression(m) => {
            walk_expression(m.object, source)?;
            if m.computed {
                walk_expression(m.property, source)?;
            }
            Ok(())
        }
        Expression::ConditionalExpression(c) => {
            walk_expression(c.test, source)?;
            walk_expression(c.consequent, source)?;
            walk_expression(c.alternate, source)
        }
        Expression::ArrowFunctionExpression(a) => {
            walk_expressions(a.params, source)?;
            match &a.body {
                ArrowFunctionBody::Expression(e) => walk_expression(e, source),
                ArrowFunctionBody::BlockStatement(b) => walk_statements(b.body, source),
            }
        }
        Expression::FunctionExpression(f) => walk_function_expression(f, source),
        Expression::ClassExpression(c) => walk_class_body(&c.body, source),
        Expression::SpreadElement(s) => walk_expression(s.argument, source),
        Expression::TemplateLiteral(t) => walk_expressions(t.expressions, source),
        Expression::TaggedTemplateExpression(t) => {
            walk_expression(t.tag, source)?;
            walk_expressions(t.quasi.expressions, source)
        }
        Expression::AwaitExpression(a) => walk_expression(a.argument, source),
        Expression::YieldExpression(y) => match y.argument {
            Some(argument) => walk_expression(argument, source),
            None => Ok(()),
        },
        Expression::SequenceExpression(s) => walk_expressions(s.expressions, source),
        Expression::AssignmentExpression(a) => {
            walk_expression(a.left, source)?;
            walk_expression(a.right, source)
        }
        Expression::ObjectPattern(p) => {
            for prop in p.properties {
                match prop {
                    ObjectPatternProperty::Property(prop) => {
                        if prop.computed {
                            walk_expression(&prop.key, source)?;
                        }
                        walk_expression(&prop.value, source)?;
                    }
                    ObjectPatternProperty::RestElement(rest) => {
                        walk_expression(rest.argument, source)?;
                    }
                }
            }
            Ok(())
        }
        Expression::ArrayPattern(p) => {
            for element in p.elements {
                walk_opt(element.as_ref(), source)?;
            }
            Ok(())
        }
        Expression::AssignmentPattern(p) => {
            walk_expression(p.left, source)?;
            walk_expression(p.right, source)
        }
        Expression::RestElement(r) => walk_expression(r.argument, source),
        Expression::TSTypeAssertion(t) => walk_expression(t.expression, source),
        Expression::TSAsExpression(t) => walk_expression(t.expression, source),
        Expression::TSSatisfiesExpression(t) => walk_expression(t.expression, source),
        Expression::TSInstantiationExpression(t) => walk_expression(t.expression, source),
        Expression::TSNonNullExpression(t) => walk_expression(t.expression, source),
        Expression::TSParameterProperty(t) => walk_expression(t.parameter, source),
        Expression::ImportExpression(i) => {
            walk_expression(i.source, source)?;
            match i.options {
                Some(options) => walk_expression(options, source),
                None => Ok(()),
            }
        }
        Expression::JsdocCast(j) => walk_expression(j.inner, source),
        Expression::ParenthesizedExpression(p) => walk_expression(p.expression, source),
    }
}
