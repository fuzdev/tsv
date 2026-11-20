//! The expression-shaped visitor methods — `visit_expression` (the full
//! pattern-aware descent, since expression slots also host `Object`/`Array`/
//! `Assignment` patterns, `RestElement`, and `TSParameterProperty`) and its
//! supporting visitors (object/array literal members, templates, decorators,
//! identifiers).

use super::super::*;
use tsv_ts::ast::{
    ArrayExpression, ArrayPattern, ArrowFunctionBody, ArrowFunctionExpression,
    AssignmentExpression, AssignmentPattern, AwaitExpression, BinaryExpression, CallExpression,
    ClassExpression, CommentBoundPair, ConditionalExpression, Decorator, FunctionExpression,
    Identifier, ImportExpression, MemberExpression, MetaProperty, NewExpression, ObjectExpression,
    ObjectPattern, ObjectPatternProperty, ObjectProperty, ParenthesizedExpression, Property,
    RestElement, SequenceExpression, SpreadElement, TSAsExpression, TSInstantiationExpression,
    TSNonNullExpression, TSParameterProperty, TSSatisfiesExpression, TSTypeAssertion,
    TaggedTemplateExpression, TemplateElement, TemplateLiteral, UnaryExpression, UpdateExpression,
    YieldExpression,
};

impl<'a> SoaWalk<'a> {
    pub(super) fn visit_params(&mut self, params: Run<Expression>, parent: NodeId) {
        for param in self.view.exprs(params) {
            self.visit_expression(param, parent);
        }
    }

    /// Visit the expression `id` names, read in place from its store.
    pub(super) fn visit_expr(&mut self, id: ExprId, parent: NodeId) {
        self.visit_expression(self.view.expr(id), parent);
    }

    /// Visit the expressions of an id run, in order.
    fn visit_expr_list(&mut self, list: Run<ExprId>, parent: NodeId) {
        for &e in self.view.expr_list(list) {
            self.visit_expr(e, parent);
        }
    }

    /// Visit any expression position, including the pattern-shaped ones
    /// (`Object`/`Array`/`Assignment` pattern, `RestElement`, `TSParameterProperty`)
    /// that occupy parameter, declarator, assignment-target, and for-left slots. A
    /// binding identifier / pattern also carries an optional type annotation and
    /// parameter decorators — `None` outside those positions, so descending them
    /// unconditionally lets this one method serve every expression slot.
    pub(super) fn visit_expression(&mut self, expr: &'a Expression, parent: NodeId) {
        use ExpressionKind as E;
        let view = self.view;
        match &expr.kind {
            E::Identifier(idn) => self.visit_identifier(idn, parent),
            E::Literal(lit) => self.leaf(NodeKind::Literal, lit.span, addr_of(lit), parent),
            E::PrivateIdentifier(pid) => {
                self.leaf(NodeKind::PrivateIdentifier, expr.span, addr_of(pid), parent);
            }
            E::RegexLiteral(r) => self.leaf(NodeKind::RegexLiteral, expr.span, addr_of(r), parent),
            E::ThisExpression(t) => {
                self.leaf(NodeKind::ThisExpression, expr.span, addr_of(t), parent);
            }
            E::Super(s) => self.leaf(NodeKind::Super, expr.span, addr_of(s), parent),
            E::ObjectExpression(o) => self.visit_object_expression(o, expr.span, parent),
            E::ArrayExpression(a) => self.visit_array_expression(a, expr.span, parent),
            E::UnaryExpression(u) => self.visit_unary_expression(u, parent),
            E::UpdateExpression(u) => self.visit_update_expression(u, expr.span, parent),
            E::BinaryExpression(b) => self.visit_binary_expression(b, expr.span, parent),
            E::CallExpression(c) => self.visit_call_expression(c, expr.span, parent),
            E::NewExpression(n) => self.visit_new_expression(n, expr.span, parent),
            E::MemberExpression(m) => self.visit_member_expression(m, expr.span, parent),
            E::ConditionalExpression(c) => self.visit_conditional_expression(c, expr.span, parent),
            &E::ArrowFunctionExpression(a) => {
                self.visit_arrow_function_expression(view.arrow_fn(a), expr.span, parent);
            }
            &E::FunctionExpression(f) => self.visit_function_expression(view.fn_expr(f), parent),
            &E::ClassExpression(c) => {
                self.visit_class_expression(view.class_expr(c), expr.span, parent);
            }
            E::SpreadElement(s) => self.visit_spread(s, parent),
            E::TemplateLiteral(t) => self.visit_template_literal(t, expr.span, parent),
            &E::TaggedTemplateExpression(t) => {
                self.visit_tagged_template_expression(view.tagged_tpl(t), expr.span, parent);
            }
            E::AwaitExpression(a) => self.visit_await_expression(a, expr.span, parent),
            E::YieldExpression(y) => self.visit_yield_expression(y, expr.span, parent),
            E::SequenceExpression(s) => self.visit_sequence_expression(s, expr.span, parent),
            E::AssignmentExpression(a) => self.visit_assignment_expression(a, expr.span, parent),
            E::ObjectPattern(op) => self.visit_object_pattern(op, expr.span, parent),
            E::ArrayPattern(ap) => self.visit_array_pattern(ap, expr.span, parent),
            E::AssignmentPattern(a) => self.visit_assignment_pattern(a, expr.span, parent),
            E::RestElement(r) => self.visit_rest_element(r, parent),
            E::TSTypeAssertion(t) => self.visit_ts_type_assertion(t, expr.span, parent),
            E::TSAsExpression(t) => self.visit_ts_as_expression(t, expr.span, parent),
            E::TSSatisfiesExpression(t) => self.visit_ts_satisfies_expression(t, expr.span, parent),
            E::TSInstantiationExpression(t) => {
                self.visit_ts_instantiation_expression(t, expr.span, parent);
            }
            E::TSNonNullExpression(t) => self.visit_ts_non_null_expression(t, expr.span, parent),
            E::TSParameterProperty(pp) => self.visit_ts_parameter_property(pp, expr.span, parent),
            E::ImportExpression(i) => self.visit_import_expression(i, expr.span, parent),
            &E::MetaProperty(m) => self.visit_meta_property(view.meta_prop(m), expr.span, parent),
            &E::CommentBoundPair(c) => {
                self.visit_comment_bound_pair(view.comment_pair(c), expr.span, parent);
            }
            E::ParenthesizedExpression(p) => {
                self.visit_parenthesized_expression(p, expr.span, parent);
            }
        }
        // Lockstep guard: the arm above must have registered this expression
        // under the `(address, kind)` key the shared `expression_addr_kind`
        // mapping (the flow walk's resolver) predicts — drift between the two
        // is caught here per lowered expression in debug builds (which the
        // conformance gate runs), before the strict resolver would hard-fail.
        debug_assert!(
            self.address_map
                .contains_key(&expression_addr_kind(view, expr)),
            "visit_expression and expression_addr_kind disagree on an expression's (address, kind) key"
        );
    }

    #[inline]
    fn visit_object_expression(&mut self, o: &'a ObjectExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::ObjectExpression, span, parent, addr_of(o));
        for prop in self.view.obj_props(o.properties) {
            self.visit_object_property(prop, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_array_expression(&mut self, a: &'a ArrayExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::ArrayExpression, span, parent, addr_of(a));
        for &el in self.view.expr_opt_list(a.elements).iter().flatten() {
            self.visit_expr(el, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_unary_expression(&mut self, u: &'a UnaryExpression, parent: NodeId) {
        let id = self.add(NodeKind::UnaryExpression, u.span, parent, addr_of(u));
        self.visit_expr(u.argument, id);
        self.close(id);
    }

    #[inline]
    fn visit_update_expression(&mut self, u: &'a UpdateExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::UpdateExpression, span, parent, addr_of(u));
        self.visit_expr(u.argument, id);
        self.close(id);
    }

    #[inline]
    fn visit_binary_expression(&mut self, b: &'a BinaryExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::BinaryExpression, span, parent, addr_of(b));
        self.visit_expr(b.left, id);
        self.visit_expr(b.right, id);
        self.close(id);
    }

    #[inline]
    fn visit_call_expression(&mut self, c: &'a CallExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::CallExpression, span, parent, addr_of(c));
        self.visit_expr(c.callee, id);
        if let Some(ta) = c.type_args {
            self.visit_type_args(self.view.type_args(ta), id);
        }
        self.visit_expr_list(c.arguments, id);
        self.close(id);
    }

    #[inline]
    fn visit_new_expression(&mut self, n: &'a NewExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::NewExpression, span, parent, addr_of(n));
        self.visit_expr(n.callee, id);
        if let Some(ta) = n.type_args {
            self.visit_type_args(self.view.type_args(ta), id);
        }
        self.visit_expr_list(n.arguments, id);
        self.close(id);
    }

    #[inline]
    fn visit_member_expression(&mut self, m: &'a MemberExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::MemberExpression, span, parent, addr_of(m));
        self.visit_expr(m.object, id);
        self.visit_expr(m.property, id);
        self.close(id);
    }

    #[inline]
    fn visit_conditional_expression(
        &mut self,
        c: &'a ConditionalExpression,
        span: Span,
        parent: NodeId,
    ) {
        let id = self.add(NodeKind::ConditionalExpression, span, parent, addr_of(c));
        self.visit_expr(c.test, id);
        self.visit_expr(c.consequent, id);
        self.visit_expr(c.alternate, id);
        self.close(id);
    }

    #[inline]
    fn visit_arrow_function_expression(
        &mut self,
        a: &'a ArrowFunctionExpression,
        span: Span,
        parent: NodeId,
    ) {
        let id = self.add(NodeKind::ArrowFunctionExpression, span, parent, addr_of(a));
        self.visit_type_params(a.type_parameters.as_ref(), id);
        self.visit_params(a.params, id);
        self.visit_type_annotation_opt(a.return_type.as_ref(), id);
        match &a.body {
            &ArrowFunctionBody::Expression(e) => self.visit_expr(e, id),
            ArrowFunctionBody::BlockStatement(b) => self.visit_statements(b.body, id),
        }
        self.close(id);
    }

    #[inline]
    fn visit_class_expression(&mut self, c: &'a ClassExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::ClassExpression, span, parent, addr_of(c));
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
        t: &'a TaggedTemplateExpression,
        span: Span,
        parent: NodeId,
    ) {
        let id = self.add(NodeKind::TaggedTemplateExpression, span, parent, addr_of(t));
        self.visit_expr(t.tag, id);
        if let Some(ta) = &t.type_arguments {
            self.visit_type_args(ta, id);
        }
        self.visit_template_literal(&t.quasi, t.quasi_span, id);
        self.close(id);
    }

    #[inline]
    fn visit_await_expression(&mut self, a: &'a AwaitExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::AwaitExpression, span, parent, addr_of(a));
        self.visit_expr(a.argument, id);
        self.close(id);
    }

    #[inline]
    fn visit_yield_expression(&mut self, y: &'a YieldExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::YieldExpression, span, parent, addr_of(y));
        if let Some(a) = y.argument {
            self.visit_expr(a, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_sequence_expression(&mut self, s: &'a SequenceExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::SequenceExpression, span, parent, addr_of(s));
        self.visit_expr_list(s.expressions, id);
        self.close(id);
    }

    #[inline]
    fn visit_assignment_expression(
        &mut self,
        a: &'a AssignmentExpression,
        span: Span,
        parent: NodeId,
    ) {
        let id = self.add(NodeKind::AssignmentExpression, span, parent, addr_of(a));
        // `a.left` may be an Object/Array pattern (destructuring assignment)
        // — pattern-aware descent, never swallowed by a wildcard.
        self.visit_expr(a.left, id);
        self.visit_expr(a.right, id);
        self.close(id);
    }

    #[inline]
    fn visit_object_pattern(&mut self, op: &'a ObjectPattern, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::ObjectPattern, span, parent, addr_of(op));
        if let Some(decs) = op.decorators(self.view) {
            self.visit_decorators(decs, id);
        }
        self.visit_type_annotation_opt(op.type_annotation(self.view), id);
        for prop in self.view.pat_props(op.properties) {
            self.visit_object_pattern_property(prop, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_array_pattern(&mut self, ap: &'a ArrayPattern, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::ArrayPattern, span, parent, addr_of(ap));
        if let Some(decs) = ap.decorators(self.view) {
            self.visit_decorators(decs, id);
        }
        self.visit_type_annotation_opt(ap.type_annotation(self.view), id);
        for &el in self.view.expr_opt_list(ap.elements).iter().flatten() {
            self.visit_expr(el, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_assignment_pattern(&mut self, a: &'a AssignmentPattern, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::AssignmentPattern, span, parent, addr_of(a));
        if let Some(decs) = self.view.opt_decorators(a.decorators) {
            self.visit_decorators(decs, id);
        }
        self.visit_expr(a.left, id);
        self.visit_expr(a.right, id);
        self.close(id);
    }

    #[inline]
    fn visit_ts_type_assertion(&mut self, t: &'a TSTypeAssertion, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::TSTypeAssertion, span, parent, addr_of(t));
        self.visit_ty(t.type_annotation, id);
        self.visit_expr(t.expression, id);
        self.close(id);
    }

    #[inline]
    fn visit_ts_as_expression(&mut self, t: &'a TSAsExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::TSAsExpression, span, parent, addr_of(t));
        self.visit_expr(t.expression, id);
        self.visit_ty(t.type_annotation, id);
        self.close(id);
    }

    #[inline]
    fn visit_ts_satisfies_expression(
        &mut self,
        t: &'a TSSatisfiesExpression,
        span: Span,
        parent: NodeId,
    ) {
        let id = self.add(NodeKind::TSSatisfiesExpression, span, parent, addr_of(t));
        self.visit_expr(t.expression, id);
        self.visit_ty(t.type_annotation, id);
        self.close(id);
    }

    #[inline]
    fn visit_ts_instantiation_expression(
        &mut self,
        t: &'a TSInstantiationExpression,
        span: Span,
        parent: NodeId,
    ) {
        let id = self.add(
            NodeKind::TSInstantiationExpression,
            span,
            parent,
            addr_of(t),
        );
        self.visit_expr(t.expression, id);
        self.visit_type_args(&t.type_arguments, id);
        self.close(id);
    }

    #[inline]
    fn visit_ts_non_null_expression(
        &mut self,
        t: &'a TSNonNullExpression,
        span: Span,
        parent: NodeId,
    ) {
        let id = self.add(NodeKind::TSNonNullExpression, span, parent, addr_of(t));
        self.visit_expr(t.expression, id);
        self.close(id);
    }

    #[inline]
    fn visit_ts_parameter_property(
        &mut self,
        pp: &'a TSParameterProperty,
        span: Span,
        parent: NodeId,
    ) {
        let id = self.add(NodeKind::TSParameterProperty, span, parent, addr_of(pp));
        self.visit_expr(pp.parameter, id);
        self.close(id);
    }

    #[inline]
    fn visit_import_expression(&mut self, i: &'a ImportExpression, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::ImportExpression, span, parent, addr_of(i));
        self.visit_expr(i.source, id);
        if let Some(o) = i.options {
            self.visit_expr(o, id);
        }
        self.close(id);
    }

    #[inline]
    fn visit_meta_property(&mut self, m: &'a MetaProperty, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::MetaProperty, span, parent, addr_of(m));
        self.visit_identifier(&m.meta, id);
        self.visit_identifier(&m.property, id);
        self.close(id);
    }

    #[inline]
    fn visit_comment_bound_pair(&mut self, c: &'a CommentBoundPair, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::CommentBoundPair, span, parent, addr_of(c));
        self.visit_expr(c.inner, id);
        self.close(id);
    }

    #[inline]
    fn visit_parenthesized_expression(
        &mut self,
        p: &'a ParenthesizedExpression,
        span: Span,
        parent: NodeId,
    ) {
        let id = self.add(NodeKind::ParenthesizedExpression, span, parent, addr_of(p));
        self.visit_expr(p.expression, id);
        self.close(id);
    }

    pub(super) fn visit_function_expression(&mut self, f: &'a FunctionExpression, parent: NodeId) {
        let id = self.add(NodeKind::FunctionExpression, f.span, parent, addr_of(f));
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

    fn visit_object_property(&mut self, prop: &'a ObjectProperty, parent: NodeId) {
        match prop {
            ObjectProperty::Property(pr) => self.visit_property(pr, parent),
            ObjectProperty::SpreadElement(s) => self.visit_spread(s, parent),
        }
    }

    fn visit_object_pattern_property(&mut self, prop: &'a ObjectPatternProperty, parent: NodeId) {
        match prop {
            ObjectPatternProperty::Property(pr) => self.visit_property(pr, parent),
            ObjectPatternProperty::RestElement(r) => self.visit_rest_element(r, parent),
        }
    }

    fn visit_property(&mut self, pr: &'a Property, parent: NodeId) {
        let id = self.add(NodeKind::Property, pr.span, parent, addr_of(pr));
        self.visit_expr(pr.key, id);
        self.visit_expr(pr.value, id);
        self.close(id);
    }

    fn visit_spread(&mut self, s: &'a SpreadElement, parent: NodeId) {
        let id = self.add(NodeKind::SpreadElement, s.span, parent, addr_of(s));
        self.visit_expr(s.argument, id);
        self.close(id);
    }

    fn visit_rest_element(&mut self, r: &'a RestElement, parent: NodeId) {
        let id = self.add(NodeKind::RestElement, r.span, parent, addr_of(r));
        self.visit_type_annotation_opt(r.type_annotation(self.view), id);
        self.visit_expr(r.argument, id);
        self.close(id);
    }

    /// A template literal, `span` its own: the node keeps none (an expression's span
    /// is its wrapper's, a tagged template's its `quasi_span`).
    fn visit_template_literal(&mut self, t: &'a TemplateLiteral, span: Span, parent: NodeId) {
        let id = self.add(NodeKind::TemplateLiteral, span, parent, addr_of(t));
        for q in self.view.quasis(t.quasis) {
            self.visit_template_element(q, id);
        }
        self.visit_expr_list(t.expressions, id);
        self.close(id);
    }

    pub(super) fn visit_template_element(&mut self, q: &'a TemplateElement, parent: NodeId) {
        self.leaf(NodeKind::TemplateElement, q.span, addr_of(q), parent);
    }

    pub(super) fn visit_decorators(&mut self, decorators: &'a [Decorator], parent: NodeId) {
        for d in decorators {
            let id = self.add(NodeKind::Decorator, d.span, parent, addr_of(d));
            self.visit_expr(d.expression, id);
            self.close(id);
        }
    }

    // --- identifiers ---------------------------------------------------------

    /// Id an identifier, then descend the binding-only extras (parameter
    /// decorators + type annotation) it carries — both `None` for a reference, so
    /// this serves reference and binding positions alike.
    pub(super) fn visit_identifier(&mut self, ident: &'a Identifier, parent: NodeId) {
        let id = self.add(NodeKind::Identifier, ident.span, parent, addr_of(ident));
        if let Some(decs) = ident.decorators(self.view) {
            self.visit_decorators(decs, id);
        }
        if let Some(ann) = ident.type_annotation(self.view) {
            self.visit_type_annotation(ann, id);
        }
        self.close(id);
    }
}
