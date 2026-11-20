//! The expression-shaped bind-descent — `visit_expression` (function/arrow/
//! class-expression scopes, the pattern-aware nested descent) and
//! `bind_object_expression` (the object-literal member table). Contributes its
//! own `impl SymbolBinder` block; the struct and the scope helpers live in the
//! parent module. Purely a locality split — no behavior distinction.

use super::super::symbols::SymbolFlags;
use super::{DeclInput, SymbolBinder};
use crate::ids::NodeId;
use tsv_ts::ast::{
    ArrowFunctionBody, ExprId, Expression, ExpressionKind, ObjectExpression, ObjectProperty,
    PropertyKind, Run,
};

impl<'a> SymbolBinder<'a> {
    // --- expressions (nested scopes) -----------------------------------------

    /// Visit the expression `id` names, read in place from its store.
    pub(super) fn visit_expr(&mut self, id: ExprId) {
        self.visit_expression(self.view.expr(id));
    }

    /// Visit the expressions of an id run, in order.
    fn visit_expr_list(&mut self, list: Run<ExprId>) {
        for &e in self.view.expr_list(list) {
            self.visit_expr(e);
        }
    }

    pub(super) fn visit_expression(&mut self, expr: &'a Expression) {
        use ExpressionKind as E;
        let view = self.view;
        match &expr.kind {
            &E::FunctionExpression(f) => {
                let f = view.fn_expr(f);
                self.with_function_scope(f.type_parameters.as_ref(), |b| {
                    b.bind_params(f.params);
                    b.bind_statement_list(f.body.body, true);
                });
            }
            &E::ArrowFunctionExpression(a) => {
                let a = view.arrow_fn(a);
                self.with_function_scope(a.type_parameters.as_ref(), |b| {
                    b.bind_params(a.params);
                    match &a.body {
                        &ArrowFunctionBody::Expression(e) => b.visit_expr(e),
                        ArrowFunctionBody::BlockStatement(block) => {
                            b.bind_statement_list(block.body, true);
                        }
                    }
                });
            }
            &E::ClassExpression(c) => {
                let c = view.class_expr(c);
                let sym = c.id.as_ref().map(|_| {
                    let name = self.atoms.intern("__class");
                    self.new_symbol(SymbolFlags::CLASS, name)
                });
                self.bind_class_body(&c.body, sym, c.type_parameters.as_ref());
            }
            E::ParenthesizedExpression(p) => self.visit_expr(p.expression),
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
                self.visit_expr_list(c.arguments);
            }
            E::NewExpression(n) => {
                self.visit_expr(n.callee);
                self.visit_expr_list(n.arguments);
            }
            E::MemberExpression(m) => {
                self.visit_expr(m.object);
                self.visit_expr(m.property);
            }
            E::TSNonNullExpression(t) => self.visit_expr(t.expression),
            E::TSAsExpression(t) => self.visit_expr(t.expression),
            E::TSSatisfiesExpression(t) => self.visit_expr(t.expression),
            E::TSInstantiationExpression(t) => self.visit_expr(t.expression),
            E::SpreadElement(s) => self.visit_expr(s.argument),
            E::ArrayExpression(a) => {
                for &e in view.expr_opt_list(a.elements).iter().flatten() {
                    self.visit_expr(e);
                }
            }
            E::ObjectExpression(o) => self.bind_object_expression(o),
            E::TemplateLiteral(t) => self.visit_expr_list(t.expressions),
            &E::TaggedTemplateExpression(t) => {
                let t = view.tagged_tpl(t);
                self.visit_expr(t.tag);
                self.visit_expr_list(t.quasi.expressions);
            }
            _ => {}
        }
    }

    // --- object literals -----------------------------------------------------

    /// Bind an object literal's members into a fresh member table so duplicate
    /// members conflict. tsgo binds the literal an anonymous `ObjectLiteral`
    /// container; tsv builds the member table locally and swaps no scope — an
    /// object literal is not a `HasLocals` container, and nothing consumes the
    /// literal's symbol, so nested function/arrow *values* still open their own
    /// scope through the per-value [`Self::visit_expression`] recursion.
    ///
    /// The load-bearing choice is the object-literal-method exclude: it is the
    /// whole `Value` mask (tsgo `IsObjectLiteralMethod ? SymbolFlagsValue :
    /// SymbolFlagsMethodExcludes`), and `Value ⊇ Method`, so two same-named
    /// object-literal methods conflict — while class/interface methods
    /// (`METHOD_EXCLUDES`) keep their silent-merge untouched.
    ///
    /// tsgo: internal/binder/binder.go bindPropertyOrMethodOrAccessor
    ///       (KindObjectLiteralExpression member cases)
    fn bind_object_expression(&mut self, obj: &'a ObjectExpression) {
        let view = self.view;
        let table = self.new_table();
        for prop in view.obj_props(obj.properties) {
            match prop {
                ObjectProperty::Property(pr) => {
                    if let Some(key) = self.resolve_member_key(view.expr(pr.key), pr.computed, None)
                    {
                        let (inc, exc) = match pr.kind {
                            PropertyKind::Get => (
                                SymbolFlags::GET_ACCESSOR,
                                SymbolFlags::GET_ACCESSOR_EXCLUDES,
                            ),
                            PropertyKind::Set => (
                                SymbolFlags::SET_ACCESSOR,
                                SymbolFlags::SET_ACCESSOR_EXCLUDES,
                            ),
                            PropertyKind::Init if pr.method => {
                                (SymbolFlags::METHOD, SymbolFlags::VALUE)
                            }
                            PropertyKind::Init => {
                                (SymbolFlags::PROPERTY, SymbolFlags::PROPERTY_EXCLUDES)
                            }
                        };
                        let d = DeclInput {
                            name: key.key,
                            display: key.display,
                            error_span: key.span,
                            is_default_export: false,
                            is_export_assignment_default: false,
                            exported: false,
                            node: NodeId::FIRST,
                        };
                        self.declare_symbol(table, None, d, inc, exc);
                    }
                    self.visit_expr(pr.value);
                }
                ObjectProperty::SpreadElement(s) => self.visit_expr(s.argument),
            }
        }
    }
}
