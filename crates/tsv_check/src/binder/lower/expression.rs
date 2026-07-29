//! The expression-shaped visitor methods — `visit_expression` (the full
//! pattern-aware descent, since expression slots also host `Object`/`Array`/
//! `Assignment` patterns, `RestElement`, and `TSParameterProperty`) and its
//! supporting visitors (object/array literal members, templates, decorators,
//! identifiers).

use super::super::*;
use tsv_ts::ast::internal::{
    ArrayExpression, ArrayPattern, ArrowFunctionBody, ArrowFunctionExpression,
    AssignmentExpression, AssignmentPattern, AwaitExpression, BinaryExpression, CallExpression,
    ClassExpression, ConditionalExpression, Decorator, Expression, FunctionExpression, Identifier,
    ImportExpression, JsdocCast, MemberExpression, MetaProperty, NewExpression, ObjectExpression,
    ObjectPattern, ObjectPatternProperty, ObjectProperty, ParenthesizedExpression, Property,
    RestElement, SequenceExpression, SpreadElement, TSAsExpression, TSInstantiationExpression,
    TSNonNullExpression, TSParameterProperty, TSSatisfiesExpression, TSTypeAssertion,
    TaggedTemplateExpression, TemplateElement, TemplateLiteral, UnaryExpression, UpdateExpression,
    YieldExpression,
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
            E::ObjectExpression(o) => self.visit_object_expression(o, parent),
            E::ArrayExpression(a) => self.visit_array_expression(a, parent),
            E::UnaryExpression(u) => self.visit_unary_expression(u, parent),
            E::UpdateExpression(u) => self.visit_update_expression(u, parent),
            E::BinaryExpression(b) => self.visit_binary_expression(b, parent),
            E::CallExpression(c) => self.visit_call_expression(c, parent),
            E::NewExpression(n) => self.visit_new_expression(n, parent),
            E::MemberExpression(m) => self.visit_member_expression(m, parent),
            E::ConditionalExpression(c) => self.visit_conditional_expression(c, parent),
            E::ArrowFunctionExpression(a) => self.visit_arrow_function_expression(a, parent),
            E::FunctionExpression(f) => self.visit_function_expression(f, parent),
            E::ClassExpression(c) => self.visit_class_expression(c, parent),
            E::SpreadElement(s) => self.visit_spread(s, parent),
            E::TemplateLiteral(t) => self.visit_template_literal(t, parent),
            E::TaggedTemplateExpression(t) => self.visit_tagged_template_expression(t, parent),
            E::AwaitExpression(a) => self.visit_await_expression(a, parent),
            E::YieldExpression(y) => self.visit_yield_expression(y, parent),
            E::SequenceExpression(s) => self.visit_sequence_expression(s, parent),
            E::AssignmentExpression(a) => self.visit_assignment_expression(a, parent),
            E::ObjectPattern(op) => self.visit_object_pattern(op, parent),
            E::ArrayPattern(ap) => self.visit_array_pattern(ap, parent),
            E::AssignmentPattern(a) => self.visit_assignment_pattern(a, parent),
            E::RestElement(r) => self.visit_rest_element(r, parent),
            E::TSTypeAssertion(t) => self.visit_ts_type_assertion(t, parent),
            E::TSAsExpression(t) => self.visit_ts_as_expression(t, parent),
            E::TSSatisfiesExpression(t) => self.visit_ts_satisfies_expression(t, parent),
            E::TSInstantiationExpression(t) => self.visit_ts_instantiation_expression(t, parent),
            E::TSNonNullExpression(t) => self.visit_ts_non_null_expression(t, parent),
            E::TSParameterProperty(pp) => self.visit_ts_parameter_property(pp, parent),
            E::ImportExpression(i) => self.visit_import_expression(i, parent),
            E::MetaProperty(m) => self.visit_meta_property(m, parent),
            E::JsdocCast(c) => self.visit_jsdoc_cast(c, parent),
            E::ParenthesizedExpression(p) => self.visit_parenthesized_expression(p, parent),
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

    #[inline]
    fn visit_object_expression(&mut self, o: &ObjectExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::ObjectExpression, o.span, Some(parent), addr_of(o));
        for prop in o.properties {
            self.visit_object_property(prop, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_array_expression(&mut self, a: &ArrayExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::ArrayExpression, a.span, Some(parent), addr_of(a));
        for el in a.elements.iter().flatten() {
            self.visit_expression(el, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::UnaryExpression, u.span, Some(parent), addr_of(u));
        self.visit_expression(u.argument, id);
        self.close(id);
    }

    #[inline]
    fn visit_update_expression(&mut self, u: &UpdateExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::UpdateExpression, u.span, Some(parent), addr_of(u));
        self.visit_expression(u.argument, id);
        self.close(id);
    }

    #[inline]
    fn visit_binary_expression(&mut self, b: &BinaryExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::BinaryExpression, b.span, Some(parent), addr_of(b));
        self.visit_expression(b.left, id);
        self.visit_expression(b.right, id);
        self.close(id);
    }

    #[inline]
    fn visit_call_expression(&mut self, c: &CallExpression<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_new_expression(&mut self, n: &NewExpression<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_member_expression(&mut self, m: &MemberExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::MemberExpression, m.span, Some(parent), addr_of(m));
        self.visit_expression(m.object, id);
        self.visit_expression(m.property, id);
        self.close(id);
    }

    #[inline]
    fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_arrow_function_expression(&mut self, a: &ArrowFunctionExpression<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_class_expression(&mut self, c: &ClassExpression<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_tagged_template_expression(
        &mut self,
        t: &TaggedTemplateExpression<'_>,
        parent: NodeId,
    ) {
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

    #[inline]
    fn visit_await_expression(&mut self, a: &AwaitExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::AwaitExpression, a.span, Some(parent), addr_of(a));
        self.visit_expression(a.argument, id);
        self.close(id);
    }

    #[inline]
    fn visit_yield_expression(&mut self, y: &YieldExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::YieldExpression, y.span, Some(parent), addr_of(y));
        if let Some(a) = y.argument {
            self.visit_expression(a, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_sequence_expression(&mut self, s: &SequenceExpression<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_object_pattern(&mut self, op: &ObjectPattern<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_array_pattern(&mut self, ap: &ArrayPattern<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_assignment_pattern(&mut self, a: &AssignmentPattern<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_ts_type_assertion(&mut self, t: &TSTypeAssertion<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TSTypeAssertion, t.span, Some(parent), addr_of(t));
        self.visit_type(t.type_annotation, id);
        self.visit_expression(t.expression, id);
        self.close(id);
    }

    #[inline]
    fn visit_ts_as_expression(&mut self, t: &TSAsExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TSAsExpression, t.span, Some(parent), addr_of(t));
        self.visit_expression(t.expression, id);
        self.visit_type(t.type_annotation, id);
        self.close(id);
    }

    #[inline]
    fn visit_ts_satisfies_expression(&mut self, t: &TSSatisfiesExpression<'_>, parent: NodeId) {
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

    #[inline]
    fn visit_ts_instantiation_expression(
        &mut self,
        t: &TSInstantiationExpression<'_>,
        parent: NodeId,
    ) {
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

    #[inline]
    fn visit_ts_non_null_expression(&mut self, t: &TSNonNullExpression<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::TSNonNullExpression,
            t.span,
            Some(parent),
            addr_of(t),
        );
        self.visit_expression(t.expression, id);
        self.close(id);
    }

    #[inline]
    fn visit_ts_parameter_property(&mut self, pp: &TSParameterProperty<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::TSParameterProperty,
            pp.span,
            Some(parent),
            addr_of(pp),
        );
        self.visit_expression(pp.parameter, id);
        self.close(id);
    }

    #[inline]
    fn visit_import_expression(&mut self, i: &ImportExpression<'_>, parent: NodeId) {
        let id = self.add(NodeKind::ImportExpression, i.span, Some(parent), addr_of(i));
        self.visit_expression(i.source, id);
        if let Some(o) = i.options {
            self.visit_expression(o, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_meta_property(&mut self, m: &MetaProperty<'_>, parent: NodeId) {
        let id = self.add(NodeKind::MetaProperty, m.span, Some(parent), addr_of(m));
        self.visit_identifier(&m.meta, id);
        self.visit_identifier(&m.property, id);
        self.close(id);
    }

    #[inline]
    fn visit_jsdoc_cast(&mut self, c: &JsdocCast<'_>, parent: NodeId) {
        let id = self.add(NodeKind::JsdocCast, c.span, Some(parent), addr_of(c));
        self.visit_expression(c.inner, id);
        self.close(id);
    }

    #[inline]
    fn visit_parenthesized_expression(&mut self, p: &ParenthesizedExpression<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::ParenthesizedExpression,
            p.span,
            Some(parent),
            addr_of(p),
        );
        self.visit_expression(p.expression, id);
        self.close(id);
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
