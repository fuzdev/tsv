//! The syntactic check pass — a standalone walk over `&Program` that emits the
//! check-time diagnostics the binder's symbol cascade cannot (they are not
//! same-table flag conflicts).
//!
//! It is deliberately **not** the binder: it never consults the symbol tables
//! (walking the shared interface member table would break declaration-merging).
//! It descends every syntactic position — class / interface / type-literal bodies,
//! and every type-annotation site (variable / parameter / return-type / predicate /
//! function-type / union / intersection / assertion target / …) — and runs a set
//! of per-node checks. Today that is the duplicate-member check
//! ([`duplicate_members`]); the traversal is general so a second per-node check
//! (type-parameter identity) hooks into the same walk without a second descent.
//!
//! The output folds into each file's diagnostics in [`crate::program`], alongside
//! the bind product, then the whole program is canonically sorted + deduped — so a
//! diagnostic this pass and the binder both emit (identical span/code/args)
//! collapses to one, exactly as tsgo's binder + checker outputs do.
//
// tsgo: internal/checker/checker.go checkSourceElement dispatch (the per-node
//       checks this walk ports piecemeal)

mod duplicate_members;

use crate::diag::Diagnostic;
use crate::ids::FileId;
use duplicate_members::MemberCtx;
use string_interner::DefaultStringInterner;
use tsv_ts::ast::Program;
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ClassBody, ClassMember, ExportDefaultValue, Expression, ForInOfLeft,
    ForInit, ObjectPatternProperty, ObjectProperty, Statement, TSModuleDeclaration,
    TSModuleDeclarationBody, TSType, TSTypeAnnotation, TSTypeElement, TSTypeParameterDeclaration,
    TSTypeParameterInstantiation, VariableDeclaration,
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
            Statement::FunctionDeclaration(f) => {
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation_opt(f.return_type.as_ref());
                for s in f.body.body {
                    self.visit_statement(s);
                }
            }
            Statement::TSDeclareFunction(f) => {
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation_opt(f.return_type.as_ref());
            }
            Statement::ClassDeclaration(c) => {
                self.visit_type_params(c.type_parameters.as_ref());
                self.visit_class_body(&c.body);
            }
            Statement::TSInterfaceDeclaration(i) => {
                self.visit_type_params(i.type_parameters.as_ref());
                self.visit_type_elements(i.body.body);
            }
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
            ExportDefaultValue::FunctionDeclaration(f) => {
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation_opt(f.return_type.as_ref());
                for s in f.body.body {
                    self.visit_statement(s);
                }
            }
            ExportDefaultValue::TSDeclareFunction(f) => {
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation_opt(f.return_type.as_ref());
            }
            ExportDefaultValue::ClassDeclaration(c) => {
                self.visit_type_params(c.type_parameters.as_ref());
                self.visit_class_body(&c.body);
            }
            ExportDefaultValue::TSInterfaceDeclaration(i) => {
                self.visit_type_params(i.type_parameters.as_ref());
                self.visit_type_elements(i.body.body);
            }
        }
    }

    // --- expressions ---------------------------------------------------------

    fn visit_expression(&mut self, expr: &Expression<'_>) {
        use Expression as E;
        match expr {
            E::FunctionExpression(f) => {
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation_opt(f.return_type.as_ref());
                for s in f.body.body {
                    self.visit_statement(s);
                }
            }
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
            E::ClassExpression(c) => {
                self.visit_type_params(c.type_parameters.as_ref());
                self.visit_class_body(&c.body);
            }
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

    fn visit_class_body(&mut self, body: &ClassBody<'_>) {
        let ctx = self.member_ctx();
        duplicate_members::check_class_members(&ctx, body.body, &mut self.diagnostics);
        for member in body.body {
            match member {
                ClassMember::MethodDefinition(m) => {
                    self.visit_type_params(m.value.type_parameters.as_ref());
                    self.visit_params(m.value.params);
                    self.visit_type_annotation_opt(m.value.return_type.as_ref());
                    for s in m.value.body.body {
                        self.visit_statement(s);
                    }
                }
                ClassMember::PropertyDefinition(p) => {
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
                if let Some(ann) = id.type_annotation() {
                    self.visit_type_annotation(ann);
                }
            }
            Expression::ObjectPattern(op) => {
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
                if let Some(ann) = &ap.type_annotation {
                    self.visit_type_annotation(ann);
                }
                for el in ap.elements.iter().flatten() {
                    self.visit_param(el);
                }
            }
            Expression::AssignmentPattern(a) => {
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

    /// The per-type-parameter-declaration hook (constraints + defaults are types).
    /// Slice 3's type-parameter-identity check attaches here alongside the descent.
    fn visit_type_params(&mut self, params: Option<&TSTypeParameterDeclaration<'_>>) {
        if let Some(decl) = params {
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
            TSType::Keyword(_) | TSType::Literal(_) | TSType::ThisType(_) => {}
        }
    }
}
