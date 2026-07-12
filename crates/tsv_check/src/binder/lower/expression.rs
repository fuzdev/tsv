//! The expression-shaped visitor methods — `visit_expression` (the full
//! pattern-aware descent, since expression slots also host `Object`/`Array`/
//! `Assignment` patterns, `RestElement`, and `TSParameterProperty`) and its
//! supporting visitors (object/array literal members, templates, decorators,
//! identifiers).

use super::super::*;
use tsv_ts::ast::internal::{
    ArrowFunctionBody, Decorator, Expression, FunctionExpression, Identifier,
    ObjectPatternProperty, ObjectProperty, Property, RestElement, SpreadElement, TemplateElement,
    TemplateLiteral,
};

impl SoaWalk {
    pub(super) fn visit_params(&mut self, params: &[Expression<'_>], parent: NodeId) {
        for param in params {
            self.visit_expression(param, parent);
        }
    }

    /// Visit any expression position, including the pattern-shaped ones
    /// (`Object`/`Array`/`Assignment` pattern, `RestElement`, `TSParameterProperty`)
    /// that occupy parameter, declarator, assignment-target, and for-left slots. A
    /// binding identifier / pattern also carries an optional type annotation and
    /// parameter decorators — `None` outside those positions, so descending them
    /// unconditionally lets this one method serve every expression slot.
    pub(super) fn visit_expression(&mut self, expr: &Expression<'_>, parent: NodeId) {
        use Expression as E;
        match expr {
            E::Identifier(idn) => self.visit_identifier(idn, parent),
            E::Literal(lit) => self.leaf(NodeKind::Literal, lit.span, addr_of(lit), parent),
            E::PrivateIdentifier(pid) => {
                self.leaf(NodeKind::PrivateIdentifier, pid.span, addr_of(pid), parent);
            }
            E::RegexLiteral(r) => self.leaf(NodeKind::RegexLiteral, r.span, addr_of(r), parent),
            E::ThisExpression(t) => self.leaf(NodeKind::ThisExpression, t.span, addr_of(t), parent),
            E::Super(s) => self.leaf(NodeKind::Super, s.span, addr_of(s), parent),
            E::ObjectExpression(o) => {
                let id = self.add(NodeKind::ObjectExpression, o.span, Some(parent), addr_of(o));
                for prop in o.properties {
                    self.visit_object_property(prop, id);
                }
                self.close(id);
            }
            E::ArrayExpression(a) => {
                let id = self.add(NodeKind::ArrayExpression, a.span, Some(parent), addr_of(a));
                for el in a.elements.iter().flatten() {
                    self.visit_expression(el, id);
                }
                self.close(id);
            }
            E::UnaryExpression(u) => {
                let id = self.add(NodeKind::UnaryExpression, u.span, Some(parent), addr_of(u));
                self.visit_expression(u.argument, id);
                self.close(id);
            }
            E::UpdateExpression(u) => {
                let id = self.add(NodeKind::UpdateExpression, u.span, Some(parent), addr_of(u));
                self.visit_expression(u.argument, id);
                self.close(id);
            }
            E::BinaryExpression(b) => {
                let id = self.add(NodeKind::BinaryExpression, b.span, Some(parent), addr_of(b));
                self.visit_expression(b.left, id);
                self.visit_expression(b.right, id);
                self.close(id);
            }
            E::CallExpression(c) => {
                let id = self.add(NodeKind::CallExpression, c.span, Some(parent), addr_of(c));
                self.visit_expression(c.callee, id);
                if let Some(ta) = &c.type_arguments {
                    self.visit_type_args(ta, id);
                }
                for a in c.arguments {
                    self.visit_expression(a, id);
                }
                self.close(id);
            }
            E::NewExpression(n) => {
                let id = self.add(NodeKind::NewExpression, n.span, Some(parent), addr_of(n));
                self.visit_expression(n.callee, id);
                if let Some(ta) = &n.type_arguments {
                    self.visit_type_args(ta, id);
                }
                for a in n.arguments {
                    self.visit_expression(a, id);
                }
                self.close(id);
            }
            E::MemberExpression(m) => {
                let id = self.add(NodeKind::MemberExpression, m.span, Some(parent), addr_of(m));
                self.visit_expression(m.object, id);
                self.visit_expression(m.property, id);
                self.close(id);
            }
            E::ConditionalExpression(c) => {
                let id = self.add(
                    NodeKind::ConditionalExpression,
                    c.span,
                    Some(parent),
                    addr_of(c),
                );
                self.visit_expression(c.test, id);
                self.visit_expression(c.consequent, id);
                self.visit_expression(c.alternate, id);
                self.close(id);
            }
            E::ArrowFunctionExpression(a) => {
                let id = self.add(
                    NodeKind::ArrowFunctionExpression,
                    a.span,
                    Some(parent),
                    addr_of(a),
                );
                self.visit_type_params(a.type_parameters.as_ref(), id);
                self.visit_params(a.params, id);
                self.visit_type_annotation_opt(a.return_type.as_ref(), id);
                match &a.body {
                    ArrowFunctionBody::Expression(e) => self.visit_expression(e, id),
                    ArrowFunctionBody::BlockStatement(b) => self.visit_statements(b.body, id),
                }
                self.close(id);
            }
            E::FunctionExpression(f) => self.visit_function_expression(f, parent),
            E::ClassExpression(c) => {
                let id = self.add(NodeKind::ClassExpression, c.span, Some(parent), addr_of(c));
                if let Some(name) = &c.id {
                    self.visit_identifier(name, id);
                }
                // Kept in sync with `descend_class` (see the coverage test).
                self.visit_type_params(c.type_parameters.as_ref(), id);
                self.visit_class_heritage(
                    c.decorators,
                    c.super_class,
                    c.super_type_parameters.as_ref(),
                    c.implements,
                    id,
                );
                self.visit_class_body(&c.body, id);
                self.close(id);
            }
            E::SpreadElement(s) => self.visit_spread(s, parent),
            E::TemplateLiteral(t) => self.visit_template_literal(t, parent),
            E::TaggedTemplateExpression(t) => {
                let id = self.add(
                    NodeKind::TaggedTemplateExpression,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                self.visit_expression(t.tag, id);
                if let Some(ta) = &t.type_arguments {
                    self.visit_type_args(ta, id);
                }
                self.visit_template_literal(&t.quasi, id);
                self.close(id);
            }
            E::AwaitExpression(a) => {
                let id = self.add(NodeKind::AwaitExpression, a.span, Some(parent), addr_of(a));
                self.visit_expression(a.argument, id);
                self.close(id);
            }
            E::YieldExpression(y) => {
                let id = self.add(NodeKind::YieldExpression, y.span, Some(parent), addr_of(y));
                if let Some(a) = y.argument {
                    self.visit_expression(a, id);
                }
                self.close(id);
            }
            E::SequenceExpression(s) => {
                let id = self.add(
                    NodeKind::SequenceExpression,
                    s.span,
                    Some(parent),
                    addr_of(s),
                );
                for e in s.expressions {
                    self.visit_expression(e, id);
                }
                self.close(id);
            }
            E::AssignmentExpression(a) => {
                let id = self.add(
                    NodeKind::AssignmentExpression,
                    a.span,
                    Some(parent),
                    addr_of(a),
                );
                // `a.left` may be an Object/Array pattern (destructuring assignment)
                // — pattern-aware descent, never swallowed by a wildcard.
                self.visit_expression(a.left, id);
                self.visit_expression(a.right, id);
                self.close(id);
            }
            E::ObjectPattern(op) => {
                let id = self.add(NodeKind::ObjectPattern, op.span, Some(parent), addr_of(op));
                if let Some(decs) = op.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_type_annotation_opt(op.type_annotation.as_ref(), id);
                for prop in op.properties {
                    self.visit_object_pattern_property(prop, id);
                }
                self.close(id);
            }
            E::ArrayPattern(ap) => {
                let id = self.add(NodeKind::ArrayPattern, ap.span, Some(parent), addr_of(ap));
                if let Some(decs) = ap.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_type_annotation_opt(ap.type_annotation.as_ref(), id);
                for el in ap.elements.iter().flatten() {
                    self.visit_expression(el, id);
                }
                self.close(id);
            }
            E::AssignmentPattern(a) => {
                let id = self.add(
                    NodeKind::AssignmentPattern,
                    a.span,
                    Some(parent),
                    addr_of(a),
                );
                if let Some(decs) = a.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_expression(a.left, id);
                self.visit_expression(a.right, id);
                self.close(id);
            }
            E::RestElement(r) => self.visit_rest_element(r, parent),
            E::TSTypeAssertion(t) => {
                let id = self.add(NodeKind::TSTypeAssertion, t.span, Some(parent), addr_of(t));
                self.visit_type(t.type_annotation, id);
                self.visit_expression(t.expression, id);
                self.close(id);
            }
            E::TSAsExpression(t) => {
                let id = self.add(NodeKind::TSAsExpression, t.span, Some(parent), addr_of(t));
                self.visit_expression(t.expression, id);
                self.visit_type(t.type_annotation, id);
                self.close(id);
            }
            E::TSSatisfiesExpression(t) => {
                let id = self.add(
                    NodeKind::TSSatisfiesExpression,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                self.visit_expression(t.expression, id);
                self.visit_type(t.type_annotation, id);
                self.close(id);
            }
            E::TSInstantiationExpression(t) => {
                let id = self.add(
                    NodeKind::TSInstantiationExpression,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                self.visit_expression(t.expression, id);
                self.visit_type_args(&t.type_arguments, id);
                self.close(id);
            }
            E::TSNonNullExpression(t) => {
                let id = self.add(
                    NodeKind::TSNonNullExpression,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                self.visit_expression(t.expression, id);
                self.close(id);
            }
            E::TSParameterProperty(pp) => {
                let id = self.add(
                    NodeKind::TSParameterProperty,
                    pp.span,
                    Some(parent),
                    addr_of(pp),
                );
                self.visit_expression(pp.parameter, id);
                self.close(id);
            }
            E::ImportExpression(i) => {
                let id = self.add(NodeKind::ImportExpression, i.span, Some(parent), addr_of(i));
                self.visit_expression(i.source, id);
                if let Some(o) = i.options {
                    self.visit_expression(o, id);
                }
                self.close(id);
            }
            E::MetaProperty(m) => {
                let id = self.add(NodeKind::MetaProperty, m.span, Some(parent), addr_of(m));
                self.visit_identifier(&m.meta, id);
                self.visit_identifier(&m.property, id);
                self.close(id);
            }
            E::JsdocCast(c) => {
                let id = self.add(NodeKind::JsdocCast, c.span, Some(parent), addr_of(c));
                self.visit_expression(c.inner, id);
                self.close(id);
            }
            E::ParenthesizedExpression(p) => {
                let id = self.add(
                    NodeKind::ParenthesizedExpression,
                    p.span,
                    Some(parent),
                    addr_of(p),
                );
                self.visit_expression(p.expression, id);
                self.close(id);
            }
        }
        // Lockstep guard: the arm above must have registered this expression
        // under the `(address, kind)` key the shared `expression_addr_kind`
        // mapping (the flow walk's resolver) predicts — drift between the two
        // is caught here per lowered expression in debug builds (which the
        // conformance gate runs), before the strict resolver would hard-fail.
        debug_assert!(
            self.address_map.contains_key(&expression_addr_kind(expr)),
            "visit_expression and expression_addr_kind disagree on an expression's (address, kind) key"
        );
    }

    pub(super) fn visit_function_expression(&mut self, f: &FunctionExpression<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::FunctionExpression,
            f.span,
            Some(parent),
            addr_of(f),
        );
        self.descend_function_common(
            id,
            f.id.as_ref(),
            f.type_parameters.as_ref(),
            f.params,
            f.return_type.as_ref(),
            f.body.body,
        );
        self.close(id);
    }

    fn visit_object_property(&mut self, prop: &ObjectProperty<'_>, parent: NodeId) {
        match prop {
            ObjectProperty::Property(pr) => self.visit_property(pr, parent),
            ObjectProperty::SpreadElement(s) => self.visit_spread(s, parent),
        }
    }

    fn visit_object_pattern_property(&mut self, prop: &ObjectPatternProperty<'_>, parent: NodeId) {
        match prop {
            ObjectPatternProperty::Property(pr) => self.visit_property(pr, parent),
            ObjectPatternProperty::RestElement(r) => self.visit_rest_element(r, parent),
        }
    }

    fn visit_property(&mut self, pr: &Property<'_>, parent: NodeId) {
        let id = self.add(NodeKind::Property, pr.span, Some(parent), addr_of(pr));
        self.visit_expression(&pr.key, id);
        self.visit_expression(&pr.value, id);
        self.close(id);
    }

    fn visit_spread(&mut self, s: &SpreadElement<'_>, parent: NodeId) {
        let id = self.add(NodeKind::SpreadElement, s.span, Some(parent), addr_of(s));
        self.visit_expression(s.argument, id);
        self.close(id);
    }

    fn visit_rest_element(&mut self, r: &RestElement<'_>, parent: NodeId) {
        let id = self.add(NodeKind::RestElement, r.span, Some(parent), addr_of(r));
        self.visit_type_annotation_opt(r.type_annotation.as_ref(), id);
        self.visit_expression(r.argument, id);
        self.close(id);
    }

    fn visit_template_literal(&mut self, t: &TemplateLiteral<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TemplateLiteral, t.span, Some(parent), addr_of(t));
        for q in t.quasis {
            self.visit_template_element(q, id);
        }
        for e in t.expressions {
            self.visit_expression(e, id);
        }
        self.close(id);
    }

    pub(super) fn visit_template_element(&mut self, q: &TemplateElement<'_>, parent: NodeId) {
        self.leaf(NodeKind::TemplateElement, q.span, addr_of(q), parent);
    }

    pub(super) fn visit_decorators(&mut self, decorators: &[Decorator<'_>], parent: NodeId) {
        for d in decorators {
            let id = self.add(NodeKind::Decorator, d.span, Some(parent), addr_of(d));
            self.visit_expression(&d.expression, id);
            self.close(id);
        }
    }

    // --- identifiers ---------------------------------------------------------

    /// Id an identifier, then descend the binding-only extras (parameter
    /// decorators + type annotation) it carries — both `None` for a reference, so
    /// this serves reference and binding positions alike.
    pub(super) fn visit_identifier(&mut self, ident: &Identifier<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::Identifier,
            ident.span,
            Some(parent),
            addr_of(ident),
        );
        if let Some(decs) = ident.decorators() {
            self.visit_decorators(decs, id);
        }
        if let Some(ann) = ident.type_annotation() {
            self.visit_type_annotation(ann, id);
        }
        self.close(id);
    }
}
