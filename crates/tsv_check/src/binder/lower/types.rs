//! The type-shaped visitor methods — `visit_type` and everything a type
//! position (annotations, type arguments/parameters, entity names, interface /
//! type-literal members) descends into. `visit_index_signature` lives here too:
//! one `TSIndexSignature` node serves both a class member and a type-element
//! position, and its shape (parameters + an optional type annotation) is purely
//! type-flavored.

use super::super::*;
use tsv_ts::ast::internal::{
    TSEntityName, TSImportType, TSIndexSignature, TSLiteralType, TSMappedTypeParameter,
    TSQualifiedName, TSType, TSTypeAnnotation, TSTypeElement, TSTypeParameter,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypeQueryExprName,
};

impl SoaWalk {
    pub(super) fn visit_index_signature(&mut self, i: &TSIndexSignature<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TSIndexSignature, i.span, Some(parent), addr_of(i));
        for p in i.parameters {
            self.visit_identifier(p, id);
        }
        self.visit_type_annotation_opt(i.type_annotation.as_ref(), id);
        self.close(id);
    }

    /// A `TSTypeAnnotation` (`: T`) is a transparent wrapper — not idd; the walk
    /// descends straight into the inner `TSType`, which is the node.
    pub(super) fn visit_type_annotation(&mut self, ann: &TSTypeAnnotation<'_>, parent: NodeId) {
        self.visit_type(ann.type_annotation, parent);
    }

    pub(super) fn visit_type_annotation_opt(
        &mut self,
        ann: Option<&TSTypeAnnotation<'_>>,
        parent: NodeId,
    ) {
        if let Some(a) = ann {
            self.visit_type_annotation(a, parent);
        }
    }

    pub(super) fn visit_type_args(
        &mut self,
        args: &TSTypeParameterInstantiation<'_>,
        parent: NodeId,
    ) {
        let id = self.add(
            NodeKind::TSTypeParameterInstantiation,
            args.span,
            Some(parent),
            addr_of(args),
        );
        for t in args.params {
            self.visit_type(t, id);
        }
        self.close(id);
    }

    pub(super) fn visit_type_params(
        &mut self,
        params: Option<&TSTypeParameterDeclaration<'_>>,
        parent: NodeId,
    ) {
        if let Some(decl) = params {
            let id = self.add(
                NodeKind::TSTypeParameterDeclaration,
                decl.span,
                Some(parent),
                addr_of(decl),
            );
            for p in decl.params {
                self.visit_type_parameter(p, id);
            }
            self.close(id);
        }
    }

    fn visit_type_parameter(&mut self, p: &TSTypeParameter<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TSTypeParameter, p.span, Some(parent), addr_of(p));
        self.visit_identifier(&p.name, id);
        if let Some(c) = p.constraint {
            self.visit_type(c, id);
        }
        if let Some(d) = p.default {
            self.visit_type(d, id);
        }
        self.close(id);
    }

    fn visit_mapped_type_parameter(&mut self, mtp: &TSMappedTypeParameter<'_>, parent: NodeId) {
        // The `name` is a bare `IdentName` (no child identifier node); the mapped
        // type parameter's own span covers the name token.
        let id = self.add(
            NodeKind::TSMappedTypeParameter,
            mtp.span,
            Some(parent),
            addr_of(mtp),
        );
        self.visit_type(mtp.constraint, id);
        self.close(id);
    }

    pub(super) fn visit_entity_name(&mut self, name: &TSEntityName<'_>, parent: NodeId) {
        match name {
            TSEntityName::Identifier(id) => self.visit_identifier(id, parent),
            TSEntityName::QualifiedName(qn) => self.visit_qualified_name(qn, parent),
        }
    }

    fn visit_qualified_name(&mut self, qn: &TSQualifiedName<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::TSQualifiedName,
            qn.span,
            Some(parent),
            addr_of(qn),
        );
        self.visit_entity_name(&qn.left, id);
        self.visit_identifier(&qn.right, id);
        self.close(id);
    }

    fn visit_import_type(&mut self, i: &TSImportType<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TSImportType, i.span, Some(parent), addr_of(i));
        self.leaf(NodeKind::Literal, i.argument.span, addr_of(&i.argument), id);
        if let Some(o) = i.options {
            self.visit_expression(o, id);
        }
        if let Some(q) = &i.qualifier {
            self.visit_entity_name(q, id);
        }
        if let Some(ta) = &i.type_arguments {
            self.visit_type_args(ta, id);
        }
        self.close(id);
    }

    pub(super) fn visit_type_elements(&mut self, members: &[TSTypeElement<'_>], parent: NodeId) {
        for member in members {
            self.visit_type_element(member, parent);
        }
    }

    fn visit_type_element(&mut self, member: &TSTypeElement<'_>, parent: NodeId) {
        match member {
            TSTypeElement::PropertySignature(p) => {
                let id = self.add(
                    NodeKind::TSPropertySignature,
                    p.span,
                    Some(parent),
                    addr_of(p),
                );
                self.visit_expression(&p.key, id);
                self.visit_type_annotation_opt(p.type_annotation.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::MethodSignature(m) => {
                let id = self.add(
                    NodeKind::TSMethodSignature,
                    m.span,
                    Some(parent),
                    addr_of(m),
                );
                self.visit_expression(&m.key, id);
                self.visit_type_params(m.type_parameters.as_ref(), id);
                self.visit_params(m.params, id);
                self.visit_type_annotation_opt(m.return_type.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::CallSignature(c) => {
                let id = self.add(
                    NodeKind::TSCallSignatureDeclaration,
                    c.span,
                    Some(parent),
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
                    Some(parent),
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

    pub(super) fn visit_type(&mut self, ty: &TSType<'_>, parent: NodeId) {
        match ty {
            TSType::Keyword(kw) => self.leaf(NodeKind::TSKeywordType, kw.span, addr_of(kw), parent),
            TSType::ThisType(t) => self.leaf(NodeKind::TSThisType, t.span, addr_of(t), parent),
            TSType::Literal(lit) => self.visit_literal_type(lit, parent),
            TSType::Array(a) => {
                let id = self.add(NodeKind::TSArrayType, a.span, Some(parent), addr_of(a));
                self.visit_type(a.element_type, id);
                self.close(id);
            }
            TSType::Union(u) => {
                let id = self.add(NodeKind::TSUnionType, u.span, Some(parent), addr_of(u));
                for t in u.types {
                    self.visit_type(t, id);
                }
                self.close(id);
            }
            TSType::Intersection(i) => {
                let id = self.add(
                    NodeKind::TSIntersectionType,
                    i.span,
                    Some(parent),
                    addr_of(i),
                );
                for t in i.types {
                    self.visit_type(t, id);
                }
                self.close(id);
            }
            TSType::TypeReference(r) => {
                let id = self.add(NodeKind::TSTypeReference, r.span, Some(parent), addr_of(r));
                self.visit_entity_name(&r.type_name, id);
                if let Some(ta) = &r.type_arguments {
                    self.visit_type_args(ta, id);
                }
                self.close(id);
            }
            TSType::TypeLiteral(tl) => {
                let id = self.add(NodeKind::TSTypeLiteral, tl.span, Some(parent), addr_of(tl));
                self.visit_type_elements(tl.members, id);
                self.close(id);
            }
            TSType::Function(f) => {
                let id = self.add(NodeKind::TSFunctionType, f.span, Some(parent), addr_of(f));
                self.visit_type_params(f.type_parameters.as_ref(), id);
                self.visit_params(f.params, id);
                self.visit_type_annotation(&f.return_type, id);
                self.close(id);
            }
            TSType::Constructor(c) => {
                let id = self.add(
                    NodeKind::TSConstructorType,
                    c.span,
                    Some(parent),
                    addr_of(c),
                );
                self.visit_type_params(c.type_parameters.as_ref(), id);
                self.visit_params(c.params, id);
                self.visit_type_annotation(&c.return_type, id);
                self.close(id);
            }
            TSType::Tuple(t) => {
                let id = self.add(NodeKind::TSTupleType, t.span, Some(parent), addr_of(t));
                for e in t.element_types {
                    self.visit_type(e, id);
                }
                self.close(id);
            }
            TSType::Parenthesized(p) => {
                let id = self.add(
                    NodeKind::TSParenthesizedType,
                    p.span,
                    Some(parent),
                    addr_of(p),
                );
                self.visit_type(p.type_annotation, id);
                self.close(id);
            }
            TSType::TypePredicate(p) => {
                let id = self.add(NodeKind::TSTypePredicate, p.span, Some(parent), addr_of(p));
                self.visit_identifier(&p.parameter_name, id);
                if let Some(t) = p.type_annotation {
                    self.visit_type(t, id);
                }
                self.close(id);
            }
            TSType::Conditional(c) => {
                let id = self.add(
                    NodeKind::TSConditionalType,
                    c.span,
                    Some(parent),
                    addr_of(c),
                );
                self.visit_type(c.check_type, id);
                self.visit_type(c.extends_type, id);
                self.visit_type(c.true_type, id);
                self.visit_type(c.false_type, id);
                self.close(id);
            }
            TSType::Mapped(m) => {
                let id = self.add(NodeKind::TSMappedType, m.span, Some(parent), addr_of(m));
                self.visit_mapped_type_parameter(&m.type_parameter, id);
                if let Some(nt) = m.name_type {
                    self.visit_type(nt, id);
                }
                if let Some(ta) = m.type_annotation {
                    self.visit_type(ta, id);
                }
                self.close(id);
            }
            TSType::TypeOperator(o) => {
                let id = self.add(NodeKind::TSTypeOperator, o.span, Some(parent), addr_of(o));
                self.visit_type(o.type_annotation, id);
                self.close(id);
            }
            TSType::Import(i) => self.visit_import_type(i, parent),
            TSType::TypeQuery(q) => {
                let id = self.add(NodeKind::TSTypeQuery, q.span, Some(parent), addr_of(q));
                match &q.expr_name {
                    TSTypeQueryExprName::EntityName(en) => self.visit_entity_name(en, id),
                    TSTypeQueryExprName::Import(imp) => self.visit_import_type(imp, id),
                }
                if let Some(ta) = &q.type_arguments {
                    self.visit_type_args(ta, id);
                }
                self.close(id);
            }
            TSType::IndexedAccess(i) => {
                let id = self.add(
                    NodeKind::TSIndexedAccessType,
                    i.span,
                    Some(parent),
                    addr_of(i),
                );
                self.visit_type(i.object_type, id);
                self.visit_type(i.index_type, id);
                self.close(id);
            }
            TSType::Rest(r) => {
                let id = self.add(NodeKind::TSRestType, r.span, Some(parent), addr_of(r));
                self.visit_type(r.type_annotation, id);
                self.close(id);
            }
            TSType::Optional(o) => {
                let id = self.add(NodeKind::TSOptionalType, o.span, Some(parent), addr_of(o));
                self.visit_type(o.type_annotation, id);
                self.close(id);
            }
            TSType::NamedTupleMember(n) => {
                let id = self.add(
                    NodeKind::TSNamedTupleMember,
                    n.span,
                    Some(parent),
                    addr_of(n),
                );
                self.visit_identifier(&n.label, id);
                self.visit_type(n.element_type, id);
                self.close(id);
            }
            TSType::Infer(inf) => {
                let id = self.add(NodeKind::TSInferType, inf.span, Some(parent), addr_of(inf));
                self.visit_type_parameter(&inf.type_parameter, id);
                self.close(id);
            }
        }
    }

    /// The nested `TSLiteralType` dispatcher: a template-literal type is its own
    /// node (`TSTemplateLiteralType`); a string/number/bigint literal type reuses
    /// `Literal`; a negative-number literal type reuses `UnaryExpression`.
    fn visit_literal_type(&mut self, lit: &TSLiteralType<'_>, parent: NodeId) {
        match lit {
            TSLiteralType::TemplateLiteral(t) => {
                let id = self.add(
                    NodeKind::TSTemplateLiteralType,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                for q in t.quasis {
                    self.visit_template_element(q, id);
                }
                for ty in t.types {
                    self.visit_type(ty, id);
                }
                self.close(id);
            }
            TSLiteralType::String(l) | TSLiteralType::Number(l) | TSLiteralType::BigInt(l) => {
                self.leaf(NodeKind::Literal, l.span, addr_of(l), parent);
            }
            TSLiteralType::UnaryExpression(u) => {
                let id = self.add(NodeKind::UnaryExpression, u.span, Some(parent), addr_of(u));
                self.visit_expression(u.argument, id);
                self.close(id);
            }
        }
    }
}
