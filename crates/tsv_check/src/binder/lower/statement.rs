//! The statement-shaped visitor methods — `visit_statement` and everything a
//! statement position (declarations, class/module bodies, import/export
//! specifiers) descends into.

use super::super::*;
use tsv_ts::ast::internal::{
    CatchClause, ClassBody, ClassDeclaration, ClassMember, Decorator, ExportDefaultValue,
    ExportSpecifier, Expression, ForInOfLeft, ForInit, FunctionDeclaration, Identifier,
    ImportAttribute, ImportAttributeKey, ImportSpecifier, ModuleExportName, Statement, SwitchCase,
    TSDeclareFunction, TSEnumMember, TSEnumMemberId, TSInterfaceDeclaration, TSInterfaceHeritage,
    TSModuleDeclaration, TSModuleDeclarationBody, TSModuleName, TSModuleReference,
    TSTypeAnnotation, TSTypeParameterDeclaration, TSTypeParameterInstantiation,
    VariableDeclaration, VariableDeclarator,
};

impl SoaWalk {
    pub(super) fn visit_statements(&mut self, stmts: &[Statement<'_>], parent: NodeId) {
        for stmt in stmts {
            self.visit_statement(stmt, parent);
        }
    }

    /// Visit a statement: assign its id (keyed on the `&Statement` address, the key
    /// the symbol bind and the address-map tests use), descend, then close.
    pub(in crate::binder) fn visit_statement(&mut self, stmt: &Statement<'_>, parent: NodeId) {
        let id = self.add(
            statement_kind(stmt),
            stmt.span(),
            Some(parent),
            addr_of(stmt),
        );
        match stmt {
            Statement::ExpressionStatement(s) => self.visit_expression(&s.expression, id),
            Statement::VariableDeclaration(decl) => self.visit_declarators(decl, id),
            Statement::FunctionDeclaration(f) => self.descend_function(f, id),
            Statement::ClassDeclaration(c) => self.descend_class(c, id),
            Statement::TSDeclareFunction(f) => self.descend_declare_function(f, id),
            Statement::TSTypeAliasDeclaration(t) => {
                self.visit_identifier(&t.id, id);
                self.visit_type_params(t.type_parameters.as_ref(), id);
                self.visit_type(&t.type_annotation, id);
            }
            Statement::TSInterfaceDeclaration(i) => self.descend_interface(i, id),
            Statement::TSEnumDeclaration(e) => {
                self.visit_identifier(&e.id, id);
                for member in e.members {
                    self.visit_enum_member(member, id);
                }
            }
            Statement::TSModuleDeclaration(m) => self.descend_module(m, id),
            Statement::ImportDeclaration(imp) => {
                for spec in imp.specifiers {
                    self.visit_import_specifier(spec, id);
                }
                self.leaf(NodeKind::Literal, imp.source.span, addr_of(&imp.source), id);
                if let Some(attrs) = imp.attributes {
                    for a in attrs {
                        self.visit_import_attribute(a, id);
                    }
                }
            }
            Statement::TSImportEqualsDeclaration(ie) => {
                self.visit_identifier(&ie.id, id);
                self.visit_module_reference(&ie.module_reference, id);
            }
            Statement::ExportNamedDeclaration(e) => {
                if let Some(inner) = e.declaration {
                    self.visit_statement(inner, id);
                } else {
                    for spec in e.specifiers {
                        self.visit_export_specifier(spec, id);
                    }
                }
                if let Some(src) = &e.source {
                    self.leaf(NodeKind::Literal, src.span, addr_of(src), id);
                }
                if let Some(attrs) = e.attributes {
                    for a in attrs {
                        self.visit_import_attribute(a, id);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(e) => self.visit_export_default(&e.declaration, id),
            Statement::ExportAllDeclaration(e) => {
                if let Some(exp) = &e.exported {
                    self.visit_module_export_name(exp, id);
                }
                self.leaf(NodeKind::Literal, e.source.span, addr_of(&e.source), id);
                if let Some(attrs) = e.attributes {
                    for a in attrs {
                        self.visit_import_attribute(a, id);
                    }
                }
            }
            Statement::TSExportAssignment(ea) => self.visit_expression(&ea.expression, id),
            Statement::TSNamespaceExportDeclaration(n) => self.visit_identifier(&n.id, id),
            Statement::ReturnStatement(s) => {
                if let Some(a) = &s.argument {
                    self.visit_expression(a, id);
                }
            }
            // A function/try/catch/finally body `BlockStatement` is flattened by
            // its owner (a list-wrapper, per today's shape); a *standalone* block
            // statement is its own node whose body follows here.
            Statement::BlockStatement(block) => self.visit_statements(block.body, id),
            Statement::IfStatement(s) => {
                self.visit_expression(&s.test, id);
                self.visit_statement(s.consequent, id);
                if let Some(alt) = s.alternate {
                    self.visit_statement(alt, id);
                }
            }
            Statement::ForStatement(s) => {
                match &s.init {
                    Some(ForInit::VariableDeclaration(decl)) => {
                        self.visit_variable_declaration(decl, id);
                    }
                    Some(ForInit::Expression(e)) => self.visit_expression(e, id),
                    None => {}
                }
                if let Some(t) = &s.test {
                    self.visit_expression(t, id);
                }
                if let Some(u) = &s.update {
                    self.visit_expression(u, id);
                }
                self.visit_statement(s.body, id);
            }
            Statement::ForInStatement(s) => {
                self.visit_for_left(&s.left, id);
                self.visit_expression(&s.right, id);
                self.visit_statement(s.body, id);
            }
            Statement::ForOfStatement(s) => {
                self.visit_for_left(&s.left, id);
                self.visit_expression(&s.right, id);
                self.visit_statement(s.body, id);
            }
            Statement::WhileStatement(s) => {
                self.visit_expression(&s.test, id);
                self.visit_statement(s.body, id);
            }
            Statement::DoWhileStatement(s) => {
                self.visit_statement(s.body, id);
                self.visit_expression(&s.test, id);
            }
            Statement::SwitchStatement(s) => {
                self.visit_expression(&s.discriminant, id);
                for case in s.cases {
                    self.visit_switch_case(case, id);
                }
            }
            Statement::TryStatement(s) => {
                self.visit_statements(s.block.body, id);
                if let Some(handler) = &s.handler {
                    self.visit_catch_clause(handler, id);
                }
                if let Some(finalizer) = &s.finalizer {
                    self.visit_statements(finalizer.body, id);
                }
            }
            Statement::ThrowStatement(s) => self.visit_expression(&s.argument, id),
            Statement::BreakStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label, id);
                }
            }
            Statement::ContinueStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label, id);
                }
            }
            Statement::LabeledStatement(s) => {
                self.visit_identifier(&s.label, id);
                self.visit_statement(s.body, id);
            }
            Statement::EmptyStatement(_) | Statement::DebuggerStatement(_) => {}
        }
        self.close(id);
    }

    // --- declaration descents (shared between statement + export-default) -----

    fn descend_function(&mut self, f: &FunctionDeclaration<'_>, id: NodeId) {
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
        name: Option<&Identifier<'_>>,
        type_parameters: Option<&TSTypeParameterDeclaration<'_>>,
        params: &[Expression<'_>],
        return_type: Option<&TSTypeAnnotation<'_>>,
        body: &[Statement<'_>],
    ) {
        if let Some(name) = name {
            self.visit_identifier(name, id);
        }
        self.visit_type_params(type_parameters, id);
        self.visit_params(params, id);
        self.visit_type_annotation_opt(return_type, id);
        self.visit_statements(body, id);
    }

    fn descend_declare_function(&mut self, f: &TSDeclareFunction<'_>, id: NodeId) {
        self.visit_identifier(&f.id, id);
        self.visit_type_params(f.type_parameters.as_ref(), id);
        self.visit_params(f.params, id);
        self.visit_type_annotation_opt(f.return_type.as_ref(), id);
    }

    fn descend_class(&mut self, c: &ClassDeclaration<'_>, id: NodeId) {
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

    fn descend_interface(&mut self, i: &TSInterfaceDeclaration<'_>, id: NodeId) {
        self.visit_identifier(&i.id, id);
        self.visit_type_params(i.type_parameters.as_ref(), id);
        self.visit_heritages(i.extends, id);
        // `TSInterfaceBody` is a list-wrapper: its members stay flat under the
        // interface (no separate node), matching today's shape.
        self.visit_type_elements(i.body.body, id);
    }

    fn visit_export_default(&mut self, value: &ExportDefaultValue<'_>, parent: NodeId) {
        match value {
            ExportDefaultValue::Expression(e) => self.visit_expression(e, parent),
            ExportDefaultValue::FunctionDeclaration(f) => {
                let id = self.add(
                    NodeKind::FunctionDeclaration,
                    f.span,
                    Some(parent),
                    addr_of(f),
                );
                self.descend_function(f, id);
                self.close(id);
            }
            ExportDefaultValue::TSDeclareFunction(f) => {
                let id = self.add(
                    NodeKind::TSDeclareFunction,
                    f.span,
                    Some(parent),
                    addr_of(f),
                );
                self.descend_declare_function(f, id);
                self.close(id);
            }
            ExportDefaultValue::ClassDeclaration(c) => {
                let id = self.add(NodeKind::ClassDeclaration, c.span, Some(parent), addr_of(c));
                self.descend_class(c, id);
                self.close(id);
            }
            ExportDefaultValue::TSInterfaceDeclaration(i) => {
                let id = self.add(
                    NodeKind::TSInterfaceDeclaration,
                    i.span,
                    Some(parent),
                    addr_of(i),
                );
                self.descend_interface(i, id);
                self.close(id);
            }
        }
    }

    // --- variable declarations / for headers ---------------------------------

    fn visit_variable_declaration(&mut self, decl: &VariableDeclaration<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::VariableDeclaration,
            decl.span,
            Some(parent),
            addr_of(decl),
        );
        self.visit_declarators(decl, id);
        self.close(id);
    }

    fn visit_declarators(&mut self, decl: &VariableDeclaration<'_>, parent: NodeId) {
        for declarator in decl.declarations {
            self.visit_declarator(declarator, parent);
        }
    }

    fn visit_declarator(&mut self, declarator: &VariableDeclarator<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::VariableDeclarator,
            declarator.span,
            Some(parent),
            addr_of(declarator),
        );
        // The binding target — an identifier (with its type annotation) or a
        // destructuring pattern — is an `Expression`, routed through the
        // pattern-aware `visit_expression`.
        self.visit_expression(&declarator.id, id);
        if let Some(init) = &declarator.init {
            self.visit_expression(init, id);
        }
        self.close(id);
    }

    fn visit_for_left(&mut self, left: &ForInOfLeft<'_>, parent: NodeId) {
        match left {
            ForInOfLeft::VariableDeclaration(decl) => self.visit_variable_declaration(decl, parent),
            // A pattern here may be an Object/ArrayPattern — pattern-aware descent.
            ForInOfLeft::Pattern(e) => self.visit_expression(e, parent),
        }
    }

    // --- modules / enums / cases / catch -------------------------------------

    /// Descend a module's name and body (the module's own node is `module_id`).
    fn descend_module(&mut self, m: &TSModuleDeclaration<'_>, module_id: NodeId) {
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
                    Some(module_id),
                    addr_of(block),
                );
                self.visit_statements(block.body, id);
                self.close(id);
            }
            // The dotted-namespace continuation (`namespace A.B {}`) — a nested
            // `TSModuleDeclaration` node (reused kind), recursed.
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                let id = self.add(
                    NodeKind::TSModuleDeclaration,
                    nested.span,
                    Some(module_id),
                    addr_of(nested),
                );
                self.descend_module(nested, id);
                self.close(id);
            }
            None => {}
        }
    }

    fn visit_enum_member(&mut self, member: &TSEnumMember<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::TSEnumMember,
            member.span,
            Some(parent),
            addr_of(member),
        );
        match &member.id {
            TSEnumMemberId::Identifier(idn) => self.visit_identifier(idn, id),
            TSEnumMemberId::String(lit) => self.leaf(NodeKind::Literal, lit.span, addr_of(lit), id),
        }
        if let Some(init) = &member.initializer {
            self.visit_expression(init, id);
        }
        self.close(id);
    }

    fn visit_switch_case(&mut self, case: &SwitchCase<'_>, parent: NodeId) {
        let id = self.add(NodeKind::SwitchCase, case.span, Some(parent), addr_of(case));
        if let Some(t) = &case.test {
            self.visit_expression(t, id);
        }
        self.visit_statements(case.consequent, id);
        self.close(id);
    }

    fn visit_catch_clause(&mut self, h: &CatchClause<'_>, parent: NodeId) {
        let id = self.add(NodeKind::CatchClause, h.span, Some(parent), addr_of(h));
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
        decorators: Option<&[Decorator<'_>]>,
        super_class: Option<&Expression<'_>>,
        super_type_parameters: Option<&TSTypeParameterInstantiation<'_>>,
        heritages: &[TSInterfaceHeritage<'_>],
        parent: NodeId,
    ) {
        if let Some(decs) = decorators {
            self.visit_decorators(decs, parent);
        }
        if let Some(sc) = super_class {
            self.visit_expression(sc, parent);
        }
        if let Some(tp) = super_type_parameters {
            self.visit_type_args(tp, parent);
        }
        self.visit_heritages(heritages, parent);
    }

    fn visit_heritages(&mut self, heritages: &[TSInterfaceHeritage<'_>], parent: NodeId) {
        for h in heritages {
            let id = self.add(
                NodeKind::TSInterfaceHeritage,
                h.span,
                Some(parent),
                addr_of(h),
            );
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
    pub(super) fn visit_class_body(&mut self, body: &ClassBody<'_>, parent: NodeId) {
        for member in body.body {
            self.visit_class_member(member, parent);
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember<'_>, parent: NodeId) {
        match member {
            ClassMember::MethodDefinition(m) => {
                let id = self.add(NodeKind::MethodDefinition, m.span, Some(parent), addr_of(m));
                if let Some(decs) = m.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_expression(&m.key, id);
                self.visit_function_expression(&m.value, id);
                self.close(id);
            }
            ClassMember::PropertyDefinition(p) => {
                let id = self.add(
                    NodeKind::PropertyDefinition,
                    p.span,
                    Some(parent),
                    addr_of(p),
                );
                if let Some(decs) = p.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_expression(&p.key, id);
                self.visit_type_annotation_opt(p.type_annotation.as_ref(), id);
                if let Some(v) = &p.value {
                    self.visit_expression(v, id);
                }
                self.close(id);
            }
            ClassMember::StaticBlock(s) => {
                let id = self.add(NodeKind::StaticBlock, s.span, Some(parent), addr_of(s));
                self.visit_statements(s.body, id);
                self.close(id);
            }
            ClassMember::IndexSignature(i) => self.visit_index_signature(i, parent),
        }
    }

    // --- imports / exports ----------------------------------------------------

    fn visit_import_specifier(&mut self, spec: &ImportSpecifier<'_>, parent: NodeId) {
        match spec {
            ImportSpecifier::Default(d) => {
                let id = self.add(
                    NodeKind::ImportDefaultSpecifier,
                    d.span,
                    Some(parent),
                    addr_of(d),
                );
                self.visit_identifier(&d.local, id);
                self.close(id);
            }
            ImportSpecifier::Named(n) => {
                let id = self.add(
                    NodeKind::ImportNamedSpecifier,
                    n.span,
                    Some(parent),
                    addr_of(n),
                );
                self.visit_module_export_name(&n.imported, id);
                self.visit_identifier(&n.local, id);
                self.close(id);
            }
            ImportSpecifier::Namespace(n) => {
                let id = self.add(
                    NodeKind::ImportNamespaceSpecifier,
                    n.span,
                    Some(parent),
                    addr_of(n),
                );
                self.visit_identifier(&n.local, id);
                self.close(id);
            }
        }
    }

    fn visit_export_specifier(&mut self, spec: &ExportSpecifier<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::ExportSpecifier,
            spec.span,
            Some(parent),
            addr_of(spec),
        );
        self.visit_module_export_name(&spec.local, id);
        self.visit_module_export_name(&spec.exported, id);
        self.close(id);
    }

    fn visit_module_export_name(&mut self, name: &ModuleExportName<'_>, parent: NodeId) {
        match name {
            ModuleExportName::Identifier(id) => self.visit_identifier(id, parent),
            ModuleExportName::Literal(lit) => {
                self.leaf(NodeKind::Literal, lit.span, addr_of(lit), parent);
            }
        }
    }

    fn visit_import_attribute(&mut self, attr: &ImportAttribute<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::ImportAttribute,
            attr.span,
            Some(parent),
            addr_of(attr),
        );
        match &attr.key {
            ImportAttributeKey::Identifier(idn) => self.visit_identifier(idn, id),
            ImportAttributeKey::Literal(lit) => {
                self.leaf(NodeKind::Literal, lit.span, addr_of(lit), id);
            }
        }
        self.leaf(NodeKind::Literal, attr.value.span, addr_of(&attr.value), id);
        self.close(id);
    }

    fn visit_module_reference(&mut self, mr: &TSModuleReference<'_>, parent: NodeId) {
        match mr {
            TSModuleReference::ExternalModuleReference(ext) => {
                let id = self.add(
                    NodeKind::TSExternalModuleReference,
                    ext.span,
                    Some(parent),
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
