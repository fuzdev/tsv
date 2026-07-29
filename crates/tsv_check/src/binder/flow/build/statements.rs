//! The statement-shaped flow visitors — `visit_statement` and the per-statement
//! flow shapers (conditions, loops, switch, try/finally, labeled statements),
//! plus the declaration-container descents. Contributes its own
//! `impl FlowBuilder` block; the struct and traversal driver live in the parent
//! module. Purely a locality split — no behavior distinction.

use super::super::*;
use super::predicates::*;
use super::{ActiveLabelEntry, FlowBuilder};
use crate::binder::{NodeKind, addr_of, statement_kind};
use smallvec::SmallVec;
use tsv_ts::ast::internal::{
    BreakStatement, ClassDeclaration, ClassMember, ContinueStatement, Decorator, DoWhileStatement,
    Expression, ForInOfLeft, ForInit, ForStatement, FunctionDeclaration, Identifier, IfStatement,
    LabeledStatement, MethodDefinition, MethodKind, ObjectPatternProperty, Statement, SwitchCase,
    SwitchStatement, TSModuleDeclarationBody, TryStatement, VariableDeclarator, WhileStatement,
};

impl<'a> FlowBuilder<'a> {
    // --- statements -------------------------------------------------------

    pub(super) fn visit_statement(&mut self, stmt: &Statement<'_>) {
        let id = self.require(addr_of(stmt), statement_kind(stmt));
        if self.current_unreachable() {
            // bindChildren dead path (binder.go:1651): the non-leaf statement's
            // flow attachment is nil (already `None`); mark potentially-
            // executable nodes; then descend generically (no flow shaping).
            if is_potentially_executable(stmt) {
                self.node_flags[id.index()] |= crate::binder::NODE_FLAGS_UNREACHABLE;
            }
            self.descend_children_generic(stmt);
            return;
        }
        // Reachable: statement-range nodes capture the entry flow before the
        // construct dispatches (binder.go:1663).
        if is_statement_range(stmt) {
            self.flow_of_node[id.index()] = Some(self.current_flow);
        }
        match stmt {
            Statement::ExpressionStatement(s) => {
                self.visit_expression(&s.expression);
                self.maybe_bind_expression_flow_if_call(&s.expression);
            }
            Statement::VariableDeclaration(d) => {
                for decl in d.declarations {
                    self.bind_variable_declaration_flow(decl);
                }
            }
            Statement::ReturnStatement(s) => {
                // `bindReturnStatement` (binder.go:1939).
                if let Some(a) = &s.argument {
                    self.visit_expression(a);
                }
                if let Some(rt) = self.current_return_target {
                    self.add_antecedent(rt, self.current_flow);
                }
                self.current_flow = self.unreachable_flow;
                self.has_explicit_return = true;
                self.has_flow_effects = true;
            }
            Statement::ThrowStatement(s) => {
                // `bindThrowStatement` (binder.go:1949).
                self.visit_expression(&s.argument);
                self.current_flow = self.unreachable_flow;
                self.has_flow_effects = true;
            }
            // --- F1b: branching control-flow topology ---------------------
            Statement::IfStatement(s) => self.bind_if_statement(s),
            Statement::WhileStatement(s) => self.bind_while_statement(id, s),
            Statement::DoWhileStatement(s) => self.bind_do_statement(id, s),
            Statement::ForStatement(s) => self.bind_for_statement(id, s),
            Statement::ForInStatement(s) => {
                self.bind_for_in_or_of(id, &s.left, &s.right, s.body);
            }
            Statement::ForOfStatement(s) => {
                self.bind_for_in_or_of(id, &s.left, &s.right, s.body);
            }
            Statement::BreakStatement(s) => self.bind_break_statement(s),
            Statement::ContinueStatement(s) => self.bind_continue_statement(s),
            Statement::SwitchStatement(s) => self.bind_switch_statement(id, s),
            Statement::TryStatement(s) => self.bind_try_statement(s),
            Statement::LabeledStatement(s) => self.bind_labeled_statement(s),
            // Everything else (declarations, blocks, exports, modules) threads
            // flow linearly through its children.
            _ => self.descend_children_generic(stmt),
        }
    }

    /// Descend a statement's value children threading `current_flow` linearly,
    /// with **no** flow shaping — the `bindEachChild` analog. Used by the
    /// **dead-code path** (where linear descent is correct — nothing is
    /// reachable) for every statement kind, and by the reachable `_` arm for the
    /// kinds without their own shaper (declarations, blocks, and the F2
    /// sequential placeholders — labeled / try / exports / modules). The
    /// branching arms below (`if` / the loops / `switch` / `break` / `continue`)
    /// are therefore reached **only in dead code**; the reachable topology lives
    /// in `visit_statement`. Containers nested here still open their own `Start`
    /// regions, so a function body stays reachable even in dead code.
    fn descend_children_generic(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::ExpressionStatement(s) => self.visit_expression(&s.expression),
            Statement::VariableDeclaration(d) => {
                for decl in d.declarations {
                    self.visit_expression(&decl.id);
                    if let Some(init) = &decl.init {
                        self.visit_expression(init);
                    }
                }
            }
            Statement::FunctionDeclaration(f) => {
                let id = self.require(addr_of(stmt), statement_kind(stmt));
                self.visit_function_declaration(f, id);
            }
            Statement::ClassDeclaration(c) => self.visit_class_decl(c),
            Statement::ReturnStatement(s) => {
                if let Some(a) = &s.argument {
                    self.visit_expression(a);
                }
            }
            Statement::ThrowStatement(s) => self.visit_expression(&s.argument),
            Statement::BlockStatement(b) => self.visit_statement_list(b.body),
            // --- dead-path linear descent for the branching kinds (their real
            //     topology lives in `visit_statement`; reached only when dead) ---
            Statement::IfStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.consequent);
                if let Some(alt) = s.alternate {
                    self.visit_statement(alt);
                }
            }
            Statement::ForStatement(s) => {
                match &s.init {
                    Some(ForInit::VariableDeclaration(d)) => {
                        for decl in d.declarations {
                            self.visit_expression(&decl.id);
                            if let Some(init) = &decl.init {
                                self.visit_expression(init);
                            }
                        }
                    }
                    Some(ForInit::Expression(e)) => self.visit_expression(e),
                    None => {}
                }
                if let Some(t) = &s.test {
                    self.visit_expression(t);
                }
                if let Some(u) = &s.update {
                    self.visit_expression(u);
                }
                self.visit_statement(s.body);
            }
            Statement::ForInStatement(s) => {
                self.visit_for_left(&s.left);
                self.visit_expression(&s.right);
                self.visit_statement(s.body);
            }
            Statement::ForOfStatement(s) => {
                self.visit_for_left(&s.left);
                self.visit_expression(&s.right);
                self.visit_statement(s.body);
            }
            Statement::WhileStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.body);
            }
            Statement::DoWhileStatement(s) => {
                self.visit_statement(s.body);
                self.visit_expression(&s.test);
            }
            Statement::SwitchStatement(s) => {
                self.visit_expression(&s.discriminant);
                for case in s.cases {
                    if let Some(t) = &case.test {
                        self.visit_expression(t);
                    }
                    self.visit_statement_list(case.consequent);
                }
            }
            Statement::TryStatement(s) => {
                self.visit_statement_list(s.block.body);
                if let Some(handler) = &s.handler {
                    if let Some(param) = &handler.param {
                        self.visit_expression(param);
                    }
                    self.visit_statement_list(handler.body.body);
                }
                if let Some(finalizer) = &s.finalizer {
                    self.visit_statement_list(finalizer.body);
                }
            }
            Statement::LabeledStatement(s) => {
                // Dead-path fallback; the reachable topology lives in
                // `bind_labeled_statement`.
                self.visit_identifier(&s.label);
                self.visit_statement(s.body);
            }
            Statement::BreakStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label);
                }
            }
            Statement::ContinueStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label);
                }
            }
            Statement::ExportNamedDeclaration(e) => {
                if let Some(inner) = e.declaration {
                    self.visit_statement(inner);
                }
                // export specifiers / source are non-value (skipped).
            }
            Statement::ExportDefaultDeclaration(e) => self.visit_export_default(e),
            Statement::TSExportAssignment(ea) => self.visit_expression(&ea.expression),
            Statement::TSModuleDeclaration(m) => self.visit_module(m),
            // No value content (types / imports / enum bodies / empty): skipped,
            // per the "types are not descended" scope note. See module docs.
            Statement::TSTypeAliasDeclaration(_)
            | Statement::TSInterfaceDeclaration(_)
            | Statement::TSDeclareFunction(_)
            | Statement::TSEnumDeclaration(_)
            | Statement::ImportDeclaration(_)
            | Statement::TSImportEqualsDeclaration(_)
            | Statement::ExportAllDeclaration(_)
            | Statement::TSNamespaceExportDeclaration(_)
            | Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_) => {}
        }
    }

    fn visit_for_left(&mut self, left: &ForInOfLeft<'_>) {
        use ForInOfLeft as L;
        match left {
            L::VariableDeclaration(d) => {
                for decl in d.declarations {
                    self.visit_expression(&decl.id);
                    if let Some(init) = &decl.init {
                        self.visit_expression(init);
                    }
                }
            }
            L::Pattern(e) => self.visit_expression(e),
        }
    }

    // --- statement flow shapers -------------------------------------------

    /// `bindVariableDeclarationFlow` + `bindInitializedVariableFlow`
    /// (binder.go:2314) — a `var/let/const x = e` with an initializer emits one
    /// unconditional `Assignment`. The name binds as a **binding target**
    /// (`bind_binding_target`), so a destructuring default (`let {a = e} = …`)
    /// forks per `bindInitializer`. A destructuring pattern emits one
    /// `Assignment` per declarator (tsv has no binding-element node — see the
    /// module scope note).
    fn bind_variable_declaration_flow(&mut self, decl: &VariableDeclarator<'_>) {
        self.bind_binding_target(&decl.id);
        if let Some(init) = &decl.init {
            self.visit_expression(init);
        }
        if decl.init.is_some() {
            let decl_id = self.require(addr_of(decl), NodeKind::VariableDeclarator);
            self.current_flow =
                self.create_flow_mutation(FlowFlags::ASSIGNMENT, self.current_flow, decl_id);
        }
    }

    // --- branching statement flow shapers ---------------------------------

    /// `bindIfStatement` (binder.go:1924) — then/else branch labels merge at
    /// `postIf`; each branch binds against the condition-split flow.
    fn bind_if_statement(&mut self, s: &IfStatement<'_>) {
        let then_label = self.create_branch_label();
        let else_label = self.create_branch_label();
        let post_if = self.create_branch_label();
        self.bind_condition(Some(&s.test), then_label, else_label, false);
        self.current_flow = self.finish_flow_label(then_label);
        self.visit_statement(s.consequent);
        self.add_antecedent(post_if, self.current_flow);
        self.current_flow = self.finish_flow_label(else_label);
        if let Some(alt) = s.alternate {
            self.visit_statement(alt);
        }
        self.add_antecedent(post_if, self.current_flow);
        self.current_flow = self.finish_flow_label(post_if);
    }

    /// `bindWhileStatement` (binder.go:1857) — the entry edge is added to the
    /// loop label **before** it becomes `current_flow`; the back edge **after**
    /// the body.
    fn bind_while_statement(&mut self, stmt_id: NodeId, s: &WhileStatement<'_>) {
        let loop_label = self.create_loop_label();
        let pre_while = self.set_continue_target(stmt_id, loop_label);
        let pre_body = self.create_branch_label();
        let post_while = self.create_branch_label();
        self.add_antecedent(pre_while, self.current_flow); // entry edge (first)
        self.current_flow = pre_while;
        self.bind_condition(Some(&s.test), pre_body, post_while, false);
        self.current_flow = self.finish_flow_label(pre_body);
        self.bind_iterative_statement(s.body, post_while, pre_while);
        self.add_antecedent(pre_while, self.current_flow); // back edge (after)
        self.current_flow = self.finish_flow_label(post_while);
    }

    /// `bindDoStatement` (binder.go:1871) — the body runs from the loop label
    /// first; the continue target is a **pre-condition** branch label (not the
    /// loop label), and the condition loops back to the loop label.
    fn bind_do_statement(&mut self, stmt_id: NodeId, s: &DoWhileStatement<'_>) {
        let pre_do = self.create_loop_label();
        let condition_label = self.create_branch_label();
        let pre_condition = self.set_continue_target(stmt_id, condition_label);
        let post_do = self.create_branch_label();
        self.add_antecedent(pre_do, self.current_flow);
        self.current_flow = pre_do;
        self.bind_iterative_statement(s.body, post_do, pre_condition);
        self.add_antecedent(pre_condition, self.current_flow);
        self.current_flow = self.finish_flow_label(pre_condition);
        self.bind_condition(Some(&s.test), pre_do, post_do, false);
        self.current_flow = self.finish_flow_label(post_do);
    }

    /// `bindForStatement` (binder.go:1885) — init → loop label → condition →
    /// body (continue = the increment label) → incrementor → back edge.
    fn bind_for_statement(&mut self, stmt_id: NodeId, s: &ForStatement<'_>) {
        let loop_label = self.create_loop_label();
        let pre_loop = self.set_continue_target(stmt_id, loop_label);
        let pre_body = self.create_branch_label();
        let pre_increment = self.create_branch_label();
        let post_loop = self.create_branch_label();
        match &s.init {
            Some(ForInit::VariableDeclaration(d)) => {
                for decl in d.declarations {
                    self.bind_variable_declaration_flow(decl);
                }
            }
            Some(ForInit::Expression(e)) => self.visit_expression(e),
            None => {}
        }
        self.add_antecedent(pre_loop, self.current_flow);
        self.current_flow = pre_loop;
        // A nil condition is a true passthrough / false-unreachable, handled by
        // `create_flow_condition`'s nil-expression arm.
        self.bind_condition(s.test.as_ref(), pre_body, post_loop, false);
        self.current_flow = self.finish_flow_label(pre_body);
        self.bind_iterative_statement(s.body, post_loop, pre_increment);
        self.add_antecedent(pre_increment, self.current_flow);
        self.current_flow = self.finish_flow_label(pre_increment);
        if let Some(u) = &s.update {
            self.visit_expression(u);
        }
        self.add_antecedent(pre_loop, self.current_flow); // back edge
        self.current_flow = self.finish_flow_label(post_loop);
    }

    /// `bindForInOrOfStatement` (binder.go:1904). The exit edge is
    /// **unconditional** (a for-in/of can exit after zero iterations); continue
    /// targets the loop label. Shared by `for-in` and `for-of` (the for-of
    /// `await` modifier is a `bool` in tsv — no node to bind, no fork).
    fn bind_for_in_or_of(
        &mut self,
        stmt_id: NodeId,
        left: &ForInOfLeft<'_>,
        right: &Expression<'_>,
        body: &Statement<'_>,
    ) {
        let loop_label = self.create_loop_label();
        let pre_loop = self.set_continue_target(stmt_id, loop_label);
        let post_loop = self.create_branch_label();
        self.visit_expression(right);
        self.add_antecedent(pre_loop, self.current_flow);
        self.current_flow = pre_loop;
        self.add_antecedent(post_loop, self.current_flow); // unconditional exit
        // Bind the initializer (binder.go:1915-1918). A declaration-list variable
        // is assigned each iteration (`bindVariableDeclarationFlow`'s for-in/of
        // guard, binder.go:2316 — the `Assignment` mutation even with no
        // initializer); a pattern initializer runs `bindAssignmentTargetFlow`.
        match left {
            ForInOfLeft::VariableDeclaration(d) => {
                for decl in d.declarations {
                    self.bind_binding_target(&decl.id);
                    if let Some(init) = &decl.init {
                        self.visit_expression(init);
                    }
                    let decl_id = self.require(addr_of(decl), NodeKind::VariableDeclarator);
                    self.current_flow = self.create_flow_mutation(
                        FlowFlags::ASSIGNMENT,
                        self.current_flow,
                        decl_id,
                    );
                }
            }
            ForInOfLeft::Pattern(p) => {
                self.visit_expression(p);
                self.bind_assignment_target_flow(p);
            }
        }
        self.bind_iterative_statement(body, post_loop, pre_loop);
        self.add_antecedent(pre_loop, self.current_flow); // back edge
        self.current_flow = self.finish_flow_label(post_loop);
    }

    /// `setContinueTarget` (binder.go:1779) — walk the parent chain up from a
    /// loop while each parent is a `LabeledStatement`, assigning that label's
    /// continue target (so `continue L` on a labeled loop lands on the loop's
    /// continue point), in lockstep with the active-label stack from its top. No
    /// enclosing labeled statements → a no-op returning `target` unchanged.
    fn set_continue_target(&mut self, loop_node: NodeId, target: FlowNodeId) -> FlowNodeId {
        let mut node = loop_node;
        let mut i = self.active_label_list.len();
        loop {
            let Some(parent) = self.bound.parents[node.index()] else {
                break;
            };
            if self.bound.kinds[parent.index()] != NodeKind::LabeledStatement || i == 0 {
                break;
            }
            i -= 1;
            self.active_label_list[i].continue_target = Some(target);
            node = parent;
        }
        target
    }

    /// `bindIterativeStatement` (binder.go:1807) — bind a loop body with its
    /// break/continue targets installed, restored on exit.
    fn bind_iterative_statement(
        &mut self,
        body: &Statement<'_>,
        break_target: FlowNodeId,
        continue_target: FlowNodeId,
    ) {
        let save_break = self.current_break_target;
        let save_continue = self.current_continue_target;
        self.current_break_target = Some(break_target);
        self.current_continue_target = Some(continue_target);
        self.visit_statement(body);
        self.current_break_target = save_break;
        self.current_continue_target = save_continue;
    }

    /// `bindBreakStatement` (binder.go:1955) — a labeled `break L` resolves to
    /// `L`'s **break** target (`findActiveLabel`, marking it referenced); an
    /// unlabeled `break` uses `current_break_target`. An unresolved label is a
    /// no-op (deferred diagnostic).
    fn bind_break_statement(&mut self, s: &BreakStatement<'_>) {
        match &s.label {
            None => {
                let target = self.current_break_target;
                self.bind_break_or_continue_flow(target);
            }
            Some(label) => {
                self.visit_identifier(label);
                let name = self.label_text(label);
                if let Some(i) = self.find_active_label(name) {
                    self.active_label_list[i].referenced = true;
                    let target = Some(self.active_label_list[i].break_target);
                    self.bind_break_or_continue_flow(target);
                }
            }
        }
    }

    /// `bindContinueStatement` (binder.go:1959) — a labeled `continue L` resolves
    /// to `L`'s **continue** target; an unlabeled `continue` uses
    /// `current_continue_target`. A missing/`None` target is a no-op.
    fn bind_continue_statement(&mut self, s: &ContinueStatement<'_>) {
        match &s.label {
            None => {
                let target = self.current_continue_target;
                self.bind_break_or_continue_flow(target);
            }
            Some(label) => {
                self.visit_identifier(label);
                let name = self.label_text(label);
                if let Some(i) = self.find_active_label(name) {
                    self.active_label_list[i].referenced = true;
                    let target = self.active_label_list[i].continue_target;
                    self.bind_break_or_continue_flow(target);
                }
            }
        }
    }

    /// `bindBreakOrContinueFlow` (binder.go:1985) — route to the target and go
    /// unreachable; a `None` target (break/continue outside any loop/switch) is a
    /// no-op (the parser accepts it; the illegal-jump diagnostic is F3+).
    fn bind_break_or_continue_flow(&mut self, target: Option<FlowNodeId>) {
        if let Some(t) = target {
            self.add_antecedent(t, self.current_flow);
            self.current_flow = self.unreachable_flow;
            self.has_flow_effects = true;
        }
    }

    /// `bindSwitchStatement` (binder.go:2074) — a `switch` with a **local**
    /// post-switch break target (so a contained `break` resolves here, not at an
    /// enclosing loop) and the real clause topology (`bind_case_block`). When no
    /// clause is a `default`, an **unconditional** `(0, 0)` `SwitchClause`
    /// exhaustiveness sentinel — "no clause matched" — feeds the post-switch
    /// label alongside the case-block exit. `preSwitchCaseFlow` is captured
    /// **after** the discriminant is bound (the flow every clause forks from) and
    /// saved/restored here, as in tsgo (it is not in the container save set).
    fn bind_switch_statement(&mut self, switch_id: NodeId, s: &SwitchStatement<'_>) {
        let post_switch = self.create_branch_label();
        self.visit_expression(&s.discriminant);
        let save_break = self.current_break_target;
        let save_pre_switch = self.pre_switch_case_flow;
        self.current_break_target = Some(post_switch);
        self.pre_switch_case_flow = Some(self.current_flow);
        self.bind_case_block(switch_id, s);
        self.add_antecedent(post_switch, self.current_flow);
        let has_default = s.cases.iter().any(|c| c.test.is_none());
        if !has_default {
            // The "no clause matched" fall-off: reachable from the switch head
            // regardless of narrowing (an empty `(0, 0)` range is the sentinel).
            let pre_switch = self.pre_switch_case_flow.unwrap_or(self.unreachable_flow);
            let sentinel = self.create_flow_switch_clause(pre_switch, switch_id, 0, 0);
            self.add_antecedent(post_switch, sentinel);
        }
        self.current_break_target = save_break;
        self.pre_switch_case_flow = save_pre_switch;
        self.current_flow = self.finish_flow_label(post_switch);
    }

    /// `bindCaseBlock` (binder.go:2095) — thread the clauses. Each clause's
    /// `preCase` label is fed **from the switch head** (`preSwitchCaseFlow`,
    /// unconditionally — a narrowing switch wraps it in a per-clause
    /// `SwitchClause` node) plus the prior clause's fallthrough edge, so a clause
    /// reached only after a prior `break`/`return` stays reachable (the F2a
    /// reachability fix). An empty-clause run (`case a: case b:` with no
    /// statements) re-points to the head only when nothing live falls into it.
    fn bind_case_block(&mut self, switch_id: NodeId, s: &SwitchStatement<'_>) {
        let clauses = s.cases;
        let is_narrowing_switch =
            is_true_keyword(&s.discriminant) || is_narrowing_expression(&s.discriminant);
        let last = clauses.len().wrapping_sub(1);
        let mut fallthrough_flow = self.unreachable_flow;
        let mut i = 0;
        while i < clauses.len() {
            let clause_start = i as u32;
            // Empty-clause run: advance past clauses with no statements (bar the
            // last), re-pointing to the head only when nothing live falls in.
            while clauses[i].consequent.is_empty() && i + 1 < clauses.len() {
                if fallthrough_flow == self.unreachable_flow {
                    self.current_flow = self.pre_switch_case_flow.unwrap_or(self.unreachable_flow);
                }
                self.bind_case_or_default_clause(&clauses[i]);
                i += 1;
            }
            let pre_case = self.create_branch_label();
            let pre_switch = self.pre_switch_case_flow.unwrap_or(self.unreachable_flow);
            let pre_case_flow = if is_narrowing_switch {
                self.create_flow_switch_clause(pre_switch, switch_id, clause_start, i as u32 + 1)
            } else {
                pre_switch
            };
            self.add_antecedent(pre_case, pre_case_flow); // head edge (reachability fix)
            self.add_antecedent(pre_case, fallthrough_flow); // fallthrough (unreachable = no-op)
            self.current_flow = self.finish_flow_label(pre_case);
            self.bind_case_or_default_clause(&clauses[i]);
            fallthrough_flow = self.current_flow;
            if !self.current_unreachable() && i != last {
                let clause_id = self.require(addr_of(&clauses[i]), NodeKind::SwitchCase);
                self.fallthrough_flow.push((clause_id, self.current_flow));
            }
            i += 1;
        }
    }

    /// `bindCaseOrDefaultClause` (binder.go:2126) — the clause's test expression
    /// binds under the switch head (`preSwitchCaseFlow`, saved/restored), its
    /// statements under the current (post-`preCase`) flow.
    fn bind_case_or_default_clause(&mut self, case: &SwitchCase<'_>) {
        if let Some(test) = &case.test {
            let saved = self.current_flow;
            self.current_flow = self.pre_switch_case_flow.unwrap_or(self.unreachable_flow);
            self.visit_expression(test);
            self.current_flow = saved;
        }
        self.visit_statement_list(case.consequent);
    }

    // --- try / catch / finally --------------------------------------------

    /// A snapshot of a label's pending antecedent list (`label.Antecedents`) —
    /// the try/finally combine reads three of these directly (the pointer-free
    /// `combineFlowLists` analog).
    fn scratch_snapshot(&self, label: FlowNodeId) -> SmallVec<[FlowNodeId; 4]> {
        self.label_scratch.get(&label).cloned().unwrap_or_default()
    }

    /// `bindTryStatement` (binder.go:1993). Three fresh labels — `normalExit`,
    /// `returnLabel`, `exceptionLabel` — thread the "any instruction can throw"
    /// edge (`exceptionLabel` seeded from `current_flow` **before** the try
    /// block, `current_exception_target` repointed so `create_flow_mutation`'s
    /// fan-out comes alive). A catch is bound as a **second try** (a fresh
    /// `exceptionLabel` seeded from the first one's finish). With a finally, the
    /// finally label's antecedents = `normal ++ exception ++ return`
    /// (`combineFlowLists`), it becomes `current_flow`, and up to three
    /// `ReduceLabel`s route the finally's completion back through the return /
    /// outer-exception / normal-exit subsets (binder.go:2052-2067).
    fn bind_try_statement(&mut self, s: &TryStatement<'_>) {
        let save_return_target = self.current_return_target;
        let save_exception_target = self.current_exception_target;
        let normal_exit = self.create_branch_label();
        let return_label = self.create_branch_label();
        let mut exception_label = self.create_branch_label();
        if s.finalizer.is_some() {
            self.current_return_target = Some(return_label);
        }
        // The exception edge for exceptions before any mutation.
        self.add_antecedent(exception_label, self.current_flow);
        self.current_exception_target = Some(exception_label);
        self.visit_statement_list(s.block.body);
        self.add_antecedent(normal_exit, self.current_flow);
        if let Some(handler) = &s.handler {
            // The catch is the target of exceptions from the try block; its own
            // exceptions flow to a fresh label (catch = a second try).
            self.current_flow = self.finish_flow_label(exception_label);
            exception_label = self.create_branch_label();
            self.add_antecedent(exception_label, self.current_flow);
            self.current_exception_target = Some(exception_label);
            if let Some(param) = &handler.param {
                // The catch variable is a binding position (tsgo reaches it via
                // bindBindingElementFlow → bindInitializer), so a flow-changing
                // destructuring default forks — bind_binding_target, not the plain
                // value walk. Equivalent for a non-defaulted param.
                self.bind_binding_target(param);
            }
            self.visit_statement_list(handler.body.body);
            self.add_antecedent(normal_exit, self.current_flow);
        }
        // Restore BEFORE the finally — the finally isn't inside its own try.
        self.current_return_target = save_return_target;
        self.current_exception_target = save_exception_target;
        if let Some(finalizer) = &s.finalizer {
            let normal_list = self.scratch_snapshot(normal_exit);
            let exception_list = self.scratch_snapshot(exception_label);
            let return_list = self.scratch_snapshot(return_label);
            let finally_label = self.create_branch_label();
            // finallyLabel.Antecedents = normal ++ exception ++ return
            // (combineFlowLists, no dedup — faithful to binder.go:2043).
            let mut combined: SmallVec<[FlowNodeId; 4]> = SmallVec::new();
            combined.extend(normal_list.iter().copied());
            combined.extend(exception_list.iter().copied());
            combined.extend(return_list.iter().copied());
            self.label_scratch.insert(finally_label, combined);
            self.current_flow = finally_label;
            self.visit_statement_list(finalizer.body);
            if self.current_unreachable() {
                // An unreachable end-of-finally makes the whole try unreachable.
                self.current_flow = self.unreachable_flow;
            } else {
                // Route the finally's completion back through the return-only
                // subset (IIFE/constructor return target), then the outer
                // exception-only subset, then continue via the normal subset.
                if let Some(rt) = self.current_return_target
                    && !return_list.is_empty()
                {
                    let rl =
                        self.create_reduce_label(finally_label, &return_list, self.current_flow);
                    self.add_antecedent(rt, rl);
                }
                if let Some(et) = self.current_exception_target
                    && !exception_list.is_empty()
                {
                    let el =
                        self.create_reduce_label(finally_label, &exception_list, self.current_flow);
                    self.add_antecedent(et, el);
                }
                if normal_list.is_empty() {
                    self.current_flow = self.unreachable_flow;
                } else {
                    self.current_flow =
                        self.create_reduce_label(finally_label, &normal_list, self.current_flow);
                }
            }
        } else {
            self.current_flow = self.finish_flow_label(normal_exit);
        }
    }

    // --- labeled statements -----------------------------------------------

    /// `bindLabeledStatement` (binder.go:2153). Push an active-label entry
    /// (break target = `postStatementLabel`, continue target set later by a
    /// directly-enclosed loop's `set_continue_target`), bind the label + body,
    /// then pop; an **unreferenced** label gets the `Unreachable` stamp on its
    /// identifier (the TS7028 signal, binder.go:2167). The post label merges the
    /// body's exit.
    fn bind_labeled_statement(&mut self, s: &LabeledStatement<'_>) {
        let post = self.create_branch_label();
        let label_id = self.require(addr_of(&s.label), NodeKind::Identifier);
        self.active_label_list.push(ActiveLabelEntry {
            break_target: post,
            continue_target: None,
            referenced: false,
            label_node_id: label_id,
        });
        self.visit_identifier(&s.label);
        self.visit_statement(s.body);
        // Balanced with the push above (pop is always `Some`). An unreferenced
        // label's identifier gets the `Unreachable` stamp (the TS7028 signal).
        if let Some(entry) = self.active_label_list.pop()
            && !entry.referenced
        {
            self.node_flags[entry.label_node_id.index()] |= crate::binder::NODE_FLAGS_UNREACHABLE;
        }
        self.add_antecedent(post, self.current_flow);
        self.current_flow = self.finish_flow_label(post);
    }

    /// `findActiveLabel` (binder.go:1976) — innermost-first (the stack top is the
    /// last element, so scan from the end). Returns the stack index.
    fn find_active_label(&self, name: &str) -> Option<usize> {
        self.active_label_list
            .iter()
            .rposition(|e| self.bound.spans[e.label_node_id.index()].extract(self.source) == name)
    }

    /// The source text of a label identifier (the break/continue label name).
    fn label_text(&self, ident: &Identifier<'_>) -> &'a str {
        let id = self.require(addr_of(ident), NodeKind::Identifier);
        self.bound.spans[id.index()].extract(self.source)
    }

    // --- containers -------------------------------------------------------

    fn visit_function_declaration(&mut self, f: &FunctionDeclaration<'_>, anchor: NodeId) {
        let saved = self.enter_container(None, false, false);
        self.bind_params(f.params);
        self.visit_statement_list(f.body.body);
        self.exit_container(saved, false, true, true, anchor, false);
    }

    pub(super) fn bind_params(&mut self, params: &[Expression<'_>]) {
        for param in params {
            self.bind_binding_target(param);
        }
    }

    /// `bindInitializer` (binder.go:2474) — bind a parameter / binding-element
    /// **default** and fork `current_flow` around it, but **only** when binding
    /// the default actually changed the flow (a `BindingElement`/`Parameter` has
    /// no side effects when its initializer isn't evaluated — GH#49759). The
    /// entry/exit pointer-equality guard is exact: a literal default (`= 1`)
    /// leaves `current_flow` untouched and mints no label.
    fn bind_initializer(&mut self, initializer: &Expression<'_>) {
        let entry = self.current_flow;
        self.visit_expression(initializer);
        if entry == self.unreachable_flow || entry == self.current_flow {
            return;
        }
        let exit = self.create_branch_label();
        self.add_antecedent(exit, entry);
        self.add_antecedent(exit, self.current_flow);
        self.current_flow = self.finish_flow_label(exit);
    }

    /// Bind a **binding target** (declaration / parameter position):
    /// `bindParameterFlow` / `bindBindingElementFlow` (binder.go:2463/2450). A
    /// defaulted element's initializer is bound **before** the name (TC39 order,
    /// via `bind_initializer`, which forks only when the default changed the
    /// flow). Distinct from the value traversal (`visit_expression`) so the
    /// assignment-target destructuring recursion — a separate deferred item —
    /// stays untouched; for a non-defaulted target the two are equivalent.
    fn bind_binding_target(&mut self, node: &Expression<'_>) {
        use Expression as E;
        match node {
            E::AssignmentPattern(a) => {
                self.visit_decorators(a.decorators);
                self.bind_initializer(a.right);
                self.bind_binding_target(a.left);
            }
            E::ObjectPattern(op) => {
                self.visit_decorators(op.decorators);
                for prop in op.properties {
                    match prop {
                        ObjectPatternProperty::Property(pr) => {
                            self.visit_expression(&pr.key);
                            self.bind_binding_target(&pr.value);
                        }
                        ObjectPatternProperty::RestElement(r) => {
                            self.bind_binding_target(r.argument);
                        }
                    }
                }
            }
            E::ArrayPattern(ap) => {
                self.visit_decorators(ap.decorators);
                for el in ap.elements.iter().flatten() {
                    self.bind_binding_target(el);
                }
            }
            E::RestElement(r) => self.bind_binding_target(r.argument),
            E::TSParameterProperty(pp) => self.bind_binding_target(pp.parameter),
            // A plain identifier / other leaf binding: the ordinary traversal.
            _ => self.visit_expression(node),
        }
    }

    fn visit_class_decl(&mut self, c: &ClassDeclaration<'_>) {
        self.visit_class_common(
            c.id.as_ref(),
            c.decorators,
            c.super_class,
            c.body.body,
            false,
        );
    }

    /// The value-flow class descent shared by the declaration and expression forms
    /// (distinct types with the same field shape): the name binding, decorators, and
    /// the `extends` expression, then each member. Type positions (type params /
    /// super type args / `implements`) are skipped. `is_class_expression` threads
    /// the parent-kind half of tsgo's
    /// `IsObjectLiteralOrClassExpressionMethodOrAccessor` gate (utilities.go:566)
    /// down to `visit_method` — tsv expressions carry no parent pointer.
    pub(super) fn visit_class_common(
        &mut self,
        name: Option<&Identifier<'_>>,
        decorators: Option<&[Decorator<'_>]>,
        super_class: Option<&Expression<'_>>,
        members: &[ClassMember<'_>],
        is_class_expression: bool,
    ) {
        if let Some(name) = name {
            self.visit_identifier(name);
        }
        self.visit_decorators(decorators);
        if let Some(sc) = super_class {
            self.visit_expression(sc);
        }
        for member in members {
            self.visit_class_member(member, is_class_expression);
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember<'_>, is_class_expression: bool) {
        match member {
            ClassMember::MethodDefinition(m) => self.visit_method(m, is_class_expression),
            ClassMember::PropertyDefinition(p) => {
                self.visit_decorators(p.decorators);
                self.visit_expression(&p.key);
                // property type annotation is a type position (skip).
                if let Some(value) = &p.value {
                    // A property-with-initializer is a control-flow container
                    // (binder.go:2584): fresh Start around the initializer.
                    let p_id = self.require(addr_of(p), NodeKind::PropertyDefinition);
                    let saved = self.enter_container(None, false, false);
                    self.visit_expression(value);
                    self.exit_container(saved, false, false, false, p_id, false);
                }
            }
            ClassMember::StaticBlock(s) => {
                // A class static block is flow-transparent (binder.go:1525-1528)
                // with its own return target; `return_flow` anchors on it.
                let s_id = self.require(addr_of(s), NodeKind::StaticBlock);
                let saved = self.enter_container(None, true, true);
                self.visit_statement_list(s.body);
                self.exit_container(saved, true, true, true, s_id, true);
            }
            // index signatures are type-only (skip).
            ClassMember::IndexSignature(_) => {}
        }
    }

    fn visit_method(&mut self, m: &MethodDefinition<'_>, is_class_expression: bool) {
        self.visit_decorators(m.decorators);
        let is_ctor = m.kind == MethodKind::Constructor;
        self.visit_expression(&m.key);
        // The method body lives in `value` (a FunctionExpression); the method is
        // a control-flow container anchored on that FunctionExpression — the
        // body-bearing node (tsv wraps a method body in a FunctionExpression,
        // where tsc's method node holds the body directly). The `MethodDefinition`
        // and its inline `value` share an address (a repr reorder puts `value` at
        // offset 0), so the address map keys on `(address, NodeKind)`; anchoring
        // here resolves the FunctionExpression id via its kind, and the method
        // itself resolves separately by `NodeKind::MethodDefinition`.
        let anchor = self.require(addr_of(&m.value), NodeKind::FunctionExpression);
        // A **class-expression** method/accessor (never a constructor, never a
        // class-declaration member) gets the outer-flow write on the METHOD node
        // (bindPropertyOrMethodOrAccessor, binder.go:981) and becomes the body
        // Start's subject (binder.go:1534) — the P3 narrowing hint
        // (`IsObjectLiteralOrClassExpressionMethodOrAccessor`, utilities.go:566;
        // the object-literal half lives in `visit_object_expr_property`).
        let start_subject = if is_class_expression && !is_ctor {
            let method_id = self.require(addr_of(m), NodeKind::MethodDefinition);
            self.set_flow_leaf(method_id);
            Some(method_id)
        } else {
            None
        };
        let saved = self.enter_container(start_subject, false, is_ctor);
        self.bind_params(m.value.params);
        self.visit_statement_list(m.value.body.body);
        self.exit_container(saved, false, true, true, anchor, is_ctor);
    }

    fn visit_module(&mut self, m: &tsv_ts::ast::internal::TSModuleDeclaration<'_>) {
        use tsv_ts::ast::internal::TSModuleName;
        if let TSModuleName::Identifier(name) = &m.id {
            self.visit_identifier(name);
        }
        match &m.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                // A ModuleBlock is a control-flow container (binder.go:2582) —
                // fresh Start, no return target, not function-like.
                let block_id = self.require(addr_of(block), NodeKind::TSModuleBlock);
                let saved = self.enter_container(None, false, false);
                self.visit_statement_list(block.body);
                self.exit_container(saved, false, false, false, block_id, false);
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                self.visit_module(nested);
            }
            None => {}
        }
    }

    fn visit_export_default(&mut self, e: &tsv_ts::ast::internal::ExportDefaultDeclaration<'_>) {
        use tsv_ts::ast::internal::ExportDefaultValue as V;
        match &e.declaration {
            V::Expression(expr) => self.visit_expression(expr),
            V::FunctionDeclaration(f) => {
                let id = self.require(addr_of(f), NodeKind::FunctionDeclaration);
                self.visit_function_declaration(f, id);
            }
            V::ClassDeclaration(c) => self.visit_class_decl(c),
            // A declare function / interface has no value body (skip).
            V::TSDeclareFunction(_) | V::TSInterfaceDeclaration(_) => {}
        }
    }
}
