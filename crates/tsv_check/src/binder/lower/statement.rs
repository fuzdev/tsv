//! The statement-shaped visitor methods — `visit_statement` and everything a
//! statement position (declarations, class/module bodies, import/export
//! specifiers) descends into.

use super::super::*;
use tsv_ts::ast::{
    CatchClause, ClassBody, ClassDeclaration, ClassMember, Decorator, ExportDefaultValue,
    ExportSpecifier, ForInOfLeft, ForInit, FunctionDeclaration, Identifier, ImportAttribute,
    ImportAttributeKey, ImportSpecifier, ModuleExportName, SwitchCase, TSDeclareFunction,
    TSEnumMember, TSEnumMemberId, TSInterfaceDeclaration, TSInterfaceHeritage, TSModuleDeclaration,
    TSModuleDeclarationBody, TSModuleName, TSModuleReference, TSTypeAnnotation,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, VariableDeclaration,
    VariableDeclarator,
};

impl<'a> SoaWalk<'a> {
    pub(in crate::binder) fn visit_statements(&mut self, stmts: Run<StmtId>, parent: NodeId) {
        for &stmt in self.view.stmt_list(stmts) {
            self.visit_statement(self.view.stmt(stmt), parent);
        }
    }

    /// Visit a statement: assign its id (keyed on its `Statement` row's address, the
    /// key the symbol bind and the address-map tests use), descend, then close.
    pub(in crate::binder) fn visit_statement(&mut self, stmt: &'a Statement, parent: NodeId) {
        let view = self.view;
        let id = self.add(statement_kind(stmt), stmt.span(), parent, addr_of(stmt));
        match &stmt.kind {
            StatementKind::ExpressionStatement(s) => self.visit_expr(s.expression, id),
            StatementKind::VariableDeclaration(decl) => self.visit_declarators(decl, id),
            &StatementKind::FunctionDeclaration(f) => self.descend_function(view.fn_decl(f), id),
            &StatementKind::ClassDeclaration(c) => self.descend_class(view.class_decl(c), id),
            &StatementKind::TSDeclareFunction(f) => {
                self.descend_declare_function(view.declare_fn(f), id);
            }
            &StatementKind::TSTypeAliasDeclaration(t) => {
                let t = view.type_alias(t);
                self.visit_identifier(&t.id, id);
                self.visit_type_params(t.type_parameters.as_ref(), id);
                self.visit_ty(t.type_annotation, id);
            }
            &StatementKind::TSInterfaceDeclaration(i) => {
                self.descend_interface(view.interface_decl(i), id);
            }
            &StatementKind::TSEnumDeclaration(e) => {
                let e = view.enum_decl(e);
                self.visit_identifier(&e.id, id);
                for member in view.enum_members(e.members) {
                    self.visit_enum_member(member, id);
                }
            }
            &StatementKind::TSModuleDeclaration(m) => self.descend_module(view.module_decl(m), id),
            &StatementKind::ImportDeclaration(imp) => {
                let imp = view.import_decl(imp);
                for spec in view.import_specs(imp.specifiers) {
                    self.visit_import_specifier(spec, id);
                }
                self.leaf(NodeKind::Literal, imp.source.span, addr_of(&imp.source), id);
                if let Some(attrs) = imp.attributes {
                    for a in view.import_attrs(attrs) {
                        self.visit_import_attribute(a, id);
                    }
                }
            }
            &StatementKind::TSImportEqualsDeclaration(ie) => {
                let ie = view.import_equals(ie);
                self.visit_identifier(&ie.id, id);
                self.visit_module_reference(&ie.module_reference, id);
            }
            &StatementKind::ExportNamedDeclaration(e) => {
                let e = view.export_named(e);
                if let Some(inner) = e.declaration {
                    self.visit_statement(view.stmt(inner), id);
                } else {
                    for spec in view.export_specs(e.specifiers) {
                        self.visit_export_specifier(spec, id);
                    }
                }
                if let Some(src) = &e.source {
                    self.leaf(NodeKind::Literal, src.span, addr_of(src), id);
                }
                if let Some(attrs) = e.attributes {
                    for a in view.import_attrs(attrs) {
                        self.visit_import_attribute(a, id);
                    }
                }
            }
            &StatementKind::ExportDefaultDeclaration(e) => {
                self.visit_export_default(&view.export_default(e).declaration, id);
            }
            &StatementKind::ExportAllDeclaration(e) => {
                let e = view.export_all(e);
                if let Some(exp) = &e.exported {
                    self.visit_module_export_name(exp, id);
                }
                self.leaf(NodeKind::Literal, e.source.span, addr_of(&e.source), id);
                if let Some(attrs) = e.attributes {
                    for a in view.import_attrs(attrs) {
                        self.visit_import_attribute(a, id);
                    }
                }
            }
            &StatementKind::TSExportAssignment(ea) => {
                self.visit_expr(view.export_assign(ea).expression, id);
            }
            StatementKind::TSNamespaceExportDeclaration(n) => self.visit_identifier(&n.id, id),
            StatementKind::ReturnStatement(s) => {
                if let Some(a) = s.argument {
                    self.visit_expr(a, id);
                }
            }
            // A function/try/catch/finally body `BlockStatement` is flattened by
            // its owner (a list-wrapper, per today's shape); a *standalone* block
            // statement is its own node whose body follows here.
            StatementKind::BlockStatement(block) => self.visit_statements(block.body, id),
            StatementKind::IfStatement(s) => {
                self.visit_expr(s.test, id);
                self.visit_statement(view.stmt(s.consequent), id);
                if let Some(alt) = view.opt_stmt(s.alternate) {
                    self.visit_statement(alt, id);
                }
            }
            StatementKind::ForStatement(s) => {
                match view.opt_for_head(s.init) {
                    Some(ForInit::VariableDeclaration(decl)) => {
                        self.visit_variable_declaration(decl, id);
                    }
                    Some(&ForInit::Expression(e)) => self.visit_expr(e, id),
                    None => {}
                }
                if let Some(t) = s.test {
                    self.visit_expr(t, id);
                }
                if let Some(u) = s.update {
                    self.visit_expr(u, id);
                }
                self.visit_statement(view.stmt(s.body), id);
            }
            StatementKind::ForInStatement(s) => {
                self.visit_for_left(view.for_left(s.left), id);
                self.visit_expr(s.right, id);
                self.visit_statement(view.stmt(s.body), id);
            }
            StatementKind::ForOfStatement(s) => {
                self.visit_for_left(view.for_left(s.left), id);
                self.visit_expr(s.right, id);
                self.visit_statement(view.stmt(s.body), id);
            }
            StatementKind::WhileStatement(s) => {
                self.visit_expr(s.test, id);
                self.visit_statement(view.stmt(s.body), id);
            }
            StatementKind::DoWhileStatement(s) => {
                self.visit_statement(view.stmt(s.body), id);
                self.visit_expr(s.test, id);
            }
            // The `with` object environment is a checker gap — tsc refuses `with` in a
            // TypeScript file outright (TS2410), so there is no name resolution to model
            // and a plain walk of the two children is the honest shape.
            StatementKind::WithStatement(s) => {
                self.visit_expr(s.object, id);
                self.visit_statement(view.stmt(s.body), id);
            }
            StatementKind::SwitchStatement(s) => {
                self.visit_expr(s.discriminant, id);
                for case in view.cases(s.cases) {
                    self.visit_switch_case(case, id);
                }
            }
            &StatementKind::TryStatement(s) => {
                let s = view.try_stmt(s);
                self.visit_statements(s.block.body, id);
                if let Some(handler) = view.opt_catch(s.handler) {
                    self.visit_catch_clause(handler, id);
                }
                if let Some(finalizer) = &s.finalizer {
                    self.visit_statements(finalizer.body, id);
                }
            }
            StatementKind::ThrowStatement(s) => self.visit_expr(s.argument, id),
            StatementKind::BreakStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label, id);
                }
            }
            StatementKind::ContinueStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label, id);
                }
            }
            &StatementKind::LabeledStatement(s) => {
                let s = view.labeled_stmt(s);
                self.visit_identifier(&s.label, id);
                self.visit_statement(view.stmt(s.body), id);
            }
            StatementKind::EmptyStatement(_) | StatementKind::DebuggerStatement(_) => {}
        }
        self.close(id);
    }

    // --- declaration descents (shared between statement + export-default) -----

    fn descend_function(&mut self, f: &'a FunctionDeclaration, id: NodeId) {
        self.descend_function_common(
            id,
            f.id.as_ref(),
            f.type_parameters.as_ref(),
            f.params,
            f.return_type.as_ref(),
            f.body.body,
        );
    }

    /// The body-bearing function descent shared by the declaration form
    /// ([`Self::descend_function`]) and the method-value / function-expression form
    /// (`SoaWalk::visit_function_expression`), keyed on the already-minted `id`. Kept
    /// as one helper so `FunctionDeclaration` and `FunctionExpression` — distinct
    /// types with the same field shape — never drift in what the walk descends.
    pub(super) fn descend_function_common(
        &mut self,
        id: NodeId,
        name: Option<&'a Identifier>,
        type_parameters: Option<&'a TSTypeParameterDeclaration>,
        params: Run<Expression>,
        return_type: Option<&'a TSTypeAnnotation>,
        body: Run<StmtId>,
    ) {
        if let Some(name) = name {
            self.visit_identifier(name, id);
        }
        self.visit_type_params(type_parameters, id);
        self.visit_params(params, id);
        self.visit_type_annotation_opt(return_type, id);
        self.visit_statements(body, id);
    }

    fn descend_declare_function(&mut self, f: &'a TSDeclareFunction, id: NodeId) {
        self.visit_identifier(&f.id, id);
        self.visit_type_params(f.type_parameters.as_ref(), id);
        self.visit_params(f.params, id);
        self.visit_type_annotation_opt(f.return_type.as_ref(), id);
    }

    fn descend_class(&mut self, c: &'a ClassDeclaration, id: NodeId) {
        if let Some(name) = &c.id {
            self.visit_identifier(name, id);
        }
        // The class's own `<T>` — kept in sync with the `ClassExpression` arm in
        // `visit_expression` (guarded by the `require_node_id` coverage test).
        self.visit_type_params(c.type_parameters.as_ref(), id);
        self.visit_class_heritage(
            c.decorators,
            c.super_class,
            c.super_type_parameters.as_ref(),
            c.implements,
            id,
        );
        self.visit_class_body(&c.body, id);
    }

    fn descend_interface(&mut self, i: &'a TSInterfaceDeclaration, id: NodeId) {
        self.visit_identifier(&i.id, id);
        self.visit_type_params(i.type_parameters.as_ref(), id);
        self.visit_heritages(i.extends, id);
        // `TSInterfaceBody` is a list-wrapper: its members stay flat under the
        // interface (no separate node), matching today's shape.
        self.visit_type_elements(i.body.body, id);
    }

    fn visit_export_default(&mut self, value: &'a ExportDefaultValue, parent: NodeId) {
        match value {
            &ExportDefaultValue::Expression(e) => self.visit_expr(e, parent),
            ExportDefaultValue::FunctionDeclaration(f) => {
                let id = self.add(NodeKind::FunctionDeclaration, f.span, parent, addr_of(f));
                self.descend_function(f, id);
                self.close(id);
            }
            ExportDefaultValue::TSDeclareFunction(f) => {
                let id = self.add(NodeKind::TSDeclareFunction, f.span, parent, addr_of(f));
                self.descend_declare_function(f, id);
                self.close(id);
            }
            ExportDefaultValue::ClassDeclaration(c) => {
                let id = self.add(NodeKind::ClassDeclaration, c.span, parent, addr_of(c));
                self.descend_class(c, id);
                self.close(id);
            }
            ExportDefaultValue::TSInterfaceDeclaration(i) => {
                let id = self.add(NodeKind::TSInterfaceDeclaration, i.span, parent, addr_of(i));
                self.descend_interface(i, id);
                self.close(id);
            }
        }
    }

    // --- variable declarations / for headers ---------------------------------

    fn visit_variable_declaration(&mut self, decl: &'a VariableDeclaration, parent: NodeId) {
        let id = self.add(
            NodeKind::VariableDeclaration,
            decl.span,
            parent,
            addr_of(decl),
        );
        self.visit_declarators(decl, id);
        self.close(id);
    }

    fn visit_declarators(&mut self, decl: &'a VariableDeclaration, parent: NodeId) {
        for declarator in self.view.declarators(decl.declarations) {
            self.visit_declarator(declarator, parent);
        }
    }

    fn visit_declarator(&mut self, declarator: &'a VariableDeclarator, parent: NodeId) {
        let id = self.add(
            NodeKind::VariableDeclarator,
            declarator.span,
            parent,
            addr_of(declarator),
        );
        // The binding target — an identifier (with its type annotation) or a
        // destructuring pattern — is an `Expression`, routed through the
        // pattern-aware `visit_expression`.
        self.visit_expr(declarator.id, id);
        if let Some(init) = declarator.init {
            self.visit_expr(init, id);
        }
        self.close(id);
    }

    fn visit_for_left(&mut self, left: &'a ForInOfLeft, parent: NodeId) {
        match left {
            ForInOfLeft::VariableDeclaration(decl) => self.visit_variable_declaration(decl, parent),
            // A pattern here may be an Object/ArrayPattern — pattern-aware descent.
            &ForInOfLeft::Pattern(e) => self.visit_expr(e, parent),
        }
    }

    // --- modules / enums / cases / catch -------------------------------------

    /// Descend a module's name and body (the module's own node is `module_id`).
    fn descend_module(&mut self, m: &'a TSModuleDeclaration, module_id: NodeId) {
        match &m.id {
            TSModuleName::Identifier(id) => self.visit_identifier(id, module_id),
            TSModuleName::Literal(lit) => {
                self.leaf(NodeKind::Literal, lit.span, addr_of(lit), module_id);
            }
        }
        match &m.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                let id = self.add(
                    NodeKind::TSModuleBlock,
                    block.span,
                    module_id,
                    addr_of(block),
                );
                self.visit_statements(block.body, id);
                self.close(id);
            }
            // The dotted-namespace continuation (`namespace A.B {}`) — a nested
            // `TSModuleDeclaration` node (reused kind), keyed on its own row in the
            // module store, recursed.
            &Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                let nested = self.view.module_decl(nested);
                let id = self.add(
                    NodeKind::TSModuleDeclaration,
                    nested.span,
                    module_id,
                    addr_of(nested),
                );
                self.descend_module(nested, id);
                self.close(id);
            }
            None => {}
        }
    }

    fn visit_enum_member(&mut self, member: &'a TSEnumMember, parent: NodeId) {
        let id = self.add(NodeKind::TSEnumMember, member.span, parent, addr_of(member));
        match &member.id {
            TSEnumMemberId::Identifier(idn) => self.visit_identifier(idn, id),
            TSEnumMemberId::String(lit) => self.leaf(NodeKind::Literal, lit.span, addr_of(lit), id),
        }
        if let Some(init) = member.initializer {
            self.visit_expr(init, id);
        }
        self.close(id);
    }

    fn visit_switch_case(&mut self, case: &'a SwitchCase, parent: NodeId) {
        let id = self.add(NodeKind::SwitchCase, case.span, parent, addr_of(case));
        if let Some(t) = case.test {
            self.visit_expr(t, id);
        }
        self.visit_statements(case.consequent, id);
        self.close(id);
    }

    fn visit_catch_clause(&mut self, h: &'a CatchClause, parent: NodeId) {
        let id = self.add(NodeKind::CatchClause, h.span, parent, addr_of(h));
        if let Some(param) = &h.param {
            self.visit_expression(param, id);
        }
        // The catch body block is flattened (list-wrapper, today's shape).
        self.visit_statements(h.body.body, id);
        self.close(id);
    }

    // --- classes -------------------------------------------------------------

    /// Descend class heritage: decorators, the `extends` expression + its type
    /// arguments, and each `implements`/`extends` heritage clause.
    pub(super) fn visit_class_heritage(
        &mut self,
        decorators: Option<Run<Decorator>>,
        super_class: Option<ExprId>,
        super_type_parameters: Option<&'a TSTypeParameterInstantiation>,
        heritages: Run<TSInterfaceHeritage>,
        parent: NodeId,
    ) {
        if let Some(decs) = self.view.opt_decorators(decorators) {
            self.visit_decorators(decs, parent);
        }
        if let Some(sc) = super_class {
            self.visit_expr(sc, parent);
        }
        if let Some(tp) = super_type_parameters {
            self.visit_type_args(tp, parent);
        }
        self.visit_heritages(heritages, parent);
    }

    fn visit_heritages(&mut self, heritages: Run<TSInterfaceHeritage>, parent: NodeId) {
        for h in self.view.heritages(heritages) {
            let id = self.add(NodeKind::TSInterfaceHeritage, h.span, parent, addr_of(h));
            // The heritage target (`extends Base` / `implements Base`) — an entity
            // name — plus its type arguments.
            self.visit_entity_name(&h.expression, id);
            if let Some(ta) = &h.type_arguments {
                self.visit_type_args(ta, id);
            }
            self.close(id);
        }
    }

    /// `ClassBody` is a list-wrapper: its members stay flat under the class (no
    /// separate node), matching today's shape.
    pub(super) fn visit_class_body(&mut self, body: &'a ClassBody, parent: NodeId) {
        for member in self.view.class_members(body.body) {
            self.visit_class_member(member, parent);
        }
    }

    fn visit_class_member(&mut self, member: &'a ClassMember, parent: NodeId) {
        match member {
            ClassMember::MethodDefinition(m) => {
                let id = self.add(NodeKind::MethodDefinition, m.span, parent, addr_of(m));
                if let Some(decs) = self.view.opt_decorators(m.decorators) {
                    self.visit_decorators(decs, id);
                }
                self.visit_expr(m.key, id);
                self.visit_function_expression(&m.value, id);
                self.close(id);
            }
            ClassMember::PropertyDefinition(p) => {
                let id = self.add(NodeKind::PropertyDefinition, p.span, parent, addr_of(p));
                if let Some(decs) = self.view.opt_decorators(p.decorators) {
                    self.visit_decorators(decs, id);
                }
                self.visit_expr(p.key, id);
                self.visit_type_annotation_opt(p.type_annotation.as_ref(), id);
                if let Some(v) = p.value {
                    self.visit_expr(v, id);
                }
                self.close(id);
            }
            ClassMember::StaticBlock(s) => {
                let id = self.add(NodeKind::StaticBlock, s.span, parent, addr_of(s));
                self.visit_statements(s.body, id);
                self.close(id);
            }
            ClassMember::IndexSignature(i) => self.visit_index_signature(i, parent),
        }
    }

    // --- imports / exports ----------------------------------------------------

    fn visit_import_specifier(&mut self, spec: &'a ImportSpecifier, parent: NodeId) {
        match spec {
            ImportSpecifier::Default(d) => {
                let id = self.add(NodeKind::ImportDefaultSpecifier, d.span, parent, addr_of(d));
                self.visit_identifier(&d.local, id);
                self.close(id);
            }
            ImportSpecifier::Named(n) => {
                let id = self.add(NodeKind::ImportNamedSpecifier, n.span, parent, addr_of(n));
                self.visit_module_export_name(&n.imported, id);
                self.visit_identifier(&n.local, id);
                self.close(id);
            }
            ImportSpecifier::Namespace(n) => {
                let id = self.add(
                    NodeKind::ImportNamespaceSpecifier,
                    n.span,
                    parent,
                    addr_of(n),
                );
                self.visit_identifier(&n.local, id);
                self.close(id);
            }
        }
    }

    fn visit_export_specifier(&mut self, spec: &'a ExportSpecifier, parent: NodeId) {
        let id = self.add(NodeKind::ExportSpecifier, spec.span, parent, addr_of(spec));
        self.visit_module_export_name(&spec.local, id);
        self.visit_module_export_name(&spec.exported, id);
        self.close(id);
    }

    fn visit_module_export_name(&mut self, name: &'a ModuleExportName, parent: NodeId) {
        match name {
            ModuleExportName::Identifier(id) => self.visit_identifier(id, parent),
            ModuleExportName::Literal(lit) => {
                self.leaf(NodeKind::Literal, lit.span, addr_of(lit), parent);
            }
        }
    }

    fn visit_import_attribute(&mut self, attr: &'a ImportAttribute, parent: NodeId) {
        let id = self.add(NodeKind::ImportAttribute, attr.span, parent, addr_of(attr));
        match &attr.key {
            ImportAttributeKey::Identifier(idn) => self.visit_identifier(idn, id),
            ImportAttributeKey::Literal(lit) => {
                self.leaf(NodeKind::Literal, lit.span, addr_of(lit), id);
            }
        }
        self.leaf(NodeKind::Literal, attr.value.span, addr_of(&attr.value), id);
        self.close(id);
    }

    fn visit_module_reference(&mut self, mr: &'a TSModuleReference, parent: NodeId) {
        match mr {
            TSModuleReference::ExternalModuleReference(ext) => {
                let id = self.add(
                    NodeKind::TSExternalModuleReference,
                    ext.span,
                    parent,
                    addr_of(ext),
                );
                self.leaf(
                    NodeKind::Literal,
                    ext.expression.span,
                    addr_of(&ext.expression),
                    id,
                );
                self.close(id);
            }
            TSModuleReference::EntityName(en) => self.visit_entity_name(en, parent),
        }
    }
}
