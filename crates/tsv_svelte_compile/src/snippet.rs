//! `{#snippet}` hoist analysis — a name-based port of Svelte's
//! `can_hoist_snippet` (`2-analyze/visitors/SnippetBlock.js`).
//!
//! A **top-level** snippet (a direct child of the component's root fragment)
//! whose references all resolve to module scope hoists to true module scope: the
//! oracle emits its `function` declaration between the imports and the exported
//! component function. Any free reference to an instance binding (a prop,
//! `$state`/`$derived`, or a plain top-level `const`/`let`/`function`/`class` —
//! **imports do not disqualify**) keeps the function inside the component body.
//! A snippet that references another top-level snippet hoists only if that
//! snippet also hoists (the oracle's recursion), so hoistability is a fixpoint.
//!
//! The oracle decides this with a scope-sensitive walk; this port is name-based,
//! so it computes each snippet's free references by collecting every referenced
//! identifier and subtracting the names bound within the snippet (parameters and
//! body-local declarations). Parameters shadow the whole body unconditionally
//! and are trusted; a *nested* (non-parameter) local that collides with an
//! instance-binding name is genuinely ambiguous under the flat name model
//! (free-in-one-place vs bound-in-another can't be told apart), so that snippet
//! **refuses**. An identifier this port can't name (a `\u`-escaped reference)
//! also makes the decision undecidable and refuses.

use std::collections::HashMap;

use tsv_svelte::ast::internal::{
    AttributeNode, AwaitBlock, ConstTag, DebugTag, DeclarationTag, EachBlock, Element, Fragment,
    FragmentNode, HtmlTag, IfBlock, KeyBlock, RenderTag, Root, SnippetBlock, SpecialElement,
};
use tsv_ts::ast::internal::{
    ArrowFunctionBody, ClassBody, ClassMember, Expression, ForInOfLeft, ForInit,
    FunctionExpression, Identifier, ObjectPatternProperty, ObjectProperty, Statement,
    VariableDeclaration,
};

use crate::analyze::{NameSet, pattern_binding_names};
use crate::attr_refs::{
    each_attribute_expression, each_child_fragment, each_emitted_directive_name,
    each_reference_bearing_attribute_expression, each_reference_bearing_directive_name,
    special_element_reference_expression,
};
use crate::{CompileError, Refusal};

/// The snippet analysis product consumed by the server transform.
pub(crate) struct SnippetAnalysis {
    /// Top-level snippet name → whether it hoists to module scope.
    hoistable: HashMap<String, bool>,
    /// Every snippet name declared anywhere in the component (top-level and
    /// nested) — the render-callee classification and generated-name collision
    /// both consult this.
    pub names: NameSet,
}

impl SnippetAnalysis {
    /// Whether a top-level snippet of this name hoists to module scope (`false`
    /// for a body-local or a nested snippet, which never hoists).
    pub fn is_hoisted(&self, name: &str) -> bool {
        self.hoistable.get(name).copied().unwrap_or(false)
    }
}

/// The plain (non-escaped) name of a snippet's `expression` identifier.
pub(crate) fn snippet_name<'s>(snippet: &SnippetBlock<'_>, source: &'s str) -> Option<&'s str> {
    match &snippet.expression {
        Expression::Identifier(id) => plain_name(id, source),
        _ => None,
    }
}

/// The plain (non-escaped) name of an identifier, `None` for an escaped one.
fn plain_name<'s>(id: &Identifier<'_>, source: &'s str) -> Option<&'s str> {
    if id.escaped_name.is_some() {
        return None;
    }
    let start = id.span.start as usize;
    Some(&source[start..start + id.name_len as usize])
}

/// Analyze the component's snippets: collect every snippet name, and decide
/// which top-level snippets hoist to module scope.
///
/// `instance_bindings` is the set of top-level instance binding names (the
/// evaluator's table); `import_names` is the subset that are imports (which do
/// **not** disqualify hoisting). A snippet whose free-vs-shadowed classification
/// is ambiguous, or a duplicate top-level snippet name, returns `Err`.
pub(crate) fn analyze_snippets(
    root: &Root<'_>,
    source: &str,
    instance_bindings: &NameSet,
    import_names: &NameSet,
) -> Result<SnippetAnalysis, CompileError> {
    let mut names = NameSet::default();
    let mut name_collector = Collector::new(source);
    name_collector.collect_names(&root.fragment, &mut names);

    let top_level: Vec<&SnippetBlock<'_>> = root
        .fragment
        .nodes
        .iter()
        .filter_map(|node| match node {
            FragmentNode::SnippetBlock(s) => Some(s),
            _ => None,
        })
        .collect();

    // Duplicate top-level names are oracle-rejected; refuse rather than emit two
    // `function` declarations.
    let mut seen = NameSet::default();
    for snippet in &top_level {
        if let Some(name) = snippet_name(snippet, source)
            && !seen.insert(name.to_string())
        {
            return Err(unsupported(Refusal::DuplicateSnippetName {
                name: name.to_string(),
            }));
        }
    }

    let top_level_names: NameSet = top_level
        .iter()
        .filter_map(|s| snippet_name(s, source).map(str::to_string))
        .collect();

    // Per snippet: is it *directly* non-hoistable (a free ref to an instance
    // binding), and which top-level snippets does it depend on?
    let mut direct_ok: HashMap<String, bool> = HashMap::new();
    let mut deps: HashMap<String, Vec<String>> = HashMap::new();
    for snippet in &top_level {
        let Some(name) = snippet_name(snippet, source) else {
            continue;
        };

        let mut c = Collector::new(source);
        c.collect_snippet_scope(snippet);
        if c.opaque {
            return Err(unsupported(Refusal::SnippetHoistAmbiguous {
                name: name.to_string(),
            }));
        }

        let mut ok = true;
        let mut snippet_deps = Vec::new();
        for r in &c.refs {
            if c.params.contains(r) {
                continue; // a parameter shadows the whole body — safe
            }
            let is_instance = instance_bindings.contains(r) && !import_names.contains(r);
            if c.locals.contains(r) {
                if is_instance {
                    return Err(unsupported(Refusal::SnippetHoistAmbiguous {
                        name: name.to_string(),
                    }));
                }
                continue; // an ordinary local — not a free reference
            }
            if is_instance {
                ok = false; // a free reference to an instance binding
            } else if top_level_names.contains(r) {
                snippet_deps.push(r.clone()); // depends on another snippet
            }
            // else: a global or an import — does not disqualify
        }
        direct_ok.insert(name.to_string(), ok);
        deps.insert(name.to_string(), snippet_deps);
    }

    // Fixpoint: a snippet hoists only if it is directly ok AND every snippet it
    // depends on hoists. Non-hoistability only spreads, so iterate to stable.
    let mut hoistable: HashMap<String, bool> = direct_ok.clone();
    loop {
        let mut changed = false;
        for (name, ok) in &direct_ok {
            if !ok || !hoistable[name] {
                continue;
            }
            let all_deps_ok = deps[name]
                .iter()
                .all(|d| hoistable.get(d).copied() == Some(true));
            if !all_deps_ok {
                hoistable.insert(name.clone(), false);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    Ok(SnippetAnalysis { hoistable, names })
}

fn unsupported(reason: Refusal) -> CompileError {
    CompileError::Unsupported(reason)
}

/// A flat free-variable collector over a snippet's body.
///
/// `refs` accumulates every referenced identifier (member/call roots and bare
/// references, never property names or keys); `locals` accumulates every name
/// bound within the snippet (parameters and nested declarations); `params` is
/// the snippet's own parameters (a trusted subset of `locals`). `opaque` is set
/// by any identifier the port can't name (an escaped identifier), which makes
/// the hoist decision undecidable.
struct Collector<'s> {
    source: &'s str,
    refs: NameSet,
    locals: NameSet,
    params: NameSet,
    opaque: bool,
    /// Whether the walk is inside a **dropped** `{:catch}` subtree, where the
    /// emitter never walks the fragment so the emission refusals that
    /// `each_attribute_expression` relies on never fire. There every attribute
    /// reference (element spreads, directive expressions, `{@attach}`) must be
    /// counted to match the oracle. Sticky for the whole catch subtree.
    in_dropped_catch: bool,
}

impl<'s> Collector<'s> {
    fn new(source: &'s str) -> Self {
        Self {
            source,
            refs: NameSet::default(),
            locals: NameSet::default(),
            params: NameSet::default(),
            opaque: false,
            in_dropped_catch: false,
        }
    }

    /// Collect the snippet's parameter names (into `params`+`locals`) and its
    /// body references and locals.
    fn collect_snippet_scope(&mut self, snippet: &SnippetBlock<'_>) {
        for param in snippet.parameters {
            self.bind_pattern_into_params(param);
            self.param_defaults(param);
        }
        self.fragment(&snippet.body);
    }

    fn ident_ref(&mut self, id: &Identifier<'_>) {
        match plain_name(id, self.source) {
            Some(name) => {
                self.refs.insert(name.to_string());
            }
            None => self.opaque = true,
        }
    }

    fn ident_local(&mut self, id: &Identifier<'_>) {
        match plain_name(id, self.source) {
            Some(name) => {
                self.locals.insert(name.to_string());
            }
            None => self.opaque = true,
        }
    }

    /// Bind a parameter pattern into `params` and `locals`.
    fn bind_pattern_into_params(&mut self, pattern: &Expression<'_>) {
        let mut names = Vec::new();
        if pattern_binding_names(pattern, self.source, &mut names).is_err() {
            self.opaque = true;
            return;
        }
        for name in names {
            self.params.insert(name.clone());
            self.locals.insert(name);
        }
    }

    /// Bind a pattern's names into `locals` (nested declarations, each items,
    /// `{@const}`/`{:then}` bindings, catch params).
    fn bind_pattern(&mut self, pattern: &Expression<'_>) {
        let mut names = Vec::new();
        if pattern_binding_names(pattern, self.source, &mut names).is_err() {
            self.opaque = true;
            return;
        }
        for name in names {
            self.locals.insert(name);
        }
    }

    /// Walk a parameter pattern for its reference-bearing default expressions
    /// (`{a = expr}`).
    fn param_defaults(&mut self, pattern: &Expression<'_>) {
        match pattern {
            Expression::AssignmentPattern(p) => self.expr(p.right),
            Expression::ObjectPattern(p) => {
                for prop in p.properties {
                    if let ObjectPatternProperty::Property(prop) = prop {
                        self.param_defaults(&prop.value);
                    }
                }
            }
            Expression::ArrayPattern(p) => {
                for element in p.elements.iter().flatten() {
                    self.param_defaults(element);
                }
            }
            _ => {}
        }
    }

    // ── Snippet-name collection (a separate, lighter pass) ──────────────────

    fn collect_names(&mut self, fragment: &Fragment<'_>, out: &mut NameSet) {
        for node in fragment.nodes {
            // Only a snippet contributes a name; the recursion into every child
            // fragment rides the shared seam, so a new template shape can't drop
            // out of this pass silently (the former `_ => {}` arm).
            if let FragmentNode::SnippetBlock(s) = node
                && let Some(name) = snippet_name(s, self.source)
            {
                out.insert(name.to_string());
            }
            each_child_fragment(node, &mut |child| self.collect_names(child, out));
        }
    }

    // ── Fragment walk ───────────────────────────────────────────────────────

    fn fragment(&mut self, fragment: &Fragment<'_>) {
        for node in fragment.nodes {
            self.fragment_node(node);
        }
    }

    fn fragment_node(&mut self, node: &FragmentNode<'_>) {
        match node {
            FragmentNode::Text(_) | FragmentNode::Comment(_) => {}
            FragmentNode::Element(e) => self.element(e),
            FragmentNode::SpecialElement(se) => self.special_element(se),
            FragmentNode::ExpressionTag(t) => self.expr(&t.expression),
            FragmentNode::HtmlTag(t) => self.html_tag(t),
            FragmentNode::IfBlock(b) => self.if_block(b),
            FragmentNode::EachBlock(b) => self.each_block(b),
            FragmentNode::AwaitBlock(b) => self.await_block(b),
            FragmentNode::KeyBlock(b) => self.key_block(b),
            FragmentNode::ConstTag(t) => self.const_tag(t),
            FragmentNode::RenderTag(t) => self.render_tag(t),
            FragmentNode::SnippetBlock(s) => self.nested_snippet(s),
            FragmentNode::DeclarationTag(t) => self.declaration_tag(t),
            FragmentNode::DebugTag(t) => self.debug_tag(t),
        }
    }

    fn element(&mut self, element: &Element<'_>) {
        // The shared traversal (`attr_refs`) defines which attribute expressions
        // are reference-bearing — including component `{...spread}` expressions and
        // the no-op drop family (`use:`/`transition:`/`animate:`/`{@attach}`) on a
        // regular element, whose free references must disqualify hoisting exactly
        // like a plain attribute value's (a module-hoisted snippet referencing an
        // instance binding is a runtime ReferenceError). The drop-family directive
        // *names* (a use/transition/animate action reference) ride the companion
        // name walk. Inside a dropped `{:catch}` the emitter never walks the
        // fragment, so the emission refusals that let the default traversal skip
        // element spreads / the refused legacy directives never fire — there every
        // attribute reference must be counted, `style:` shorthand names included.
        if self.in_dropped_catch {
            self.dropped_attribute_refs(element.attributes);
        } else {
            each_attribute_expression(element, &mut |expr| self.expr(expr));
            let source = self.source;
            each_emitted_directive_name(element, source, &mut |name| {
                self.directive_name_ref(name);
            });
        }
        self.fragment(&element.fragment);
    }

    fn special_element(&mut self, se: &SpecialElement<'_>) {
        // A special element is refused at emission everywhere else, so its own
        // references (the `this={…}` expression, attributes, directive names) are
        // reachable only through a dropped `{:catch}` — count them there.
        if self.in_dropped_catch {
            if let Some(expr) = special_element_reference_expression(se) {
                self.expr(expr);
            }
            self.dropped_attribute_refs(se.attributes);
        }
        self.fragment(&se.fragment);
    }

    /// Count every reference in an attribute list on the dropped-`{:catch}` path:
    /// each reference-bearing attribute expression, plus each value-binding
    /// directive name.
    fn dropped_attribute_refs(&mut self, attributes: &[AttributeNode<'_>]) {
        each_reference_bearing_attribute_expression(attributes, &mut |expr| self.expr(expr));
        let source = self.source;
        each_reference_bearing_directive_name(attributes, source, &mut |name| {
            self.directive_name_ref(name);
        });
    }

    /// Record a value-binding directive name (`use:`/`transition:`/`animate:`) as a
    /// free reference. The name may be a member path (`use:a.b`); the referenced
    /// binding is the root identifier, matching the oracle.
    fn directive_name_ref(&mut self, name: &str) {
        let root = name.split(['.', '[']).next().unwrap_or(name);
        if !root.is_empty() {
            self.refs.insert(root.to_string());
        }
    }

    fn html_tag(&mut self, tag: &HtmlTag<'_>) {
        self.expr(&tag.expression);
    }

    fn if_block(&mut self, block: &IfBlock<'_>) {
        self.expr(&block.test);
        self.fragment(&block.consequent);
        if let Some(alt) = &block.alternate {
            self.fragment(alt);
        }
    }

    fn each_block(&mut self, block: &EachBlock<'_>) {
        self.expr(&block.expression);
        if let Some(key) = &block.key {
            self.expr(key);
        }
        if let Some(context) = &block.context {
            self.bind_pattern(context);
        }
        if let Some(index) = block.index {
            self.locals.insert(index.to_string());
        }
        self.fragment(&block.body);
        if let Some(fallback) = &block.fallback {
            self.fragment(fallback);
        }
    }

    fn await_block(&mut self, block: &AwaitBlock<'_>) {
        self.expr(&block.expression);
        if let Some(value) = &block.value {
            self.bind_pattern(value);
        }
        if let Some(error) = &block.error {
            self.bind_pattern(error);
        }
        // Pending/then are emitted (so their skipped attribute positions refuse at
        // emission); the `{:catch}` branch is dropped, so it is walked with the
        // inclusive attribute traversal. The flag is scoped to the catch subtree.
        for frag in [&block.pending, &block.then].into_iter().flatten() {
            self.fragment(frag);
        }
        if let Some(catch) = &block.catch {
            let prev = self.in_dropped_catch;
            self.in_dropped_catch = true;
            self.fragment(catch);
            self.in_dropped_catch = prev;
        }
    }

    fn key_block(&mut self, block: &KeyBlock<'_>) {
        self.expr(&block.expression);
        self.fragment(&block.fragment);
    }

    fn const_tag(&mut self, tag: &ConstTag<'_>) {
        self.bind_pattern(&tag.id);
        self.expr(&tag.init);
    }

    fn render_tag(&mut self, tag: &RenderTag<'_>) {
        self.expr(&tag.expression);
    }

    fn nested_snippet(&mut self, snippet: &SnippetBlock<'_>) {
        // The nested snippet's name and parameters are locals of the enclosing
        // snippet's subtree; its body references count toward the enclosing scope.
        if let Some(name) = snippet_name(snippet, self.source) {
            self.locals.insert(name.to_string());
        }
        for param in snippet.parameters {
            self.bind_pattern(param);
            self.param_defaults(param);
        }
        self.fragment(&snippet.body);
    }

    fn declaration_tag(&mut self, tag: &DeclarationTag<'_>) {
        self.var_decl(&tag.declaration);
    }

    fn debug_tag(&mut self, tag: &DebugTag<'_>) {
        for id in tag.identifiers {
            self.expr(id);
        }
    }

    fn var_decl(&mut self, decl: &VariableDeclaration<'_>) {
        for declarator in decl.declarations {
            self.bind_pattern(&declarator.id);
            if let Some(init) = &declarator.init {
                self.expr(init);
            }
        }
    }

    // ── Expression walk ──────────────────────────────────────────────────────

    fn exprs(&mut self, exprs: &[Expression<'_>]) {
        for expr in exprs {
            self.expr(expr);
        }
    }

    fn expr(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::Identifier(id) => self.ident_ref(id),
            Expression::MemberExpression(m) => {
                self.expr(m.object);
                if m.computed {
                    self.expr(m.property);
                }
            }
            Expression::CallExpression(c) => {
                self.expr(c.callee);
                self.exprs(c.arguments);
            }
            Expression::NewExpression(n) => {
                self.expr(n.callee);
                self.exprs(n.arguments);
            }
            Expression::ArrowFunctionExpression(a) => {
                for param in a.params {
                    self.bind_pattern(param);
                    self.param_defaults(param);
                }
                match &a.body {
                    ArrowFunctionBody::Expression(e) => self.expr(e),
                    ArrowFunctionBody::BlockStatement(b) => self.stmts(b.body),
                }
            }
            Expression::FunctionExpression(f) => self.function_expr(f),
            Expression::ClassExpression(c) => self.class_body(&c.body),
            Expression::ObjectExpression(obj) => {
                for prop in obj.properties {
                    match prop {
                        ObjectProperty::Property(p) => {
                            if p.computed {
                                self.expr(&p.key);
                            }
                            self.expr(&p.value);
                        }
                        ObjectProperty::SpreadElement(s) => self.expr(s.argument),
                    }
                }
            }
            Expression::ArrayExpression(arr) => {
                for element in arr.elements.iter().flatten() {
                    self.expr(element);
                }
            }
            Expression::UnaryExpression(u) => self.expr(u.argument),
            Expression::UpdateExpression(u) => self.expr(u.argument),
            Expression::BinaryExpression(b) => {
                self.expr(b.left);
                self.expr(b.right);
            }
            Expression::ConditionalExpression(c) => {
                self.expr(c.test);
                self.expr(c.consequent);
                self.expr(c.alternate);
            }
            Expression::SpreadElement(s) => self.expr(s.argument),
            Expression::TemplateLiteral(t) => self.exprs(t.expressions),
            Expression::TaggedTemplateExpression(t) => {
                self.expr(t.tag);
                self.exprs(t.quasi.expressions);
            }
            Expression::AwaitExpression(a) => self.expr(a.argument),
            Expression::YieldExpression(y) => {
                if let Some(arg) = &y.argument {
                    self.expr(arg);
                }
            }
            Expression::SequenceExpression(s) => self.exprs(s.expressions),
            Expression::AssignmentExpression(a) => {
                self.expr(a.left);
                self.expr(a.right);
            }
            Expression::ObjectPattern(p) => {
                for prop in p.properties {
                    match prop {
                        ObjectPatternProperty::Property(prop) => {
                            if prop.computed {
                                self.expr(&prop.key);
                            }
                            self.expr(&prop.value);
                        }
                        ObjectPatternProperty::RestElement(rest) => self.expr(rest.argument),
                    }
                }
            }
            Expression::ArrayPattern(p) => {
                for element in p.elements.iter().flatten() {
                    self.expr(element);
                }
            }
            Expression::AssignmentPattern(p) => {
                self.expr(p.left);
                self.expr(p.right);
            }
            Expression::RestElement(r) => self.expr(r.argument),
            Expression::TSTypeAssertion(t) => self.expr(t.expression),
            Expression::TSAsExpression(t) => self.expr(t.expression),
            Expression::TSSatisfiesExpression(t) => self.expr(t.expression),
            Expression::TSInstantiationExpression(t) => self.expr(t.expression),
            Expression::TSNonNullExpression(t) => self.expr(t.expression),
            Expression::TSParameterProperty(t) => self.expr(t.parameter),
            Expression::ImportExpression(i) => {
                self.expr(i.source);
                if let Some(options) = &i.options {
                    self.expr(options);
                }
            }
            Expression::JsdocCast(j) => self.expr(j.inner),
            Expression::ParenthesizedExpression(p) => self.expr(p.expression),
            Expression::Literal(_)
            | Expression::PrivateIdentifier(_)
            | Expression::RegexLiteral(_)
            | Expression::ThisExpression(_)
            | Expression::Super(_)
            | Expression::MetaProperty(_) => {}
        }
    }

    fn function_expr(&mut self, f: &FunctionExpression<'_>) {
        if let Some(id) = &f.id {
            self.ident_local(id);
        }
        for param in f.params {
            self.bind_pattern(param);
            self.param_defaults(param);
        }
        self.stmts(f.body.body);
    }

    fn class_body(&mut self, body: &ClassBody<'_>) {
        for member in body.body {
            match member {
                ClassMember::MethodDefinition(m) => {
                    if m.computed {
                        self.expr(&m.key);
                    }
                    self.function_expr(&m.value);
                }
                ClassMember::PropertyDefinition(p) => {
                    if p.computed {
                        self.expr(&p.key);
                    }
                    if let Some(value) = &p.value {
                        self.expr(value);
                    }
                }
                ClassMember::StaticBlock(b) => self.stmts(b.body),
                ClassMember::IndexSignature(_) => {}
            }
        }
    }

    // ── Statement walk ───────────────────────────────────────────────────────

    fn stmts(&mut self, stmts: &[Statement<'_>]) {
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::VariableDeclaration(d) => self.var_decl(d),
            Statement::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    self.ident_local(id);
                }
                for param in f.params {
                    self.bind_pattern(param);
                    self.param_defaults(param);
                }
                self.stmts(f.body.body);
            }
            Statement::ClassDeclaration(c) => {
                if let Some(id) = &c.id {
                    self.ident_local(id);
                }
                self.class_body(&c.body);
            }
            Statement::ExpressionStatement(s) => self.expr(&s.expression),
            Statement::ReturnStatement(s) => {
                if let Some(arg) = &s.argument {
                    self.expr(arg);
                }
            }
            Statement::BlockStatement(s) => self.stmts(s.body),
            Statement::IfStatement(s) => {
                self.expr(&s.test);
                self.stmt(s.consequent);
                if let Some(alt) = s.alternate {
                    self.stmt(alt);
                }
            }
            Statement::ForStatement(s) => {
                match &s.init {
                    Some(ForInit::VariableDeclaration(d)) => self.var_decl(d),
                    Some(ForInit::Expression(e)) => self.expr(e),
                    None => {}
                }
                if let Some(test) = &s.test {
                    self.expr(test);
                }
                if let Some(update) = &s.update {
                    self.expr(update);
                }
                self.stmt(s.body);
            }
            Statement::ForInStatement(s) => {
                self.for_left(&s.left);
                self.expr(&s.right);
                self.stmt(s.body);
            }
            Statement::ForOfStatement(s) => {
                self.for_left(&s.left);
                self.expr(&s.right);
                self.stmt(s.body);
            }
            Statement::WhileStatement(s) => {
                self.expr(&s.test);
                self.stmt(s.body);
            }
            Statement::DoWhileStatement(s) => {
                self.stmt(s.body);
                self.expr(&s.test);
            }
            Statement::SwitchStatement(s) => {
                self.expr(&s.discriminant);
                for case in s.cases {
                    if let Some(test) = &case.test {
                        self.expr(test);
                    }
                    self.stmts(case.consequent);
                }
            }
            Statement::TryStatement(s) => {
                self.stmts(s.block.body);
                if let Some(handler) = &s.handler {
                    if let Some(param) = &handler.param {
                        self.bind_pattern(param);
                    }
                    self.stmts(handler.body.body);
                }
                if let Some(finalizer) = &s.finalizer {
                    self.stmts(finalizer.body);
                }
            }
            Statement::ThrowStatement(s) => self.expr(&s.argument),
            Statement::LabeledStatement(s) => self.stmt(s.body),
            // No reference-bearing children, or refused before emission.
            Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_)
            | Statement::ImportDeclaration(_)
            | Statement::ExportNamedDeclaration(_)
            | Statement::ExportDefaultDeclaration(_)
            | Statement::ExportAllDeclaration(_)
            | Statement::TSNamespaceExportDeclaration(_)
            | Statement::TSImportEqualsDeclaration(_)
            | Statement::TSExportAssignment(_)
            | Statement::TSTypeAliasDeclaration(_)
            | Statement::TSInterfaceDeclaration(_)
            | Statement::TSDeclareFunction(_)
            | Statement::TSEnumDeclaration(_)
            | Statement::TSModuleDeclaration(_) => {}
        }
    }

    fn for_left(&mut self, left: &ForInOfLeft<'_>) {
        match left {
            ForInOfLeft::VariableDeclaration(decl) => self.var_decl(decl),
            ForInOfLeft::Pattern(pattern) => self.expr(pattern),
        }
    }
}
