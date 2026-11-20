//! The syntactic check pass — a standalone walk over a parsed file that emits the
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
use tsv_ts::ast::{
    ArrowFunctionBody, AstView, ClassBody, ClassMember, Decorator, ExportDefaultValue, ExprId,
    Expression, ExpressionKind, ForInOfLeft, ForInit, ObjectPatternProperty, ObjectProperty,
    ParsedRef, Run, Statement, StatementKind, StmtId, TSInterfaceDeclaration, TSInterfaceHeritage,
    TSLiteralType, TSModuleDeclaration, TSModuleDeclarationBody, TSType, TSTypeAnnotation,
    TSTypeElement, TSTypeParameterDeclaration, TSTypeParameterInstantiation, TyId,
    VariableDeclaration,
};

/// Run the syntactic check pass over one parsed file, returning its check-time
/// diagnostics (unsorted — the program-wide sort/dedup canonicalizes order).
#[must_use]
pub fn check_file_members(parsed: ParsedRef<'_>, source: &str, file: FileId) -> Vec<Diagnostic> {
    let ParsedRef { program, view } = parsed;
    let mut walk = CheckWalk {
        view,
        source,
        file,
        diagnostics: Vec::new(),
    };
    walk.visit_statements(program.body);
    walk.diagnostics
}

/// The check walk's per-file state, and the view it reads the tree through.
struct CheckWalk<'a> {
    view: AstView<'a>,
    source: &'a str,
    file: FileId,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> CheckWalk<'a> {
    /// The derivation context the per-node checks need (view + source + file).
    /// Built from disjoint field copies (all `Copy`), so it does not borrow `self` —
    /// leaving `self.diagnostics` free to borrow mutably alongside.
    fn member_ctx(&self) -> MemberCtx<'a> {
        MemberCtx {
            view: self.view,
            source: self.source,
            file: self.file,
        }
    }

    // --- statements ----------------------------------------------------------

    fn visit_statements(&mut self, stmts: Run<StmtId>) {
        for &stmt in self.view.stmt_list(stmts) {
            self.visit_statement(self.view.stmt(stmt));
        }
    }

    fn visit_stmt(&mut self, id: StmtId) {
        self.visit_statement(self.view.stmt(id));
    }

    fn visit_statement(&mut self, stmt: &'a Statement) {
        let view = self.view;
        match &stmt.kind {
            StatementKind::ExpressionStatement(s) => self.visit_expr(s.expression),
            StatementKind::VariableDeclaration(d) => self.visit_variable_declaration(d),
            &StatementKind::FunctionDeclaration(f) => {
                let f = view.fn_decl(f);
                self.check_function_common(
                    f.type_parameters.as_ref(),
                    f.params,
                    f.return_type.as_ref(),
                    f.body.body,
                );
            }
            &StatementKind::TSDeclareFunction(f) => {
                let f = view.declare_fn(f);
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation_opt(f.return_type.as_ref());
            }
            &StatementKind::ClassDeclaration(c) => {
                let c = view.class_decl(c);
                self.check_class_common(
                    c.type_parameters.as_ref(),
                    c.decorators,
                    c.super_class,
                    c.super_type_parameters.as_ref(),
                    c.implements,
                    &c.body,
                );
            }
            &StatementKind::TSInterfaceDeclaration(i) => {
                self.check_interface_common(view.interface_decl(i));
            }
            &StatementKind::TSTypeAliasDeclaration(t) => {
                let t = view.type_alias(t);
                self.visit_type_params(t.type_parameters.as_ref());
                self.visit_ty(t.type_annotation);
            }
            &StatementKind::TSEnumDeclaration(e) => {
                for member in view.enum_members(view.enum_decl(e).members) {
                    if let Some(init) = member.initializer {
                        self.visit_expr(init);
                    }
                }
            }
            &StatementKind::TSModuleDeclaration(m) => {
                self.visit_module_declaration(view.module_decl(m));
            }
            StatementKind::ReturnStatement(s) => {
                if let Some(a) = s.argument {
                    self.visit_expr(a);
                }
            }
            StatementKind::BlockStatement(b) => self.visit_statements(b.body),
            StatementKind::IfStatement(s) => {
                self.visit_expr(s.test);
                self.visit_stmt(s.consequent);
                if let Some(alt) = view.opt_stmt(s.alternate) {
                    self.visit_statement(alt);
                }
            }
            StatementKind::ForStatement(s) => {
                match view.opt_for_head(s.init) {
                    Some(ForInit::VariableDeclaration(d)) => self.visit_variable_declaration(d),
                    Some(&ForInit::Expression(e)) => self.visit_expr(e),
                    None => {}
                }
                if let Some(t) = s.test {
                    self.visit_expr(t);
                }
                if let Some(u) = s.update {
                    self.visit_expr(u);
                }
                self.visit_stmt(s.body);
            }
            StatementKind::ForInStatement(s) => {
                self.visit_for_left(view.for_left(s.left));
                self.visit_expr(s.right);
                self.visit_stmt(s.body);
            }
            StatementKind::ForOfStatement(s) => {
                self.visit_for_left(view.for_left(s.left));
                self.visit_expr(s.right);
                self.visit_stmt(s.body);
            }
            StatementKind::WhileStatement(s) => {
                self.visit_expr(s.test);
                self.visit_stmt(s.body);
            }
            StatementKind::DoWhileStatement(s) => {
                self.visit_stmt(s.body);
                self.visit_expr(s.test);
            }
            StatementKind::WithStatement(s) => {
                self.visit_expr(s.object);
                self.visit_stmt(s.body);
            }
            StatementKind::SwitchStatement(s) => {
                self.visit_expr(s.discriminant);
                for case in view.cases(s.cases) {
                    if let Some(t) = case.test {
                        self.visit_expr(t);
                    }
                    self.visit_statements(case.consequent);
                }
            }
            &StatementKind::TryStatement(s) => {
                let s = view.try_stmt(s);
                self.visit_statements(s.block.body);
                if let Some(h) = view.opt_catch(s.handler) {
                    if let Some(param) = &h.param {
                        self.visit_param(param);
                    }
                    self.visit_statements(h.body.body);
                }
                if let Some(f) = &s.finalizer {
                    self.visit_statements(f.body);
                }
            }
            StatementKind::ThrowStatement(s) => self.visit_expr(s.argument),
            &StatementKind::LabeledStatement(s) => self.visit_stmt(view.labeled_stmt(s).body),
            &StatementKind::ExportNamedDeclaration(e) => {
                if let Some(inner) = view.export_named(e).declaration {
                    self.visit_stmt(inner);
                }
            }
            &StatementKind::ExportDefaultDeclaration(e) => {
                self.visit_export_default(&view.export_default(e).declaration);
            }
            &StatementKind::TSExportAssignment(ea) => {
                self.visit_expr(view.export_assign(ea).expression);
            }
            StatementKind::ExportAllDeclaration(_)
            | StatementKind::TSNamespaceExportDeclaration(_)
            | StatementKind::ImportDeclaration(_)
            | StatementKind::TSImportEqualsDeclaration(_)
            | StatementKind::BreakStatement(_)
            | StatementKind::ContinueStatement(_)
            | StatementKind::EmptyStatement(_)
            | StatementKind::DebuggerStatement(_) => {}
        }
    }

    fn visit_variable_declaration(&mut self, decl: &'a VariableDeclaration) {
        for d in self.view.declarators(decl.declarations) {
            self.visit_param(self.view.expr(d.id));
            if let Some(init) = d.init {
                self.visit_expr(init);
            }
        }
    }

    fn visit_for_left(&mut self, left: &'a ForInOfLeft) {
        match left {
            ForInOfLeft::VariableDeclaration(d) => self.visit_variable_declaration(d),
            &ForInOfLeft::Pattern(p) => self.visit_expr(p),
        }
    }

    /// Descend a `namespace`/`module` body — a block, or the nested declaration a
    /// dotted `namespace X.Y {}` parses to (read in place from its store).
    fn visit_module_declaration(&mut self, m: &'a TSModuleDeclaration) {
        match &m.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                self.visit_statements(block.body);
            }
            &Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                self.visit_module_declaration(self.view.module_decl(nested));
            }
            None => {}
        }
    }

    fn visit_export_default(&mut self, value: &'a ExportDefaultValue) {
        match value {
            &ExportDefaultValue::Expression(e) => self.visit_expr(e),
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
        type_parameters: Option<&'a TSTypeParameterDeclaration>,
        decorators: Option<Run<Decorator>>,
        super_class: Option<ExprId>,
        super_type_parameters: Option<&'a TSTypeParameterInstantiation>,
        implements: Run<TSInterfaceHeritage>,
        body: &'a ClassBody,
    ) {
        self.visit_type_params(type_parameters);
        self.visit_class_heritage(decorators, super_class, super_type_parameters, implements);
        self.visit_class_body(body);
    }

    /// The interface descent shared by the declaration and export-default forms.
    fn check_interface_common(&mut self, interface: &'a TSInterfaceDeclaration) {
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
        type_parameters: Option<&'a TSTypeParameterDeclaration>,
        params: Run<Expression>,
        return_type: Option<&'a TSTypeAnnotation>,
        body: Run<StmtId>,
    ) {
        self.visit_type_params(type_parameters);
        self.visit_params(params);
        self.visit_type_annotation_opt(return_type);
        self.visit_statements(body);
    }

    // --- expressions ---------------------------------------------------------

    /// Visit the expression `id` names, read in place from its store.
    fn visit_expr(&mut self, id: ExprId) {
        self.visit_expression(self.view.expr(id));
    }

    fn visit_expr_list(&mut self, list: Run<ExprId>) {
        for &e in self.view.expr_list(list) {
            self.visit_expr(e);
        }
    }

    fn visit_expression(&mut self, expr: &'a Expression) {
        use ExpressionKind as E;
        let view = self.view;
        match &expr.kind {
            &E::FunctionExpression(f) => {
                let f = view.fn_expr(f);
                self.check_function_common(
                    f.type_parameters.as_ref(),
                    f.params,
                    f.return_type.as_ref(),
                    f.body.body,
                );
            }
            &E::ArrowFunctionExpression(a) => {
                let a = view.arrow_fn(a);
                self.visit_type_params(a.type_parameters.as_ref());
                self.visit_params(a.params);
                self.visit_type_annotation_opt(a.return_type.as_ref());
                match &a.body {
                    &ArrowFunctionBody::Expression(e) => self.visit_expr(e),
                    ArrowFunctionBody::BlockStatement(b) => self.visit_statements(b.body),
                }
            }
            &E::ClassExpression(c) => {
                let c = view.class_expr(c);
                self.check_class_common(
                    c.type_parameters.as_ref(),
                    c.decorators,
                    c.super_class,
                    c.super_type_parameters.as_ref(),
                    c.implements,
                    &c.body,
                );
            }
            E::TSAsExpression(t) => {
                self.visit_expr(t.expression);
                self.visit_ty(t.type_annotation);
            }
            E::TSSatisfiesExpression(t) => {
                self.visit_expr(t.expression);
                self.visit_ty(t.type_annotation);
            }
            E::TSTypeAssertion(t) => {
                self.visit_ty(t.type_annotation);
                self.visit_expr(t.expression);
            }
            E::TSInstantiationExpression(t) => {
                self.visit_expr(t.expression);
                self.visit_type_args(&t.type_arguments);
            }
            E::TSNonNullExpression(t) => self.visit_expr(t.expression),
            E::ParenthesizedExpression(p) => self.visit_expr(p.expression),
            &E::CommentBoundPair(c) => self.visit_expr(view.comment_pair(c).inner),
            E::UnaryExpression(u) => self.visit_expr(u.argument),
            E::UpdateExpression(u) => self.visit_expr(u.argument),
            E::AwaitExpression(a) => self.visit_expr(a.argument),
            E::YieldExpression(y) => {
                if let Some(a) = y.argument {
                    self.visit_expr(a);
                }
            }
            E::BinaryExpression(b) => {
                self.visit_expr(b.left);
                self.visit_expr(b.right);
            }
            E::AssignmentExpression(a) => {
                self.visit_expr(a.left);
                self.visit_expr(a.right);
            }
            E::ConditionalExpression(c) => {
                self.visit_expr(c.test);
                self.visit_expr(c.consequent);
                self.visit_expr(c.alternate);
            }
            E::SequenceExpression(s) => self.visit_expr_list(s.expressions),
            E::CallExpression(c) => {
                self.visit_expr(c.callee);
                if let Some(ta) = c.type_args {
                    self.visit_type_args(view.type_args(ta));
                }
                self.visit_expr_list(c.arguments);
            }
            E::NewExpression(n) => {
                self.visit_expr(n.callee);
                if let Some(ta) = n.type_args {
                    self.visit_type_args(view.type_args(ta));
                }
                self.visit_expr_list(n.arguments);
            }
            E::MemberExpression(m) => {
                self.visit_expr(m.object);
                self.visit_expr(m.property);
            }
            E::SpreadElement(s) => self.visit_expr(s.argument),
            E::ArrayExpression(a) => {
                for &e in view.expr_opt_list(a.elements).iter().flatten() {
                    self.visit_expr(e);
                }
            }
            E::ObjectExpression(o) => {
                for prop in view.obj_props(o.properties) {
                    match prop {
                        ObjectProperty::Property(pr) => self.visit_expr(pr.value),
                        ObjectProperty::SpreadElement(s) => self.visit_expr(s.argument),
                    }
                }
            }
            E::TemplateLiteral(t) => self.visit_expr_list(t.expressions),
            &E::TaggedTemplateExpression(t) => {
                let t = view.tagged_tpl(t);
                self.visit_expr(t.tag);
                if let Some(ta) = &t.type_arguments {
                    self.visit_type_args(ta);
                }
                self.visit_expr_list(t.quasi.expressions);
            }
            E::ImportExpression(i) => {
                self.visit_expr(i.source);
                if let Some(o) = i.options {
                    self.visit_expr(o);
                }
            }
            _ => {}
        }
    }

    // --- classes / interfaces / type literals --------------------------------

    /// Descend a decorator list — each decorator's expression can host a type
    /// literal (`@dec({} as {x; x})`). tsgo's `checkSourceElement` checks decorators.
    fn visit_decorators(&mut self, decorators: Option<&'a [Decorator]>) {
        if let Some(decs) = decorators {
            for d in decs {
                self.visit_expr(d.expression);
            }
        }
    }

    /// Descend a class's decorators + heritage: the `extends` expression and its
    /// type arguments, and each `implements` clause's type arguments — every site a
    /// type literal can hide (`extends Base<{x; x}>`). tsgo's `checkSourceElement`
    /// descends the heritage type arguments and decorators.
    fn visit_class_heritage(
        &mut self,
        decorators: Option<Run<Decorator>>,
        super_class: Option<ExprId>,
        super_type_parameters: Option<&'a TSTypeParameterInstantiation>,
        implements: Run<TSInterfaceHeritage>,
    ) {
        self.visit_decorators(self.view.opt_decorators(decorators));
        if let Some(sc) = super_class {
            self.visit_expr(sc);
        }
        if let Some(tp) = super_type_parameters {
            self.visit_type_args(tp);
        }
        self.visit_heritage_type_args(implements);
    }

    /// Descend each heritage clause's type arguments (shared by a class's
    /// `implements` and an interface's `extends` — both are `TSInterfaceHeritage`).
    fn visit_heritage_type_args(&mut self, heritages: Run<TSInterfaceHeritage>) {
        for h in self.view.heritages(heritages) {
            if let Some(ta) = &h.type_arguments {
                self.visit_type_args(ta);
            }
        }
    }

    fn visit_class_body(&mut self, body: &'a ClassBody) {
        let view = self.view;
        let members = view.class_members(body.body);
        let ctx = self.member_ctx();
        duplicate_members::check_class_members(&ctx, members, &mut self.diagnostics);
        for member in members {
            match member {
                ClassMember::MethodDefinition(m) => {
                    self.visit_decorators(view.opt_decorators(m.decorators));
                    self.visit_type_params(m.value.type_parameters.as_ref());
                    self.visit_params(m.value.params);
                    self.visit_type_annotation_opt(m.value.return_type.as_ref());
                    self.visit_statements(m.value.body.body);
                }
                ClassMember::PropertyDefinition(p) => {
                    self.visit_decorators(view.opt_decorators(p.decorators));
                    self.visit_type_annotation_opt(p.type_annotation.as_ref());
                    if let Some(v) = p.value {
                        self.visit_expr(v);
                    }
                }
                ClassMember::StaticBlock(s) => self.visit_statements(s.body),
                ClassMember::IndexSignature(i) => {
                    self.visit_type_annotation_opt(i.type_annotation.as_ref());
                }
            }
        }
    }

    fn visit_type_elements(&mut self, members: Run<TSTypeElement>) {
        let members = self.view.type_members(members);
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

    fn visit_params(&mut self, params: Run<Expression>) {
        for param in self.view.exprs(params) {
            self.visit_param(param);
        }
    }

    fn visit_param_id(&mut self, id: ExprId) {
        self.visit_param(self.view.expr(id));
    }

    fn visit_param(&mut self, param: &'a Expression) {
        let view = self.view;
        match &param.kind {
            ExpressionKind::Identifier(id) => {
                self.visit_decorators(id.decorators(view));
                if let Some(ann) = id.type_annotation(view) {
                    self.visit_type_annotation(ann);
                }
            }
            ExpressionKind::ObjectPattern(op) => {
                self.visit_decorators(op.decorators(view));
                if let Some(ann) = op.type_annotation(view) {
                    self.visit_type_annotation(ann);
                }
                for prop in view.pat_props(op.properties) {
                    match prop {
                        ObjectPatternProperty::Property(pr) => self.visit_param_id(pr.value),
                        ObjectPatternProperty::RestElement(r) => self.visit_param_id(r.argument),
                    }
                }
            }
            ExpressionKind::ArrayPattern(ap) => {
                self.visit_decorators(ap.decorators(view));
                if let Some(ann) = ap.type_annotation(view) {
                    self.visit_type_annotation(ann);
                }
                for &el in view.expr_opt_list(ap.elements).iter().flatten() {
                    self.visit_param_id(el);
                }
            }
            ExpressionKind::AssignmentPattern(a) => {
                self.visit_decorators(view.opt_decorators(a.decorators));
                self.visit_param_id(a.left);
                self.visit_expr(a.right);
            }
            ExpressionKind::RestElement(r) => {
                if let Some(ann) = r.type_annotation(view) {
                    self.visit_type_annotation(ann);
                }
                self.visit_param_id(r.argument);
            }
            ExpressionKind::TSParameterProperty(pp) => self.visit_param_id(pp.parameter),
            _ => {}
        }
    }

    // --- types (general recursion) -------------------------------------------

    fn visit_type_annotation_opt(&mut self, ann: Option<&'a TSTypeAnnotation>) {
        if let Some(a) = ann {
            self.visit_type_annotation(a);
        }
    }

    fn visit_type_annotation(&mut self, ann: &'a TSTypeAnnotation) {
        self.visit_ty(ann.type_annotation);
    }

    fn visit_type_args(&mut self, args: &'a TSTypeParameterInstantiation) {
        self.visit_ty_list(args.params);
    }

    /// The per-type-parameter-declaration hook: the duplicate-name identity check
    /// (`check_type_parameters`) plus the descent into each parameter's constraint
    /// and default (both are types).
    fn visit_type_params(&mut self, params: Option<&'a TSTypeParameterDeclaration>) {
        if let Some(decl) = params {
            let ctx = self.member_ctx();
            duplicate_members::check_type_parameters(&ctx, decl, &mut self.diagnostics);
            for p in self.view.type_params(decl.params) {
                if let Some(c) = p.constraint {
                    self.visit_ty(c);
                }
                if let Some(d) = p.default {
                    self.visit_ty(d);
                }
            }
        }
    }

    /// Visit the type `id` names, read in place from its store.
    fn visit_ty(&mut self, id: TyId) {
        self.visit_type(self.view.ty(id));
    }

    fn visit_ty_list(&mut self, list: Run<TyId>) {
        for &t in self.view.ty_list(list) {
            self.visit_ty(t);
        }
    }

    fn visit_type(&mut self, ty: &'a TSType) {
        let view = self.view;
        match ty {
            TSType::TypeLiteral(tl) => self.visit_type_elements(tl.members),
            TSType::Array(a) => self.visit_ty(a.element_type),
            TSType::Union(u) => self.visit_ty_list(u.types),
            TSType::Intersection(i) => self.visit_ty_list(i.types),
            TSType::Parenthesized(p) => self.visit_ty(p.type_annotation),
            &TSType::Function(_, f) => {
                let f = view.fn_type(f);
                self.visit_type_params(f.type_parameters.as_ref());
                self.visit_params(f.params);
                self.visit_type_annotation(&f.return_type);
            }
            &TSType::Constructor(_, c) => {
                let c = view.ctor_type(c);
                self.visit_type_params(c.type_parameters.as_ref());
                self.visit_params(c.params);
                self.visit_type_annotation(&c.return_type);
            }
            TSType::Tuple(t) => self.visit_ty_list(t.element_types),
            TSType::TypePredicate(p) => {
                if let Some(t) = p.type_annotation {
                    self.visit_ty(t);
                }
            }
            TSType::Conditional(c) => {
                self.visit_ty(c.check_type);
                self.visit_ty(c.extends_type);
                self.visit_ty(c.true_type);
                self.visit_ty(c.false_type);
            }
            TSType::Mapped(m) => {
                self.visit_ty(m.type_parameter.constraint);
                if let Some(nt) = m.name_type {
                    self.visit_ty(nt);
                }
                if let Some(ta) = m.type_annotation {
                    self.visit_ty(ta);
                }
            }
            TSType::TypeOperator(o) => self.visit_ty(o.type_annotation),
            TSType::IndexedAccess(i) => {
                self.visit_ty(i.object_type);
                self.visit_ty(i.index_type);
            }
            TSType::Rest(r) => self.visit_ty(r.type_annotation),
            TSType::Optional(o) => self.visit_ty(o.type_annotation),
            TSType::NamedTupleMember(n) => self.visit_ty(n.element_type),
            &TSType::Infer(_, inf) => {
                let inf = view.infer_type(inf);
                if let Some(c) = inf.type_parameter.constraint {
                    self.visit_ty(c);
                }
                if let Some(d) = inf.type_parameter.default {
                    self.visit_ty(d);
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
            &TSType::Import(_, i) => {
                if let Some(args) = &view.import_type(i).type_arguments {
                    self.visit_type_args(args);
                }
            }
            TSType::Literal(lit) => {
                // A template-literal type's interpolations are types, which can be
                // type literals (`` `p-${ {x; x} }` ``); every other literal type is a
                // leaf.
                if let TSLiteralType::TemplateLiteral(t) = lit {
                    self.visit_ty_list(t.types);
                }
            }
            TSType::Keyword(_) | TSType::ThisType(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsv_ts::ast::Parsed;

    fn parse(source: &str) -> Parsed {
        tsv_ts::__private::parse(source).expect("parse")
    }

    /// The check pass alone over `source`: its raw (unsorted, un-deduped) diagnostics.
    fn check_diags(source: &str) -> Vec<Diagnostic> {
        check_file_members((&parse(source)).into(), source, FileId::ROOT)
    }

    /// Run the check pass in isolation over `source`, returning the raw (unsorted,
    /// un-deduped) diagnostic codes — the check pass alone, no binder, no
    /// program-wide sort/dedup.
    fn check_codes(source: &str) -> Vec<u32> {
        check_diags(source).iter().map(|d| d.code).collect()
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
        let diags = check_diags("function f<T, T, T>() {}");
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
        let diags = check_diags("interface I { ['a']: number; ['a']: string; }");
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
