//! The statement-shaped bind-descent — `visit_statement` (the dispatch) and
//! every `bind_*_statement`, the export/default/function-name group, the
//! param/binding helpers, and the module/import/export-specifier binds.
//! Contributes its own `impl SymbolBinder` block; the struct, the
//! functions-first statement-list driver, and the scope helpers live in the
//! parent module. Purely a locality split — no behavior distinction.

use super::super::symbols::SymbolFlags;
use super::{ContainerKind, DeclInput, DeclMods, NodeKind, Scope, SymbolBinder};
use crate::ids::NodeId;
use tsv_lang::Span;
use tsv_ts::ast::internal::{
    ExportDefaultValue, ExportSpecifier, Expression, ForInOfLeft, ForInit, Identifier,
    ImportSpecifier, ModuleExportName, ObjectPatternProperty, Statement, TSModuleDeclarationBody,
    TSModuleName,
};

impl<'a> SymbolBinder<'a> {
    /// Sub-step A: declare a hoisted function's symbol only (no body descent),
    /// unwrapping any `export`/`export default` wrapper for its modifiers.
    pub(super) fn declare_hoisted_function(&mut self, stmt: &Statement<'a>) {
        match stmt {
            Statement::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    self.bind_function_name(id, f.span, DeclMods::default());
                }
            }
            Statement::TSDeclareFunction(f) => {
                self.bind_function_name(&f.id, f.span, DeclMods::default());
            }
            Statement::ExportNamedDeclaration(e) => {
                if let Some(inner) = e.declaration {
                    self.declare_hoisted_function_inner(
                        inner,
                        DeclMods {
                            exported: true,
                            default: false,
                        },
                    );
                }
            }
            Statement::ExportDefaultDeclaration(e) => {
                let mods = DeclMods {
                    exported: true,
                    default: true,
                };
                match &e.declaration {
                    ExportDefaultValue::FunctionDeclaration(f) => {
                        self.bind_default_function(f.id.as_ref(), e.span, mods);
                    }
                    ExportDefaultValue::TSDeclareFunction(f) => {
                        self.bind_default_function(Some(&f.id), e.span, mods);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn declare_hoisted_function_inner(&mut self, inner: &Statement<'a>, mods: DeclMods) {
        match inner {
            Statement::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    self.bind_function_name(id, f.span, mods);
                }
            }
            Statement::TSDeclareFunction(f) => self.bind_function_name(&f.id, f.span, mods),
            _ => {}
        }
    }

    // --- statements ----------------------------------------------------------

    pub(super) fn visit_statement(
        &mut self,
        stmt: &Statement<'a>,
        mods: DeclMods,
        skip_symbol: bool,
    ) {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                let (includes, excludes, block_scoped) = var_flags(decl.kind);
                for d in decl.declarations {
                    self.bind_binding(&d.id, includes, excludes, block_scoped, mods, decl.span);
                    if let Some(init) = &d.init {
                        self.visit_expression(init);
                    }
                }
            }
            Statement::FunctionDeclaration(f) => {
                if !skip_symbol && let Some(id) = &f.id {
                    self.bind_function_name(id, f.span, mods);
                }
                self.with_function_scope(f.type_parameters.as_ref(), |b| {
                    b.bind_params(f.params);
                    b.bind_statement_list(f.body.body, true);
                });
            }
            Statement::TSDeclareFunction(f) => {
                if !skip_symbol {
                    self.bind_function_name(&f.id, f.span, mods);
                }
                self.with_function_scope(f.type_parameters.as_ref(), |b| {
                    b.bind_params(f.params);
                });
            }
            Statement::ClassDeclaration(c) => self.bind_class_statement(c, mods, skip_symbol),
            Statement::TSInterfaceDeclaration(i) => {
                let d = self.decl_from_ident(&i.id, i.span, mods);
                let sym = self.declare_block_scoped(
                    d,
                    SymbolFlags::INTERFACE,
                    SymbolFlags::INTERFACE_EXCLUDES,
                );
                self.bind_interface_body(&i.body, sym, i.type_parameters.as_ref());
            }
            Statement::TSEnumDeclaration(e) => self.bind_enum_statement(e, mods),
            Statement::TSModuleDeclaration(m) => self.bind_module(m, mods),
            Statement::TSTypeAliasDeclaration(t) => self.bind_type_alias_statement(t, mods),
            Statement::ImportDeclaration(imp) => {
                for spec in imp.specifiers {
                    self.bind_import_specifier(spec);
                }
            }
            Statement::TSImportEqualsDeclaration(ie) => self.bind_import_equals_statement(ie),
            Statement::ExportNamedDeclaration(e) => {
                self.bind_export_named_statement(e, skip_symbol);
            }
            Statement::ExportDefaultDeclaration(e) => self.bind_export_default(e, skip_symbol),
            // Control flow: descend for nested bindings + block scopes.
            Statement::BlockStatement(b) => {
                self.with_block_scope(|bd| bd.bind_statement_list(b.body, true));
            }
            Statement::IfStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.consequent, DeclMods::default(), false);
                if let Some(alt) = s.alternate {
                    self.visit_statement(alt, DeclMods::default(), false);
                }
            }
            Statement::ForStatement(s) => self.bind_for_statement(s),
            Statement::ForInStatement(s) => self.with_block_scope(|bd| {
                bd.bind_for_left(&s.left);
                bd.visit_expression(&s.right);
                bd.visit_statement(s.body, DeclMods::default(), false);
            }),
            Statement::ForOfStatement(s) => self.with_block_scope(|bd| {
                bd.bind_for_left(&s.left);
                bd.visit_expression(&s.right);
                bd.visit_statement(s.body, DeclMods::default(), false);
            }),
            Statement::WhileStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.body, DeclMods::default(), false);
            }
            Statement::DoWhileStatement(s) => {
                self.visit_statement(s.body, DeclMods::default(), false);
                self.visit_expression(&s.test);
            }
            Statement::SwitchStatement(s) => self.bind_switch_statement(s),
            Statement::TryStatement(s) => self.bind_try_statement(s),
            Statement::LabeledStatement(s) => {
                self.visit_statement(s.body, DeclMods::default(), false);
            }
            Statement::ReturnStatement(s) => {
                if let Some(a) = &s.argument {
                    self.visit_expression(a);
                }
            }
            Statement::ThrowStatement(s) => self.visit_expression(&s.argument),
            Statement::ExpressionStatement(s) => self.visit_expression(&s.expression),
            Statement::TSExportAssignment(ea) => self.bind_export_assignment_statement(ea),
            Statement::ExportAllDeclaration(_)
            | Statement::TSNamespaceExportDeclaration(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_) => {}
        }
    }

    #[inline]
    fn bind_class_statement(
        &mut self,
        c: &tsv_ts::ast::internal::ClassDeclaration<'a>,
        mods: DeclMods,
        skip_symbol: bool,
    ) {
        let sym = if skip_symbol {
            None
        } else {
            c.id.as_ref().map(|id| {
                let d = self.decl_from_ident(id, c.span, mods);
                self.declare_block_scoped(d, SymbolFlags::CLASS, SymbolFlags::CLASS_EXCLUDES)
            })
        };
        self.bind_class_body(&c.body, sym, c.type_parameters.as_ref());
    }

    #[inline]
    fn bind_enum_statement(
        &mut self,
        e: &tsv_ts::ast::internal::TSEnumDeclaration<'a>,
        mods: DeclMods,
    ) {
        let (inc, exc) = if e.r#const {
            (SymbolFlags::CONST_ENUM, SymbolFlags::CONST_ENUM_EXCLUDES)
        } else {
            (
                SymbolFlags::REGULAR_ENUM,
                SymbolFlags::REGULAR_ENUM_EXCLUDES,
            )
        };
        let d = self.decl_from_ident(&e.id, e.span, mods);
        let sym = self.declare_block_scoped(d, inc, exc);
        self.bind_enum_members(e.members, sym);
    }

    #[inline]
    fn bind_type_alias_statement(
        &mut self,
        t: &tsv_ts::ast::internal::TSTypeAliasDeclaration<'a>,
        mods: DeclMods,
    ) {
        // tsgo's `declareSymbolEx` adds a TS1369 "Did you mean
        // 'export type { T }'?" related info when a conflicting declaration
        // is `export type T;` — a type alias with a *missing* `= type`
        // (binder.go:260). That shape is deliberately unported: tsv's parser
        // rejects `export type T;` ("Expected '='"), so the declaration never
        // reaches this cascade. The sole corpus baseline exercising the hint
        // (`exportDeclaration_missingBraces.ts`) is therefore a tsv
        // parse-rejection, not a gradeable bind.
        let d = self.decl_from_ident(&t.id, t.span, mods);
        self.declare_block_scoped(d, SymbolFlags::TYPE_ALIAS, SymbolFlags::TYPE_ALIAS_EXCLUDES);
        self.bind_type_params_in_new_locals(t.type_parameters.as_ref());
    }

    #[inline]
    fn bind_import_equals_statement(
        &mut self,
        ie: &tsv_ts::ast::internal::TSImportEqualsDeclaration<'a>,
    ) {
        let d = self.decl_from_ident(
            &ie.id,
            ie.span,
            DeclMods {
                exported: ie.is_export,
                default: false,
            },
        );
        // An `import =` with an external reference or a plain entity name
        // is an alias either way for the family (locals unless exported).
        let _ = &ie.module_reference;
        self.declare_alias(d, ie.is_export);
    }

    #[inline]
    fn bind_export_named_statement(
        &mut self,
        e: &tsv_ts::ast::internal::ExportNamedDeclaration<'a>,
        skip_symbol: bool,
    ) {
        if let Some(inner) = e.declaration {
            self.visit_statement(
                inner,
                DeclMods {
                    exported: true,
                    default: false,
                },
                skip_symbol,
            );
        } else {
            for spec in e.specifiers {
                self.bind_export_specifier(spec);
            }
        }
    }

    #[inline]
    fn bind_for_statement(&mut self, s: &tsv_ts::ast::internal::ForStatement<'a>) {
        self.with_block_scope(|bd| {
            if let Some(init) = &s.init {
                match init {
                    ForInit::VariableDeclaration(decl) => bd.bind_var_declaration(decl),
                    ForInit::Expression(e) => bd.visit_expression(e),
                }
            }
            if let Some(t) = &s.test {
                bd.visit_expression(t);
            }
            if let Some(u) = &s.update {
                bd.visit_expression(u);
            }
            bd.visit_statement(s.body, DeclMods::default(), false);
        });
    }

    #[inline]
    fn bind_switch_statement(&mut self, s: &tsv_ts::ast::internal::SwitchStatement<'a>) {
        self.visit_expression(&s.discriminant);
        self.with_block_scope(|bd| {
            for case in s.cases {
                if let Some(t) = &case.test {
                    bd.visit_expression(t);
                }
                bd.bind_statement_list(case.consequent, false);
            }
        });
    }

    #[inline]
    fn bind_try_statement(&mut self, s: &tsv_ts::ast::internal::TryStatement<'a>) {
        self.with_block_scope(|bd| bd.bind_statement_list(s.block.body, true));
        if let Some(h) = &s.handler {
            // The catch clause is a block scope holding the (block-scoped)
            // parameter; its body is a *separate* nested block scope, so a
            // `const e` shadowing `catch(e)` is a check-time TS2492, not a
            // binder conflict (tsgo `bindVariableDeclarationOrBindingElement`
            // -> `IsBlockOrCatchScoped`).
            self.with_block_scope(|bd| {
                if let Some(param) = &h.param {
                    bd.bind_binding(
                        param,
                        SymbolFlags::BLOCK_SCOPED_VARIABLE,
                        SymbolFlags::BLOCK_SCOPED_VARIABLE_EXCLUDES,
                        true,
                        DeclMods::default(),
                        h.span,
                    );
                }
                bd.with_block_scope(|body| body.bind_statement_list(h.body.body, true));
            });
        }
        if let Some(f) = &s.finalizer {
            self.with_block_scope(|bd| bd.bind_statement_list(f.body, true));
        }
    }

    #[inline]
    fn bind_export_assignment_statement(
        &mut self,
        ea: &tsv_ts::ast::internal::TSExportAssignment<'a>,
    ) {
        // `export = x` — tsgo `bindExportAssignment` with `IsExportEquals`:
        // declared into `exports` under the `"export="` name with ALL
        // excludes (self-merge-only), so a second `export =` conflicts.
        if let Some(sym) = self.container.symbol {
            let name = self.atoms.export_equals();
            // The name node is the expression when it is a bare identifier
            // (tsgo `getNonAssignedNameOfDeclaration`), else the whole node.
            let error_span = match &ea.expression {
                Expression::Identifier(id) => id.name_span(),
                _ => ea.span,
            };
            let d = DeclInput {
                name,
                display: name,
                error_span,
                is_default_export: false,
                is_export_assignment_default: false,
                exported: true,
                node: self.node_id_of(ea, NodeKind::TSExportAssignment),
            };
            let table = self.exports_of(sym);
            self.declare_symbol(table, Some(sym), d, SymbolFlags::PROPERTY, SymbolFlags::ALL);
        }
        self.visit_expression(&ea.expression);
    }

    fn bind_var_declaration(&mut self, decl: &tsv_ts::ast::internal::VariableDeclaration<'a>) {
        let (includes, excludes, block_scoped) = var_flags(decl.kind);
        for d in decl.declarations {
            self.bind_binding(
                &d.id,
                includes,
                excludes,
                block_scoped,
                DeclMods::default(),
                decl.span,
            );
            if let Some(init) = &d.init {
                self.visit_expression(init);
            }
        }
    }

    fn bind_for_left(&mut self, left: &ForInOfLeft<'a>) {
        match left {
            ForInOfLeft::VariableDeclaration(decl) => self.bind_var_declaration(decl),
            ForInOfLeft::Pattern(_) => {}
        }
    }

    // --- export default ------------------------------------------------------

    fn bind_export_default(
        &mut self,
        e: &tsv_ts::ast::internal::ExportDefaultDeclaration<'a>,
        skip_symbol: bool,
    ) {
        let mods = DeclMods {
            exported: true,
            default: true,
        };
        match &e.declaration {
            ExportDefaultValue::Expression(expr) => {
                // tsgo `bindExportAssignment` (non-`export =`): excludes = ALL. An
                // entity-name expression (`export default foo`) is an **alias**
                // (`ExpressionIsAlias`) whose diagnostic points at the name; any
                // other expression (`export default 0`) is a `Property` pointing at
                // the whole `export default` node.
                if let Some(sym) = self.container.symbol {
                    let name = self.atoms.default_export();
                    let is_alias = matches!(
                        expr,
                        Expression::Identifier(_) | Expression::MemberExpression(_)
                    );
                    let flags = if is_alias {
                        SymbolFlags::ALIAS
                    } else {
                        SymbolFlags::PROPERTY
                    };
                    // The name node is the expression only when it is a bare
                    // identifier (tsgo `getNonAssignedNameOfDeclaration`); otherwise
                    // the whole `export default` node.
                    let error_span = match expr {
                        Expression::Identifier(id) => id.name_span(),
                        _ => e.span,
                    };
                    let d = DeclInput {
                        name,
                        display: name,
                        error_span,
                        is_default_export: false,
                        is_export_assignment_default: true,
                        exported: false,
                        node: self.node_id_of(e, NodeKind::ExportDefaultDeclaration),
                    };
                    let table = self.exports_of(sym);
                    self.declare_symbol(table, Some(sym), d, flags, SymbolFlags::ALL);
                }
                self.visit_expression(expr);
            }
            ExportDefaultValue::FunctionDeclaration(f) => {
                if !skip_symbol {
                    self.bind_default_function(f.id.as_ref(), e.span, mods);
                }
                self.with_function_scope(f.type_parameters.as_ref(), |b| {
                    b.bind_params(f.params);
                    b.bind_statement_list(f.body.body, true);
                });
            }
            ExportDefaultValue::TSDeclareFunction(f) => {
                if !skip_symbol {
                    self.bind_default_function(Some(&f.id), e.span, mods);
                }
                self.with_function_scope(f.type_parameters.as_ref(), |b| b.bind_params(f.params));
            }
            ExportDefaultValue::ClassDeclaration(c) => {
                let d = self.default_decl(c.id.as_ref(), e.span);
                let sym = self.container.symbol.map(|cs| {
                    let table = self.exports_of(cs);
                    self.declare_symbol(
                        table,
                        Some(cs),
                        d,
                        SymbolFlags::CLASS,
                        SymbolFlags::CLASS_EXCLUDES,
                    )
                });
                self.bind_class_body(&c.body, sym, c.type_parameters.as_ref());
            }
            ExportDefaultValue::TSInterfaceDeclaration(i) => {
                let d = self.default_decl(Some(&i.id), e.span);
                if let Some(cs) = self.container.symbol {
                    let table = self.exports_of(cs);
                    self.declare_symbol(
                        table,
                        Some(cs),
                        d,
                        SymbolFlags::INTERFACE,
                        SymbolFlags::INTERFACE_EXCLUDES,
                    );
                }
                self.bind_interface_body_symbol_less(&i.body, i.type_parameters.as_ref());
            }
        }
    }

    fn default_decl(&mut self, id: Option<&Identifier<'a>>, node_span: Span) -> DeclInput {
        let display = match id {
            Some(i) => {
                let name = i.name(self.source);
                self.atoms.intern(name)
            }
            None => self.atoms.default_export(),
        };
        DeclInput {
            name: self.atoms.default_export(),
            display,
            error_span: id.map_or(node_span, Identifier::name_span),
            is_default_export: true,
            is_export_assignment_default: false,
            exported: false,
            node: NodeId::FIRST,
        }
    }

    fn bind_default_function(
        &mut self,
        id: Option<&Identifier<'a>>,
        node_span: Span,
        _mods: DeclMods,
    ) {
        if let Some(cs) = self.container.symbol {
            let d = self.default_decl(id, node_span);
            let table = self.exports_of(cs);
            self.declare_symbol(
                table,
                Some(cs),
                d,
                SymbolFlags::FUNCTION,
                SymbolFlags::FUNCTION_EXCLUDES,
            );
        }
    }

    // --- function names --------------------------------------------------------

    fn bind_function_name(&mut self, id: &Identifier<'a>, node_span: Span, mods: DeclMods) {
        let d = self.decl_from_ident(id, node_span, mods);
        self.declare_block_scoped(d, SymbolFlags::FUNCTION, SymbolFlags::FUNCTION_EXCLUDES);
    }

    // --- params + bindings ---------------------------------------------------

    pub(super) fn bind_params(&mut self, params: &[Expression<'a>]) {
        for param in params {
            self.bind_param(param);
        }
    }

    pub(super) fn bind_param(&mut self, param: &Expression<'a>) {
        match param {
            Expression::TSParameterProperty(pp) => {
                // The inner parameter binds as a parameter; a property-parameter
                // also declares a class member (handled where the constructor's
                // owning class scope is live — the constructor scope's parent).
                self.bind_param(pp.parameter);
            }
            _ => self.bind_binding(
                param,
                SymbolFlags::FUNCTION_SCOPED_VARIABLE,
                SymbolFlags::PARAMETER_EXCLUDES,
                false,
                DeclMods::default(),
                param_span(param),
            ),
        }
    }

    /// Bind a binding target: an identifier leaf routes through the given flags;
    /// object/array patterns recurse; assignment patterns and rest unwrap.
    fn bind_binding(
        &mut self,
        target: &Expression<'a>,
        includes: SymbolFlags,
        excludes: SymbolFlags,
        block_scoped: bool,
        mods: DeclMods,
        node_span: Span,
    ) {
        match target {
            Expression::Identifier(id) => {
                let d = self.decl_from_ident(id, node_span, mods);
                if block_scoped {
                    self.declare_block_scoped(d, includes, excludes);
                } else {
                    self.declare_in_container(d, includes, excludes);
                }
                // The binder's one type-annotation entry point: a typed binding
                // (`var a: { … }`) descends into its annotation so a type literal's
                // members bind (its method-signature params conflict, its duplicate
                // members silent-merge). Narrow by design — an incomplete traversal
                // only leaves family instances missing, never fabricates a conflict.
                if let Some(ann) = id.type_annotation() {
                    self.bind_type_annotation(ann);
                }
            }
            Expression::ObjectPattern(p) => {
                for prop in p.properties {
                    match prop {
                        ObjectPatternProperty::Property(pr) => {
                            self.bind_binding(
                                &pr.value,
                                includes,
                                excludes,
                                block_scoped,
                                mods,
                                pr.span,
                            );
                        }
                        ObjectPatternProperty::RestElement(r) => {
                            self.bind_binding(
                                r.argument,
                                includes,
                                excludes,
                                block_scoped,
                                mods,
                                r.span,
                            );
                        }
                    }
                }
            }
            Expression::ArrayPattern(p) => {
                for el in p.elements.iter().flatten() {
                    self.bind_binding(el, includes, excludes, block_scoped, mods, el_span(el));
                }
            }
            Expression::AssignmentPattern(a) => {
                self.bind_binding(a.left, includes, excludes, block_scoped, mods, node_span);
                self.visit_expression(a.right);
            }
            Expression::RestElement(r) => {
                self.bind_binding(r.argument, includes, excludes, block_scoped, mods, r.span);
            }
            _ => {}
        }
    }

    pub(super) fn decl_from_ident(
        &mut self,
        id: &Identifier<'a>,
        _node_span: Span,
        mods: DeclMods,
    ) -> DeclInput {
        let name = self.ident_atom(id);
        DeclInput {
            name,
            display: name,
            error_span: id.name_span(),
            is_default_export: mods.default,
            is_export_assignment_default: false,
            exported: mods.exported,
            node: self.node_id_of(id, NodeKind::Identifier),
        }
    }

    // --- modules ---------------------------------------------------------------

    fn bind_module(&mut self, m: &tsv_ts::ast::internal::TSModuleDeclaration<'a>, mods: DeclMods) {
        // The module's own symbol (name = identifier, or `"name"` for ambient).
        let (name, display, span) = match &m.id {
            TSModuleName::Identifier(id) => {
                let a = self.ident_atom(id);
                (a, a, id.name_span())
            }
            TSModuleName::Literal(lit) => {
                let raw = lit.span.extract(self.source);
                let key = self.atoms.intern(raw);
                (key, key, lit.span)
            }
        };
        let d = DeclInput {
            name,
            display,
            error_span: span,
            is_default_export: mods.default,
            is_export_assignment_default: false,
            exported: mods.exported,
            node: self.node_id_of(m, NodeKind::TSModuleDeclaration),
        };
        // Instantiation state (tsgo `GetModuleInstanceState`): a namespace of only
        // types binds as the inert `NamespaceModule`, so it never conflicts with a
        // `var`/`let`/`type` of the same name; one with value content is `ValueModule`.
        let (inc, exc) = if module_instantiated(m) {
            (
                SymbolFlags::VALUE_MODULE,
                SymbolFlags::VALUE_MODULE_EXCLUDES,
            )
        } else {
            (
                SymbolFlags::NAMESPACE_MODULE,
                SymbolFlags::NAMESPACE_MODULE_EXCLUDES,
            )
        };
        let sym = self.declare_block_scoped(d, inc, exc);

        // Record cross-declaration-space augmentations for the merge phase — only
        // top-level ones (container still the source file). `declare global {}` is
        // a global-scope augmentation (its exports merge into globals);
        // `declare module "X"` in an external module is a module augmentation
        // (tsgo `IsModuleAugmentationExternal`, the `KindSourceFile` arm).
        if self.container.kind == ContainerKind::SourceFile {
            if m.global {
                self.global_aug_symbols.push(sym);
            } else if self.is_external
                && let TSModuleName::Literal(lit) = &m.id
            {
                let unquoted = self.string_atom(lit);
                self.module_augs.push((unquoted, lit.span));
            }
        }

        let saved = (self.container, self.block_scope);
        let locals = self.new_table();
        let scope = Scope {
            kind: ContainerKind::Module,
            symbol: Some(sym),
            locals: Some(locals),
            is_external_module: false,
            is_export_context: m.declare,
        };
        self.container = scope;
        self.block_scope = scope;
        self.exports_of(sym);
        match &m.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                self.bind_statement_list(block.body, true);
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                // A dotted-namespace continuation: `namespace X.Y.Z {}` parses to a
                // nested `TSModuleDeclaration` chain, and this body variant is
                // constructed only by that dot path. tsgo's parser synthesizes an
                // implicit `export` modifier (`NodeFlagsReparsed`) on every
                // dot-continuation segment, so the intermediate segments land in the
                // enclosing namespace's persistent *exports* table — the same table an
                // explicit `export namespace Y {}` routes to — letting the dotted and
                // explicit-nested forms merge (and their members conflict) instead of
                // splitting into fresh per-instance locals that never meet.
                //
                // tsgo: internal/parser/parser.go parseModuleOrNamespaceDeclaration
                self.bind_module(
                    nested,
                    DeclMods {
                        exported: true,
                        default: false,
                    },
                );
            }
            None => {}
        }
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    // --- imports / exports (aliases) -----------------------------------------

    fn bind_import_specifier(&mut self, spec: &ImportSpecifier<'a>) {
        let id = match spec {
            ImportSpecifier::Default(d) => &d.local,
            ImportSpecifier::Named(n) => &n.local,
            ImportSpecifier::Namespace(n) => &n.local,
        };
        let d = self.decl_from_ident(id, id.span, DeclMods::default());
        self.declare_alias(d, false);
    }

    fn bind_export_specifier(&mut self, spec: &ExportSpecifier<'a>) {
        // An export specifier's *exported* name is the table key in `exports`.
        let (name, span) = self.module_export_name_atom(&spec.exported);
        let is_default = matches!(&spec.exported, ModuleExportName::Identifier(id)
            if id.name(self.source) == "default");
        let d = DeclInput {
            name,
            display: name,
            error_span: span,
            is_default_export: is_default,
            is_export_assignment_default: false,
            exported: false,
            node: NodeId::FIRST,
        };
        self.declare_alias(d, true);
    }
}

/// A [`SymbolFlags`] triple for a variable declaration kind: `(includes,
/// excludes, block_scoped)`. `block_scoped` selects `bindBlockScopedDeclaration`
/// (block-scope routing) over `declareSymbolAndAddToSymbolTable` (container).
fn var_flags(
    kind: tsv_ts::ast::internal::VariableDeclarationKind,
) -> (SymbolFlags, SymbolFlags, bool) {
    use tsv_ts::ast::internal::VariableDeclarationKind as K;
    match kind {
        // `var` is function-scoped (routes through the container).
        K::Var => (
            SymbolFlags::FUNCTION_SCOPED_VARIABLE,
            SymbolFlags::FUNCTION_SCOPED_VARIABLE_EXCLUDES,
            false,
        ),
        // `let` / `const` / `using` / `await using` are block-scoped.
        K::Let | K::Const | K::Using | K::AwaitUsing => (
            SymbolFlags::BLOCK_SCOPED_VARIABLE,
            SymbolFlags::BLOCK_SCOPED_VARIABLE_EXCLUDES,
            true,
        ),
    }
}

/// The span a bare parameter expression points a diagnostic at.
fn param_span(param: &Expression<'_>) -> Span {
    match param {
        Expression::Identifier(id) => id.name_span(),
        _ => param.span(),
    }
}

/// The span an array-pattern element points a diagnostic at.
fn el_span(el: &Expression<'_>) -> Span {
    match el {
        Expression::Identifier(id) => id.name_span(),
        _ => el.span(),
    }
}

/// Whether a namespace/module is instantiated (a `ValueModule`) — a faithful-
/// enough port of tsgo's `getModuleInstanceState`. A module is *non*-instantiated
/// (an inert `NamespaceModule`) only when its whole body is types: interfaces,
/// type aliases, non-exported imports, uninstantiated nested namespaces, and
/// specifier-only named exports (approximated as non-instantiated). Any value
/// content — a var/function/class/enum, an `export =`/`export default`, an
/// expression — makes it instantiated.
fn module_instantiated(m: &tsv_ts::ast::internal::TSModuleDeclaration<'_>) -> bool {
    match &m.body {
        None => true,
        Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => module_instantiated(nested),
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
            !block.body.iter().all(statement_is_non_instantiated)
        }
    }
}

/// Whether a module-body statement contributes no value (tsgo
/// `getModuleInstanceStateWorker`).
fn statement_is_non_instantiated(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => true,
        Statement::ImportDeclaration(_) => true,
        Statement::TSImportEqualsDeclaration(ie) => !ie.is_export,
        Statement::TSModuleDeclaration(nested) => !module_instantiated(nested),
        // `export interface`/`export type` wrap a non-instantiated declaration;
        // specifier-only named exports are approximated non-instantiated.
        Statement::ExportNamedDeclaration(e) => match e.declaration {
            Some(inner) => statement_is_non_instantiated(inner),
            None => true,
        },
        _ => false,
    }
}
