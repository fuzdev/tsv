//! The expression-shaped flow visitors — `visit_expression`, the `bindCondition`
//! machinery (conditions / logical / conditional expressions), the function /
//! class-expression containers, and the pattern visitors. Contributes its own
//! `impl FlowBuilder` block; the struct and traversal driver live in the parent
//! module. Purely a locality split — no behavior distinction.

use super::super::*;
use super::FlowBuilder;
use super::predicates::*;
use crate::binder::{NodeKind, addr_of, expression_addr_kind};
use tsv_ts::ast::internal::{
    ArrowFunctionBody, AssignmentOperator, BinaryExpression, BinaryOperator, ClassExpression,
    ConditionalExpression, Decorator, Expression, FunctionExpression, Identifier,
    ObjectPatternProperty, ObjectProperty, Property, UnaryOperator,
};

impl<'a> FlowBuilder<'a> {
    /// `maybeBindExpressionFlowIfCall` (binder.go:2143): a top-level dotted-name
    /// (non-`super`) call is a potential assertion → `createFlowCall`.
    pub(super) fn maybe_bind_expression_flow_if_call(&mut self, expr: &Expression<'_>) {
        if let Expression::CallExpression(c) = expr
            && !matches!(c.callee, Expression::Super(_))
            && is_dotted_name(c.callee)
        {
            let call_id = self.require(addr_of(c), NodeKind::CallExpression);
            self.current_flow = self.create_flow_call(self.current_flow, call_id);
        }
    }

    // --- condition binding (the bindCondition machinery) ------------------

    /// `doWithConditionalBranches` (binder.go:1789) — bind `value` with the given
    /// true/false targets installed, restored on exit. A `None` value is the
    /// nil-node no-op (`for (;;)`).
    fn do_with_conditional_branches(
        &mut self,
        value: Option<&Expression<'_>>,
        true_target: FlowNodeId,
        false_target: FlowNodeId,
    ) {
        let saved_true = self.current_true_target;
        let saved_false = self.current_false_target;
        self.current_true_target = Some(true_target);
        self.current_false_target = Some(false_target);
        if let Some(v) = value {
            self.visit_expression(v);
        }
        self.current_true_target = saved_true;
        self.current_false_target = saved_false;
    }

    /// `bindCondition` (binder.go:1799) — bind the condition through the
    /// true/false targets, then, **only for an atomic condition** (not a logical
    /// `&&`/`||`/`??` or logical compound-assignment, whose sub-binder already
    /// wired the targets), add the true/false condition edges. `parent_is_nullish`
    /// is the caller-supplied `IsNullishCoalesce(parent)` guard (the operands of a
    /// `??`); `is_optional_chain_root` is always `false` here (optional chains are
    /// atomic — their dedicated short-circuit machinery is F2).
    pub(super) fn bind_condition(
        &mut self,
        node: Option<&Expression<'_>>,
        true_target: FlowNodeId,
        false_target: FlowNodeId,
        parent_is_nullish: bool,
    ) {
        self.do_with_conditional_branches(node, true_target, false_target);
        if node.is_none_or(|e| !is_logical_condition(e)) {
            let with_id = node.map(|e| (e, self.expr_id(e)));
            let is_narrowing = node.is_some_and(is_narrowing_expression);
            let tc = self.create_flow_condition(
                FlowFlags::TRUE_CONDITION,
                self.current_flow,
                with_id,
                is_narrowing,
                false,
                parent_is_nullish,
            );
            self.add_antecedent(true_target, tc);
            let fc = self.create_flow_condition(
                FlowFlags::FALSE_CONDITION,
                self.current_flow,
                with_id,
                is_narrowing,
                false,
                parent_is_nullish,
            );
            self.add_antecedent(false_target, fc);
        }
    }

    /// `bindBinaryExpressionFlow`'s logical branch (binder.go:2219) — decides
    /// value-context (top-level → a `hasFlowEffects` post-label materializes only
    /// if the subtree had flow effects) vs condition-context (nested → the
    /// enclosing true/false targets). The pointer-free
    /// `isTopLevelLogicalExpression` test is `current_true_target.is_none()` (see
    /// the module header). `assign_target` is the LHS for a logical
    /// compound-assignment (`&&=`/`||=`/`??=`), else `None`.
    fn bind_binary_expression_flow(
        &mut self,
        node: &Expression<'_>,
        left: &Expression<'_>,
        right: &Expression<'_>,
        is_and: bool,
        is_nullish: bool,
        assign_target: Option<&Expression<'_>>,
    ) {
        if self.current_true_target.is_none() {
            let post = self.create_branch_label();
            let saved_flow = self.current_flow;
            let saved_effects = self.has_flow_effects;
            self.has_flow_effects = false;
            self.bind_logical_like_expression(
                node,
                left,
                right,
                is_and,
                is_nullish,
                assign_target,
                post,
                post,
            );
            self.current_flow = if self.has_flow_effects {
                self.finish_flow_label(post)
            } else {
                saved_flow
            };
            self.has_flow_effects = self.has_flow_effects || saved_effects;
        } else {
            let t = self.current_true_target.unwrap_or(self.unreachable_flow);
            let f = self.current_false_target.unwrap_or(self.unreachable_flow);
            self.bind_logical_like_expression(
                node,
                left,
                right,
                is_and,
                is_nullish,
                assign_target,
                t,
                f,
            );
        }
    }

    /// `bindLogicalLikeExpression` (binder.go:2261) — narrow the left operand
    /// against a fresh `preRight` label (vs the false target for `&&`/`&&=`, vs
    /// the true target otherwise), then the right against the original targets;
    /// a logical compound-assignment additionally mutates its target and tests
    /// the whole node.
    #[allow(clippy::too_many_arguments)] // faithful port of the tsgo signature
    fn bind_logical_like_expression(
        &mut self,
        node: &Expression<'_>,
        left: &Expression<'_>,
        right: &Expression<'_>,
        is_and: bool,
        is_nullish: bool,
        assign_target: Option<&Expression<'_>>,
        true_target: FlowNodeId,
        false_target: FlowNodeId,
    ) {
        let pre_right = self.create_branch_label();
        if is_and {
            self.bind_condition(Some(left), pre_right, false_target, is_nullish);
        } else {
            self.bind_condition(Some(left), true_target, pre_right, is_nullish);
        }
        self.current_flow = self.finish_flow_label(pre_right);
        if let Some(target) = assign_target {
            // Logical compound-assignment (binder.go:2271-2275): bind the RHS, mutate
            // the target, then test `node` (never a boolean keyword → the parent-nullish
            // guard is irrelevant). tsgo binds the RHS with `doWithConditionalBranches`
            // (targets = the outer true/false), but the value-vs-condition split is then
            // taken by `isTopLevelLogicalExpression(right)` on `right`'s PARENT — which
            // is this `&&=`/`||=`/`??=` node, not a logical operator — so `right` is
            // classified TOP-LEVEL and its internal conditions never thread into the
            // outer targets (only the whole-node truthiness below does). tsv emulates
            // `isTopLevelLogicalExpression` pointer-free as `current_true_target.is_none()`,
            // so binding the RHS with the targets SET would misclassify a logical `right`
            // (`a &&= x && y`) as nested. The faithful adaptation is to CLEAR the targets
            // (not set them) around the RHS bind: a logical `right` then sees itself
            // top-level (its own discarded post-label), and a non-logical `right` is
            // identical either way (the targets are only read by the logical branch).
            let saved_true = self.current_true_target.take();
            let saved_false = self.current_false_target.take();
            self.visit_expression(right);
            self.current_true_target = saved_true;
            self.current_false_target = saved_false;
            self.bind_assignment_target_flow(target);
            let node_id = self.expr_id(node);
            let is_narrowing = is_narrowing_expression(node);
            let tc = self.create_flow_condition(
                FlowFlags::TRUE_CONDITION,
                self.current_flow,
                Some((node, node_id)),
                is_narrowing,
                false,
                false,
            );
            self.add_antecedent(true_target, tc);
            let fc = self.create_flow_condition(
                FlowFlags::FALSE_CONDITION,
                self.current_flow,
                Some((node, node_id)),
                is_narrowing,
                false,
                false,
            );
            self.add_antecedent(false_target, fc);
        } else {
            self.bind_condition(Some(right), true_target, false_target, is_nullish);
        }
    }

    /// `bindConditionalExpressionFlow` (binder.go:2289) — a `?:` as a value: the
    /// condition splits to true/false labels feeding the two arms, which merge at
    /// a `hasFlowEffects`-gated post label.
    fn bind_conditional_expression_flow(&mut self, c: &ConditionalExpression<'_>) {
        let true_label = self.create_branch_label();
        let false_label = self.create_branch_label();
        let post = self.create_branch_label();
        let saved_flow = self.current_flow;
        let saved_effects = self.has_flow_effects;
        self.has_flow_effects = false;
        self.bind_condition(Some(c.test), true_label, false_label, false);
        self.current_flow = self.finish_flow_label(true_label);
        self.visit_expression(c.consequent);
        self.add_antecedent(post, self.current_flow);
        self.current_flow = self.finish_flow_label(false_label);
        self.visit_expression(c.alternate);
        self.add_antecedent(post, self.current_flow);
        self.current_flow = if self.has_flow_effects {
            self.finish_flow_label(post)
        } else {
            saved_flow
        };
        self.has_flow_effects = self.has_flow_effects || saved_effects;
    }

    /// `bindAssignmentTargetFlow` (binder.go:1821), **default branch only**: a
    /// narrowable-reference target gets an `Assignment` mutation. The
    /// array/object-literal destructuring recursion (the `inAssignmentPattern`
    /// per-element machinery) is F2, alongside parameter-default forks — a
    /// destructuring target is not a narrowable reference, so it mints no mutation
    /// here, and its sub-references were already visited.
    pub(super) fn bind_assignment_target_flow(&mut self, target: &Expression<'_>) {
        if is_narrowable_reference(target) {
            let id = self.expr_id(target);
            self.current_flow =
                self.create_flow_mutation(FlowFlags::ASSIGNMENT, self.current_flow, id);
        }
    }

    /// The F0 [`NodeId`] of an expression node — its variant payload's
    /// `(address, kind)` in the address map, via the shared
    /// [`expression_addr_kind`] mapping (the same one `visit_expression`'s
    /// lockstep guard pins in `binder/mod.rs`). Condition / mutation subjects are
    /// always value expressions F0 lowered, so this never misses.
    fn expr_id(&self, e: &Expression<'_>) -> NodeId {
        let (addr, kind) = expression_addr_kind(e);
        self.require(addr, kind)
    }

    // --- function-like / class expression containers ----------------------

    /// A function expression. `is_iife` marks a call callee (an IIFE): the
    /// container is entered **transparently** (no fresh `Start`, `current_flow`
    /// not restored on exit) with its own return target, so the body joins the
    /// containing control flow (binder.go:1525-1544). The return-flow anchor
    /// stays off (`is_ctor_or_static = false`) — tsgo writes it only for
    /// constructors / static blocks, never a plain IIFE.
    fn visit_function_expression(
        &mut self,
        f: &FunctionExpression<'_>,
        node_id: NodeId,
        is_iife: bool,
    ) {
        // The function-expression flow write is captured at the OUTER flow,
        // before the body's Start (binder.go:915). Unconditional: the container
        // path does not nil it in dead code.
        self.set_flow_leaf(node_id);
        let saved = self.enter_container(Some(node_id), is_iife, is_iife);
        self.bind_params(f.params);
        self.visit_statement_list(f.body.body);
        self.exit_container(saved, is_iife, true, true, node_id, false);
    }

    fn visit_arrow(
        &mut self,
        a: &tsv_ts::ast::internal::ArrowFunctionExpression<'_>,
        node_id: NodeId,
        is_iife: bool,
    ) {
        self.set_flow_leaf(node_id); // binder.go:915 (arrows dispatch here too)
        let saved = self.enter_container(Some(node_id), is_iife, is_iife);
        self.bind_params(a.params);
        match &a.body {
            ArrowFunctionBody::Expression(e) => self.visit_expression(e),
            ArrowFunctionBody::BlockStatement(block) => self.visit_statement_list(block.body),
        }
        self.exit_container(saved, is_iife, true, true, node_id, false);
    }

    fn visit_class_expr(&mut self, c: &ClassExpression<'_>) {
        self.visit_class_common(
            c.id.as_ref(),
            c.decorators,
            c.super_class,
            c.body.body,
            true,
        );
    }

    // --- expressions ------------------------------------------------------

    pub(super) fn visit_expression(&mut self, expr: &Expression<'_>) {
        use Expression as E;
        // A **value** sub-position resets the condition targets, so a logical
        // expression nested inside one (`if (f(x && y))`, `if (c ? x && y : z)`,
        // `if (g([x && y]))`) is classified top-level — a value with its own
        // post-label — not a sub-condition of the enclosing `bind_condition`. This
        // is the pointer-free `isTopLevelLogicalExpression` (binder.go:2782): only
        // the *threading* variants (`!`, `&&`/`||`/`??`, logical-assignment, parens)
        // propagate the targets into their operands; every other expression is a
        // value boundary. Without the reset, `current_true_target.is_none()` stays
        // false through the whole condition subtree and mis-wires nested logicals.
        let restore = if is_condition_threading(expr) {
            None
        } else {
            Some((
                self.current_true_target.take(),
                self.current_false_target.take(),
            ))
        };
        match expr {
            E::Identifier(idn) => self.visit_identifier(idn),
            E::ThisExpression(t) => {
                let id = self.require(addr_of(t), NodeKind::ThisExpression);
                self.set_flow_leaf(id);
            }
            E::Super(s) => {
                let id = self.require(addr_of(s), NodeKind::Super);
                self.set_flow_leaf(id);
            }
            E::MetaProperty(m) => {
                // Non-leaf write (nil'd in dead code). tsv models `import`/`new`
                // and `meta`/`target` as identifiers; they are keyword-ish, not
                // references, so only the MetaProperty node is stamped.
                let id = self.require(addr_of(m), NodeKind::MetaProperty);
                self.set_flow_nonleaf(id);
            }
            E::MemberExpression(m) => self.bind_member_expression_flow(expr, m),
            E::Literal(_) | E::PrivateIdentifier(_) | E::RegexLiteral(_) => {}
            E::ObjectExpression(o) => {
                for prop in o.properties {
                    self.visit_object_property(prop);
                }
            }
            E::ArrayExpression(a) => {
                for el in a.elements.iter().flatten() {
                    self.visit_expression(el);
                }
            }
            E::UnaryExpression(u) if u.operator == UnaryOperator::Bang => {
                self.bind_prefix_unary_expression_flow(u);
            }
            E::UnaryExpression(u) => self.visit_expression(u.argument),
            E::UpdateExpression(u) => self.visit_expression(u.argument),
            E::BinaryExpression(b) if b.operator.is_logical() => {
                self.bind_logical_binary_expression_flow(expr, b);
            }
            E::BinaryExpression(b) => {
                self.visit_expression(b.left);
                self.visit_expression(b.right);
            }
            E::CallExpression(c) => self.bind_call_expression_flow(c),
            E::NewExpression(n) => self.visit_new_expression(n),
            E::ConditionalExpression(c) => self.bind_conditional_expression_flow(c),
            E::ArrowFunctionExpression(a) => {
                let id = self.require(addr_of(a), NodeKind::ArrowFunctionExpression);
                self.visit_arrow(a, id, false);
            }
            E::FunctionExpression(f) => {
                let id = self.require(addr_of(f), NodeKind::FunctionExpression);
                self.visit_function_expression(f, id, false);
            }
            E::ClassExpression(c) => self.visit_class_expr(c),
            E::SpreadElement(s) => self.visit_expression(s.argument),
            E::TemplateLiteral(t) => {
                for e in t.expressions {
                    self.visit_expression(e);
                }
            }
            E::TaggedTemplateExpression(t) => self.visit_tagged_template_expression(t),
            E::AwaitExpression(a) => self.visit_expression(a.argument),
            E::YieldExpression(y) => {
                if let Some(a) = y.argument {
                    self.visit_expression(a);
                }
            }
            E::SequenceExpression(s) => self.bind_sequence_expression_flow(s),
            E::AssignmentExpression(a) if is_logical_assign_op(a.operator) => {
                self.bind_logical_assignment_expression_flow(expr, a);
            }
            E::AssignmentExpression(a) => self.bind_assignment_expression_flow(a),
            E::ObjectPattern(op) => self.visit_object_pattern(op),
            E::ArrayPattern(ap) => self.visit_array_pattern(ap),
            E::AssignmentPattern(a) => self.visit_assignment_pattern(a),
            E::RestElement(r) => self.visit_expression(r.argument),
            E::TSTypeAssertion(t) => self.visit_expression(t.expression),
            E::TSAsExpression(t) => self.visit_expression(t.expression),
            E::TSSatisfiesExpression(t) => self.visit_expression(t.expression),
            E::TSInstantiationExpression(t) => self.visit_expression(t.expression),
            E::TSNonNullExpression(t) => self.visit_expression(t.expression),
            E::TSParameterProperty(pp) => self.visit_expression(pp.parameter),
            E::ImportExpression(i) => self.visit_import_expression(i),
            E::JsdocCast(c) => self.visit_expression(c.inner),
            E::ParenthesizedExpression(p) => self.visit_expression(p.expression),
        }
        if let Some((t, f)) = restore {
            self.current_true_target = t;
            self.current_false_target = f;
        }
    }

    #[inline]
    fn bind_member_expression_flow(
        &mut self,
        expr: &Expression<'_>,
        m: &tsv_ts::ast::internal::MemberExpression<'_>,
    ) {
        // The access flow write (binder.go:618): non-leaf, reachable-
        // only, gated on `isNarrowableReference`.
        if is_narrowable_reference(expr) {
            let id = self.require(addr_of(m), NodeKind::MemberExpression);
            self.set_flow_nonleaf(id);
        }
        self.visit_expression(m.object);
        self.visit_expression(m.property);
    }

    #[inline]
    fn bind_prefix_unary_expression_flow(
        &mut self,
        u: &tsv_ts::ast::internal::UnaryExpression<'_>,
    ) {
        // `bindPrefixUnaryExpressionFlow` (binder.go:2174): swap the
        // condition targets around the operand so `!x` narrows inversely.
        // The pre/post swaps are symmetric — any sub-binder restores the
        // targets to their entry value (via `do_with_conditional_branches`
        // / the `!`-swap), so the second swap is a faithful restore.
        std::mem::swap(
            &mut self.current_true_target,
            &mut self.current_false_target,
        );
        self.visit_expression(u.argument);
        std::mem::swap(
            &mut self.current_true_target,
            &mut self.current_false_target,
        );
    }

    #[inline]
    fn bind_logical_binary_expression_flow(
        &mut self,
        expr: &Expression<'_>,
        b: &BinaryExpression<'_>,
    ) {
        // `bindBinaryExpressionFlow` logical branch (binder.go:2219).
        let is_and = b.operator == BinaryOperator::AmpersandAmpersand;
        let is_nullish = b.operator == BinaryOperator::QuestionQuestion;
        self.bind_binary_expression_flow(expr, b.left, b.right, is_and, is_nullish, None);
    }

    #[inline]
    fn bind_call_expression_flow(&mut self, c: &tsv_ts::ast::internal::CallExpression<'_>) {
        use Expression as E;
        // IIFE detection (`GetImmediatelyInvokedFunctionExpression`,
        // utilities.go:1834; `bindCallExpressionFlow`, binder.go:2419):
        // a non-async (non-generator) function/arrow callee — through any
        // grouping parens — is inlined into the containing flow. Its
        // arguments bind FIRST so the callee's flow write captures the
        // post-argument flow.
        let mut unwrapped = c.callee;
        while let E::ParenthesizedExpression(p) = unwrapped {
            unwrapped = p.expression;
        }
        match unwrapped {
            E::ArrowFunctionExpression(a) if !a.r#async => {
                for arg in c.arguments {
                    self.visit_expression(arg);
                }
                let id = self.require(addr_of(a), NodeKind::ArrowFunctionExpression);
                self.visit_arrow(a, id, true);
            }
            E::FunctionExpression(f) if !f.r#async && !f.generator => {
                for arg in c.arguments {
                    self.visit_expression(arg);
                }
                let id = self.require(addr_of(f), NodeKind::FunctionExpression);
                self.visit_function_expression(f, id, true);
            }
            _ => {
                self.visit_expression(c.callee);
                for arg in c.arguments {
                    self.visit_expression(arg);
                }
            }
        }
    }

    #[inline]
    fn visit_new_expression(&mut self, n: &tsv_ts::ast::internal::NewExpression<'_>) {
        self.visit_expression(n.callee);
        for a in n.arguments {
            self.visit_expression(a);
        }
    }

    #[inline]
    fn visit_tagged_template_expression(
        &mut self,
        t: &tsv_ts::ast::internal::TaggedTemplateExpression<'_>,
    ) {
        self.visit_expression(t.tag);
        for e in t.quasi.expressions {
            self.visit_expression(e);
        }
    }

    #[inline]
    fn bind_sequence_expression_flow(&mut self, s: &tsv_ts::ast::internal::SequenceExpression<'_>) {
        // `bindBinaryExpressionFlow` comma branch: each operand's value
        // is discarded (statement-like), so a top-level dotted-name call
        // is a potential assertion — apply maybe-call per operand
        // (visit-then-maybe, like `ExpressionStatement`). tsgo nests
        // comma as left-assoc `BinaryExpression`s applying maybe-call to
        // both `Left`/`Right` at each level; the flattened form applies
        // it once per leaf operand (intermediate comma nodes are no-op
        // non-calls), so the effect matches.
        // tsgo: binder.go bindBinaryExpressionFlow (comma branch)
        for e in s.expressions {
            self.visit_expression(e);
            self.maybe_bind_expression_flow_if_call(e);
        }
    }

    #[inline]
    fn bind_logical_assignment_expression_flow(
        &mut self,
        expr: &Expression<'_>,
        a: &tsv_ts::ast::internal::AssignmentExpression<'_>,
    ) {
        // `bindBinaryExpressionFlow` logical compound-assignment branch.
        let is_and = a.operator == AssignmentOperator::LogicalAndAssign;
        let is_nullish = a.operator == AssignmentOperator::NullishAssign;
        self.bind_binary_expression_flow(expr, a.left, a.right, is_and, is_nullish, Some(a.left));
    }

    #[inline]
    fn bind_assignment_expression_flow(
        &mut self,
        a: &tsv_ts::ast::internal::AssignmentExpression<'_>,
    ) {
        // `bindBinaryExpressionFlow` assignment branch (binder.go:2249) —
        // bind operands, then the target's `Assignment` mutation.
        self.visit_expression(a.left);
        self.visit_expression(a.right);
        self.bind_assignment_target_flow(a.left);
    }

    #[inline]
    fn visit_object_pattern(&mut self, op: &tsv_ts::ast::internal::ObjectPattern<'_>) {
        self.visit_decorators(op.decorators);
        for prop in op.properties {
            self.visit_object_pattern_property(prop);
        }
    }

    #[inline]
    fn visit_array_pattern(&mut self, ap: &tsv_ts::ast::internal::ArrayPattern<'_>) {
        self.visit_decorators(ap.decorators);
        for el in ap.elements.iter().flatten() {
            self.visit_expression(el);
        }
    }

    #[inline]
    fn visit_assignment_pattern(&mut self, a: &tsv_ts::ast::internal::AssignmentPattern<'_>) {
        self.visit_decorators(a.decorators);
        self.visit_expression(a.left);
        self.visit_expression(a.right);
    }

    #[inline]
    fn visit_import_expression(&mut self, i: &tsv_ts::ast::internal::ImportExpression<'_>) {
        self.visit_expression(i.source);
        if let Some(o) = i.options {
            self.visit_expression(o);
        }
    }

    pub(super) fn visit_identifier(&mut self, ident: &Identifier<'_>) {
        // Identifier flow write (binder.go:602): a leaf — unconditional, so a
        // dead identifier keeps `Some(unreachable)`. Its decorators (parameter
        // decorators) are value expressions; its type annotation is a type
        // position (skipped).
        let id = self.require(addr_of(ident), NodeKind::Identifier);
        self.set_flow_leaf(id);
        self.visit_decorators(ident.decorators());
    }

    pub(super) fn visit_decorators(&mut self, decorators: Option<&[Decorator<'_>]>) {
        if let Some(decs) = decorators {
            for d in decs {
                self.visit_expression(&d.expression);
            }
        }
    }

    fn visit_object_property(&mut self, prop: &ObjectProperty<'_>) {
        match prop {
            ObjectProperty::Property(pr) => self.visit_object_expr_property(pr),
            ObjectProperty::SpreadElement(s) => self.visit_expression(s.argument),
        }
    }

    fn visit_object_expr_property(&mut self, pr: &Property<'_>) {
        let is_method_or_accessor =
            pr.method || pr.kind != tsv_ts::ast::internal::PropertyKind::Init;
        if let (true, Expression::FunctionExpression(f)) = (is_method_or_accessor, &pr.value) {
            // An object-literal method/accessor is a control-flow container
            // anchored on its value FunctionExpression — the body-bearing node
            // (unlike `MethodDefinition`, a `Property` does NOT share its value's
            // address, so this is a consistency choice with `visit_method`, not a
            // collision workaround). The PROPERTY node — tsv's analog of tsgo's
            // object-literal MethodDeclaration — gets the outer-flow write
            // (bindPropertyOrMethodOrAccessor, binder.go:981) and becomes the
            // body Start's subject (binder.go:1534) — the P3 narrowing hint
            // (`IsObjectLiteralOrClassExpressionMethodOrAccessor`,
            // utilities.go:566; the class-expression half lives in `visit_method`).
            self.visit_expression(&pr.key);
            let anchor = self.require(addr_of(f), NodeKind::FunctionExpression);
            let prop_id = self.require(addr_of(pr), NodeKind::Property);
            self.set_flow_leaf(prop_id);
            let saved = self.enter_container(Some(prop_id), false, false);
            self.bind_params(f.params);
            self.visit_statement_list(f.body.body);
            self.exit_container(saved, false, true, true, anchor, false);
        } else {
            self.visit_expression(&pr.key);
            self.visit_expression(&pr.value);
        }
    }

    fn visit_object_pattern_property(&mut self, prop: &ObjectPatternProperty<'_>) {
        match prop {
            ObjectPatternProperty::Property(pr) => {
                self.visit_expression(&pr.key);
                self.visit_expression(&pr.value);
            }
            ObjectPatternProperty::RestElement(r) => self.visit_expression(r.argument),
        }
    }
}
