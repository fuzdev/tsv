//! The class/interface/type-literal/enum member and type descents —
//! `bind_class_body`/`bind_class_member`/`bind_constructor_params`, the
//! interface/type-literal member bind (`bind_interface_body`/
//! `bind_type_element`/`declare_type_member`), enum members, and type
//! parameters. Contributes its own `impl SymbolBinder` block; the struct and
//! the scope helpers live in the parent module. Purely a locality split — no
//! behavior distinction.

use super::super::symbols::{SymbolFlags, SymbolId};
use super::{ContainerKind, DeclInput, DeclMods, Scope, SymbolBinder};
use crate::ids::NodeId;
use tsv_ts::ast::internal::{
    ClassBody, ClassMember, Expression, MethodKind, Statement, TSEnumMemberId, TSInterfaceBody,
    TSType, TSTypeAnnotation, TSTypeElement, TSTypeLiteral, TSTypeParameterDeclaration,
};

impl<'a> SymbolBinder<'a> {
    // --- classes -------------------------------------------------------------

    // tsgo: internal/binder/binder.go bindClassLikeDeclaration (the static-`prototype` clash, :971)
    pub(super) fn bind_class_body(
        &mut self,
        body: &ClassBody<'a>,
        class_symbol: Option<SymbolId>,
        type_params: Option<&TSTypeParameterDeclaration<'a>>,
    ) {
        let Some(class_symbol) = class_symbol else {
            // Anonymous / skipped class: still descend member values for nested
            // bindings, but no member tables to conflict in.
            self.descend_class_values(body);
            return;
        };
        // The static-`prototype` clash (checker.go:971): a pre-seeded export.
        let proto = self.atoms.intern("prototype");
        let exports = self.exports_of(class_symbol);
        if let Some(existing) = self.tables[exports.index()].get(&proto).copied()
            && let Some(pdecl) = self.symbols[existing.index()].decls.first().copied()
        {
            let name = self.atoms.resolve(pdecl.display).to_string();
            let diag = self.make_diag(pdecl.error_span, 2300, Some(&name));
            self.diagnostics.push(diag);
        }
        let proto_sym = self.new_symbol(SymbolFlags::PROPERTY.union(SymbolFlags::PROTOTYPE), proto);
        self.symbols[proto_sym.index()].parent = Some(class_symbol);
        self.tables[exports.index()].insert(proto, proto_sym);

        let saved = (self.container, self.block_scope);
        let scope = Scope {
            kind: ContainerKind::Class,
            symbol: Some(class_symbol),
            locals: None,
            is_external_module: false,
            is_export_context: false,
        };
        self.container = scope;
        self.block_scope = scope;
        self.bind_type_params(type_params);
        for member in body.body {
            self.bind_class_member(member, class_symbol);
        }
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    fn bind_class_member(&mut self, member: &ClassMember<'a>, class_symbol: SymbolId) {
        match member {
            ClassMember::MethodDefinition(m) => {
                let is_static = m.is_static;
                let (inc, exc) = match m.kind {
                    MethodKind::Constructor => (SymbolFlags::CONSTRUCTOR, SymbolFlags::NONE),
                    MethodKind::Get => (
                        SymbolFlags::GET_ACCESSOR,
                        SymbolFlags::GET_ACCESSOR_EXCLUDES,
                    ),
                    MethodKind::Set => (
                        SymbolFlags::SET_ACCESSOR,
                        SymbolFlags::SET_ACCESSOR_EXCLUDES,
                    ),
                    MethodKind::Method => {
                        let opt = if m.optional {
                            SymbolFlags::OPTIONAL
                        } else {
                            SymbolFlags::NONE
                        };
                        (SymbolFlags::METHOD.union(opt), SymbolFlags::METHOD_EXCLUDES)
                    }
                };
                if let MethodKind::Constructor = m.kind {
                    let d = DeclInput {
                        name: self.atoms.intern("__constructor"),
                        display: self.atoms.intern("__constructor"),
                        error_span: m.span,
                        is_default_export: false,
                        is_export_assignment_default: false,
                        exported: false,
                        node: NodeId::FIRST,
                    };
                    self.declare_class_member(d, inc, exc, is_static);
                    // Bind constructor params (incl. parameter properties -> class members).
                    self.with_function_scope(m.value.type_parameters.as_ref(), |b| {
                        b.bind_constructor_params(m.value.params, class_symbol);
                        b.bind_statement_list(method_body(&m.value), true);
                    });
                } else if let Some(key) =
                    self.resolve_member_key(&m.key, m.computed, Some(class_symbol))
                {
                    let d = DeclInput {
                        name: key.key,
                        display: key.display,
                        error_span: key.span,
                        is_default_export: false,
                        is_export_assignment_default: false,
                        exported: false,
                        node: NodeId::FIRST,
                    };
                    self.declare_class_member(d, inc, exc, is_static);
                    self.with_function_scope(m.value.type_parameters.as_ref(), |b| {
                        b.bind_params(m.value.params);
                        b.bind_statement_list(method_body(&m.value), true);
                    });
                } else {
                    // Dynamic computed key: anonymous member, no conflict; still
                    // descend the value for nested bindings.
                    self.with_function_scope(m.value.type_parameters.as_ref(), |b| {
                        b.bind_params(m.value.params);
                        b.bind_statement_list(method_body(&m.value), true);
                    });
                }
            }
            ClassMember::PropertyDefinition(p) => {
                let (inc, exc) = if p.accessor {
                    (SymbolFlags::ACCESSOR, SymbolFlags::ACCESSOR_EXCLUDES)
                } else {
                    let opt = if p.modifier == tsv_ts::ast::internal::PropertyModifier::Optional {
                        SymbolFlags::OPTIONAL
                    } else {
                        SymbolFlags::NONE
                    };
                    (
                        SymbolFlags::PROPERTY.union(opt),
                        SymbolFlags::PROPERTY_EXCLUDES,
                    )
                };
                if let Some(key) = self.resolve_member_key(&p.key, p.computed, Some(class_symbol)) {
                    let d = DeclInput {
                        name: key.key,
                        display: key.display,
                        error_span: key.span,
                        is_default_export: false,
                        is_export_assignment_default: false,
                        exported: false,
                        node: NodeId::FIRST,
                    };
                    self.declare_class_member(d, inc, exc, p.is_static);
                }
                if let Some(v) = &p.value {
                    self.visit_expression(v);
                }
            }
            ClassMember::StaticBlock(s) => {
                self.with_block_scope(|b| b.bind_statement_list(s.body, true));
            }
            ClassMember::IndexSignature(_) => {}
        }
    }

    fn bind_constructor_params(&mut self, params: &[Expression<'a>], class_symbol: SymbolId) {
        for param in params {
            match param {
                Expression::TSParameterProperty(pp) => {
                    // Bind as a parameter (in the constructor scope)...
                    self.bind_param(pp.parameter);
                    // ...and as a class instance member (tsgo bindParameter).
                    if let Expression::Identifier(id) = ident_of_param(pp.parameter) {
                        let opt = if id.optional {
                            SymbolFlags::OPTIONAL
                        } else {
                            SymbolFlags::NONE
                        };
                        let d = self.decl_from_ident(id, pp.span, DeclMods::default());
                        let table = self.members_of(class_symbol);
                        self.declare_symbol(
                            table,
                            Some(class_symbol),
                            d,
                            SymbolFlags::PROPERTY.union(opt),
                            SymbolFlags::PROPERTY_EXCLUDES,
                        );
                    }
                }
                _ => self.bind_param(param),
            }
        }
    }

    fn descend_class_values(&mut self, body: &ClassBody<'a>) {
        for member in body.body {
            match member {
                ClassMember::MethodDefinition(m) => {
                    self.with_function_scope(m.value.type_parameters.as_ref(), |b| {
                        b.bind_params(m.value.params);
                        b.bind_statement_list(method_body(&m.value), true);
                    });
                }
                ClassMember::PropertyDefinition(p) => {
                    if let Some(v) = &p.value {
                        self.visit_expression(v);
                    }
                }
                ClassMember::StaticBlock(s) => {
                    self.with_block_scope(|b| b.bind_statement_list(s.body, true));
                }
                ClassMember::IndexSignature(_) => {}
            }
        }
    }

    // --- interfaces / enums / modules ---------------------------------------

    pub(super) fn bind_interface_body(
        &mut self,
        body: &TSInterfaceBody<'a>,
        interface_symbol: SymbolId,
        type_params: Option<&TSTypeParameterDeclaration<'a>>,
    ) {
        let saved = (self.container, self.block_scope);
        let scope = Scope {
            kind: ContainerKind::Interface,
            symbol: Some(interface_symbol),
            locals: None,
            is_external_module: false,
            is_export_context: false,
        };
        self.container = scope;
        self.block_scope = scope;
        self.bind_type_params(type_params);
        for member in body.body {
            self.bind_type_element(member);
        }
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    pub(super) fn bind_interface_body_symbol_less(
        &self,
        _body: &TSInterfaceBody<'a>,
        _type_params: Option<&TSTypeParameterDeclaration<'a>>,
    ) {
        // `export default interface` with no container symbol: nothing to bind.
    }

    // --- type annotations ----------------------------------------------------

    /// Descend a binding's type annotation.
    pub(super) fn bind_type_annotation(&mut self, ann: &TSTypeAnnotation<'a>) {
        self.bind_type(ann.type_annotation);
    }

    /// Bind the only type shape whose members reach the family cascade — a type
    /// literal. Every other variant is a deliberate no-op: a narrower-than-tsgo
    /// traversal can only leave things missing, never fabricate an extra.
    //
    // TODO: this descent is both shallow (direct `TypeLiteral` only) and reached
    // from only a few sites — it never runs on a type-alias RHS, heritage type
    // arguments, a nested class expression, or a union/array-wrapped nested literal,
    // so a method-vs-property conflict in a type literal there is missed at bind
    // (miss-only; extra=0 holds; unexercised by the corpus). The coherent fix is one
    // general bind-side type descent mirroring the check pass's `CheckWalk::visit_type`,
    // wired into those sites together — not patched per-position.
    fn bind_type(&mut self, ty: &TSType<'a>) {
        if let TSType::TypeLiteral(tl) = ty {
            self.bind_type_literal_body(tl);
        }
    }

    /// Bind a type literal's members under an anonymous `TypeLiteral` symbol —
    /// mirrors [`Self::bind_interface_body`]'s member scope, so a method
    /// signature's duplicate params conflict and its duplicate members
    /// silent-merge (the property/member family is check-time, out of this bind).
    ///
    /// tsgo: internal/binder/binder.go bindAnonymousDeclaration
    ///       (SymbolFlagsTypeLiteral, InternalSymbolNameType)
    fn bind_type_literal_body(&mut self, tl: &TSTypeLiteral<'a>) {
        let name = self.atoms.intern("__type");
        let sym = self.new_symbol(SymbolFlags::TYPE_LITERAL, name);
        let saved = (self.container, self.block_scope);
        let scope = Scope {
            kind: ContainerKind::Interface,
            symbol: Some(sym),
            locals: None,
            is_external_module: false,
            is_export_context: false,
        };
        self.container = scope;
        self.block_scope = scope;
        for member in tl.members {
            self.bind_type_element(member);
        }
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    fn bind_type_element(&mut self, element: &TSTypeElement<'a>) {
        match element {
            TSTypeElement::PropertySignature(p) => {
                self.declare_type_member(
                    &p.key,
                    p.computed,
                    SymbolFlags::PROPERTY,
                    SymbolFlags::PROPERTY_EXCLUDES,
                );
                // Descend the member's own type — a nested type literal's members bind
                // (its method-vs-property conflict is bind-time, so it is missed unless
                // this recurses). tsgo binds nested type-literal members; a
                // property/property nested dup is caught separately by the check pass at
                // any depth, so this closes only the bind-time family gap.
                if let Some(ann) = &p.type_annotation {
                    self.bind_type_annotation(ann);
                }
            }
            TSTypeElement::MethodSignature(m) => {
                self.declare_type_member(
                    &m.key,
                    m.computed,
                    SymbolFlags::METHOD,
                    SymbolFlags::METHOD_EXCLUDES,
                );
                // A method signature is itself a `HasLocals` function-like container
                // (tsgo `GetContainerFlags` KindMethodSignature), so its parameters
                // bind into a fresh function scope — duplicate params within one
                // signature conflict (TS2300) independently of the enclosing member
                // table.
                self.with_function_scope(m.type_parameters.as_ref(), |b| b.bind_params(m.params));
                // The return type descends for the same nested-type-literal reason as a
                // property signature (param type literals already descend via
                // `bind_binding`).
                if let Some(ann) = &m.return_type {
                    self.bind_type_annotation(ann);
                }
            }
            // Call/construct signatures are anonymous in the member table: tsgo binds
            // them `SymbolFlagsSignature` with no excludes, so they never conflict —
            // tsv skips that inert declaration and binds only their parameters, into
            // their own function scope. Index signatures have a single parameter that
            // cannot self-conflict, so nothing binds.
            // tsgo: internal/binder/binder.go GetContainerFlags (Kind{Call,Construct}Signature)
            TSTypeElement::CallSignature(c) => {
                self.with_function_scope(c.type_parameters.as_ref(), |b| b.bind_params(c.params));
                if let Some(ann) = &c.return_type {
                    self.bind_type_annotation(ann);
                }
            }
            TSTypeElement::ConstructSignature(c) => {
                self.with_function_scope(c.type_parameters.as_ref(), |b| b.bind_params(c.params));
                if let Some(ann) = &c.return_type {
                    self.bind_type_annotation(ann);
                }
            }
            TSTypeElement::IndexSignature(_) => {}
        }
    }

    /// Declare a type-literal / interface member (property or method signature)
    /// keyed by its name into the current member container.
    fn declare_type_member(
        &mut self,
        key_expr: &Expression<'a>,
        computed: bool,
        inc: SymbolFlags,
        exc: SymbolFlags,
    ) {
        if let Some(key) = self.resolve_member_key(key_expr, computed, None) {
            let d = DeclInput {
                name: key.key,
                display: key.display,
                error_span: key.span,
                is_default_export: false,
                is_export_assignment_default: false,
                exported: false,
                node: NodeId::FIRST,
            };
            self.declare_in_container(d, inc, exc);
        }
    }

    pub(super) fn bind_enum_members(
        &mut self,
        members: &[tsv_ts::ast::internal::TSEnumMember<'a>],
        enum_symbol: SymbolId,
    ) {
        let saved = (self.container, self.block_scope);
        let scope = Scope {
            kind: ContainerKind::Enum,
            symbol: Some(enum_symbol),
            locals: None,
            is_external_module: false,
            is_export_context: false,
        };
        self.container = scope;
        self.block_scope = scope;
        for member in members {
            let (key, span) = match &member.id {
                TSEnumMemberId::Identifier(id) => (self.ident_atom(id), id.name_span()),
                TSEnumMemberId::String(lit) => (self.string_atom(lit), lit.span),
            };
            let d = DeclInput {
                name: key,
                display: key,
                error_span: span,
                is_default_export: false,
                is_export_assignment_default: false,
                exported: false,
                node: NodeId::FIRST,
            };
            self.declare_in_container(
                d,
                SymbolFlags::ENUM_MEMBER,
                SymbolFlags::ENUM_MEMBER_EXCLUDES,
            );
            if let Some(init) = &member.initializer {
                self.visit_expression(init);
            }
        }
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    // --- type parameters -----------------------------------------------------

    pub(super) fn bind_type_params(
        &mut self,
        type_params: Option<&TSTypeParameterDeclaration<'a>>,
    ) {
        if let Some(tp) = type_params {
            for p in tp.params {
                let d = self.decl_from_ident(&p.name, p.span, DeclMods::default());
                self.declare_in_container(
                    d,
                    SymbolFlags::TYPE_PARAMETER,
                    SymbolFlags::TYPE_PARAMETER_EXCLUDES,
                );
            }
        }
    }

    pub(super) fn bind_type_params_in_new_locals(
        &mut self,
        type_params: Option<&TSTypeParameterDeclaration<'a>>,
    ) {
        if type_params.is_none() {
            return;
        }
        self.with_function_scope(type_params, |_| {});
    }
}

/// The binding identifier of a parameter, unwrapping a default (`AssignmentPattern`).
fn ident_of_param<'b, 'a>(param: &'b Expression<'a>) -> &'b Expression<'a> {
    match param {
        Expression::AssignmentPattern(a) => a.left,
        other => other,
    }
}

/// A method's body statements (a `FunctionExpression`'s block body).
fn method_body<'a>(f: &tsv_ts::ast::internal::FunctionExpression<'a>) -> &'a [Statement<'a>] {
    f.body.body
}
