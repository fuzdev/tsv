//! The syntactic check pass — a standalone walk over `&Program` that emits the
//! check-time diagnostics the binder's symbol cascade cannot (they are not
//! same-table flag conflicts).
//!
//! It is deliberately **not** the binder: it never consults the symbol tables
//! (walking the shared interface member table would break declaration-merging).
//! It descends every syntactic position — class / interface / type-literal bodies,
//! every type-annotation site (variable / parameter / return-type / predicate /
//! function-type / union / intersection / assertion target / …), class and
//! interface heritage type arguments, decorators (class / member / parameter),
//! template-literal-type interpolations, and every type-parameter declaration — and
//! runs a set of per-node checks. Those are the duplicate-member check and the
//! type-parameter-identity check (both in [`duplicate_members`]), sharing the one
//! descent.
//!
//! The output folds into each file's diagnostics in [`crate::program`], alongside
//! the bind product, then the whole program is canonically sorted + deduped — so a
//! diagnostic this pass and the binder both emit (identical span/code/args)
//! collapses to one, exactly as tsgo's binder + checker outputs do.
//
// tsgo: internal/checker/checker.go checkSourceElement dispatch (the per-node
//       checks this walk ports piecemeal)

mod duplicate_members;
pub(crate) mod unreachable;

use crate::diag::Diagnostic;
use crate::ids::FileId;
use duplicate_members::MemberCtx;
use string_interner::DefaultStringInterner;
use tsv_ts::ast::Program;
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ClassBody, ClassMember, Decorator, ExportDefaultValue, Expression,
    ForInOfLeft, ForInit, ObjectPatternProperty, ObjectProperty, Statement, TSInterfaceDeclaration,
    TSInterfaceHeritage, TSLiteralType, TSModuleDeclaration, TSModuleDeclarationBody, TSType,
    TSTypeAnnotation, TSTypeElement, TSTypeParameterDeclaration, TSTypeParameterInstantiation,
    VariableDeclaration,
};

/// Run the syntactic check pass over one parsed file, returning its check-time
/// diagnostics (unsorted — the program-wide sort/dedup canonicalizes order).
#[must_use]
pub fn check_file_members(program: &Program<'_>, source: &str, file: FileId) -> Vec<Diagnostic> {
    let interner = program.interner.borrow();
    let mut walk = CheckWalk {
        source,
        interner: &interner,
        file,
        diagnostics: Vec::new(),
    };
    for stmt in program.body {
        walk.visit_statement(stmt);
    }
    walk.diagnostics
}

/// The check walk's per-file state.
struct CheckWalk<'a> {
    source: &'a str,
    interner: &'a DefaultStringInterner,
    file: FileId,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> CheckWalk<'a> {
    /// The derivation context the per-node checks need (source + interner + file).
    /// Built from disjoint field copies (the references are `Copy`), so it does not
    /// borrow `self` — leaving `self.diagnostics` free to borrow mutably alongside.
    fn member_ctx(&self) -> MemberCtx<'a> {
        MemberCtx {
            source: self.source,
            interner: self.interner,
            file: self.file,
        }
    }

    // --- statements ----------------------------------------------------------

    fn visit_statement(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::ExpressionStatement(s) => self.visit_expression(&s.expression),
            Statement::VariableDeclaration(d) => self.visit_variable_declaration(d),
            Statement::FunctionDeclaration(f) => self.check_function_common(
                f.type_parameters.as_ref(),
                f.params,
                f.return_type.as_ref(),
                f.body.body,
            ),
            Statement::TSDeclareFunction(f) => {
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation_opt(f.return_type.as_ref());
            }
            Statement::ClassDeclaration(c) => self.check_class_common(
                c.type_parameters.as_ref(),
                c.decorators,
                c.super_class,
                c.super_type_parameters.as_ref(),
                c.implements,
                &c.body,
            ),
            Statement::TSInterfaceDeclaration(i) => self.check_interface_common(i),
            Statement::TSTypeAliasDeclaration(t) => {
                self.visit_type_params(t.type_parameters.as_ref());
                self.visit_type(&t.type_annotation);
            }
            Statement::TSEnumDeclaration(e) => {
                for member in e.members {
                    if let Some(init) = &member.initializer {
                        self.visit_expression(init);
                    }
                }
            }
            Statement::TSModuleDeclaration(m) => self.visit_module_declaration(m),
            Statement::ReturnStatement(s) => {
                if let Some(a) = &s.argument {
                    self.visit_expression(a);
                }
            }
            Statement::BlockStatement(b) => {
                for s in b.body {
                    self.visit_statement(s);
                }
            }
            Statement::IfStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.consequent);
                if let Some(alt) = s.alternate {
                    self.visit_statement(alt);
                }
            }
            Statement::ForStatement(s) => {
                if let Some(init) = &s.init {
                    match init {
                        ForInit::VariableDeclaration(d) => self.visit_variable_declaration(d),
                        ForInit::Expression(e) => self.visit_expression(e),
                    }
                }
                if let Some(t) = &s.test {
                    self.visit_expression(t);
                }
                if let Some(u) = &s.update {
                    self.visit_expression(u);
                }
                self.visit_statement(s.body);
            }
            Statement::ForInStatement(s) => {
                self.visit_for_left(&s.left);
                self.visit_expression(&s.right);
                self.visit_statement(s.body);
            }
            Statement::ForOfStatement(s) => {
                self.visit_for_left(&s.left);
                self.visit_expression(&s.right);
                self.visit_statement(s.body);
            }
            Statement::WhileStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.body);
            }
            Statement::DoWhileStatement(s) => {
                self.visit_statement(s.body);
                self.visit_expression(&s.test);
            }
            Statement::SwitchStatement(s) => {
                self.visit_expression(&s.discriminant);
                for case in s.cases {
                    if let Some(t) = &case.test {
                        self.visit_expression(t);
                    }
                    for stmt in case.consequent {
                        self.visit_statement(stmt);
                    }
                }
            }
            Statement::TryStatement(s) => {
                for stmt in s.block.body {
                    self.visit_statement(stmt);
                }
                if let Some(h) = &s.handler {
                    if let Some(param) = &h.param {
                        self.visit_param(param);
                    }
                    for stmt in h.body.body {
                        self.visit_statement(stmt);
                    }
                }
                if let Some(f) = &s.finalizer {
                    for stmt in f.body {
                        self.visit_statement(stmt);
                    }
                }
            }
            Statement::ThrowStatement(s) => self.visit_expression(&s.argument),
            Statement::LabeledStatement(s) => self.visit_statement(s.body),
            Statement::ExportNamedDeclaration(e) => {
                if let Some(inner) = e.declaration {
                    self.visit_statement(inner);
                }
            }
            Statement::ExportDefaultDeclaration(e) => self.visit_export_default(&e.declaration),
            Statement::TSExportAssignment(ea) => self.visit_expression(&ea.expression),
            Statement::ExportAllDeclaration(_)
            | Statement::TSNamespaceExportDeclaration(_)
            | Statement::ImportDeclaration(_)
            | Statement::TSImportEqualsDeclaration(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_) => {}
        }
    }

    fn visit_variable_declaration(&mut self, decl: &VariableDeclaration<'_>) {
        for d in decl.declarations {
            self.visit_param(&d.id);
            if let Some(init) = &d.init {
                self.visit_expression(init);
            }
        }
    }

    fn visit_for_left(&mut self, left: &ForInOfLeft<'_>) {
        match left {
            ForInOfLeft::VariableDeclaration(d) => self.visit_variable_declaration(d),
            ForInOfLeft::Pattern(p) => self.visit_expression(p),
        }
    }

    /// Descend a `namespace`/`module` body — a block, or the nested declaration a
    /// dotted `namespace X.Y {}` parses to (recursed without cloning the node).
    fn visit_module_declaration(&mut self, m: &TSModuleDeclaration<'_>) {
        match &m.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                for s in block.body {
                    self.visit_statement(s);
                }
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                self.visit_module_declaration(nested);
            }
            None => {}
        }
    }

    fn visit_export_default(&mut self, value: &ExportDefaultValue<'_>) {
        match value {
            ExportDefaultValue::Expression(e) => self.visit_expression(e),
            ExportDefaultValue::FunctionDeclaration(f) => self.check_function_common(
                f.type_parameters.as_ref(),
                f.params,
                f.return_type.as_ref(),
                f.body.body,
            ),
            ExportDefaultValue::TSDeclareFunction(f) => {
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation_opt(f.return_type.as_ref());
            }
            ExportDefaultValue::ClassDeclaration(c) => self.check_class_common(
                c.type_parameters.as_ref(),
                c.decorators,
                c.super_class,
                c.super_type_parameters.as_ref(),
                c.implements,
                &c.body,
            ),
            ExportDefaultValue::TSInterfaceDeclaration(i) => self.check_interface_common(i),
        }
    }

    // --- shared declaration descents -----------------------------------------

    /// The class descent shared by the declaration, export-default, and expression
    /// forms: type parameters, heritage (decorators / `extends` / `implements`), and
    /// the member body. The three sites are byte-identical across `ClassDeclaration`
    /// and `ClassExpression` (distinct types with the same field shape).
    fn check_class_common(
        &mut self,
        type_parameters: Option<&TSTypeParameterDeclaration<'_>>,
        decorators: Option<&[Decorator<'_>]>,
        super_class: Option<&Expression<'_>>,
        super_type_parameters: Option<&TSTypeParameterInstantiation<'_>>,
        implements: &[TSInterfaceHeritage<'_>],
        body: &ClassBody<'_>,
    ) {
        self.visit_type_params(type_parameters);
        self.visit_class_heritage(decorators, super_class, super_type_parameters, implements);
        self.visit_class_body(body);
    }

    /// The interface descent shared by the declaration and export-default forms.
    fn check_interface_common(&mut self, interface: &TSInterfaceDeclaration<'_>) {
        self.visit_type_params(interface.type_parameters.as_ref());
        self.visit_heritage_type_args(interface.extends);
        self.visit_type_elements(interface.body.body);
    }

    /// The body-bearing function descent shared by the declaration, export-default,
    /// and expression forms: type parameters, parameters, return type, then the body
    /// statements. (The bodyless `TSDeclareFunction` arms share only the header, so
    /// they stay inline.)
    fn check_function_common(
        &mut self,
        type_parameters: Option<&TSTypeParameterDeclaration<'_>>,
        params: &[Expression<'_>],
        return_type: Option<&TSTypeAnnotation<'_>>,
        body: &[Statement<'_>],
    ) {
        self.visit_type_params(type_parameters);
        self.visit_params(params);
        self.visit_type_annotation_opt(return_type);
        for s in body {
            self.visit_statement(s);
        }
    }

    // --- expressions ---------------------------------------------------------

    fn visit_expression(&mut self, expr: &Expression<'_>) {
        use Expression as E;
        match expr {
            E::FunctionExpression(f) => self.check_function_common(
                f.type_parameters.as_ref(),
                f.params,
                f.return_type.as_ref(),
                f.body.body,
            ),
            E::ArrowFunctionExpression(a) => {
                self.visit_type_params(a.type_parameters.as_ref());
                self.visit_params(a.params);
                self.visit_type_annotation_opt(a.return_type.as_ref());
                match &a.body {
                    ArrowFunctionBody::Expression(e) => self.visit_expression(e),
                    ArrowFunctionBody::BlockStatement(b) => {
                        for s in b.body {
                            self.visit_statement(s);
                        }
                    }
                }
            }
            E::ClassExpression(c) => self.check_class_common(
                c.type_parameters.as_ref(),
                c.decorators,
                c.super_class,
                c.super_type_parameters.as_ref(),
                c.implements,
                &c.body,
            ),
            E::TSAsExpression(t) => {
                self.visit_expression(t.expression);
                self.visit_type(t.type_annotation);
            }
            E::TSSatisfiesExpression(t) => {
                self.visit_expression(t.expression);
                self.visit_type(t.type_annotation);
            }
            E::TSTypeAssertion(t) => {
                self.visit_type(t.type_annotation);
                self.visit_expression(t.expression);
            }
            E::TSInstantiationExpression(t) => {
                self.visit_expression(t.expression);
                self.visit_type_args(&t.type_arguments);
            }
            E::TSNonNullExpression(t) => self.visit_expression(t.expression),
            E::ParenthesizedExpression(p) => self.visit_expression(p.expression),
            E::JsdocCast(c) => self.visit_expression(c.inner),
            E::UnaryExpression(u) => self.visit_expression(u.argument),
            E::UpdateExpression(u) => self.visit_expression(u.argument),
            E::AwaitExpression(a) => self.visit_expression(a.argument),
            E::YieldExpression(y) => {
                if let Some(a) = y.argument {
                    self.visit_expression(a);
                }
            }
            E::BinaryExpression(b) => {
                self.visit_expression(b.left);
                self.visit_expression(b.right);
            }
            E::AssignmentExpression(a) => {
                self.visit_expression(a.left);
                self.visit_expression(a.right);
            }
            E::ConditionalExpression(c) => {
                self.visit_expression(c.test);
                self.visit_expression(c.consequent);
                self.visit_expression(c.alternate);
            }
            E::SequenceExpression(s) => {
                for e in s.expressions {
                    self.visit_expression(e);
                }
            }
            E::CallExpression(c) => {
                self.visit_expression(c.callee);
                if let Some(ta) = &c.type_arguments {
                    self.visit_type_args(ta);
                }
                for a in c.arguments {
                    self.visit_expression(a);
                }
            }
            E::NewExpression(n) => {
                self.visit_expression(n.callee);
                if let Some(ta) = &n.type_arguments {
                    self.visit_type_args(ta);
                }
                for a in n.arguments {
                    self.visit_expression(a);
                }
            }
            E::MemberExpression(m) => {
                self.visit_expression(m.object);
                self.visit_expression(m.property);
            }
            E::SpreadElement(s) => self.visit_expression(s.argument),
            E::ArrayExpression(a) => {
                for e in a.elements.iter().flatten() {
                    self.visit_expression(e);
                }
            }
            E::ObjectExpression(o) => {
                for prop in o.properties {
                    match prop {
                        ObjectProperty::Property(pr) => self.visit_expression(&pr.value),
                        ObjectProperty::SpreadElement(s) => self.visit_expression(s.argument),
                    }
                }
            }
            E::TemplateLiteral(t) => {
                for e in t.expressions {
                    self.visit_expression(e);
                }
            }
            E::TaggedTemplateExpression(t) => {
                self.visit_expression(t.tag);
                if let Some(ta) = &t.type_arguments {
                    self.visit_type_args(ta);
                }
                for e in t.quasi.expressions {
                    self.visit_expression(e);
                }
            }
            E::ImportExpression(i) => {
                self.visit_expression(i.source);
                if let Some(o) = i.options {
                    self.visit_expression(o);
                }
            }
            _ => {}
        }
    }

    // --- classes / interfaces / type literals --------------------------------

    /// Descend a decorator list — each decorator's expression can host a type
    /// literal (`@dec({} as {x; x})`). tsgo's `checkSourceElement` checks decorators.
    fn visit_decorators(&mut self, decorators: Option<&[Decorator<'_>]>) {
        if let Some(decs) = decorators {
            for d in decs {
                self.visit_expression(&d.expression);
            }
        }
    }

    /// Descend a class's decorators + heritage: the `extends` expression and its
    /// type arguments, and each `implements` clause's type arguments — every site a
    /// type literal can hide (`extends Base<{x; x}>`). tsgo's `checkSourceElement`
    /// descends the heritage type arguments and decorators.
    fn visit_class_heritage(
        &mut self,
        decorators: Option<&[Decorator<'_>]>,
        super_class: Option<&Expression<'_>>,
        super_type_parameters: Option<&TSTypeParameterInstantiation<'_>>,
        implements: &[TSInterfaceHeritage<'_>],
    ) {
        self.visit_decorators(decorators);
        if let Some(sc) = super_class {
            self.visit_expression(sc);
        }
        if let Some(tp) = super_type_parameters {
            self.visit_type_args(tp);
        }
        self.visit_heritage_type_args(implements);
    }

    /// Descend each heritage clause's type arguments (shared by a class's
    /// `implements` and an interface's `extends` — both are `TSInterfaceHeritage`).
    fn visit_heritage_type_args(&mut self, heritages: &[TSInterfaceHeritage<'_>]) {
        for h in heritages {
            if let Some(ta) = &h.type_arguments {
                self.visit_type_args(ta);
            }
        }
    }

    fn visit_class_body(&mut self, body: &ClassBody<'_>) {
        let ctx = self.member_ctx();
        duplicate_members::check_class_members(&ctx, body.body, &mut self.diagnostics);
        for member in body.body {
            match member {
                ClassMember::MethodDefinition(m) => {
                    self.visit_decorators(m.decorators);
                    self.visit_type_params(m.value.type_parameters.as_ref());
                    self.visit_params(m.value.params);
                    self.visit_type_annotation_opt(m.value.return_type.as_ref());
                    for s in m.value.body.body {
                        self.visit_statement(s);
                    }
                }
                ClassMember::PropertyDefinition(p) => {
                    self.visit_decorators(p.decorators);
                    self.visit_type_annotation_opt(p.type_annotation.as_ref());
                    if let Some(v) = &p.value {
                        self.visit_expression(v);
                    }
                }
                ClassMember::StaticBlock(s) => {
                    for stmt in s.body {
                        self.visit_statement(stmt);
                    }
                }
                ClassMember::IndexSignature(i) => {
                    self.visit_type_annotation_opt(i.type_annotation.as_ref());
                }
            }
        }
    }

    fn visit_type_elements(&mut self, members: &[TSTypeElement<'_>]) {
        let ctx = self.member_ctx();
        duplicate_members::check_type_elements(&ctx, members, &mut self.diagnostics);
        for member in members {
            match member {
                TSTypeElement::PropertySignature(p) => {
                    self.visit_type_annotation_opt(p.type_annotation.as_ref());
                }
                TSTypeElement::MethodSignature(m) => {
                    self.visit_type_params(m.type_parameters.as_ref());
                    self.visit_params(m.params);
                    self.visit_type_annotation_opt(m.return_type.as_ref());
                }
                TSTypeElement::CallSignature(c) => {
                    self.visit_type_params(c.type_parameters.as_ref());
                    self.visit_params(c.params);
                    self.visit_type_annotation_opt(c.return_type.as_ref());
                }
                TSTypeElement::ConstructSignature(c) => {
                    self.visit_type_params(c.type_parameters.as_ref());
                    self.visit_params(c.params);
                    self.visit_type_annotation_opt(c.return_type.as_ref());
                }
                TSTypeElement::IndexSignature(i) => {
                    self.visit_type_annotation_opt(i.type_annotation.as_ref());
                }
            }
        }
    }

    // --- parameters ----------------------------------------------------------

    fn visit_params(&mut self, params: &[Expression<'_>]) {
        for param in params {
            self.visit_param(param);
        }
    }

    fn visit_param(&mut self, param: &Expression<'_>) {
        match param {
            Expression::Identifier(id) => {
                self.visit_decorators(id.decorators());
                if let Some(ann) = id.type_annotation() {
                    self.visit_type_annotation(ann);
                }
            }
            Expression::ObjectPattern(op) => {
                self.visit_decorators(op.decorators);
                if let Some(ann) = &op.type_annotation {
                    self.visit_type_annotation(ann);
                }
                for prop in op.properties {
                    match prop {
                        ObjectPatternProperty::Property(pr) => self.visit_param(&pr.value),
                        ObjectPatternProperty::RestElement(r) => self.visit_param(r.argument),
                    }
                }
            }
            Expression::ArrayPattern(ap) => {
                self.visit_decorators(ap.decorators);
                if let Some(ann) = &ap.type_annotation {
                    self.visit_type_annotation(ann);
                }
                for el in ap.elements.iter().flatten() {
                    self.visit_param(el);
                }
            }
            Expression::AssignmentPattern(a) => {
                self.visit_decorators(a.decorators);
                self.visit_param(a.left);
                self.visit_expression(a.right);
            }
            Expression::RestElement(r) => {
                if let Some(ann) = &r.type_annotation {
                    self.visit_type_annotation(ann);
                }
                self.visit_param(r.argument);
            }
            Expression::TSParameterProperty(pp) => self.visit_param(pp.parameter),
            _ => {}
        }
    }

    // --- types (general recursion) -------------------------------------------

    fn visit_type_annotation_opt(&mut self, ann: Option<&TSTypeAnnotation<'_>>) {
        if let Some(a) = ann {
            self.visit_type_annotation(a);
        }
    }

    fn visit_type_annotation(&mut self, ann: &TSTypeAnnotation<'_>) {
        self.visit_type(ann.type_annotation);
    }

    fn visit_type_args(&mut self, args: &TSTypeParameterInstantiation<'_>) {
        for t in args.params {
            self.visit_type(t);
        }
    }

    /// The per-type-parameter-declaration hook: the duplicate-name identity check
    /// (`check_type_parameters`) plus the descent into each parameter's constraint
    /// and default (both are types).
    fn visit_type_params(&mut self, params: Option<&TSTypeParameterDeclaration<'_>>) {
        if let Some(decl) = params {
            let ctx = self.member_ctx();
            duplicate_members::check_type_parameters(&ctx, decl, &mut self.diagnostics);
            for p in decl.params {
                if let Some(c) = p.constraint {
                    self.visit_type(c);
                }
                if let Some(d) = p.default {
                    self.visit_type(d);
                }
            }
        }
    }

    fn visit_type(&mut self, ty: &TSType<'_>) {
        match ty {
            TSType::TypeLiteral(tl) => self.visit_type_elements(tl.members),
            TSType::Array(a) => self.visit_type(a.element_type),
            TSType::Union(u) => {
                for t in u.types {
                    self.visit_type(t);
                }
            }
            TSType::Intersection(i) => {
                for t in i.types {
                    self.visit_type(t);
                }
            }
            TSType::Parenthesized(p) => self.visit_type(p.type_annotation),
            TSType::Function(f) => {
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation(&f.return_type);
            }
            TSType::Constructor(c) => {
                self.visit_type_params(c.type_parameters.as_ref());
                self.visit_params(c.params);
                self.visit_type_annotation(&c.return_type);
            }
            TSType::Tuple(t) => {
                for e in t.element_types {
                    self.visit_type(e);
                }
            }
            TSType::TypePredicate(p) => {
                if let Some(t) = p.type_annotation {
                    self.visit_type(t);
                }
            }
            TSType::Conditional(c) => {
                self.visit_type(c.check_type);
                self.visit_type(c.extends_type);
                self.visit_type(c.true_type);
                self.visit_type(c.false_type);
            }
            TSType::Mapped(m) => {
                self.visit_type(m.type_parameter.constraint);
                if let Some(nt) = m.name_type {
                    self.visit_type(nt);
                }
                if let Some(ta) = m.type_annotation {
                    self.visit_type(ta);
                }
            }
            TSType::TypeOperator(o) => self.visit_type(o.type_annotation),
            TSType::IndexedAccess(i) => {
                self.visit_type(i.object_type);
                self.visit_type(i.index_type);
            }
            TSType::Rest(r) => self.visit_type(r.type_annotation),
            TSType::Optional(o) => self.visit_type(o.type_annotation),
            TSType::NamedTupleMember(n) => self.visit_type(n.element_type),
            TSType::Infer(inf) => {
                if let Some(c) = inf.type_parameter.constraint {
                    self.visit_type(c);
                }
                if let Some(d) = inf.type_parameter.default {
                    self.visit_type(d);
                }
            }
            TSType::TypeReference(r) => {
                if let Some(args) = &r.type_arguments {
                    self.visit_type_args(args);
                }
            }
            TSType::TypeQuery(q) => {
                if let Some(args) = &q.type_arguments {
                    self.visit_type_args(args);
                }
            }
            TSType::Import(i) => {
                if let Some(args) = &i.type_arguments {
                    self.visit_type_args(args);
                }
            }
            TSType::Literal(lit) => {
                // A template-literal type's interpolations are types, which can be
                // type literals (`` `p-${ {x; x} }` ``); every other literal type is a
                // leaf.
                if let TSLiteralType::TemplateLiteral(t) = lit {
                    for ty in t.types {
                        self.visit_type(ty);
                    }
                }
            }
            TSType::Keyword(_) | TSType::ThisType(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    /// Run the check pass in isolation over `source`, returning the raw (unsorted,
    /// un-deduped) diagnostic codes — the check pass alone, no binder, no
    /// program-wide sort/dedup.
    fn check_codes(source: &str) -> Vec<u32> {
        let arena = Bump::new();
        let program = tsv_ts::parse(source, &arena).expect("parse");
        check_file_members(&program, source, FileId::ROOT)
            .iter()
            .map(|d| d.code)
            .collect()
    }

    #[test]
    fn type_parameters_duplicate_fires_at_later_param() {
        // The identity check emits one TS2300 (at the second occurrence); no binder.
        assert_eq!(check_codes("function f<T, T>() {}"), vec![2300]);
        assert_eq!(check_codes("class C<T, U, T> {}"), vec![2300]);
        assert_eq!(check_codes("interface I<A, A> {}"), vec![2300]);
        // Distinct names never fire.
        assert!(check_codes("function g<T, U>() {}").is_empty());
    }

    #[test]
    fn type_parameters_three_way_pushes_raw_before_dedup() {
        // The raw per-(i, j) push: T₂ fires once (j=0), T₃ fires twice (j=0, j=1) —
        // three diagnostics at two distinct spans. The program-wide sort/dedup (see
        // the binder-side `diag_codes` test) later collapses the T₃ pair to one.
        let arena = Bump::new();
        let src = "function f<T, T, T>() {}";
        let program = tsv_ts::parse(src, &arena).expect("parse");
        let diags = check_file_members(&program, src, FileId::ROOT);
        assert_eq!(
            diags.iter().map(|d| d.code).collect::<Vec<_>>(),
            vec![2300, 2300, 2300]
        );
        let distinct: std::collections::BTreeSet<u32> =
            diags.iter().map(|d| d.span.start).collect();
        assert_eq!(distinct.len(), 2, "two distinct spans (T2 once, T3 twice)");
    }

    #[test]
    fn accessor_accessor_is_check_pass_noop() {
        // The state machine leaves a same-named accessor/accessor pair to the binder
        // (the coarse kind can't tell get from set), so the check pass emits nothing.
        assert!(check_codes("class C { get x() {} get x() {} }").is_empty());
        assert!(check_codes("class C { get x() {} set x(v) {} }").is_empty());
    }

    #[test]
    fn static_and_instance_members_are_separate_buckets() {
        // `static x` and instance `x` live in different (key, is_static) buckets, so
        // the check pass never merges them into a duplicate.
        assert!(check_codes("class C { static x = 1; x = 2; }").is_empty());
        // A same-static-ness duplicate DOES fire — the separation is by
        // (key, is_static), not by ignoring static.
        assert_eq!(
            check_codes("class C { static x = 1; static x = 2; }"),
            vec![2300, 2300]
        );
    }

    #[test]
    fn heritage_and_decorator_and_template_type_literals_are_checked() {
        // The walk descends class/interface heritage type arguments, decorators, and
        // template-literal-type interpolations — every syntactic position a type
        // literal can hide — so a duplicate member there is caught (two raw TS2300).
        assert_eq!(
            check_codes("class C extends Base<{x:number;x:string}> {}"),
            vec![2300, 2300]
        );
        assert_eq!(
            check_codes("interface I extends Base<{x;x}> {}"),
            vec![2300, 2300]
        );
        assert_eq!(
            check_codes("class C implements Base<{x;x}> {}"),
            vec![2300, 2300]
        );
        assert_eq!(
            check_codes("@dec({} as {x;x}) class C {}"),
            vec![2300, 2300]
        );
        assert_eq!(
            check_codes("class C { @dec({} as {x;x}) m() {} }"),
            vec![2300, 2300]
        );
        assert_eq!(
            check_codes("class C { @dec({} as {x;x}) p = 1; }"),
            vec![2300, 2300]
        );
        // A constructor parameter's own decorator.
        assert_eq!(
            check_codes("class C { m(@dec({} as {x;x}) p) {} }"),
            vec![2300, 2300]
        );
        assert_eq!(check_codes("type T = `p-${ {x;x} }`;"), vec![2300, 2300]);
    }

    #[test]
    fn computed_literal_key_display_is_raw_bracket_source() {
        // A computed key's message arg is the raw `[ … ]` source (tsgo's
        // `symbolToString`), not the decoded value — while its grouping key stays the
        // decoded value. Interface computed keys fire property/property in the check
        // pass, so this exercises the check-side display in isolation.
        let arena = Bump::new();
        let src = "interface I { ['a']: number; ['a']: string; }";
        let program = tsv_ts::parse(src, &arena).expect("parse");
        let diags = check_file_members(&program, src, FileId::ROOT);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.code == 2300));
        assert!(diags.iter().all(|d| d.args == vec!["['a']".to_string()]));
    }

    #[test]
    fn constructor_parameter_property_participates_in_batch() {
        // A constructor parameter property (`public x`) is an instance property that
        // matches the batch on key alone; paired with a field `x` the check pass
        // fires at both (2 raw diagnostics — property/property silent-merges at bind).
        assert_eq!(
            check_codes("class C { constructor(public x: number) {} x = 1; }"),
            vec![2300, 2300]
        );
    }
}
