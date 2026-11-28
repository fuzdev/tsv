//! The `needs_context` analysis: does the component require the
//! `$$renderer.component(($$renderer) => …)` wrapper?
//!
//! Ports Svelte's phase-2 `needs_context` accumulation — the flag the server
//! transform reads (`should_inject_context = dev || analysis.needs_context`) to
//! decide whether to wrap the whole component body. This walk runs late by
//! necessity, not by habit: it consumes the *erased* instance and module bodies,
//! the binding table's `store_names`, and `unassignable_names`, none of which exist
//! at the point the earlier walks run. See
//! [the walk inventory](crate#the-walks-and-their-oracle-phases). The oracle sets it,
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

use tsv_lang::store::Run;
use tsv_svelte::ast::{
    AttributeNode, AwaitBlock, ConstTag, EachBlock, Element, Fragment, FragmentNode, HtmlTag,
    IfBlock, KeyBlock, RenderTag, Root, SnippetBlock, SpecialElement, SpecialElementKind,
    SvelteView,
};
use tsv_ts::ast::{
    ArrowFunctionBody, AstView, ClassBody, ClassMember, ExportDefaultValue, ExprId, Expression,
    ExpressionKind, ForInOfLeft, ForInit, ForLeftId, FunctionExpression, Identifier,
    ObjectPatternProperty, ObjectProperty, StatementKind, StmtId, VariableDeclaration,
    VariableDeclarationKind,
};

use crate::analyze::{NameSet, RuneInit, classify_rune_init, pattern_binding_names};
use crate::attr_refs::{
    each_attribute_expression, each_reference_bearing_attribute_expression,
    special_element_reference_expression,
};
use crate::refusal;
use crate::script_decls::plain_identifier_str;
use crate::snippet_emit::render_call_expression;
use crate::{CompileError, Refusal};

/// The accumulating analysis state.
#[expect(clippy::struct_excessive_bools)] // independent monotonic accumulator flags, not a state machine
struct Nc<'a> {
    /// The template's view.
    sv: SvelteView<'a>,
    /// The TypeScript view: both scripts and every template expression.
    view: AstView<'a>,
    source: &'a str,
    /// Prop + import names — the roots whose member/call access is unsafe.
    context_roots: &'a NameSet,
    /// The instance script's `$props()` **rest_prop** binding names — the roots
    /// whose `.$$…` member access the oracle rejects
    /// (`props_illegal_name`, `MemberExpression.js`). A `$props()`
    /// declarator produces a `rest_prop` in exactly two forms: the whole-object
    /// `let props = $props()` and the REST element of `let { a, ...rest } =
    /// $props()`; the named props are `prop`, not `rest_prop`. See
    /// [`collect_rest_prop_names`].
    rest_prop_names: &'a NameSet,
    /// Top-level component binding names — the store-subscription base set. Any
    /// `$name` reference whose `$`-stripped base is here is a store access, so
    /// the component needs the `$$store_subs` injection (the oracle's gate:
    /// `analysis.instance.scope.declarations` holds a `store_sub` binding).
    store_names: &'a NameSet,
    /// Top-level `const` declarator + import-local names, from the instance
    /// **and** module scripts — the oracle's `constant_assignment` /
    /// `constant_binding` set (it keys on the DECLARATION KEYWORD, so a
    /// `const c = $state(0)` is in here despite being reactive).
    constants: &'a NameSet,
    /// `{#each}` context binding names currently in scope — the oracle's
    /// `each_item_invalid_assignment` set. Block-scoped: entered on the way into
    /// an `{#each}` body and restored on the way out.
    each_items: NameSet,
    /// `{#snippet}` parameter names currently in scope — the oracle's
    /// `snippet_parameter_assignment` set. Block-scoped like `each_items`.
    snippet_params: NameSet,
    /// TEMPLATE-scoped `const` names currently in scope — a `{@const}` name, a
    /// `{:then}`/`{:catch}` value, and an `{#each}` INDEX. All three are
    /// `declaration_kind: 'const'` to the oracle (the `VariableDeclaration`
    /// visitor in `phases/scope.js` via the `ConstTag` parent test, and its
    /// `AwaitBlock` and `EachBlock` visitors), so a write to one is `constant_assignment` —
    /// unlike the each ITEM beside the index, which is `kind: 'each'` and is
    /// EXCLUDED from that rule by `validate_no_const_assignment` in favor of its
    /// own.
    ///
    /// Block-scoped like [`Nc::each_items`], at the extent the oracle's own scope
    /// covers: a `{@const}` to its enclosing FRAGMENT (every `Fragment` gets a
    /// child scope, and a fragment holding a declaration tag is never porous —
    /// the parser's `pop` in `1-parse/index.js`), a `{:then}`/`{:catch}` value to that branch's
    /// fragment, an `{#each}` index to body + fallback.
    ///
    /// Consulted AFTER [`Nc::js_scope`] (a JS scope always nests inside a template
    /// one, so a handler parameter shadowing a `{@const}` still wins) but BEFORE
    /// `each_items`/`snippet_params`. That order is the SAFE one where the two
    /// disagree: the const rule fires at any pattern depth while the each/snippet
    /// rules fire only on a whole-identifier target, so consulting the const set
    /// last could drop a refusal the oracle raises. The cost is only a mislabeled
    /// refusal in the exotic shadowing case (a `{@const}` shadowed by an inner
    /// `{#each}` item of the same name) — the oracle rejects that write too, just
    /// under the other rule's message.
    template_consts: NameSet,
    /// Names bound anywhere below the component's top-level instance scope.
    shadowed: NameSet,
    /// The JS bindings of the scopes currently OPEN around the walk — a function's
    /// parameters and name, a nested `let`/`const`/`var`/`class`/function
    /// declaration, a `catch` parameter, a `for`-head binding. Unlike
    /// [`Nc::shadowed`] (a whole-component union, deliberately cumulative) this is
    /// scoped: pushed at the declaration and popped when its scope closes, so it
    /// answers "which JS binding of this name is in scope here, and of what kind?".
    /// `refuse_invalid_assign_target` reads it first, because a JS scope is always
    /// innermost relative to the template scopes (`each_items`/`snippet_params`) —
    /// template blocks never nest inside a JS scope, only the reverse. A lookup
    /// scans BACKWARD, so it finds the innermost binding of a name.
    ///
    /// It is a stack rather than a set because the binding's KIND is part of the
    /// answer: a nested `const` is not a plain shadow that suppresses the rule, it
    /// is `declaration_kind: 'const'` to the oracle and carries
    /// `constant_assignment` itself. A set could record that a name is bound and
    /// that some binding of it is `const`, but not WHICH of two nested ones is
    /// innermost — and the two orderings have opposite verdicts
    /// (`let a; { const a; a = 1 }` refuses, `const a; { let a; a = 1 }` compiles).
    /// Scanning back gets both right, and makes a re-declaration of an open name
    /// self-restoring: it is pushed like any other, and popping it uncovers the
    /// outer entry that was never disturbed.
    js_scope: Vec<JsBinding>,
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
    /// Current **non-arrow** function nesting depth — the oracle's
    /// `invalid_arguments_usage` ancestor test (`Identifier.js`): the count of
    /// `FunctionDeclaration`/`FunctionExpression` ancestors around the walk.
    /// ⚠️ DISTINCT from [`Nc::fn_depth`], which ALSO counts arrows, static blocks,
    /// and `{#snippet}` bodies — none of which suppress the `arguments` rule. A
    /// reference to `arguments` refuses exactly when this is 0 (see
    /// [`Refusal::InvalidArgumentsUsage`]). Incremented at the three non-arrow
    /// function sites only — `walk_function_expression`, the
    /// `StatementKind::FunctionDeclaration` arm, and the
    /// `ExportDefaultValue::FunctionDeclaration` arm — BEFORE the params walk, so a
    /// function parameter default (`function f(g = arguments)`) counts as inside
    /// the function while an arrow's (`(g = arguments) =>`) does not.
    nonarrow_fn_depth: u32,
    /// Set by any `$$slots` reference (the oracle's `uses_slots`): the component
    /// gains `const $$slots = $.sanitize_slots($$props)` and the `$$props` param.
    uses_slots: bool,
    /// Set by any valid `$name` store reference anywhere in the component (read
    /// or write, emitted or dropped): the component gains the `var $$store_subs;`
    /// / `$.unsubscribe_stores(…)` injection. Analysis-driven, exactly like the
    /// oracle's store-subscription gate — so a store referenced only in a dropped
    /// event handler still injects.
    uses_stores: bool,
    /// Whether the walk is inside a **dropped** `{:catch}` subtree, where the
    /// emitter never walks the fragment so the emission refusals that let the
    /// default attribute traversal skip element spreads / directives / `{@attach}`
    /// never fire. There a `new`/prop-rooted access in such a position must still
    /// trigger the wrapper, so those positions are walked. Sticky for the whole
    /// catch subtree.
    in_dropped_catch: bool,
}

/// One JS binding open around the walk (an entry of [`Nc::js_scope`]).
struct JsBinding {
    name: String,
    /// Whether the oracle would read this binding as `declaration_kind: 'const'`,
    /// i.e. whether a write to it is `constant_assignment`.
    is_const: bool,
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
    /// Whether the component makes any valid `$name` store reference anywhere
    /// (read or write, emitted or dropped) — the `var $$store_subs;` /
    /// `$.unsubscribe_stores(…)` injection gate.
    pub uses_stores: bool,
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
/// `instance_body` and `module_body` are the **type-erased** instance- and
/// module-script statement lists (see `erase`), not the un-erased
/// `root.{instance,module}.content.body` — those still carry TypeScript nodes the
/// walk must never see. The oracle's phase-2 analysis spans the module body too: a
/// module import member/call, or a module-body `new`, fires the wrapper, and a
/// module `let` reassigned anywhere stays dynamic.
#[expect(clippy::too_many_arguments)] // the two views beside the document, both bodies and the two name sets
pub(crate) fn analyze_component(
    root: &Root,
    sv: SvelteView<'_>,
    view: AstView<'_>,
    source: &str,
    instance_body: Run<StmtId>,
    module_body: Run<StmtId>,
    store_names: &NameSet,
    constants: &NameSet,
) -> Result<ComponentContext, CompileError> {
    let mut context_roots = NameSet::default();
    collect_context_roots(view, instance_body, source, &mut context_roots);
    collect_context_roots(view, module_body, source, &mut context_roots);

    // The rest_prop roots are collected from the INSTANCE body only: a module
    // `$props()` refuses upstream (`analyze_module_script`), so no rest_prop
    // binding ever originates in the module script.
    let mut rest_prop_names = NameSet::default();
    collect_rest_prop_names(view, instance_body, source, &mut rest_prop_names);

    let mut nc = Nc {
        sv,
        view,
        source,
        context_roots: &context_roots,
        rest_prop_names: &rest_prop_names,
        store_names,
        constants,
        each_items: NameSet::default(),
        snippet_params: NameSet::default(),
        template_consts: NameSet::default(),
        shadowed: NameSet::default(),
        js_scope: Vec::new(),
        member_roots: NameSet::default(),
        needs: false,
        refuse: None,
        reassigned: NameSet::default(),
        fn_declared: NameSet::default(),
        fn_depth: 0,
        nonarrow_fn_depth: 0,
        uses_slots: false,
        uses_stores: false,
        in_dropped_catch: false,
    };

    // Each phase is its own JS-scope root: nothing a script's nested scopes
    // declared may still be open when the next script or the template is walked.
    // Every scope-introducing node already restores itself, so these rewinds are
    // belt-and-braces — but they are what makes "a shadow cannot outlive its
    // phase" hold by construction rather than by auditing every walk arm (the
    // one direction that would turn a safe over-refusal into an over-acceptance).
    for &stmt in view.stmt_list(instance_body) {
        walk_stmt(stmt, &mut nc, false);
    }
    js_scope_restore(&mut nc, 0);
    for &stmt in view.stmt_list(module_body) {
        walk_stmt(stmt, &mut nc, false);
    }
    js_scope_restore(&mut nc, 0);
    walk_fragment(root.fragment, &mut nc);

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
        uses_stores: nc.uses_stores,
    })
}

fn unsupported(reason: Refusal) -> CompileError {
    CompileError::Unsupported(reason)
}

/// The plain (non-escaped) name of an identifier, `None` for a unicode-escaped
/// or synthetic identifier.
fn plain_name<'s>(id: &Identifier, source: &'s str) -> Option<&'s str> {
    plain_identifier_str(id, source)
}

/// Collect the top-level prop (incl. rest-prop) and import names into
/// `context_roots` — the roots whose member/call access sets `needs_context`.
fn collect_context_roots(
    view: AstView<'_>,
    instance_body: Run<StmtId>,
    source: &str,
    out: &mut NameSet,
) {
    for &stmt in view.stmt_list(instance_body) {
        match &view.stmt(stmt).kind {
            StatementKind::ImportDeclaration(import) => {
                use tsv_ts::ast::ImportSpecifier;
                for spec in view.import_specs(view.import_decl(*import).specifiers) {
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
            StatementKind::VariableDeclaration(decl) => {
                for declarator in view.declarators(decl.declarations) {
                    let is_props = declarator
                        .init
                        .and_then(|init| classify_rune_init(view, view.expr(init), source))
                        .is_some_and(|r| matches!(r, RuneInit::Props));
                    if is_props {
                        let mut names = Vec::new();
                        // Best-effort: a malformed props pattern is refused later
                        // by the binding analysis; here we simply record what we
                        // can resolve.
                        let _ = pattern_binding_names(
                            view,
                            view.expr(declarator.id),
                            source,
                            &mut names,
                        );
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

/// Collect the instance script's `$props()` **rest_prop** binding names — the
/// roots whose `.$$…` member access the oracle's `props_illegal_name`
/// rejects (`MemberExpression.js`).
///
/// A `$props()` declarator produces a `rest_prop` binding in exactly two forms
/// (`VariableDeclarator.js`):
///
/// - the whole-object `let props = $props()` — an Identifier `node.id`, whose
///   binding becomes `rest_prop`;
/// - the REST element of a destructure `let { a, ...rest } = $props()` —
///   `path.is_rest`. The NAMED props (`a`) are `prop`/`bindable_prop`,
///   NOT `rest_prop`, so they are deliberately excluded here (unlike
///   [`collect_context_roots`], which takes every prop name).
///
/// An escaped rest-prop ROOT binding returns `None` from [`plain_name`] and falls
/// through — a narrow residual left by DESIGN: `rest_prop_names` is a plain-name
/// set, and the member-site check that reads it decodes only the PROPERTY name (the
/// `$$…` half). The declare-site `$$`-key check
/// (`script_props::rewrite_props_pattern`) now DECODES, so it is no longer paired
/// with this one.
fn collect_rest_prop_names(
    view: AstView<'_>,
    instance_body: Run<StmtId>,
    source: &str,
    out: &mut NameSet,
) {
    for &stmt in view.stmt_list(instance_body) {
        let StatementKind::VariableDeclaration(decl) = &view.stmt(stmt).kind else {
            continue;
        };
        for declarator in view.declarators(decl.declarations) {
            let is_props = declarator
                .init
                .and_then(|init| classify_rune_init(view, view.expr(init), source))
                .is_some_and(|r| matches!(r, RuneInit::Props));
            if !is_props {
                continue;
            }
            match &view.expr(declarator.id).kind {
                ExpressionKind::Identifier(id) => {
                    if let Some(name) = plain_name(id, source) {
                        out.insert(name.to_string());
                    }
                }
                ExpressionKind::ObjectPattern(obj) => {
                    for prop in view.pat_props(obj.properties) {
                        if let ObjectPatternProperty::RestElement(rest) = prop
                            && let ExpressionKind::Identifier(id) = &view.expr(rest.argument).kind
                            && let Some(name) = plain_name(id, source)
                        {
                            out.insert(name.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// The names an assignment, an update, or a `bind:` may not write to — one set,
/// because the oracle reaches ONE validator from all three positions
/// (`validate_assignment`, `phases/2-analyze/visitors/shared/utils.js`).
///
/// It keys `constant_assignment` / `constant_binding` on the DECLARATION KEYWORD,
/// not on inferred reassignability — an `import` local, or a `const` binding whose
/// kind is not `each` (`validate_no_const_assignment`) — so a `const c = $state(0)`
/// is in here despite being reactive. That is why it is its own set and not a filter
/// on `state_names`.
///
/// Spans BOTH script bodies: the declaration keyword is what the rule reads, and a
/// module-script `const`/import is exactly as unrebindable as an instance one.
///
/// ⚠️ **Top-level statements only.** A `const` in a nested block or function body,
/// and every TEMPLATE-scoped const (a `{@const}` name, a `{:then}`/`{:catch}`
/// value, an `{#each}` INDEX — all `declaration_kind: 'const'` to the oracle) are
/// absent, so writing to one still compiles. A residual over-acceptance, tracked
/// in `../../docs/checklist_svelte_compiler.md`.
pub(crate) fn collect_constant_names(
    view: AstView<'_>,
    instance_body: Run<StmtId>,
    module_body: Run<StmtId>,
    source: &str,
) -> NameSet {
    use tsv_ts::ast::ImportSpecifier;

    let mut out = NameSet::default();
    for &stmt in view
        .stmt_list(instance_body)
        .iter()
        .chain(view.stmt_list(module_body))
    {
        match &view.stmt(stmt).kind {
            StatementKind::VariableDeclaration(decl)
                if decl.kind == VariableDeclarationKind::Const =>
            {
                for declarator in view.declarators(decl.declarations) {
                    // Best-effort: a pattern this cannot enumerate is one
                    // `analyze_declarator` refuses anyway, so nothing reaches a
                    // write gate unnamed.
                    let mut names = Vec::new();
                    if pattern_binding_names(view, view.expr(declarator.id), source, &mut names)
                        .is_ok()
                    {
                        out.extend(names);
                    }
                }
            }
            StatementKind::ImportDeclaration(import) => {
                for spec in view.import_specs(view.import_decl(*import).specifiers) {
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
            _ => {}
        }
    }
    out
}

/// Refuse an assignment / update / `bind:` target the oracle rejects outright —
/// its `validate_assignment` family
/// (`phases/2-analyze/visitors/shared/utils.js`, one function reached from
/// `AssignmentExpression`, `UpdateExpression` and `BindDirective` alike, which is
/// why one walk covers all three positions).
///
/// The oracle's shape is reproduced exactly, and the two halves recurse
/// differently:
///
/// - `validate_no_const_assignment` recurses through **patterns** —
///   `ArrayPattern` elements and `ObjectPattern` property *values*, and nothing
///   else. A `RestElement` and an `AssignmentPattern` match no branch and fall
///   through with **no** error, so `[...rest] = x` and `[c = 1] = x` are accepted
///   even for a `const` `rest`/`c`; so is a `MemberExpression` target, which
///   mutates through the binding rather than rebinding it.
/// - the `each` / `snippet` rules (in `validate_assignment`) test `argument.type === 'Identifier'`
///   on the argument **itself** and never recurse — hence the `top` flag.
///
/// TypeScript wrappers are peeled because the oracle analyzes a type-stripped
/// tree, where `(x as any) = 1` is simply `x = 1`; parentheses are peeled because
/// ESTree has no paren node.
///
/// Set membership is **scoped, not merely name-based**. `each_items` /
/// `snippet_params` are block-scoped to their template blocks, and a nested JS
/// re-declaration — a function parameter, a `catch` parameter, a `for`-head
/// binding, a local `let`/`const`/`var`/`class`/function sharing a name with a
/// component `const` — enters [`Nc::js_scope`] for exactly its scope, as the
/// oracle's scope chain does.
///
/// ⚠️ **Recording a binding is not the same as suppressing the rule**, and
/// conflating the two was a live over-acceptance. Every JS binding form enters
/// the scope stack, but what it then means splits by KIND: a `let`/`var`/
/// parameter/`catch`/function/class binding carries no rule, so a write to it is
/// accepted; a nested `const` is `declaration_kind: 'const'` to the oracle
/// wherever it sits, so it carries `constant_assignment` itself and the write
/// REFUSES. Treating the stack as a uniform "shadow ⇒ no rule" set therefore
/// compiled a write the oracle rejects. The stack stores `is_const` per entry
/// precisely so the two cannot be conflated again.
///
/// The enumeration of declaration FORMS is deliberately allowed to be
/// **incomplete**, but ⚠️ that incompleteness is **not unconditionally safe**, and
/// reading it as such was itself a live over-acceptance. A form the walk does not
/// record leaves no binding, so the write falls through — and what it falls
/// through TO decides the direction:
///
/// - the name is ALSO in a component-level set (a top-level `const`, an import
///   local, an `{#each}` item, a `{#snippet}` parameter): the fall-through hits
///   that set's rule and REFUSES. The write was really to the unrecorded local, so
///   this is an over-refusal — safe, and what the refusal contract permits;
/// - the name is purely LOCAL: the fall-through hits **nothing at all**, no rule
///   applies, and the write is ACCEPTED. If that local was a `const`, the oracle
///   rejects it and tsv has an OVER-ACCEPTANCE — a refusal-contract bug.
///
/// So a missing form is safe exactly for the NON-const forms (which carry no rule
/// either way) and for a name that collides with a component-level one. A missing
/// `const` form is a bug. Two such gaps were closed by recording the binding
/// rather than by narrowing anything — a `switch` now gets ONE scope shared by all
/// its cases (the oracle's `SwitchStatement: create_block_scope`), and a block's
/// `const` declarations hoist into scope before its statements are walked
/// ([`hoist_block_consts`], the oracle's scope pre-pass). Known remaining gaps:
///
/// - a `var` is recorded at its BLOCK, not hoisted to its function scope, so a
///   write to it elsewhere in the function still refuses. SAFE — a `var` is never
///   `const`, so the miss can only over-refuse;
/// - a non-`const` declaration (`let`, `class`, a function name) is recorded where
///   the walk REACHES it rather than hoisted, so a write textually before it still
///   refuses. SAFE for the same reason; the hoist is deliberately `const`-only
///   because hoisting a rule-free binding could only REMOVE a refusal;
/// - a class EXPRESSION's own name (`const C = class C { … }`) is not recorded —
///   only a class *declaration*'s is. SAFE: that name is `'let'` to the oracle
///   (`phases/scope.js`'s `ClassDeclaration` visitor), not `const`.
///
/// The TEMPLATE-scoped consts — a `{@const}`, a `{:then}`/`{:catch}` value, an
/// `{#each}` index — were the one gap of this shape that was NOT safe (`const` to
/// the oracle, purely template-local, so an unrecorded one would fall through to nothing
/// at all and the write be accepted). They are recorded in
/// [`Nc::template_consts`], which the lookup below consults between `js_scope` and
/// the two template-rule sets.
///
/// ⚠️ **One write position MASKS that rule**, and the masking has already been
/// mistaken for a refutation of this whole entry. An assignment sitting directly in
/// an emitted template expression (`{(c = 2)}`) refuses as `mutation inside a
/// template expression`, an unrelated general rule that fires whatever the target
/// is — verified target-independent, since a plain `let` write there refuses too
/// while the oracle COMPILES it. So the most natural repro reads green either way,
/// and a probe of this family must use an event-handler arrow
/// (`onclick={() => (c = 2)}`) or a write in a dropped `{:catch}`. When something
/// refuses, establish WHICH rule caught it — `compile_corpus_compare` reports the
/// tsv-side reason on every oracle-rejected file for exactly this purpose.
///
/// The one direction that must never occur is a binding that OUTLIVES its scope:
/// it would suppress a genuine refusal and produce an over-acceptance.
/// [`js_scope_restore`] is what forecloses it.
///
/// Sets `nc.refuse` rather than returning: like every other refusal this walk
/// raises, it is resolved late so the script-loop refusals keep winning for an
/// input that trips both.
fn refuse_invalid_assign_target(target: &Expression, nc: &mut Nc<'_>, top: bool) {
    match &target.kind {
        ExpressionKind::Identifier(id) => {
            let Some(name) = plain_name(id, nc.source) else {
                return;
            };
            // Innermost binding wins: an `{#each}` item / `{#snippet}` parameter
            // SHADOWS a same-named script `const`, and its own rule is the one the
            // oracle applies (an each binding is `declaration_kind: 'const'` too,
            // but `validate_no_const_assignment` excludes `kind === 'each'`).
            // A JS binding open around this write SHADOWS every component-level
            // and template-level name, so the write resolves to that binding and
            // the outer names are irrelevant. Checked first because a JS scope is
            // always the innermost one here: a template block can never nest
            // inside a JS scope. Which rule then applies is the INNERMOST JS
            // binding's own: a `const` is `declaration_kind: 'const'` to the
            // oracle wherever it is declared, so it carries `constant_assignment`
            // exactly as a top-level one does; every other binding form carries no
            // rule at all.
            let reason = if let Some(binding) = js_binding(nc, name) {
                binding
                    .is_const
                    .then_some(refusal::INVALID_ASSIGNMENT_CONSTANT)
            } else if nc.template_consts.contains(name) {
                // A `{@const}` name, a `{:then}`/`{:catch}` value, an `{#each}`
                // INDEX: `declaration_kind: 'const'` to the oracle, so
                // `constant_assignment` at ANY pattern depth — hence no `top`
                // gate, unlike the two template rules below.
                Some(refusal::INVALID_ASSIGNMENT_CONSTANT)
            } else if nc.each_items.contains(name) {
                top.then_some(refusal::INVALID_ASSIGNMENT_EACH_ITEM)
            } else if nc.snippet_params.contains(name) {
                top.then_some(refusal::INVALID_ASSIGNMENT_SNIPPET_PARAMETER)
            } else if nc.constants.contains(name) {
                Some(refusal::INVALID_ASSIGNMENT_CONSTANT)
            } else {
                None
            };
            if let Some(target) = reason
                && nc.refuse.is_none()
            {
                nc.refuse = Some(Refusal::InvalidAssignmentTarget { target });
            }
        }
        ExpressionKind::ArrayPattern(pattern) => {
            for &element in nc.view.expr_opt_list(pattern.elements).iter().flatten() {
                refuse_invalid_assign_target(nc.view.expr(element), nc, false);
            }
        }
        ExpressionKind::ObjectPattern(pattern) => {
            for prop in nc.view.pat_props(pattern.properties) {
                if let ObjectPatternProperty::Property(p) = prop {
                    refuse_invalid_assign_target(nc.view.expr(p.value), nc, false);
                }
            }
        }
        ExpressionKind::TSNonNullExpression(t) => {
            refuse_invalid_assign_target(nc.view.expr(t.expression), nc, top);
        }
        ExpressionKind::TSAsExpression(t) => {
            refuse_invalid_assign_target(nc.view.expr(t.expression), nc, top);
        }
        ExpressionKind::TSSatisfiesExpression(t) => {
            refuse_invalid_assign_target(nc.view.expr(t.expression), nc, top);
        }
        ExpressionKind::TSTypeAssertion(t) => {
            refuse_invalid_assign_target(nc.view.expr(t.expression), nc, top);
        }
        // A `MemberExpression` target (and everything else) matches no oracle
        // branch — it writes THROUGH the binding, never rebinds it.
        _ => {}
    }
}

/// Enter a template block scope: add `names` to `set` and return the ones that
/// were newly added, for [`exit_block_scope`] to remove on the way out. A name
/// already present belongs to an enclosing block of the same kind and must
/// survive the restore.
fn enter_block_scope(set: &mut NameSet, names: Vec<String>) -> Vec<String> {
    names
        .into_iter()
        .filter(|name| set.insert(name.clone()))
        .collect()
}

/// Undo an [`enter_block_scope`].
fn exit_block_scope(set: &mut NameSet, added: Vec<String>) {
    for name in added {
        set.remove(&name);
    }
}

/// The names an optional template binding pattern declares, for
/// [`Nc::template_consts`].
///
/// Deliberately NOT paired with the `enter_block_scope`/`exit_block_scope` calls
/// in a single helper: the no-leak property rests on every enter having a visible
/// exit on the same straight-line path, and folding the pair behind a function
/// would move that audit off the page.
fn template_const_names(view: AstView<'_>, pattern: Option<ExprId>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(pattern) = pattern {
        let _ = pattern_binding_names(view, view.expr(pattern), source, &mut names);
    }
    names
}

/// Open a JS scope: the mark [`js_scope_restore`] rewinds to.
fn js_scope_mark(nc: &Nc<'_>) -> usize {
    nc.js_scope.len()
}

/// Close a JS scope opened at `mark`, removing every binding declared inside it.
///
/// Truncating the stack is what makes this **leak-proof by construction**: whatever
/// the walk did in between, no binding pushed after `mark` can survive. That
/// direction is the load-bearing one — a leaked shadow would suppress a genuine
/// `constant_assignment` refusal, an over-acceptance and a refusal-contract
/// violation, where a MISSING shadow only over-refuses.
fn js_scope_restore(nc: &mut Nc<'_>, mark: usize) {
    nc.js_scope.truncate(mark);
}

/// Record a JS binding in the innermost open scope (see [`Nc::js_scope`]).
fn declare_js_local(name: String, is_const: bool, nc: &mut Nc<'_>) {
    nc.js_scope.push(JsBinding { name, is_const });
}

/// The innermost open JS binding of `name`, or `None` when the name is not bound
/// by any JS scope around the walk.
fn js_binding<'b>(nc: &'b Nc<'_>, name: &str) -> Option<&'b JsBinding> {
    nc.js_scope
        .iter()
        .rev()
        .find(|binding| binding.name == name)
}

/// [`declare_pattern`] plus the JS-scope record. The two are separate entry points
/// on purpose: the TEMPLATE binding sites (an `{#each}` context, a `{#snippet}`
/// parameter) call the bare form, because each has its own oracle rule and folding
/// it into `js_scope` would SUPPRESS that rule's refusal.
///
/// `is_const` is true only for a `const` declarator (including a `for (const … of
/// …)` head) — the one JS binding form the oracle reads as
/// `declaration_kind: 'const'`. Every other site (a parameter, a `catch` binding,
/// a `let`/`var`, a function or class name) passes false. A `using` /
/// `await using` declarator is deliberately NOT const here, on the SOURCE reading
/// alone: the oracle's test is `declaration_kind === 'const'` exactly
/// (`shared/utils.js`), and forbidding a write to one is JS's own early
/// error rather than a Svelte rule. ⚠️ The behavioral half is **undemonstrable**
/// against the pinned oracle, which cannot parse `using` at all (its acorn rejects
/// the declaration outright with `js_parse_error`), so this choice is unreachable
/// there and untested by probe. That parse gap is itself a standing tsv
/// over-acceptance, owed to the frontend — see
/// `../../docs/checklist_svelte_compiler.md` §Frontend-rooted over-acceptances.
fn declare_js_pattern(pattern: &Expression, nc: &mut Nc<'_>, is_const: bool) {
    declare_pattern(pattern, nc);
    let mut names = Vec::new();
    if pattern_binding_names(nc.view, pattern, nc.source, &mut names).is_ok() {
        for name in names {
            declare_js_local(name, is_const, nc);
        }
    }
}

/// [`declare_ident`] plus the JS-scope record. A function or class NAME is never a
/// `const` binding.
fn declare_js_ident(id: &Identifier, nc: &mut Nc<'_>) {
    declare_ident(id, nc);
    if let Some(name) = plain_name(id, nc.source) {
        let name = name.to_string();
        declare_js_local(name, false, nc);
    }
}

/// Record the `const` declarations of a statement list into the CURRENTLY open JS
/// scope, before any of its statements is walked.
///
/// The oracle builds its scopes in a **pre-pass** (`phases/scope.js`'s
/// `create_scopes`), so every binding of a block exists before any reference in
/// that block is validated: a write textually EARLIER than the `const` still
/// resolves to it and is `constant_assignment` (a TDZ error at runtime, a compile
/// error to Svelte either way).
///
/// Deliberately `const`-only. Hoisting a `let`/`class`/function name would also be
/// faithful to the pre-pass, but those bindings carry NO rule — recording one
/// earlier can only turn a refusal into an acceptance, the unsafe direction. A
/// `const` can only add refusals, so the hoist is safe by construction; the
/// non-const forms stay recorded where the walk reaches them, an over-refusal the
/// refusal contract permits.
///
/// A `const` recorded here is recorded a second time when [`walk_var_decl`]
/// reaches its declarator. The duplicate entry is benign — same name, same kind,
/// and the backward scan finds either — so the two paths are left independent.
fn hoist_block_consts(stmts: Run<StmtId>, nc: &mut Nc<'_>) {
    let view = nc.view;
    for &stmt in view.stmt_list(stmts) {
        if let StatementKind::VariableDeclaration(decl) = &view.stmt(stmt).kind
            && decl.kind == VariableDeclarationKind::Const
        {
            for declarator in view.declarators(decl.declarations) {
                declare_js_pattern(view.expr(declarator.id), nc, true);
            }
        }
    }
}

/// Walk a nested statement list as its own JS block scope.
fn walk_block(stmts: Run<StmtId>, nc: &mut Nc<'_>) {
    let mark = js_scope_mark(nc);
    hoist_block_consts(stmts, nc);
    for &stmt in nc.view.stmt_list(stmts) {
        walk_stmt(stmt, nc, true);
    }
    js_scope_restore(nc, mark);
}

/// Add every name a binding pattern declares to `shadowed` (best-effort — a
/// pattern shape `pattern_binding_names` can't collect just records nothing, so
/// the set stays a superset of the true nested bindings). Inside a function-like
/// subtree the names also join `fn_declared` (see the field docs).
fn declare_pattern(pattern: &Expression, nc: &mut Nc<'_>) {
    let mut names = Vec::new();
    if pattern_binding_names(nc.view, pattern, nc.source, &mut names).is_ok() {
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
fn declare_ident(id: &Identifier, nc: &mut Nc<'_>) {
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
/// refusal. `CommentBoundPair` is the sixth transparent wrapper (valid JS, and
/// the oracle has no such node at all).
fn root_of<'e>(view: AstView<'e>, expr: &'e Expression) -> &'e Expression {
    let mut node = expr;
    loop {
        let next = match &node.kind {
            ExpressionKind::MemberExpression(m) => m.object,
            ExpressionKind::TSAsExpression(t) => t.expression,
            ExpressionKind::TSSatisfiesExpression(t) => t.expression,
            ExpressionKind::TSNonNullExpression(t) => t.expression,
            ExpressionKind::TSTypeAssertion(t) => t.expression,
            ExpressionKind::TSInstantiationExpression(t) => t.expression,
            ExpressionKind::CommentBoundPair(j) => view.comment_pair(*j).inner,
            _ => return node,
        };
        node = view.expr(next);
    }
}

/// Classify a member/call access by its root (`is_safe_identifier`): a
/// non-identifier root is unsafe (→ `needs`); a plain-identifier root that is a
/// context-root is recorded (resolved against `shadowed` at the end); an escaped
/// root can't be classified (→ refuse).
fn check_root(access: &Expression, nc: &mut Nc<'_>) {
    match &root_of(nc.view, access).kind {
        ExpressionKind::Identifier(id) => match plain_name(id, nc.source) {
            Some(name) => {
                // A `$name` store read roots at its base binding `name` (the
                // subscription reads `store`), so a member/call on a store whose
                // base is a prop/import is unsafe exactly as `store.foo` would be.
                // A rune callee (`$props()`, `$state.raw()`) is NOT a store read —
                // `store_read_base` returns `None`, so it keeps its `$name` form and
                // never matches a context root (as before this change).
                let root_name = crate::analyze::store_read_base(name).unwrap_or(name);
                if nc.context_roots.contains(root_name) {
                    nc.member_roots.insert(root_name.to_string());
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

fn walk_exprs(exprs: Run<ExprId>, nc: &mut Nc<'_>) {
    for &expr in nc.view.expr_list(exprs) {
        walk_id(expr, nc);
    }
}

fn walk_opt(expr: Option<ExprId>, nc: &mut Nc<'_>) {
    if let Some(expr) = expr {
        walk_id(expr, nc);
    }
}

/// [`walk_expr`] over the expression `id` names.
fn walk_id(id: ExprId, nc: &mut Nc<'_>) {
    walk_expr(nc.view.expr(id), nc);
}

/// Declare each of a function's parameters (a run of rows held by value) in its
/// scope, then walk it, one at a time.
fn walk_params(params: Run<Expression>, nc: &mut Nc<'_>) {
    for param in nc.view.exprs(params) {
        declare_js_pattern(param, nc, false);
        walk_expr(param, nc);
    }
}

/// Walk an expression: detect `new`/unsafe-member/unsafe-call triggers, and
/// collect any nested function/arrow/class bindings into `shadowed`.
fn walk_expr(expr: &Expression, nc: &mut Nc<'_>) {
    let view = nc.view;
    match &expr.kind {
        ExpressionKind::NewExpression(new_expr) => {
            nc.needs = true;
            walk_id(new_expr.callee, nc);
            walk_exprs(new_expr.arguments, nc);
        }
        ExpressionKind::CallExpression(call) => {
            // The oracle's `rune_invalid_spread` (`CallExpression.js`), checked
            // on every rune call BEFORE its dispatch and before recursing — so the
            // outer rune call's spread wins, matching the oracle's visit order. Any
            // rune but `$inspect`; fires wherever the call sits (this walk covers
            // the instance/module scripts and the template, pre-rewrite).
            if nc.refuse.is_none()
                && let Some(rune) = crate::analyze::rune_call_spread(view, call, nc.source)
            {
                nc.refuse = Some(Refusal::RuneInvalidSpread { rune });
            }
            check_root(view.expr(call.callee), nc);
            walk_id(call.callee, nc);
            walk_exprs(call.arguments, nc);
        }
        ExpressionKind::MemberExpression(member) => {
            // The oracle's `props_illegal_name` REFERENCE-site rule
            // (`MemberExpression.js`): a `rest_prop.$$…` access — a plain
            // Identifier object bound to a `$props()` rest_prop, and an
            // Identifier property whose name starts with `$$` (reserved for
            // Svelte internals). One error code, shared with the declare-site
            // `Refusal::PropsIllegalName`. Placed at the TOP of the arm,
            // mirroring the oracle placing it above its own
            // `is_safe_identifier`/`needs_context` check (the `check_root`
            // analog below), and first-wins (`nc.refuse.is_none()`).
            //
            // This matches the oracle's condition EXACTLY — `node.object.type
            // === 'Identifier' && node.property.type === 'Identifier'`, with NO
            // `computed` gate. `computed` must NOT be gated: a computed
            // IDENTIFIER key (`rest[$$slots]`) matches the oracle's condition
            // (its property IS an Identifier) and the oracle rejects it, but
            // `$$slots` is EXEMPT from tsv's own `$$`-ref rule
            // (`rune_guard.rs`, `if name != "$$slots"` — the legit sanitize_slots
            // reference), so gating on `!computed` here would suppress this rule
            // and nothing else would fire → an OVER-ACCEPTANCE. A computed STRING
            // key (`rest['$$slots']`) is excluded on its own: its property is a
            // Literal, not an Identifier, so the `ExpressionKind::Identifier(prop)`
            // arm fails — matching the oracle, which also compiles it.
            //
            // The property NAME is DECODED via `Identifier::name`,
            // so an escaped `$$` property (`rest.$$foo` written `$$foo`)
            // refuses too — the oracle reads the DECODED `node.name`. The object
            // ROOT stays span-identity (`plain_name`): `rest_prop_names` is a
            // plain-name set, so an escaped rest-prop ROOT binding is a separate,
            // narrower residual left by `collect_rest_prop_names`.
            if nc.refuse.is_none()
                && let ExpressionKind::Identifier(obj) = &view.expr(member.object).kind
                && let Some(obj_name) = plain_name(obj, nc.source)
                && nc.rest_prop_names.contains(obj_name)
                && let ExpressionKind::Identifier(prop) = &view.expr(member.property).kind
                && prop.name(nc.source, view).starts_with("$$")
            {
                nc.refuse = Some(Refusal::PropsIllegalName);
            }
            check_root(expr, nc);
            walk_id(member.object, nc);
            if member.computed {
                walk_id(member.property, nc);
            }
        }

        // Nested function scopes: their params/bindings shadow the component
        // scope, so record them and walk the body (always nested).
        ExpressionKind::ArrowFunctionExpression(a) => {
            let a = view.arrow_fn(*a);
            nc.fn_depth += 1;
            let mark = js_scope_mark(nc);
            walk_params(a.params, nc);
            match &a.body {
                ArrowFunctionBody::Expression(e) => walk_id(*e, nc),
                ArrowFunctionBody::BlockStatement(b) => walk_block(b.body, nc),
            }
            js_scope_restore(nc, mark);
            nc.fn_depth -= 1;
        }
        ExpressionKind::FunctionExpression(f) => walk_function_expression(view.fn_expr(*f), nc),
        ExpressionKind::ClassExpression(c) => walk_class_body(&view.class_expr(*c).body, nc),

        // A bare identifier reference: detect `$$slots` (the oracle's
        // `uses_slots`) and a `$name` store access (the store-subscription gate),
        // otherwise a leaf.
        ExpressionKind::Identifier(id) => {
            // The oracle's `invalid_arguments_usage` (`Identifier.js`): a
            // REFERENCE to `arguments` with no
            // `FunctionDeclaration`/`FunctionExpression` ancestor. This arm is
            // reached only for reference-position identifiers (the member-property /
            // object-key walks are `computed`-gated), so the `is_reference` guard is
            // satisfied by construction; the ancestor test is `nonarrow_fn_depth ==
            // 0` (arrows / snippet bodies / class field inits / static blocks do NOT
            // suppress it). The name is DECODED via `Identifier::name`, so an
            // escaped `arguments` reference outside a non-arrow function refuses
            // too — the oracle reads the DECODED `node.name`. Resolved late,
            // first-wins; the cheap gates short-circuit before the decode.
            if nc.nonarrow_fn_depth == 0
                && nc.refuse.is_none()
                && id.name(nc.source, view) == "arguments"
            {
                nc.refuse = Some(Refusal::InvalidArgumentsUsage);
            }
            // `uses_slots` / `uses_stores` are EMISSION gates (not refusals) that
            // key on the DECODED identifier name — the oracle reads `node.name`,
            // so an escaped `$$slots` or a `$name` store reference counts exactly
            // as its plain spelling. The plain fast path stays a span slice; only
            // an escaped identifier reads its escaped name's pool text.
            let decoded;
            let name = if let Some(name) = plain_name(id, nc.source) {
                name
            } else {
                decoded = id.name(nc.source, view).to_string();
                &decoded
            };
            if name == "$$slots" {
                nc.uses_slots = true;
            } else if crate::analyze::store_read_base(name)
                .is_some_and(|base| nc.store_names.contains(base))
            {
                nc.uses_stores = true;
            }
        }
        // Leaves — no children, no bindings.
        ExpressionKind::Literal(_)
        | ExpressionKind::PrivateIdentifier(_)
        | ExpressionKind::RegexLiteral(_)
        | ExpressionKind::ThisExpression(_)
        | ExpressionKind::Super(_)
        | ExpressionKind::MetaProperty(_) => {}

        ExpressionKind::ObjectExpression(obj) => {
            for prop in view.obj_props(obj.properties) {
                match prop {
                    ObjectProperty::Property(p) => {
                        if p.computed {
                            walk_id(p.key, nc);
                        }
                        walk_id(p.value, nc);
                    }
                    ObjectProperty::SpreadElement(s) => walk_id(s.argument, nc),
                }
            }
        }
        ExpressionKind::ArrayExpression(arr) => {
            for &element in view.expr_opt_list(arr.elements) {
                walk_opt(element, nc);
            }
        }
        ExpressionKind::UnaryExpression(u) => walk_id(u.argument, nc),
        ExpressionKind::UpdateExpression(u) => {
            crate::rune_guard::assign_target_roots(
                nc.view,
                nc.view.expr(u.argument),
                nc.source,
                &mut nc.reassigned,
            );
            refuse_invalid_assign_target(nc.view.expr(u.argument), nc, true);
            walk_id(u.argument, nc);
        }
        ExpressionKind::BinaryExpression(b) => {
            walk_id(b.left, nc);
            walk_id(b.right, nc);
        }
        ExpressionKind::ConditionalExpression(c) => {
            walk_id(c.test, nc);
            walk_id(c.consequent, nc);
            walk_id(c.alternate, nc);
        }
        ExpressionKind::SpreadElement(s) => walk_id(s.argument, nc),
        ExpressionKind::TemplateLiteral(t) => walk_exprs(t.expressions, nc),
        ExpressionKind::TaggedTemplateExpression(t) => {
            let t = view.tagged_tpl(*t);
            walk_id(t.tag, nc);
            walk_exprs(t.quasi.expressions, nc);
        }
        ExpressionKind::AwaitExpression(a) => walk_id(a.argument, nc),
        ExpressionKind::YieldExpression(y) => walk_opt(y.argument, nc),
        ExpressionKind::SequenceExpression(s) => walk_exprs(s.expressions, nc),
        ExpressionKind::AssignmentExpression(a) => {
            crate::rune_guard::assign_target_roots(
                nc.view,
                nc.view.expr(a.left),
                nc.source,
                &mut nc.reassigned,
            );
            refuse_invalid_assign_target(nc.view.expr(a.left), nc, true);
            walk_id(a.left, nc);
            walk_id(a.right, nc);
        }
        ExpressionKind::ObjectPattern(p) => {
            for prop in view.pat_props(p.properties) {
                match prop {
                    ObjectPatternProperty::Property(prop) => {
                        if prop.computed {
                            walk_id(prop.key, nc);
                        }
                        walk_id(prop.value, nc);
                    }
                    ObjectPatternProperty::RestElement(rest) => walk_id(rest.argument, nc),
                }
            }
        }
        ExpressionKind::ArrayPattern(p) => {
            for &element in view.expr_opt_list(p.elements) {
                walk_opt(element, nc);
            }
        }
        ExpressionKind::AssignmentPattern(p) => {
            walk_id(p.left, nc);
            walk_id(p.right, nc);
        }
        ExpressionKind::RestElement(r) => walk_id(r.argument, nc),
        ExpressionKind::TSTypeAssertion(t) => walk_id(t.expression, nc),
        ExpressionKind::TSAsExpression(t) => walk_id(t.expression, nc),
        ExpressionKind::TSSatisfiesExpression(t) => walk_id(t.expression, nc),
        ExpressionKind::TSInstantiationExpression(t) => walk_id(t.expression, nc),
        ExpressionKind::TSNonNullExpression(t) => walk_id(t.expression, nc),
        ExpressionKind::TSParameterProperty(t) => walk_id(t.parameter, nc),
        ExpressionKind::ImportExpression(i) => {
            walk_id(i.source, nc);
            walk_opt(i.options, nc);
        }
        ExpressionKind::CommentBoundPair(j) => walk_id(view.comment_pair(*j).inner, nc),
        ExpressionKind::ParenthesizedExpression(p) => walk_expr(p.unreachable_contents(view), nc),
    }
}

fn walk_function_expression(f: &FunctionExpression, nc: &mut Nc<'_>) {
    nc.fn_depth += 1;
    // A `FunctionExpression` suppresses `invalid_arguments_usage` — before the
    // params, so a parameter default (`function(g = arguments)`) counts as inside.
    nc.nonarrow_fn_depth += 1;
    let mark = js_scope_mark(nc);
    // A function EXPRESSION's own name binds inside its own scope only.
    if let Some(id) = &f.id {
        declare_js_ident(id, nc);
    }
    walk_params(f.params, nc);
    walk_block(f.body.body, nc);
    js_scope_restore(nc, mark);
    nc.nonarrow_fn_depth -= 1;
    nc.fn_depth -= 1;
}

fn walk_class_body(body: &ClassBody, nc: &mut Nc<'_>) {
    for member in nc.view.class_members(body.body) {
        match member {
            ClassMember::MethodDefinition(m) => {
                if m.computed {
                    walk_id(m.key, nc);
                }
                walk_function_expression(&m.value, nc);
            }
            ClassMember::PropertyDefinition(p) => {
                if p.computed {
                    walk_id(p.key, nc);
                }
                walk_opt(p.value, nc);
            }
            ClassMember::StaticBlock(b) => {
                nc.fn_depth += 1;
                walk_block(b.body, nc);
                nc.fn_depth -= 1;
            }
            ClassMember::IndexSignature(_) => {}
        }
    }
}

/// Walk a variable declaration: at nested depth its pattern names shadow the
/// component scope; the init/default expressions are always trigger-checked.
fn walk_var_decl(decl: &VariableDeclaration, nc: &mut Nc<'_>, shadow: bool) {
    let is_const = decl.kind == VariableDeclarationKind::Const;
    for declarator in nc.view.declarators(decl.declarations) {
        if shadow {
            declare_js_pattern(nc.view.expr(declarator.id), nc, is_const);
        }
        walk_id(declarator.id, nc);
        walk_opt(declarator.init, nc);
    }
}

fn walk_for_left(left: ForLeftId, nc: &mut Nc<'_>) {
    match nc.view.for_left(left) {
        ForInOfLeft::VariableDeclaration(decl) => walk_var_decl(decl, nc, true),
        ForInOfLeft::Pattern(pattern) => walk_id(*pattern, nc),
    }
}

/// Walk a statement. `shadow` is false only for the component's top-level
/// instance statements — where a declaration's own name is a component binding,
/// not a shadow; everywhere else (nested scopes, template) it is true.
fn walk_stmt(stmt: StmtId, nc: &mut Nc<'_>, shadow: bool) {
    let view = nc.view;
    match &view.stmt(stmt).kind {
        StatementKind::VariableDeclaration(d) => walk_var_decl(d, nc, shadow),
        StatementKind::FunctionDeclaration(f) => {
            let f = view.fn_decl(*f);
            // The declaration's own name binds in the ENCLOSING scope, so it is
            // recorded before the mark; the parameters and body are its own.
            if shadow && let Some(id) = &f.id {
                declare_js_ident(id, nc);
            }
            nc.fn_depth += 1;
            // A `FunctionDeclaration` suppresses `invalid_arguments_usage` — before
            // the params, so a parameter default counts as inside the function.
            nc.nonarrow_fn_depth += 1;
            let mark = js_scope_mark(nc);
            walk_params(f.params, nc);
            walk_block(f.body.body, nc);
            js_scope_restore(nc, mark);
            nc.nonarrow_fn_depth -= 1;
            nc.fn_depth -= 1;
        }
        StatementKind::ClassDeclaration(c) => {
            let c = view.class_decl(*c);
            if shadow && let Some(id) = &c.id {
                declare_js_ident(id, nc);
            }
            walk_class_body(&c.body, nc);
        }
        StatementKind::ExpressionStatement(s) => walk_id(s.expression, nc),
        StatementKind::ReturnStatement(s) => walk_opt(s.argument, nc),
        StatementKind::BlockStatement(s) => walk_block(s.body, nc),
        StatementKind::IfStatement(s) => {
            walk_id(s.test, nc);
            walk_stmt(s.consequent, nc, true);
            if let Some(alt) = s.alternate {
                walk_stmt(alt, nc, true);
            }
        }
        // A `for`-head binding scopes over the head AND the body, so the mark
        // wraps both.
        StatementKind::ForStatement(s) => {
            let mark = js_scope_mark(nc);
            match view.opt_for_head(s.init) {
                Some(ForInit::VariableDeclaration(d)) => walk_var_decl(d, nc, true),
                Some(ForInit::Expression(e)) => walk_id(*e, nc),
                None => {}
            }
            walk_opt(s.test, nc);
            walk_opt(s.update, nc);
            walk_stmt(s.body, nc, true);
            js_scope_restore(nc, mark);
        }
        StatementKind::ForInStatement(s) => {
            let mark = js_scope_mark(nc);
            walk_for_left(s.left, nc);
            walk_id(s.right, nc);
            walk_stmt(s.body, nc, true);
            js_scope_restore(nc, mark);
        }
        StatementKind::ForOfStatement(s) => {
            let mark = js_scope_mark(nc);
            walk_for_left(s.left, nc);
            walk_id(s.right, nc);
            walk_stmt(s.body, nc, true);
            js_scope_restore(nc, mark);
        }
        StatementKind::WhileStatement(s) => {
            walk_id(s.test, nc);
            walk_stmt(s.body, nc, true);
        }
        StatementKind::DoWhileStatement(s) => {
            walk_stmt(s.body, nc, true);
            walk_id(s.test, nc);
        }
        // Unreachable in practice: a Svelte `<script>` is Module code, so it is strict
        // and the parser refuses `with` there.
        StatementKind::WithStatement(s) => {
            walk_id(s.object, nc);
            walk_stmt(s.body, nc, true);
        }
        StatementKind::SwitchStatement(s) => {
            walk_id(s.discriminant, nc);
            // The oracle gives a `switch` ONE block scope shared by ALL its cases
            // (`phases/scope.js`: `SwitchStatement: create_block_scope`), so a
            // `const` in one case is in scope for a write in another and refuses.
            // Scoping per case instead was an OVER-ACCEPTANCE, not the over-refusal
            // it looked like: a case-local name has no component-level entry to fall
            // through to, so the write met no rule at all. Every case's consts hoist
            // first, so both orderings (const-then-write and write-then-const) see
            // the binding.
            let mark = js_scope_mark(nc);
            for case in view.cases(s.cases) {
                hoist_block_consts(case.consequent, nc);
            }
            for case in view.cases(s.cases) {
                walk_opt(case.test, nc);
                for &stmt in view.stmt_list(case.consequent) {
                    walk_stmt(stmt, nc, true);
                }
            }
            js_scope_restore(nc, mark);
        }
        StatementKind::TryStatement(s) => {
            let s = view.try_stmt(*s);
            walk_block(s.block.body, nc);
            if let Some(handler) = view.opt_catch(s.handler) {
                let mark = js_scope_mark(nc);
                if let Some(param) = &handler.param {
                    declare_js_pattern(param, nc, false);
                    walk_expr(param, nc);
                }
                walk_block(handler.body.body, nc);
                js_scope_restore(nc, mark);
            }
            if let Some(finalizer) = &s.finalizer {
                walk_block(finalizer.body, nc);
            }
        }
        StatementKind::ThrowStatement(s) => walk_id(s.argument, nc),
        StatementKind::LabeledStatement(s) => walk_stmt(view.labeled_stmt(*s).body, nc, true),
        StatementKind::ExportNamedDeclaration(s) => {
            if let Some(decl) = view.export_named(*s).declaration {
                walk_stmt(decl, nc, shadow);
            }
        }
        StatementKind::ExportDefaultDeclaration(s) => match &view.export_default(*s).declaration {
            ExportDefaultValue::Expression(e) => walk_id(*e, nc),
            ExportDefaultValue::FunctionDeclaration(f) => {
                nc.fn_depth += 1;
                // Suppresses `invalid_arguments_usage` — before the params.
                nc.nonarrow_fn_depth += 1;
                let mark = js_scope_mark(nc);
                walk_params(f.params, nc);
                walk_block(f.body.body, nc);
                js_scope_restore(nc, mark);
                nc.nonarrow_fn_depth -= 1;
                nc.fn_depth -= 1;
            }
            ExportDefaultValue::ClassDeclaration(c) => walk_class_body(&c.body, nc),
            ExportDefaultValue::TSDeclareFunction(_)
            | ExportDefaultValue::TSInterfaceDeclaration(_) => {}
        },
        StatementKind::TSExportAssignment(s) => walk_id(view.export_assign(*s).expression, nc),
        // No trigger-bearing children, or refused elsewhere (TS enum/module are
        // refused by type erasure before this analysis runs; the rune guard
        // refuses them too as its own defense in depth).
        StatementKind::BreakStatement(_)
        | StatementKind::ContinueStatement(_)
        | StatementKind::EmptyStatement(_)
        | StatementKind::DebuggerStatement(_)
        | StatementKind::ImportDeclaration(_)
        | StatementKind::ExportAllDeclaration(_)
        | StatementKind::TSNamespaceExportDeclaration(_)
        | StatementKind::TSImportEqualsDeclaration(_)
        | StatementKind::TSTypeAliasDeclaration(_)
        | StatementKind::TSInterfaceDeclaration(_)
        | StatementKind::TSDeclareFunction(_)
        | StatementKind::TSEnumDeclaration(_)
        | StatementKind::TSModuleDeclaration(_) => {}
    }
}

/// Walk a template fragment: trigger-check every rendered expression and record
/// the block-local bindings (each item/index, `{:then}`/`{:catch}` values,
/// `{@const}` names) that shadow the component scope.
fn walk_fragment(fragment: Fragment, nc: &mut Nc<'_>) {
    let (sv, view) = (nc.sv, nc.view);
    // The `{@const}` names of this fragment enter scope BEFORE any of its nodes is
    // walked, mirroring the oracle's scope PRE-PASS (`create_scopes` completes
    // before any reference is validated), so a write textually earlier than the
    // `{@const}` still resolves to it.
    let mut names = Vec::new();
    for node in sv.nodes(fragment.nodes) {
        if let FragmentNode::ConstTag(tag) = node {
            let _ = pattern_binding_names(view, view.expr(tag.id), nc.source, &mut names);
        }
    }
    let added = enter_block_scope(&mut nc.template_consts, names);
    for node in sv.nodes(fragment.nodes) {
        walk_fragment_node(node, nc);
    }
    exit_block_scope(&mut nc.template_consts, added);
}

fn walk_fragment_node(node: &FragmentNode, nc: &mut Nc<'_>) {
    match node {
        FragmentNode::Text(_) | FragmentNode::Comment(_) => {}
        FragmentNode::Element(element) => walk_element(element, nc),
        FragmentNode::ExpressionTag(tag) => walk_id(tag.expression, nc),
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
        // nothing), `<svelte:element>` (which emits `$.element(…)`), and
        // `<svelte:head>`/`<title>` (which emit `$.head`/`$$renderer.title`) — are
        // walked on the emitted path, because the oracle runs its phase-2 analysis
        // over their expressions regardless of what SSR emits: a `new`/prop-rooted
        // member/call in a `this={…}` / bind / handler, or inside a head child (a
        // `<meta content={new Date()}>` / `<title>{new Date()}</title>`), fires the
        // wrapper, and a `bind:` marks its target reassigned. The refused-at-emission
        // kinds are reachable only through a dropped `{:catch}` (matched explicitly
        // so a new variant fails compilation here).
        FragmentNode::SpecialElement(se) => {
            // Exhaustive `match` (not `matches!`) so a new `SpecialElementKind`
            // variant fails compilation here rather than silently defaulting to
            // the refused-at-emission set.
            let walk_on_emitted = match &se.kind {
                SpecialElementKind::SvelteWindow
                | SpecialElementKind::SvelteBody
                | SpecialElementKind::SvelteDocument
                | SpecialElementKind::SvelteElement { .. }
                | SpecialElementKind::SvelteHead
                | SpecialElementKind::TitleElement
                // `<svelte:boundary>` is walked UNCONDITIONALLY — including the
                // children a `pending` snippet discards. The oracle visits that
                // fragment whatever it later emits, so a `new`/prop-rooted access
                // there fires the wrapper and a `$name` store read there injects
                // `$$store_subs`, even though nothing it contains reaches output.
                | SpecialElementKind::SvelteBoundary => true,
                SpecialElementKind::SvelteComponent { .. }
                | SpecialElementKind::SvelteSelf
                | SpecialElementKind::SlotElement
                | SpecialElementKind::SvelteFragment => false,
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
fn walk_special_element(se: &SpecialElement, nc: &mut Nc<'_>) {
    let sv = nc.sv;
    if let Some(expr) = special_element_reference_expression(se) {
        walk_id(expr, nc);
    }
    // A `bind:` is two-way — it MUTATES its target, so the target's root binding
    // is reassigned component-wide (the oracle marks it at analysis time). Without
    // this a `<svelte:window bind:scrollY={y}>` would let a later `{y}` read fold to
    // its initial value where the oracle keeps it dynamic — mirrors `walk_element`.
    for attr_node in sv.attributes(se.attributes) {
        if let AttributeNode::BindDirective(d) = attr_node {
            crate::rune_guard::assign_target_roots(
                nc.view,
                nc.view.expr(d.expression),
                nc.source,
                &mut nc.reassigned,
            );
            // A `bind:` reaches the SAME oracle validator as an assignment
            // (`BindDirective.js`), so its target obeys the same three rules.
            refuse_invalid_assign_target(nc.view.expr(d.expression), nc, true);
        }
    }
    each_reference_bearing_attribute_expression(sv, sv.attributes(se.attributes), &mut |expr| {
        walk_id(expr, nc);
    });
    walk_fragment(se.fragment, nc);
}

/// A `{#snippet}` is a function-like subtree: its parameters + body locals
/// shadow the component scope, and any `new`/unsafe member/call in its body
/// still triggers the wrapper (a prop-rooted access inside a snippet fires
/// `needs_context`, and a `new` in a *hoistable* snippet body fires it too).
fn walk_snippet_block(snippet: &SnippetBlock, nc: &mut Nc<'_>) {
    nc.fn_depth += 1;
    // Parameters are `kind: 'snippet'` in the oracle (the `SnippetBlock` visitor
    // in `phases/scope.js`), so writing to one inside the body is
    // `snippet_parameter_assignment` — and, unlike the each rule, it is NOT gated
    // on runes mode.
    let mut names = Vec::new();
    let view = nc.view;
    for param in view.exprs(snippet.parameters) {
        declare_pattern(param, nc);
        walk_expr(param, nc);
        let _ = pattern_binding_names(view, param, nc.source, &mut names);
    }
    let added = enter_block_scope(&mut nc.snippet_params, names);
    walk_fragment(snippet.body, nc);
    exit_block_scope(&mut nc.snippet_params, added);
    nc.fn_depth -= 1;
}

/// A `{@render}` walks only its call arguments — the oracle visits the render
/// callee with expression metadata (not as a `CallExpression`), so a plain
/// snippet/prop callee never triggers `needs_context`; a member callee is
/// refused at emission time. Arguments are ordinary template expressions.
fn walk_render_tag(tag: &RenderTag, nc: &mut Nc<'_>) {
    // Inside a dropped `{:catch}` the "member callee refused at emission"
    // assumption doesn't hold — the callee is never emitted — so the whole render
    // expression is trigger-checked (a member-rooted callee over a prop must fire
    // the wrapper, matching the oracle).
    if nc.in_dropped_catch {
        walk_id(tag.expression, nc);
        return;
    }
    // The same (possibly-parenthesized) call unwrap the emitter uses. A non-call
    // render refuses at emission, so here it simply yields no arguments to check.
    if let Some(call) = render_call_expression(nc.view.expr(tag.expression)) {
        walk_exprs(call.arguments, nc);
    }
}

fn walk_element(element: &Element, nc: &mut Nc<'_>) {
    let sv = nc.sv;
    // A `bind:` directive is a two-way binding — it MUTATES its target, so the
    // target's root binding is reassigned component-wide (the oracle marks the
    // binding mutated at analysis time, before it decides what to emit). Without
    // this a bound `$state` would statically fold to its initial value where the
    // oracle keeps the read dynamic (`bind:group={value}` beside a `{value}`
    // interpolation). Collected for every bind — a bind tsv refuses at emission
    // makes the whole component refuse, so the mark is harmless there — and rides
    // `assign_target_roots`, which unwraps the raw template's TypeScript
    // assignment-target wrappers.
    for attr_node in sv.attributes(element.attributes) {
        if let AttributeNode::BindDirective(d) = attr_node {
            crate::rune_guard::assign_target_roots(
                nc.view,
                nc.view.expr(d.expression),
                nc.source,
                &mut nc.reassigned,
            );
            // A `bind:` reaches the SAME oracle validator as an assignment
            // (`BindDirective.js`), so its target obeys the same three rules.
            refuse_invalid_assign_target(nc.view.expr(d.expression), nc, true);
        }
    }
    // The shared traversal (`attr_refs`) defines which attribute expressions are
    // reference-bearing on the emitted path: plain attribute values on any element,
    // a `{...spread}` expression on EITHER element kind (`each_attribute_expression`
    // visits it unconditionally — a component's `$.spread_props` element and a
    // regular element's fused `$.attributes({…})` object element both emit it, so
    // `<div {...arguments}>` is walked and refuses), a `class:`/`style:`/`bind:`
    // expression on a regular element (emitted as `$.attr_class`/`$.attr_style`/an
    // inline `$.attr`), and the no-op drop family
    // (`use:`/`transition:`/`animate:`/`{@attach}`) on a regular element — dropped
    // from the tag but still walked, so a prop-rooted access inside a `use:`
    // argument still fires the wrapper. Only the refused legacy `on:`/`let:`
    // directives are skipped (their emission refusal fires). A bare directive *name*
    // never triggers `needs_context`, so the name walk is the snippet analysis's
    // alone. Inside a dropped `{:catch}` the emitter never
    // walks the fragment, so those emission refusals never fire — there a
    // `new`/prop-rooted access in any skipped position must still trigger the
    // wrapper, so every attribute reference is walked.
    if nc.in_dropped_catch {
        each_reference_bearing_attribute_expression(
            sv,
            sv.attributes(element.attributes),
            &mut |expr| walk_id(expr, nc),
        );
    } else {
        each_attribute_expression(sv, element, &mut |expr| walk_id(expr, nc));
    }
    walk_fragment(element.fragment, nc);
}

fn walk_html_tag(tag: &HtmlTag, nc: &mut Nc<'_>) {
    walk_id(tag.expression, nc);
}

fn walk_if_block(block: &IfBlock, nc: &mut Nc<'_>) {
    walk_id(block.test, nc);
    walk_fragment(block.consequent, nc);
    if let Some(alt) = &block.alternate {
        walk_fragment(*alt, nc);
    }
}

fn walk_each_block(block: &EachBlock, nc: &mut Nc<'_>) {
    walk_id(block.expression, nc);
    if let Some(key) = &block.key {
        walk_id(key.expression, nc);
    }
    // The context binding is `kind: 'each'` in the oracle (the `EachBlock` visitor
    // in `phases/scope.js`), so writing to it inside the block is
    // `each_item_invalid_assignment`. Scoped to the block's body and fallback — the
    // same extent the oracle's child scope covers. The INDEX binding is
    // deliberately absent: it is `kind: 'template'` / `'static'` with
    // `declaration_kind: 'const'` (the same visitor), so the oracle rejects a write to it as
    // `constant_assignment` — a template-scoped const, part of the residual this
    // slice leaves open (see the checklist).
    let mut each_added = Vec::new();
    if let Some(context) = block.context {
        let context = nc.view.expr(context);
        declare_pattern(context, nc);
        walk_expr(context, nc);
        let mut names = Vec::new();
        if pattern_binding_names(nc.view, context, nc.source, &mut names).is_ok() {
            each_added = enter_block_scope(&mut nc.each_items, names);
        }
    }
    let mut index_added = Vec::new();
    if let Some(index) = block.index {
        let index = index.extract(nc.source);
        nc.shadowed.insert(index.to_string());
        // `('template' | 'static', 'const')` at the `EachBlock` visitor in
        // `phases/scope.js` — so unlike the ITEM beside it (`('each', 'const')`,
        // excluded from the const rule), a write to the index is
        // `constant_assignment`. Same extent as the item: the each block's child
        // scope, covering body + fallback.
        index_added = enter_block_scope(&mut nc.template_consts, vec![index.to_string()]);
    }
    walk_fragment(block.body, nc);
    if let Some(fallback) = &block.fallback {
        walk_fragment(*fallback, nc);
    }
    exit_block_scope(&mut nc.template_consts, index_added);
    exit_block_scope(&mut nc.each_items, each_added);
}

fn walk_await_block(block: &AwaitBlock, nc: &mut Nc<'_>) {
    walk_id(block.expression, nc);
    if let Some(value) = block.value {
        declare_pattern(nc.view.expr(value), nc);
        walk_id(value, nc);
    }
    if let Some(error) = block.error {
        declare_pattern(nc.view.expr(error), nc);
        walk_id(error, nc);
    }
    // Pending/then are emitted (their skipped attribute positions refuse at
    // emission); the `{:catch}` branch is dropped, so it is walked with the
    // inclusive attribute traversal. The flag is scoped to the catch subtree.
    if let Some(pending) = &block.pending {
        walk_fragment(*pending, nc);
    }
    // The `{:then}` value and the `{:catch}` error are each declared `'const'`
    // into their OWN branch fragment's scope (the `AwaitBlock` visitor in
    // `phases/scope.js`), so a write to one is `constant_assignment`,
    // scoped to that branch alone.
    if let Some(then) = &block.then {
        let names = template_const_names(nc.view, block.value, nc.source);
        let added = enter_block_scope(&mut nc.template_consts, names);
        walk_fragment(*then, nc);
        exit_block_scope(&mut nc.template_consts, added);
    }
    if let Some(catch) = &block.catch {
        let names = template_const_names(nc.view, block.error, nc.source);
        let added = enter_block_scope(&mut nc.template_consts, names);
        let prev = nc.in_dropped_catch;
        nc.in_dropped_catch = true;
        walk_fragment(*catch, nc);
        nc.in_dropped_catch = prev;
        exit_block_scope(&mut nc.template_consts, added);
    }
}

fn walk_key_block(block: &KeyBlock, nc: &mut Nc<'_>) {
    walk_id(block.expression, nc);
    walk_fragment(block.fragment, nc);
}

fn walk_const_tag(tag: &ConstTag, nc: &mut Nc<'_>) {
    declare_pattern(nc.view.expr(tag.id), nc);
    walk_id(tag.id, nc);
    walk_id(tag.init, nc);
}
