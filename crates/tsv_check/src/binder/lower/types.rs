//! The type-shaped visitor methods — `visit_type` and everything a type
//! position (annotations, type arguments/parameters, entity names, interface /
//! type-literal members) descends into. `visit_index_signature` lives here too:
//! one `TSIndexSignature` node serves both a class member and a type-element
//! position, and its shape (parameters + an optional type annotation) is purely
//! type-flavored.

use super::super::*;
use tsv_ts::ast::{
    TSEntityName, TSImportType, TSIndexSignature, TSLiteralType, TSMappedTypeParameter,
    TSQualifiedName, TSType, TSTypeAnnotation, TSTypeElement, TSTypeParameter,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypeQueryExprName, TyId,
};

impl<'a> SoaWalk<'a> {
    pub(super) fn visit_index_signature(&mut self, i: &'a TSIndexSignature, parent: NodeId) {
        let id = self.add(NodeKind::TSIndexSignature, i.span, parent, addr_of(i));
        for p in self.view.index_params(i.parameters) {
            self.visit_identifier(p, id);
        }
        self.visit_type_annotation_opt(i.type_annotation.as_ref(), id);
        self.close(id);
    }

    /// A `TSTypeAnnotation` (`: T`) is a transparent wrapper — not idd; the walk
    /// descends straight into the inner `TSType`, which is the node.
    pub(super) fn visit_type_annotation(&mut self, ann: &'a TSTypeAnnotation, parent: NodeId) {
        self.visit_ty(ann.type_annotation, parent);
    }

    pub(super) fn visit_type_annotation_opt(
        &mut self,
        ann: Option<&'a TSTypeAnnotation>,
        parent: NodeId,
    ) {
        if let Some(a) = ann {
            self.visit_type_annotation(a, parent);
        }
    }

    pub(super) fn visit_type_args(
        &mut self,
        args: &'a TSTypeParameterInstantiation,
        parent: NodeId,
    ) {
        let id = self.add(
            NodeKind::TSTypeParameterInstantiation,
            args.span,
            parent,
            addr_of(args),
        );
        for &t in self.view.ty_list(args.params) {
            self.visit_ty(t, id);
        }
        self.close(id);
    }

    pub(super) fn visit_type_params(
        &mut self,
        params: Option<&'a TSTypeParameterDeclaration>,
        parent: NodeId,
    ) {
        if let Some(decl) = params {
            let id = self.add(
                NodeKind::TSTypeParameterDeclaration,
                decl.span,
                parent,
                addr_of(decl),
            );
            for p in self.view.type_params(decl.params) {
                self.visit_type_parameter(p, id);
            }
            self.close(id);
        }
    }

    fn visit_type_parameter(&mut self, p: &'a TSTypeParameter, parent: NodeId) {
        let id = self.add(NodeKind::TSTypeParameter, p.span, parent, addr_of(p));
        self.visit_identifier(&p.name, id);
        if let Some(c) = p.constraint {
            self.visit_ty(c, id);
        }
        if let Some(d) = p.default {
            self.visit_ty(d, id);
        }
        self.close(id);
    }

    fn visit_mapped_type_parameter(&mut self, mtp: &'a TSMappedTypeParameter, parent: NodeId) {
        // The `name` is a bare `IdentName` (no child identifier node); the mapped
        // type parameter's own span covers the name token.
        let id = self.add(
            NodeKind::TSMappedTypeParameter,
            mtp.span,
            parent,
            addr_of(mtp),
        );
        self.visit_ty(mtp.constraint, id);
        self.close(id);
    }

    pub(super) fn visit_entity_name(&mut self, name: &'a TSEntityName, parent: NodeId) {
        match name {
            TSEntityName::Identifier(id) => self.visit_identifier(id, parent),
            &TSEntityName::QualifiedName(_, qn) => {
                self.visit_qualified_name(self.view.qual_name(qn), parent);
            }
        }
    }

    fn visit_qualified_name(&mut self, qn: &'a TSQualifiedName, parent: NodeId) {
        let id = self.add(NodeKind::TSQualifiedName, qn.span, parent, addr_of(qn));
        self.visit_entity_name(&qn.left, id);
        self.visit_identifier(&qn.right, id);
        self.close(id);
    }

    fn visit_import_type(&mut self, i: &'a TSImportType, parent: NodeId) {
        let id = self.add(NodeKind::TSImportType, i.span, parent, addr_of(i));
        self.leaf(NodeKind::Literal, i.argument.span, addr_of(&i.argument), id);
        if let Some(o) = i.options {
            self.visit_expr(o, id);
        }
        if let Some(q) = &i.qualifier {
            self.visit_entity_name(q, id);
        }
        if let Some(ta) = &i.type_arguments {
            self.visit_type_args(ta, id);
        }
        self.close(id);
    }

    pub(super) fn visit_type_elements(&mut self, members: Run<TSTypeElement>, parent: NodeId) {
        for member in self.view.type_members(members) {
            self.visit_type_element(member, parent);
        }
    }

    fn visit_type_element(&mut self, member: &'a TSTypeElement, parent: NodeId) {
        match member {
            TSTypeElement::PropertySignature(p) => {
                let id = self.add(NodeKind::TSPropertySignature, p.span, parent, addr_of(p));
                self.visit_expr(p.key, id);
                self.visit_type_annotation_opt(p.type_annotation.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::MethodSignature(m) => {
                let id = self.add(NodeKind::TSMethodSignature, m.span, parent, addr_of(m));
                self.visit_expr(m.key, id);
                self.visit_type_params(m.type_parameters.as_ref(), id);
                self.visit_params(m.params, id);
                self.visit_type_annotation_opt(m.return_type.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::CallSignature(c) => {
                let id = self.add(
                    NodeKind::TSCallSignatureDeclaration,
                    c.span,
                    parent,
                    addr_of(c),
                );
                self.visit_type_params(c.type_parameters.as_ref(), id);
                self.visit_params(c.params, id);
                self.visit_type_annotation_opt(c.return_type.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::ConstructSignature(c) => {
                let id = self.add(
                    NodeKind::TSConstructSignatureDeclaration,
                    c.span,
                    parent,
                    addr_of(c),
                );
                self.visit_type_params(c.type_parameters.as_ref(), id);
                self.visit_params(c.params, id);
                self.visit_type_annotation_opt(c.return_type.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::IndexSignature(i) => self.visit_index_signature(i, parent),
        }
    }

    /// Visit the type `id` names, read in place from its store.
    pub(super) fn visit_ty(&mut self, id: TyId, parent: NodeId) {
        self.visit_type(self.view.ty(id), parent);
    }

    pub(super) fn visit_type(&mut self, ty: &'a TSType, parent: NodeId) {
        let view = self.view;
        match ty {
            TSType::Keyword(kw) => self.leaf(NodeKind::TSKeywordType, kw.span, addr_of(kw), parent),
            TSType::ThisType(t) => self.leaf(NodeKind::TSThisType, t.span, addr_of(t), parent),
            TSType::Literal(lit) => self.visit_literal_type(lit, parent),
            TSType::Array(a) => {
                let id = self.add(NodeKind::TSArrayType, a.span, parent, addr_of(a));
                self.visit_ty(a.element_type, id);
                self.close(id);
            }
            TSType::Union(u) => {
                let id = self.add(NodeKind::TSUnionType, u.span, parent, addr_of(u));
                for &t in self.view.ty_list(u.types) {
                    self.visit_ty(t, id);
                }
                self.close(id);
            }
            TSType::Intersection(i) => {
                let id = self.add(NodeKind::TSIntersectionType, i.span, parent, addr_of(i));
                for &t in self.view.ty_list(i.types) {
                    self.visit_ty(t, id);
                }
                self.close(id);
            }
            TSType::TypeReference(r) => {
                let id = self.add(NodeKind::TSTypeReference, r.span, parent, addr_of(r));
                self.visit_entity_name(&r.type_name, id);
                if let Some(ta) = &r.type_arguments {
                    self.visit_type_args(ta, id);
                }
                self.close(id);
            }
            TSType::TypeLiteral(tl) => {
                let id = self.add(NodeKind::TSTypeLiteral, tl.span, parent, addr_of(tl));
                self.visit_type_elements(tl.members, id);
                self.close(id);
            }
            &TSType::Function(_, f) => {
                let f = view.fn_type(f);
                let id = self.add(NodeKind::TSFunctionType, f.span, parent, addr_of(f));
                self.visit_type_params(f.type_parameters.as_ref(), id);
                self.visit_params(f.params, id);
                self.visit_type_annotation(&f.return_type, id);
                self.close(id);
            }
            &TSType::Constructor(_, c) => {
                let c = view.ctor_type(c);
                let id = self.add(NodeKind::TSConstructorType, c.span, parent, addr_of(c));
                self.visit_type_params(c.type_parameters.as_ref(), id);
                self.visit_params(c.params, id);
                self.visit_type_annotation(&c.return_type, id);
                self.close(id);
            }
            TSType::Tuple(t) => {
                let id = self.add(NodeKind::TSTupleType, t.span, parent, addr_of(t));
                for &e in self.view.ty_list(t.element_types) {
                    self.visit_ty(e, id);
                }
                self.close(id);
            }
            TSType::Parenthesized(p) => {
                let id = self.add(NodeKind::TSParenthesizedType, p.span, parent, addr_of(p));
                self.visit_ty(p.type_annotation, id);
                self.close(id);
            }
            TSType::TypePredicate(p) => {
                let id = self.add(NodeKind::TSTypePredicate, p.span, parent, addr_of(p));
                self.visit_identifier(&p.parameter_name, id);
                if let Some(t) = p.type_annotation {
                    self.visit_ty(t, id);
                }
                self.close(id);
            }
            TSType::Conditional(c) => {
                let id = self.add(NodeKind::TSConditionalType, c.span, parent, addr_of(c));
                self.visit_ty(c.check_type, id);
                self.visit_ty(c.extends_type, id);
                self.visit_ty(c.true_type, id);
                self.visit_ty(c.false_type, id);
                self.close(id);
            }
            TSType::Mapped(m) => {
                let id = self.add(NodeKind::TSMappedType, m.span, parent, addr_of(m));
                self.visit_mapped_type_parameter(&m.type_parameter, id);
                if let Some(nt) = m.name_type {
                    self.visit_ty(nt, id);
                }
                if let Some(ta) = m.type_annotation {
                    self.visit_ty(ta, id);
                }
                self.close(id);
            }
            TSType::TypeOperator(o) => {
                let id = self.add(NodeKind::TSTypeOperator, o.span, parent, addr_of(o));
                self.visit_ty(o.type_annotation, id);
                self.close(id);
            }
            &TSType::Import(_, i) => self.visit_import_type(view.import_type(i), parent),
            TSType::TypeQuery(q) => {
                let id = self.add(NodeKind::TSTypeQuery, q.span, parent, addr_of(q));
                match &q.expr_name {
                    TSTypeQueryExprName::EntityName(en) => self.visit_entity_name(en, id),
                    &TSTypeQueryExprName::Import(_, imp) => {
                        self.visit_import_type(view.import_type(imp), id);
                    }
                }
                if let Some(ta) = &q.type_arguments {
                    self.visit_type_args(ta, id);
                }
                self.close(id);
            }
            TSType::IndexedAccess(i) => {
                let id = self.add(NodeKind::TSIndexedAccessType, i.span, parent, addr_of(i));
                self.visit_ty(i.object_type, id);
                self.visit_ty(i.index_type, id);
                self.close(id);
            }
            TSType::Rest(r) => {
                let id = self.add(NodeKind::TSRestType, r.span, parent, addr_of(r));
                self.visit_ty(r.type_annotation, id);
                self.close(id);
            }
            TSType::Optional(o) => {
                let id = self.add(NodeKind::TSOptionalType, o.span, parent, addr_of(o));
                self.visit_ty(o.type_annotation, id);
                self.close(id);
            }
            TSType::NamedTupleMember(n) => {
                let id = self.add(NodeKind::TSNamedTupleMember, n.span, parent, addr_of(n));
                self.visit_identifier(&n.label, id);
                self.visit_ty(n.element_type, id);
                self.close(id);
            }
            &TSType::Infer(_, inf) => {
                let inf = view.infer_type(inf);
                let id = self.add(NodeKind::TSInferType, inf.span, parent, addr_of(inf));
                self.visit_type_parameter(&inf.type_parameter, id);
                self.close(id);
            }
        }
    }

    /// The nested `TSLiteralType` dispatcher: a template-literal type is its own
    /// node (`TSTemplateLiteralType`); a string/number/bigint literal type reuses
    /// `Literal`; a negative-number literal type reuses `UnaryExpression`.
    fn visit_literal_type(&mut self, lit: &'a TSLiteralType, parent: NodeId) {
        match lit {
            TSLiteralType::TemplateLiteral(t) => {
                let id = self.add(NodeKind::TSTemplateLiteralType, t.span, parent, addr_of(t));
                for q in self.view.quasis(t.quasis) {
                    self.visit_template_element(q, id);
                }
                for &ty in self.view.ty_list(t.types) {
                    self.visit_ty(ty, id);
                }
                self.close(id);
            }
            TSLiteralType::String(l) | TSLiteralType::Number(l) | TSLiteralType::BigInt(l) => {
                self.leaf(NodeKind::Literal, l.span, addr_of(l), parent);
            }
            TSLiteralType::UnaryExpression(u) => {
                let id = self.add(NodeKind::UnaryExpression, u.span, parent, addr_of(u));
                self.visit_expr(u.argument, id);
                self.close(id);
            }
        }
    }
}
