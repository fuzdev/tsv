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
//! path already forces the wrapper via `has_effects`, and `$bindable` is refused
//! by the rune guard, so neither is re-derived here.)
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
    AttributeNode, AttributeValue, AwaitBlock, ConstTag, EachBlock, Element, Fragment,
    FragmentNode, HtmlTag, IfBlock, KeyBlock, Root,
};
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ClassBody, ClassMember, ExportDefaultValue, Expression, ForInOfLeft,
    ForInit, FunctionExpression, ObjectPatternProperty, ObjectProperty, Statement,
    VariableDeclaration,
};

use crate::analyze::{NameSet, RuneInit, classify_rune_init, pattern_binding_names};
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
}

/// The whole-component analysis product consumed by the server transform.
pub(crate) struct ComponentContext {
    /// Whether the component needs the `$$renderer.component(…)` wrapper.
    pub needs_context: bool,
    /// Names reassigned anywhere in the component (script + template, including
    /// inside dropped event handlers).
    pub reassigned: NameSet,
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
pub(crate) fn analyze_component(
    root: &Root<'_>,
    source: &str,
) -> Result<ComponentContext, CompileError> {
    let mut context_roots = NameSet::default();
    collect_context_roots(root, source, &mut context_roots);

    let mut nc = Nc {
        source,
        context_roots: &context_roots,
        shadowed: NameSet::default(),
        member_roots: NameSet::default(),
        needs: false,
        refuse: None,
        reassigned: NameSet::default(),
    };

    if let Some(script) = root.instance {
        for stmt in script.content.body {
            walk_stmt(stmt, &mut nc, false);
        }
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
fn collect_context_roots(root: &Root<'_>, source: &str, out: &mut NameSet) {
    let Some(script) = root.instance else {
        return;
    };
    for stmt in script.content.body {
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
/// the set stays a superset of the true nested bindings).
fn declare_pattern(pattern: &Expression<'_>, nc: &mut Nc<'_>) {
    let mut names = Vec::new();
    if pattern_binding_names(pattern, nc.source, &mut names).is_ok() {
        for name in names {
            nc.shadowed.insert(name);
        }
    }
}

/// Add a single identifier's name to `shadowed`.
fn declare_ident(id: &tsv_ts::ast::internal::Identifier<'_>, nc: &mut Nc<'_>) {
    if let Some(name) = plain_name(id, nc.source) {
        nc.shadowed.insert(name.to_string());
    }
}

/// Peel `MemberExpression.object` and `ParenthesizedExpression` to the root node
/// — Svelte's `is_safe_identifier` walks `.object` down member chains, and tsv's
/// AST additionally carries parenthesization the oracle's ESTree view doesn't,
/// so both are peeled to reach the same root.
fn root_of<'e>(expr: &'e Expression<'e>) -> &'e Expression<'e> {
    let mut node = expr;
    loop {
        match node {
            Expression::MemberExpression(m) => node = m.object,
            Expression::ParenthesizedExpression(p) => node = p.expression,
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
        }
        Expression::FunctionExpression(f) => walk_function_expression(f, nc),
        Expression::ClassExpression(c) => walk_class_body(&c.body, nc),

        // Leaves — no children, no bindings.
        Expression::Identifier(_)
        | Expression::Literal(_)
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
                for stmt in b.body {
                    walk_stmt(stmt, nc, true);
                }
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
            for param in f.params {
                declare_pattern(param, nc);
                walk_expr(param, nc);
            }
            for s in f.body.body {
                walk_stmt(s, nc, true);
            }
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
                for param in f.params {
                    declare_pattern(param, nc);
                    walk_expr(param, nc);
                }
                for stmt in f.body.body {
                    walk_stmt(stmt, nc, true);
                }
            }
            ExportDefaultValue::ClassDeclaration(c) => walk_class_body(&c.body, nc),
            ExportDefaultValue::TSDeclareFunction(_)
            | ExportDefaultValue::TSInterfaceDeclaration(_) => {}
        },
        Statement::TSExportAssignment(s) => walk_expr(&s.expression, nc),
        // No trigger-bearing children, or refused elsewhere (TS enum/module are
        // rejected by the rune guard before this analysis runs).
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
        // Emission refuses these shapes, so their contents never reach output;
        // matched explicitly so a new variant fails compilation here.
        FragmentNode::SpecialElement(_)
        | FragmentNode::SnippetBlock(_)
        | FragmentNode::DeclarationTag(_)
        | FragmentNode::DebugTag(_)
        | FragmentNode::RenderTag(_) => {}
    }
}

fn walk_element(element: &Element<'_>, nc: &mut Nc<'_>) {
    for attr_node in element.attributes {
        // Only plain attributes reach emission; directives/spreads are refused
        // there, so their expressions never affect the compiled output.
        if let AttributeNode::Attribute(attr) = attr_node
            && let Some(values) = attr.value
        {
            for value in values {
                if let AttributeValue::ExpressionTag(tag) = value {
                    walk_expr(&tag.expression, nc);
                }
            }
        }
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
    if let Some(pending) = &block.pending {
        walk_fragment(pending, nc);
    }
    if let Some(then) = &block.then {
        walk_fragment(then, nc);
    }
    if let Some(catch) = &block.catch {
        walk_fragment(catch, nc);
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
