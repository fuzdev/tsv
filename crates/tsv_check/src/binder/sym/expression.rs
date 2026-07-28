//! The expression-shaped bind-descent — `visit_expression` (function/arrow/
//! class-expression scopes, the pattern-aware nested descent) and
//! `bind_object_expression` (the object-literal member table). Contributes its
//! own `impl SymbolBinder` block; the struct and the scope helpers live in the
//! parent module. Purely a locality split — no behavior distinction.

use super::super::symbols::SymbolFlags;
use super::{DeclInput, SymbolBinder};
use crate::ids::NodeId;
use tsv_ts::ast::internal::{Expression, ObjectExpression, ObjectProperty, PropertyKind};

impl<'a> SymbolBinder<'a> {
    // --- expressions (nested scopes) -----------------------------------------

    pub(super) fn visit_expression(&mut self, expr: &Expression<'a>) {
        use Expression as E;
        match expr {
            E::FunctionExpression(f) => {
                self.with_function_scope(f.type_parameters.as_ref(), |b| {
                    b.bind_params(f.params);
                    b.bind_statement_list(f.body.body, true);
                });
            }
            E::ArrowFunctionExpression(a) => {
                self.with_function_scope(a.type_parameters.as_ref(), |b| {
                    b.bind_params(a.params);
                    match &a.body {
                        tsv_ts::ast::internal::ArrowFunctionBody::Expression(e) => {
                            b.visit_expression(e);
                        }
                        tsv_ts::ast::internal::ArrowFunctionBody::BlockStatement(block) => {
                            b.bind_statement_list(block.body, true);
                        }
                    }
                });
            }
            E::ClassExpression(c) => {
                let sym = c.id.as_ref().map(|_| {
                    let name = self.atoms.intern("__class");
                    self.new_symbol(SymbolFlags::CLASS, name)
                });
                self.bind_class_body(&c.body, sym, c.type_parameters.as_ref());
            }
            E::ParenthesizedExpression(p) => self.visit_expression(p.expression),
            E::UnaryExpression(u) => self.visit_expression(u.argument),
            E::UpdateExpression(u) => self.visit_expression(u.argument),
            E::AwaitExpression(a) => self.visit_expression(a.argument),
            E::YieldExpression(y) => {
                if let Some(a) = y.argument {
                    self.visit_expression(a);
                }
            }
            E::BinaryExpression(b) => {
                self.visit_expression(b.left);
                self.visit_expression(b.right);
            }
            E::AssignmentExpression(a) => {
                self.visit_expression(a.left);
                self.visit_expression(a.right);
            }
            E::ConditionalExpression(c) => {
                self.visit_expression(c.test);
                self.visit_expression(c.consequent);
                self.visit_expression(c.alternate);
            }
            E::SequenceExpression(s) => {
                for e in s.expressions {
                    self.visit_expression(e);
                }
            }
            E::CallExpression(c) => {
                self.visit_expression(c.callee);
                for a in c.arguments {
                    self.visit_expression(a);
                }
            }
            E::NewExpression(n) => {
                self.visit_expression(n.callee);
                for a in n.arguments {
                    self.visit_expression(a);
                }
            }
            E::MemberExpression(m) => {
                self.visit_expression(m.object);
                self.visit_expression(m.property);
            }
            E::TSNonNullExpression(t) => self.visit_expression(t.expression),
            E::TSAsExpression(t) => self.visit_expression(t.expression),
            E::TSSatisfiesExpression(t) => self.visit_expression(t.expression),
            E::TSInstantiationExpression(t) => self.visit_expression(t.expression),
            E::SpreadElement(s) => self.visit_expression(s.argument),
            E::ArrayExpression(a) => {
                for e in a.elements.iter().flatten() {
                    self.visit_expression(e);
                }
            }
            E::ObjectExpression(o) => self.bind_object_expression(o),
            E::TemplateLiteral(t) => {
                for e in t.expressions {
                    self.visit_expression(e);
                }
            }
            E::TaggedTemplateExpression(t) => {
                self.visit_expression(t.tag);
                for e in t.quasi.expressions {
                    self.visit_expression(e);
                }
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
    fn bind_object_expression(&mut self, obj: &ObjectExpression<'a>) {
        let table = self.new_table();
        for prop in obj.properties {
            match prop {
                ObjectProperty::Property(pr) => {
                    if let Some(key) = self.resolve_member_key(&pr.key, pr.computed, None) {
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
                    self.visit_expression(&pr.value);
                }
                ObjectProperty::SpreadElement(s) => self.visit_expression(s.argument),
            }
        }
    }
}
