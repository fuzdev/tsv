//! The `needs_context` analysis: does the component require the
//! `$$renderer.component(($$renderer) => …)` wrapper?
//!
//! Ports Svelte's phase-2 `needs_context` accumulation — the flag the server
//! transform reads (`should_inject_context = dev || analysis.needs_context`) to
//! decide whether to wrap the whole component body. The oracle sets it,
//! monotonically, walking the **entire un-folded** instance + template AST, when
//! it sees any of:
//!
//! - a `new` expression (`NewExpression.js` sets it unconditionally), or
//! - a member/call whose root is **unsafe** per `is_safe_identifier`: the root
//!   (walking down `.object`) is not a plain identifier, or is a binding whose
//!   `declaration_kind` is `import` or whose `kind` is `prop`/`bindable_prop`/
//!   `rest_prop`. A plain local (`normal`), a global (no binding), and rune
//!   bindings (`state`/`derived`/`each`/…) are all safe.
//!
//! (`$effect`/`$bindable` also set `needs_context` in the oracle; the effect
//! path already forces the wrapper via `has_effects`, and a `$bindable` prop
//! forces it via the collected bindable set in `compile_server`, so neither is
//! re-derived here.)
//!
//! This port folds props + imports into `context_roots`. Because the oracle's
//! check is scope-sensitive but this port is name-based, a member/call rooted at
//! a `context_root` that is **also bound in some nested scope** (`shadowed`) is
//! genuinely ambiguous — the specific use might resolve to the shadow — so it
//! **refuses** rather than risk an over- or under-wrap. Every other case is
//! decided exactly: an unshadowed context-root member/call triggers; a
//! local/global member/call does not.
//!
//! The matches are exhaustive on purpose — a new `Statement`/`Expression`/
//! `FragmentNode` variant fails compilation here instead of silently slipping
//! past the analysis.

use tsv_svelte::ast::internal::{
    AttributeNode, AwaitBlock, ConstTag, EachBlock, Element, Fragment, FragmentNode, HtmlTag,
    IfBlock, KeyBlock, RenderTag, Root, SnippetBlock, SpecialElement, SpecialElementKind,
};
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ClassBody, ClassMember, ExportDefaultValue, Expression, ForInOfLeft,
    ForInit, FunctionExpression, ObjectPatternProperty, ObjectProperty, Statement,
    VariableDeclaration,
};

use crate::analyze::{NameSet, RuneInit, classify_rune_init, pattern_binding_names};
use crate::attr_refs::{
    each_attribute_expression, each_reference_bearing_attribute_expression,
    special_element_reference_expression,
};
use crate::snippet_emit::render_call_expression;
use crate::{CompileError, Refusal};

/// The accumulating analysis state.
struct Nc<'a> {
    source: &'a str,
    /// Prop + import names — the roots whose member/call access is unsafe.
    context_roots: &'a NameSet,
    /// Names bound anywhere below the component's top-level instance scope.
    shadowed: NameSet,
    /// Context-root names observed as the root of a member/call access.
    member_roots: NameSet,
    /// Set by a `new` expression or a non-identifier member/call root.
    needs: bool,
    /// A shape whose classification isn't portable (an escaped identifier root).
    refuse: Option<Refusal>,
    /// Names reassigned/updated anywhere in the component — collected during the
    /// same walk so mutations inside dropped event handlers still mark a binding
    /// updated (and so it is not statically folded).
    reassigned: NameSet,
    /// Names declared anywhere inside a function-like subtree (params + local
    /// declarations; `fn_depth > 0`). An assignment target inside such a subtree
    /// may resolve to the local, not the component binding, so a component
    /// binding in this set must go `Opaque` (refuse-on-read) rather than trust
    /// the shadow-naive `reassigned` mark — the same envelope the script side
    /// uses for its `nested_declared` names.
    fn_declared: NameSet,
    /// Current function-like nesting depth (arrows, function expressions and
    /// declarations, class methods, static blocks).
    fn_depth: u32,
    /// Set by any `$$slots` reference (the oracle's `uses_slots`): the component
    /// gains `const $$slots = $.sanitize_slots($$props)` and the `$$props` param.
    uses_slots: bool,
    /// Whether the walk is inside a **dropped** `{:catch}` subtree, where the
    /// emitter never walks the fragment so the emission refusals that let the
    /// default attribute traversal skip element spreads / directives / `{@attach}`
    /// never fire. There a `new`/prop-rooted access in such a position must still
    /// trigger the wrapper, so those positions are walked. Sticky for the whole
    /// catch subtree.
    in_dropped_catch: bool,
}

/// The whole-component analysis product consumed by the server transform.
pub(crate) struct ComponentContext {
    /// Whether the component needs the `$$renderer.component(…)` wrapper.
    pub needs_context: bool,
    /// Names reassigned anywhere in the component (script + template, including
    /// inside dropped event handlers).
    pub reassigned: NameSet,
    /// Names declared inside function-like subtrees anywhere in the component.
    /// A same-named component binding must be marked `Opaque` — a `reassigned`
    /// mark for it may belong to the shadowing local, and folding OR escaping on
    /// that guess would each miscompile some shape, so reads refuse instead.
    pub fn_declared: NameSet,
    /// Whether the component references `$$slots` (oracle's `uses_slots`).
    pub uses_slots: bool,
}

/// Analyze the component for the `$$renderer.component(…)` wrapper decision and
/// the component-wide reassignment set, in one walk.
///
/// `needs_context` is `true` when a wrapper trigger is proven, `false` when
/// proven absent; the walk returns `Err(Unsupported)` when a shape's
/// classification can't be pinned to the oracle (a shadowed context-root
/// member/call, or an escaped root). `reassigned` names every binding mutated
/// anywhere in the component — including inside dropped event handlers, which
/// the server transform needs so a mutated binding is not statically folded.
///
/// `instance_body` is the **type-erased** instance-script statement list (see
/// `erase`), not `root.instance.content.body` — the un-erased tree still carries
/// TypeScript nodes the walk must never see.
pub(crate) fn analyze_component(
    root: &Root<'_>,
    source: &str,
    instance_body: &[Statement<'_>],
) -> Result<ComponentContext, CompileError> {
    let mut context_roots = NameSet::default();
    collect_context_roots(instance_body, source, &mut context_roots);

    let mut nc = Nc {
        source,
        context_roots: &context_roots,
        shadowed: NameSet::default(),
        member_roots: NameSet::default(),
        needs: false,
        refuse: None,
        reassigned: NameSet::default(),
        fn_declared: NameSet::default(),
        fn_depth: 0,
        uses_slots: false,
        in_dropped_catch: false,
    };

    for stmt in instance_body {
        walk_stmt(stmt, &mut nc, false);
    }
    walk_fragment(&root.fragment, &mut nc);

    if let Some(reason) = nc.refuse {
        return Err(unsupported(reason));
    }
    for name in &nc.member_roots {
        if nc.shadowed.contains(name) {
            return Err(unsupported(Refusal::MemberCallAmbiguousRoot {
                name: name.clone(),
            }));
        }
    }
    if !nc.member_roots.is_empty() {
        nc.needs = true;
    }
    Ok(ComponentContext {
        needs_context: nc.needs,
        reassigned: nc.reassigned,
        fn_declared: nc.fn_declared,
        uses_slots: nc.uses_slots,
    })
}

fn unsupported(reason: Refusal) -> CompileError {
    CompileError::Unsupported(reason)
}

/// The plain (non-escaped) name of an identifier, `None` for a unicode-escaped
/// or synthetic identifier.
fn plain_name<'s>(id: &tsv_ts::ast::internal::Identifier<'_>, source: &'s str) -> Option<&'s str> {
    if id.escaped_name.is_some() {
        return None;
    }
    let start = id.span.start as usize;
    Some(&source[start..start + id.name_len as usize])
}

/// Collect the top-level prop (incl. rest-prop) and import names into
/// `context_roots` — the roots whose member/call access sets `needs_context`.
fn collect_context_roots(instance_body: &[Statement<'_>], source: &str, out: &mut NameSet) {
    for stmt in instance_body {
        match stmt {
            Statement::ImportDeclaration(import) => {
                use tsv_ts::ast::internal::ImportSpecifier;
                for spec in import.specifiers {
                    let local = match spec {
                        ImportSpecifier::Default(s) => &s.local,
                        ImportSpecifier::Named(s) => &s.local,
                        ImportSpecifier::Namespace(s) => &s.local,
                    };
                    if let Some(name) = plain_name(local, source) {
                        out.insert(name.to_string());
                    }
                }
            }
            Statement::VariableDeclaration(decl) => {
                for declarator in decl.declarations {
                    let is_props = declarator
                        .init
                        .as_ref()
                        .and_then(|init| classify_rune_init(init, source))
                        .is_some_and(|r| matches!(r, RuneInit::Props));
                    if is_props {
                        let mut names = Vec::new();
                        // Best-effort: a malformed props pattern is refused later
                        // by the binding analysis; here we simply record what we
                        // can resolve.
                        let _ = pattern_binding_names(&declarator.id, source, &mut names);
                        for name in names {
                            out.insert(name);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Add every name a binding pattern declares to `shadowed` (best-effort — a
/// pattern shape `pattern_binding_names` can't collect just records nothing, so
/// the set stays a superset of the true nested bindings). Inside a function-like
/// subtree the names also join `fn_declared` (see the field docs).
fn declare_pattern(pattern: &Expression<'_>, nc: &mut Nc<'_>) {
    let mut names = Vec::new();
    if pattern_binding_names(pattern, nc.source, &mut names).is_ok() {
        for name in names {
            if nc.fn_depth > 0 {
                nc.fn_declared.insert(name.clone());
            }
            nc.shadowed.insert(name);
        }
    }
}

/// Add a single identifier's name to `shadowed` (and `fn_declared` inside a
/// function-like subtree).
fn declare_ident(id: &tsv_ts::ast::internal::Identifier<'_>, nc: &mut Nc<'_>) {
    if let Some(name) = plain_name(id, nc.source) {
        if nc.fn_depth > 0 {
            nc.fn_declared.insert(name.to_string());
        }
        nc.shadowed.insert(name.to_string());
    }
}

/// Peel to the root node Svelte's `is_safe_identifier` would see: `.object` down
/// member chains, plus the wrappers tsv's AST carries that the oracle's ESTree
/// view does not.
///
/// The TypeScript wrappers are **not** defense in depth here, they are
/// load-bearing: this walk runs over the raw `root.fragment` (the Svelte AST is
/// never rebuilt — template erasure happens per-expression at the emitter's
/// borrow points), so a template member/call still carries them when
/// `needs_context` classifies it. Missing an arm makes a *safe* root (a plain
/// local, a `$state` binding, a block local, a global) read as a non-identifier
/// and spuriously fire `needs_context` — wrapping the whole body in
/// `$$renderer.component(…)` the oracle never emits. A silent MISMATCH, not a
/// refusal. `JsdocCast` is the sixth transparent wrapper (valid JavaScript, and
/// the oracle has no such node at all).
fn root_of<'e>(expr: &'e Expression<'e>) -> &'e Expression<'e> {
    let mut node = expr;
    loop {
        match node {
            Expression::MemberExpression(m) => node = m.object,
            Expression::ParenthesizedExpression(p) => node = p.expression,
            Expression::TSAsExpression(t) => node = t.expression,
            Expression::TSSatisfiesExpression(t) => node = t.expression,
            Expression::TSNonNullExpression(t) => node = t.expression,
            Expression::TSTypeAssertion(t) => node = t.expression,
            Expression::TSInstantiationExpression(t) => node = t.expression,
            Expression::JsdocCast(j) => node = j.inner,
            _ => return node,
        }
    }
}

/// Classify a member/call access by its root (`is_safe_identifier`): a
/// non-identifier root is unsafe (→ `needs`); a plain-identifier root that is a
/// context-root is recorded (resolved against `shadowed` at the end); an escaped
/// root can't be classified (→ refuse).
fn check_root(access: &Expression<'_>, nc: &mut Nc<'_>) {
    match root_of(access) {
        Expression::Identifier(id) => match plain_name(id, nc.source) {
            Some(name) => {
                if nc.context_roots.contains(name) {
                    nc.member_roots.insert(name.to_string());
                }
            }
            None => {
                if nc.refuse.is_none() {
                    nc.refuse = Some(Refusal::MemberCallEscapedRoot);
                }
            }
        },
        _ => nc.needs = true,
    }
}

fn walk_exprs(exprs: &[Expression<'_>], nc: &mut Nc<'_>) {
    for expr in exprs {
        walk_expr(expr, nc);
    }
}

fn walk_opt(expr: Option<&Expression<'_>>, nc: &mut Nc<'_>) {
    if let Some(expr) = expr {
        walk_expr(expr, nc);
    }
}

/// Walk an expression: detect `new`/unsafe-member/unsafe-call triggers, and
/// collect any nested function/arrow/class bindings into `shadowed`.
fn walk_expr(expr: &Expression<'_>, nc: &mut Nc<'_>) {
    match expr {
        Expression::NewExpression(new_expr) => {
            nc.needs = true;
            walk_expr(new_expr.callee, nc);
            walk_exprs(new_expr.arguments, nc);
        }
        Expression::CallExpression(call) => {
            check_root(call.callee, nc);
            walk_expr(call.callee, nc);
            walk_exprs(call.arguments, nc);
        }
        Expression::MemberExpression(member) => {
            check_root(expr, nc);
            walk_expr(member.object, nc);
            if member.computed {
                walk_expr(member.property, nc);
            }
        }

        // Nested function scopes: their params/bindings shadow the component
        // scope, so record them and walk the body (always nested).
        Expression::ArrowFunctionExpression(a) => {
            nc.fn_depth += 1;
            for param in a.params {
                declare_pattern(param, nc);
                walk_expr(param, nc);
            }
            match &a.body {
                ArrowFunctionBody::Expression(e) => walk_expr(e, nc),
                ArrowFunctionBody::BlockStatement(b) => {
                    for stmt in b.body {
                        walk_stmt(stmt, nc, true);
                    }
                }
            }
            nc.fn_depth -= 1;
        }
        Expression::FunctionExpression(f) => walk_function_expression(f, nc),
        Expression::ClassExpression(c) => walk_class_body(&c.body, nc),

        // A bare identifier reference: detect `$$slots` (the oracle's
        // `uses_slots`), otherwise a leaf.
        Expression::Identifier(id) => {
            if plain_name(id, nc.source) == Some("$$slots") {
                nc.uses_slots = true;
            }
        }
        // Leaves — no children, no bindings.
        Expression::Literal(_)
        | Expression::PrivateIdentifier(_)
        | Expression::RegexLiteral(_)
        | Expression::ThisExpression(_)
        | Expression::Super(_)
        | Expression::MetaProperty(_) => {}

        Expression::ObjectExpression(obj) => {
            for prop in obj.properties {
                match prop {
                    ObjectProperty::Property(p) => {
                        if p.computed {
                            walk_expr(&p.key, nc);
                        }
                        walk_expr(&p.value, nc);
                    }
                    ObjectProperty::SpreadElement(s) => walk_expr(s.argument, nc),
                }
            }
        }
        Expression::ArrayExpression(arr) => {
            for element in arr.elements {
                walk_opt(element.as_ref(), nc);
            }
        }
        Expression::UnaryExpression(u) => walk_expr(u.argument, nc),
        Expression::UpdateExpression(u) => {
            crate::rune_guard::assign_target_roots(u.argument, nc.source, &mut nc.reassigned);
            walk_expr(u.argument, nc);
        }
        Expression::BinaryExpression(b) => {
            walk_expr(b.left, nc);
            walk_expr(b.right, nc);
        }
        Expression::ConditionalExpression(c) => {
            walk_expr(c.test, nc);
            walk_expr(c.consequent, nc);
            walk_expr(c.alternate, nc);
        }
        Expression::SpreadElement(s) => walk_expr(s.argument, nc),
        Expression::TemplateLiteral(t) => walk_exprs(t.expressions, nc),
        Expression::TaggedTemplateExpression(t) => {
            walk_expr(t.tag, nc);
            walk_exprs(t.quasi.expressions, nc);
        }
        Expression::AwaitExpression(a) => walk_expr(a.argument, nc),
        Expression::YieldExpression(y) => walk_opt(y.argument, nc),
        Expression::SequenceExpression(s) => walk_exprs(s.expressions, nc),
        Expression::AssignmentExpression(a) => {
            crate::rune_guard::assign_target_roots(a.left, nc.source, &mut nc.reassigned);
            walk_expr(a.left, nc);
            walk_expr(a.right, nc);
        }
        Expression::ObjectPattern(p) => {
            for prop in p.properties {
                match prop {
                    ObjectPatternProperty::Property(prop) => {
                        if prop.computed {
                            walk_expr(&prop.key, nc);
                        }
                        walk_expr(&prop.value, nc);
                    }
                    ObjectPatternProperty::RestElement(rest) => walk_expr(rest.argument, nc),
                }
            }
        }
        Expression::ArrayPattern(p) => {
            for element in p.elements {
                walk_opt(element.as_ref(), nc);
            }
        }
        Expression::AssignmentPattern(p) => {
            walk_expr(p.left, nc);
            walk_expr(p.right, nc);
        }
        Expression::RestElement(r) => walk_expr(r.argument, nc),
        Expression::TSTypeAssertion(t) => walk_expr(t.expression, nc),
        Expression::TSAsExpression(t) => walk_expr(t.expression, nc),
        Expression::TSSatisfiesExpression(t) => walk_expr(t.expression, nc),
        Expression::TSInstantiationExpression(t) => walk_expr(t.expression, nc),
        Expression::TSNonNullExpression(t) => walk_expr(t.expression, nc),
        Expression::TSParameterProperty(t) => walk_expr(t.parameter, nc),
        Expression::ImportExpression(i) => {
            walk_expr(i.source, nc);
            walk_opt(i.options, nc);
        }
        Expression::JsdocCast(j) => walk_expr(j.inner, nc),
        Expression::ParenthesizedExpression(p) => walk_expr(p.expression, nc),
    }
}

fn walk_function_expression(f: &FunctionExpression<'_>, nc: &mut Nc<'_>) {
    nc.fn_depth += 1;
    if let Some(id) = &f.id {
        declare_ident(id, nc);
    }
    for param in f.params {
        declare_pattern(param, nc);
        walk_expr(param, nc);
    }
    for stmt in f.body.body {
        walk_stmt(stmt, nc, true);
    }
    nc.fn_depth -= 1;
}

fn walk_class_body(body: &ClassBody<'_>, nc: &mut Nc<'_>) {
    for member in body.body {
        match member {
            ClassMember::MethodDefinition(m) => {
                if m.computed {
                    walk_expr(&m.key, nc);
                }
                walk_function_expression(&m.value, nc);
            }
            ClassMember::PropertyDefinition(p) => {
                if p.computed {
                    walk_expr(&p.key, nc);
                }
                walk_opt(p.value.as_ref(), nc);
            }
            ClassMember::StaticBlock(b) => {
                nc.fn_depth += 1;
                for stmt in b.body {
                    walk_stmt(stmt, nc, true);
                }
                nc.fn_depth -= 1;
            }
            ClassMember::IndexSignature(_) => {}
        }
    }
}

/// Walk a variable declaration: at nested depth its pattern names shadow the
/// component scope; the init/default expressions are always trigger-checked.
fn walk_var_decl(decl: &VariableDeclaration<'_>, nc: &mut Nc<'_>, shadow: bool) {
    for declarator in decl.declarations {
        if shadow {
            declare_pattern(&declarator.id, nc);
        }
        walk_expr(&declarator.id, nc);
        walk_opt(declarator.init.as_ref(), nc);
    }
}

fn walk_for_left(left: &ForInOfLeft<'_>, nc: &mut Nc<'_>) {
    match left {
        ForInOfLeft::VariableDeclaration(decl) => walk_var_decl(decl, nc, true),
        ForInOfLeft::Pattern(pattern) => walk_expr(pattern, nc),
    }
}

/// Walk a statement. `shadow` is false only for the component's top-level
/// instance statements — where a declaration's own name is a component binding,
/// not a shadow; everywhere else (nested scopes, template) it is true.
fn walk_stmt(stmt: &Statement<'_>, nc: &mut Nc<'_>, shadow: bool) {
    match stmt {
        Statement::VariableDeclaration(d) => walk_var_decl(d, nc, shadow),
        Statement::FunctionDeclaration(f) => {
            if shadow && let Some(id) = &f.id {
                declare_ident(id, nc);
            }
            nc.fn_depth += 1;
            for param in f.params {
                declare_pattern(param, nc);
                walk_expr(param, nc);
            }
            for s in f.body.body {
                walk_stmt(s, nc, true);
            }
            nc.fn_depth -= 1;
        }
        Statement::ClassDeclaration(c) => {
            if shadow && let Some(id) = &c.id {
                declare_ident(id, nc);
            }
            walk_class_body(&c.body, nc);
        }
        Statement::ExpressionStatement(s) => walk_expr(&s.expression, nc),
        Statement::ReturnStatement(s) => walk_opt(s.argument.as_ref(), nc),
        Statement::BlockStatement(s) => {
            for stmt in s.body {
                walk_stmt(stmt, nc, true);
            }
        }
        Statement::IfStatement(s) => {
            walk_expr(&s.test, nc);
            walk_stmt(s.consequent, nc, true);
            if let Some(alt) = s.alternate {
                walk_stmt(alt, nc, true);
            }
        }
        Statement::ForStatement(s) => {
            match &s.init {
                Some(ForInit::VariableDeclaration(d)) => walk_var_decl(d, nc, true),
                Some(ForInit::Expression(e)) => walk_expr(e, nc),
                None => {}
            }
            walk_opt(s.test.as_ref(), nc);
            walk_opt(s.update.as_ref(), nc);
            walk_stmt(s.body, nc, true);
        }
        Statement::ForInStatement(s) => {
            walk_for_left(&s.left, nc);
            walk_expr(&s.right, nc);
            walk_stmt(s.body, nc, true);
        }
        Statement::ForOfStatement(s) => {
            walk_for_left(&s.left, nc);
            walk_expr(&s.right, nc);
            walk_stmt(s.body, nc, true);
        }
        Statement::WhileStatement(s) => {
            walk_expr(&s.test, nc);
            walk_stmt(s.body, nc, true);
        }
        Statement::DoWhileStatement(s) => {
            walk_stmt(s.body, nc, true);
            walk_expr(&s.test, nc);
        }
        Statement::SwitchStatement(s) => {
            walk_expr(&s.discriminant, nc);
            for case in s.cases {
                walk_opt(case.test.as_ref(), nc);
                for stmt in case.consequent {
                    walk_stmt(stmt, nc, true);
                }
            }
        }
        Statement::TryStatement(s) => {
            for stmt in s.block.body {
                walk_stmt(stmt, nc, true);
            }
            if let Some(handler) = &s.handler {
                if let Some(param) = &handler.param {
                    declare_pattern(param, nc);
                    walk_expr(param, nc);
                }
                for stmt in handler.body.body {
                    walk_stmt(stmt, nc, true);
                }
            }
            if let Some(finalizer) = &s.finalizer {
                for stmt in finalizer.body {
                    walk_stmt(stmt, nc, true);
                }
            }
        }
        Statement::ThrowStatement(s) => walk_expr(&s.argument, nc),
        Statement::LabeledStatement(s) => walk_stmt(s.body, nc, true),
        Statement::ExportNamedDeclaration(s) => {
            if let Some(decl) = &s.declaration {
                walk_stmt(decl, nc, shadow);
            }
        }
        Statement::ExportDefaultDeclaration(s) => match &s.declaration {
            ExportDefaultValue::Expression(e) => walk_expr(e, nc),
            ExportDefaultValue::FunctionDeclaration(f) => {
                nc.fn_depth += 1;
                for param in f.params {
                    declare_pattern(param, nc);
                    walk_expr(param, nc);
                }
                for stmt in f.body.body {
                    walk_stmt(stmt, nc, true);
                }
                nc.fn_depth -= 1;
            }
            ExportDefaultValue::ClassDeclaration(c) => walk_class_body(&c.body, nc),
            ExportDefaultValue::TSDeclareFunction(_)
            | ExportDefaultValue::TSInterfaceDeclaration(_) => {}
        },
        Statement::TSExportAssignment(s) => walk_expr(&s.expression, nc),
        // No trigger-bearing children, or refused elsewhere (TS enum/module are
        // refused by type erasure before this analysis runs; the rune guard
        // refuses them too as its own defense in depth).
        Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::ImportDeclaration(_)
        | Statement::ExportAllDeclaration(_)
        | Statement::TSNamespaceExportDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSDeclareFunction(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::TSModuleDeclaration(_) => {}
    }
}

/// Walk a template fragment: trigger-check every rendered expression and record
/// the block-local bindings (each item/index, `{:then}`/`{:catch}` values,
/// `{@const}` names) that shadow the component scope.
fn walk_fragment(fragment: &Fragment<'_>, nc: &mut Nc<'_>) {
    for node in fragment.nodes {
        walk_fragment_node(node, nc);
    }
}

fn walk_fragment_node(node: &FragmentNode<'_>, nc: &mut Nc<'_>) {
    match node {
        FragmentNode::Text(_) | FragmentNode::Comment(_) => {}
        FragmentNode::Element(element) => walk_element(element, nc),
        FragmentNode::ExpressionTag(tag) => walk_expr(&tag.expression, nc),
        FragmentNode::HtmlTag(tag) => walk_html_tag(tag, nc),
        FragmentNode::IfBlock(block) => walk_if_block(block, nc),
        FragmentNode::EachBlock(block) => walk_each_block(block, nc),
        FragmentNode::AwaitBlock(block) => walk_await_block(block, nc),
        FragmentNode::KeyBlock(block) => walk_key_block(block, nc),
        FragmentNode::ConstTag(tag) => walk_const_tag(tag, nc),
        FragmentNode::SnippetBlock(snippet) => walk_snippet_block(snippet, nc),
        FragmentNode::RenderTag(tag) => walk_render_tag(tag, nc),
        // The special elements that COMPILE — the SSR-inert kinds
        // (`<svelte:window>`/`<svelte:body>`/`<svelte:document>`, which emit
        // nothing) and `<svelte:element>` (which emits `$.element(…)`) — are walked
        // on the emitted path, because the oracle runs its phase-2 analysis over
        // their expressions regardless of what SSR emits: a `new`/prop-rooted
        // member/call in a `this={…}` / bind / handler fires the wrapper, and a
        // `bind:` marks its target reassigned. The refused-at-emission kinds are
        // reachable only through a dropped `{:catch}` (matched explicitly so a new
        // variant fails compilation here).
        FragmentNode::SpecialElement(se) => {
            // Exhaustive `match` (not `matches!`) so a new `SpecialElementKind`
            // variant fails compilation here rather than silently defaulting to
            // the refused-at-emission set.
            let walk_on_emitted = match &se.kind {
                SpecialElementKind::SvelteWindow
                | SpecialElementKind::SvelteBody
                | SpecialElementKind::SvelteDocument
                | SpecialElementKind::SvelteElement { .. } => true,
                SpecialElementKind::SvelteHead
                | SpecialElementKind::SvelteComponent { .. }
                | SpecialElementKind::SvelteSelf
                | SpecialElementKind::SlotElement
                | SpecialElementKind::SvelteFragment
                | SpecialElementKind::SvelteBoundary
                | SpecialElementKind::TitleElement => false,
            };
            if nc.in_dropped_catch || walk_on_emitted {
                walk_special_element(se, nc);
            }
        }
        FragmentNode::DeclarationTag(tag) => {
            if nc.in_dropped_catch {
                walk_var_decl(&tag.declaration, nc, true);
            }
        }
        // `{@debug}` carries only bare identifiers — no `new`/member-call root — so
        // it can never trigger the wrapper, dropped or not.
        FragmentNode::DebugTag(_) => {}
    }
}

/// Trigger-check a special element's references (the `this={…}` expression,
/// attributes, and children). Reached on the emitted path for the kinds that
/// compile — the SSR-inert `<svelte:window>`/`<svelte:body>`/`<svelte:document>`
/// (which emit nothing but whose attributes the oracle still analyzes) and
/// `<svelte:element>` (whose `this={…}` / attributes / children emit) — and, for
/// every kind, through a dropped `{:catch}` (refused at emission there, so
/// reachable only this way).
fn walk_special_element(se: &SpecialElement<'_>, nc: &mut Nc<'_>) {
    if let Some(expr) = special_element_reference_expression(se) {
        walk_expr(expr, nc);
    }
    // A `bind:` is two-way — it MUTATES its target, so the target's root binding
    // is reassigned component-wide (the oracle marks it at analysis time). Without
    // this a `<svelte:window bind:scrollY={y}>` would let a later `{y}` read fold to
    // its initial value where the oracle keeps it dynamic — mirrors `walk_element`.
    for attr_node in se.attributes {
        if let AttributeNode::BindDirective(d) = attr_node {
            crate::rune_guard::assign_target_roots(&d.expression, nc.source, &mut nc.reassigned);
        }
    }
    each_reference_bearing_attribute_expression(se.attributes, &mut |expr| walk_expr(expr, nc));
    walk_fragment(&se.fragment, nc);
}

/// A `{#snippet}` is a function-like subtree: its parameters + body locals
/// shadow the component scope, and any `new`/unsafe member/call in its body
/// still triggers the wrapper (a prop-rooted access inside a snippet fires
/// `needs_context`, and a `new` in a *hoistable* snippet body fires it too).
fn walk_snippet_block(snippet: &SnippetBlock<'_>, nc: &mut Nc<'_>) {
    nc.fn_depth += 1;
    for param in snippet.parameters {
        declare_pattern(param, nc);
        walk_expr(param, nc);
    }
    walk_fragment(&snippet.body, nc);
    nc.fn_depth -= 1;
}

/// A `{@render}` walks only its call arguments — the oracle visits the render
/// callee with expression metadata (not as a `CallExpression`), so a plain
/// snippet/prop callee never triggers `needs_context`; a member callee is
/// refused at emission time. Arguments are ordinary template expressions.
fn walk_render_tag(tag: &RenderTag<'_>, nc: &mut Nc<'_>) {
    // Inside a dropped `{:catch}` the "member callee refused at emission"
    // assumption doesn't hold — the callee is never emitted — so the whole render
    // expression is trigger-checked (a member-rooted callee over a prop must fire
    // the wrapper, matching the oracle).
    if nc.in_dropped_catch {
        walk_expr(&tag.expression, nc);
        return;
    }
    // The same (possibly-parenthesized) call unwrap the emitter uses. A non-call
    // render refuses at emission, so here it simply yields no arguments to check.
    if let Some(call) = render_call_expression(&tag.expression) {
        walk_exprs(call.arguments, nc);
    }
}

fn walk_element(element: &Element<'_>, nc: &mut Nc<'_>) {
    // A `bind:` directive is a two-way binding — it MUTATES its target, so the
    // target's root binding is reassigned component-wide (the oracle marks the
    // binding mutated at analysis time, before it decides what to emit). Without
    // this a bound `$state` would statically fold to its initial value where the
    // oracle keeps the read dynamic (`bind:group={value}` beside a `{value}`
    // interpolation). Collected for every bind — a bind tsv refuses at emission
    // makes the whole component refuse, so the mark is harmless there — and rides
    // `assign_target_roots`, which unwraps the raw template's TypeScript
    // assignment-target wrappers.
    for attr_node in element.attributes {
        if let AttributeNode::BindDirective(d) = attr_node {
            crate::rune_guard::assign_target_roots(&d.expression, nc.source, &mut nc.reassigned);
        }
    }
    // The shared traversal (`attr_refs`) defines which attribute expressions are
    // reference-bearing on the emitted path: plain attribute values on any element,
    // component `{...spread}` expressions (emitted as `$.spread_props` elements),
    // and the no-op drop family (`use:`/`transition:`/`animate:`/`{@attach}`) on a
    // regular element — dropped from the tag but still walked, so a prop-rooted
    // access inside a `use:` argument still fires the wrapper. Element spreads and
    // the refused legacy directives are not visited (their emission refusal fires).
    // A bare directive *name* never triggers `needs_context`, so the name walk is
    // the snippet analysis's alone. Inside a dropped `{:catch}` the emitter never
    // walks the fragment, so those emission refusals never fire — there a
    // `new`/prop-rooted access in any skipped position must still trigger the
    // wrapper, so every attribute reference is walked.
    if nc.in_dropped_catch {
        each_reference_bearing_attribute_expression(element.attributes, &mut |expr| {
            walk_expr(expr, nc);
        });
    } else {
        each_attribute_expression(element, &mut |expr| walk_expr(expr, nc));
    }
    walk_fragment(&element.fragment, nc);
}

fn walk_html_tag(tag: &HtmlTag<'_>, nc: &mut Nc<'_>) {
    walk_expr(&tag.expression, nc);
}

fn walk_if_block(block: &IfBlock<'_>, nc: &mut Nc<'_>) {
    walk_expr(&block.test, nc);
    walk_fragment(&block.consequent, nc);
    if let Some(alt) = &block.alternate {
        walk_fragment(alt, nc);
    }
}

fn walk_each_block(block: &EachBlock<'_>, nc: &mut Nc<'_>) {
    walk_expr(&block.expression, nc);
    if let Some(key) = &block.key {
        walk_expr(key, nc);
    }
    if let Some(context) = &block.context {
        declare_pattern(context, nc);
        walk_expr(context, nc);
    }
    if let Some(index) = block.index {
        nc.shadowed.insert(index.to_string());
    }
    walk_fragment(&block.body, nc);
    if let Some(fallback) = &block.fallback {
        walk_fragment(fallback, nc);
    }
}

fn walk_await_block(block: &AwaitBlock<'_>, nc: &mut Nc<'_>) {
    walk_expr(&block.expression, nc);
    if let Some(value) = &block.value {
        declare_pattern(value, nc);
        walk_expr(value, nc);
    }
    if let Some(error) = &block.error {
        declare_pattern(error, nc);
        walk_expr(error, nc);
    }
    // Pending/then are emitted (their skipped attribute positions refuse at
    // emission); the `{:catch}` branch is dropped, so it is walked with the
    // inclusive attribute traversal. The flag is scoped to the catch subtree.
    if let Some(pending) = &block.pending {
        walk_fragment(pending, nc);
    }
    if let Some(then) = &block.then {
        walk_fragment(then, nc);
    }
    if let Some(catch) = &block.catch {
        let prev = nc.in_dropped_catch;
        nc.in_dropped_catch = true;
        walk_fragment(catch, nc);
        nc.in_dropped_catch = prev;
    }
}

fn walk_key_block(block: &KeyBlock<'_>, nc: &mut Nc<'_>) {
    walk_expr(&block.expression, nc);
    walk_fragment(&block.fragment, nc);
}

fn walk_const_tag(tag: &ConstTag<'_>, nc: &mut Nc<'_>) {
    declare_pattern(&tag.id, nc);
    walk_expr(&tag.id, nc);
    walk_expr(&tag.init, nc);
}
